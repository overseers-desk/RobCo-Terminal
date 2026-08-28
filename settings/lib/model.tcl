# The settings model: the schema this window's coder knows on one side,
# the user's config file on the other, and the resolution rule from
# docs/config.md between them. A key's value is the file's if the file
# pins it, otherwise the value the table's named preset gives it,
# otherwise the shipped default.
#
# The schema - defaults, presets, enum value lists, the bundled font
# catalogue - is literal data below, stated as raw TOML values so an
# edited value can be formatted after its default's own spelling. The
# terminal's Rust source remains the authority on what these values are:
# `robco-term --dump-settings` still prints them, and tests/schema.test
# fails the build the moment the literals and the binary disagree. The
# one answer that can never be literals is the machine's installed
# faces, which system_fonts below asks the binary for.
#
# Every write here goes through the machine-write contract
# (docs/config-format.md): re-read the file, edit the one key's bytes,
# atomic_write. The re-read is what keeps the read-modify-write window
# short; there is no lock, and last writer wins.

package require Tcl 9.0

source [file join [file dirname [info script]] tomledit-1.0.tm]

namespace eval ::rcsettings::model {
    namespace export init load path set_path text reload \
        effective effective_raw base base_raw pinned preset_name \
        set_value reset switch_preset pin_overrides default_config_path \
        ssh_default ssh_hosts set_ssh_default add_ssh_host remove_ssh_host \
        set_ssh_host \
        shipped has_default table_keys preset_names has_preset preset \
        enum enum_names fonts system_fonts system_fonts_text

    # The whole model is one document: the GUI edits a single config
    # file, so there is nothing to instantiate.
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

    # ------------------------------------------------------- the schema --
    #
    # Values are raw TOML text, quoting intact, exactly as the binary
    # prints them. `[ssh_host_defaults]` is not a table of the config
    # file: it is what one `[[ssh.host]]` row's fields fall back to.
    # Presets are diffs against their table's defaults, the shape the
    # terminal's own preset code states them in.

