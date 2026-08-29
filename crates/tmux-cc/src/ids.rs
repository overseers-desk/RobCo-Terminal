//! The three object ids tmux hands out, each keeping its sigil.
//!
//! A pane is `%3`, a window `@1`, a session `$0`. The sigil is what a `-t`
//! target takes, so these types keep the whole token and validate it on the
//! way in: a `%output` naming `@1` is a parse failure, not a mystery upstream.

macro_rules! tmux_id {
    ($name:ident, $sigil:literal, $what:literal) => {
        #[doc = concat!("A tmux ", $what, " id, sigil included (`", $sigil, "3`).")]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Take a token from the wire. `None` unless it is the sigil
            /// followed by at least one digit: tmux numbers these, and a
            /// non-numeric tail is a field boundary gone wrong.
            pub fn parse(token: &str) -> Option<Self> {
                let rest = token.strip_prefix($sigil)?;
                if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                Some(Self(token.to_string()))
            }

            /// The whole token, sigil included: what a `-t` target wants.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// The number after the sigil.
            pub fn number(&self) -> u64 {
                self.0[1..].parse().unwrap_or_default()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

tmux_id!(PaneId, '%', "pane");
tmux_id!(WindowId, '@', "window");
tmux_id!(SessionId, '$', "session");

/// Read the line [`Command::Server`](crate::Command::Server) comes back with:
/// socket path, pid, the attached session's id, host name. Split from the
/// right, because only the socket can hold a space.
pub fn parse_server(line: &str) -> Option<(String, u32, SessionId, String)> {
    let mut fields = line.trim().rsplitn(4, char::is_whitespace);
    let host = fields.next()?.to_string();
    let session = SessionId::parse(fields.next()?)?;
    let pid = fields.next()?.parse().ok()?;
    let socket = fields.next()?.to_string();
    (!socket.is_empty() && !host.is_empty()).then_some((socket, pid, session, host))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sigil_belongs_to_exactly_one_kind() {
        assert_eq!(PaneId::parse("%3").unwrap().as_str(), "%3");
        assert!(PaneId::parse("@3").is_none());
        assert!(WindowId::parse("%3").is_none());
        assert_eq!(SessionId::parse("$0").unwrap().number(), 0);
    }

    #[test]
    fn a_bare_sigil_or_a_non_numeric_tail_is_not_an_id() {
        assert!(PaneId::parse("%").is_none());
        assert!(PaneId::parse("%1a").is_none());
        assert!(WindowId::parse("").is_none());
    }

    #[test]
    fn the_number_is_the_tail() {
        assert_eq!(WindowId::parse("@17").unwrap().number(), 17);
    }

    #[test]
    fn the_server_line_gives_up_its_four_fields() {
        let (socket, pid, session, host) =
            parse_server("/tmp/tmux-1000/default 4242 $0 workshop").unwrap();
        assert_eq!(socket, "/tmp/tmux-1000/default");
        assert_eq!(pid, 4242);
        assert_eq!(session.as_str(), "$0");
        assert_eq!(host, "workshop");
    }

    #[test]
    fn a_socket_path_may_hold_a_space() {
        let (socket, _, session, host) =
            parse_server("/tmp/my sockets/default 7 $12 host\n").unwrap();
        assert_eq!(socket, "/tmp/my sockets/default");
        assert_eq!(session.number(), 12);
        assert_eq!(host, "host");
    }

    #[test]
    fn a_line_that_is_not_four_fields_is_not_a_server() {
        assert!(parse_server("/tmp/s 4242 $0").is_none());
        assert!(parse_server("/tmp/s notapid $0 host").is_none());
        assert!(parse_server("/tmp/s 4242 @0 host").is_none());
        assert!(parse_server("").is_none());
    }
}
