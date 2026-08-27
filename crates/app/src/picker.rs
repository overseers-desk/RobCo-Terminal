//! The destination picker `Shift+Alt+T` raises: where one new channel
//! goes, chosen on the glass, and whether that choice becomes the default.
//!
//! The page is a channel: the chord takes a free home slot with a bare
//! screen ([`term::TmuxPane`], a grid with no transport), these functions
//! paint it by escape sequences through its own parser, and the surface
//! intercepts its keys ahead of the keytab, the shape `gateway_key` set.
//! So the phosphor, the curvature and the cursor all apply for free, and
//! nothing new is rendered anywhere.
//!
//! Pure over the config's `[[ssh.host]]` rows and one small state, the
//! `prompt.rs` shape: no I/O, no session, no file, the host wires the
//! actions. Digits choose, the bank's own idiom; `1` is localhost, matching
//! the settings tab's radio order; `0` opens the arm where a destination
//! that is not configured is typed instead of chosen; `Esc` steps back out
//! of the arm, and out of the page.
//!
//! The typed arm is a [`crate::prompt::Line`], the same editor a connection's
//! questions are answered into. There is one line editor in this program
//! because there is one way to type a line into the glass, and an echoing
//! line is what a hostname is.
//!
//! The checkbox is the whole of the default connection's provenance. Nothing
//! else in the program writes `ssh.default`: the destination the user is
//! looking at, at the moment they choose it, is the only place where "and
//! from now on, this one" is a question with an obvious subject. A tick
//! costs one keystroke on a page already open, and an untouched page writes
//! nothing at all.

use config::SshHost;
use winit::keyboard::{Key, NamedKey};

use crate::prompt::{self, Stroke};
use crate::ssh::SshRequest;

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
    /// Open a connection to a destination the user typed, spelled the way
    /// `--ssh` spells one. It parsed here before it was handed over, so the
    /// arm could stay open if it did not.
    Typed(String),
    /// Close the picker, open nothing.
    Cancel,
    /// Not a key that opens anything: swallowed, so the page under it stays
    /// put. The page may well have changed -- a character typed, the
    /// checkbox toggled -- which is why the host repaints on this verdict.
    Ignored,
}

/// The page while it stands: which slot it is on, and everything the user
/// has said to it so far.
pub struct Picker {
    /// The home slot its page holds.
    pub slot: u32,
    /// The typed arm, while it is open. `None` is the list: digits choose.
    typed: Option<prompt::Line>,
    /// Whether the destination chosen here becomes the default connection.
    make_default: bool,
    /// Why the last commit did not connect, under the field it was typed in.
    error: Option<String>,
}

impl Picker {
    /// A page on `slot`, showing the list, the checkbox clear: the state a
    /// chord raises, and the state a page that writes nothing is in.
    pub fn new(slot: u32) -> Self {
        Self { slot, typed: None, make_default: false, error: None }
    }

    /// Whether the user ticked the box. Read by the host when a verdict
    /// opens something, because that is when the tick means anything.
    pub fn make_default(&self) -> bool {
        self.make_default
    }

    /// Read one key against the page over `hosts`.
    ///
    /// `text` is winit's decoding of the event, which the line editor needs
    /// and the digits do not; it is passed through to the arm untouched.
    ///
    /// `Tab` toggles the checkbox in both modes rather than reaching the
    /// line. It is not a character a hostname can hold and it is not a
    /// digit, so it cannot be a key the user meant for anything else, and
    /// the box is reachable without the hand leaving the keyboard for a
    /// mouse the glass does not have.
    ///
    /// The echo bytes the line hands back are dropped on purpose: the host
    /// repaints the whole page for every key it took, because the field is
    /// not the only thing on it that a keystroke moves.
    pub fn key(&mut self, logical: &Key, text: Option<&str>, hosts: &[SshHost]) -> Verdict {
        if matches!(logical, Key::Named(NamedKey::Tab)) {
            self.make_default = !self.make_default;
            return Verdict::Ignored;
        }
        let Some(line) = self.typed.as_mut() else {
            return self.list_key(logical, hosts);
        };
        let (stroke, _echo) = line.key(logical, text);
        match stroke {
            Stroke::Commit => {
                let spec = line.shown().trim().to_string();
                match SshRequest::parse(&spec) {
                    Ok(_) => {
                        self.typed = None;
                        self.error = None;
                        Verdict::Typed(spec)
                    }
                    // A typo costs a keystroke, not the page: the arm stays
                    // open with what was typed still in it, and the reason
                    // stands under the field.
                    Err(why) => {
                        self.error = Some(why);
                        Verdict::Ignored
                    }
                }
            }
            // Esc steps out of the arm and back to the list; a second one,
            // read below, takes the page down. So the way out of a
            // half-typed hostname is not also the way out of the picker.
            Stroke::Cancel => {
                self.typed = None;
                self.error = None;
                Verdict::Ignored
            }
            Stroke::Typed | Stroke::Backspace => {
                self.error = None;
                Verdict::Ignored
            }
            Stroke::Ignored => Verdict::Ignored,
        }
    }