    variable Defaults {
        general {
            effects_frame_skip 3
            window_scaling 1.0
            show_terminal_size true
            font_scaling 1.0
            show_menubar false
            bloom_quality 0.5
            burn_in_quality 0.5
            use_custom_command false
            custom_command {""}
            led_characters 12
            chassis_shown true
            grapheme_clustering false
        }
        screen {
            name {"Default Amber"}
            background_color {"#000000"}
            font_color {"#ff8100"}
            flickering 0.1
            horizontal_sync 0.1
            static_noise 0.1
            chroma_color 0.2
            saturation_color 0.2
            screen_curvature 0.2
            glowing_line 0.2
            burn_in 0.3
            bloom 0.6
            rasterization {"no_rasterization"}
            jitter 0.2
            rgb_shift 0.0
            brightness 0.5
            contrast 0.8
            ambient_light 0.3
            window_opacity 1.0
            font_name {"TERMINESS_SCALED"}
            font_source {"bundled_fonts"}
            font_width 1.0
            line_spacing 0.1
            margin 0.3
            blinking_cursor false
            frame_size 0.1
            screen_radius 0.1
            frame_color {"#cfcfcf"}
            frame_shininess 0.3
        }
        chassis {
            name {"Annunciator"}
            shell {"annunciator"}
            channel_indicator {"glow"}
            channel_display {"led"}
            frame_size 0.45
            screen_radius 0.44
            frame_color {"#001735"}
            frame_shininess 0.3
            bank_font_name {"COZETTE_SCALED"}
        }
        ssh {
            default {""}
            host {[]}
        }
        ssh_host_defaults {
            host {""}
            user {""}
            port 22
            key {""}
        }
    }
    variable PresetOrder {
        screen {
            {Default Amber}
            {Monochrome Green}
            {Deep Blue}
            {Commodore 64}
            {Commodore PET}
            {Apple ][}
            {Atari 400}
            {IBM VGA 8x16}
            {IBM 3278 Reborn}
            {Neon Cyan}
            {Ghost Terminal}
            Plasma
            Boring
            E-Ink
        }
        chassis {
            Annunciator
            {Slide Rule}
            Switchboard
        }
    }
    variable Presets {
        screen {
            {Default Amber} {}
            {Monochrome Green} {font_color {"#0ccc68"} chroma_color 0.0 saturation_color 0.0 screen_curvature 0.3 bloom 0.5 font_name {"DEPARTURE_MONO_SCALED"} screen_radius 0.2 frame_color {"#d4d4d4"} frame_shininess 0.1}
            {Deep Blue} {font_color {"#7fb4ff"} chroma_color 1.0 screen_curvature 0.4 ambient_light 0.0 font_name {"BIGBLUE_TERMINAL_SCALED"} frame_color {"#ffffff"} frame_shininess 0.9}
            {Commodore 64} {background_color {"#3b3b8f"} font_color {"#a9a7ff"} horizontal_sync 0.0 chroma_color 0.0 saturation_color 0.0 screen_curvature 0.5 glowing_line 0.1 burn_in 0.1 bloom 0.4 rasterization {"scanline_rasterization"} jitter 0.0 brightness 0.6 contrast 0.7 ambient_light 0.4 font_name {"COMMODORE_64_SCALED"} frame_size 0.5 frame_color {"#999999"} frame_shininess 0.0}
            {Commodore PET} {font_color {"#ffffff"} flickering 0.2 horizontal_sync 0.2 static_noise 0.2 chroma_color 0.0 saturation_color 0.0 screen_curvature 0.7 glowing_line 0.3 burn_in 0.4 bloom 0.4 rasterization {"scanline_rasterization"} jitter 0.15 ambient_light 0.0 font_name {"COMMODORE_PET_SCALED"} font_width 1.25 margin 0.2 frame_size 0.5 screen_radius 0.3 frame_color {"#000000"} frame_shininess 0.6}
            {Apple ][} {background_color {"#001100"} font_color {"#4dff6b"} flickering 0.2 horizontal_sync 0.2 static_noise 0.2 chroma_color 0.0 saturation_color 0.0 screen_curvature 0.5 glowing_line 0.3 bloom 0.3 rasterization {"scanline_rasterization"} ambient_light 1.0 font_name {"APPLE_II_SCALED"} font_width 1.25 margin 0.0 frame_size 0.2 screen_radius 0.3 frame_color {"#ffffff"} frame_shininess 0.8}
            {Atari 400} {background_color {"#0f1f5a"} font_color {"#8ed6ff"} horizontal_sync 0.0 chroma_color 0.0 saturation_color 0.0 screen_curvature 0.4 glowing_line 0.1 burn_in 0.2 bloom 0.1 rasterization {"scanline_rasterization"} jitter 0.0 brightness 0.6 contrast 0.9 ambient_light 0.1 font_name {"ATARI_400_SCALED"} margin 0.2 frame_size 0.4 screen_radius 0.2 frame_color {"#cccccc"}}
            {IBM VGA 8x16} {font_color {"#c0c0c0"} horizontal_sync 0.0 static_noise 0.0 chroma_color 0.5 saturation_color 0.0 screen_curvature 0.3 glowing_line 0.1 burn_in 0.1 bloom 0.2 rasterization {"scanline_rasterization"} jitter 0.0 rgb_shift 0.1 brightness 0.6 contrast 1.0 ambient_light 0.2 font_name {"IBM_VGA_8x16"} margin 0.2 frame_color {"#ffffff"}}
            {IBM 3278 Reborn} {font_color {"#3cff7a"} flickering 0.0 horizontal_sync 0.0 static_noise 0.0 chroma_color 0.0 saturation_color 0.0 screen_curvature 0.0 glowing_line 0.0 burn_in 0.5 bloom 0.2 rasterization {"modern_rasterization"} jitter 0.0 ambient_light 0.2 font_name {"IBM_3278"} margin 0.1 frame_size 0.0 screen_radius 0.0 frame_color {"#ffffff"} frame_shininess 0.2}
            {Neon Cyan} {background_color {"#001018"} font_color {"#52f7ff"} horizontal_sync 0.0 chroma_color 1.0 saturation_color 0.6 screen_curvature 0.0 burn_in 0.1 rasterization {"modern_rasterization"} jitter 0.1 brightness 0.6 contrast 0.9 ambient_light 0.1 window_opacity 0.8 font_name {"IOSEVKA"} margin 0.1 frame_size 0.0 screen_radius 0.0 frame_color {"#c3c3c3"} frame_shininess 0.2}
            {Ghost Terminal} {background_color {"#0b1014"} font_color {"#a6b3c0"} flickering 0.0 horizontal_sync 0.0 chroma_color 0.0 saturation_color 0.0 screen_curvature 0.0 glowing_line 0.1 burn_in 0.2 bloom 0.3 rasterization {"modern_rasterization"} jitter 0.0 brightness 0.6 contrast 0.5 window_opacity 0.7 font_name {"JETBRAINS_MONO"} margin 0.1 frame_size 0.0 screen_radius 0.0 frame_color {"#a7a7a7"} frame_shininess 0.2}
            Plasma {background_color {"#070014"} font_color {"#ff9bd6"} horizontal_sync 0.0 chroma_color 1.0 saturation_color 0.8 screen_curvature 0.0 burn_in 0.1 bloom 0.7 rasterization {"modern_rasterization"} jitter 0.1 rgb_shift 0.1 brightness 0.6 ambient_light 0.1 font_name {"FIRA_CODE"} margin 0.1 frame_size 0.0 screen_radius 0.0 frame_color {"#d0d0d0"} frame_shininess 0.2}
            Boring {font_color {"#ffffff"} flickering 0.0 horizontal_sync 0.0 static_noise 0.0 chroma_color 1.0 saturation_color 0.0 screen_curvature 0.0 glowing_line 0.1 burn_in 0.05 bloom 0.5 rasterization {"modern_rasterization"} jitter 0.0 ambient_light 0.1 font_name {"JETBRAINS_MONO"} margin 0.0 frame_size 0.0 screen_radius 0.0 frame_color {"#c0c0c0"} frame_shininess 0.2}
            E-Ink {background_color {"#f2f2ec"} font_color {"#101010"} flickering 0.0 horizontal_sync 0.0 static_noise 0.0 chroma_color 0.0 saturation_color 0.0 screen_curvature 0.0 glowing_line 0.0 burn_in 0.6 bloom 0.0 rasterization {"modern_rasterization"} jitter 0.0 brightness 1.0 contrast 0.5 ambient_light 0.6 font_name {"HACK"} margin 0.1 frame_size 0.0 screen_radius 0.0 frame_color {"#cdcdcd"} frame_shininess 0.2}
        }
        chassis {
            Annunciator {}
            {Slide Rule} {shell {"slide-rule"} channel_indicator {"pointer"} frame_size 0.7 screen_radius 1.0 frame_color {"#a77d37"} frame_shininess 0.15}
            Switchboard {shell {"switchboard"} channel_indicator {"switch"} channel_display {"tape"} frame_size 0.2 screen_radius 0.7 frame_color {"#461725"} frame_shininess 0.2 bank_font_name {"DEPARTURE_MONO_SCALED"}}
        }
    }
    variable Values {
        rasterization {no_rasterization scanline_rasterization pixel_rasterization subpixel_rasterization modern_rasterization}
        shell {annunciator slide-rule switchboard}
        channel_indicator {glow pointer switch}
        channel_display {led tape}
    }
    variable Fonts {
        {TERMINESS_SCALED Terminess}
        {BIGBLUE_TERMINAL_SCALED {BigBlue Terminal}}
        {EXCELSIOR_SCALED {Fixedsys Excelsior}}
        {GREYBEARD_SCALED Greybeard}
        {COMMODORE_PET_SCALED {Commodore PET}}
        {GOHU_11_SCALED {Gohu 11}}
        {COZETTE_SCALED Cozette}
        {UNSCII_8_SCALED {Unscii 8}}
        {UNSCII_8_THIN_SCALED {Unscii 8 Thin}}
        {UNSCII_16_SCALED {Unscii 16}}
        {APPLE_II_SCALED {Apple ][}}
        {ATARI_400_SCALED {Atari 400-800}}
        {COMMODORE_64_SCALED {Commodore 64}}
        {IBM_EGA_8x8 {IBM EGA 8x8}}
        {IBM_VGA_8x16 {IBM VGA 8x16}}
        {TERMINESS Terminess}
        {HACK Hack}
        {FIRA_CODE {Fira Code}}
        {IOSEVKA Iosevka}
        {JETBRAINS_MONO {JetBrains Mono}}
        {IBM_3278 {IBM 3278}}
        {SOURCE_CODE_PRO {Source Code Pro}}
        {DEPARTURE_MONO_SCALED {Departure Mono}}
        {OPENDYSLEXIC OpenDyslexic}
    }

