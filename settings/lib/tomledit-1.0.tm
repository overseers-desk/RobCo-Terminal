# tomledit is published on the teatotal module shelf,
# <https://github.com/teatotal/teatotal>, where its test suite, man page and
# updates live. A copy carried inside another project is vendored from there.
#
# Copyright (c) 2025 Weiwu Zhang. SPDX-License-Identifier: MIT

package require Tcl 9
package provide tomledit 1.0

# tomledit - TOML edits that change one key's bytes and no others.
#
# A settings window, a wizard, or a tool writing into a config file the user
# also edits by hand faces a file that is not its own: hand-written comments,
# blank-line rhythm, odd spacing, keys some other tool owns. A
# parse-and-reserialize round trip rewrites all of it to say one new value,
# and the user's file stops being theirs. tomledit works on the file's raw
# lines instead: a writer that changes one key changes only that key's
# bytes, and every edit is provably that narrow - diff the before and after
# and one line moves. The addressing is DOM-like, the materialisation
# deliberately is not: a path resolves to a span of lines, never to a tree
# that would be re-serialised.
#
# Text in and text out: every edit takes the whole document as a string and
# returns the edited whole. Nothing here touches the filesystem except
# read_file/atomic_write.
#
# A path names what an edit touches. `screen.bloom` is key bloom of table
# [screen]; `ssh.host[1].port` is key port of the second [[ssh.host]] row;
# `ssh.host[1]` is that row itself. The last dot splits table from key,
# which is unambiguous because the subset refuses dotted keys.
#
#   tomledit::get text path ?fallback? -> raw value
#   tomledit::put text path value -> text   (value pre-formatted, see format_value)
#   tomledit::del text path -> text         (a key's line, or a whole row)
#   tomledit::add text name kvlist -> text  (a new [[name]] row)
#   tomledit::count text name -> how many [[name]] rows
#   tomledit::ensure_table text table ?above? -> text
#   tomledit::parse text -> dict {tables {t {k rawvalue ...}} arrays {n {row ...}}}
#   tomledit::type_of raw -> string|bool|int|float
#   tomledit::format_value type value -> raw TOML scalar
#   tomledit::plain raw -> the unquoted Tcl value
#   tomledit::unsafe text -> "" or one line naming what falls outside the subset
#   tomledit::read_file path -> text ("" when absent)
#   tomledit::atomic_write path text
#
# The subset: flat scalar values in named tables, plus arrays of tables
# (`[[name]]`) whose rows are themselves flat, so a row is a span of lines
# and a row's key is a scalar inside that span. Values are raw TOML text on
# the way in and out of parse/get - quoting intact - so an edited value
# can be formatted after the type of the value it replaces (type_of,
# format_value) and unquoted only where displayed (plain). A document using
# TOML beyond the subset (multiline strings, dotted keys, inline tables)
# can fool a line-oriented span finder - a multiline string may hold a line
# shaped like a header, and an edit would land inside the string - so a
# writer asks `unsafe` first and refuses rather than guessing: a refused
# edit costs the caller's user a hand edit, a misplaced one costs them
# their file.

namespace eval ::tomledit {
    namespace export parse get put del add count type_of format_value \
        plain unsafe read_file atomic_write ensure_table

    # ------------------------------------------------------------ paths --

    # What a path names: {table t key} for `t.key`, {row name index key}
    # for `name[i].key`, {rowonly name index} for `name[i]`. A path that
    # fits none of these shapes is an error naming the path, because a
    # writer fed a malformed address must not guess where it lands.
    proc resolve {path} {
        if {[regexp {^([A-Za-z0-9_.-]+)\[(\d+)\]\.([A-Za-z0-9_-]+)$} \
                $path -> name index key]} {
            return [list row $name $index $key]
        }
        if {[regexp {^([A-Za-z0-9_.-]+)\[(\d+)\]$} $path -> name index]} {
            return [list rowonly $name $index]
        }
        set dot [string last "." $path]
        if {$dot > 0 && $dot < [string length $path] - 1
            && [string first "\[" $path] < 0} {
            set table [string range $path 0 [expr {$dot - 1}]]
            set key [string range $path [expr {$dot + 1}] end]
            return [list table $table $key]
        }
        error "not a path: \"$path\" (expected table.key, name\[i\].key or name\[i\])"
    }

