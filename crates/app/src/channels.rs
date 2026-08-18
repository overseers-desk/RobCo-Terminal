//! The channel model: what a window's sessions are numbered, which one is on
//! the air, and what the tube does when that changes.
//!
//! The rules, in full: sessions occupy numbered channel slots on
//! machine-scoped pages. The home page holds the local shells; every tmux
//! -CC attachment is a page of its own, anchored at channel 1 by the very
//! channel that attached. Within a page, a slot whose shell has exited goes
//! dark and stays dark: no renumbering. New channels take the lowest free
//! slot of their page.
//!
//! # What is here and what is not
//!
//! Everything in this file is the state machine: slot arithmetic, the current
//! pair, the kinds a row can be, and the transitions a tmux attachment puts it
//! through. What is *not* here is a gateway. A page row does not hold a live
//! client itself; that client is [`crate::tmux::Gateway`], held by the host
//! (`window::TerminalSurface`), and where this model would otherwise call it
//! directly it instead returns a [`Close`] or a `PageId` and lets the host do
//! it. The transitions themselves are pure, which is what gives
//! [`ChannelKind`] and the held-slot rule their meaning without a wire in the
//! room. One gateway law lives with the host, not here: the client-size
//! broadcast. The glass's grid has to reach *every* gateway on attach and on
//! resize, because a per-page publish silently corrupts the sizes tmux draws
//! other sessions at; the host's `publish_client_size` is that law's one
//! home.
//!
//! # One deliberate choice
//!
//! Every query walks [`Channels::rows`], which is at most 99 entries per
//! page, rather than maintaining a derived cache kept in sync by a single
//! writer. A cache invalidated by hand is only as sound as its own
//! discipline: "reassigned wholesale rather than mutated, so only the
//! assignment notifies readers" is a rule that has to be remembered at every
//! call site that ever touches the state. Walking the small, bounded row set
//! on every query removes the rule instead of keeping it.

use std::mem;

/// Slots are two engraved digits, so 99 is the last one a panel can name.
pub const CHANNEL_CAP: u32 = 99;

/// A page's id. Row 0 is home and never leaves; tmux attachments take
/// 1, 2, ... in attach order, and ids only ever grow, so sorting rows by
/// `(page, channel)` puts home first and the attachments behind it in the
/// order they arrived.
pub type PageId = u32;

/// What a page is: a page's kind speaks its own vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    /// This machine's own shells.
    Home,
    /// One `tmux -CC` attachment.
    Tmux,
}

/// What a channel row is: the row's kind is a separate enumeration from
/// [`PageKind`], and the two are never compared against each other. Both are
/// called "kind" because each speaks its own vocabulary, and collapsing them
/// into one type would blur two different questions ("what page is this?"
/// vs. "what role does this row play on it?") behind one shared name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelKind {
    /// A shell of this machine's.
    Local,
    /// A tmux window fed through its page's gateway.
    Remote,
    /// The transported channel standing at a page's slot 1.
    Anchor,
}

/// One machine-scoped page (`:43-46`).
#[derive(Clone, Debug)]
pub struct Page {
    pub id: PageId,
    pub kind: PageKind,
    pub host: String,
    /// The home slot the anchor holds while abroad. Meaningless on home.
    pub home_slot: u32,
    /// A window this page asked tmux for, still on its way: asking for a
    /// channel puts you in it, so the next window to arrive takes the air while
    /// the ones tmux volunteers do not (`:34-39`).
    pub follow: bool,
    /// The attachment's first window has arrived and the greeting is spent.
    ///
    /// The greeting is the attach's own one-off (see
    /// [`Channels::open_remote_channel`]) and this is what counts it. It has to
    /// be counted rather than read off the slot number: the first window lands
    /// on slot 2, but so does a window another client volunteers *after* the
    /// user has closed the one that was standing there, and that second slot-2
    /// window must not take the air.
    pub greeted: bool,
}

/// One channel slot with a session in it (`:100-107`).
#[derive(Clone, Debug)]
pub struct Row<S> {
    pub page: PageId,
    pub channel: u32,
    pub title: String,
    pub kind: ChannelKind,
    pub window_id: String,
    pub pane_id: String,
    /// What the slot holds. `app` puts a `term::Session` here; a test puts
    /// whatever it can assert on.
    pub session: S,
}

/// A page's stretch of the bank's flattened pager space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewPage {
    pub page: PageId,
    /// How many bank pages this store page unrolls into.
    pub count: i32,
}

/// What a close or a death asks the host for: the host is where the gateway
/// call or the window close actually happens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Close {
    /// No such row; nothing happened.
    Nothing,
    /// The row is gone.
    Removed,
    /// That was the last channel anywhere: the appliance switches off.
    CloseWindow,
    /// An anchor: detach the page's session. tmux keeps it, the channel comes
    /// home, and the row goes when the detach echoes back.
    Detach(PageId),
    /// A remote window is tmux's to kill; the row goes when `%window-close`
    /// comes back.
    KillWindow { page: PageId, window_id: String },
}

/// The per-window collection of channels and the pages they are numbered on.
pub struct Channels<S> {
    pages: Vec<Page>,
    /// Kept sorted ascending by `(page, channel)`.
    rows: Vec<Row<S>>,
    next_page_id: PageId,
    current_page: PageId,
    current_channel: u32,
    /// The page the bank is showing. `None` means this has never been set
    /// independently of the current page: a profile with no bank never
    /// writes it, and it simply follows the air.
    page_on_view: Option<PageId>,
    /// The set only flinches once it is on: bringing up the first channel is
    /// not a channel change.
    degauss_armed: bool,
    degauss_pending: bool,
    /// `channelStored` acknowledgements the bank has not blinked yet.
    stored: Vec<u32>,
}

