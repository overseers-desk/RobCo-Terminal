//! What the glass asks the user and what it does with the answer: a
//! question a connection raised, and the find line over the scrollback.
//!
//! Both are modal for what raised them and both hold the keyboard while they
//! stand, so they sit together: everything typed at a password or a query is
//! swallowed here rather than falling through to a wire or the keytab. The
//! line editor under both is [`crate::prompt::Line`], and what a find steps
//! through is [`crate::find`]; neither needs a window.
//!
//! Fields touched: `banks`, whose record holds the desk a connection asks
//! over and the question standing on it; `find`, the line while it stands;
//! `channels`, because a question is painted onto the channel it was asked
//! on and a query onto the channel it was raised over; and `scroll`, which
//! is how a hit found in history is brought onto the screen.

use ssh_link::{Answer, Ask, AskDesk, Question};
use term::ChannelSession;
use winit::keyboard::ModifiersState;

use crate::channels::BankId;
use crate::ssh::notice_bytes;

use super::TerminalSurface;

/// A question a connection asked, standing on the glass while the answer
/// is typed.
///
/// The connection thread is blocked on the far side of the `Question` for
/// as long as this lives, which is what makes the pairing exact: the
/// question is answered once, or dropped -- and dropping it cancels, so a
/// bank swept out from under a prompt releases the thread instead of
/// stranding it.
pub(super) struct Pending {
    question: Question,
    line: crate::prompt::Line,
}

impl TerminalSurface {
    /// Drain what the connections are asking, after their wires have been
    /// drained and their dead banks swept.
    ///
    /// After, and that ordering is the law `ssh_link::ask` states from the
    /// other end: a notice and the question it explains travel by two
    /// carriers, so the transport puts the notice on the wire before it
    /// asks and the surface reads the wire before it reads the desk. Read
    /// the other way round, a fingerprint would arrive under the question
    /// about it.
    ///
    /// A question lands on the bank that asked, whether or not that bank is
    /// on the air. Stealing the air would be the connection interrupting
    /// whatever the user is doing on another channel; instead the question
    /// waits where it belongs, and turning the knob back finds it standing.
    pub(super) fn pump_asks(&mut self) {
        let banks: Vec<BankId> = self
            .banks
            .iter()
            .filter(|(_, runtime)| runtime.desk.is_some())
            .map(|(bank, _)| *bank)
            .collect();
        for bank in banks {
            loop {
                // A bank already holding a question is not asked another:
                // the thread that asked is blocked until this one is
                // answered, so there cannot be a second, and refusing to
                // take one is cheaper than reasoning about whether there is.
                let Some(runtime) = self.banks.get_mut(&bank) else {
                    break;
                };
                if runtime.prompt.is_some() {
                    break;
                }
                let Some(ask) = runtime.desk.as_mut().and_then(AskDesk::take) else {
                    break;
                };
                match ask {
                    Ask::Say(text) => {
                        let bytes = notice_bytes(&text);
                        self.feed_prompt_channel(bank, &bytes);
                    }
                    Ask::Question(question) => {
                        let bytes = crate::prompt::paint(question.prompt());
                        self.feed_prompt_channel(bank, &bytes);
                        let line = crate::prompt::Line::new(question.kind() != Answer::Secret);
                        if let Some(runtime) = self.banks.get_mut(&bank) {
                            runtime.prompt = Some(Pending { question, line });
                        }
                    }
                }
            }
        }
    }

    /// Put local bytes on the channel a connection's questions belong to,
    /// which is always slot 1: everything asked here is asked before a
    /// shell exists, so there is no second channel yet to ask on.
    fn feed_prompt_channel(&mut self, bank: BankId, bytes: &[u8]) {
        if let Some(row) = self
            .channels
            .rows_mut()
            .find(|r| r.bank == bank && r.channel == 1)
        {
            if let ChannelSession::Ssh(channel) = &mut row.session {
                channel.feed(bytes);
            }
        }
    }

    /// The prompt's keyboard, on the `picker_key` model: only while a
    /// question stands on the bank on the air, and only on the channel it
    /// was asked on. Answers whether the key was the prompt's.
    ///
    /// Everything that is not an answer is swallowed rather than passed on.
    /// A question is modal for the connection that asked it -- the thread is
    /// blocked -- so a keystroke that fell through to the wire would be
    /// typing at a shell that does not exist yet.
    pub(super) fn prompt_key(
        &mut self,
        logical: &winit::keyboard::Key,
        text: Option<&str>,
    ) -> bool {
        let (bank, channel) = self.channels.on_air();
        if channel != 1 {
            return false;
        }
        let Some(pending) = self.prompt_mut(bank) else {
            return false;
        };
        let (stroke, echo) = pending.line.key(logical, text);
        if !echo.is_empty() {
            self.feed_prompt_channel(bank, &echo);
        }
        match stroke {
            crate::prompt::Stroke::Commit => {
                if let Some(mut pending) = self.take_prompt(bank) {
                    let answer = pending.line.take();
                    pending.question.answer(answer);
                }
            }
            crate::prompt::Stroke::Cancel => {
                // Cancelling hands the transport a `None`, and the
                // transport's own supervisor does the rest: it says so on
                // the wire and Eofs the channel. The row stays under the
                // Eof law, wearing why it is dead.
                if let Some(pending) = self.take_prompt(bank) {
                    pending.question.cancel();
                }
            }
            _ => {}
        }
        true
    }

