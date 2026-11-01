//! The JSON between the app's Kotlin side and the worker (docs/android.md#boundary).
//!
//! Commands come in as `{"command": "noise", "mode": "anc", "level": "deep"}`, and events go out
//! as `{"event": "noise", "mode": "anc", "level": "deep"}`. The values are the status types the
//! CLI's `--json` writes, except that switches keep their labels.

use budctl_core::status::{AncLevel, EqBand, NoiseMode, Status};
use budctl_core::worker::{Command, Event};
use budctl_core::{Error, Kind, PairedDevice, Result};
use serde::Deserialize;
use serde_json::{Map, Value, to_value};

use crate::phone::app_error;

/// A command, as the app sends it.
#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
enum Request {
    Read,
    /// `off`, `transparency` or `anc`; for `anc`, `light`, `medium` or `deep` (the default).
    Noise {
        mode: String,
        level: Option<String>,
    },
    Switch {
        key: String,
        on: bool,
    },
    Eq {
        preset: String,
    },
    CustomEq {
        preset: String,
        bands: Vec<Band>,
    },
    Touch {
        side: String,
        gesture: String,
        action: String,
    },
    HoldCycle {
        modes: Vec<String>,
    },
    Find {
        ring: bool,
    },
    Bass {
        level: i8,
    },
    CreateEq {
        name: String,
    },
    RenameEq {
        preset: String,
        name: String,
    },
    DeleteEq {
        preset: String,
    },
}

#[derive(Debug, Deserialize)]
struct Band {
    hz: u16,
    gain: i8,
}

/// The worker command for a JSON command from the app.
pub fn command(json: &str) -> Result<Command> {
    let request: Request =
        serde_json::from_str(json).map_err(|e| Error::Usage(format!("not a command budctl knows: {e}")))?;
    Ok(match request {
        Request::Read => Command::Read,
        Request::Noise { mode, level } => Command::Noise(noise_mode(&mode, level.as_deref())?),
        Request::Switch { key, on } => Command::Switch(key, on),
        Request::Eq { preset } => Command::Eq(preset),
        Request::CustomEq { preset, bands } => Command::CustomEq(
            preset,
            bands
                .into_iter()
                .map(|band| EqBand {
                    hz: band.hz,
                    gain: band.gain,
                })
                .collect(),
        ),
        Request::Touch { side, gesture, action } => Command::Touch { side, gesture, action },
        Request::HoldCycle { modes } => Command::HoldCycle(modes),
        Request::Find { ring } => Command::Find(ring),
        Request::Bass { level } => Command::Bass(level),
        Request::CreateEq { name } => Command::CreateEq(name),
        Request::RenameEq { preset, name } => Command::RenameEq(preset, name),
        Request::DeleteEq { preset } => Command::DeleteEq(preset),
    })
}

/// `anc` with no level is the deepest, as `budctl anc on` is.
fn noise_mode(mode: &str, level: Option<&str>) -> Result<NoiseMode> {
    match (mode, level) {
        ("off", None) => Ok(NoiseMode::Off),
        ("transparency", None) => Ok(NoiseMode::Transparency),
        ("anc", None | Some("deep")) => Ok(NoiseMode::Anc(AncLevel::Deep)),
        ("anc", Some("medium")) => Ok(NoiseMode::Anc(AncLevel::Medium)),
        ("anc", Some("light")) => Ok(NoiseMode::Anc(AncLevel::Light)),
        _ => Err(Error::Usage(format!(
            "no noise-control mode {mode:?} with level {level:?}"
        ))),
    }
}