impl<S> Default for Channels<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Channels<S> {
    /// An empty set with the home page and nothing on it, degauss disarmed:
    /// the state before anything has been opened.
    pub fn new() -> Self {
        Channels {
            pages: vec![Page {
                id: 0,
                kind: PageKind::Home,
                host: String::new(),
                home_slot: 0,
                follow: false,
                greeted: false,
            }],
            rows: Vec::new(),
            next_page_id: 1,
            current_page: 0,
            current_channel: 0,
            page_on_view: None,
            degauss_armed: false,
            degauss_pending: false,
            stored: Vec::new(),
        }
    }

    /// `Component.onCompleted` (`:698-701`): open channel 1 and arm the tube.
    /// The first channel is not a channel change, so nothing flinches.
    pub fn start(&mut self, session: impl FnOnce() -> Option<S>) {
        self.open_channel(0, 1, session);
        self.degauss_armed = true;
        self.degauss_pending = false;
    }

    // ---- reading the state ------------------------------------------

    pub fn rows(&self) -> &[Row<S>] {
        &self.rows
    }

    /// Every row, for the two things a host writes on one directly: pumping its
    /// session and taking the title that session's escapes set.
    ///
    /// `page` and `channel` are the sort key and are not a host's to write:
    /// where those move, the transition that moves them ([`Self::move_current_to`],
    /// [`Self::attach_gateway`], [`Self::collapse_page`]) re-sorts, and a write
    /// here would not.
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row<S>> {
        self.rows.iter_mut()
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn current_page(&self) -> PageId {
        self.current_page
    }

    pub fn current_channel(&self) -> u32 {
        self.current_channel
    }

    /// `:60-62`. The page the user is looking at: the bank binds its viewed
    /// page here while it steps the pager without stealing the air.
    pub fn page_on_view(&self) -> PageId {
        self.page_on_view.unwrap_or(self.current_page)
    }

    pub fn set_page_on_view(&mut self, page: PageId) {
        self.page_on_view = Some(page);
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    fn row_of(&self, page: PageId, channel: u32) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| r.page == page && r.channel == channel)
    }

    fn page_row_of(&self, page: PageId) -> Option<usize> {
        self.pages.iter().position(|p| p.id == page)
    }

    /// The title standing at a slot, or `None` for a dark one. Computed
    /// rather than cached (see the module doc).
    pub fn slot_title(&self, page: PageId, channel: u32) -> Option<&str> {
        self.row_of(page, channel)
            .map(|i| self.rows[i].title.as_str())
    }

    /// What the window title reads. `None` is where the caller falls back to
    /// the application's own name, which is the host's string to supply.
    pub fn current_title(&self) -> Option<&str> {
        match self.slot_title(self.current_page, self.current_channel) {
            Some(t) if !t.is_empty() => Some(t),
            _ => None,
        }
    }

    pub fn current(&self) -> Option<&Row<S>> {
        self.row_of(self.current_page, self.current_channel)
            .map(|i| &self.rows[i])
    }

    pub fn current_mut(&mut self) -> Option<&mut Row<S>> {
        self.row_of(self.current_page, self.current_channel)
            .map(|i| &mut self.rows[i])
    }

    /// Every session, current or not. Every channel's shell keeps running
    /// whether or not it is the one shown; a channel whose shell keeps
    /// printing while another is on the air is still printing when you come
    /// back to it.
    pub fn sessions_mut(&mut self) -> impl Iterator<Item = &mut S> {
        self.rows.iter_mut().map(|r| &mut r.session)
    }

    /// The session on the air, which is the one the glass is showing.
    pub fn session(&self) -> Option<&S> {
        self.current().map(|r| &r.session)
    }

    pub fn session_mut(&mut self) -> Option<&mut S> {
        self.current_mut().map(|r| &mut r.session)
    }

    /// `:138-145`. A transported channel's home slot stays dark and held while
    /// it is abroad; the hold lives on the page, not on any row.
    fn slot_held(&self, channel: u32) -> bool {
        self.pages
            .iter()
            .any(|p| p.kind == PageKind::Tmux && p.home_slot == channel)
    }

    /// `:149-168`. The lowest free slot of a page. On home the held slots count
    /// as taken; on an attachment the anchor holds 1, so windows fill from 2.
    /// Zero when the page is full to the cap.
    pub fn first_free(&self, page: PageId) -> u32 {
        let mut taken = [false; CHANNEL_CAP as usize + 1];
        for row in self.rows.iter().filter(|r| r.page == page) {
            if row.channel <= CHANNEL_CAP {
                taken[row.channel as usize] = true;
            }
        }
        if page == 0 {
            for p in self.pages.iter().filter(|p| p.kind == PageKind::Tmux) {
                if p.home_slot <= CHANNEL_CAP {
                    taken[p.home_slot as usize] = true;
                }
            }
        }
        (1..=CHANNEL_CAP).find(|n| !taken[*n as usize]).unwrap_or(0)
    }

