#!/usr/bin/env tclsh9.0
# Stage robco-settings's payload and fold it into one executable with `zipfs
# mkimg`.
#
#   tclsh9.0 zipfs/build.tcl
#
# Stages the launcher, lib/, ui/ and zipfs/main.tcl into a temporary
# directory, then stubs them onto a wish to make a single file. Two env vars
# select what kind of image:
#
#   ROBCO_SETTINGS_WISH      Path to the wish used as the stub. Default:
#                            wish9.0 from PATH, which is dynamically linked,
#                            so the image still needs the Tcl 9 runtime on the
#                            host (tcl9.0, tk9.0) and carries robco-settings's
#                            code only.
#   ROBCO_SETTINGS_RUNTIME   Path to a runtime tree (tcl_library/,
#                            tk_library/) to stage alongside the payload. Set
#                            together with a from-source static wish, this
#                            produces a self-contained image that needs no
#                            Tcl on the target.
#                            zipfs/build-selfcontained.sh builds both and
#                            calls here.
#   ROBCO_SETTINGS_ZIP_OUT   Path to write the staged payload to as a plain
#                            zip, for a build that embeds the archive in
#                            somebody else's executable rather than stubbing
#                            it onto a wish of its own: the Windows terminal
#                            carries this zip in its PE image and mounts it
#                            from C. Set alone, no image is built and no wish
#                            is needed; set together with
#                            ROBCO_SETTINGS_WISH, one invocation produces
#                            both, so the standalone image stays available as
#                            the proof that the payload runs.
#   ROBCO_SETTINGS_TCLTEST   Path to a tcltest-*.tm module to stage, so the
#                            suites in tests/ can be run from inside the
#                            finished image. Unset, the module is left out
#                            and the image's --selftest says so plainly
#                            rather than failing.
#
# A further env var, ROBCO_SETTINGS_DIST_DIR, overrides where the image is
# written; it defaults to <repo>/dist.

package require Tcl 9

set repo  [file dirname [file dirname [file normalize [info script]]]]

# Version is read from the launcher so it has exactly one home.
set launcher [file join $repo robco-settings]
set fh [open $launcher r]
set launcher_text [read $fh]
close $fh
set ver ""
foreach line [split $launcher_text \n] {
    if {[regexp {^set ROBCO_SETTINGS_VERSION\s+(\S+)} $line -> ver]} break
}
if {$ver eq ""} {
    puts stderr "build: could not read ROBCO_SETTINGS_VERSION from $launcher"
    exit 1
}

# What this run is asked to produce. A bare zip is for an executable that
# is not ours to stub - the Windows terminal links Tcl and Tk itself and
# mounts this archive out of its own image - so a run asked only for the zip
# needs no wish at all. Asked for both, it makes both from one staging.
set zipout ""
if {[info exists ::env(ROBCO_SETTINGS_ZIP_OUT)] && $::env(ROBCO_SETTINGS_ZIP_OUT) ne ""} {
    set zipout $::env(ROBCO_SETTINGS_ZIP_OUT)
}
if {[info exists ::env(ROBCO_SETTINGS_WISH)] && $::env(ROBCO_SETTINGS_WISH) ne ""} {
    set wish $::env(ROBCO_SETTINGS_WISH)
} elseif {$zipout eq ""} {
    set wish [lindex [auto_execok wish9.0] 0]
} else {
    # Only the zip was asked for. Falling back to a wish on PATH here would
    # build an image nobody asked for out of whatever interpreter the build
    # host happens to have.
    set wish ""
}
if {$wish eq "" && $zipout eq ""} {
    puts stderr "build: wish stub not found (set ROBCO_SETTINGS_WISH or install wish9.0)"
    exit 1
}
if {$wish ne "" && ![file executable $wish]} {
    puts stderr "build: wish stub $wish is not executable"
    exit 1
}

