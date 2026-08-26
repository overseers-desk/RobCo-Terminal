# ::rcsettings::ui::theme - every colour this window draws with, named once.
#
# This is the first file to require Tk: the entry script sources the lib
# namespaces under a bare tclsh and only the ui/ files pull the toolkit in,
# so `robco-settings --version` answers on a machine with no display.
#
# The ttk base is clam rather than the platform theme. clam draws every widget
# in pure Tk and honours `ttk::style configure` the same way everywhere; the
# X11 default theme and aqua each ignore a different subset of the options
# below, so a dark window tuned under one would come out half-recoloured under
# the other. A settings window for a CRT terminal that arrived in system grey
# with amber patches would look like a bug rather than a choice.
#
# The palette is the terminal's own first-launch look: near-black glass, the
# #ff8100 phosphor of the Default Amber preset, and warm greys for the type
# that has to stay readable at length.

package require Tcl 9
package require Tk

namespace eval ::rcsettings::ui::theme {
    namespace export c init

    variable Palette {
        bg          #1a1208
        panel       #211809
        field       #120c05
        trough      #2b1f0c
        ink         #e8dcc4
        muted       #a3906d
        faint       #6f5f43
        inert       #4a3f2b
        border      #4a3a20
        border_hi   #7a5f2e
        accent      #ff8100
        accent_dim  #b35a00
        accent_ink  #1a1208
        sel         #3d2a0c
        error       #ff6a4a
        swatch_edge #7a5f2e
    }
}

# Colour for a role. An unknown role errors rather than returning a default,
# so a typo surfaces when the window is built and not as an invisible
# miscolour somewhere down the form.
proc ::rcsettings::ui::theme::c {role} {
    variable Palette
    return [dict get $Palette $role]
}

# Switch ttk to clam and paint every widget class this window uses. Call once,
# after Tk is up and before the first widget.
proc ::rcsettings::ui::theme::init {} {
    ttk::style theme use clam

    set bg [c bg]
    set ink [c ink]
    set field [c field]
    set border [c border]
    set accent [c accent]

    # The root window is a plain Tk widget, so it takes its colour directly
    # rather than through a style.
    . configure -background $bg

    ttk::style configure . \
        -background $bg -foreground $ink \
        -fieldbackground $field -troughcolor [c trough] \
        -bordercolor $border -darkcolor $bg -lightcolor $bg \
        -focuscolor [c accent_dim] -selectbackground [c sel] \
        -selectforeground $ink -insertcolor $ink
    ttk::style map . -foreground [list disabled [c faint]]

    ttk::style configure TFrame -background $bg
    ttk::style configure TLabel -background $bg -foreground $ink

    # Group headings inside a tab. clam draws a labelframe's border from
    # -bordercolor and its title from the style with the .Label suffix.
    ttk::style configure TLabelframe -background $bg -bordercolor $border \
        -relief solid -borderwidth 1
    ttk::style configure TLabelframe.Label -background $bg -foreground $accent

    ttk::style configure TNotebook -background $bg -bordercolor $border \
        -tabmargins {2 4 2 0}
    ttk::style configure TNotebook.Tab -background [c panel] -foreground [c muted] \
        -bordercolor $border -padding {14 5}
    ttk::style map TNotebook.Tab \
        -background [list selected $bg active [c sel]] \
        -foreground [list selected $accent active $ink] \
        -expand [list selected {1 1 1 0}]

    ttk::style configure TButton -background [c panel] -foreground $ink \
        -bordercolor $border -relief raised -padding {12 4}
    ttk::style map TButton \
        -background [list active [c sel] pressed [c sel] disabled $bg] \
        -bordercolor [list active [c border_hi]]

    # The per-row reset control: a glyph on the tab surface with no plate of
    # its own, so a form of twenty-five rows does not read as a wall of
    # buttons. It is disabled, not hidden, on a row the file does not pin:
    # a control that vanishes moves every column beside it.
    ttk::style configure Reset.TButton -background $bg -foreground [c faint] \
        -bordercolor $bg -relief flat -borderwidth 0 -padding {4 0} \
        -focuscolor $bg
    ttk::style map Reset.TButton \
        -background [list active $bg pressed $bg disabled $bg] \
        -foreground [list active $accent pressed $accent disabled [c inert]]

    ttk::style configure TCheckbutton -background $bg -foreground $ink \
        -indicatorbackground $field -indicatorforeground $accent \
        -bordercolor $border -focuscolor $bg -padding {0 1}
    ttk::style map TCheckbutton \
        -indicatorbackground [list pressed [c trough] active [c trough]] \
        -foreground [list active $ink]

    ttk::style configure TEntry -fieldbackground $field -foreground $ink \
        -bordercolor $border -insertcolor $accent -padding {4 2}

    ttk::style configure TSpinbox -fieldbackground $field -foreground $ink \
        -background [c panel] -bordercolor $border -arrowcolor $accent \
        -insertcolor $accent -padding {4 2}
    ttk::style map TSpinbox -bordercolor [list focus [c border_hi]]

    ttk::style configure TCombobox -fieldbackground $field -foreground $ink \
        -background [c panel] -bordercolor $border -arrowcolor $accent \
        -padding {4 2}
    ttk::style map TCombobox \
        -fieldbackground [list readonly $field] \
        -foreground [list readonly $ink] \
        -bordercolor [list focus [c border_hi]]
    # The combobox drop-down is a Tk listbox inside a toplevel of ttk's own
    # making, out of reach of ttk::style. The option database is the only
    # handle on it, and without these three it opens white.
    option add *TCombobox*Listbox.background $field
    option add *TCombobox*Listbox.foreground $ink
    option add *TCombobox*Listbox.selectBackground [c sel]
    option add *TCombobox*Listbox.selectForeground $accent

    ttk::style configure Horizontal.TScale -background $bg \
        -troughcolor [c trough] -bordercolor $border \
        -darkcolor [c accent_dim] -lightcolor [c accent] \
        -sliderthickness 16
    ttk::style map Horizontal.TScale \
        -lightcolor [list active [c accent]] \
        -darkcolor [list active [c accent]]

    ttk::style configure TScrollbar -background [c panel] -troughcolor [c trough] \
        -bordercolor $border -arrowcolor [c muted] \
        -darkcolor [c panel] -lightcolor [c panel]
    ttk::style map TScrollbar \
        -background [list active [c sel]] -arrowcolor [list active $accent]

    # ---- roles this form invents ------------------------------------------
    # The pinned dot: one glyph in a fixed-width column at the head of every
    # row, amber when the file pins that key and the tab's own background
    # when it does not, so the column holds its width either way.
    ttk::style configure Pin.TLabel -background $bg -foreground $accent \
        -anchor center
    ttk::style configure Unpinned.TLabel -background $bg -foreground $bg \
        -anchor center
    # The numeric readout beside a slider, and the hex beside a swatch.
    ttk::style configure Value.TLabel -background $bg -foreground [c muted] \
        -anchor e
    ttk::style configure Field.TLabel -background $bg -foreground $ink -anchor w
    # The status line along the foot: the config path in the resting state,
    # a transient message over it, and an error in the one colour that is
    # not amber.
    ttk::style configure Status.TLabel -background [c panel] -foreground [c muted] \
        -padding {8 3}
    ttk::style configure StatusError.TLabel -background [c panel] \
        -foreground [c error] -padding {8 3}
    ttk::style configure Sep.TFrame -background $border
}
