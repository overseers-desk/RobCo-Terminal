//! The destination picker `Shift+Alt+T` raises: where one new channel
//! goes, chosen on the glass, the configured default untouched.
//!
//! The page is a channel: the chord takes a free home slot with a bare
//! screen ([`term::TmuxPane`], a grid with no transport), these functions
//! paint it by escape sequences through its own parser, and the surface
//! intercepts its keys ahead of the keytab, the shape `gateway_key` set.
//! So the phosphor, the curvature and the cursor all apply for free, and
//! nothing new is rendered anywhere.
//!
//! Pure functions over the config's `[[ssh.host]]` rows, the `chord.rs`
//! shape: no I/O, no session, the host wires the actions. Digits choose,
//! the bank's own idiom; `1` is localhost, matching the settings tab's
//! radio order; `Esc` cancels.

use config::SshHost;
use winit::keyboard::{Key, NamedKey};

/// How many rows a digit can reach: `1` is localhost, `2` to `9` are the
/// first eight configured servers. Rows past that are named in the page's
/// footer as the settings window's, rather than paged here.
pub const DIGIT_ROWS: usize = 8;

/// What one key means to the picker.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Open a local shell.
    Localhost,
    /// Open a connection to `hosts[index]`.
    Host(usize),
    /// Close the picker, open nothing.
    Cancel,
    /// Not the picker's key: swallowed, so the page under it stays put.
    Ignored,
}

/// Read one key against the page over `hosts`.
pub fn read_key(logical: &Key, hosts: &[SshHost]) -> Verdict {
    match logical {
        Key::Named(NamedKey::Escape) => Verdict::Cancel,
        Key::Character(c) => {
            let [digit] = c.as_bytes() else {
                return Verdict::Ignored;
            };
            match digit {
                b'1' => Verdict::Localhost,
                b'2'..=b'9' => {
                    let index = usize::from(digit - b'2');
                    if index < hosts.len().min(DIGIT_ROWS) {
                        Verdict::Host(index)
                    } else {
                        Verdict::Ignored
                    }
                }
                _ => Verdict::Ignored,
            }
        }
        _ => Verdict::Ignored,
    }
}

/// A row's label: how the destination is spelled on the page.
fn label(row: &SshHost) -> String {
    let mut label = String::new();
    if !row.user.is_empty() {
        label.push_str(&row.user);
        label.push('@');
    }
    label.push_str(&row.host);
    if row.port != 22 {
        label.push(':');
        label.push_str(&row.port.to_string());
    }
    label
}

/// The whole page as bytes for the picker channel's parser: clear, home,
/// the numbered destinations, the footer. Repainted whole on raise and on
/// resize; a full repaint of a dozen lines is cheaper than tracking damage.
pub fn paint(hosts: &[SshHost]) -> Vec<u8> {
    let mut page = String::from("\x1b[2J\x1b[H\r\n  SELECT DESTINATION\r\n\r\n");
    page.push_str("   1  localhost (a local shell)\r\n");
    for (index, row) in hosts.iter().take(DIGIT_ROWS).enumerate() {
        page.push_str(&format!("   {}  {}\r\n", index + 2, label(row)));
    }
    if hosts.len() > DIGIT_ROWS {
        page.push_str(&format!(
            "\x1b[2m      and {} more, reachable through the settings window\x1b[0m\r\n",
            hosts.len() - DIGIT_ROWS
        ));
    }
    page.push_str(
        "\r\n\x1b[2m  a digit connects; Esc cancels; the default connection is unchanged\x1b[0m\r\n",
    );
    page.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(name: &str, user: &str, port: u16) -> SshHost {
        SshHost { host: name.into(), user: user.into(), port, key: String::new() }
    }

    #[test]
    fn digits_map_to_rows_and_the_rest_is_swallowed() {
        let hosts = vec![host("vault", "overseer", 22), host("gw", "", 2222)];
        let key = |s: &str| Key::Character(s.into());
        assert_eq!(read_key(&key("1"), &hosts), Verdict::Localhost);
        assert_eq!(read_key(&key("2"), &hosts), Verdict::Host(0));
        assert_eq!(read_key(&key("3"), &hosts), Verdict::Host(1));
        assert_eq!(read_key(&key("4"), &hosts), Verdict::Ignored, "no fourth row");
        assert_eq!(read_key(&key("x"), &hosts), Verdict::Ignored);
        assert_eq!(read_key(&Key::Named(NamedKey::Escape), &hosts), Verdict::Cancel);
        assert_eq!(read_key(&Key::Named(NamedKey::Enter), &hosts), Verdict::Ignored);
    }

    #[test]
    fn the_page_numbers_localhost_first_and_names_the_overflow() {
        let mut hosts: Vec<SshHost> = (0..10).map(|n| host(&format!("h{n}"), "", 22)).collect();
        hosts[1] = host("vault", "overseer", 2222);
        let page = String::from_utf8(paint(&hosts)).unwrap();
        assert!(page.contains("1  localhost"));
        assert!(page.contains("3  overseer@vault:2222"), "{page}");
        assert!(page.contains("9  h7"));
        assert!(!page.contains("h8"), "digits stop at nine rows");
        assert!(page.contains("and 2 more"));
        assert!(page.contains("default connection is unchanged"));
    }
}
