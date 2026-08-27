//! What `~/.ssh/config` is allowed to say about a destination, and what it
//! is refused for saying.
//!
//! The file is read whole or not at all. Honouring `HostName` while
//! passing over `ProxyJump` connects, confidently, to the wrong place;
//! `HostKeyAlias` silently changes which `known_hosts` entry a key is
//! checked against. Both failures look like success from the glass, which
//! is the one thing a trust-bearing path may never do. So there are two
//! outcomes and no third: either the matched block yields nothing this
//! build cannot honour, and every word of it is taken -- or it carries one
//! word this build cannot honour, and the whole file's counsel is set
//! aside out loud, the connection going where the destination was spelled
//! as though no file existed.
//!
//! The parse is [`russh_config`]'s, the same family as the pinned russh:
//! one parser, with `Host` patterns, negations and the merge order the
//! file's own rules give them. What that crate hands back is only what it
//! knows how to name, though, and a directive it has never heard of is
//! dropped without a word -- `HostKeyAlias` among them. A dropped
//! directive is precisely the silence this design exists to avoid, so the
//! block is also read here as text, and every directive in it is held
//! against the table below before any of the parse is believed.
//!
//! # What is honoured
//!
//! `HostName`, `User`, `Port`, `IdentityFile`. Nothing else this build has
//! an implementation of, because there is nothing else it has an
//! implementation of.
//!
//! # What is refused
//!
//! Anything that decides where a connection goes, whom it authenticates
//! as, which identity it offers, which key it trusts, or what the far side
//! runs -- and that this build cannot carry out. `ProxyJump`,
//! `HostKeyAlias`, `UserKnownHostsFile`, `LocalForward`, `RemoteCommand`
//! and their kind, plus the boolean knobs whose one honourable value is
//! the behaviour this build already has (`ForwardAgent no` costs nothing;
//! `ForwardAgent yes` is a promise this build cannot keep).
//!
//! `Include` and `Match` are refused wherever in the file they stand,
//! matched block or not. An `Include` means the file in front of the
//! parser is not the whole file; a `Match` block is invisible to
//! `russh_config`, which folds its directives into the `Host` block above
//! it, so a file carrying one may be misread rather than under-read.
//!
//! Tuning and cosmetics -- `ServerAliveInterval`, `LogLevel`,
//! `VisualHostKey` -- decide none of those things and are passed over in
//! silence, as `ssh` itself would pass over a setting for a feature it was
//! built without.

use std::path::{Path, PathBuf};

/// What the file has to say about one destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Counsel {
    /// Nothing: no file, nothing readable in it, or no block matching this
    /// host. The overwhelmingly common case, and the one that has to leave
    /// no trace at all on the glass.
    Silent,
    /// What the matched block yields, every word of it honourable.
    Says(Says),
    /// The block, or the file's shape, carries something this build cannot
    /// honour. The text is the notice for the channel's glass; the caller
    /// connects to the destination as spelled.
    Refused(String),
}

/// The four directives this build implements, as the file gives them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Says {
    /// `HostName`: where the destination actually is. It replaces the
    /// spelled host outright, because `Host` is the name of a lookup and
    /// not the name of a machine.
    pub host_name: Option<String>,
    /// `User`, which fills a user the operator left unsaid.
    pub user: Option<String>,
    /// `Port`, which fills a port the operator left unsaid.
    pub port: Option<u16>,
    /// `IdentityFile`, in the file's own order, `~/` already expanded.
    /// All of them: taking the first and dropping the rest is the
    /// half-obedience this module exists to refuse.
    pub identity_files: Vec<PathBuf>,
}

/// The user's own `~/.ssh/config`, wherever this platform keeps a home
/// directory. `%USERPROFILE%\.ssh\config` on Windows, by the same
/// `home_dir` the default key files are found under.
pub fn home_file() -> Option<PathBuf> {
    std::env::home_dir().map(|home| home.join(".ssh").join("config"))
}

