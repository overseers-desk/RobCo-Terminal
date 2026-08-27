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
//! * **Unknown**: no key is recorded for the host -- put the fingerprint on
//!   the glass and ask, in full words, whether to accept and record it.
//!   Accepting a first key is a trust decision and belongs to the person at
//!   the terminal; the terminal's job is to make sure they are shown what
//!   they are deciding about. `yes` records the key in the user's own
//!   `known_hosts` and connects; anything else refuses.
//! * **Mismatch**: a key is recorded and the presented one differs --
//!   refuse, always, both fingerprints and the offending line shown. No
//!   question is asked, because there is no answer that should change it.
//!
//! Only the user's own file is ever written. The machine-wide trust file
//! (`/etc/ssh/ssh_known_hosts`, or `%ProgramData%\ssh\ssh_known_hosts`) is
//! read and never written: it is the administrator's statement about the
//! machine, and one user answering a prompt is not an administrator.
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
use ssh_link::{Answer, Asker, ChannelHandle, HostPolicy, WireEvent};
use term::ssh_channel::{SshEvent, SshWire};

/// The environment variable that names the invoking user on this
/// platform: what `ssh` itself falls back on when no user is spelled.
pub(crate) const USER_VAR: &str = if cfg!(windows) { "USERNAME" } else { "USER" };

/// The invoking user per [`USER_VAR`], an empty value read as unset.
pub(crate) fn invoking_user() -> Option<String> {
    std::env::var(USER_VAR).ok().filter(|u| !u.is_empty())
}

/// A destination as the command line spells it: `[user@]host[:port]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshRequest {
    pub user: String,
    pub host: String,
    pub port: u16,
    /// The private key the `[[ssh.host]]` row names, `~`-expanded. `None`
    /// leaves the transport to the agent and the default key files.
    pub key: Option<std::path::PathBuf>,
}

/// A row's `key` as a path: empty is `None`, and a leading `~/` is the
/// invoking user's home, the one spelling `ssh` accepts that the
/// filesystem does not.
pub(crate) fn key_path(row_key: &str) -> Option<std::path::PathBuf> {
    if row_key.is_empty() {
        return None;
    }
    if let Some(rest) = row_key.strip_prefix("~/") {
        if let Some(home) = std::env::home_dir() {
            return Some(home.join(rest));
        }
    }
    Some(std::path::PathBuf::from(row_key))
}

impl SshRequest {
    /// Parse `[user@]host[:port]`. The user defaults to the invoking
    /// user's name and the port to 22, which is what the same spelling
    /// means to `ssh` itself.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let (user, rest) = match spec.split_once('@') {
            Some((user, rest)) if !user.is_empty() => (user.to_string(), rest),
            Some(_) => return Err(format!("no user before the '@' in '{spec}'")),
            None => match invoking_user() {
                Some(user) => (user, spec),
                None => return Err(format!("no user in '{spec}' and {USER_VAR} is unset")),
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
        // The spelling carries no key; a `[[ssh.host]]` row is where one
        // is named.
        Ok(Self { user: user.to_string(), host: host.to_string(), port, key: None })
    }
}

/// The `[ssh]` table's default connection, or `None` for localhost. A
/// default naming no row, or a bare row with no invoking user in the
/// environment, is logged and treated as `None`: never an error, and
/// never a blocked window.
pub fn default_request(cfg: &config::Config) -> Option<SshRequest> {
    if cfg.ssh.default.is_empty() {
        return None;
    }
    let Some(row) = cfg.ssh.hosts.iter().find(|h| h.host == cfg.ssh.default) else {
        log::warn!(
            "[ssh] default {:?} matches no [[ssh.host]] row; starting local",
            cfg.ssh.default
        );
        return None;
    };
    let user = if row.user.is_empty() {
        match invoking_user() {
            Some(user) => user,
            None => {
                log::warn!(
                    "[[ssh.host]] {:?} names no user and {USER_VAR} is unset; starting local",
                    row.host
                );
                return None;
            }
        }
    } else {
        row.user.clone()
    };
    Some(SshRequest {
        user,
        host: row.host.clone(),
        port: row.port,
        key: key_path(&row.key),
    })
}

