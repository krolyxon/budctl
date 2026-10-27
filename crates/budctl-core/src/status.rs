//! What a session reports, in device-neutral terms. Front ends print these, and `--json`
//! serializes them as they are.

use serde::{Serialize, Serializer};

use crate::bt::BdAddr;

/// Bumped whenever the JSON shape changes incompatibly.
pub const JSON_SCHEMA: u32 = 1;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Status {
    pub battery: Battery,
    pub noise_control: Option<NoiseControl>,
    /// Name of the current EQ preset.
    pub eq: Option<String>,
    pub bass_level: Option<i8>,
    /// On/off settings, serialized as `{"game": false, …}`.
    #[serde(serialize_with = "switches_as_map")]
    pub switches: Vec<Switch>,
    /// Audio codec of the link the buds report.
    pub codec: Option<String>,
    pub firmware: Option<String>,
    /// Reads that failed. The rest of the status is still valid.
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Battery {
    pub left: Option<Charge>,
    pub right: Option<Charge>,
    pub case: Option<Charge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Charge {
    /// Percent.
    pub level: u8,
    pub charging: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoiseControl {
    /// `off`, `transparency` or `anc`, or `unknown (…)` for a value the device module cannot name.
    pub mode: String,
    /// The noise-cancelling level, when the device reports one.
    pub level: Option<String>,
}

/// A noise-control mode to set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseMode {
    Off,
    Transparency,
    Anc(AncLevel),
}

/// How strongly noise cancelling works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AncLevel {
    Light,
    Medium,
    Deep,
}

impl AncLevel {
    /// The name [`NoiseControl::level`] uses for it.
    pub fn name(self) -> &'static str {
        match self {
            AncLevel::Light => "light",
            AncLevel::Medium => "medium",
            AncLevel::Deep => "deep",
        }
    }
}

impl NoiseMode {
    /// The name [`NoiseControl::mode`] uses for it.
    pub fn name(self) -> &'static str {
        match self {
            NoiseMode::Off => "off",
            NoiseMode::Transparency => "transparency",
            NoiseMode::Anc(_) => "anc",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Switch {
    /// Stable name used on the command line and in JSON (`game`, `dual`…).
    pub key: &'static str,
    /// Human name (`Game mode`).
    pub label: &'static str,
    pub on: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct TouchControls {
    pub gestures: Vec<Gesture>,
    /// Noise-control modes a hold steps through, when the device has that setting.
    pub hold_cycle: Option<Vec<String>>,
    /// The actions each gesture can be given, when the device can change them.
    pub choices: Vec<GestureChoice>,
    /// The modes the hold cycle can include, when the device can change it.
    pub cycle_modes: Vec<String>,
    pub warnings: Vec<String>,
}

/// The actions one gesture can be given, as the device offers them, in its order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GestureChoice {
    /// `double-tap`, `triple-tap`, `hold`…
    pub gesture: String,
    /// `none`, `play-pause`…
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gesture {
    /// `left`, `right` or `both`.
    pub side: String,
    /// `double-tap`, `triple-tap`, `hold`…
    pub gesture: String,
    /// `play-pause`, `next`, `none`…
    pub action: String,
    /// Raw button group, for entries outside the main gestures (calls). `None` for the main group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<u8>,
}

/// The EQ presets the buds have, and which one is in use.
#[derive(Debug, Default, Clone, Serialize)]
pub struct EqPresets {
    /// Name of the preset in use, as in [`Status::eq`].
    pub current: Option<String>,
    /// Built-in presets first, then custom ones.
    pub presets: Vec<EqPreset>,
    /// Reads that failed. The rest is still valid.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EqPreset {
    /// Built-in presets: the name `eq <preset>` takes. Custom ones: the name stored on the buds.
    pub name: String,
    /// The buds' id for it.
    pub id: u8,
    pub custom: bool,
    /// Custom presets only: gain per band. Empty for built-in presets.
    pub bands: Vec<EqBand>,
    /// Custom presets only: the lowest and the highest gain a band takes, dB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<(i8, i8)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EqBand {
    pub hz: u16,
    /// dB.
    pub gain: i8,
}

/// A phone or computer the buds keep for dual connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DualDevice {
    pub address: BdAddr,
    /// The name the device gave the buds.
    pub name: String,
    pub connected: bool,
    /// The device that asked: the computer budctl runs on.
    pub this_computer: bool,
    /// The device the buds take audio from.
    pub main_audio: bool,
    pub playing: bool,
}

/// A touch-control change: what changes, the write that does it and, once sent, the table the
/// buds report afterwards.
#[derive(Debug, Serialize)]
pub struct TouchChange {
    /// The entries that change. Empty when every target already has the wanted action.
    pub changes: Vec<GestureChange>,
    /// The write that makes the change, or `None` when there is nothing to change.
    pub write: Option<RawWrite>,
    /// Whether the write went out. `false` on a dry run and when nothing changes.
    pub sent: bool,
    /// The whole table: as read back after the write, or as it would be after a dry run.
    pub gestures: Vec<Gesture>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GestureChange {
    pub side: String,
    pub gesture: String,
    pub from: String,
    pub to: String,
}

/// One command and its payload, as `--dry-run` shows them. The frame around it (sequence byte
/// and so on) is added when it is sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RawWrite {
    /// `0x0401`.
    #[serde(serialize_with = "command_as_hex")]
    pub command: u16,
    /// `06 01 01 02 01 …`.
    #[serde(serialize_with = "bytes_as_hex")]
    pub payload: Vec<u8>,
}

fn command_as_hex<S: Serializer>(command: &u16, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&format!("0x{command:04X}"))
}

fn bytes_as_hex<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&crate::debug::hex(bytes))
}

fn switches_as_map<S: Serializer>(switches: &[Switch], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_map(switches.iter().map(|s| (s.key, s.on)))
}
