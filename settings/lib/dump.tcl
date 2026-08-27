# The terminal's own answer to "what are the settings, and what are they
# by default": `robco-term --dump-settings` prints a TOML document of
# fully-resolved defaults, the built-in presets for both axes, the font
# catalogue and the enumerated value lists. This namespace finds the
# binary, runs it, and hands the parse out through accessors.
#
# There are no fallback tables here on purpose. A settings GUI that cannot
# reach the binary does not know what the binary's defaults are, and a
# stale copy compiled into this file would write a user's config against
# the wrong base. Not finding the binary is an error, not a degraded mode.

package require Tcl 9.0

source [file join [file dirname [info script]] toml.tcl]

namespace eval ::rcsettings::dump {
    namespace export locate load load_text defaults default has_default \
        table_keys preset_names preset has_preset fonts enum enum_names

    # A binary the suites point this namespace at, so a test can run the
    # real --dump-settings against a build tree. It is empty in every run
    # but a test's own: nothing user-facing sets it, and the app finds its
    # terminal by looking beside itself and then on PATH, which is the whole
    # of what a user has to know.
    variable ForcedBinary ""

    # What a terminal is called on this platform. Windows resolves an
    # executable by extension, so the sibling looked for there is
    # robco-term.exe; nowhere else carries a suffix. Looking for the bare
    # name on Windows is how the sibling arm silently never matched, which
    # left every launch to PATH.
    proc exe_suffix {} {
        return [expr {$::tcl_platform(platform) eq "windows" ? ".exe" : ""}]
    }

    # Where the binary is looked for, in order. The result is a full path
    # (or a name auto_execok resolved), never a bare guess: the error
    # message below repeats this list, so keep the two in step.
    #
    # Embedded in the terminal's own executable there is no sibling to find:
    # the terminal is this process, and the C entry point says so by setting
    # ::rcsettings::embedded(terminal) before any script runs. That answer
    # comes first because it is the only one that cannot be wrong.
    proc candidates {} {
        variable ForcedBinary
        set out {}
        if {$ForcedBinary ne ""} {
            lappend out $ForcedBinary
        }
        if {[info exists ::rcsettings::embedded(terminal)]
            && $::rcsettings::embedded(terminal) ne ""} {
            lappend out $::rcsettings::embedded(terminal)
        }
        set exe [info nameofexecutable]
        if {$exe ne ""} {
            lappend out [file join [file dirname $exe] robco-term[exe_suffix]]
        }
        set found [auto_execok robco-term]
        if {$found ne ""} {
            lappend out [lindex $found 0]
        }
        return $out
    }

    proc locate {} {
        foreach path [candidates] {
            if {[file executable $path] && ![file isdirectory $path]} {
                return $path
            }
        }
        error [describe_search]
    }

    proc describe_search {} {
        set exe [info nameofexecutable]
        set where {}
        if {[info exists ::rcsettings::embedded(terminal)]
            && $::rcsettings::embedded(terminal) ne ""} {
            lappend where "the terminal this window is embedded in\
                ($::rcsettings::embedded(terminal))"
        }
        lappend where "robco-term[exe_suffix] beside this interpreter\
            ([file join [file dirname $exe] robco-term[exe_suffix]])"
        lappend where "robco-term on PATH\
            ([expr {[info exists ::env(PATH)] ? $::env(PATH) : "PATH not set"}])"
        return "cannot find the robco-term binary. Looked for:\
            [join $where {; }]. Put robco-term beside this program or on PATH."
    }

    # Run the binary and parse what it prints. A non-zero exit or unusable
    # output is an error carrying the binary's own stderr: the caller has
    # no way to guess defaults without it.
    proc load {{path ""}} {
        if {$path eq ""} { set path [locate] }
        # The binary logs to stderr, and a log line is not a failure, so
        # stderr goes to a file rather than through exec's error path or
        # out of this process. Only a non-zero exit is a failure, and then
        # what the binary said is the useful half of the message.
        set ch [file tempfile errpath]
        close $ch
        set failed [catch {exec -- $path --dump-settings 2> $errpath} out]
        set said [string trim [::rcsettings::toml::read_file $errpath]]
        file delete -- $errpath
        if {$failed} {
            error "running \"$path --dump-settings\" failed: $out\
                [expr {$said eq "" ? "" : "\n$said"}]"
        }
        return [load_text $out $path]
    }