/// The machine-wide trust file, where this platform's OpenSSH keeps it:
/// `%ProgramData%\ssh\ssh_known_hosts` on Windows, `/etc/ssh/ssh_known_hosts`
/// elsewhere.
fn system_known_hosts() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("ProgramData")
            .map(|d| PathBuf::from(d).join("ssh").join("ssh_known_hosts"))
    } else {
        Some(PathBuf::from("/etc/ssh/ssh_known_hosts"))
    }
}

/// The files trust is read from: the user's, then the machine's.
fn known_hosts_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(home) = std::env::home_dir() {
        files.push(home.join(".ssh").join("known_hosts"));
    }
    if let Some(system) = system_known_hosts().filter(|p| p.exists()) {
        files.push(system);
    }
    files
}

/// The one file an accepted key may be written to: the user's own. The
/// machine-wide file is read above and never appears here.
fn learnable_known_hosts() -> Option<PathBuf> {
    std::env::home_dir().map(|home| home.join(".ssh").join("known_hosts"))
}

fn sha256(key: &ssh_link::russh::keys::PublicKey) -> Fingerprint {
    key.fingerprint(HashAlg::Sha256)
}

/// The `known_hosts` policy described in the module doc.
pub struct KnownHosts {
    files: Vec<PathBuf>,
    /// Where an accepted first key is recorded, and the only file this
    /// program ever writes. `None` -- no home directory, nowhere to put
    /// it -- makes an unknown host a refusal with nothing to ask about.
    learn_into: Option<PathBuf>,
}

impl KnownHosts {
    pub fn new() -> Self {
        Self { files: known_hosts_files(), learn_into: learnable_known_hosts() }
    }

    /// For tests: the same policy over explicit files. The first is the
    /// one an accepted key is recorded in, mirroring the real order where
    /// the user's own file leads and the machine's follows it read-only.
    pub fn over(files: Vec<PathBuf>) -> Self {
        let learn_into = files.first().cloned();
        Self { files, learn_into }
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
        ask: &Asker,
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
        self.first_key(host, port, key, ask)
    }
}

impl KnownHosts {
    /// The unknown-host branch: show what is being decided, ask for it in
    /// full words, and record only on `yes`.
    ///
    /// The full word is OpenSSH's rule and it is kept for OpenSSH's reason.
    /// A single keystroke is what a hand does while the eye is elsewhere,
    /// and the eye has to be here: the fingerprint above the question is
    /// the whole of the evidence. Anything that is not an answer re-asks,
    /// so the way past this is to answer it. Capitals are not part of the
    /// friction, so `YES` is a yes.
    fn first_key(
        &self,
        host: &str,
        port: u16,
        key: &ssh_link::russh::keys::PublicKey,
        ask: &Asker,
    ) -> Result<(), String> {
        let refused =
            || format!("the host key {} for {host} was not accepted; connection refused", sha256(key));
        let Some(path) = &self.learn_into else {
            return Err(format!(
                "no host key is recorded for {host} (key: {}) and there is no known_hosts \
                 file to record one in; connection refused",
                sha256(key)
            ));
        };
        let mut question = format!(
            "The authenticity of {host} port {port} cannot be established.\n\
             Its {} key fingerprint is {}.\n\
             Type yes to accept and record it, no to refuse: ",
            key.algorithm().as_str(),
            sha256(key)
        );
        loop {
            let Some(answer) = ask.ask(question.clone(), Answer::YesNo) else {
                return Err(refused());
            };
            let answer = answer.trim();
            if answer.eq_ignore_ascii_case("yes") {
                if let Err(e) = known_hosts::learn_known_hosts_path(host, port, key, path) {
                    // The user made the decision; a file this program could
                    // not write is this program's problem, not a reason to
                    // put the decision back to them. It holds for the
                    // session and is asked again next time.
                    ask.say(format!(
                        "the key was accepted but could not be recorded in {}: {e}; \
                         it holds for this session only",
                        path.display()
                    ));
                }
                return Ok(());
            }
            if answer.eq_ignore_ascii_case("no") {
                return Err(refused());
            }
            question = "Please type 'yes' or 'no': ".to_string();
        }
    }
}

