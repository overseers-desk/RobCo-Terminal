# ::rcsettings::ui::app - the window, and the two things it owns that no row
# does: the snapshot Cancel restores, and the watch on the file the rows are
# not the only writer of.
#
# There is no Apply button because there is nothing to apply: every row has
# already written, and the terminal has already redrawn. What OK and Cancel
# choose between is therefore not "save or discard" but "keep the file as it
# now stands, or put back the bytes it had when this window opened". The
# snapshot is taken as bytes rather than as settings, so restoring it returns
# the user's comments, key order and spacing along with their values, and an
# absent file is restored by deleting the one this window created.
#
# The file is also editable by hand and by the terminal itself while this
# window is open, so the window re-reads on regaining focus. Focus is the
# cheapest moment that is always after the other writer finished: a poll would
# have to run forever to catch an edit nobody is about to look at.

package require Tcl 9
package require Tk

namespace eval ::rcsettings::ui::app {
    namespace export start

    # The config file exactly as this window found it, and whether it was
    # there at all. Cancel restores both.
    variable Snapshot ""
    variable Existed 0
    # mtime and size of the file at the last read this window knows about.
    # Size joins mtime because mtime has one-second resolution on most
    # filesystems and a hand edit landing in the same second as ours would
    # otherwise be invisible.
    variable Stamp ""
    # The after token of the transient status message, so a second message
    # does not get wiped by the first one's expiry.
    variable StatusAfter ""
    variable Status ""
}

proc ::rcsettings::ui::app::start {} {
    # Nothing is drawn until the model is up: a window that appeared and then
    # replaced itself with an error about a missing binary would be worse
    # than the error alone.
    wm withdraw .
    styles

    if {[catch {::rcsettings::dump::load} dumpdata]} {
        tk_messageBox -icon error -title "RobCo Terminal Settings" \
            -message "Cannot read the terminal's settings schema." \
            -detail $dumpdata
        exit 1
    }
    if {[catch {::rcsettings::model::init $dumpdata} err]} {
        tk_messageBox -icon error -title "RobCo Terminal Settings" \
            -message "Cannot read the configuration file." -detail $err
        exit 1
    }

    snapshot
    build
    note_stamp
    wm deiconify .
}

# The ttk base, and the two styles this window needs for function. The base
# is clam rather than the platform theme: clam draws every widget in pure Tk
# and so puts the same window on X11, Windows and macOS, where the X11 default
# theme and aqua each ignore a different subset of the options a form sets.
# Everything else wears clam's stock look, this window having no business
# theming itself when the terminal's own glass is what the user chose.
proc ::rcsettings::ui::app::styles {} {
    ttk::style theme use clam
    # The root is a plain Tk widget and does not follow ttk's background, so
    # it is told what clam's own frames use.
    . configure -background [ttk::style lookup TFrame -background]
    # An error on the status line has to be readable as an error without the
    # message being read, so it is the one thing here that carries a colour.
    ttk::style configure StatusError.TLabel -foreground #b00020
    ::rcsettings::ui::form::styles
}

# The bytes Cancel puts back, and whether there were any.
proc ::rcsettings::ui::app::snapshot {} {
    variable Snapshot
    variable Existed
    set path [::rcsettings::model::path]
    set Existed [file exists $path]
    set Snapshot [::rcsettings::toml::read_file $path]
}