/// Read `path` for what it says about `host`.
///
/// An absent or unreadable file is [`Counsel::Silent`]: a terminal whose
/// user has no `~/.ssh/config` must be indistinguishable from one built
/// without this module, and a file the process may not open is not a
/// statement about anything. A file that exists and does not parse is a
/// notice, because there the user did write something and it is not being
/// followed.
pub fn read(path: &Path, host: &str) -> Counsel {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Counsel::Silent;
    };
    // The file's shape first: these two are refused wherever they stand,
    // because neither is a directive that applies to a host so much as a
    // statement that the parse below is not reading what the user wrote.
    if let Some(word) = structural(&text) {
        return Counsel::Refused(format!(
            "{} carries {word}, which this build cannot follow; none of the file is \
             taken and the connection goes to {host} as spelled",
            path.display()
        ));
    }
    // Then the matched block, directive by directive, as text. This runs
    // before the parse is believed and not after, because what it is
    // looking for is exactly what the parse cannot report.
    for (key, value) in directives_for(&text, host) {
        if unhonourable(key, value) {
            return Counsel::Refused(format!(
                "{} sets {key} for {host}, which this build cannot honour; none of the \
                 file's counsel is taken and the connection goes to {host} as spelled",
                path.display()
            ));
        }
    }
    let config = match russh_config::parse(&text, host) {
        Ok(config) => config,
        Err(e) => {
            return Counsel::Refused(format!(
                "{} could not be read ({e}); none of its counsel is taken and the \
                 connection goes to {host} as spelled",
                path.display()
            ))
        }
    };
    let block = config.host_config;
    let says = Says {
        host_name: block.hostname,
        user: block.user,
        port: block.port,
        identity_files: block.identity_file.unwrap_or_default(),
    };
    if says == Says::default() {
        // No block matched, or the ones that did said nothing this build
        // reads. Either way the file has no counsel here, and a file with
        // no counsel is a file that is not there.
        Counsel::Silent
    } else {
        Counsel::Says(says)
    }
}

/// `Include` or `Match`, as written, if the file carries either.
fn structural(text: &str) -> Option<&'static str> {
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_whitespace().next().map(str::to_lowercase).as_deref() {
            Some("include") => return Some("Include"),
            Some("match") => return Some("Match"),
            _ => {}
        }
    }
    None
}

/// Every directive in the blocks whose `Host` patterns match, as written
/// and in file order.
///
/// The line reading is `russh_config`'s own, deliberately: a line it skips
/// (a tab between key and value, a key with no value) must be a line this
/// skips too, or the two halves of this module would be reading different
/// files.
fn directives_for<'a>(text: &'a str, host: &str) -> Vec<(&'a str, &'a str)> {
    let mut found = Vec::new();
    let mut applies = false;
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let (Some(key), Some(value)) = (parts.next(), parts.next()) else {
            continue;
        };
        let value = value.trim_start();
        if key.eq_ignore_ascii_case("host") {
            applies = patterns_match(value, host);
        } else if applies {
            found.push((key, value));
        }
    }
    found
}

/// A `Host` line's patterns against a hostname, by the file's own rule: a
/// negated pattern that matches throws the whole line out, and any other
/// match takes it.
///
/// The glob is `globset`'s because that is what `russh_config` matches
/// with, and the two must agree about which blocks apply: a block this
/// pass reads and the parse does not would refuse a connection over a
/// directive that was never going to be honoured anyway, and a block the
/// parse reads and this one does not is the silence the whole module is
/// built to prevent.
fn patterns_match(patterns: &str, host: &str) -> bool {
    let mut matched = false;
    for pattern in patterns.split_ascii_whitespace() {
        let (pattern, negated) = match pattern.strip_prefix('!') {
            Some(pattern) => (pattern, true),
            None => (pattern, false),
        };
        let hit = globset::Glob::new(pattern)
            .map(|glob| glob.compile_matcher().is_match(host))
            .unwrap_or(false);
        if hit {
            if negated {
                return false;
            }
            matched = true;
        }
    }
    matched
}

