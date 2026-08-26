# Headless suite runner: tclsh9.0 settings/tests/all.tcl
# The tests that run the real --dump-settings need a robco-term to run.
# One on PATH is found the way the app finds it; a build tree's is named by
# setting ROBCO_SETTINGS_TEST_BINARY, which helpers.tcl hands to the dump
# namespace. That variable is the suite's own and not configuration: the app
# itself reads no environment to find its terminal. Without either, those
# tests skip.

package require tcltest 2.5

::tcltest::configure -testdir [file dirname [file normalize [info script]]]
# tcltest's temporary directory is the working directory by default, and
# the suites write config files into it. Keep them out of the checkout.
::tcltest::configure -tmpdir [file tempdir robco-settings-tests]
::tcltest::configure {*}$argv
::tcltest::runAllTests