    # The shipped default of table.key, raw. Asking for a key the schema
    # does not carry is a caller's bug and errors by name.
    proc shipped {table key} {
        variable Defaults
        if {![dict exists $Defaults $table $key]} {
            error "no default for $table.$key in the schema"
        }
        return [dict get $Defaults $table $key]
    }

    proc has_default {table key} {
        variable Defaults
        return [dict exists $Defaults $table $key]
    }

    # Declaration order, which is the order a preset picker should offer.
    proc table_keys {table} {
        variable Defaults
        if {![dict exists $Defaults $table]} {
            error "no table \"$table\" in the schema"
        }
        return [dict keys [dict get $Defaults $table]]
    }

    proc preset_names {axis} {
        variable PresetOrder
        if {![dict exists $PresetOrder $axis]} {
            error "no presets for axis \"$axis\""
        }
        # lrange canonicalises the literal above, whose own spelling is
        # one name per line.
        return [lrange [dict get $PresetOrder $axis] 0 end]
    }

    proc has_preset {axis name} {
        variable Presets
        return [dict exists $Presets $axis $name]
    }

    # The preset's own diff against the axis table's defaults: a key it
    # does not carry falls back to `shipped`, which is what base_raw_under
    # does with it.
    proc preset {axis name} {
        variable Presets
        if {![has_preset $axis $name]} {
            error "no built-in $axis preset named \"$name\""
        }
        return [dict get $Presets $axis $name]
    }

