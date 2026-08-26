# ::rcsettings::ui::preset_dialog - the one question this window asks.
#
# Choosing a preset while the file pins values is two different edits, and
# docs/config-format.md says a tool offering a preset picker has to decide
# deliberately between them. This window does not decide: switching drops the
# user's pinned values for the preset's own, and keeping the look writes every
# visible value out under the new name. Neither is recoverable from the other
# once the file is written, so the choice is the user's.
#
# Single instance, modal, and held in namespace variables rather than an
# object: there is no state that outlives one question.
#
# The lifecycle is questlog's move dialog: transient to the parent's toplevel
# so the window manager keeps it above and iconifies it with its parent, every
# way out of the window funnelled to one of the three answers, the grab taken
# after the widgets exist and released before the destroy, and the caller's
# callback invoked after the teardown so that what it opens next is not
# fighting a grab this window still holds.

package require Tcl 9
package require Tk

namespace eval ::rcsettings::ui::preset_dialog {
    namespace export open

    variable Top ""
    variable OnDone ""
    variable Chosen ""
}

# $parent is any widget in the window asking; $table is screen or chassis;
# $chosen is the preset name the picker was moved to. $on_done is called with
# an answer of switch, keep or cancel, and the chosen name.
proc ::rcsettings::ui::preset_dialog::open {parent table chosen on_done} {
    variable Top
    variable OnDone
    variable Chosen

    set OnDone $on_done
    set Chosen $chosen

    set Top .rcs_presetdlg
    if {[winfo exists $Top]} { destroy $Top }
    toplevel $Top
    wm title $Top "Switch preset"
    wm resizable $Top 0 0
    wm transient $Top [winfo toplevel $parent]

    set old [::rcsettings::model::preset_name $table]
    ttk::frame $Top.f -padding 14
    pack $Top.f -fill both -expand 1

    ttk::label $Top.f.head \
        -text "The $table table has values of your own."
    pack $Top.f.head -side top -anchor w

    ttk::label $Top.f.body -justify left -wraplength 380 \
        -text "You are moving the $table from $old to $chosen. Switching\
            drops the values you have pinned and shows $chosen as it ships.\
            Keeping your look writes those values into the file under the\
            new name, so nothing on the glass moves."
    pack $Top.f.body -side top -anchor w -pady {8 0}

    ttk::frame $Top.f.btn
    pack $Top.f.btn -side top -fill x -pady {14 0}
    ttk::button $Top.f.btn.cancel -text "Cancel" \
        -command [list ::rcsettings::ui::preset_dialog::answer cancel]
    ttk::button $Top.f.btn.keep -text "Keep my look" \
        -command [list ::rcsettings::ui::preset_dialog::answer keep]
    ttk::button $Top.f.btn.switch -text "Switch" \
        -command [list ::rcsettings::ui::preset_dialog::answer switch]
    pack $Top.f.btn.switch -side right
    pack $Top.f.btn.keep   -side right -padx {0 6}
    pack $Top.f.btn.cancel -side right -padx {0 6}

    # Escape and the window manager's close button are the same answer: a
    # window dismissed without a button pressed has consented to nothing.
    bind $Top <Escape> [list ::rcsettings::ui::preset_dialog::answer cancel]
    wm protocol $Top WM_DELETE_WINDOW \
        [list ::rcsettings::ui::preset_dialog::answer cancel]

    grab set $Top
    focus $Top.f.btn.switch
}

proc ::rcsettings::ui::preset_dialog::answer {what} {
    variable Top
    variable OnDone
    variable Chosen
    set cb $OnDone
    set chosen $Chosen
    if {$Top ne "" && [winfo exists $Top]} {
        grab release $Top
        destroy $Top
    }
    set Top ""
    set OnDone ""
    if {$cb ne ""} { {*}$cb $what $chosen }
}
