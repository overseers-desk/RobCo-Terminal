# Shared by the suites. Not a *.test file, so runAllTests does not run it.

set ::libdir [file normalize [file join [file dirname [info script]] .. lib]]

# The one thing every writer test asserts: which lines an edit replaced.
# Common prefix and common suffix are peeled off first, so a result of
# {{old} {new}} is proof that every other byte of the document is
# identical. {} on either side is a pure deletion or insertion.
proc surgery {before after} {
    set a [split $before "\n"]
    set b [split $after "\n"]
    set p 0
    while {$p < [llength $a] && $p < [llength $b]
           && [lindex $a $p] eq [lindex $b $p]} {
        incr p
    }
    set i [expr {[llength $a] - 1}]
    set j [expr {[llength $b] - 1}]
    while {$i >= $p && $j >= $p && [lindex $a $i] eq [lindex $b $j]} {
        incr i -1
        incr j -1
    }
    return [list [lrange $a $p $i] [lrange $b $p $j]]
}

# A config file with everything a writer must not disturb: a header
# comment block, blank lines, odd spacing, a trailing same-line comment,
# a key and a table this tool knows nothing about.
set ::fixture {# The workshop terminal.
# Do not let the bloom get away from you again.

[general]
font_scaling   =   1.2	# bigger type, fewer rows
effects_frame_skip = 2


# the look
[screen]
name = "Deep Blue"
bloom = 0.9
mystery_key = "kept"

[dotfiles_tool]
generated_at = "2026-08-01"
}

# A config file whose `[[ssh.host]]` rows carry everything a row writer
# must not disturb: a comment block introducing a row, odd spacing, a
# trailing same-line comment, an unknown key inside a row, an unknown key
# in the `[ssh]` table itself, and a table after the last row.
set ::sshfixture {# Servers.

[ssh]
default = "vault"
odd_key = "kept"

# the vault, behind the door
[[ssh.host]]
host = "vault"
user   =   "overseer"	# the account, not the person
note = "unknown to this tool"

[[ssh.host]]
host = "relay"
port = 2222

[[ssh.host]]
host = "spare"

[dotfiles_tool]
generated_at = "2026-08-01"
}

# A canned --dump-settings document: the shape of the real one (the
# fully-resolved default tables, the ssh row defaults, both preset axes,
# the font catalogue, the enumerated value lists) cut down to what the
# model tests exercise.
set ::dumptext {[general]
effects_frame_skip = 3
window_scaling = 1.0
font_scaling = 1.0
show_terminal_size = true
custom_command = ""
led_characters = 12
chassis_shown = true

[screen]
name = "Default Amber"
background_color = "#000000"
font_color = "#ff8100"
bloom = 0.6
burn_in = 0.3
jitter = 0.2
rasterization = "no_rasterization"
font_name = "TERMINESS_SCALED"
blinking_cursor = false

[chassis]
name = "Annunciator"
shell = "annunciator"
channel_indicator = "glow"
channel_display = "led"
frame_size = 0.45
bank_font_name = "COZETTE_SCALED"

[ssh]
default = ""
host = []

[ssh_host_defaults]
host = ""
user = ""
port = 22
key = ""

[[screen_presets]]
name = "Default Amber"
background_color = "#000000"
font_color = "#ff8100"
bloom = 0.6
burn_in = 0.3
jitter = 0.2
rasterization = "no_rasterization"
font_name = "TERMINESS_SCALED"
blinking_cursor = false

[[screen_presets]]
name = "Deep Blue"
background_color = "#000000"
font_color = "#5c9dff"
bloom = 0.5
burn_in = 0.3
jitter = 0.2
rasterization = "scanline_rasterization"
font_name = "COZETTE_SCALED"
blinking_cursor = false

[[screen_presets]]
name = "E-Ink"
background_color = "#e8e8e8"
font_color = "#101010"
bloom = 0.0
burn_in = 0.0
jitter = 0.0
rasterization = "no_rasterization"
font_name = "IOSEVKA"
blinking_cursor = true

[[chassis_presets]]
name = "Annunciator"
shell = "annunciator"
channel_indicator = "glow"
channel_display = "led"
frame_size = 0.45
bank_font_name = "COZETTE_SCALED"

[[chassis_presets]]
name = "Switchboard"
shell = "switchboard"
channel_indicator = "switch"
channel_display = "tape"
frame_size = 0.5
bank_font_name = "DEPARTURE_MONO_SCALED"

[[fonts]]
name = "TERMINESS_SCALED"
text = "Terminess"

[[fonts]]
name = "COZETTE_SCALED"
text = "Cozette"

[[fonts]]
name = "IOSEVKA"
text = "Iosevka"

[values]
rasterization = [
    "no_rasterization",
    "scanline_rasterization",
    "pixel_rasterization",
]
shell = [
    "annunciator",
    "slide-rule",
    "switchboard",
]
channel_indicator = [
    "glow",
    "pointer",
    "switch",
]
channel_display = [
    "led",
    "tape",
]
}