    /// The list's own keys, the arm closed.
    fn list_key(&mut self, logical: &Key, hosts: &[SshHost]) -> Verdict {
        match logical {
            Key::Named(NamedKey::Escape) => Verdict::Cancel,
            Key::Character(c) => {
                let [digit] = c.as_bytes() else {
                    return Verdict::Ignored;
                };
                match digit {
                    // The digits run out where the configured rows do, so
                    // the one left over opens the arm: a destination nobody
                    // configured is typed here rather than configured first.
                    b'0' => {
                        self.typed = Some(prompt::Line::new(true));
                        Verdict::Ignored
                    }
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
/// the numbered destinations, the typed arm if it is open, the checkbox,
/// the footer.
///
/// Repainted whole for every key the page took, and on resize. A dozen
/// lines is cheaper than tracking which of them a keystroke moved, and a
/// keystroke moves more of them than it looks: a character typed into the
/// field also clears the error line above the checkbox below it.
///
/// The last thing painted is the cursor, put back on the field when the arm
/// is open, so the eye is where the hand is rather than under the footer.
pub fn paint(hosts: &[SshHost], state: &Picker) -> Vec<u8> {
    let mut lines: Vec<String> = vec![String::new(), "  SELECT DESTINATION".into(), String::new()];
    lines.push("   1  localhost (a local shell)".into());
    for (index, row) in hosts.iter().take(DIGIT_ROWS).enumerate() {
        lines.push(format!("   {}  {}", index + 2, label(row)));
    }
    if hosts.len() > DIGIT_ROWS {
        lines.push(format!(
            "\x1b[2m      and {} more, reachable through the settings window\x1b[0m",
            hosts.len() - DIGIT_ROWS
        ));
    }
    // The arm stands where the row that opens it stood, so the field is in
    // the column the eye was already reading.
    let field = state.typed.as_ref().map(|line| {
        lines.push(format!("   0  {}", line.shown()));
        lines.len()
    });
    if field.is_none() {
        lines.push("   0  a destination you type".into());
    }
    if let Some(why) = &state.error {
        lines.push(format!("      {why}"));
    }
    lines.push(String::new());
    lines.push(format!(
        "  [{}] Tab  make this the default connection",
        if state.make_default { "x" } else { " " }
    ));
    lines.push(String::new());
    lines.push(match state.typed {
        Some(_) => "\x1b[2m  Enter connects; Esc goes back to the list\x1b[0m".into(),
        None => "\x1b[2m  a digit connects; Esc cancels\x1b[0m".to_string(),
    });

    let mut page = String::from("\x1b[2J\x1b[H");
    page.push_str(&lines.join("\r\n"));
    page.push_str("\r\n");
    if let Some(row) = field {
        // Rows and columns are one-based, and the field line carries no
        // escape sequence, so its characters are its columns.
        let column = lines[row - 1].chars().count() + 1;
        page.push_str(&format!("\x1b[{row};{column}H"));
    }
    page.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(name: &str, user: &str, port: u16) -> SshHost {
        SshHost { host: name.into(), user: user.into(), port, key: String::new() }
    }

    fn ch(s: &str) -> Key {
        Key::Character(s.into())
    }

    fn named(key: NamedKey) -> Key {
        Key::Named(key)
    }

    /// Type `spec` into an open arm, character by character, as a hand does.
    fn type_spec(picker: &mut Picker, hosts: &[SshHost], spec: &str) {
        for c in spec.chars() {
            let text = c.to_string();
            assert_eq!(
                picker.key(&ch(&text), Some(&text), hosts),
                Verdict::Ignored,
                "a character typed at the field opens nothing"
            );
        }
    }

    #[test]
    fn digits_map_to_rows_and_the_rest_is_swallowed() {
        let hosts = vec![host("vault", "overseer", 22), host("gw", "", 2222)];
        let mut picker = Picker::new(3);
        let mut key = |k: Key| picker.key(&k, None, &hosts);
        assert_eq!(key(ch("1")), Verdict::Localhost);
        assert_eq!(key(ch("2")), Verdict::Host(0));
        assert_eq!(key(ch("3")), Verdict::Host(1));
        assert_eq!(key(ch("4")), Verdict::Ignored, "no fourth row");
        assert_eq!(key(ch("x")), Verdict::Ignored);
        assert_eq!(key(named(NamedKey::Escape)), Verdict::Cancel);
        assert_eq!(key(named(NamedKey::Enter)), Verdict::Ignored);
    }

    #[test]
    fn the_page_numbers_localhost_first_and_names_the_overflow() {
        let mut hosts: Vec<SshHost> = (0..10).map(|n| host(&format!("h{n}"), "", 22)).collect();
        hosts[1] = host("vault", "overseer", 2222);
        let page = String::from_utf8(paint(&hosts, &Picker::new(3))).unwrap();
        assert!(page.contains("1  localhost"));
        assert!(page.contains("3  overseer@vault:2222"), "{page}");
        assert!(page.contains("9  h7"));
        assert!(!page.contains("h8"), "digits stop at nine rows");
        assert!(page.contains("and 2 more"));
        assert!(page.contains("0  a destination you type"), "{page}");
        assert!(page.contains("[ ] Tab  make this the default"), "{page}");
    }

    /// `0` is the arm: digits after it are characters of a hostname rather
    /// than choices, and the commit hands the spec over.
    #[test]
    fn the_typed_arm_opens_on_zero_and_takes_digits_as_characters() {
        let hosts = vec![host("vault", "overseer", 22)];
        let mut picker = Picker::new(3);
        assert_eq!(picker.key(&ch("0"), Some("0"), &hosts), Verdict::Ignored);

        type_spec(&mut picker, &hosts, "resident@10.0.0.2:2222");
        let page = String::from_utf8(paint(&hosts, &picker)).unwrap();
        assert!(page.contains("0  resident@10.0.0.2:2222"), "{page}");
        assert!(page.contains("Enter connects"), "the footer says what the arm's keys do");
        assert!(
            page.contains("1  localhost"),
            "the list stays readable under the field: {page}"
        );

        assert_eq!(
            picker.key(&named(NamedKey::Enter), None, &hosts),
            Verdict::Typed("resident@10.0.0.2:2222".to_string())
        );
        let page = String::from_utf8(paint(&hosts, &picker)).unwrap();
        assert!(page.contains("0  a destination you type"), "the arm closed: {page}");
    }

    #[test]
    fn tab_ticks_the_box_in_both_modes_and_the_page_shows_it() {
        let hosts = vec![host("vault", "overseer", 22)];
        let mut picker = Picker::new(3);
        assert!(!picker.make_default(), "an untouched page writes nothing");
        assert_eq!(picker.key(&named(NamedKey::Tab), Some("\t"), &hosts), Verdict::Ignored);
        assert!(picker.make_default());
        let page = String::from_utf8(paint(&hosts, &picker)).unwrap();
        assert!(page.contains("[x] Tab  make this the default connection"), "{page}");

        // In the arm too, and it is not a character of the hostname.
        picker.key(&ch("0"), Some("0"), &hosts);
        type_spec(&mut picker, &hosts, "vault");
        picker.key(&named(NamedKey::Tab), Some("\t"), &hosts);
        assert!(!picker.make_default(), "the same key unticks it");
        let page = String::from_utf8(paint(&hosts, &picker)).unwrap();
        assert!(page.contains("[ ] Tab"), "{page}");
        assert!(page.contains("0  vault"), "the tab did not reach the field: {page}");
    }

    #[test]
    fn a_spec_that_does_not_parse_keeps_the_arm_open_and_says_why() {
        let hosts = vec![host("vault", "overseer", 22)];
        let mut picker = Picker::new(3);
        picker.key(&ch("0"), Some("0"), &hosts);
        type_spec(&mut picker, &hosts, "resident@vault:door");

        assert_eq!(
            picker.key(&named(NamedKey::Enter), None, &hosts),
            Verdict::Ignored,
            "nothing is dialled on a spec that does not parse"
        );
        let page = String::from_utf8(paint(&hosts, &picker)).unwrap();
        assert!(page.contains("'door' is not a port number"), "{page}");
        assert!(
            page.contains("0  resident@vault:door"),
            "what was typed is still there: {page}"
        );

        // Fixing it clears the reason, and the fixed spec goes through.
        for _ in 0..4 {
            picker.key(&named(NamedKey::Backspace), None, &hosts);
        }
        type_spec(&mut picker, &hosts, "22");
        let page = String::from_utf8(paint(&hosts, &picker)).unwrap();
        assert!(!page.contains("not a port number"), "{page}");
        assert_eq!(
            picker.key(&named(NamedKey::Enter), None, &hosts),
            Verdict::Typed("resident@vault:22".to_string())
        );
    }

    /// Esc steps: out of the arm first, and only then off the page.
    #[test]
    fn escape_leaves_the_arm_before_it_leaves_the_page() {
        let hosts = vec![host("vault", "overseer", 22)];
        let mut picker = Picker::new(3);
        picker.key(&ch("0"), Some("0"), &hosts);
        type_spec(&mut picker, &hosts, "gw");

        assert_eq!(picker.key(&named(NamedKey::Escape), None, &hosts), Verdict::Ignored);
        let page = String::from_utf8(paint(&hosts, &picker)).unwrap();
        assert!(page.contains("0  a destination you type"), "{page}");
        assert!(!page.contains("gw"), "the abandoned answer went with the arm: {page}");

        assert_eq!(picker.key(&named(NamedKey::Escape), None, &hosts), Verdict::Cancel);
    }

    /// The tick survives the trip through the arm: it is the page's state,
    /// not the field's.
    #[test]
    fn the_tick_is_the_pages_and_outlasts_the_arm() {
        let hosts = vec![host("vault", "overseer", 22)];
        let mut picker = Picker::new(3);
        picker.key(&named(NamedKey::Tab), Some("\t"), &hosts);
        picker.key(&ch("0"), Some("0"), &hosts);
        type_spec(&mut picker, &hosts, "gw");
        picker.key(&named(NamedKey::Escape), None, &hosts);
        assert!(picker.make_default());
        assert_eq!(picker.key(&ch("2"), Some("2"), &hosts), Verdict::Host(0));
    }
}
