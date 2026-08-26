//! The SSH policy this program applies, and the adapter between the
//! transport (`ssh_link`) and the session variant (`term::ssh_channel`).
//!
//! The two crates deliberately cannot see each other; this module is where
//! they meet, the role `crate::tmux` plays for the gateway. What lives here
//! is everything with an opinion in it: how a destination is spelled on the
//! command line, how a notice looks on the glass, and above all what this
//! program trusts.
//!
//! # Host keys, three outcomes
//!
//! * **Match**: the presented key is recorded for the host -- connect.
//! * **Unknown**: no key is recorded for the host -- refuse, and print the
//!   fingerprint with the command that records it. Accepting a first key
//!   is a trust decision; a build with no prompt would be making it *for*
//!   the user, so the honest substitute is a refusal that says what to
//!   type. `ssh` itself is that prompt, on every box this program runs on.
//! * **Mismatch**: a key is recorded and the presented one differs --
//!   refuse, always, both fingerprints and the offending line shown.
//!
//! `known_hosts` is read-only to this program: nothing here calls
//! `learn_known_hosts`, which is the no-trust-on-first-use decision as a
//! code property. The prompted acceptance path arrives with the operator
//! interface (#14), which puts a human back in the loop.
//!
//! Ceilings of the reader (russh's, plus this module's pre-pass), each of
//! which under a refuse-by-default policy costs a spurious refusal and
//! never a false accept: glob and negation host patterns are compared
//! literally; `@cert-authority` lines never match (certificate host keys
//! are refused outright); a tab-separated line does not parse. `@revoked`
//! gets better than the ceiling: a pre-pass refuses any presented key a
//! revocation line names, whatever host it is filed under.

use std::path::PathBuf;

use ssh_link::russh::keys::ssh_key::known_hosts::Entry;
use ssh_link::russh::keys::ssh_key::{Fingerprint, HashAlg};
use ssh_link::russh::keys::{known_hosts, Algorithm, Error as KeysError, PublicKeyOrCertificate};
use ssh_link::{ChannelHandle, HostPolicy, WireEvent};
use term::ssh_channel::{SshEvent, SshWire};

/// A destination as the command line spells it: `[user@]host[:port]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshRequest {
    pub user: String,
    pub host: String,
    pub port: u16,
}

impl SshRequest {
    /// Parse `[user@]host[:port]`. The user defaults to `$USER` and the
    /// port to 22, which is what the same spelling means to `ssh` itself.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let (user, rest) = match spec.split_once('@') {
            Some((user, rest)) if !user.is_empty() => (user.to_string(), rest),
            Some(_) => return Err(format!("no user before the '@' in '{spec}'")),
            None => match std::env::var("USER") {
                Ok(user) if !user.is_empty() => (user, spec),
                _ => return Err(format!("no user in '{spec}' and $USER is unset")),
            },
        };
        let (host, port) = match rest.rsplit_once(':') {
            Some((host, port)) => {
                let port = port
                    .parse::<u16>()
                    .map_err(|_| format!("'{port}' is not a port number"))?;
                (host, port)
            }
            None => (rest, 22),
        };
        if host.is_empty() {
            return Err(format!("no host in '{spec}'"));
        }
        Ok(Self { user: user.to_string(), host: host.to_string(), port })
    }
}

/// The files trust is read from: the user's, then the machine's.
fn known_hosts_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(home) = std::env::home_dir() {
        files.push(home.join(".ssh").join("known_hosts"));
    }
    let system = PathBuf::from("/etc/ssh/ssh_known_hosts");
    if system.exists() {
        files.push(system);
    }
    files
}

fn sha256(key: &ssh_link::russh::keys::PublicKey) -> Fingerprint {
    key.fingerprint(HashAlg::Sha256)
}

/// The refuse-by-default `known_hosts` policy described in the module doc.
pub struct KnownHosts {
    files: Vec<PathBuf>,
}

impl KnownHosts {
    pub fn new() -> Self {
        Self { files: known_hosts_files() }
    }