/// Directives that decide where a connection goes, whom it authenticates
/// as, which identity it offers, what it trusts, or what the far side
/// runs, and that this build has no implementation of at all. Presence is
/// the refusal; there is no value of `ProxyJump` this build can carry out.
const UNHONOURABLE: &[&str] = &[
    // Where the connection goes.
    "proxycommand",
    "proxyjump",
    "proxyusefdpass",
    "bindaddress",
    "bindinterface",
    "canonicaldomains",
    "canonicalizefallbacklocal",
    "canonicalizemaxdots",
    "canonicalizepermittedcnames",
    // Which recorded key it is checked against, and where that record is.
    "hostkeyalias",
    "userknownhostsfile",
    "globalknownhostsfile",
    "knownhostscommand",
    "revokedhostkeys",
    // Which identity is offered, and how.
    "certificatefile",
    "identityagent",
    "pkcs11provider",
    "securitykeyprovider",
    "preferredauthentications",
    // What is negotiated. This build offers russh's sets and cannot
    // widen or narrow them, so a file that names its own is a file whose
    // instruction would go unfollowed.
    "ciphers",
    "hostkeyalgorithms",
    "kexalgorithms",
    "macs",
    "pubkeyacceptedalgorithms",
    "pubkeyacceptedkeytypes",
    "requiredrsasize",
    // What the far side runs, and what is carried over the wire beside it.
    "remotecommand",
    "localcommand",
    "dynamicforward",
    "localforward",
    "remoteforward",
    "sendenv",
    "setenv",
    // The multiplexing socket, which this build does not open or join.
    "controlpath",
    "controlpersist",
];

/// Directives naming a behaviour this build already has, and the values
/// that name it. The file asking for what is already true costs nothing;
/// asking for anything else is a promise this build cannot keep, and is
/// refused like any other.
const ONLY_IF: &[(&str, &[&str])] = &[
    ("addkeystoagent", &["no"]),
    ("addressfamily", &["any"]),
    ("batchmode", &["no"]),
    ("canonicalizehostname", &["no"]),
    ("checkhostip", &["no"]),
    ("compression", &["no"]),
    ("controlmaster", &["no"]),
    ("forwardagent", &["no"]),
    ("forwardx11", &["no"]),
    ("forwardx11trusted", &["no"]),
    ("gssapiauthentication", &["no"]),
    ("hashknownhosts", &["no"]),
    ("hostbasedauthentication", &["no"]),
    // The three methods this build implements, and it implements all of
    // them: switching one off is the instruction it cannot follow.
    ("kbdinteractiveauthentication", &["yes"]),
    ("passwordauthentication", &["yes"]),
    ("pubkeyauthentication", &["yes"]),
    // A named key never stops the agent or the default files being tried.
    ("identitiesonly", &["no"]),
    ("permitlocalcommand", &["no"]),
    // Every channel here carries a pty; `auto` and `force` both end in one.
    ("requesttty", &["auto", "yes", "force"]),
    // Ask on an unknown key, refuse a changed one: `ask` is this build's
    // policy exactly, and neither `yes` nor `no` is available to it.
    ("stricthostkeychecking", &["ask"]),
    ("verifyhostkeydns", &["no"]),
];

