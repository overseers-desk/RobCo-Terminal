//! The workspace's evaluation harness: deterministic screenshots, carve-out
//! masking, screenshot comparison, the CLI/window contract check, and the
//! Linux install/dist/deb targets. See the doc comment on each subcommand
//! module for what it does and the calls it makes along the way.
//!
//! The `snap` subcommand is parameterized entirely on an executable path: it
//! makes no assumption that the target binary is `robco-term` itself, only
//! that it honors the CLI/window contract `snap::CONTRACT` states, so it can
//! be pointed at any binary implementing that contract.

mod compare;
mod fanout;
mod install;
mod mask;
mod proc;
mod snap;
mod verify;
mod x11;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "xtask", about = "RobCo Terminal eval harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Screenshot one build of the terminal at a given profile and size.
    Snap {
        /// Path to the terminal binary under test.
        binary: PathBuf,
        /// Profile name, passed as `--profile <name>`.
        profile: String,
        /// Output PNG path.
        out: PathBuf,
        #[arg(long, default_value_t = 1448)]
        width: u32,
        #[arg(long, default_value_t = 1086)]
        height: u32,
        /// Total channels (tabs) to open via repeated Ctrl+Shift+T, counting
        /// the one the app starts with. 1 or fewer opens none.
        #[arg(long, default_value_t = 16)]
        channels: u32,
        /// If given, re-fit the LED strips to this character count by
        /// dragging the bank/screen-well seam, the way a user would.
        #[arg(long)]
        units: Option<i64>,
        /// Launch the binary's first channel with a fixed `-e yes <pattern>`
        /// instead of the user's shell, so the glass shows the same
        /// deterministic text at the same positions on every binary and
        /// every run: a scene RMSE taken over a real login shell's banner
        /// absorbs whatever that shell's prompt/motd happens to wrap to,
        /// which is content noise, not a rendering difference.
        #[arg(long)]
        deterministic: bool,
    },
    /// Compare two screenshots by RMSE, over the full frame, a named region,
    /// or a literal pixel crop. See `compare`'s module doc for the metric,
    /// the region conventions, and the self-floor workflow.
    Compare {
        /// First screenshot.
        a: PathBuf,
        /// Second screenshot.
        b: PathBuf,
        /// A literal `x,y,w,h` pixel crop, mutually exclusive with `--region`.
        #[arg(long, value_parser = compare::parse_crop, conflicts_with = "region")]
        crop: Option<compare::Crop>,
        /// A named region carrying this repo's recorded crop conventions,
        /// mutually exclusive with `--crop`.
        #[arg(long, value_enum, conflicts_with = "crop")]
        region: Option<compare::Region>,
        /// Also print the luma/chroma split of the RMSE.
        #[arg(long)]
        channels: bool,
    },
    /// Blacken the judged-out regions of a terminal image, from a
    /// mock-metrics JSON.
    Mask {
        /// Carve-out geometry JSON (e.g. deep-blue-metrics.json).
        metrics: PathBuf,
        /// Input screenshot.
        image: PathBuf,
        /// Output masked PNG.
        out: PathBuf,
    },
    /// Print the CLI/window contract a binary must honor for `snap` to be
    /// able to drive it at all.
    Contract,
    /// Count a setting's non-test homes across the crates (issue #4's
    /// fan-out measure), or with --loc the workspace's non-test line count.
    Fanout {
        /// The setting or term to count, e.g. `bloom`. Omit with --loc.
        term: Option<String>,
        /// Print the whole workspace's Rust line count via cloc instead.
        #[arg(long)]
        loc: bool,
    },
    /// Drive a binary through that contract item by item and report which
    /// ones it honors, including the single-instance rule (a second
    /// invocation opens a window in the first instance).
    Verify {
        /// Path to the terminal binary under test.
        binary: PathBuf,
        /// Profile name, passed as `--profile <name>`.
        #[arg(long, default_value = "Default Amber")]
        profile: String,
        #[arg(long, default_value_t = 1448)]
        width: u32,
        #[arg(long, default_value_t = 1086)]
        height: u32,
    },
    /// Install the Linux build into a prefix: bin/, share/applications/ and
    /// share/icons/ per the XDG spec. See `install`'s module doc for what is
    /// compiled into the binary versus written to disk.
    Install {
        /// Where the installation goes, e.g. `$HOME/.local` or `/usr`.
        #[arg(long)]
        prefix: PathBuf,
        /// Stage under this directory instead of writing to the prefix
        /// directly, the usual DESTDIR idiom. The prefix is still the prefix.
        #[arg(long)]
        destdir: Option<PathBuf>,
        /// Package this binary instead of building a fresh release one.
        #[arg(long)]
        binary: Option<PathBuf>,
    },
    /// Build a versioned `tar.gz` of the install layout. The version is read
    /// from the workspace manifest; nothing is bumped and nothing is
    /// published.
    Dist {
        /// Where the tarball is written.
        #[arg(long, default_value = "dist")]
        out_dir: PathBuf,
        /// Package this binary instead of building a fresh release one.
        #[arg(long)]
        binary: Option<PathBuf>,
    },
    /// Build a binary `.deb` from the install layout, without root.
    Deb {
        /// Where the package is written.
        #[arg(long, default_value = "dist")]
        out_dir: PathBuf,
        /// Package this binary instead of building a fresh release one.
        #[arg(long)]
        binary: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Snap {
            binary,
            profile,
            out,
            width,
            height,
            channels,
            units,
            deterministic,
        } => snap::run(snap::SnapArgs {
            binary,
            profile,
            out,
            width,
            height,
            channels,
            units,
            deterministic,
        }),
        Command::Compare {
            a,
            b,
            crop,
            region,
            channels,
        } => compare::run(compare::CompareArgs {
            a,
            b,
            crop,
            region,
            channels,
        }),
        Command::Mask {
            metrics,
            image,
            out,
        } => mask::run(&metrics, &image, &out),
        Command::Contract => {
            print!("{}", snap::CONTRACT);
            Ok(())
        }
        Command::Fanout { term, loc } => {
            if loc {
                fanout::loc()
            } else {
                match term {
                    Some(t) => fanout::fanout(&t),
                    None => anyhow::bail!("fanout needs a term, or --loc"),
                }
            }
        }
        Command::Verify {
            binary,
            profile,
            width,
            height,
        } => verify::run(verify::VerifyArgs {
            binary,
            profile,
            width,
            height,
        }),
        Command::Install {
            prefix,
            destdir,
            binary,
        } => install::install(install::InstallArgs {
            prefix,
            destdir,
            binary,
        }),
        Command::Dist { out_dir, binary } => install::dist(install::DistArgs { out_dir, binary }),
        Command::Deb { out_dir, binary } => install::deb(install::DebArgs { out_dir, binary }),
    }
}
