# TOML surgery for the terminal's config file, under the machine-write
# contract (docs/config-format.md): a writer that changes one key changes
# only that key's bytes. Everything here therefore works on the file's raw
# lines, never through a parse-and-reserialize round trip. The file's
# values are all flat scalars, which is what makes line surgery adequate.
#
# Text in and text out: every edit takes the whole document as a string and
# returns the edited whole. Nothing here touches the filesystem except
# read_file/atomic_write.

namespace eval ::rcsettings::toml {
    namespace export parse get_key set_key unset_key type_of format_value \
        read_file atomic_write

    # Split into lines, remembering whether the text ended with a newline so
    # joining reproduces the original bytes.
    proc lines {text} {
        set trailing [string equal [string index $text end] "\n"]
        set lines [split $text "\n"]
        if {$trailing} {
            # split leaves one empty element after a trailing newline
            set lines [lrange $lines 0 end-1]
        }
        return [list $lines $trailing]
    }

    proc join_lines {lines trailing} {
        set text [join $lines "\n"]
        if {$trailing} { append text "\n" }
        return $text
    }

    # Which table a header line opens, or -1. `[[name]]` counts as a header
    # too: the config file has none, but the dump does, and an unknown
    # table of either shape must end the span of the table before it.
    proc header_of {line} {
        if {[regexp {^\s*\[\[([^\]]+)\]\]\s*(?:#.*)?$} $line -> name]} {
            return [list array $name]
        }
        if {[regexp {^\s*\[([^\]]+)\]\s*(?:#.*)?$} $line -> name]} {
            return [list table $name]
        }
        return {}
    }

    proc key_of {line} {
        if {[regexp {^\s*([A-Za-z0-9_.-]+)\s*=} $line -> key]} {
            return $key
        }
        return {}
    }

    # The raw value text after `=`, trailing same-line comment stripped.
    proc value_of {line} {
        if {![regexp {^\s*[A-Za-z0-9_.-]+\s*=\s*(.*)$} $line -> rest]} {
            return {}
        }
        set rest [string trim $rest]
        if {[string index $rest 0] eq "\""} {
            # Basic string: take through the closing quote, honouring \".
            if {[regexp {^"(?:[^"\\]|\\.)*"} $rest match]} {
                return $match
            }
            return $rest
        }
        if {[string index $rest 0] eq "'"} {
            # Literal string: no escapes, so through the next quote. The
            # dump uses this form for a system font family whose name
            # itself carries double quotes.
            if {[regexp {^'[^']*'} $rest match]} {
                return $match
            }
            return $rest
        }
        # Unquoted scalar: a # begins a comment.
        set hash [string first "#" $rest]
        if {$hash >= 0} {
            set rest [string trim [string range $rest 0 [expr {$hash - 1}]]]
        }
        return $rest
    }

    # Parse into a dict: tables -> dict key -> raw value. A `[[name]]`
    # header appends a fresh dict to the list under arrays -> name.
    # Multi-line arrays (the dump's value lists) are joined before parsing.
    # Returns dict with keys: tables, arrays.
    proc parse {text} {
        lassign [lines $text] all trailing
        set tables [dict create]
        set arrays [dict create]
        set current ""
        set mode table
        set pending ""
        set joined {}
        # Join multi-line array values first: a value opening more brackets
        # than it closes absorbs following lines until balanced.
        foreach line $all {
            if {$pending ne ""} {
                append pending " " [string trim $line]
                if {[balanced $pending]} {
                    lappend joined $pending
                    set pending ""
                }
                continue
            }
            set key [key_of $line]
            if {$key ne "" && ![balanced $line]} {
                set pending $line
                continue
            }
            lappend joined $line
        }
        if {$pending ne ""} { lappend joined $pending }
        foreach line $joined {
            set h [header_of $line]
            if {$h ne {}} {
                lassign $h kind name
                set current $name
                set mode $kind
                if {$kind eq "array"} {
                    dict lappend arrays $name [dict create]
                } elseif {![dict exists $tables $name]} {
                    dict set tables $name [dict create]
                }
                continue
            }
            set key [key_of $line]
            if {$key eq ""} { continue }
            set value [value_of $line]
            if {$mode eq "array"} {
                set items [dict get $arrays $current]
                set last [lindex $items end]
                dict set last $key $value
                dict set arrays $current [lreplace $items end end $last]
            } else {
                dict set tables $current $key $value
            }
        }
        return [dict create tables $tables arrays $arrays]
    }

    # Are brackets outside quoted strings balanced on this line?
    proc balanced {text} {
        set depth 0
        set inq 0
        set n [string length $text]
        for {set i 0} {$i < $n} {incr i} {
            set c [string index $text $i]
            if {$inq} {
                if {$c eq "\\"} { incr i; continue }
                if {$c eq "\""} { set inq 0 }
                continue
            }
            switch -exact -- $c {
                "\"" { set inq 1 }
                "\[" { incr depth }
                "\]" { incr depth -1 }
                "#"  { break }
            }
        }
        return [expr {$depth <= 0}]
    }

    # The raw value of table.key, or $fallback when absent.
    proc get_key {text table key {fallback {}}} {
        set parsed [parse $text]
        if {[dict exists $parsed tables $table $key]} {
            return [dict get $parsed tables $table $key]
        }
        return $fallback
    }

    # The [start, end) line span of a table's body: from the line after its
    # header to the next header or EOF. Returns {} when the header is absent.
    proc table_span {all table} {
        set start -1
        set end [llength $all]
        for {set i 0} {$i < [llength $all]} {incr i} {
            set h [header_of [lindex $all $i]]
            if {$h eq {}} { continue }
            if {$start >= 0} { set end $i; break }
            if {[lindex $h 1] eq $table && [lindex $h 0] eq "table"} {
                set start [expr {$i + 1}]
            }
        }
        if {$start < 0} { return {} }
        return [list $start $end]
    }

    # Set table.key to the pre-formatted TOML value $value, touching only
    # that key's bytes. A present key keeps its line's leading whitespace,
    # its `key = ` spelling and anything trailing the old value, a
    # same-line comment included; an absent key is appended at the end of
    # the table's span, above the blank lines and comment block that
    # introduce the next table; an absent table is appended at EOF.
    proc set_key {text table key value} {
        lassign [lines $text] all trailing
        set span [table_span $all $table]
        if {$span eq {}} {
            # An empty document splits to one empty line, which is not a
            # blank line the user wrote.
            if {[llength $all] == 1 && [lindex $all 0] eq ""} { set all {} }
            if {[llength $all] > 0 && [string trim [lindex $all end]] ne ""} {
                lappend all ""
            }
            lappend all "\[$table\]" "$key = $value"
            return [join_lines $all 1]
        }
        lassign $span start end
        for {set i $start} {$i < $end} {incr i} {
            set line [lindex $all $i]
            if {[key_of $line] ne $key} { continue }
            regexp {^(\s*[A-Za-z0-9_.-]+\s*=\s*)} $line -> prefix
            set rest [string range $line [string length $prefix] end]
            set suffix [string range $rest [string length [value_of $line]] end]
            set all [lreplace $all $i $i "$prefix$value$suffix"]
            return [join_lines $all $trailing]
        }
        set all [linsert $all [append_at $all $start $end] "$key = $value"]
        return [join_lines $all $trailing]
    }

    # Where a new key goes at the end of a table's span: above the blank
    # padding, and above a comment block sitting directly on the next
    # table's header, which is that table's comment and not this one's.
    proc append_at {all start end} {
        set at $end
        if {$at < [llength $all]} {
            while {$at > $start && [string match "#*" \
                    [string trim [lindex $all [expr {$at - 1}]]]]} {
                incr at -1
            }
        }
        while {$at > $start && [string trim [lindex $all [expr {$at - 1}]]] eq ""} {
            incr at -1
        }
        return $at
    }

    # Remove table.key's line; everything else keeps its bytes. Removing
    # the last key of a table does not remove the header: an empty table
    # means the same as an absent one, and the header may carry a comment.
    proc unset_key {text table key} {
        lassign [lines $text] all trailing
        set span [table_span $all $table]
        if {$span eq {}} { return $text }
        lassign $span start end
        for {set i $start} {$i < $end} {incr i} {
            if {[key_of [lindex $all $i]] eq $key} {
                set all [lreplace $all $i $i]
                return [join_lines $all $trailing]
            }
        }
        return $text
    }

    # What kind of scalar a raw TOML value is: string, bool, int or float.
    proc type_of {raw} {
        if {[string index $raw 0] in {\" '}} { return string }
        if {$raw in {true false}} { return bool }
        if {[regexp {^[+-]?\d+$} $raw]} { return int }
        if {[regexp {^[+-]?\d+\.\d+(?:[eE][+-]?\d+)?$} $raw]} { return float }
        return string
    }

    # Format a plain Tcl value as a TOML scalar of the given type. Floats
    # always carry a decimal point, matching how the file's values read.
    proc format_value {type value} {
        switch -exact -- $type {
            bool { return [expr {$value ? "true" : "false"}] }
            int { return [expr {int($value)}] }
            float {
                set out [format %g $value]
                if {![string match *.* $out] && ![string match *e* $out]} {
                    append out .0
                }
                return $out
            }
            default {
                set escaped [string map {\\ \\\\ \" \\\"} $value]
                return "\"$escaped\""
            }
        }
    }

    # The unquoted Tcl value of a raw TOML scalar. A literal string keeps
    # its bytes; only a basic string carries escapes to undo.
    proc plain {raw} {
        if {[string index $raw 0] eq "'"} {
            return [string range $raw 1 end-1]
        }
        if {[string index $raw 0] ne "\""} { return $raw }
        set body [string range $raw 1 end-1]
        return [string map {\\\" \" \\\\ \\ \\n \n \\t \t} $body]
    }

    # Whole file as bytes; a missing file is the empty document, per the
    # contract.
    proc read_file {path} {
        if {![file exists $path]} { return "" }
        set ch [open $path rb]
        set text [encoding convertfrom utf-8 [read $ch]]
        close $ch
        return $text
    }

    # Write-temp-then-rename in the file's own directory, the one write
    # pattern the terminal's live watch is designed around.
    proc atomic_write {path text} {
        set dir [file dirname $path]
        file mkdir $dir
        set ch [file tempfile tmp [file join $dir .config.toml]]
        fconfigure $ch -translation binary
        puts -nonewline $ch [encoding convertto utf-8 $text]
        close $ch
        file rename -force $tmp $path
    }
}
