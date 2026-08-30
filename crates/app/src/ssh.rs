//! The SSH policy this program applies, and the adapter between the
//! transport (`ssh_link`) and the session variant (`term::ssh_channel`).
//!
//! The two crates deliberately cannot see each other; this module is where
//! they meet, the role `crate::tmux` plays for the gateway. What lives here
//! is everything with an opinion in it: how a destination is spelled on the
//! command line, how a notice looks on the glass, and above all what this
//! program trusts.
//!
//! What `~/.ssh/config` is allowed to say about a destination is next
//! door, in [`crate::ssh_config`]; this module holds the precedence, which
//! is `ssh`'s own: an explicit field on the `[[ssh.host]]` row or in the
//! `--ssh` spelling outranks the file, and the file fills only what was
//! left unsaid ([`Unsaid`]).
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
///
/// Public because a test that means to leave a user unsaid has to know
/// which name the fallback will read, and there is one right answer per
/// platform rather than one per test.
pub const USER_VAR: &str = if cfg!(windows) { "USERNAME" } else { "USER" };

/// The invoking user per [`USER_VAR`], an empty value read as unset.
pub(crate) fn invoking_user() -> Option<String> {
    std::env::var(USER_VAR).ok().filter(|u| !u.is_empty())
}

/// Which of a destination's fields the operator actually spelled, as
/// against the ones this filled in for them.
///
/// It is what `~/.ssh/config` is measured against: the file fills what was
/// left unsaid and never outranks what was said, which is the precedence
/// `ssh` itself applies to its own command line. Everything said by
/// default, which is what [`Default`] gives, is a destination the file may
/// only translate (`HostName`) and never re-address.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unsaid {
    /// No user in the spelling and none in the row: the name in `user` is
    /// the invoking user's, and a file naming one outranks it.
    pub user: bool,
    /// No port in the spelling, or a `[[ssh.host]]` row's own default. The
    /// row's port is 22 whether the file said so or said nothing, and the
    /// config crate reads an absent port as "ssh's own", so 22 there is
    /// this program filling a gap rather than the operator naming a port.
    pub port: bool,
}

/// A destination as the command line spells it: `[user@]host[:port]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshRequest {
    pub user: String,
    pub host: String,
    pub port: u16,
    /// The private keys to offer ahead of the agent: the one a
    /// `[[ssh.host]]` row names (`~`-expanded), or the `IdentityFile`
    /// list `~/.ssh/config` gives for this destination. Empty leaves the
    /// transport to the agent and the default key files.
    pub keys: Vec<std::path::PathBuf>,
    /// What the operator left for something else to fill.
    pub unsaid: Unsaid,
    /// What the resolution has to say about this destination on the
    /// channel's own glass, said the moment the channel stands. Set when
    /// `~/.ssh/config` had counsel this build could not honour, which is
    /// the one thing about a connection's address the user has to be told
    /// out loud (see [`crate::ssh_config`]).
    pub notice: Option<String>,
}

/// A row's `key` as a path list: empty names nothing, and a leading `~/`
/// is the invoking user's home, the one spelling `ssh` accepts that the
/// filesystem does not.
pub(crate) fn key_path(row_key: &str) -> Vec<std::path::PathBuf> {
    if row_key.is_empty() {
        return Vec::new();
    }
    if let Some(rest) = row_key.strip_prefix("~/") {
        if let Some(home) = std::env::home_dir() {
            return vec![home.join(rest)];
        }
    }
    vec![std::path::PathBuf::from(row_key)]
}

