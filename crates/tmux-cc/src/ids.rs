//! The three object ids tmux hands out, each keeping its sigil.
//!
//! A pane is `%3`, a window `@1`, a session `$0`. The sigil is part of the id
//! and not decoration: it is what a `-t` target takes, so an id that has been
//! stripped of it has to be rebuilt before it can be used, and the rebuild is
//! where the wrong sigil gets attached. These types keep the whole token and
//! validate the sigil on the way in, so a `%output` naming `@1` is a parse
//! failure here rather than a mystery three layers up.

macro_rules! tmux_id {
    ($name:ident, $sigil:literal, $what:literal) => {
        #[doc = concat!("A tmux ", $what, " id, sigil included (`", $sigil, "3`).")]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// The sigil this id must carry.
            pub const SIGIL: char = $sigil;

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
}
