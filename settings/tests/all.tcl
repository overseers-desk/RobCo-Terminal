# Headless suite runner: tclsh9.0 settings/tests/all.tcl
# Set ROBCO_TERM to a robco-term binary to include the test that runs the
# real --dump-settings; without it that one test is skipped.

package require tcltest 2.5

::tcltest::configure -testdir [file dirname [file normalize [info script]]]
# tcltest's temporary directory is the working directory by default, and
# the suites write config files into it. Keep them out of the checkout.
::tcltest::configure -tmpdir [file tempdir robco-settings-tests]
::tcltest::configure {*}$argv
::tcltest::runAllTests
