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
#
# A third env var, ROBCO_SETTINGS_DIST_DIR, overrides where the image is
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

if {[info exists ::env(ROBCO_SETTINGS_WISH)] && $::env(ROBCO_SETTINGS_WISH) ne ""} {
    set wish $::env(ROBCO_SETTINGS_WISH)
} else {
    set wish [lindex [auto_execok wish9.0] 0]
}
if {$wish eq "" || ![file executable $wish]} {
    puts stderr "build: wish stub not found (set ROBCO_SETTINGS_WISH or install wish9.0)"
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
foreach d {lib ui} {
    file copy [file join $repo $d] [file join $stage $d]
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
