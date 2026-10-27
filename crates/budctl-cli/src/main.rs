//! `budctl`: control wireless earbuds from Linux.
//!
//! A thin layer over budctl-core: parse the command line, run one command, print the result,
//! turn errors into the exit codes listed in CLAUDE.md.

mod output;

use std::io::{self, Write};
use std::process::ExitCode;

use budctl_core::bt::BdAddr;
use budctl_core::status::{AncLevel, EqBand, NoiseMode};
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
    /// Touch controls (gesture table) of each bud, or give one gesture a new action.
    ///
    /// Gestures: double-tap, triple-tap, hold. Actions: none, play-pause, next, previous,
    /// voice-assistant, volume-up, volume-down, noise-cycle, game-mode (not every gesture takes
    /// every action). `budctl touch cycle …` sets what a hold steps through.
    #[command(args_conflicts_with_subcommands = true)]
    Touch {
        #[command(subcommand)]
        cycle: Option<TouchCommand>,
        /// The bud to change: left, right, or both.
        #[arg(value_enum, requires = "gesture")]
        side: Option<Side>,
        /// The gesture to change.
        #[arg(requires = "action")]
        gesture: Option<String>,
        /// Its new action.
        action: Option<String>,
        /// Read the table and show the write, but send nothing.
        #[arg(long, requires = "side")]
        dry_run: bool,
    },
    /// Set noise control, then read it back.
    Anc {
        #[arg(value_enum)]
        mode: AncArg,
    },
    /// List the EQ presets, or select one (pure-raw, deep-bass, pure-vocals, pure-bass, custom-<id>) and read it back.
    ///
    /// For a custom preset, `<hz>=<gain>` pairs change those bands' gains (dB), then save and
    /// select it: `budctl eq custom-4 50=+3 16000=-2`. `budctl eq new <name>` creates a preset,
    /// `budctl eq rename custom-<id> <name>` renames one.
    #[command(args_conflicts_with_subcommands = true)]
    Eq {
        #[command(subcommand)]
        action: Option<EqCommand>,
        preset: Option<String>,
        /// Bands to change, as `<hz>=<gain>`. Custom presets only.
        #[arg(requires = "preset", value_parser = parse_band, allow_negative_numbers = true)]
        bands: Vec<EqBand>,
    },
    /// Turn an on/off setting on or off, then read it back. Switches: game, bass-boost, spatial (3D sound), wind, dual.
    Set {
        /// The switch's name, as `status --json` lists it.
        switch: String,
        #[arg(value_enum)]
        state: OnOff,
    },
    /// Set the bass level, -5 to +5. The buds cannot report it back, so only their ack is checked.
    Bass {
        /// Checked here, before connecting; core checks it again.
        #[arg(allow_negative_numbers = true, value_parser = clap::value_parser!(i8).range(-5..=5))]
        level: i8,
    },
    /// Ring the buds so you can find them (loud: take them out of your ears first).
    Find {
        #[arg(value_enum)]
        action: FindAction,
        /// Confirm that the buds are out of your ears. Needed to start ringing.
        #[arg(long)]
        yes: bool,
    },
    /// Turn game mode (low latency) on or off, then read it back.
    Game {
        #[arg(value_enum)]
        state: OnOff,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum FindAction {
    Start,
    Stop,
}

#[derive(Subcommand)]
enum EqCommand {
    /// Create a custom preset, flat, with the bands of the buds' custom presets, then read it back.
    New {
        /// Its name, 1 to 16 bytes.
        #[arg(value_parser = parse_name)]
        name: String,
    },
    /// Give a custom preset a new name, then read it back. The buds save it, which also selects
    /// it.
    Rename {
        /// The preset, as `budctl eq` lists it: custom-<id>.
        preset: String,
        /// Its new name, 1 to 16 bytes.
        #[arg(value_parser = parse_name)]
        name: String,
    },
}

#[derive(Subcommand)]
enum TouchCommand {
    /// Set the noise-control modes a hold steps through (both buds), then read it back.
    ///
    /// At least two modes. The buds step through them in their own order.
    Cycle {
        #[arg(value_enum, required = true, num_args = 2..=3, value_name = "MODE")]
        modes: Vec<CycleMode>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CycleMode {
    Off,
    Transparency,
    /// Noise cancelling.
    Anc,
}

impl CycleMode {
    fn name(self) -> &'static str {
        match self {
            CycleMode::Off => "off",
            CycleMode::Transparency => "transparency",
            CycleMode::Anc => "anc",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Side {
    Left,
    Right,
    /// The left and the right bud.
    Both,
}

impl Side {
    fn name(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Both => "both",
        }
    }
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
    // Checked before connecting: ringing is loud. Spec: realme_t310.md#find
    if let Command::Find {
        action: FindAction::Start,
        yes: false,
    } = cli.command
    {
        return Err(Error::Usage(
            "this rings the buds loudly; take them out of your ears, then run `budctl find start --yes`".to_owned(),
        )
        .into());
    }
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
        Command::Touch {
            cycle: Some(TouchCommand::Cycle { ref modes }),
            ..
        } => {
            let names: Vec<&str> = modes.iter().map(|m| m.name()).collect();
            let now = session.set_hold_cycle(&names)?;
            output::hold_cycle(out, &device, model, &now, cli.json)
        }
        Command::Touch {
            side: Some(side),
            gesture: Some(ref gesture),
            action: Some(ref action),
            dry_run,
            ..
        } => {
            let change = session.set_touch(side.name(), gesture, action, dry_run)?;
            output::touch_change(out, &device, model, &change, cli.json)
        }
        Command::Touch { .. } => {
            let touch = session.touch_controls()?;
            output::touch(out, &device, model, &touch, cli.json)
        }
        Command::Anc { mode } => {
            let now = session.set_noise_control(mode.into())?;
            output::noise_control(out, &device, model, &now, cli.json)
        }
        Command::Eq {
            action: Some(EqCommand::New { ref name }),
            ..
        } => {
            let created = session.create_custom_eq(name)?;
            output::created_eq(out, &device, model, &created, cli.json)
        }
        Command::Eq {
            action: Some(EqCommand::Rename { ref preset, ref name }),
            ..
        } => {
            let now = session.rename_custom_eq(preset, name)?;
            output::renamed_eq(out, &device, model, &now, cli.json)
        }
        Command::Eq { preset: None, .. } => {
            let presets = session.eq_presets()?;
            output::eq_presets(out, &device, model, &presets, cli.json)
        }
        Command::Eq {
            preset: Some(ref preset),
            ref bands,
            ..
        } if !bands.is_empty() => {
            let now = session.set_custom_eq(preset, bands)?;
            output::custom_eq(out, &device, model, &now, cli.json)
        }
        Command::Eq {
            preset: Some(ref preset),
            ..
        } => {
            let now = session.set_eq(preset)?;
            output::eq(out, &device, model, &now, cli.json)
        }
        Command::Set { ref switch, state } => {
            let now = session.set_switch(switch, matches!(state, OnOff::On))?;
            output::switch(out, &device, model, &now, cli.json)
        }
        Command::Bass { level } => {
            session.set_bass_level(level)?;
            output::bass_level(out, &device, model, level, cli.json)
        }
        Command::Find { action, .. } => {
            let ring = matches!(action, FindAction::Start);
            session.find(ring)?;
            output::find(out, &device, model, ring, cli.json)
        }
        Command::Game { state } => {
            let now = session.set_switch("game", matches!(state, OnOff::On))?;
            output::switch(out, &device, model, &now, cli.json)
        }
    }
}

/// A preset name, checked before connecting: not empty, no spaces around it, no control
/// characters. The buds' own limits are checked once their list is read.
fn parse_name(text: &str) -> Result<String, String> {
    if text.is_empty() || text.trim() != text {
        return Err("a preset name cannot be empty, or start or end with a space".to_owned());
    }
    if text.chars().any(char::is_control) {
        return Err("a preset name cannot hold control characters".to_owned());
    }
    Ok(text.to_owned())
}

/// `50=+3` → 50 Hz at +3 dB.
fn parse_band(text: &str) -> Result<EqBand, String> {
    let (hz, gain) = text
        .split_once('=')
        .ok_or_else(|| format!("{text} is not <hz>=<gain>, e.g. 50=+3"))?;
    Ok(EqBand {
        hz: hz.parse().map_err(|_| format!("{hz} is not a frequency in Hz"))?,
        gain: gain.parse().map_err(|_| format!("{gain} is not a gain in whole dB"))?,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Command {
        Cli::try_parse_from(std::iter::once("budctl").chain(args.iter().copied()))
            .unwrap()
            .command
    }

    #[test]
    fn eq_takes_a_preset_bands_or_new() {
        assert!(matches!(
            parse(&["eq"]),
            Command::Eq {
                action: None,
                preset: None,
                ..
            }
        ));
        assert!(matches!(
            parse(&["eq", "custom-4", "50=+3", "16000=-2"]),
            Command::Eq { action: None, preset: Some(p), bands } if p == "custom-4" && bands.len() == 2
        ));
        assert!(matches!(
            parse(&["eq", "deep-bass"]),
            Command::Eq { action: None, preset: Some(p), .. } if p == "deep-bass"
        ));
        assert!(matches!(
            parse(&["eq", "new", "Gym mix"]),
            Command::Eq { action: Some(EqCommand::New { name }), .. } if name == "Gym mix"
        ));
        assert!(matches!(
            parse(&["eq", "rename", "custom-5", "Gym"]),
            Command::Eq { action: Some(EqCommand::Rename { preset, name }), .. }
                if preset == "custom-5" && name == "Gym"
        ));
    }
}