    /// For tests: the same policy over explicit files.
    pub fn over(files: Vec<PathBuf>) -> Self {
        Self { files }
    }

    /// Any `@revoked` line naming this key, in any file, under any host:
    /// the key itself is what was revoked, so no host matching is owed.
    fn revoked(&self, key: &ssh_link::russh::keys::PublicKey) -> bool {
        self.files.iter().any(|file| {
            let Ok(text) = std::fs::read_to_string(file) else {
                return false;
            };
            text.lines()
                .filter(|l| l.trim_start().starts_with("@revoked"))
                .filter_map(|l| l.trim().parse::<Entry>().ok())
                .any(|entry| entry.public_key() == key)
        })
    }
}

impl Default for KnownHosts {
    fn default() -> Self {
        Self::new()
    }
}

impl HostPolicy for KnownHosts {
    fn key_order(&mut self, host: &str, port: u16) -> Option<Vec<Algorithm>> {
        let mut order: Vec<Algorithm> = Vec::new();
        for file in &self.files {
            let Ok(keys) = known_hosts::known_host_keys_path(host, port, file) else {
                continue;
            };
            for (_, key) in keys {
                if !order.contains(&key.algorithm()) {
                    order.push(key.algorithm());
                }
            }
        }
        (!order.is_empty()).then_some(order)
    }

    fn verify(
        &mut self,
        host: &str,
        port: u16,
        key: &PublicKeyOrCertificate,
    ) -> Result<(), String> {
        let key = match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key,
            PublicKeyOrCertificate::Certificate(_) => {
                return Err(format!(
                    "{host} presented a certificate host key, which this build does not \
                     verify; connection refused"
                ));
            }
        };
        if self.revoked(key) {
            return Err(format!(
                "the host key {} for {host} is revoked in known_hosts; connection refused",
                sha256(key)
            ));
        }
        for file in &self.files {
            match known_hosts::check_known_hosts_path(host, port, key, file) {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(KeysError::KeyChanged { line }) => {
                    let recorded = known_hosts::known_host_keys_path(host, port, file)
                        .ok()
                        .and_then(|keys| keys.into_iter().find(|(l, _)| *l == line))
                        .map(|(_, key)| sha256(&key).to_string())
                        .unwrap_or_else(|| "unreadable".into());
                    return Err(format!(
                        "HOST KEY CHANGED for {host}: {recorded} is recorded at {}:{line}, \
                         but the server presented {}; someone could be eavesdropping. \
                         Connection refused; if the change is real, remove that line and \
                         reconnect.",
                        file.display(),
                        sha256(key)
                    ));
                }
                Err(_) => {}
            }
        }
        Err(format!(
            "no host key is recorded for {host} (key: {}); this build does not accept \
             first keys itself. To record it, run: ssh -p {port} {host} exit \
             -- then reconnect here.",
            sha256(key)
        ))
    }
}

/// The transport's endpoints wearing the session's trait, with the one
/// presentational decision this side owns: how a transport notice looks
/// on the glass. Dim, bracketed, on its own line, and in scrollback like
/// everything else that ever happened on the channel.
pub struct WireAdapter {
    handle: ChannelHandle,
}

impl WireAdapter {
    pub fn new(handle: ChannelHandle) -> Self {
        Self { handle }
    }
}

impl SshWire for WireAdapter {
    fn try_event(&mut self) -> Option<SshEvent> {
        Some(match self.handle.try_event()? {
            WireEvent::Data(bytes) => SshEvent::Data(bytes),
            WireEvent::Notice(text) => {
                SshEvent::Notice(format!("\r\n\x1b[2m[ssh: {text}]\x1b[0m\r\n").into_bytes())
            }
            WireEvent::ExitStatus(status) => SshEvent::ExitStatus(status),
            WireEvent::Eof => SshEvent::Eof,
        })
    }

    fn send(&mut self, bytes: &[u8]) {
        self.handle.send(bytes);
    }

    fn window_change(&mut self, cols: u16, rows: u16, pix_w: u16, pix_h: u16) {
        self.handle.window_change(cols, rows, pix_w, pix_h);
    }