    /// `:170-178`.
    pub fn highest_open(&self, page: PageId) -> u32 {
        self.rows
            .iter()
            .filter(|r| r.page == page)
            .map(|r| r.channel)
            .max()
            .unwrap_or(0)
    }

    /// `:184-195`. The bank's pager walks one flattened space over every page:
    /// each page unrolls into as many bank pages as its slots need, every open
    /// slot reachable and so is the next free one, the slot a new channel will
    /// take.
    pub fn view_pages(&self, rows_visible: i32) -> Vec<ViewPage> {
        let rows = rows_visible.max(1);
        self.pages
            .iter()
            .map(|p| {
                let span = self.highest_open(p.id).max(self.first_free(p.id)).max(1) as i32;
                ViewPage {
                    page: p.id,
                    // JS `Math.ceil(span / rows)` on two positive integers.
                    count: (span + rows - 1) / rows,
                }
            })
            .collect()
    }

    /// `:504-514`. True when `buf` is a strict prefix of some *open* slot of
    /// the named page's stretch rooted at `base`: the chord keeps waiting only
    /// for digits that can still land.
    pub fn page_slot_prefix_exists(&self, page: PageId, buf: &str, base: u32, count: u32) -> bool {
        (1..=count).any(|n| {
            if self.slot_title(page, base + n).is_none() {
                return false;
            }
            let s = n.to_string();
            s.len() > buf.len() && s.starts_with(buf)
        })
    }

    // ---- the tube ----------------------------------------------------

    /// Whether a channel change since the last call asked the tube to flinch,
    /// clearing the flag. `:706-713`: turning the knob makes the tube flinch,
    /// and turning it to another machine's page no less; re-selecting the
    /// current channel never reaches there, because nothing changed.
    pub fn take_degauss(&mut self) -> bool {
        mem::take(&mut self.degauss_pending)
    }

    /// The `channelStored` acknowledgements the bank has not blinked yet. At
    /// most two, because each store clears the last one's: an acknowledgement
    /// nobody drains is not queued either, and a blink for a store two stores
    /// ago would land on the wrong panel anyway.
    ///
    /// **No caller yet, deliberately left unconsumed.** The eventual consumer
    /// is a per-row blink: an animation that restarts on the row a store just
    /// landed on -- a *display* state, held by the window and animated over
    /// time. Neither display kit has one: `chassis::displays::{led,tape}` map
    /// a slot's `open`/`bright` to an appearance and hold nothing that varies
    /// with a clock, and the strip pieces the furniture emits are rebuilt
    /// whole per frame from [`chassis::BankStrips`]. So there is nowhere to
    /// drain this to that would show anything, and a drain that dropped the
    /// acknowledgements on the floor would be worse than none: the model
    /// would look wired. The list is bounded at two by
    /// [`Channels::move_current_to`]'s own clear, so leaving it undrained
    /// costs nothing. Whoever gives a strip a clock takes this on too.
    pub fn take_stored(&mut self) -> Vec<u32> {
        mem::take(&mut self.stored)
    }

    /// The one writer of the current pair, and so the one place the tube is
    /// asked to flinch.
    fn set_current(&mut self, page: PageId, channel: u32) {
        let moved = page != self.current_page || channel != self.current_channel;
        self.current_page = page;
        self.current_channel = channel;
        if moved && self.degauss_armed {
            self.degauss_pending = true;
        }
    }

    // ---- opening -----------------------------------------------------

    /// `:213-225`. Local shells live on home alone: an attachment's channels
    /// are tmux's to give. A held slot refuses; it is a transported channel's
    /// berth. `session` is called only once the slot is known to be takeable,
    /// so a refused open costs no pty.
    pub fn open_channel(
        &mut self,
        page: PageId,
        channel: u32,
        session: impl FnOnce() -> Option<S>,
    ) -> bool {
        if page != 0 {
            return false;
        }
        if !(1..=CHANNEL_CAP).contains(&channel) {
            return false;
        }
        if self.row_of(page, channel).is_some() || self.slot_held(channel) {
            return false;
        }
        let Some(session) = session() else {
            return false;
        };
        self.insert_row(Row {
            page: 0,
            channel,
            title: String::new(),
            kind: ChannelKind::Local,
            window_id: String::new(),
            pane_id: String::new(),
            session,
        });
        self.set_current(0, channel);
        true
    }

    /// `:284-288`.
    pub fn open_first_free(&mut self, session: impl FnOnce() -> Option<S>) -> bool {
        let slot = self.first_free(0);
        if slot == 0 {
            return false;
        }
        self.open_channel(0, slot, session)
    }

    /// `:292-301`: what `Ctrl+Shift+T` asks for. A new channel goes to the page
    /// on view: on home the lowest free slot with a shell in it, on an
    /// attachment another window of that session, which is the gateway's to
    /// give. Returns the page whose gateway must be asked, having set its
    /// `follow` flag, or `None` when the shell was opened here.
    pub fn new_channel(&mut self, session: impl FnOnce() -> Option<S>) -> Option<PageId> {
        let view = self.page_on_view();
        match self.page_row_of(view) {
            Some(i) if self.pages[i].kind == PageKind::Tmux => {
                self.pages[i].follow = true;
                Some(view)
            }
            _ => {
                self.open_first_free(session);
                None
            }
        }
    }

    /// `:307-313`. Ask the page the air is on for another window, by name
    /// rather than by where the bank stands.
    pub fn new_remote_channel(&mut self) -> Option<PageId> {
        let page = self.current_page;
        let i = self.page_row_of(page)?;
        if self.pages[i].kind != PageKind::Tmux {
            return None;
        }
        self.pages[i].follow = true;
        Some(page)
    }