/// Whether one directive, as written, is one this build cannot honour.
fn unhonourable(key: &str, value: &str) -> bool {
    let key = key.to_lowercase();
    if UNHONOURABLE.contains(&key.as_str()) {
        return true;
    }
    ONLY_IF
        .iter()
        .find(|(name, _)| *name == key)
        .is_some_and(|(_, allowed)| !allowed.iter().any(|ok| value.eq_ignore_ascii_case(ok)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(text: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, text).unwrap();
        (dir, path)
    }

    fn read_text(text: &str, host: &str) -> Counsel {
        let (_dir, path) = write(text);
        read(&path, host)
    }

    #[test]
    fn a_file_that_is_not_there_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(&dir.path().join("nowhere"), "vault"), Counsel::Silent);
        // Nor does one that is there and holds nothing for this host.
        assert_eq!(
            read_text("Host elsewhere\n  HostName 10.0.0.9\n", "vault"),
            Counsel::Silent
        );
        assert_eq!(read_text("", "vault"), Counsel::Silent);
    }

    #[test]
    fn a_matched_block_yields_what_this_build_implements() {
        let counsel = read_text(
            "Host vault\n  HostName 10.0.0.5\n  User overseer\n  Port 2222\n  \
             IdentityFile /keys/id_vault\n  ServerAliveInterval 60\n",
            "vault",
        );
        assert_eq!(
            counsel,
            Counsel::Says(Says {
                host_name: Some("10.0.0.5".into()),
                user: Some("overseer".into()),
                port: Some(2222),
                identity_files: vec![PathBuf::from("/keys/id_vault")],
            }),
            "a tuning directive decides nothing and is passed over"
        );
    }

    #[test]
    fn several_identity_files_are_all_taken_in_the_files_order() {
        let counsel = read_text(
            "Host vault\n  IdentityFile /keys/first\nHost *\n  IdentityFile /keys/last\n",
            "vault",
        );
        let Counsel::Says(says) = counsel else { panic!("{counsel:?}") };
        assert_eq!(
            says.identity_files,
            vec![PathBuf::from("/keys/first"), PathBuf::from("/keys/last")]
        );
    }

    #[test]
    fn a_directive_this_build_cannot_honour_refuses_the_whole_file() {
        for (text, word, host) in [
            ("Host vault\n  HostName 10.0.0.5\n  ProxyJump gate\n", "ProxyJump", "vault"),
            ("Host vault\n  HostName 10.0.0.5\n  HostKeyAlias other\n", "HostKeyAlias", "vault"),
            ("Host vault\n  UserKnownHostsFile /tmp/kh\n", "UserKnownHostsFile", "vault"),
            ("Host vault\n  StrictHostKeyChecking no\n", "StrictHostKeyChecking", "vault"),
            ("Host vault\n  ForwardAgent yes\n", "ForwardAgent", "vault"),
            // In a glob block that covers this host, not only in its own.
            ("Host *.vault\n  ProxyCommand nc %h %p\n", "ProxyCommand", "a.vault"),
        ] {
            let counsel = read_text(text, host);
            let Counsel::Refused(notice) = counsel else {
                panic!("{word} was not refused: {counsel:?}")
            };
            assert!(notice.contains(word), "{notice}");
            assert!(notice.contains("as spelled"), "{notice}");
        }
    }

    #[test]
    fn the_honourable_value_of_a_knob_costs_nothing() {
        // The file asking for what is already true is not an instruction
        // this build is failing to follow.
        let counsel = read_text(
            "Host vault\n  HostName 10.0.0.5\n  ForwardAgent no\n  \
             StrictHostKeyChecking ask\n  RequestTTY yes\n",
            "vault",
        );
        assert_eq!(
            counsel,
            Counsel::Says(Says { host_name: Some("10.0.0.5".into()), ..Says::default() })
        );
    }

    #[test]
    fn a_block_that_does_not_match_is_not_read_for_refusals() {
        let counsel = read_text(
            "Host gate\n  ProxyJump elsewhere\nHost vault\n  HostName 10.0.0.5\n",
            "vault",
        );
        assert_eq!(
            counsel,
            Counsel::Says(Says { host_name: Some("10.0.0.5".into()), ..Says::default() })
        );
        // A negated pattern throws its own line out, so the block whose
        // patterns cover this host by glob but exclude it by name is not
        // this host's block either.
        let counsel = read_text(
            "Host *.vault !a.vault\n  ProxyJump elsewhere\nHost a.vault\n  Port 2222\n",
            "a.vault",
        );
        assert_eq!(counsel, Counsel::Says(Says { port: Some(2222), ..Says::default() }));
    }

    #[test]
    fn include_and_match_refuse_the_file_wherever_they_stand() {
        for (text, word) in [
            ("Include ~/.ssh/config.d/*\nHost vault\n  Port 2222\n", "Include"),
            ("Host vault\n  Port 2222\nMatch host gate\n  User someone\n", "Match"),
        ] {
            let counsel = read_text(text, "vault");
            let Counsel::Refused(notice) = counsel else {
                panic!("{word} was not refused: {counsel:?}")
            };
            assert!(notice.contains(word), "{notice}");
        }
    }

    #[test]
    fn a_file_that_does_not_parse_is_a_notice_and_not_a_crash() {
        // A directive before any Host line: the parser has nowhere to put
        // it, and the user gets told rather than obeyed halfway.
        let counsel = read_text("HostName 10.0.0.5\nHost vault\n  Port 2222\n", "vault");
        let Counsel::Refused(notice) = counsel else { panic!("{counsel:?}") };
        assert!(notice.contains("could not be read"), "{notice}");
    }
}
