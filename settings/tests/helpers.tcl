# Shared by the suites. Not a *.test file, so runAllTests does not run it.

set ::libdir [file normalize [file join [file dirname [info script]] .. lib]]

# The tests that run the real binary need a robco-term. One on PATH is
# found the way the app finds it; a build tree's is named by the suite's
# own ROBCO_SETTINGS_TEST_BINARY and handed straight to the model's
# internal hook. This is the test harness reaching in, not configuration.
# Call it after model.tcl is sourced.
proc force_test_binary {} {
    if {[info exists ::env(ROBCO_SETTINGS_TEST_BINARY)]
        && $::env(ROBCO_SETTINGS_TEST_BINARY) ne ""} {
        set ::rcsettings::model::ForcedBinary $::env(ROBCO_SETTINGS_TEST_BINARY)
    }
}

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