    /// `:232-258`. A tmux window becomes an ordinary channel on the lowest free
    /// slot of its page, from 2 up: the anchor holds 1. Windows the attach
    /// lists line up behind the anchor without taking the air; a window this
    /// set asked for is the exception and takes it outright.
    pub fn open_remote_channel(
        &mut self,
        page: PageId,
        window_id: &str,
        pane_id: &str,
        name: &str,
        session: impl FnOnce() -> Option<S>,
    ) -> bool {
        let channel = self.first_free(page);
        if channel < 1 {
            return false;
        }
        let Some(page_row) = self.page_row_of(page) else {
            return false;
        };
        // Whatever arrives next answers the request, and nothing after it does.
        let asked = self.pages[page_row].follow;
        if asked {
            self.pages[page_row].follow = false;
        }
        // An attach's first window also takes the air: the user who typed
        // `tmux -CC` is greeted by a live prompt, the anchor standing by at
        // slot 1, not by the anchor's own frozen glass.
        //
        // "First" is the attachment's own count (`Page::greeted`) and not the
        // slot number. Written as `channel == 2` it was a positional test that
        // fires again every time slot 2 falls vacant: close the window standing
        // there, land back on the anchor, and the next window another client
        // volunteers -- one this set never asked for -- would take the air off
        // the anchor as if it were the greeting.
        let greets =
            !self.pages[page_row].greeted && self.current_page == page && self.current_channel == 1;
        let Some(session) = session() else {
            return false;
        };
        // The greeting is spent by the window arriving, wherever the air
        // happened to be standing: a first window that arrived while the user
        // was on another page does not leave the greeting owing to the second.
        self.pages[page_row].greeted = true;
        self.insert_row(Row {
            page,
            channel,
            title: normalize_title(name),
            kind: ChannelKind::Remote,
            window_id: window_id.to_string(),
            pane_id: pane_id.to_string(),
            session,
        });
        if asked || greets {
            self.set_current(page, channel);
        }
        true
    }

    fn insert_row(&mut self, row: Row<S>) {
        let dest = self
            .rows
            .iter()
            .position(|r| !less_than(r, &row))
            .unwrap_or(self.rows.len());
        self.rows.insert(dest, row);
    }

    /// `:273-282`. A row whose page or channel was rewritten in place slides to
    /// where the sort order wants it.
    fn resort(&mut self) {
        self.rows.sort_by_key(|r| (r.page, r.channel));
    }

    // ---- closing -----------------------------------------------------

    /// `:320-341`. What the user's close asks for, by what the row is.
    pub fn close_channel(&mut self, page: PageId, channel: u32) -> Close {
        let Some(index) = self.row_of(page, channel) else {
            return Close::Nothing;
        };
        match self.rows[index].kind {
            ChannelKind::Anchor => Close::Detach(page),
            ChannelKind::Remote => Close::KillWindow {
                page,
                window_id: self.rows[index].window_id.clone(),
            },
            ChannelKind::Local => {
                if self.rows.len() <= 1 {
                    return Close::CloseWindow;
                }
                self.remove_row(page, channel);
                Close::Removed
            }
        }
    }

    /// `:348-361`. A row's own program died. For a local shell that is the
    /// ordinary end of a channel; for an anchor it is the gateway dying under
    /// the session, and the page collapses.
    pub fn session_died(&mut self, page: PageId, channel: u32) -> Close {
        let Some(index) = self.row_of(page, channel) else {
            return Close::Nothing;
        };
        if self.rows[index].kind == ChannelKind::Anchor {
            return self.anchor_died(page);
        }
        if self.rows.len() <= 1 {
            return Close::CloseWindow;
        }
        self.remove_row(page, channel);
        Close::Removed
    }

    /// `:366-379`. The anchor's own shell died: collapse the page as a detach
    /// would, which lands its row home as a local channel, then remove that
    /// row, there being no live process to come home to.
    fn anchor_died(&mut self, page: PageId) -> Close {
        let Some(page_row) = self.page_row_of(page) else {
            return Close::Nothing;
        };
        let home_slot = self.pages[page_row].home_slot;
        self.collapse_page(page);
        self.remove_row(0, home_slot);
        // Nothing survived it: the appliance has no channel left to show.
        if self.rows.is_empty() {
            Close::CloseWindow
        } else {
            Close::Removed
        }
    }

    /// `:384-399`. Where a single row goes. The nearest surviving row of the
    /// same page takes the air when the removed one had it.
    fn remove_row(&mut self, page: PageId, channel: u32) {
        let Some(index) = self.row_of(page, channel) else {
            return;
        };
        let was_current = page == self.current_page && channel == self.current_channel;
        self.rows.remove(index);
        if was_current {
            if let Some(next) = self.nearest_row(index, page) {
                let (p, c) = (self.rows[next].page, self.rows[next].channel);
                self.set_current(p, c);
            }
        }
    }

    /// `:404-416`. The row nearest the hole a removed row left, its own page's
    /// rows first: the one that slid into its place, else the nearest one
    /// before it, else whatever another page still holds.
    fn nearest_row(&self, index: usize, page: PageId) -> Option<usize> {
        if let Some(after) = (index..self.rows.len()).find(|i| self.rows[*i].page == page) {
            return Some(after);
        }
        if let Some(before) = (0..index.min(self.rows.len()))
            .rev()
            .find(|i| self.rows[*i].page == page)
        {
            return Some(before);
        }
        if !self.rows.is_empty() {
            return Some(index.min(self.rows.len() - 1));
        }
        None
    }

