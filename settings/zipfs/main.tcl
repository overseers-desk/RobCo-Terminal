# zipfs mkimg entry point.
#
# When robco-settings is built into a single-file executable with `zipfs
# mkimg`, the interpreter auto-mounts the appended archive at //zipfs:/ and
# sources the main.tcl found at its root, then exits. So this script sources
# the real launcher (the single home of boot logic) and then holds the Tk
# event loop: without the vwait, the interpreter would exit the moment
# ::rcsettings::ui::app::start returns, flashing the window and quitting.
# The app's own quit path ends the vwait.
set _root [file dirname [info script]]
source [file join $_root robco-settings]
vwait forever