    # List of {catalogue_key display_name} pairs. lrange canonicalises
    # the literal above, whose own spelling is one pair per line.
    proc fonts {} {
        variable Fonts
        return [lrange $Fonts 0 end]
    }

    proc enum_names {} {
        variable Values
        return [dict keys $Values]
    }

    proc enum {name} {
        variable Values
        if {![dict exists $Values $name]} {
            error "no value list named \"$name\""
        }
        return [dict get $Values $name]
    }

    # ------------------------------------------- the machine's own faces --
    #
    # The installed system faces are the one schema answer no coder can
    # know ahead: they are asked of the terminal binary, which is the
    # authority on which faces it can render. The terminal names itself in
    # ROBCO_SETTINGS_TERMINAL when it opens this window; a hand launch
    # falls back to the sibling binary and then PATH.

    # A binary the suites point this namespace at, so a test can run the
    # real --list-renderable-fonts against a build tree. Nothing user-facing
    # sets it.
    variable ForcedBinary ""

    # Windows resolves an executable by extension, so the sibling looked
    # for there is robco-term.exe; nowhere else carries a suffix.
    proc exe_suffix {} {
        return [expr {$::tcl_platform(platform) eq "windows" ? ".exe" : ""}]
    }

    proc candidates {} {
        variable ForcedBinary
        global env
        set out {}
        if {$ForcedBinary ne ""} { lappend out $ForcedBinary }
        if {[info exists ::rcsettings::embedded(terminal)]
            && $::rcsettings::embedded(terminal) ne ""} {
            lappend out $::rcsettings::embedded(terminal)
        }
        if {[info exists env(ROBCO_SETTINGS_TERMINAL)]
            && $env(ROBCO_SETTINGS_TERMINAL) ne ""} {
            lappend out $env(ROBCO_SETTINGS_TERMINAL)
        }
        set exe [info nameofexecutable]
        if {$exe ne ""} {
            lappend out [file join [file dirname $exe] robco-term[exe_suffix]]
        }
        set found [auto_execok robco-term]
        if {$found ne ""} { lappend out [lindex $found 0] }
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

    # The message locate fails with: each arm it names is an arm
    # candidates actually walks, and the tests hold the two together.
    proc describe_search {} {
        return "cannot find the robco-term binary: not named in\
            ROBCO_SETTINGS_TERMINAL, not beside this program\
            (robco-term[exe_suffix]), not on PATH"
    }

    # Run the binary under one flag and hand back its raw stdout. The
    # binary logs to stderr and a log line is not a failure, so stderr
    # goes to a file; only a non-zero exit fails, and then what the
    # binary said is the useful half of the message.
    proc run_dump {path flag} {
        set ch [file tempfile errpath]
        close $ch
        set failed [catch {exec -- $path $flag 2> $errpath} out]
        set said [string trim [::tomledit::read_file $errpath]]
        file delete -- $errpath
        if {$failed} {
            error "running \"$path $flag\" failed: $out\
                [expr {$said eq "" ? "" : "\n$said"}]"
        }
        return $out
    }

    # The installed system faces. An empty machine is a legal answer, not
    # a broken one.
    proc system_fonts {{path ""}} {
        if {$path eq ""} { set path [locate] }
        return [system_fonts_text [run_dump $path --list-renderable-fonts]]
    }

    # The parse step alone, so tests can feed a canned dump. {name text}
    # pairs, same shape as `fonts`, and an empty list when the array is
    # absent altogether.
    proc system_fonts_text {text} {
        set out {}
        set parsed [::tomledit::parse $text]
        if {[dict exists $parsed arrays fonts]} {
            foreach entry [dict get $parsed arrays fonts] {
                lappend out [list \
                    [::tomledit::plain [dict get $entry name]] \
                    [::tomledit::plain [dict get $entry text]]]
            }
        }
        return $out
    }

    # ------------------------------------------------- the config file --

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

    # An empty $path takes the platform's default location.
    proc init {{path ""}} {
        variable Path
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

    proc text {} {
        variable Text
        return $Text
    }

    # A missing file is the empty document, per the contract, so this
    # never fails on a fresh install.
    proc reload {} {
        variable Path
        variable Text
        set Text [::tomledit::read_file $Path]
        return $Text
    }

    # What every mutating operation starts from: the file as it stands,
    # refused whole when it carries TOML outside the surgeon's subset
    # (tomledit's unsafe). Reading stays open to any file; only editing
    # is gated, and the refusal names the line so the user can hand-edit.
    proc edit_base {} {
        set text [reload]
        set what [::tomledit::unsafe $text]
        if {$what ne ""} {
            error "this file uses TOML the settings window cannot edit safely ($what); edit it by hand instead"
        }
        return $text
    }

    proc parsed_tables {text} {
        return [dict get [::tomledit::parse $text] tables]
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
        variable Text
        variable Axes
        if {$table ni $Axes} { return "" }
        set tables [parsed_tables $Text]
        if {[dict exists $tables $table name]} {
            return [::tomledit::plain [dict get $tables $table name]]
        }
        return [::tomledit::plain [shipped $table name]]
    }

    # The raw value a key falls back to when the file does not pin it,
    # given a preset to measure against. A name matching no built-in
    # preset falls back to the shipped default, which is what the loader
    # does with a look the user saved under a name of their own.
    proc base_raw_under {table presetname key} {
        variable Axes
        if {$table in $Axes && $key ne "name"} {
            if {[has_preset $table $presetname]} {
                set entry [preset $table $presetname]
                if {[dict exists $entry $key]} {
                    return [dict get $entry $key]
                }
            }
        }
        return [shipped $table $key]
    }

    proc base_raw {table key} {
        return [base_raw_under $table [preset_name $table] $key]
    }

    proc base {table key} {
        return [::tomledit::plain [base_raw $table $key]]
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
        return [::tomledit::plain [effective_raw $table $key]]
    }

    # Two raw values meaning the same setting. Numbers are compared as
    # numbers because the file's spelling of one ("0.90", "1") is the
    # user's and need not match the schema's.
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
    # after the type of the shipped default, which is the only place the
    # key's type is stated.
    proc set_value {table key value} {
        set type [::tomledit::type_of [shipped $table $key]]
        set raw [::tomledit::format_value $type $value]
        set text [edit_base]
        if {[same_value $type $raw [base_raw $table $key]]} {
            set text [::tomledit::del $text $table.$key]
        } else {
            set text [::tomledit::put $text $table.$key $raw]
        }
        return [commit $text]
    }

    # Unpin a key, letting it fall back to its base. Note that its base is
    # the named preset's value, not necessarily the shipped default.
    proc reset {table key} {
        set text [::tomledit::del [edit_base] $table.$key]
        return [commit $text]
    }

    # Switching presets, not renaming a look: the new preset's values are
    # what the user asked to see, so the old table's overrides go. Only
    # keys the schema knows are dropped; a key this tool does not
    # recognise belongs to someone else and is round-tripped.
    proc switch_preset {table presetname} {
        set text [edit_base]
        foreach key [table_keys $table] {
            if {$key eq "name"} { continue }
            set text [::tomledit::del $text $table.$key]
        }
        return [commit [write_name $text $table $presetname]]
    }

    # Renaming a look: the visible values are what the user asked to keep,
    # so every key whose value would move under the new base gets pinned
    # at what it shows now. A key already pinned at what the new base
    # gives it is unpinned instead, the edit being minimal either way.
    proc pin_overrides {table presetname} {
        set text [edit_base]
        set old [preset_name $table]
        set plan {}
        foreach key [table_keys $table] {
            if {$key eq "name"} { continue }
            set was [raw_in $text $table $key]
            set shown [expr {$was eq "" ? [base_raw_under $table $old $key] : $was}]
            set type [::tomledit::type_of [shipped $table $key]]
            if {[same_value $type $shown [base_raw_under $table $presetname $key]]} {
                if {$was ne ""} { lappend plan unset $key {} }
            } else {
                lappend plan set $key $shown
            }
        }
        foreach {op key raw} $plan {
            if {$op eq "set"} {
                set text [::tomledit::put $text $table.$key $raw]
            } else {
                set text [::tomledit::del $text $table.$key]
            }
        }
        return [commit [write_name $text $table $presetname]]
    }

    # `name` is only worth writing when it moves the base: naming the
    # shipped preset is what an absent `name` already means.
    proc write_name {text table presetname} {
        set shippedname [::tomledit::plain [shipped $table name]]
        if {$presetname eq $shippedname} {
            return [::tomledit::del $text $table.name]
        }
        return [::tomledit::put $text $table.name \
            [::tomledit::format_value string $presetname]]
    }

    # ------------------------------------------------------ the ssh rows --
    #
    # Same discipline as the flat tables: read through the file with the
    # schema behind it, write by re-reading and editing the one row's
    # bytes. What differs is that a row can be created and destroyed, and
    # that the `default` key names a row by its `host` string rather than
    # by position, so the two move together or the check detaches.

    proc default_in {text} {
        set raw [raw_in $text ssh default]
        if {$raw eq ""} {
            set raw [shipped ssh default]
        }
        return [::tomledit::plain $raw]
    }

    # The `host` of the row a new session starts on, empty for localhost.
    proc ssh_default {} {
        variable Text
        return [default_in $Text]
    }

    proc ssh_field_default {key} {
        return [shipped ssh_host_defaults $key]
    }

    proc hosts_in {text} {
        variable SshRows
        variable SshFields
        set arrays [dict get [::tomledit::parse $text] arrays]
        if {![dict exists $arrays $SshRows]} { return {} }
        set rows {}
        foreach entry [dict get $arrays $SshRows] {
            set row [dict create]
            foreach key $SshFields {
                # A field the row does not carry is not a hole: it is what
                # `[ssh_host_defaults]` gives it, the file being a diff.
                set raw [expr {[dict exists $entry $key]
                    ? [dict get $entry $key] : [ssh_field_default $key]}]
                dict set row $key [::tomledit::plain $raw]
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
            return [::tomledit::del $text ssh.default]
        }
        set text [::tomledit::ensure_table $text ssh $SshRows]
        return [::tomledit::put $text ssh.default \
            [::tomledit::format_value string $value]]
    }

    proc set_ssh_default {value} {
        return [commit [write_ssh_default [edit_base] $value]]
    }

    # A new row is written with its `host` alone. Every other field is what
    # `[ssh_host_defaults]` gives it, and a writer does not fill in keys at
    # their default.
    proc add_ssh_host {} {
        variable SshRows
        set text [::tomledit::add [edit_base] $SshRows \
            [list host [::tomledit::format_value string ""]]]
        return [commit $text]
    }

    # The row goes, and the check goes with it when nothing left answers to
    # its name: a `default` naming no row reads as localhost anyway, and
    # leaving it would put the check back on a row the user deleted the
    # moment another row took that name.
    proc remove_ssh_host {index} {
        variable SshRows
        set text [edit_base]
        set gone [lindex [hosts_in $text] $index]
        set text [::tomledit::del $text "$SshRows\[$index\]"]
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
        set text [edit_base]
        set rows [hosts_in $text]
        if {![string is integer -strict $index] || $index < 0
            || $index >= [llength $rows]} {
            error "no ssh host row at index $index"
        }
        set was [dict get [lindex $rows $index] host]
        set fallback [ssh_field_default $key]
        set type [::tomledit::type_of $fallback]
        set raw [::tomledit::format_value $type $value]
        # `host` is the row's identity and the string `default` names, so
        # it is written even when it holds the default's own empty value.
        # Every other field equal to the default is removed instead, which
        # is the minimal edit for it.
        if {$key ne "host" && [same_value $type $raw $fallback]} {
            set text [::tomledit::del $text "$SshRows\[$index\].$key"]
        } else {
            set text [::tomledit::put $text "$SshRows\[$index\].$key" $raw]
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
        ::tomledit::atomic_write $Path $text
        set Text $text
        return $text
    }
}