    // ---- selecting and storing ---------------------------------------

    /// `:418-426`. A dark slot is not selectable; the bank opens one instead.
    pub fn select_channel(&mut self, page: PageId, channel: u32) -> bool {
        if self.row_of(page, channel).is_none() {
            return false;
        }
        self.set_current(page, channel);
        true
    }

    /// `:433-480`. The session on screen keeps its screen; its slot number
    /// moves. An occupied slot swaps the two sessions' numbers.
    ///
    /// The LED blink is the store's acknowledgement, so the tube holds steady
    /// throughout: the degauss is disarmed for the move and re-armed after
    /// it.
    pub fn move_current_to(&mut self, page: PageId, channel: u32) -> bool {
        // The bank can be viewing one machine's page while another holds the
        // air, and a store chord lands on the page on view: there is no slot on
        // it the session could take without leaving its own machine. Nothing
        // happens, which is the whole of the answer.
        if page != self.current_page {
            return false;
        }
        if !(1..=CHANNEL_CAP).contains(&channel) || channel == self.current_channel {
            return false;
        }
        let origin = self.current_channel;
        let Some(from) = self.row_of(page, origin) else {
            return false;
        };
        if self.rows[from].kind == ChannelKind::Anchor {
            return false;
        }
        if page == 0 && self.slot_held(channel) {
            return false;
        }
        let to = self.row_of(page, channel);
        if to.is_some_and(|i| self.rows[i].kind == ChannelKind::Anchor) {
            return false;
        }

        let armed = self.degauss_armed;
        self.degauss_armed = false;
        self.stored.clear();
        // Swapping the two rows' channel numbers, on a list kept sorted by
        // `(page, channel)`, is exactly an exchange of the two numbers.
        self.rows[from].channel = channel;
        if let Some(to) = to {
            self.rows[to].channel = origin;
        }
        self.resort();
        self.set_current(page, channel);
        self.degauss_armed = armed;
        self.stored.push(channel);
        // A swap lands two sessions, and the displaced one gets its own say.
        if to.is_some() {
            self.stored.push(origin);
        }
        true
    }

    /// `:484-499`. Cycling walks the current page: the other machines' channels
    /// are a pager step away, not a knob turn.
    pub fn cycle_open(&mut self, direction: i32) {
        let slots: Vec<u32> = self
            .rows
            .iter()
            .filter(|r| r.page == self.current_page)
            .map(|r| r.channel)
            .collect();
        let Some(pos) = slots.iter().position(|c| *c == self.current_channel) else {
            return;
        };
        if slots.is_empty() {
            return;
        }
        let len = slots.len() as i32;
        let next = ((pos as i32 + direction) % len + len) % len;
        let page = self.current_page;
        self.select_channel(page, slots[next as usize]);
    }

    /// `:516-522`. A local shell's title is its own; abroad the model owns it,
    /// and a remote channel's title is tmux's to give (`:750-754`).
    pub fn set_title(&mut self, page: PageId, channel: u32, raw: &str) -> bool {
        let Some(index) = self.row_of(page, channel) else {
            return false;
        };
        let title = normalize_title(raw);
        if self.rows[index].title == title {
            return false;
        }
        self.rows[index].title = title;
        true
    }

    // ---- the tmux transitions ----------------------------------------

    /// `:530-559`. A channel's program has entered tmux control mode: the
    /// channel transports to slot 1 of a new page, titled for the machine the
    /// session is on, and its home slot is held dark behind it. The row mutates
    /// in place, never removes, so the glass keeps the screen it was showing.
    /// Transport is a renumbering, not a channel change, so the tube holds
    /// steady. Returns the new page's id.
    pub fn attach_gateway(&mut self, page: PageId, channel: u32, host: &str) -> Option<PageId> {
        let index = self.row_of(page, channel)?;
        if self.rows[index].kind != ChannelKind::Local {
            return None;
        }
        let page_id = self.next_page_id;
        self.next_page_id += 1;
        self.pages.push(Page {
            id: page_id,
            kind: PageKind::Tmux,
            host: host.to_string(),
            home_slot: channel,
            follow: false,
            greeted: false,
        });
        let was_current = page == self.current_page && channel == self.current_channel;
        let armed = self.degauss_armed;
        self.degauss_armed = false;
        self.rows[index].page = page_id;
        self.rows[index].channel = 1;
        self.rows[index].kind = ChannelKind::Anchor;
        self.rows[index].title = anchor_title(host);
        self.resort();
        if was_current {
            self.set_current(page_id, 1);
        }
        self.degauss_armed = armed;
        Some(page_id)
    }

    /// `:577-608`. Detach or gateway death: the page's remote rows vanish, the
    /// anchor transports home to the slot it never gave up and relights, and
    /// the user lands on it.
    pub fn collapse_page(&mut self, page: PageId) {
        let Some(page_row) = self.page_row_of(page) else {
            return;
        };
        if self.pages[page_row].kind != PageKind::Tmux {
            return;
        }
        let home_slot = self.pages[page_row].home_slot;
        let armed = self.degauss_armed;
        self.degauss_armed = false;
        self.rows
            .retain(|r| !(r.page == page && r.kind == ChannelKind::Remote));
        if let Some(anchor) = self.rows.iter().position(|r| r.page == page) {
            self.rows[anchor].page = 0;
            self.rows[anchor].channel = home_slot;
            self.rows[anchor].kind = ChannelKind::Local;
            self.resort();
        }
        self.pages.remove(page_row);
        if self.page_on_view == Some(page) {
            self.page_on_view = None;
        }
        self.set_current(0, home_slot);
        self.degauss_armed = armed;
    }