    # The parse step alone, so tests can feed a canned dump.
    proc load_text {text {origin "--dump-settings output"}} {
        set parsed [::rcsettings::toml::parse $text]
        set tables [dict get $parsed tables]
        set arrays [dict get $parsed arrays]
        set data [dict create]
        # `[ssh_host_defaults]` is not a table of the config file: it is
        # what one `[[ssh.host]]` row's fields fall back to, which the dump
        # states here because a row has no preset axis to resolve against.
        foreach table {general screen chassis ssh ssh_host_defaults} {
            if {![dict exists $tables $table] || [dict size [dict get $tables $table]] == 0} {
                error "$origin has no \[$table\] table"
            }
            dict set data defaults $table [dict get $tables $table]
        }
        foreach {axis key} {screen screen_presets chassis chassis_presets} {
            set order {}
            set byname [dict create]
            if {[dict exists $arrays $key]} {
                foreach entry [dict get $arrays $key] {
                    if {![dict exists $entry name]} {
                        error "$origin has a \[\[$key\]\] entry with no name key"
                    }
                    set name [::rcsettings::toml::plain [dict get $entry name]]
                    lappend order $name
                    dict set byname $name $entry
                }
            }
            if {[llength $order] == 0} {
                error "$origin has no \[\[$key\]\] entries"
            }
            dict set data preset_order $axis $order
            dict set data presets $axis $byname
        }
        set fonts {}
        if {[dict exists $arrays fonts]} {
            foreach entry [dict get $arrays fonts] {
                lappend fonts [list \
                    [::rcsettings::toml::plain [dict get $entry name]] \
                    [::rcsettings::toml::plain [dict get $entry text]]]
            }
        }
        if {[llength $fonts] == 0} {
            error "$origin has no \[\[fonts\]\] entries"
        }
        dict set data fonts $fonts
        set values [dict create]
        if {[dict exists $tables values]} {
            dict for {name raw} [dict get $tables values] {
                dict set values $name [string_array $raw]
            }
        }
        if {[dict size $values] == 0} {
            error "$origin has no \[values\] table"
        }
        dict set data values $values
        return $data
    }

    # The elements of a TOML array of strings, whose raw text the parser
    # has already joined onto one line. Only string arrays appear in
    # [values]; nothing else in the dump is an array-valued key.
    proc string_array {raw} {
        set out {}
        foreach {- item} [regexp -all -inline {"((?:[^"\\]|\\.)*)"} $raw] {
            lappend out [::rcsettings::toml::plain "\"$item\""]
        }
        return $out
    }

    # Raw TOML values, not plain ones: the model formats an edited value
    # after the type of the raw default, so the quoting has to survive.
    proc defaults {data table} {
        if {![dict exists $data defaults $table]} {
            error "no defaults for table \"$table\" in the dump"
        }
        return [dict get $data defaults $table]
    }

    proc default {data table key} {
        if {![has_default $data $table $key]} {
            error "no default for $table.$key in the dump"
        }
        return [dict get $data defaults $table $key]
    }

    proc has_default {data table key} {
        return [dict exists $data defaults $table $key]
    }

    # Declaration order, which is the order the dump printed them in and
    # so the order a preset picker should offer.
    proc table_keys {data table} {
        return [dict keys [defaults $data $table]]
    }

    proc preset_names {data axis} {
        if {![dict exists $data preset_order $axis]} {
            error "no presets for axis \"$axis\" in the dump"
        }
        return [dict get $data preset_order $axis]
    }

    proc has_preset {data axis name} {
        return [dict exists $data presets $axis $name]
    }

    proc preset {data axis name} {
        if {![has_preset $data $axis $name]} {
            error "no built-in $axis preset named \"$name\""
        }
        return [dict get $data presets $axis $name]
    }

    # List of {catalogue_key display_name} pairs.
    proc fonts {data} {
        return [dict get $data fonts]
    }

    proc enum_names {data} {
        return [dict keys [dict get $data values]]
    }

    proc enum {data name} {
        if {![dict exists $data values $name]} {
            error "no value list named \"$name\" in the dump"
        }
        return [dict get $data values $name]
    }
}