    # The raw value the path names, or $fallback when absent. A row index
    # out of range is an error, as it is for every row edit.
    proc get {text path {fallback {}}} {
        lassign [resolve $path] kind a b c
        switch -exact -- $kind {
            table { return [get_key $text $a $b $fallback] }
            row {
                set rows [dict get [parse $text] arrays]
                if {![dict exists $rows $a]} { return $fallback }
                set items [dict get $rows $a]
                if {$b >= [llength $items]} { return $fallback }
                set row [lindex $items $b]
                if {![dict exists $row $c]} { return $fallback }
                return [dict get $row $c]
            }
        }
        error "cannot get \"$path\": a row is not a value"
    }

    # Set what the path names to the pre-formatted TOML scalar $value,
    # touching only that key's bytes.
    proc put {text path value} {
        lassign [resolve $path] kind a b c
        switch -exact -- $kind {
            table { return [set_key $text $a $b $value] }
            row   { return [set_array_key $text $a $b $c $value] }
        }
        error "cannot put \"$path\": a row takes add, not put"
    }

    # Remove what the path names: a key's line, or a whole row. An absent
    # key is a byte-for-byte no-op; an absent row is an error.
    proc del {text path} {
        lassign [resolve $path] kind a b c
        switch -exact -- $kind {
            table   { return [unset_key $text $a $b] }
            row     { return [unset_array_key $text $a $b $c] }
            rowonly { return [remove_array_row $text $a $b] }
        }
    }

    # A new [[name]] row carrying $kvlist, a flat list of keys and
    # pre-formatted values; rows are addressed, never pathed into being.
    proc add {text name kvlist} {
        return [append_array_row $text $name $kvlist]
    }

    # How many [[name]] rows the document holds.
    proc count {text name} {
        return [llength [array_spans $text $name]]
    }

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

    # Which table a header line opens, or {}. `[[name]]` counts as a header
    # too: even in a document holding none, an unknown header of either
    # shape must end the span of the table before it.
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
            # form a writer reaches for when the value itself carries
            # double quotes.
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
    # Multi-line arrays are joined before parsing.
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

