# Where a failure goes when there is nowhere to print it.
#
# On Unix this window is started from a terminal or from a desktop file, and
# an uncaught error reaches a console or the session journal either way. On
# Windows the shipped image is linked /SUBSYSTEM:WINDOWS and, double-clicked
# from Explorer, has no console at all: the interpreter's own error report
# goes to a stderr that is not connected to anything, and the user sees a
# process that started and vanished. That is the bug this namespace exists
# for, and the answer is that every failure lands in a file as well as on
# stderr, so there is always something to read afterwards.
#
# Nothing here requires Tk. It is sourced with the other Tcl-only libraries,
# before Tk enters the process, because the failures worth recording include
# the ones that happen before there is a window to put a message box in.

package require Tcl 9.0

namespace eval ::rcsettings::diag {
    namespace export path record fatal

    # The log's path, decided once per process. Deciding it again on each
    # call would be free, but a mid-run change of environment would then
    # split one run's records across two files.
    variable Path ""

    # The channel a record is echoed to as well as filed. It is stderr in
    # every run but a suite's own, which points it at a file it can read
    # back: this is the harness reaching in, the way model.tcl's ForcedBinary
    # is, and there is nothing here for a user to set.
    variable Console stderr
}

# The file failures are appended to.
#
# Embedded in the terminal, the terminal chooses the path and hands it to the
# C entry point, so its own log and this window's stay together. Standalone,
# the path follows the platform's state directory: XDG_STATE_HOME where the
# XDG layout is in force, LOCALAPPDATA on Windows, and the temporary
# directory where neither is set - which is not a good home for a log, but it
# is a writable one, and a diagnostic nobody can find is the failure this
# namespace is here to prevent.
proc ::rcsettings::diag::path {} {
    variable Path
    if {$Path ne ""} { return $Path }
    if {[info exists ::rcsettings::embedded(diagfile)]
        && $::rcsettings::embedded(diagfile) ne ""} {
        set Path $::rcsettings::embedded(diagfile)
    } else {
        set base ""
        foreach v {XDG_STATE_HOME LOCALAPPDATA} {
            if {[info exists ::env($v)] && $::env($v) ne ""} {
                set base $::env($v)
                break
            }
        }
        if {$base eq ""} {
            foreach v {TMPDIR TEMP TMP} {
                if {[info exists ::env($v)] && $::env($v) ne ""} {
                    set base $::env($v)
                    break
                }
            }
        }
        if {$base eq ""} {
            # Creates the directory, and is the last resort precisely
            # because it does: everything above names a place that already
            # exists and is the user's.
            catch {file tempdir} base
        }
        set Path [file join $base robco-term settings.log]
    }
    # The directory is made here rather than at write time so that a failure
    # to make it is one catch, not one per record. A failure at all is
    # swallowed: the write below reports through stderr, which needs no
    # directory.
    catch {file mkdir [file dirname $Path]}
    return $Path
}

# One record: what kind of thing happened, what it said, and the stack it
# said it from. The version and the executable are on every line because the
# first question asked of a log a user sends back is which build wrote it.
proc ::rcsettings::diag::record {kind text {info ""}} {
    variable Console
    # ::errorInfo is where the interpreter left the stack of the last error,
    # and in an interpreter that has not had one yet it does not exist at
    # all: a record made before anything went wrong (a startup note, a
    # message from bgerror's own caller) must not fail for want of it.
    if {$info eq "" && [info exists ::errorInfo]} { set info $::errorInfo }
    set version [expr {[info exists ::ROBCO_SETTINGS_VERSION]
        ? $::ROBCO_SETTINGS_VERSION : "unknown"}]
    set stamp [clock format [clock seconds] -format "%Y-%m-%dT%H:%M:%S"]
    set entry "$stamp robco-settings $version \[$kind\]\
        [info nameofexecutable]\n$text"
    if {[string trim $info] ne "" && [string trim $info] ne [string trim $text]} {
        append entry "\n$info"
    }
    append entry "\n"
    # Both arms are attempted and neither is allowed to fail: this is the
    # code that runs when something has already gone wrong, and an error
    # raised out of it would replace a diagnosable failure with a mysterious
    # one. A read-only state directory loses the file and keeps stderr; a
    # Windows GUI launch loses stderr and keeps the file.
    catch {
        set ch [open [path] a]
        puts -nonewline $ch $entry
        close $ch
    }
    catch { puts -nonewline $Console $entry ; flush $Console }
    return
}

# A failure the window cannot continue past. It is recorded first, so the
# record exists whether or not anything can be shown, and only then shown:
# a message box is the best of the three answers when there is a Tk to draw
# it, and there is not always one - a failure before Tk is loaded, or a
# --version run, has no window to parent a dialog on.
proc ::rcsettings::diag::fatal {message detail} {
    record fatal "$message\n$detail"
    if {[llength [info commands ::tk_messageBox]]} {
        if {[winfo exists .]} {
            catch {
                tk_messageBox -icon error -title "RobCo Terminal Settings" \
                    -message $message -detail "$detail\n\nRecorded in [path]"
            }
        }
    }
    exit 1
}
