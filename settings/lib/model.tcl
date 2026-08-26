# The settings model: the binary's dump on one side, the user's config
# file on the other, and the resolution rule from docs/config.md between
# them. A key's value is the file's if the file pins it, otherwise the
# value the table's named preset gives it, otherwise the shipped default.
#
# Every write here goes through the machine-write contract
# (docs/config-format.md): re-read the file, edit the one key's bytes,
# atomic_write. The re-read is what keeps the read-modify-write window
# short; there is no lock, and last writer wins.

package require Tcl 9.0

source [file join [file dirname [info script]] toml.tcl]
source [file join [file dirname [info script]] dump.tcl]

namespace eval ::rcsettings::model {
    namespace export init load path set_path text dump reload \
        effective effective_raw base base_raw pinned preset_name \
        set_value reset switch_preset pin_overrides default_config_path

    # The whole model is one document and one dump: the GUI edits a single
    # config file, so there is nothing to instantiate.
    variable Dump {}
    variable Path {}
    variable Text ""

    # Tables whose `name` key selects a preset base rather than labelling
    # the table. The axis name and the table name are the same word.
    variable Axes {screen chassis}

    proc default_config_path {} {
        global env tcl_platform
        if {$tcl_platform(platform) eq "windows"} {
            if {![info exists env(APPDATA)]} {
                error "APPDATA is not set; cannot locate the config file"
            }
            return [file join $env(APPDATA) robco-term config.toml]
        }
        if {$tcl_platform(os) eq "Darwin"} {
            return [file join $env(HOME) Library "Application Support" \
                robco-term config.toml]
        }
        if {[info exists env(XDG_CONFIG_HOME)] && $env(XDG_CONFIG_HOME) ne ""} {
            return [file join $env(XDG_CONFIG_HOME) robco-term config.toml]
        }
        return [file join $env(HOME) .config robco-term config.toml]
    }

    # $dumpdata is what ::rcsettings::dump::load returned. An empty $path
    # takes the platform's default location.
    proc init {dumpdata {path ""}} {
        variable Dump
        variable Path
        set Dump $dumpdata
        if {$path eq ""} { set path [default_config_path] }
        set Path $path
        reload
        return
    }

    proc load {path} {
        variable Path
        set Path $path
        reload
    }

    proc path {} {
        variable Path
        return $Path
    }

    proc set_path {path} {
        variable Path
        set Path $path
        reload
    }

    proc dump {} {
        variable Dump
        return $Dump
    }

    proc text {} {
        variable Text
        return $Text
    }

    # A missing file is the empty document, per the contract, so this
    # never fails on a fresh install.
    proc reload {} {
        variable Path
        variable Text
        set Text [::rcsettings::toml::read_file $Path]
        return $Text
    }

    proc parsed_tables {text} {
        return [dict get [::rcsettings::toml::parse $text] tables]
    }

    proc raw_in {text table key} {
        set tables [parsed_tables $text]
        if {[dict exists $tables $table $key]} {
            return [dict get $tables $table $key]
        }
        return {}
    }

    proc pinned {table key} {
        variable Text
        set tables [parsed_tables $Text]
        return [dict exists $tables $table $key]
    }

    # Which preset the table currently resolves against: the file's `name`
    # if it pins one, else the shipped table's own.
    proc preset_name {table} {
        variable Dump
        variable Text
        variable Axes
        if {$table ni $Axes} { return "" }
        set tables [parsed_tables $Text]
        if {[dict exists $tables $table name]} {
            return [::rcsettings::toml::plain [dict get $tables $table name]]
        }
        return [::rcsettings::toml::plain [::rcsettings::dump::default $Dump $table name]]
    }

    # The raw value a key falls back to when the file does not pin it,
    # given a preset to measure against. A name matching no built-in
    # preset falls back to the shipped default, which is what the loader
    # does with a look the user saved under a name of their own.
    proc base_raw_under {table presetname key} {
        variable Dump
        variable Axes
        if {$table in $Axes && $key ne "name"} {
            if {[::rcsettings::dump::has_preset $Dump $table $presetname]} {
                set entry [::rcsettings::dump::preset $Dump $table $presetname]
                if {[dict exists $entry $key]} {
                    return [dict get $entry $key]
                }
            }
        }
        return [::rcsettings::dump::default $Dump $table $key]
    }

