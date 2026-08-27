# ::rcsettings::ui::ssh_page - the SSH tab: where a new session starts, and
# the rows it chooses between.
#
# A list editor rather than the flat per-key form of the other three tabs,
# because `[[ssh.host]]` is an array of tables: rows arrive and leave, and a
# row's four fields mean nothing away from the radio that selects them. The
# form's two disciplines still hold. Every field writes the moment it changes,
# debounced so that typing a hostname is one write and not one per keystroke;
# and nothing drawn here is a widget's memory of what was typed, every repaint
# coming from the model, which is the only way the port box can be right about
# a 22 the minimal edit has just removed from the file.
#
# The radio is on `ssh.default`, which names a row by its `host` string rather
# than by position. So the check is worked out on every repaint by looking for
# that string among the rows, and a `default` naming no row draws the check on
# localhost, which is what the terminal makes of it too.

package require Tcl 9
package require Tk

namespace eval ::rcsettings::ui::ssh_page {
    namespace export init page refresh

    # A row's editable fields, in the order they sit across the page, with
    # the heading over each and the width of the box. The keys are
    # `[ssh_host_defaults]`'s own, and ui.test holds this list to the schema's.
    variable Fields {
        host "Host" 18
        user "User" 12
        port "Port"  6
        key  "Key"  18
    }

    # A port is the one field with a range, and it is the protocol's, not
    # this window's: a number outside it is not a port the terminal could
    # dial, so it is not written.
    variable PortFrom 1
    variable PortTo 65535

    # The frame the radio list is built into, and how many rows are built
    # there now. A repaint rebuilds the list only when the count moved, so a
    # write landing under the cursor does not destroy the box being typed in.
    variable List ""
    variable Built -1
    # The widget variables, one per field, named "index.key".
    variable Value
    array set Value {}
    # The checked radio: a row's index, or -1 for localhost.
    variable Selected -1
    # Pending debounced writes, "index.key" -> after token.
    variable Pending
    array set Pending {}
    # Raised while a repaint is writing widget variables, for the reason
    # form.tcl raises its own: a programmatic write fires the same -command
    # and -textvariable traces a user's does.
    variable Repainting 0

    variable StatusCmd ""
    variable WroteCmd ""
}

# The two callbacks form.tcl takes, and for the same reasons: a message
# reaches the status line, and the window re-notes the file's mtime so this
# page's own write is not read back as a hand edit.
proc ::rcsettings::ui::ssh_page::init {statuscmd wrotecmd} {
    variable StatusCmd
    variable WroteCmd
    set StatusCmd $statuscmd
    set WroteCmd $wrotecmd
}

proc ::rcsettings::ui::ssh_page::say {msg {isError 0}} {
    variable StatusCmd
    if {$StatusCmd ne ""} { {*}$StatusCmd $msg $isError }
}

# ---------------------------------------------------------------- the page --

# Build the tab under $parent and return the widget to pack. The scrolling
# canvas is form.tcl's, a list of servers being as able to outgrow the window
# as a page of sliders is.
proc ::rcsettings::ui::ssh_page::page {parent} {
    variable List

    ttk::frame $parent.page_ssh
    set outer $parent.page_ssh

    set line [font metrics TkDefaultFont -linespace]
    set canvas $outer.canvas
    canvas $canvas -highlightthickness 0 -borderwidth 0 \
        -width [expr {34 * $line}] -height [expr {26 * $line}] \
        -background [ttk::style lookup TFrame -background] \
        -yscrollcommand [list $outer.sb set]
    ttk::scrollbar $outer.sb -orient vertical -command [list $canvas yview]
    grid $canvas   -row 0 -column 0 -sticky nsew
    grid $outer.sb -row 0 -column 1 -sticky ns
    grid columnconfigure $outer 0 -weight 1
    grid rowconfigure    $outer 0 -weight 1

    set body $canvas.body
    ttk::frame $body -padding {12 10 12 12}
    set item [$canvas create window 0 0 -anchor nw -window $body]
    bind $body <Configure> [list ::rcsettings::ui::form::fit_body $canvas]
    bind $canvas <Configure> \
        [list ::rcsettings::ui::form::fit_window $canvas $item %w]
    ::rcsettings::ui::form::wheel_scrolls $canvas $body

    ttk::label $body.caption -anchor w -justify left \
        -wraplength [expr {30 * $line}] \
        -text "The row checked here is where a new session starts, unless the\
            channel is under tmux -CC control: those windows come from tmux\
            rather than from a shell this terminal spawns. The terminal reads\
            the table when it launches, so a change here reaches the next\
            session started and not the ones already running."
    pack $body.caption -side top -anchor w -pady {0 10}

    ttk::labelframe $body.list -text "Servers" -padding {10 6 10 8}
    pack $body.list -side top -fill x
    set List $body.list

    ttk::frame $body.actions
    pack $body.actions -side top -fill x -pady {8 0}
    ttk::button $body.actions.add -text "+" -width 3 \
        -command ::rcsettings::ui::ssh_page::on_add
    ttk::label $body.actions.hint -anchor w \
        -text "Add a server"
    pack $body.actions.add -side left
    pack $body.actions.hint -side left -padx {8 0}

    rebuild
    return $outer
}