/// The host and the port out of `host`, `host:port`, `[host]` or
/// `[host]:port`, which is the whole of the grammar `ssh` reads here.
///
/// The brackets are what an address made of colons needs before a colon
/// can also mean "port", so they are read wherever OpenSSH reads them and
/// written back by [`SshRequest::spec`] under the same condition. Bare, an
/// address of more than one colon is itself and carries no port: nothing
/// else it could be, and reading its last group as a port is how
/// `2001:db8::1` became a name that no resolver has ever heard of.
fn split_host_port<'a>(rest: &'a str, spec: &str) -> Result<(&'a str, Option<u16>), String> {
    let port = |text: &str| {
        text.parse::<u16>()
            .map_err(|_| format!("'{text}' is not a port number"))
    };
    if let Some(inside) = rest.strip_prefix('[') {
        let Some((host, after)) = inside.split_once(']') else {
            return Err(format!("no closing ']' in '{spec}'"));
        };
        return match after {
            "" => Ok((host, None)),
            _ => match after.strip_prefix(':') {
                Some(text) => Ok((host, Some(port(text)?))),
                None => Err(format!("'{after}' follows the ']' in '{spec}'")),
            },
        };
    }
    if rest.matches(':').count() > 1 {
        return Ok((rest, None));
    }
    match rest.rsplit_once(':') {
        Some((host, text)) => Ok((host, Some(port(text)?))),
        None => Ok((rest, None)),
    }
}