    /// `:669-681`. The host can resolve after the handshake: the page and its
    /// anchor's title follow it.
    pub fn host_changed(&mut self, page: PageId, host: &str) {
        let Some(page_row) = self.page_row_of(page) else {
            return;
        };
        self.pages[page_row].host = host.to_string();
        let title = anchor_title(host);
        for row in self.rows.iter_mut() {
            if row.page == page && row.kind == ChannelKind::Anchor {
                row.title = title.clone();
            }
        }
    }

    /// `:683-690`.
    pub fn channel_of_window(&self, page: PageId, window_id: &str) -> u32 {
        self.rows
            .iter()
            .find(|r| r.page == page && r.kind == ChannelKind::Remote && r.window_id == window_id)
            .map(|r| r.channel)
            .unwrap_or(0)
    }

    /// `:656-660`. tmux says a window closed; its row goes.
    pub fn window_closed(&mut self, page: PageId, window_id: &str) {
        let channel = self.channel_of_window(page, window_id);
        if channel > 0 {
            self.remove_row(page, channel);
        }
    }
}

/// `:112-117`.
fn normalize_title(raw: &str) -> String {
    raw.trim().to_string()
}

/// `:547`. What an anchor's row reads while it is abroad.
fn anchor_title(host: &str) -> String {
    format!("tmux -CC # @{host}")
}