# The whole list from scratch: the headings, the localhost radio, one radio
# and four boxes per row. Called when the number of rows moves, which is the
# only time the widgets themselves are wrong.
proc ::rcsettings::ui::ssh_page::rebuild {} {
    variable List
    variable Fields
    variable Built
    variable Value
    if {$List eq "" || ![winfo exists $List]} { return }
    foreach w [winfo children $List] { destroy $w }
    array unset Value
    set hosts [::rcsettings::model::ssh_hosts]

    set c 1
    foreach {key label width} $Fields {
        ttk::label $List.h_$key -text $label -anchor w
        grid $List.h_$key -row 0 -column $c -sticky w -padx {6 0}
        # The three text fields take the width the page has; a port is four
        # digits at its longest and gains nothing from stretching.
        grid columnconfigure $List $c -weight [expr {$key eq "port" ? 0 : 1}]
        incr c
    }
    # Localhost is not a row of the file: it is what an absent or empty
    # `default` means, so it carries no fields to edit.
    ttk::radiobutton $List.local -text "Local shell" \
        -variable ::rcsettings::ui::ssh_page::Selected -value -1 \
        -command ::rcsettings::ui::ssh_page::on_select
    grid $List.local -row 1 -column 0 -columnspan [expr {$c + 2}] -sticky w -pady 2

    for {set i 0} {$i < [llength $hosts]} {incr i} {
        build_row $i [expr {$i + 2}]
    }
    set Built [llength $hosts]
    refresh
}

proc ::rcsettings::ui::ssh_page::build_row {i r} {
    variable List
    variable Fields
    ttk::radiobutton $List.rb$i -text "" \
        -variable ::rcsettings::ui::ssh_page::Selected -value $i \
        -command ::rcsettings::ui::ssh_page::on_select
    grid $List.rb$i -row $r -column 0 -sticky w
    set c 1
    foreach {key label width} $Fields {
        set w $List.e${i}_$key
        ttk::entry $w -width $width -exportselection 0 \
            -textvariable ::rcsettings::ui::ssh_page::Value($i.$key)
        bind $w <KeyRelease> [list ::rcsettings::ui::ssh_page::on_edit $i $key]
        bind $w <Return>     [list ::rcsettings::ui::ssh_page::flush $i $key]
        bind $w <FocusOut>   [list ::rcsettings::ui::ssh_page::flush $i $key]
        grid $w -row $r -column $c -sticky ew -padx {6 0} -pady 1
        incr c
    }
    # The key is a path, so it can be picked as well as typed. A file
    # dialog is configuration surface, the one place a native widget is
    # allowed (INVARIANTS.md), and it opens where the keys live.
    ttk::button $List.br$i -text "…" -width 2 -takefocus 0 \
        -command [list ::rcsettings::ui::ssh_page::on_browse $i]
    grid $List.br$i -row $r -column $c -sticky w -padx {2 0}
    incr c
    # Takes no focus, so pressing it does not first move focus out of a box
    # and flush a write against the row that is about to go.
    ttk::button $List.rm$i -text "✕" -width 2 -style Reset.TButton \
        -takefocus 0 -command [list ::rcsettings::ui::ssh_page::on_remove $i]
    grid $List.rm$i -row $r -column $c -sticky e -padx {8 0}
}

# ------------------------------------------------------------ what it does --