/// A worker event, as the app reads it.
pub fn event(event: &Event) -> serde_json::Result<String> {
    let (name, fields): (&str, Vec<(&str, Value)>) = match event {
        Event::Found(name) => ("found", vec![("name", to_value(name)?)]),
        Event::Read(reading) => (
            "read",
            vec![
                ("status", status(&reading.status)?),
                ("eq", to_value(&reading.eq)?),
                ("touch", to_value(&reading.touch)?),
                ("bass_range", to_value(reading.bass_range)?),
                ("dual", to_value(&reading.dual)?),
            ],
        ),
        Event::Noise(now) => (
            "noise",
            vec![("mode", to_value(&now.mode)?), ("level", to_value(&now.level)?)],
        ),
        Event::Switch(switch) => ("switch", vec![("switch", to_value(switch)?)]),
        Event::Eq(preset) => ("eq", vec![("preset", to_value(preset)?)]),
        Event::CustomEq(preset) => ("custom-eq", vec![("preset", to_value(preset)?)]),
        Event::Touch(gestures) => ("touch", vec![("gestures", to_value(gestures)?)]),
        Event::HoldCycle(modes) => ("hold-cycle", vec![("modes", to_value(modes)?)]),
        Event::Ringing(ring) => ("ringing", vec![("ring", Value::from(*ring))]),
        Event::Bass(level) => ("bass", vec![("level", Value::from(*level))]),
        Event::Presets(eq) => ("presets", vec![("eq", to_value(eq)?)]),
        Event::Failed { kind, message } => (
            "failed",
            vec![("kind", Value::from(kind_name(*kind))), ("message", to_value(message)?)],
        ),
        Event::Lost(message) => ("lost", vec![("message", to_value(message)?)]),
    };
    let mut object = Map::new();
    object.insert("event".to_owned(), Value::from(name));
    for (key, value) in fields {
        object.insert(key.to_owned(), value);
    }
    serde_json::to_string(&object)
}

/// The status as `--json` writes it, except the switches: `--json` writes them as
/// `{"game": false}`, and the app needs their labels too.
fn status(status: &Status) -> serde_json::Result<Value> {
    let mut value = to_value(status)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("switches".to_owned(), to_value(&status.switches)?);
    }
    Ok(value)
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Usage => "usage",
        Kind::NotFound => "not-found",
        Kind::Busy => "busy",
        Kind::Failed => "failed",
    }
}

/// A paired device, as the app lists it.
#[derive(Debug, Deserialize)]
struct Paired {
    address: String,
    name: String,
    alias: String,
    connected: bool,
    /// Service UUIDs, lowercase and hyphenated, as Android cached them from the buds' SDP record.
    uuids: Vec<String>,
}

/// The paired devices, from the app's list.
pub fn paired_devices(json: &str) -> Result<Vec<PairedDevice>> {
    let list: Vec<Paired> = serde_json::from_str(json).map_err(|e| app_error("reading the app's device list", e))?;
    list.into_iter()
        .map(|paired| {
            Ok(PairedDevice {
                address: paired
                    .address
                    .parse()
                    .map_err(|e| app_error("reading the app's device list", e))?,
                name: paired.name,
                alias: paired.alias,
                connected: paired.connected,
                uuids: paired.uuids,
            })
        })
        .collect()
}

/// Why the app could not open the link.
#[derive(Debug, Deserialize)]
struct Failure {
    /// `busy`, `not-connected`, `timeout` or `failed`.
    kind: String,
    message: String,
}