/// `:267-269`.
fn less_than<S>(a: &Row<S>, b: &Row<S>) -> bool {
    if a.page != b.page {
        a.page < b.page
    } else {
        a.channel < b.channel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A channel's payload in these tests: the slot it was opened on, so a
    /// swap that moved a number rather than a session would show.
    fn channels() -> Channels<u32> {
        let mut set = Channels::new();
        let mut next = 100;
        set.start(|| {
            next += 1;
            Some(next)
        });
        set
    }

    fn open(set: &mut Channels<u32>, page: PageId, channel: u32, mark: u32) -> bool {
        set.open_channel(page, channel, || Some(mark))
    }

    #[test]
    fn the_first_channel_is_slot_one_and_does_not_flinch_the_tube() {
        let mut set = channels();
        assert_eq!(set.current_channel(), 1);
        assert_eq!(set.current_page(), 0);
        assert_eq!(set.len(), 1);
        assert!(!set.take_degauss(), "the set only flinches once it is on");
    }

    #[test]
    fn new_channels_take_the_lowest_free_slot() {
        let mut set = channels();
        assert!(set.open_first_free(|| Some(2)));
        assert_eq!(set.current_channel(), 2);
        assert!(set.open_first_free(|| Some(3)));
        assert_eq!(set.current_channel(), 3);
        // A hole left by a close is the lowest free slot.
        set.close_channel(0, 2);
        assert_eq!(set.first_free(0), 2);
        assert!(set.open_first_free(|| Some(9)));
        assert_eq!(set.current_channel(), 2);
    }

    #[test]
    fn an_exited_slot_goes_dark_and_stays_dark() {
        let mut set = channels();
        open(&mut set, 0, 2, 2);
        open(&mut set, 0, 3, 3);
        set.select_channel(0, 3);
        set.close_channel(0, 2);
        // No renumbering: 3 is still 3.
        assert!(set.slot_title(0, 2).is_none());
        assert_eq!(set.current_channel(), 3);
        assert_eq!(set.current().unwrap().session, 3);
    }

    #[test]
    fn a_taken_slot_refuses_a_second_shell() {
        let mut set = channels();
        assert!(!open(&mut set, 0, 1, 9));
        assert_eq!(set.len(), 1);
        // ...and so does a slot outside the engraved range.
        assert!(!open(&mut set, 0, 0, 9));
        assert!(!open(&mut set, 0, CHANNEL_CAP + 1, 9));
    }

    #[test]
    fn a_channel_switch_asks_the_tube_to_flinch_and_a_re_selection_does_not() {
        let mut set = channels();
        open(&mut set, 0, 2, 2);
        assert!(set.take_degauss(), "opening 2 moved the air onto it");
        set.select_channel(0, 2);
        assert!(!set.take_degauss(), "nothing changed");
        set.select_channel(0, 1);
        assert!(set.take_degauss());
        set.select_channel(0, 42);
        assert!(!set.take_degauss(), "a dark slot is not selectable");
    }

    #[test]
    fn a_store_holds_the_tube_steady_and_blinks_the_lamp_instead() {
        let mut set = channels();
        open(&mut set, 0, 2, 22);
        let _ = set.take_degauss();
        assert!(set.move_current_to(0, 7));
        assert_eq!(set.current_channel(), 7);
        assert_eq!(
            set.current().unwrap().session,
            22,
            "the session moved with it"
        );
        assert!(!set.take_degauss(), "a store is not a channel change");
        assert_eq!(set.take_stored(), vec![7]);
    }

    #[test]
    fn a_store_onto_an_open_slot_swaps_the_two_sessions() {
        let mut set = channels();
        // Slot 1 carries 101 from `start`; open 2 with a mark of its own.
        let one = set.slot_title(0, 1).map(|_| set.rows()[0].session).unwrap();
        open(&mut set, 0, 2, 22);
        assert!(set.move_current_to(0, 1));
        assert_eq!(set.current_channel(), 1);
        assert_eq!(set.current().unwrap().session, 22);
        let displaced = set.rows().iter().find(|r| r.channel == 2).unwrap();
        assert_eq!(displaced.session, one);
        // Both panels acknowledge: the one stored onto and the one displaced.
        assert_eq!(set.take_stored(), vec![1, 2]);
    }

    #[test]
    fn cycling_walks_the_open_slots_of_the_current_page_and_wraps() {
        let mut set = channels();
        open(&mut set, 0, 4, 4);
        open(&mut set, 0, 9, 9);
        set.select_channel(0, 1);
        set.cycle_open(1);
        assert_eq!(set.current_channel(), 4);
        set.cycle_open(1);
        assert_eq!(set.current_channel(), 9);
        set.cycle_open(1);
        assert_eq!(set.current_channel(), 1, "wraps");
        set.cycle_open(-1);
        assert_eq!(set.current_channel(), 9, "and backwards");
    }

    #[test]
    fn the_last_channel_closing_switches_the_appliance_off() {
        let mut set = channels();
        assert_eq!(set.close_channel(0, 1), Close::CloseWindow);
        assert_eq!(
            set.len(),
            1,
            "the row stays; the window is the host's to close"
        );
        open(&mut set, 0, 2, 2);
        assert_eq!(set.close_channel(0, 2), Close::Removed);
        assert_eq!(set.session_died(0, 1), Close::CloseWindow);
    }

    #[test]
    fn a_title_is_trimmed_and_read_back_off_the_current_pair() {
        let mut set = channels();
        assert_eq!(set.current_title(), None);
        set.set_title(0, 1, "  ~/src  ");
        assert_eq!(set.current_title(), Some("~/src"));
        set.set_title(0, 1, "   ");
        assert_eq!(set.current_title(), None, "an empty title is the fallback");
    }

    #[test]
    fn view_pages_unroll_every_page_far_enough_to_reach_its_next_free_slot() {
        let mut set = channels();
        // One channel: one bank page, which still has to reach slot 2.
        assert_eq!(set.view_pages(4), vec![ViewPage { page: 0, count: 1 }]);
        open(&mut set, 0, 4, 4);
        // Slots 1..4 open, next free 2, so four rows fit one page of four.
        assert_eq!(set.view_pages(4), vec![ViewPage { page: 0, count: 1 }]);
        open(&mut set, 0, 5, 5);
        assert_eq!(set.view_pages(4), vec![ViewPage { page: 0, count: 2 }]);
    }

    #[test]
    fn the_prefix_predicate_waits_only_on_digits_that_can_still_land() {
        let mut set = channels();
        open(&mut set, 0, 12, 12);
        // With 1 and 12 open on a page of 20 rooted at 0: "1" is a strict
        // prefix of "12", so the chord waits.
        assert!(set.page_slot_prefix_exists(0, "1", 0, 20));
        // "12" is nobody's strict prefix, so it commits at once.
        assert!(!set.page_slot_prefix_exists(0, "12", 0, 20));
        // Nothing beginning with 3 is open.
        assert!(!set.page_slot_prefix_exists(0, "3", 0, 20));
        // ...and a page of ten rows cannot reach slot 12 at all.
        assert!(!set.page_slot_prefix_exists(0, "1", 0, 10));
    }

    // ---- the tmux transitions, without a gateway ---------------------

    #[test]
    fn attaching_transports_the_channel_to_slot_one_of_a_new_page() {
        let mut set = channels();
        open(&mut set, 0, 3, 33);
        let _ = set.take_degauss();
        let page = set.attach_gateway(0, 3, "prime").unwrap();
        assert_eq!((set.current_page(), set.current_channel()), (page, 1));
        assert_eq!(set.current().unwrap().session, 33, "the same session");
        assert_eq!(set.current().unwrap().kind, ChannelKind::Anchor);
        assert_eq!(set.current_title(), Some("tmux -CC # @prime"));
        assert!(
            !set.take_degauss(),
            "transport is a renumbering, not a channel change"
        );
        // Home slot 3 is held dark behind it: nothing may take it.
        assert!(set.slot_title(0, 3).is_none());
        assert!(!open(&mut set, 0, 3, 99));
        assert_eq!(set.first_free(0), 2);
    }

    #[test]
    fn an_attachments_windows_fill_from_slot_two_and_the_first_takes_the_air() {
        let mut set = channels();
        let page = set.attach_gateway(0, 1, "prime").unwrap();
        assert!(set.open_remote_channel(page, "@1", "%1", "vim", || Some(1)));
        assert_eq!(
            (set.current_page(), set.current_channel()),
            (page, 2),
            "an attach's first window greets the user"
        );
        // The ones tmux volunteers after it line up without taking the air.
        assert!(set.open_remote_channel(page, "@2", "%2", "logs", || Some(2)));
        assert_eq!(set.current_channel(), 2);
        assert_eq!(set.slot_title(page, 3), Some("logs"));
        // ...but one this set asked for takes it outright.
        assert_eq!(set.new_remote_channel(), Some(page));
        assert!(set.open_remote_channel(page, "@3", "%3", "asked", || Some(3)));
        assert_eq!(set.current_channel(), 4);
    }

    /// The greeting is the attach's one-off, not a property of slot 2. Written
    /// as `channel == 2` it fired again every time slot 2 fell vacant, so a
    /// window another client volunteered took the air off an anchor the user
    /// was reading.
    #[test]
    fn a_volunteered_window_landing_back_on_slot_two_does_not_take_the_air() {
        let mut set = channels();
        let page = set.attach_gateway(0, 1, "prime").unwrap();
        // The greeting, spent.
        assert!(set.open_remote_channel(page, "@1", "%1", "vim", || Some(1)));
        assert_eq!(set.current_channel(), 2);

        // The user closes it and lands back on the anchor, leaving slot 2 free.
        set.window_closed(page, "@1");
        set.select_channel(page, 1);
        assert_eq!(set.current_channel(), 1);
        assert_eq!(set.first_free(page), 2);

        // Another client runs `tmux new-window`: this set never asked, so the
        // window lines up behind the anchor and the anchor keeps the air.
        assert!(set.open_remote_channel(page, "@2", "%2", "volunteered", || { Some(2) }));
        assert_eq!(set.slot_title(page, 2), Some("volunteered"));
        assert_eq!(
            set.current_channel(),
            1,
            "a volunteered window does not yank the user off the anchor"
        );

        // A window this set *did* ask for still takes it outright.
        assert_eq!(set.new_remote_channel(), Some(page));
        assert!(set.open_remote_channel(page, "@3", "%3", "asked", || Some(3)));
        assert_eq!(set.current_channel(), 3);
    }

    #[test]
    fn a_collapse_brings_the_anchor_home_to_the_slot_it_never_gave_up() {
        let mut set = channels();
        open(&mut set, 0, 5, 55);
        let page = set.attach_gateway(0, 5, "prime").unwrap();
        set.open_remote_channel(page, "@1", "%1", "vim", || Some(1));
        set.collapse_page(page);
        assert_eq!((set.current_page(), set.current_channel()), (0, 5));
        assert_eq!(set.current().unwrap().session, 55);
        assert_eq!(set.current().unwrap().kind, ChannelKind::Local);
        assert_eq!(set.len(), 2, "the remote window went with the page");
        assert_eq!(set.pages().len(), 1);
    }

    #[test]
    fn an_anchors_death_collapses_the_page_and_takes_the_returned_row_with_it() {
        let mut set = channels();
        open(&mut set, 0, 2, 22);
        let page = set.attach_gateway(0, 2, "prime").unwrap();
        set.open_remote_channel(page, "@1", "%1", "vim", || Some(1));
        assert_eq!(set.session_died(page, 1), Close::Removed);
        // Only home's slot 1 survives.
        assert_eq!(set.len(), 1);
        assert_eq!((set.current_page(), set.current_channel()), (0, 1));
        assert_eq!(set.pages().len(), 1);
    }

    #[test]
    fn a_store_never_crosses_pages_and_an_anchor_never_moves() {
        let mut set = channels();
        let page = set.attach_gateway(0, 1, "prime").unwrap();
        set.open_remote_channel(page, "@1", "%1", "vim", || Some(1));
        // The air is on slot 2 of the attachment; a store aimed at home does
        // nothing at all.
        assert!(!set.move_current_to(0, 4));
        // The anchor refuses to be stored onto...
        assert!(!set.move_current_to(page, 1));
        // ...and refuses to move when it holds the air.
        set.select_channel(page, 1);
        assert!(!set.move_current_to(page, 5));
    }

    #[test]
    fn a_close_of_a_remote_row_is_tmuxs_to_do_and_the_row_stays_until_it_says() {
        let mut set = channels();
        let page = set.attach_gateway(0, 1, "prime").unwrap();
        set.open_remote_channel(page, "@7", "%7", "vim", || Some(1));
        assert_eq!(
            set.close_channel(page, 2),
            Close::KillWindow {
                page,
                window_id: "@7".into()
            }
        );
        assert_eq!(set.len(), 2, "the row waits for %window-close");
        set.window_closed(page, "@7");
        assert_eq!(set.len(), 1);
        // And an anchor's close is a detach.
        assert_eq!(set.close_channel(page, 1), Close::Detach(page));
    }

    #[test]
    fn ctrl_shift_t_acts_on_the_page_the_bank_is_showing() {
        let mut set = channels();
        let page = set.attach_gateway(0, 1, "prime").unwrap();
        // The air is abroad, but the bank has stepped back to home: the new
        // channel is a local shell, not a tmux window.
        set.set_page_on_view(0);
        assert_eq!(set.new_channel(|| Some(7)), None);
        assert_eq!(set.slot_title(0, 2), Some(""));
        // Viewing the attachment, the same key asks its gateway instead.
        set.set_page_on_view(page);
        assert_eq!(set.new_channel(|| Some(8)), Some(page));
        assert!(set.pages().iter().find(|p| p.id == page).unwrap().follow);
    }

    #[test]
    fn the_host_can_resolve_after_the_handshake() {
        let mut set = channels();
        let page = set.attach_gateway(0, 1, "").unwrap();
        assert_eq!(set.current_title(), Some("tmux -CC # @"));
        set.host_changed(page, "prime");
        assert_eq!(set.current_title(), Some("tmux -CC # @prime"));
    }
}