# Writing through the model, with the one failure this window can meet: the
# config directory is not writable, or the rename lost a race. The page is
# repainted either way, so what stands after a failed write is what the file
# holds and not what was aimed at.
proc ::rcsettings::ui::ssh_page::write {script} {
    variable WroteCmd
    if {[catch {uplevel #0 $script} err]} {
        say "cannot write [::rcsettings::model::path]: $err" 1
        refresh
        return 0
    }
    if {$WroteCmd ne ""} { {*}$WroteCmd }
    refresh
    return 1
}

# A key in a box: the write is put on a timer, so a hostname typed at speed
# costs one write rather than one per letter.
proc ::rcsettings::ui::ssh_page::on_edit {i key} {
    variable Repainting
    variable Pending
    if {$Repainting} { return }
    set id $i.$key
    if {[info exists Pending($id)]} { after cancel $Pending($id) }
    set Pending($id) [after 150 [list ::rcsettings::ui::ssh_page::flush $i $key]]
}

proc ::rcsettings::ui::ssh_page::flush {i key} {
    variable Repainting
    variable Pending
    variable Value
    variable PortFrom
    variable PortTo
    set id $i.$key
    if {[info exists Pending($id)]} {
        after cancel $Pending($id)
        unset Pending($id)
    }
    if {$Repainting || ![info exists Value($id)]} { return }
    set hosts [::rcsettings::model::ssh_hosts]
    if {$i >= [llength $hosts]} { refresh; return }
    set v [string trim $Value($id)]
    if {$key eq "port"} {
        # A box is typable, so it can hold anything. Nonsense is not written
        # and not corrected under the cursor either: the box keeps what was
        # typed until a repaint puts the file's own value back.
        if {![string is integer -strict $v] || $v < $PortFrom || $v > $PortTo} {
            return
        }
    }
    if {$v eq [dict get [lindex $hosts $i] $key]} { return }
    write [list ::rcsettings::model::set_ssh_host $i $key $v]
}

# Moving the check. A row whose host is still empty cannot be named, so the
# file has no way to say it is checked and the repaint returns the check to
# localhost, which is what an empty `default` means.
proc ::rcsettings::ui::ssh_page::on_select {} {
    variable Repainting
    variable Selected
    if {$Repainting} { return }
    set host ""
    if {$Selected >= 0} {
        set hosts [::rcsettings::model::ssh_hosts]
        if {$Selected >= [llength $hosts]} { refresh; return }
        set host [dict get [lindex $hosts $Selected] host]
    }
    if {[write [list ::rcsettings::model::set_ssh_default $host]]} {
        say [expr {$host eq "" ? "new sessions start on a local shell"
            : "new sessions start on $host"}]
    }
}

proc ::rcsettings::ui::ssh_page::on_add {} {
    variable List
    if {![write [list ::rcsettings::model::add_ssh_host]]} { return }
    set i [expr {[llength [::rcsettings::model::ssh_hosts]] - 1}]
    if {$i >= 0 && [winfo exists $List.e${i}_host]} { focus $List.e${i}_host }
    say "a server row added; give it a host"
}

# Pick a private key file for row $i. The dialog starts in ~/.ssh when it
# exists, and a path under home is written with the prefix spelled `~/`:
# the spelling the terminal expands, and the one a config copied to
# another machine survives.
proc ::rcsettings::ui::ssh_page::on_browse {i} {
    variable Value
    set home [file home]
    set dir [file join $home .ssh]
    if {![file isdirectory $dir]} { set dir $home }
    set path [tk_getOpenFile -title "Private key" -initialdir $dir]
    if {$path eq ""} { return }
    if {[string first $home/ $path] == 0} {
        set path ~/[string range $path [string length $home/] end]
    }
    set Value($i.key) $path
    flush $i key
}

proc ::rcsettings::ui::ssh_page::on_remove {i} {
    variable Pending
    # Every index below the removed row shifts up, so a write still on the
    # timer would land on the wrong row. The gesture is the answer to what
    # was being typed in a row that is going.
    foreach id [array names Pending] { after cancel $Pending($id) }
    array unset Pending
    if {[write [list ::rcsettings::model::remove_ssh_host $i]]} {
        say "server row removed"
    }
}

# ------------------------------------------------------------------ repaint --

proc ::rcsettings::ui::ssh_page::refresh {} {
    variable List
    variable Fields
    variable Built
    variable Value
    variable Selected
    variable Repainting
    if {$List eq "" || ![winfo exists $List]} { return }
    if {[catch {::rcsettings::model::ssh_hosts} hosts]} {
        say "cannot read the ssh rows: $hosts" 1
        return
    }
    if {[llength $hosts] != $Built} { rebuild; return }
    set default [::rcsettings::model::ssh_default]
    incr Repainting
    try {
        set i 0
        foreach row $hosts {
            foreach {key label width} $Fields {
                set v [dict get $row $key]
                # Only when it moved. Writing a textvariable redisplays the
                # box, and a box redisplayed under the cursor takes the
                # user's place in the word they are still typing.
                if {![info exists Value($i.$key)] || $Value($i.$key) ne $v} {
                    set Value($i.$key) $v
                }
            }
            incr i
        }
        set Selected [checked $hosts $default]
    } finally {
        incr Repainting -1
    }
}

# Which radio `default` checks: the first row carrying that host string, or
# localhost. A name no row answers to is read as localhost by the terminal
# and logged there, so it is drawn that way rather than leaving the list with
# no check at all.
proc ::rcsettings::ui::ssh_page::checked {hosts default} {
    if {$default eq ""} { return -1 }
    set i 0
    foreach row $hosts {
        if {[dict get $row host] eq $default} { return $i }
        incr i
    }
    return -1
}