    # Whether the document sits inside the subset the line surgery
    # understands: the answer is empty, or one line naming what falls
    # outside. A construct beyond the subset can fool the span finder (a
    # multiline string may hold a line shaped like a header, and an edit
    # would then land inside the string), so a writer asks this first and
    # refuses rather than guessing. Valid TOML a reader elsewhere accepts
    # is deliberately refusable here: a refused edit costs the user a hand
    # edit, a misplaced one costs them their file.
    proc unsafe {text} {
        lassign [lines $text] all trailing
        set n 0
        foreach line $all {
            incr n
            if {[string first {"""} $line] >= 0 \
                    || [string first {'''} $line] >= 0} {
                return "line $n holds a multiline string delimiter"
            }
            set key [key_of $line]
            if {$key eq ""} { continue }
            if {[string first . $key] >= 0} {
                return "line $n holds the dotted key $key"
            }
            regexp {^\s*[A-Za-z0-9_.-]+\s*=\s*(.*)$} $line -> rest
            if {[string index [string trim $rest] 0] eq "\{"} {
                return "line $n holds an inline table"
            }
            if {![balanced $line]} {
                return "line $n opens a multiline array"
            }
        }
        return {}
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
            set all [lreplace $all $i $i [revalue $line $value]]
            return [join_lines $all $trailing]
        }
        set all [linsert $all [append_at $all $start $end] "$key = $value"]
        return [join_lines $all $trailing]
    }

    # The same key line carrying a new value: its leading whitespace, its
    # `key = ` spelling and everything trailing the old value, a same-line
    # comment included, are the user's and are kept.
    proc revalue {line value} {
        regexp {^(\s*[A-Za-z0-9_.-]+\s*=\s*)} $line -> prefix
        set rest [string range $line [string length $prefix] end]
        set suffix [string range $rest [string length [value_of $line]] end]
        return "$prefix$value$suffix"
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

    # ------------------------------------------------- arrays of tables --
    #
    # `[[name]]` is the one repeating shape, and a tool that edits it adds
    # and removes rows rather than only moving values. A row is one span of
    # lines: its own header through to the next header of either shape, or
    # EOF. The span carries the header because a row is removed as a block,
    # and a key edit works inside it.

    # The [start, end) spans of $name's rows, in declaration order.
    proc array_spans {text name} {
        lassign [lines $text] all trailing
        return [spans_in $all $name]
    }

    proc spans_in {all name} {
        set spans {}
        set start -1
        for {set i 0} {$i < [llength $all]} {incr i} {
            set h [header_of [lindex $all $i]]
            if {$h eq {}} { continue }
            if {$start >= 0} {
                lappend spans [list $start $i]
                set start -1
            }
            if {[lindex $h 0] eq "array" && [lindex $h 1] eq $name} {
                set start $i
            }
        }
        if {$start >= 0} { lappend spans [list $start [llength $all]] }
        return $spans
    }

    # A row index that names no row is a caller's bug, not a document to
    # round-trip, so it is an error rather than a silent no-op.
    proc row_span {all name index} {
        set spans [spans_in $all $name]
        if {![string is integer -strict $index] || $index < 0
            || $index >= [llength $spans]} {
            error "no \[\[$name\]\] row at index $index"
        }
        return [lindex $spans $index]
    }

    # Set the pre-formatted value of $key inside row $index of $name. Every
    # byte outside that row is identical, and inside it only the key's own
    # line moves. An absent key is added at the end of the row's body.
    proc set_array_key {text name index key value} {
        lassign [lines $text] all trailing
        lassign [row_span $all $name $index] start end
        set body [expr {$start + 1}]
        for {set i $body} {$i < $end} {incr i} {
            set line [lindex $all $i]
            if {[key_of $line] ne $key} { continue }
            set all [lreplace $all $i $i [revalue $line $value]]
            return [join_lines $all $trailing]
        }
        set all [linsert $all [append_at $all $body $end] "$key = $value"]
        return [join_lines $all $trailing]
    }

    # Remove $key's line from row $index. An absent key is a no-op, byte
    # for byte, as it is for a flat table.
    proc unset_array_key {text name index key} {
        lassign [lines $text] all trailing
        lassign [row_span $all $name $index] start end
        for {set i [expr {$start + 1}]} {$i < $end} {incr i} {
            if {[key_of [lindex $all $i]] ne $key} { continue }
            set all [lreplace $all $i $i]
            return [join_lines $all $trailing]
        }
        return $text
    }

    # A new `[[$name]]` block carrying $kvlist, a flat list of keys and
    # pre-formatted values. It goes after the last row there is, or after
    # the parent table's span when there are no rows yet, or at the end of
    # the document when there is neither. One blank line separates it from
    # whatever it follows, so a file of rows reads as a list.
    proc append_array_row {text name kvlist} {
        lassign [lines $text] all trailing
        if {[llength $all] == 1 && [lindex $all 0] eq ""} { set all {} }
        set spans [spans_in $all $name]
        if {[llength $spans] > 0} {
            lassign [lindex $spans end] start end
            set at [append_at $all [expr {$start + 1}] $end]
        } else {
            set parent [parent_of $name]
            set span [expr {$parent eq "" ? {} : [table_span $all $parent]}]
            if {$span ne {}} {
                lassign $span start end
                set at [append_at $all $start $end]
            } else {
                set at [llength $all]
            }
        }
        # A block landing at the end of the document ends it with a
        # newline, the shape every other writer here leaves behind.
        if {$at >= [llength $all]} { set trailing 1 }
        set block {}
        if {$at > 0 && [string trim [lindex $all [expr {$at - 1}]]] ne ""} {
            lappend block ""
        }
        lappend block "\[\[$name\]\]"
        foreach {key value} $kvlist { lappend block "$key = $value" }
        set all [linsert $all $at {*}$block]
        return [join_lines $all $trailing]
    }

    # The table a dotted array name hangs under: `ssh.host` is a row of the
    # `[ssh]` table. A name with no dot has no parent.
    proc parent_of {name} {
        set dot [string last "." $name]
        if {$dot < 0} { return "" }
        return [string range $name 0 [expr {$dot - 1}]]
    }

    # Row $index's header and body go; the bytes around them stay. Two
    # things are not this row's to take with it: a comment block sitting
    # directly on the next header, which introduces that header, and the
    # blank line above the row, which is only removed when the row was the
    # last thing in the document and the blank would be left dangling.
    proc remove_array_row {text name index} {
        lassign [lines $text] all trailing
        lassign [row_span $all $name $index] start end
        while {$end > $start + 1 && [string match "#*" \
                [string trim [lindex $all [expr {$end - 1}]]]]} {
            incr end -1
        }
        set all [lreplace $all $start [expr {$end - 1}]]
        if {$start >= [llength $all] && $start > 0
            && [string trim [lindex $all [expr {$start - 1}]]] eq ""} {
            set all [lreplace $all [expr {$start - 1}] [expr {$start - 1}]]
        }
        return [join_lines $all $trailing]
    }

    # Give $table a header if it has none, and answer with the document
    # either way. $above names an array of tables whose rows $table heads:
    # `[ssh]` written after the `[[ssh.host]]` rows below it is not a
    # document this parser pair reads back, so when rows exist the header
    # goes above the first of them, taking the comment block that
    # introduces that row with it.
    proc ensure_table {text table {above ""}} {
        lassign [lines $text] all trailing
        if {[table_span $all $table] ne {}} { return $text }
        if {[llength $all] == 1 && [lindex $all 0] eq ""} { set all {} }
        set spans [expr {$above eq "" ? {} : [spans_in $all $above]}]
        if {[llength $spans] == 0} {
            set at [llength $all]
        } else {
            set at [lindex [lindex $spans 0] 0]
            while {$at > 0 && [string match "#*" \
                    [string trim [lindex $all [expr {$at - 1}]]]]} {
                incr at -1
            }
        }
        if {$at >= [llength $all]} { set trailing 1 }
        set block {}
        if {$at > 0 && [string trim [lindex $all [expr {$at - 1}]]] ne ""} {
            lappend block ""
        }
        lappend block "\[$table\]"
        if {$at < [llength $all]} { lappend block "" }
        set all [linsert $all $at {*}$block]
        return [join_lines $all $trailing]
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
    # always carry a decimal point, so a written value reads back as the
    # type it was written as.
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

    # Whole file as bytes; a missing file is the empty document, so a tool
    # whose contract says "absent file, default settings" reads it without
    # a branch.
    proc read_file {path} {
        if {![file exists $path]} { return "" }
        set ch [open $path rb]
        set text [encoding convertfrom utf-8 [read $ch]]
        close $ch
        return $text
    }

    # Write-temp-then-rename in the file's own directory, the write pattern
    # a live watcher of the file can rely on: the file is never seen half
    # written, and the rename fires exactly one change.
    proc atomic_write {path text} {
        set dir [file dirname $path]
        file mkdir $dir
        set ch [file tempfile tmp [file join $dir .[file tail $path]]]
        fconfigure $ch -translation binary
        puts -nonewline $ch [encoding convertto utf-8 $text]
        close $ch
        file rename -force $tmp $path
    }
}