# Stage the archive contents. main.tcl and the launcher sit at the staging
# root so that, post-strip, they land at //zipfs:/app/main.tcl and
# //zipfs:/app/robco-settings (an mkimg image mounts at //zipfs:/app).
# Staging root: TMPDIR on Unix, TEMP/TMP on Windows, /tmp where none is set.
set tmpbase /tmp
foreach v {TMPDIR TEMP TMP} {
    if {[info exists ::env($v)] && $::env($v) ne ""} { set tmpbase $::env($v); break }
}
set stage [file join $tmpbase robco-settings-zipfs-stage-[pid]]
file delete -force $stage
file mkdir $stage
file copy $launcher                       [file join $stage robco-settings]
file copy [file join $repo zipfs main.tcl] [file join $stage main.tcl]
file copy [file join $repo zipfs selftest.tcl] [file join $stage selftest.tcl]
# tests/ travels with lib/ and ui/ so that the finished image can be asked
# to test itself. A suite that only ever runs against the checkout proves
# the scripts are right; run from inside the image it also proves the image
# carries them, which is the half that breaks on a packaging change.
foreach d {lib ui tests} {
    file copy [file join $repo $d] [file join $stage $d]
}

# tcltest is a module rather than part of the script library, so a
# from-source runtime tree does not bring it: it is named separately or the
# image simply cannot run its suites. ::tcl::tm looks under
# <dirname of [info library]>/tcl9/<major.minor>, and [info library] in the
# image is //zipfs:/app/tcl_library, so the module goes here.
if {[info exists ::env(ROBCO_SETTINGS_TCLTEST)] && $::env(ROBCO_SETTINGS_TCLTEST) ne ""} {
    set tm $::env(ROBCO_SETTINGS_TCLTEST)
    if {![file readable $tm]} {
        puts stderr "build: ROBCO_SETTINGS_TCLTEST names no readable file: $tm"
        exit 1
    }
    set tmdir [file join $stage tcl9 9.0]
    file mkdir $tmdir
    file copy $tm [file join $tmdir [file tail $tm]]
}

# A self-contained stub carries no script library of its own, so overlay the
# from-source runtime (tcl_library/, tk_library/). An mkimg image mounts at
# //zipfs:/app, so these land at //zipfs:/app/tcl_library etc., where the
# interpreter finds them on the default auto_path.
if {[info exists ::env(ROBCO_SETTINGS_RUNTIME)] && $::env(ROBCO_SETTINGS_RUNTIME) ne ""} {
    set runtime $::env(ROBCO_SETTINGS_RUNTIME)
    foreach name [glob -nocomplain -tails -directory $runtime *] {
        file copy [file join $runtime $name] [file join $stage $name]
    }
}

# The bare zip, for an executable that mounts the archive itself. Same
# staging, same root-relative paths as the image: what the terminal mounts at
# //zipfs:/app is byte for byte what the standalone image carries there.
if {$zipout ne ""} {
    file mkdir [file dirname [file normalize $zipout]]
    file delete -force $zipout
    zipfs mkzip $zipout $stage $stage
    puts "built $zipout"
}

if {$wish eq ""} {
    file delete -force $stage
    exit 0
}

set distdir [file join $repo dist]
if {[info exists ::env(ROBCO_SETTINGS_DIST_DIR)] && $::env(ROBCO_SETTINGS_DIST_DIR) ne ""} {
    set distdir $::env(ROBCO_SETTINGS_DIST_DIR)
}
file mkdir $distdir
switch -- $tcl_platform(os) {
    Darwin  { set os macos }
    default {
        set os [expr {$tcl_platform(platform) eq "windows" ? "windows" : "linux"}]
    }
}
# One token per architecture across the three platforms: Linux says aarch64
# where macOS says arm64, and Windows says amd64 where both others say x86_64.
set arch [string tolower $tcl_platform(machine)]
set arch [dict getdef {aarch64 arm64  amd64 x86_64  intel x86_64} $arch $arch]
# Windows resolves an executable by extension, so the image is only runnable
# with one; the other two platforms carry none.
set ext [expr {$os eq "windows" ? ".exe" : ""}]
set out [file join $distdir "robco-settings-$ver-$os-$arch$ext"]
file delete -force $out

# strip == stage makes archive paths root-relative; the stub wish provides the
# Tk-capable interpreter.
zipfs mkimg $out $stage $stage {} $wish
# Windows carries no permission bits on a file, and its `file attributes` has
# no -permissions option at all, so setting the exec bit is a Unix step.
if {$tcl_platform(platform) ne "windows"} {
    file attributes $out -permissions 0755
}
file delete -force $stage

puts "built $out"
