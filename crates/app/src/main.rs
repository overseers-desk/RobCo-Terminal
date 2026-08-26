//! The RobCo Terminal binary.
//!
//! Startup order is load-bearing at three points:
//!
//! 1. **Identity before anything else.** The binary's basename decides the
//!    WM_CLASS, the instance lock and the data directory, so it is
//!    resolved first and everything downstream is told what it is.
//! 2. **Crash log before the single-instance check**, so a crash in the
//!    arbitration itself is still logged.
//! 3. **Single-instance check before the event loop.** A second launch
//!    hands its request over and exits without ever opening a display
//!    connection, which is what makes it fast and what makes it work when
//!    the second invocation has no display of its own.
//!
//! The shell is the outer frame; what it puts in a window is the wgpu
//! surface and rio-vt session, behind the `shell::Surface` seam, with the
//! settings handle watching the config file alongside.

use std::process::ExitCode;
use std::sync::Arc;

use app::cli::{self, Outcome};
use app::instance::{self, NewWindow, Role};
use app::shell::{Shell, ShellConfig, ShellEvent};
use app::window::TerminalSurface;
use app::{crashlog, paths, settings};
use term::SessionConfig;

fn main() -> ExitCode {
    let identity = app::identity();

    let options = match cli::parse(&identity, std::env::args_os().skip(1)) {
        Outcome::Run(options) => options,
        Outcome::Print(text) => {
            print!("{text}");
            return ExitCode::SUCCESS;
        }
        Outcome::Fail(text) => {
            eprint!("{text}");
            return ExitCode::FAILURE;
        }
    };

    // The settings dump is pure output: no window, no instance lock, no
    // config file touched. Handled before anything else starts so external
    // tools can call it while a terminal is running.
    if options.dump_settings {
        let fonts = term::fonts()
            .iter()
            .map(|f| config::dump::FontListing {
                name: f.name.to_string(),
                text: f.text.to_string(),
            })
            .collect();
        print!("{}", config::dump::dump(fonts));
        return ExitCode::SUCCESS;
    }

    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("robco_app=info,app=info"),
    );
    if options.verbose {
        builder.filter_level(log::LevelFilter::Debug);
    }
    let _ = builder.try_init();

    let crash_log = crashlog::install(&paths::crash_dir(&identity));
    if options.verbose {
        match &crash_log {
            Some(path) => eprintln!("crash log armed: {}", path.display()),
            None => eprintln!("crash log not armed"),
        }
    }

    // The process's own working directory is what a PTY child inherits, so
    // setting it once at startup, before anything spawns, is enough.
    if let Some(workdir) = &options.workdir {
        if let Err(e) = std::env::set_current_dir(workdir) {
            eprintln!("--workdir {}: {e}", workdir.display());
            return ExitCode::FAILURE;
        }
    }

    // `--profile <name>` names a saved appliance or a built-in screen, in
    // that order, and an unknown name is refused rather than papered over
    // with the default (see `settings::ProfileSelection` for both halves of
    // that). Resolved before the single-instance handoff, so a bad name is
    // a bad name whether or not another instance is already up.
    //
    // Under `--default-settings` the saved-appliance half is off: that flag
    // means "ignoring any real user config", and a saved profile is real
    // user config.
    let profile = match &options.profile {
        None => None,
        Some(name) => match settings::select_profile(name, !options.default_settings) {
            Ok(selection) => Some(selection),
            Err(unknown) => {
                eprintln!("unknown profile: {}", unknown.name);
                eprintln!("available profiles:");
                for name in unknown.known_screens {
                    eprintln!("  {name}");
                }
                return ExitCode::FAILURE;
            }
        },
    };
    if options.verbose {
        match &profile {
            Some(settings::ProfileSelection::SavedProfile { path, .. }) => {
                eprintln!("profile: saved appliance {}", path.display());
            }
            Some(settings::ProfileSelection::ScreenPreset(preset)) => {
                eprintln!("profile: built-in screen {:?}", preset.name);
            }
            None => {}
        }
    }
    if options.verbose {
        eprintln!("default settings: {}", options.default_settings);
        if let Some(command) = &options.command {
            eprintln!("command: {:?} {:?}", command.program, command.args);
        }
    }

    let request = NewWindow {
        fullscreen: options.fullscreen,
        ssh: options.ssh.clone(),
    };
    let role = instance::acquire(&identity, request);
    if matches!(role, Role::Delivered) {
        // A primary took the new-window request: from the launcher's point
        // of view the window was opened.
        return ExitCode::SUCCESS;
    }

    // The config file behind a live handle. `--default-settings`
    // is the contract's "never touch the user's real config" switch, so
    // that is the one case where nothing is loaded and nothing is watched.
    // This is attached to every `TerminalSurface` the shell opens
    // (below), so the pointer's inverse-distortion transform reads live
    // margin/frame/curvature settings rather than the identity
    // placeholder.
    let settings_handle = if options.default_settings {
        None
    } else {
        let path = match &profile {
            Some(selection) => selection.config_path(),
            None => settings::config_path(),
        };
        match settings::SettingsHandle::spawn_with_profile(
            path.clone(),
            profile.clone(),
            |_old, new, class| {
                log::debug!(
                    "settings applied ({class:?}); font_scaling={} bloom={}",
                    new.general.font_scaling,
                    new.screen.bloom
                );
            },
        ) {
            Ok(handle) => {
                let handle = Arc::new(handle);
                if let Err(err) = settings::install_sigusr1_handler(Arc::clone(&handle)) {
                    log::error!("could not install SIGUSR1 handler: {err}");
                }
                if options.verbose {
                    let initial = handle.current();
                    eprintln!(
                        "settings: {} (screen={:?} chassis={:?})",
                        path.display(),
                        initial.screen.name,
                        initial.chassis.name
                    );
                }
                Some(handle)
            }
            Err(err) => {
                log::error!(
                    "could not start watching {}: {err}; continuing with defaults, no live reload",
                    path.display()
                );
                None
            }
        }
    };

    // What the first window's minimum-size hint reserves for the channel bank,
    // before any surface exists to measure one: the cabinet the shipped or the
    // user's profile asks for. The bank's width does not depend on the window's
    // size, so the default size is only there to build a cabinet at all. Every
    // later change -- a settings reload, a seam drag -- reaches the shell as
    // `ShellEvent::SetBankWidth` from the surface that measured it.
    let initial_config = match &settings_handle {
        Some(handle) => handle.current(),
        None => {
            // `--default-settings`: no file is read and none is watched, so
            // the overlay the handle's loader would have applied has to be
            // applied here instead. This is contract item 1's second half,
            // "seeded from the named built-in profile": without it the flag
            // pair would start from the defaults and quietly ignore the
            // name it was given.
            let mut config = config::Config::default();
            if let Some(selection) = &profile {
                selection.overlay(&mut config);
            }
            config
        }
    };

    // The look this run actually came up wearing, after the file, the
    // profile and the defaults have all had their say. Printed because it
    // is the only place the three meet: `--profile` is honored through two
    // different paths depending on `--default-settings`, and a line naming
    // the resolved screen is what makes "the flag changed the look" a thing
    // that can be checked rather than assumed.
    if options.verbose {
        eprintln!(
            "look: screen={:?} chassis={:?} font_color={:?} background_color={:?}",
            initial_config.screen.name,
            initial_config.chassis.name,
            initial_config.screen.font_color,
            initial_config.screen.background_color,
        );
    }
    let (default_width, default_height) = app::geometry::DEFAULT_SIZE;
    let bank = chassis::Cabinet::from_config(
        &initial_config,
        f64::from(default_width),
        f64::from(default_height),
    );
    let (bank_width, bank_minimum) = (bank.bank_width(), bank.min_bank_width());

    let (event_loop, proxy) = match Shell::event_loop() {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("cannot create the event loop: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Start serving new-window requests only once there is an event loop
    // to hand them to. Until this line a second launch finds the lock
    // held but nothing listening, and degrades to an independent
    // instance -- a window either way, which is the outcome that matters.
    let _primary = match role {
        Role::Primary(mut primary) => {
            if options.verbose {
                eprintln!(
                    "single instance: primary on {}",
                    primary.socket_path().display()
                );
            }
            primary.serve(move |request| {
                let _ = proxy.send_event(ShellEvent::NewWindow(request));
            });
            Some(primary)
        }
        _ => None,
    };

    // `-e cmd args...` beats `--program`: `-e` is the one that swallows the
    // rest of the command line.
    let mut session = SessionConfig::default();
    if let Some(command) = &options.command {
        session.program = Some(command.program.to_string_lossy().into_owned());
        session.args = command
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
    } else if let Some(program) = &options.program {
        session.program = Some(program.to_string_lossy().into_owned());
    }

    let mut shell_config = ShellConfig::empty(&identity);
    shell_config.fullscreen = options.fullscreen;
    shell_config.ssh = options.ssh.clone();
    shell_config.bank_width = bank_width;
    shell_config.bank_minimum = bank_minimum;
    // At scale factor 1, which is the unit the default window size is quoted
    // in; each window re-measures against its own factor once it has one.
    shell_config.well_minimum = app::window::well_minimum_for(&initial_config, 1.0);
    // A second proxy, for the other direction: the single-instance listener's
    // hands new windows in, and each surface hands its bank width back out.
    let surface_proxy = event_loop.create_proxy();
    let frame_stats_enabled = options.frame_stats;
    shell_config.surface_factory = Box::new(move |window, ssh| {
        // The spelling was validated at the CLI (and a handoff's came off a
        // CLI too); one that fails anyway opens the window on a shell.
        let dial = ssh.and_then(|spec| app::ssh::SshRequest::parse(spec).ok());
        let mut surface =
            TerminalSurface::new(window, &session.clone(), frame_stats_enabled, dial.as_ref());
        surface.set_shell_events(surface_proxy.clone());
        // The config this run resolved, whether or not a file is behind it.
        // Under `--default-settings` there is no handle to attach and this is
        // the only thing carrying `--profile`'s look to the glass; with a
        // handle the surface prefers the watched file and never reads it.
        surface.set_config(initial_config.clone());
        if let Some(handle) = &settings_handle {
            surface.set_settings(Arc::clone(handle));
        }
        Box::new(surface)
    });

    if let Err(e) = Shell::new(shell_config).run(event_loop) {
        eprintln!("event loop stopped: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
