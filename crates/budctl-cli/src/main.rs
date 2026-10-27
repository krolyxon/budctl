//! `budctl`: control wireless earbuds from Linux.
//!
//! A thin layer over budctl-core: parse the command line, run one command, print the result,
//! turn errors into the exit codes listed in CLAUDE.md.

mod output;

use std::io::{self, Write};
use std::process::ExitCode;

use budctl_core::bt::BdAddr;
use budctl_core::status::{AncLevel, NoiseMode};
use budctl_core::{Error, Kind};
use clap::{Parser, Subcommand, ValueEnum};

/// Control wireless earbuds from Linux, without the vendor app.
#[derive(Parser)]
#[command(name = "budctl", version)]
struct Cli {
    /// The buds to use, when more than one supported pair is paired.
    #[arg(long, global = true, value_name = "MAC")]
    device: Option<BdAddr>,

    /// Machine-readable output (JSON with "schema": 1).
    #[arg(long, global = true)]
    json: bool,

    /// Hex-dump every frame sent and received on stderr.
    #[arg(long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Paired devices and whether budctl recognizes them.
    List,
    /// Battery, noise control, EQ and switches.
    Status,
    /// Touch controls (gesture table) of each bud.
    Touch,
    /// Set noise control, then read it back.
    Anc {
        #[arg(value_enum)]
        mode: AncArg,
    },
    /// Select an EQ preset (pure-raw, deep-bass, pure-vocals, pure-bass), then read it back.
    Eq { preset: String },
    /// Turn an on/off setting on or off, then read it back. Switches: game, bass-boost, spatial (3D sound), wind, dual.
    Set {
        /// The switch's name, as `status --json` lists it.
        switch: String,
        #[arg(value_enum)]
        state: OnOff,
    },
    /// Turn game mode (low latency) on or off, then read it back.
    Game {
        #[arg(value_enum)]
        state: OnOff,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum OnOff {
    On,
    Off,
}

#[derive(Clone, Copy, ValueEnum)]
enum AncArg {
    Off,
    Transparency,
    /// Noise cancelling at the deep level (the buds use the same value for both).
    On,
    Light,
    Medium,
    Deep,
}

impl From<AncArg> for NoiseMode {
    fn from(arg: AncArg) -> Self {
        match arg {
            AncArg::Off => NoiseMode::Off,
            AncArg::On | AncArg::Deep => NoiseMode::Anc(AncLevel::Deep),
            AncArg::Light => NoiseMode::Anc(AncLevel::Light),
            AncArg::Medium => NoiseMode::Anc(AncLevel::Medium),
            AncArg::Transparency => NoiseMode::Transparency,
        }
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            // Help and version are not errors; everything else is a usage error (exit 1).
            let code = if error.use_stderr() { 1 } else { 0 };
            // Nothing useful can be done if printing the usage message fails.
            let _ = error.print();
            return ExitCode::from(code);
        }
    };
    if cli.debug {
        budctl_core::debug::enable();
    }

    let mut stdout = io::stdout().lock();
    let result = run(&cli, &mut stdout).and_then(|()| stdout.flush().map_err(output::Failure::Io));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        // `budctl status | head -1` is fine.
        Err(output::Failure::Io(e)) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(output::Failure::Io(e)) => {
            eprintln!("budctl: cannot write output: {e}");
            ExitCode::from(1)
        }
        Err(output::Failure::Budctl(e)) => {
            eprintln!("budctl: {e}");
            ExitCode::from(exit_code(&e))
        }
    }
}

fn run(cli: &Cli, out: &mut impl Write) -> Result<(), output::Failure> {
    let found = budctl_core::scan()?;
    if let Command::List = cli.command {
        return output::list(out, &found, cli.json);
    }

    let (device, model) = budctl_core::select(found, cli.device)?;
    let mut session = budctl_core::connect(&device, model)?;
    match cli.command {
        Command::List => Ok(()),
        Command::Status => {
            let status = session.status()?;
            output::status(out, &device, model, &status, cli.json)
        }
        Command::Touch => {
            let touch = session.touch_controls()?;
            output::touch(out, &device, model, &touch, cli.json)
        }
        Command::Anc { mode } => {
            let now = session.set_noise_control(mode.into())?;
            output::noise_control(out, &device, model, &now, cli.json)
        }
        Command::Eq { ref preset } => {
            let now = session.set_eq(preset)?;
            output::eq(out, &device, model, &now, cli.json)
        }
        Command::Set { ref switch, state } => {
            let now = session.set_switch(switch, matches!(state, OnOff::On))?;
            output::switch(out, &device, model, &now, cli.json)
        }
        Command::Game { state } => {
            let now = session.set_switch("game", matches!(state, OnOff::On))?;
            output::switch(out, &device, model, &now, cli.json)
        }
    }
}

/// Exit codes from CLAUDE.md: 1 usage, 2 not found, 3 busy, 4 rejected or timed out.
fn exit_code(error: &Error) -> u8 {
    match error.kind() {
        Kind::Usage => 1,
        Kind::NotFound => 2,
        Kind::Busy => 3,
        Kind::Failed => 4,
    }
}