proc ::rcsettings::ui::app::build {} {
    variable Status
    wm title . "RobCo Terminal Settings"
    # The window manager's close button keeps what has been written, because
    # every change is already on the glass and closing the window is not the
    # gesture that means "undo the afternoon".
    wm protocol . WM_DELETE_WINDOW [list ::rcsettings::ui::app::ok]

    # The status line is built before the pages, because building a page
    # repaints its rows and a row that cannot be read reports through here.
    set Status .status
    # The width is asked for in characters rather than left to the text: the
    # resting message is the config file's path, and a path in a deep
    # directory would otherwise open the window as wide as it is long.
    ttk::label $Status -anchor w -width 40 -padding {8 3}
    ttk::separator .sep -orient horizontal
    ttk::frame .buttons -padding {10 8}
    ttk::button .buttons.cancel -text "Cancel" \
        -command [list ::rcsettings::ui::app::cancel]
    ttk::button .buttons.ok -text "OK" \
        -command [list ::rcsettings::ui::app::ok]
    pack .buttons.ok -side right
    pack .buttons.cancel -side right -padx {0 8}

    ::rcsettings::ui::form::init \
        [list ::rcsettings::ui::app::say] [list ::rcsettings::ui::app::wrote]
    ::rcsettings::ui::ssh_page::init \
        [list ::rcsettings::ui::app::say] [list ::rcsettings::ui::app::wrote]

    ttk::notebook .nb -padding {8 8 8 0}
    foreach {table title} {general General screen Screen chassis Chassis} {
        .nb add [::rcsettings::ui::form::page .nb $table] -text $title
    }
    # The SSH tab is a list of rows rather than a page of keys, so it is its
    # own page rather than a fourth entry in the form's layout.
    .nb add [::rcsettings::ui::ssh_page::page .nb] -text "SSH"

    grid .nb      -row 0 -column 0 -sticky nsew
    grid .sep     -row 1 -column 0 -sticky ew
    grid .buttons -row 2 -column 0 -sticky ew
    grid $Status  -row 3 -column 0 -sticky ew
    grid columnconfigure . 0 -weight 1
    grid rowconfigure    . 0 -weight 1

    rest
    bind . <FocusIn> [list ::rcsettings::ui::app::on_focus %W]
}

# ------------------------------------------------------------ status line --

# The resting text: which file this window is editing. Every other message is
# transient and falls back to this.
proc ::rcsettings::ui::app::rest {} {
    variable Status
    variable StatusAfter
    if {$StatusAfter ne ""} { after cancel $StatusAfter; set StatusAfter "" }
    $Status configure -style "" -text [::rcsettings::model::path]
}

proc ::rcsettings::ui::app::say {msg {isError 0}} {
    variable Status
    variable StatusAfter
    if {$Status eq "" || ![winfo exists $Status]} { return }
    if {$StatusAfter ne ""} { after cancel $StatusAfter }
    $Status configure -text $msg \
        -style [expr {$isError ? "StatusError.TLabel" : ""}]
    # An error stands until something else happens; a report of a write that
    # worked is not worth reading twice and clears itself.
    set StatusAfter [expr {$isError ? "" \
        : [after 4000 [list ::rcsettings::ui::app::rest]]}]
}

# ------------------------------------------------- the file's other writers --

proc ::rcsettings::ui::app::stamp {} {
    set path [::rcsettings::model::path]
    if {![file exists $path]} { return "absent" }
    return [list [file mtime $path] [file size $path]]
}

# Called by the form after every write it lands, so this window's own edit is
# not read back as somebody else's.
proc ::rcsettings::ui::app::wrote {} {
    note_stamp
}

proc ::rcsettings::ui::app::note_stamp {} {
    variable Stamp
    set Stamp [stamp]
}

# The toplevel regaining focus. The binding fires for descendants too as focus
# moves between the rows, and re-reading the file on every combobox close
# would be work for nothing, so only the toplevel's own event counts.
proc ::rcsettings::ui::app::on_focus {w} {
    variable Stamp
    if {$w ne "."} { return }
    set now [stamp]
    if {$now eq $Stamp} { return }
    set Stamp $now
    # The model reads the file; a file that no longer parses the way this
    # tool reads it leaves the last good view standing rather than blanking
    # the form, which is the same choice the terminal makes on a bad parse.
    if {[catch {::rcsettings::model::reload} err]} {
        say "cannot read [::rcsettings::model::path]: $err" 1
        return
    }
    ::rcsettings::ui::form::refresh_all
    ::rcsettings::ui::ssh_page::refresh
    say "reloaded: the file changed outside this window"
}

# ---------------------------------------------------------------- the exits --

proc ::rcsettings::ui::app::ok {} {
    destroy .
    exit 0
}

# Put back the bytes the window opened on. A file that did not exist is
# restored by deleting it, not by writing an empty one: the contract says an
# absent file and an empty file mean the same settings, but only the absent
# one is what the user had. The directory this window may have created is
# left standing, the terminal making it itself in any case.
proc ::rcsettings::ui::app::cancel {} {
    variable Snapshot
    variable Existed
    set path [::rcsettings::model::path]
    if {[catch {
        if {$Existed} {
            ::rcsettings::toml::atomic_write $path $Snapshot
        } elseif {[file exists $path]} {
            file delete -- $path
        }
    } err]} {
        # The window stays open on a failed restore: closing it would leave
        # the user with changes they asked to have undone and no way back.
        say "cannot restore $path: $err" 1
        return
    }
    destroy .
    exit 0
}
