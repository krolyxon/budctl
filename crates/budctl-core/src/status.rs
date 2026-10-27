//! What a session reports, in device-neutral terms. Front ends print these, and `--json`
//! serializes them as they are.

use serde::{Serialize, Serializer};

/// Bumped whenever the JSON shape changes incompatibly.
pub const JSON_SCHEMA: u32 = 1;

#[derive(Debug, Default, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Switch {
    /// Stable name used on the command line and in JSON (`game`, `dual`…).
    pub key: &'static str,
    /// Human name (`Game mode`).
    pub label: &'static str,
    pub on: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct TouchControls {
    pub gestures: Vec<Gesture>,
    /// Noise-control modes a hold steps through, when the device has that setting.
    pub hold_cycle: Option<Vec<String>>,
    pub warnings: Vec<String>,
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

fn switches_as_map<S: Serializer>(switches: &[Switch], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_map(switches.iter().map(|s| (s.key, s.on)))
}