/// The error for the app's account of a failed connect (docs/android.md#bluetooth).
pub fn connect_failure(json: &str) -> Error {
    const CONTEXT: &str = "connecting to the control channel";
    match serde_json::from_str::<Failure>(json) {
        Ok(failure) => match failure.kind.as_str() {
            "busy" => Error::ChannelBusy(failure.message),
            "not-connected" => Error::NotConnected(failure.message),
            "timeout" => Error::Timeout(failure.message),
            _ => app_error(CONTEXT, failure.message),
        },
        Err(e) => app_error(CONTEXT, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use budctl_core::status::{NoiseControl, Switch};
    use budctl_core::worker::Reading;

    #[test]
    fn commands_parse_into_worker_commands() {
        assert!(matches!(command(r#"{"command": "read"}"#), Ok(Command::Read)));
        assert!(matches!(
            command(r#"{"command": "noise", "mode": "anc"}"#),
            Ok(Command::Noise(NoiseMode::Anc(AncLevel::Deep)))
        ));
        assert!(matches!(
            command(r#"{"command": "noise", "mode": "anc", "level": "light"}"#),
            Ok(Command::Noise(NoiseMode::Anc(AncLevel::Light)))
        ));
        assert!(matches!(
            command(r#"{"command": "switch", "key": "game", "on": true}"#),
            Ok(Command::Switch(key, true)) if key == "game"
        ));
        let Ok(Command::CustomEq(preset, bands)) =
            command(r#"{"command": "custom-eq", "preset": "custom-4", "bands": [{"hz": 50, "gain": -3}]}"#)
        else {
            panic!("a custom EQ command expected");
        };
        assert_eq!(preset, "custom-4");
        assert_eq!(bands, [EqBand { hz: 50, gain: -3 }]);
        assert!(matches!(
            command(r#"{"command": "hold-cycle", "modes": ["off", "anc"]}"#),
            Ok(Command::HoldCycle(modes)) if modes == ["off", "anc"]
        ));
    }

    #[test]
    fn bad_commands_are_usage_errors() {
        for json in [
            r#"{"command": "reboot"}"#,
            r#"{"command": "noise", "mode": "loud"}"#,
            r#"{"command": "noise", "mode": "off", "level": "deep"}"#,
            r#"{"command": "bass", "level": 300}"#,
            "not json",
        ] {
            assert_eq!(command(json).err().map(|e| e.kind()), Some(Kind::Usage), "{json}");
        }
    }

    fn parse(event: &Event) -> Value {
        serde_json::from_str(&super::event(event).unwrap()).unwrap()
    }

    #[test]
    fn events_are_tagged_objects() {
        let noise = parse(&Event::Noise(NoiseControl {
            mode: "anc".to_owned(),
            level: Some("deep".to_owned()),
        }));
        assert_eq!(
            noise,
            serde_json::json!({"event": "noise", "mode": "anc", "level": "deep"})
        );

        let failed = parse(&Event::Failed {
            kind: Kind::Busy,
            message: "control channel busy".to_owned(),
        });
        assert_eq!(
            failed,
            serde_json::json!({"event": "failed", "kind": "busy", "message": "control channel busy"})
        );
        assert_eq!(
            parse(&Event::Ringing(true)),
            serde_json::json!({"event": "ringing", "ring": true})
        );
    }

    #[test]
    fn a_reading_keeps_the_switch_labels() {
        let game = Switch {
            key: "game",
            label: "Game mode",
            on: true,
        };
        let reading = Reading {
            status: Status {
                switches: vec![game],
                ..Status::default()
            },
            eq: None,
            touch: None,
            bass_range: Some((-5, 5)),
            dual: None,
        };
        let read = parse(&Event::Read(Box::new(reading)));
        assert_eq!(read["event"], "read");
        assert_eq!(
            read["status"]["switches"],
            serde_json::json!([{"key": "game", "label": "Game mode", "on": true}])
        );
        assert_eq!(read["bass_range"], serde_json::json!([-5, 5]));
        assert_eq!(read["eq"], Value::Null);
    }

    #[test]
    fn paired_devices_come_from_the_app_list() {
        let list = paired_devices(
            r#"[{"address": "00:11:22:aa:bb:cc", "name": "realme Buds T310", "alias": "Buds",
                 "connected": true, "uuids": ["0000079a-d102-11e1-9b23-00025b00a5a5"]}]"#,
        )
        .unwrap();
        let [buds] = list.as_slice() else {
            panic!("one device expected");
        };
        assert_eq!(buds.address.to_string(), "00:11:22:AA:BB:CC");
        assert_eq!(buds.alias, "Buds");
        assert!(buds.connected);

        assert!(
            paired_devices(r#"[{"address": "nope", "name": "", "alias": "", "connected": false, "uuids": []}]"#)
                .is_err()
        );
        assert!(paired_devices("{}").is_err());
    }

    #[test]
    fn connect_failures_keep_their_kind() {
        let kind = |json: &str| connect_failure(json).kind();
        assert_eq!(kind(r#"{"kind": "busy", "message": "refused"}"#), Kind::Busy);
        assert_eq!(kind(r#"{"kind": "not-connected", "message": "off"}"#), Kind::NotFound);
        assert_eq!(kind(r#"{"kind": "timeout", "message": "8 s"}"#), Kind::Failed);
        assert_eq!(kind(r#"{"kind": "failed", "message": "?"}"#), Kind::NotFound);
        assert_eq!(kind("garbage"), Kind::NotFound);
        assert!(
            connect_failure(r#"{"kind": "busy", "message": "refused"}"#)
                .to_string()
                .contains("is another app connected?")
        );
    }
}