impl SshRequest {
    /// Parse `[user@]host[:port]`, the host bracketed as `[host]` where it
    /// is an address of its own colons. The user defaults to the invoking
    /// user's name and the port to 22, which is what the same spelling
    /// means to `ssh` itself -- and both defaults are recorded as unsaid,
    /// because a default is a gap `~/.ssh/config` is entitled to fill.
    ///
    /// No file is opened here. This is also what validates a `--ssh`
    /// argument and a destination typed into the picker, and neither of
    /// those is a connection: the file is asked once, on the way to the
    /// wire.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let (user, rest) = match spec.split_once('@') {
            Some((user, rest)) if !user.is_empty() => (Some(user.to_string()), rest),
            Some(_) => return Err(format!("no user before the '@' in '{spec}'")),
            None => (None, spec),
        };
        let (host, port) = split_host_port(rest, spec)?;
        if host.is_empty() {
            return Err(format!("no host in '{spec}'"));
        }
        let unsaid = Unsaid { user: user.is_none(), port: port.is_none() };
        let user = match user.or_else(invoking_user) {
            Some(user) => user,
            // Nothing spelled and nothing in the environment. The file may
            // yet name a user, but a destination that reaches the wire
            // without one is not a destination, and the error is owed to
            // whoever typed the spelling rather than deferred to a file
            // they may not have.
            None => return Err(format!("no user in '{spec}' and {USER_VAR} is unset")),
        };
        // The spelling carries no key; a `[[ssh.host]]` row and the
        // config file's `IdentityFile` are where one is named.
        Ok(Self {
            user,
            host: host.to_string(),
            port: port.unwrap_or(22),
            keys: Vec::new(),
            unsaid,
            notice: None,
        })
    }

    /// The destination spelled the way [`parse`](Self::parse) reads one,
    /// leaving unsaid what was unsaid: spelling in the invoking user's name
    /// here would have `~/.ssh/config` answering a question nobody asked.
    ///
    /// The spelling is the grammar's writer, and the tests hold it against
    /// [`parse`](Self::parse) so the round trip keeps a gap a gap. Requests
    /// themselves travel typed (`ShellConfig::ssh`), because the spelling
    /// cannot carry a key file.
    pub fn spec(&self) -> String {
        let mut spec = String::new();
        if !self.unsaid.user {
            spec.push_str(&self.user);
            spec.push('@');
        }
        // A port after an address of colons needs the brackets to be a
        // port at all; without one the address stands as it is written.
        if self.host.contains(':') && !self.unsaid.port {
            spec.push_str(&format!("[{}]", self.host));
        } else {
            spec.push_str(&self.host);
        }
        if !self.unsaid.port {
            spec.push_str(&format!(":{}", self.port));
        }
        spec
    }

    /// Take what `~/.ssh/config` has to say about this destination.
    ///
    /// Whole or not at all: counsel this build can honour is applied to
    /// every field the operator left unsaid, and counsel it cannot is
    /// taken nowhere and said out loud instead, the connection going where
    /// the destination was spelled. `HostName` is the exception that is
    /// not one -- it replaces a host that *was* said, because `Host` names
    /// a lookup rather than a machine, which is the whole reason the file
    /// is read.
    pub fn take_counsel(&mut self, counsel: crate::ssh_config::Counsel) {
        match counsel {
            crate::ssh_config::Counsel::Silent => {}
            crate::ssh_config::Counsel::Refused(notice) => self.notice = Some(notice),
            crate::ssh_config::Counsel::Says(says) => {
                if let Some(host_name) = says.host_name {
                    self.host = host_name;
                }
                if self.unsaid.user {
                    if let Some(user) = says.user {
                        self.user = user;
                        self.unsaid.user = false;
                    }
                }
                if self.unsaid.port {
                    if let Some(port) = says.port {
                        self.port = port;
                        self.unsaid.port = false;
                    }
                }
                if self.keys.is_empty() {
                    self.keys = says.identity_files;
                }
            }
        }
    }

    /// The same over the user's own file, which is where the connect path
    /// asks it: once, with the destination as the operator spelled it.
    pub fn consult_ssh_config(&mut self) {
        let Some(path) = crate::ssh_config::home_file() else {
            return;
        };
        self.take_counsel(crate::ssh_config::read(&path, &self.host));
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
        keys: key_path(&row.key),
        // An empty `user` is the row leaving the account to be filled, and
        // an absent `port` reaches here as 22 (the config crate reads it
        // as "ssh's own"): both are gaps, and a gap is `~/.ssh/config`'s to
        // fill. A row that names either outranks the file for that field.
        unsaid: Unsaid { user: row.user.is_empty(), port: row.port == 22 },
        notice: None,
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
        let refused = || {
            format!(
                "the host key {} for {host} was not accepted; connection refused",
                sha256(key)
            )
        };
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
            SshRequest {
                user: "overseer".into(),
                host: "vault".into(),
                port: 2222,
                keys: Vec::new(),
                unsaid: Unsaid::default(),
                notice: None,
            }
        );
        assert_eq!(run("overseer@vault").unwrap().port, 22);
        std::env::set_var(USER_VAR, "resident");
        assert_eq!(run("vault").unwrap().user, "resident");
        assert!(run("@vault").is_err());
        assert!(run("overseer@").is_err());
        assert!(run("vault:notaport").is_err());
    }

    /// An address of colons reaches the socket whole, because a socket
    /// that is handed anything else asks a resolver about it and the
    /// resolver has never heard of it.
    #[test]
    fn an_address_of_colons_keeps_its_colons() {
        std::env::set_var(USER_VAR, "resident");
        let bare = run("2001:db8::1").unwrap();
        assert_eq!(bare.host, "2001:db8::1");
        assert_eq!(bare.port, 22);
        assert!(bare.unsaid.port);

        let ported = run("overseer@[2001:db8::1]:2222").unwrap();
        assert_eq!(ported.host, "2001:db8::1");
        assert_eq!(ported.port, 2222);

        assert_eq!(run("[::1]").unwrap().host, "::1");
        assert_eq!(run("192.168.1.5").unwrap().host, "192.168.1.5");
        assert!(run("[::1").is_err());
        assert!(run("[::1]22").is_err());
        assert!(run("[::1]:notaport").is_err());
    }

    /// What was not spelled has to still read as unspelled after a round
    /// trip through the string a new window is handed, or the file would
    /// be outranked by a default nobody typed.
    #[test]
    fn a_spelling_survives_the_round_trip_carrying_what_it_left_out() {
        std::env::set_var(USER_VAR, "resident");
        for spec in [
            "overseer@vault:2222",
            "overseer@vault",
            "vault:2222",
            "vault",
            "2001:db8::1",
            "overseer@[2001:db8::1]:2222",
        ] {
            let req = run(spec).unwrap();
            assert_eq!(req.spec(), spec);
            assert_eq!(run(&req.spec()).unwrap(), req);
        }
        assert_eq!(run("vault").unwrap().unsaid, Unsaid { user: true, port: true });
        assert_eq!(
            run("overseer@vault:22").unwrap().unsaid,
            Unsaid { user: false, port: false },
            "a port that was typed was said, 22 or not"
        );
    }

    #[test]
    fn a_rows_key_becomes_a_path_and_tilde_means_home() {
        assert!(key_path("").is_empty());
        assert_eq!(key_path("/etc/key"), vec![std::path::PathBuf::from("/etc/key")]);
        let home = std::env::home_dir().unwrap();
        assert_eq!(key_path("~/.ssh/id_gw"), vec![home.join(".ssh").join("id_gw")]);
    }

    /// The precedence, field by field: the file fills a gap and never
    /// overrules a word the operator said, except `HostName`, which is
    /// what a `Host` block is for.
    #[test]
    fn the_file_fills_what_was_left_unsaid_and_nothing_that_was_said() {
        use crate::ssh_config::{Counsel, Says};
        let says = || {
            Counsel::Says(Says {
                host_name: Some("10.0.0.5".into()),
                user: Some("filed".into()),
                port: Some(2222),
                identity_files: vec![std::path::PathBuf::from("/keys/filed")],
            })
        };
        std::env::set_var(USER_VAR, "resident");

        let mut bare = run("vault").unwrap();
        bare.take_counsel(says());
        assert_eq!(bare.host, "10.0.0.5");
        assert_eq!(bare.user, "filed", "the file outranks the invoking user's name");
        assert_eq!(bare.port, 2222);
        assert_eq!(bare.keys, vec![std::path::PathBuf::from("/keys/filed")]);
        assert_eq!(bare.notice, None);

        let mut spelled = run("overseer@vault:24").unwrap();
        spelled.keys = vec![std::path::PathBuf::from("/keys/named")];
        spelled.take_counsel(says());
        assert_eq!(spelled.host, "10.0.0.5", "a Host block names a lookup, not a machine");
        assert_eq!(spelled.user, "overseer");
        assert_eq!(spelled.port, 24);
        assert_eq!(spelled.keys, vec![std::path::PathBuf::from("/keys/named")]);

        // A refusal moves nothing at all and carries the notice instead.
        let mut refused = run("vault").unwrap();
        refused.take_counsel(Counsel::Refused("ProxyJump".into()));
        assert_eq!(refused.host, "vault");
        assert_eq!(refused.port, 22);
        assert_eq!(refused.notice.as_deref(), Some("ProxyJump"));

        // And silence is silence.
        let mut quiet = run("vault").unwrap();
        let before = quiet.clone();
        quiet.take_counsel(Counsel::Silent);
        assert_eq!(quiet, before);
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
                keys: Vec::new(),
                unsaid: Unsaid::default(),
                notice: None,
            })
        );

        cfg.ssh.default = "gone".into();
        assert_eq!(default_request(&cfg), None, "a stale default costs a log line, not a window");

        // A bare row takes the invoking user's name, and says so: a name
        // filled in here is a gap the config file may fill instead.
        cfg.ssh.default = "vault".into();
        cfg.ssh.hosts[0].user = String::new();
        std::env::set_var(USER_VAR, "resident");
        let req = default_request(&cfg).unwrap();
        assert_eq!(req.user, "resident");
        assert_eq!(req.unsaid, Unsaid { user: true, port: false });
        assert_eq!(req.spec(), "vault:2222", "the round trip keeps the gap a gap");

        // And a row that names neither leaves both to be filled.
        cfg.ssh.hosts[0].port = 22;
        assert_eq!(
            default_request(&cfg).unwrap().unsaid,
            Unsaid { user: true, port: true },
            "an absent port reaches the row as 22, which is this program filling it"
        );
    }

    /// The row's key is part of the destination, and the carry from the
    /// config to a window is the request itself, so what is asserted here
    /// is what the window dials with: a spelling has nowhere to put a key.
    #[test]
    fn the_default_carries_the_rows_key_to_the_window() {
        let mut cfg = config::Config::default();
        cfg.ssh.hosts.push(config::SshHost {
            host: "vault".into(),
            user: "overseer".into(),
            port: 22,
            key: "~/.ssh/id_foo".into(),
        });
        cfg.ssh.default = "vault".into();
        let home = std::env::home_dir().unwrap();
        assert_eq!(
            default_request(&cfg).unwrap().keys,
            vec![home.join(".ssh").join("id_foo")]
        );
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