    /// Whether a question is standing on the channel currently on the air.
    /// What the clipboard consults before it decides where a paste goes.
    fn prompt_on_air(&self) -> bool {
        let (bank, channel) = self.channels.on_air();
        channel == 1
            && self
                .banks
                .get(&bank)
                .is_some_and(|runtime| runtime.prompt.is_some())
    }

    /// The question standing on a bank, for whoever is about to type into it.
    fn prompt_mut(&mut self, bank: BankId) -> Option<&mut Pending> {
        self.banks
            .get_mut(&bank)
            .and_then(|runtime| runtime.prompt.as_mut())
    }

    /// The question off the bank: answering or cancelling it ends it, and the
    /// caller holds the only copy while it does.
    fn take_prompt(&mut self, bank: BankId) -> Option<Pending> {
        self.banks
            .get_mut(&bank)
            .and_then(|runtime| runtime.prompt.take())
    }

    /// `Ctrl+Shift+F`. Raise the find line on the channel on the air, or
    /// leave the one already standing where it is: a second press is a hand
    /// reaching for a line that is already in front of it.
    ///
    /// The caret is read before the prompt is painted and the floor after,
    /// which is the whole of what [`crate::find::Find`] needs to know about
    /// the grid: where a search with no hit behind it starts, and which rows
    /// hold the query itself.
    pub(super) fn open_find(&mut self) {
        if self.find.is_some() {
            return;
        }
        let on = self.channels.on_air();
        let Some(session) = self.channels.session_mut() else {
            return;
        };
        let caret = crate::find::caret(session.term());
        session.feed(&crate::prompt::paint(crate::find::PROMPT));
        let floor = crate::find::caret(session.term()).1;
        self.find = Some(crate::find::Find::new(on, caret, floor));
    }

    /// The find line's keyboard, on the [`Self::prompt_key`] model: only
    /// while the line stands on the channel it was raised on, and every key
    /// swallowed rather than passed on. Answers whether the key was the
    /// find line's.
    ///
    /// Enter and Escape are read here rather than through
    /// [`crate::prompt::Line`], which would answer them with the `\r\n` that
    /// ends a question. A find line is not answered once: Enter steps to
    /// the next hit and the query stays where it is, to be stepped again.
    pub(super) fn find_key(
        &mut self,
        logical: &winit::keyboard::Key,
        text: Option<&str>,
        modifiers: ModifiersState,
    ) -> bool {
        use winit::keyboard::{Key, NamedKey};

        let on = self.channels.on_air();
        if self.find.as_ref().map(|find| find.on) != Some(on) {
            return false;
        }
        match logical {
            Key::Named(NamedKey::Enter) => self.find_step(!modifiers.shift_key()),
            Key::Named(NamedKey::Escape) => self.close_find(),
            _ => {
                let Some(find) = self.find.as_mut() else {
                    return false;
                };
                let (_, echo) = find.line.key(logical, text);
                if !echo.is_empty() {
                    if let Some(session) = self.channels.session_mut() {
                        session.feed(&echo);
                    }
                }
            }
        }
        true
    }

    /// Enter, or Shift+Enter the other way: step to the next hit and bring
    /// it onto the screen.
    ///
    /// The view moves through the same [`ScrollPosition`] the wheel and the
    /// scroll keys move, so a hit found in history leaves the viewport
    /// exactly where a user who had scrolled there by hand would have left
    /// it.
    fn find_step(&mut self, forward: bool) {
        let Some(find) = self.find.as_mut() else {
            return;
        };
        let Some(session) = self.channels.session_mut() else {
            return;
        };
        let Some(range) = find.step(session.term(), forward) else {
            return;
        };
        self.scroll.reveal(session.term_mut(), range.start.1);
    }

    /// Escape. The line comes down, its mark comes off the glass with it,
    /// and the cursor leaves the query's row so whatever the channel says
    /// next says it on a row of its own.
    fn close_find(&mut self) {
        if self.find.take().is_none() {
            return;
        }
        if let Some(session) = self.channels.session_mut() {
            session.feed(b"\r\n");
        }
    }

    /// A paste while a question stands on the air: the question takes it,
    /// not the wire. Answers whether it did.
    ///
    /// A password is a thing people keep in a password manager and paste;
    /// sending it down the wire because a prompt happened not to be a shell
    /// would put it in front of a server that never asked for it.
    pub(super) fn paste_into_prompt(&mut self, text: &str) -> bool {
        if !self.prompt_on_air() {
            return false;
        }
        let bank = self.channels.current_bank();
        let echo = match self.prompt_mut(bank) {
            Some(pending) => pending.line.paste(text),
            None => Vec::new(),
        };
        if !echo.is_empty() {
            self.feed_prompt_channel(bank, &echo);
        }
        true
    }
}
