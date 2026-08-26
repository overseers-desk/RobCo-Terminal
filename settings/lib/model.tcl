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
        set_value reset switch_preset pin_overrides default_config_path \
        ssh_default ssh_hosts set_ssh_default add_ssh_host remove_ssh_host \
        set_ssh_host

    # The whole model is one document and one dump: the GUI edits a single
    # config file, so there is nothing to instantiate.
    variable Dump {}
    variable Path {}
    variable Text ""

    # Tables whose `name` key selects a preset base rather than labelling
    # the table. The axis name and the table name are the same word.
    variable Axes {screen chassis}

    # The ssh rows are an array of tables, `[[ssh.host]]`, and each row is
    # a diff against `[ssh_host_defaults]` the way a flat table is a diff
    # against its preset. These two names are written out often enough
    # below to be worth naming once.
    variable SshRows ssh.host
    variable SshFields {host user port key}

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

    # ------------------------------------------------------ the ssh rows --
    #
    # Same discipline as the flat tables: read through the file with the
    # dump behind it, write by re-reading and editing the one row's bytes.
    # What differs is that a row can be created and destroyed, and that the
    # `default` key names a row by its `host` string rather than by
    # position, so the two move together or the check detaches.

    proc default_in {text} {
        variable Dump
        set raw [raw_in $text ssh default]
        if {$raw eq ""} {
            set raw [::rcsettings::dump::default $Dump ssh default]
        }
        return [::rcsettings::toml::plain $raw]
    }

    # The `host` of the row a new session starts on, empty for localhost.
    proc ssh_default {} {
        variable Text
        return [default_in $Text]
    }

    proc ssh_field_default {key} {
        variable Dump
        return [::rcsettings::dump::default $Dump ssh_host_defaults $key]
    }

    proc hosts_in {text} {
        variable SshRows
        variable SshFields
        set arrays [dict get [::rcsettings::toml::parse $text] arrays]
        if {![dict exists $arrays $SshRows]} { return {} }
        set rows {}
        foreach entry [dict get $arrays $SshRows] {
            set row [dict create]
            foreach key $SshFields {
                # A field the row does not carry is not a hole: it is what
                # `[ssh_host_defaults]` gives it, the file being a diff.
                set raw [expr {[dict exists $entry $key]
                    ? [dict get $entry $key] : [ssh_field_default $key]}]
                dict set row $key [::rcsettings::toml::plain $raw]
            }
            lappend rows $row
        }
        return $rows
    }

    # Every row in file order, each field resolved. The list's positions
    # are the indices every ssh write below takes.
    proc ssh_hosts {} {
        variable Text
        return [hosts_in $Text]
    }

    # Empty means localhost, and the minimal edit for it is to remove the
    # key rather than to write the empty string the key already defaults to.
    proc write_ssh_default {text value} {
        variable SshRows
        if {$value eq ""} {
            return [::rcsettings::toml::unset_key $text ssh default]
        }
        set text [::rcsettings::toml::ensure_table $text ssh $SshRows]
        return [::rcsettings::toml::set_key $text ssh default \
            [::rcsettings::toml::format_value string $value]]
    }

    proc set_ssh_default {value} {
        return [commit [write_ssh_default [reload] $value]]
    }

    # A new row is written with its `host` alone. Every other field is what
    # `[ssh_host_defaults]` gives it, and a writer does not fill in keys at
    # their default.
    proc add_ssh_host {} {
        variable SshRows
        set text [::rcsettings::toml::append_array_row [reload] $SshRows \
            [list host [::rcsettings::toml::format_value string ""]]]
        return [commit $text]
    }

    # The row goes, and the check goes with it when nothing left answers to
    # its name: a `default` naming no row reads as localhost anyway, and
    # leaving it would put the check back on a row the user deleted the
    # moment another row took that name.
    proc remove_ssh_host {index} {
        variable SshRows
        set text [reload]
        set gone [lindex [hosts_in $text] $index]
        set text [::rcsettings::toml::remove_array_row $text $SshRows $index]
        set host [expr {$gone eq "" ? "" : [dict get $gone host]}]
        if {$host ne "" && $host eq [default_in $text]} {
            set survives 0
            foreach row [hosts_in $text] {
                if {[dict get $row host] eq $host} { set survives 1 }
            }
            if {!$survives} { set text [write_ssh_default $text ""] }
        }
        return [commit $text]
    }

    # One field of one row. $value is a plain Tcl value, formatted after
    # the type `[ssh_host_defaults]` gives the key.
    proc set_ssh_host {index key value} {
        variable SshRows
        set text [reload]
        set rows [hosts_in $text]
        if {![string is integer -strict $index] || $index < 0
            || $index >= [llength $rows]} {
            error "no ssh host row at index $index"
        }
        set was [dict get [lindex $rows $index] host]
        set fallback [ssh_field_default $key]
        set type [::rcsettings::toml::type_of $fallback]
        set raw [::rcsettings::toml::format_value $type $value]
        # `host` is the row's identity and the string `default` names, so
        # it is written even when it holds the default's own empty value.
        # Every other field equal to the default is removed instead, which
        # is the minimal edit for it.
        if {$key ne "host" && [same_value $type $raw $fallback]} {
            set text [::rcsettings::toml::unset_array_key $text $SshRows \
                $index $key]
        } else {
            set text [::rcsettings::toml::set_array_key $text $SshRows \
                $index $key $raw]
        }
        # Renaming the checked row moves the check in the same act. A
        # `default` left naming the old string would silently detach.
        if {$key eq "host" && $was ne "" && $was eq [default_in $text]} {
            set text [write_ssh_default $text $value]
        }
        return [commit $text]
    }

    proc commit {text} {
        variable Path
        variable Text
        ::rcsettings::toml::atomic_write $Path $text
        set Text $text
        return $text
    }
}