    proc base_raw {table key} {
        return [base_raw_under $table [preset_name $table] $key]
    }

    proc base {table key} {
        return [::rcsettings::toml::plain [base_raw $table $key]]
    }

    proc effective_raw {table key} {
        variable Text
        set tables [parsed_tables $Text]
        if {[dict exists $tables $table $key]} {
            return [dict get $tables $table $key]
        }
        return [base_raw $table $key]
    }

    proc effective {table key} {
        return [::rcsettings::toml::plain [effective_raw $table $key]]
    }

    # Two raw values meaning the same setting. Numbers are compared as
    # numbers because the file's spelling of one ("0.90", "1") is the
    # user's and need not match the dump's.
    proc same_value {type a b} {
        if {$type in {int float}} {
            if {[string is double -strict $a] && [string is double -strict $b]} {
                return [expr {double($a) == double($b)}]
            }
        }
        return [string equal $a $b]
    }

    # The minimal edit for setting a key: when the new value is what the
    # key would fall back to anyway, the edit is to remove the key, not to
    # write the value out. $value is a plain Tcl value; it is formatted
    # after the type of the dump's default, which is the only place the
    # key's type is stated.
    proc set_value {table key value} {
        variable Dump
        set type [::rcsettings::toml::type_of \
            [::rcsettings::dump::default $Dump $table $key]]
        set raw [::rcsettings::toml::format_value $type $value]
        set text [reload]
        if {[same_value $type $raw [base_raw $table $key]]} {
            set text [::rcsettings::toml::unset_key $text $table $key]
        } else {
            set text [::rcsettings::toml::set_key $text $table $key $raw]
        }
        return [commit $text]
    }

    # Unpin a key, letting it fall back to its base. Note that its base is
    # the named preset's value, not necessarily the shipped default.
    proc reset {table key} {
        set text [::rcsettings::toml::unset_key [reload] $table $key]
        return [commit $text]
    }

    # Switching presets, not renaming a look: the new preset's values are
    # what the user asked to see, so the old table's overrides go. Only
    # keys the dump knows are dropped; a key this tool does not recognise
    # belongs to someone else and is round-tripped.
    proc switch_preset {table presetname} {
        variable Dump
        set text [reload]
        foreach key [::rcsettings::dump::table_keys $Dump $table] {
            if {$key eq "name"} { continue }
            set text [::rcsettings::toml::unset_key $text $table $key]
        }
        return [commit [write_name $text $table $presetname]]
    }

    # Renaming a look: the visible values are what the user asked to keep,
    # so every key whose value would move under the new base gets pinned
    # at what it shows now. A key already pinned at what the new base
    # gives it is unpinned instead, the edit being minimal either way.
    proc pin_overrides {table presetname} {
        variable Dump
        set text [reload]
        set old [preset_name $table]
        set plan {}
        foreach key [::rcsettings::dump::table_keys $Dump $table] {
            if {$key eq "name"} { continue }
            set was [raw_in $text $table $key]
            set shown [expr {$was eq "" ? [base_raw_under $table $old $key] : $was}]
            set type [::rcsettings::toml::type_of \
                [::rcsettings::dump::default $Dump $table $key]]
            if {[same_value $type $shown [base_raw_under $table $presetname $key]]} {
                if {$was ne ""} { lappend plan unset $key {} }
            } else {
                lappend plan set $key $shown
            }
        }
        foreach {op key raw} $plan {
            if {$op eq "set"} {
                set text [::rcsettings::toml::set_key $text $table $key $raw]
            } else {
                set text [::rcsettings::toml::unset_key $text $table $key]
            }
        }
        return [commit [write_name $text $table $presetname]]
    }

    # `name` is only worth writing when it moves the base: naming the
    # shipped preset is what an absent `name` already means.
    proc write_name {text table presetname} {
        variable Dump
        set shipped [::rcsettings::toml::plain \
            [::rcsettings::dump::default $Dump $table name]]
        if {$presetname eq $shipped} {
            return [::rcsettings::toml::unset_key $text $table name]
        }
        return [::rcsettings::toml::set_key $text $table name \
            [::rcsettings::toml::format_value string $presetname]]
    }

    proc commit {text} {
        variable Path
        variable Text
        ::rcsettings::toml::atomic_write $Path $text
        set Text $text
        return $text
    }
}
