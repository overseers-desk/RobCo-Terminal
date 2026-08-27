//! Compiles and links the embedded settings window, under the
//! `embedded-settings` feature and on Windows only.
//!
//! What this arranges is one executable where there were two: the Tcl/Tk
//! settings application, its interpreter and its whole script tree, linked
//! into the terminal binary and reached through a flag on its own command
//! line rather than by spawning `robco-settings.exe` from the directory
//! beside it. Windows is where that matters, because a Windows install is a
//! thing users copy and a directory of loose files beside the exe is a thing
//! they lose half of.
//!
//! Two gates, and they are not the same gate:
//!
//! * The **feature** says the operator asked for this build. Without it this
//!   script returns on its first line and the crate builds exactly as it did
//!   before the feature existed -- no C compiler, no environment, no link
//!   arguments.
//! * The **target being Windows** says the build can actually be done. A
//!   build script runs on the *host*, so `cfg!(windows)` here would answer
//!   about the machine doing the compiling, not the machine being compiled
//!   for; `CARGO_CFG_WINDOWS` is the question worth asking. Gating on it
//!   means a Linux `cargo check -p robco-app --features embedded-settings`
//!   is a no-op here rather than a demand for five environment variables and
//!   a set of MSVC import libraries that cannot exist on that machine. It
//!   costs nothing in correctness: `settings_embed`'s real module is gated on
//!   `all(windows, feature = "embedded-settings")`, so on any other target
//!   the feature already resolves to the stub, which needs nothing from here.
//!
//! The five inputs come from `settings/zipfs/build-selfcontained.ps1 -Embed`,
//! which is the script that builds Tcl and Tk static and packs the settings
//! application's scripts into the payload zip. This script does not go
//! looking for them: a wrong Tcl found by searching is a link error a
//! thousand lines long, and a missing variable is a sentence.
//!
//! One more consequence of a build script running on the host: `cc` is a
//! build-dependency of the *host* platform, declared under
//! `cfg(windows)`, so on a Linux host the crate is not there to call at all.
//! The compiling half therefore sits behind a host `cfg(windows)` -- which
//! removes the code before anything looks for `cc` -- with the other arm
//! present and saying what it is: this link wants MSVC and MSVC-built `.lib`
//! files, so it wants a Windows host.

/// What `-Embed` exports, and what each one is.
const REQUIRED: [(&str, &str); 5] = [
    ("ROBCO_SETTINGS_ZIP", "the settings payload zip"),
    (
        "ROBCO_TCL_INCLUDE",
        "the `;`-separated Tcl/Tk include directories",
    ),
    ("ROBCO_TCL_LIB", "the static Tcl library"),
    ("ROBCO_TK_LIB", "the static Tk library"),
    ("ROBCO_TCL_STUB_LIB", "the Tcl stub library"),
];

fn main() {
    if std::env::var_os("CARGO_FEATURE_EMBEDDED_SETTINGS").is_none() {
        return;
    }
    for (name, _) in REQUIRED {
        println!("cargo:rerun-if-env-changed={name}");
    }
    // Building *for* Windows, whatever we are building on. See the module
    // doc: the host's own platform is not the question.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    // Tcl and Tk are built by `-Embed` as `OPTS=static,msvcrt`: static
    // libraries that expect the *dynamic* CRT (/MD). Rust's `+crt-static`
    // would link the static CRT into this binary instead, and then every CRT
    // symbol those libraries reference resolves against nothing. The failure
    // is hundreds of unresolved externals naming functions nobody wrote, so
    // it is worth one sentence here instead.
    if std::env::var("CARGO_CFG_TARGET_FEATURE")
        .unwrap_or_default()
        .split(',')
        .any(|f| f == "crt-static")
    {
        panic!(
            "the embedded settings window cannot be built with `+crt-static`: \
             settings/zipfs/build-selfcontained.ps1 -Embed builds Tcl and Tk as \
             OPTS=static,msvcrt, which is the dynamic CRT (/MD), and a static-CRT \
             link leaves every CRT symbol they call unresolved. Drop \
             `-C target-feature=+crt-static` from this build."
        );
    }

    let mut values = Vec::new();
    for (name, what) in REQUIRED {
        match std::env::var(name) {
            Ok(value) if !value.is_empty() => values.push(value),
            _ => panic!(
                "the `embedded-settings` feature needs {name}, {what}. These are \
                 exported by settings/zipfs/build-selfcontained.ps1 -Embed; run it \
                 first and build from the environment it leaves."
            ),
        }
    }
    let [zip, includes, tcl_lib, tk_lib, stub_lib] = <[String; 5]>::try_from(values)
        .expect("one value per required variable");

    compile_appinit(&includes);

    // The three static libraries by full path rather than as `-l` names: they
    // are wherever `-Embed` built them, which is not on any search path, and
    // a path is one fact where a directory plus a name is two. Tk before Tcl
    // before the stubs, which is the order their dependencies run in.
    for lib in [&tk_lib, &tcl_lib, &stub_lib] {
        println!("cargo:rustc-link-arg={lib}");
    }
    // What Tcl and Tk themselves call into on Windows. A static Tcl/Tk
    // carries none of these; they are named here because nothing else in this
    // binary's link line has a reason to.
    for lib in [
        "netapi32",
        "user32",
        "advapi32",
        "userenv",
        "ws2_32",
        "gdi32",
        "comdlg32",
        "imm32",
        "comctl32",
        "shell32",
        "uuid",
        "ole32",
        "oleaut32",
        "winspool",
    ] {
        println!("cargo:rustc-link-lib={lib}");
    }

    // The payload's path, handed to the source so `include_bytes!` can name
    // it. The zip is a build input like any other: change it and the crate
    // rebuilds, through the rerun-if-env-changed above.
    println!("cargo:rustc-env=ROBCO_SETTINGS_ZIP={zip}");
}

/// The interpreter's own main: it mounts the payload zip as the
/// interpreter's filesystem and hands control to `Tk_Main`. Compiled with the
/// stubs off, because there is nothing to stub against -- Tcl and Tk are in
/// the same link as this object.
#[cfg(windows)]
fn compile_appinit(includes: &str) {
    let appinit = std::path::PathBuf::from("../../settings/zipfs/appinit.c");
    println!("cargo:rerun-if-changed={}", appinit.display());
    let mut build = cc::Build::new();
    build
        .file(&appinit)
        .define("STATIC_BUILD", None)
        .define("USE_TCL_STUBS", "0")
        .define("USE_TK_STUBS", "0")
        .define("ROBCO_EMBEDDED_SETTINGS", None);
    for dir in includes.split(';').filter(|d| !d.is_empty()) {
        build.include(dir);
    }
    build.compile("robco_settings_embed");
}

/// The same step on a host that cannot take it. Reaching here means a
/// cross-compile to Windows with the feature on: the libraries `-Embed`
/// hands over are MSVC `.lib` files and the C beside them wants the MSVC
/// headers, so the build wants a Windows host and says so rather than
/// failing later with a linker's vocabulary.
#[cfg(not(windows))]
fn compile_appinit(_includes: &str) {
    panic!(
        "the embedded settings window is built with MSVC against the static Tcl \
         and Tk that settings/zipfs/build-selfcontained.ps1 -Embed produces, so it \
         has to be built on Windows. Cross-compiling to Windows with the \
         `embedded-settings` feature is not a supported build."
    );
}