/// How the transport looks when it talks about itself: dim, bracketed, on
/// a line of its own, and in scrollback like everything else that ever
/// happened on the channel.
///
/// It lives as a function because the same look now arrives by two
/// carriers. A `WireEvent::Notice` comes up the channel's own wire; an
/// `Ask::Say` comes across the question desk, from a policy that was
/// speaking from a stack with no wire in reach. The user should not be
/// able to tell which road a line took, so there is one place that decides
/// how a line looks and two places that call it.
pub(crate) fn notice_bytes(text: &str) -> Vec<u8> {
    format!("\r\n\x1b[2m[ssh: {text}]\x1b[0m\r\n").into_bytes()
}

/// The transport's endpoints wearing the session's trait.
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
            WireEvent::Notice(text) => SshEvent::Notice(notice_bytes(&text)),
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
            SshRequest { user: "overseer".into(), host: "vault".into(), port: 2222, key: None }
        );
        assert_eq!(run("overseer@vault").unwrap().port, 22);
        std::env::set_var(USER_VAR, "resident");
        assert_eq!(run("vault").unwrap().user, "resident");
        assert!(run("@vault").is_err());
        assert!(run("overseer@").is_err());
        assert!(run("vault:notaport").is_err());
    }

    #[test]
    fn a_rows_key_becomes_a_path_and_tilde_means_home() {
        assert_eq!(key_path(""), None);
        assert_eq!(key_path("/etc/key"), Some(std::path::PathBuf::from("/etc/key")));
        let home = std::env::home_dir().unwrap();
        assert_eq!(key_path("~/.ssh/id_gw"), Some(home.join(".ssh").join("id_gw")));
    }

    #[test]
    fn the_configured_default_becomes_a_request_and_a_stale_one_does_not() {
        let mut cfg = config::Config::default();
        assert_eq!(default_request(&cfg), None, "no default, no dialling");

        cfg.ssh.hosts.push(config::SshHost {
            host: "vault".into(),
            user: "overseer".into(),
            port: 2222,
            key: String::new(),
        });
        cfg.ssh.default = "vault".into();
        assert_eq!(
            default_request(&cfg),
            Some(SshRequest {
                user: "overseer".into(),
                host: "vault".into(),
                port: 2222,
                key: None
            })
        );

        cfg.ssh.default = "gone".into();
        assert_eq!(default_request(&cfg), None, "a stale default costs a log line, not a window");

        // A bare row takes the invoking user's name.
        cfg.ssh.default = "vault".into();
        cfg.ssh.hosts[0].user = String::new();
        std::env::set_var(USER_VAR, "resident");
        assert_eq!(default_request(&cfg).unwrap().user, "resident");
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

    fn present(k: &PrivateKey) -> PublicKeyOrCertificate {
        PublicKeyOrCertificate::PublicKey { key: k.public_key().clone(), hash_alg: None }
    }

    /// Run `body` with a desk behind it, answering its questions from
    /// `answers` in order and cancelling once the script runs out. Answers
    /// the body's result and the transcript: every prompt asked and every
    /// line said, in the order the glass would have shown them.
    ///
    /// The desk is driven from the test's own thread and the policy runs on
    /// another, which is the real arrangement upside down -- the policy is
    /// the one on a thread of its own in a running program. It has to be
    /// one or the other: `ask` blocks, so whoever asks and whoever answers
    /// cannot be the same thread, and this is the half a test can assert on.
    fn asked<T: Send>(
        answers: &[&str],
        body: impl FnOnce(&Asker) -> T + Send,
    ) -> (T, Vec<String>) {
        let (asker, mut desk) = ssh_link::ask::desk();
        let mut transcript: Vec<String> = Vec::new();
        let mut answers = answers.iter();
        let out = std::thread::scope(|scope| {
            let running = scope.spawn(move || body(&asker));
            loop {
                match desk.take() {
                    Some(ssh_link::Ask::Question(question)) => {
                        transcript.push(question.prompt().to_string());
                        match answers.next() {
                            Some(answer) => question.answer((*answer).to_string()),
                            None => question.cancel(),
                        }
                    }
                    Some(ssh_link::Ask::Say(text)) => transcript.push(text),
                    None if running.is_finished() => {
                        // Finished means every send it was going to make
                        // has been made, so one more sweep empties the desk.
                        while let Some(ask) = desk.take() {
                            match ask {
                                ssh_link::Ask::Question(question) => {
                                    transcript.push(question.prompt().to_string());
                                    question.cancel();
                                }
                                ssh_link::Ask::Say(text) => transcript.push(text),
                            }
                        }
                        break;
                    }
                    None => std::thread::sleep(std::time::Duration::from_millis(2)),
                }
            }
            running.join().unwrap()
        });
        (out, transcript)
    }

    #[test]
    fn a_first_key_is_recorded_when_the_user_types_yes_and_never_otherwise() {
        let dir = tempfile::tempdir().unwrap();
        let vault = key(Algorithm::Ed25519);
        let file = write_lines(dir.path(), "known_hosts", &[]);
        let lines = || std::fs::read_to_string(&file).unwrap();

        // Anything that is not an answer re-asks, with the nag; then yes.
        let (verdict, transcript) = asked(&["y", "maybe", "yes"], |ask| {
            KnownHosts::over(vec![file.clone()]).verify("vault", 2222, &present(&vault), ask)
        });
        assert!(verdict.is_ok(), "{verdict:?}");
        assert!(transcript[0].contains("authenticity of vault port 2222"), "{transcript:?}");
        assert!(transcript[0].contains("SHA256:"), "{transcript:?}");
        assert!(transcript[0].contains("ssh-ed25519 key fingerprint"), "{transcript:?}");
        assert_eq!(transcript.len(), 3, "a single letter is not a full word: {transcript:?}");
        assert!(transcript[1].contains("'yes' or 'no'"), "{transcript:?}");

        // The key is in the file, and the same key now matches without a
        // question being asked at all.
        assert!(lines().contains("[vault]:2222"), "{:?}", lines());
        let (verdict, transcript) = asked(&[], |ask| {
            KnownHosts::over(vec![file.clone()]).verify("vault", 2222, &present(&vault), ask)
        });
        assert!(verdict.is_ok());
        assert!(transcript.is_empty(), "a recorded host is not asked about: {transcript:?}");

        // `no` refuses, and nothing is written.
        let stranger = key(Algorithm::Ed25519);
        let before = lines();
        let (verdict, _) = asked(&["no"], |ask| {
            KnownHosts::over(vec![file.clone()]).verify("elsewhere", 22, &present(&stranger), ask)
        });
        let text = verdict.unwrap_err();
        assert!(text.contains("was not accepted"), "{text}");
        assert_eq!(lines(), before, "a refusal writes nothing");

        // Nobody to ask is a refusal too, and the refusal names no tool
        // the user would have to leave the glass to run.
        let text = KnownHosts::over(vec![file.clone()])
            .verify("elsewhere", 22, &present(&stranger), &Asker::closed())
            .unwrap_err();
        assert!(!text.contains("ssh -p"), "{text}");
        assert_eq!(lines(), before);
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
        // Nobody at the desk throughout: every outcome below is one this
        // policy reaches without a human, and an unknown host under a
        // closed asker refuses rather than waits.
        let nobody = Asker::closed();

        // Match, including the port-qualified and two-key spellings.
        assert!(policy.verify("vault", 22, &present(&vault), &nobody).is_ok());
        assert!(policy.verify("odd", 2222, &present(&odd), &nobody).is_ok());
        assert!(policy.verify("plural", 22, &present(&rsa), &nobody).is_ok());

        // Unknown, with nobody to accept it: refused, and the refusal names
        // the key rather than a command to go and run somewhere else.
        let text = policy.verify("nowhere", 22, &present(&vault), &nobody).unwrap_err();
        assert!(text.contains("SHA256:"), "{text}");
        assert!(!text.contains("ssh -p"), "{text}");

        // Mismatch: refused unconditionally, nothing asked, the file and
        // line named.
        let text = policy.verify("vault", 22, &present(&stranger), &nobody).unwrap_err();
        assert!(text.contains("HOST KEY CHANGED"), "{text}");
        assert!(text.contains("known_hosts:1"), "{text}");

        // The glob ceiling: literal comparison, spurious refusal.
        assert!(policy.verify("city.wasteland", 22, &present(&vault), &nobody).is_err());

        // A revoked key is refused wherever it turns up.
        let text = policy.verify("elsewhere", 22, &present(&burned), &nobody).unwrap_err();
        assert!(text.contains("revoked"), "{text}");

        // The recorded algorithms lead the preference order.
        let order = policy.key_order("plural", 22).unwrap();
        assert_eq!(order[0], Algorithm::Ed25519);
        assert!(order.contains(&Algorithm::Rsa { hash: None }));
        assert_eq!(policy.key_order("nowhere", 22), None);
    }
}