    fn sheds(&self) -> u64 {
        self.handle.sheds()
    }

    fn writer(&self) -> Box<dyn std::io::Write + Send> {
        Box::new(self.handle.writer())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssh_link::russh::keys::ssh_key::private::PrivateKey;
    use std::io::Write;

    fn run(spec: &str) -> Result<SshRequest, String> {
        SshRequest::parse(spec)
    }

    #[test]
    fn a_destination_is_spelled_the_way_ssh_spells_one() {
        let full = run("overseer@vault:2222").unwrap();
        assert_eq!(
            full,
            SshRequest { user: "overseer".into(), host: "vault".into(), port: 2222 }
        );
        assert_eq!(run("overseer@vault").unwrap().port, 22);
        std::env::set_var("USER", "resident");
        assert_eq!(run("vault").unwrap().user, "resident");
        assert!(run("@vault").is_err());
        assert!(run("overseer@").is_err());
        assert!(run("vault:notaport").is_err());
    }

    fn key(alg: Algorithm) -> PrivateKey {
        PrivateKey::random(&mut rand::rng(), alg).unwrap()
    }

    fn write_lines(dir: &std::path::Path, name: &str, lines: &[String]) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
        path
    }

    fn entry(host: &str, key: &PrivateKey) -> String {
        format!("{host} {}", key.public_key().to_openssh().unwrap())
    }

    #[test]
    fn the_three_outcomes_and_the_ceilings() {
        let dir = tempfile::tempdir().unwrap();
        let vault = key(Algorithm::Ed25519);
        let odd = key(Algorithm::Ed25519);
        let burned = key(Algorithm::Ed25519);
        let stranger = key(Algorithm::Ed25519);
        let rsa = key(Algorithm::Rsa { hash: None });
        let file = write_lines(
            dir.path(),
            "known_hosts",
            &[
                entry("vault", &vault),
                entry("[odd]:2222", &odd),
                // Two keys for one host: any match wins.
                entry("plural", &vault),
                entry("plural", &rsa),
                // A glob is compared literally by the reader: a ceiling,
                // and under refuse-by-default a refusal, never an accept.
                entry("*.wasteland", &vault),
                format!("@revoked {}", entry("burned", &burned)),
                format!("@cert-authority {}", entry("authority", &vault)),
            ],
        );
        let mut policy = KnownHosts::over(vec![file.clone()]);
        let present = |k: &PrivateKey| PublicKeyOrCertificate::PublicKey {
            key: k.public_key().clone(),
            hash_alg: None,
        };

        // Match, including the port-qualified and two-key spellings.
        assert!(policy.verify("vault", 22, &present(&vault)).is_ok());
        assert!(policy.verify("odd", 2222, &present(&odd)).is_ok());
        assert!(policy.verify("plural", 22, &present(&rsa)).is_ok());

        // Unknown: refused, fingerprint and the recording command named.
        let text = policy.verify("nowhere", 22, &present(&vault)).unwrap_err();
        assert!(text.contains("SHA256:"), "{text}");
        assert!(text.contains("ssh -p 22 nowhere"), "{text}");

        // Mismatch: refused, the file and line named.
        let text = policy.verify("vault", 22, &present(&stranger)).unwrap_err();
        assert!(text.contains("HOST KEY CHANGED"), "{text}");
        assert!(text.contains("known_hosts:1"), "{text}");

        // The glob ceiling: literal comparison, spurious refusal.
        assert!(policy.verify("city.wasteland", 22, &present(&vault)).is_err());

        // A revoked key is refused wherever it turns up.
        let text = policy.verify("elsewhere", 22, &present(&burned)).unwrap_err();
        assert!(text.contains("revoked"), "{text}");

        // The recorded algorithms lead the preference order.
        let order = policy.key_order("plural", 22).unwrap();
        assert_eq!(order[0], Algorithm::Ed25519);
        assert!(order.contains(&Algorithm::Rsa { hash: None }));
        assert_eq!(policy.key_order("nowhere", 22), None);
    }
}
