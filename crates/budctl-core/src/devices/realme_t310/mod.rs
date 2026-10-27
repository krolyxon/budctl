//! realme Buds T310. Spec: docs/protocol/realme_t310.md
//!
//! `frame.rs` and `link.rs` hold the protocol family's framing and request/reply logic;
//! `codec.rs` holds everything specific to the T310.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::bluez::PairedDevice;
use crate::bt::Uuid;
use crate::devices::{Device, Session};
use crate::error::{Error, Kind, Result};
use crate::status::{
    EqBand, EqPreset, EqPresets, GestureChange, NoiseControl, NoiseMode, RawWrite, Status, Switch, TouchChange,
    TouchControls,
};
use crate::transport::Transport;

mod codec;
mod frame;
mod link;

use codec::Commands;
use link::Link;

/// When to read a setting back after the buds ack a write. Spec: realme_t310.md#write-read-back
#[derive(Clone, Copy)]
struct ReadBack {
    /// How long after the ack the first read goes out.
    first: Duration,
    /// How long after the ack budctl gives up.
    window: Duration,
}

/// Spec: realme_t310.md#write-read-back
const READ_BACK: ReadBack = ReadBack {
    first: Duration::ZERO,
    window: Duration::from_secs(2),
};
/// Game mode lands about 1 s after the ack. Spec: realme_t310.md#switch-write
const GAME_READ_BACK: ReadBack = ReadBack {
    first: Duration::from_secs(1),
    window: Duration::from_secs(3),
};
/// The gesture table is read back about 600 ms after the ack. Spec: realme_t310.md#gesture-write
const TOUCH_READ_BACK: ReadBack = ReadBack {
    first: Duration::from_millis(600),
    window: Duration::from_secs(2),
};
/// Spec: realme_t310.md#write-read-back
const READ_BACK_PAUSE: Duration = Duration::from_millis(300);

/// The switches budctl may write, each added after the previous one passed its hardware
/// checkpoint (CLAUDE.md rule 4). Spec: realme_t310.md#switch-write
const WRITABLE_SWITCHES: [(&str, ReadBack); 5] = [
    ("game", GAME_READ_BACK),
    ("bass-boost", READ_BACK),
    ("spatial", READ_BACK),
    ("wind", READ_BACK),
    ("dual", READ_BACK),
];

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// Spec: realme_t310.md#identity
const BLUETOOTH_NAME: &str = "realme Buds T310";

/// Control services, in the order to try. Spec: realme_t310.md#transport
const SERVICES: [Uuid; 2] = [
    Uuid::from_u128(0x0000_079A_D102_11E1_9B23_0002_5B00_A5A5),
    Uuid::from_u128(0x0000_1107_D102_11E1_9B23_0002_5B00_A5A5),
];

/// Quiet time after connecting, before the first request. Spec: realme_t310.md#init
const SETTLE: Duration = Duration::from_millis(300);

pub struct RealmeT310;

impl Device for RealmeT310 {
    fn name(&self) -> &'static str {
        BLUETOOTH_NAME
    }

    fn matches(&self, device: &PairedDevice) -> bool {
        device.name == BLUETOOTH_NAME
    }

    fn rfcomm_uuids(&self) -> &'static [Uuid] {
        &SERVICES
    }

    fn open(&self, transport: Box<dyn Transport>) -> Result<Box<dyn Session>> {
        Ok(Box::new(T310Session::open(Link::new(transport, unix_time))?))
    }
}

fn unix_time() -> u32 {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    u32::try_from(seconds).unwrap_or(u32::MAX)
}

struct T310Session {
    link: Link,
    commands: Commands,
}

/// Collects what a series of best-effort reads produced.
#[derive(Default)]
struct Reads {
    warnings: Vec<String>,
    answered: usize,
}

impl T310Session {
    /// The session start. Spec: realme_t310.md#init
    fn open(mut link: Link) -> Result<Self> {
        link.pump_until(Instant::now() + SETTLE)?;

        let bitmap = link.request(codec::HANDSHAKE, &[]).map_err(|error| match error {
            // Provisional until checkpoint HW1: a link that connects but stays silent is most
            // likely held by another app. Spec: realme_t310.md#open
            Error::Timeout(_) => Error::ChannelBusy("connected, but the buds did not answer the handshake".to_owned()),
            other => other,
        })?;
        let commands = Commands::parse(&bitmap)?;

        let product = codec::decode_product_id(&link.request(codec::PRODUCT_ID, &[])?)?;
        if product != codec::T310_PRODUCT_ID {
            return Err(Error::WrongModel {
                expected: BLUETOOTH_NAME,
                found: format!("{product:06X}"),
            });
        }

        let offered = codec::decode_events_offered(&link.request(codec::EVENTS_OFFERED, &[])?)?;
        let subscription = codec::subscribe_payload(&offered, &commands);
        if commands.allows(codec::SUBSCRIBE) && subscription.len() > 1 {
            // Some buds never acknowledge this, so budctl does not wait. Spec: realme_t310.md#subscribe
            link.send(codec::SUBSCRIBE, &subscription)?;
        }
        Ok(Self { link, commands })
    }

    /// Read and decode one value, if the buds offer the command. A read the buds answer badly
    /// (timeout, refusal, garbage) becomes a warning so the rest still shows; a lost link stops
    /// everything.
    fn read<T>(
        &mut self,
        reads: &mut Reads,
        what: &str,
        cmd: u16,
        payload: &[u8],
        decode: fn(&[u8]) -> Result<T>,
    ) -> Result<Option<T>> {
        if !self.commands.allows(cmd) {
            return Ok(None);
        }
        match self.link.request(cmd, payload).and_then(|reply| decode(&reply)) {
            Ok(value) => {
                reads.answered += 1;
                Ok(Some(value))
            }
            Err(error) if error.kind() == Kind::Failed => {
                reads.warnings.push(format!("{what}: {error}"));
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

impl T310Session {
    /// After an acked write, read the setting back with `cmd query` until `check` reports the
    /// new value, following `timing`. `check` returns whether the value is the one written,
    /// and the value; `not_applied` describes a value that never changed.
    /// Spec: realme_t310.md#write-read-back
    fn read_back<T>(
        &mut self,
        timing: ReadBack,
        cmd: u16,
        query: &[u8],
        check: impl Fn(&[u8]) -> Result<(bool, T)>,
        not_applied: impl Fn(T) -> String,
    ) -> Result<T> {
        let acked = Instant::now();
        self.link.pump_until(acked + timing.first)?;
        loop {
            let (applied, value) = check(&self.link.request(cmd, query)?)?;
            if applied {
                return Ok(value);
            }
            if Instant::now() >= acked + timing.window {
                return Err(Error::NotApplied(not_applied(value)));
            }
            self.link.pump_until(Instant::now() + READ_BACK_PAUSE)?;
        }
    }
}

impl Session for T310Session {
    /// Spec: realme_t310.md#buds-requests
    fn serve(&mut self) -> Result<()> {
        self.link.pump_until(Instant::now())
    }

    fn status(&mut self) -> Result<Status> {
        let mut reads = Reads::default();
        let switches = self.read(
            &mut reads,
            "switches",
            codec::SWITCHES,
            &codec::SWITCH_QUERY,
            codec::decode_switches,
        )?;
        let noise_control = self.read(
            &mut reads,
            "noise control",
            codec::NOISE_CONTROL,
            &codec::NOISE_MODE_QUERY,
            codec::decode_noise_mode,
        )?;
        let battery = self.read(&mut reads, "battery", codec::BATTERY, &[], codec::decode_battery)?;
        let eq = self.read(&mut reads, "EQ", codec::EQ_PRESET, &[], codec::decode_eq)?;
        // No bass-level read: the T310 answers 0x0124 neither with bass boost off (HW1) nor on
        // (W6a). Spec: realme_t310.md#bass-read
        let codec = self.read(&mut reads, "codec", codec::CODEC, &[], codec::decode_codec)?;
        let firmware = self.read(&mut reads, "firmware", codec::FIRMWARE, &[], codec::decode_firmware)?;

        if reads.answered == 0 && !reads.warnings.is_empty() {
            return Err(Error::Timeout(reads.warnings.join("; ")));
        }
        Ok(Status {
            battery: battery.unwrap_or_default(),
            noise_control,
            eq,
            bass_level: None,
            switches: switches.unwrap_or_default(),
            codec,
            firmware,
            warnings: reads.warnings,
        })
    }

    /// Spec: realme_t310.md#anc-write, #write-read-back
    fn set_noise_control(&mut self, mode: NoiseMode) -> Result<NoiseControl> {
        if !self.commands.allows(codec::NOISE_CONTROL_WRITE) || !self.commands.allows(codec::NOISE_CONTROL) {
            return Err(Error::Unsupported(
                "these buds do not offer noise-control settings".to_owned(),
            ));
        }
        let reply = self
            .link
            .request(codec::NOISE_CONTROL_WRITE, &codec::noise_write_payload(mode))?;
        codec::check_write_reply(&reply, "the noise-control change")?;

        let wanted = codec::noise_write_mask(mode);
        self.read_back(
            READ_BACK,
            codec::NOISE_CONTROL,
            &codec::NOISE_MODE_QUERY,
            |reply| {
                Ok((
                    codec::decode_noise_mask(reply)? == wanted,
                    codec::decode_noise_mode(reply)?,
                ))
            },
            |now| {
                let wanted = match mode {
                    NoiseMode::Anc(level) => format!("anc ({})", level.name()),
                    _ => mode.name().to_owned(),
                };
                let reported = match &now.level {
                    Some(level) => format!("{} ({level})", now.mode),
                    None => now.mode,
                };
                format!("the buds accepted noise control {wanted}, but still report {reported}")
            },
        )
    }

    /// Spec: realme_t310.md#eq-read, #custom-eq
    fn eq_presets(&mut self) -> Result<EqPresets> {
        let mut reads = Reads::default();
        let current = self.read(&mut reads, "EQ", codec::EQ_PRESET, &[], codec::decode_eq)?;
        let custom = self.read(
            &mut reads,
            "custom EQ list",
            codec::CUSTOM_EQ_LIST,
            &[],
            codec::decode_custom_eqs,
        )?;
        let mut presets = codec::builtin_eqs();
        presets.extend(custom.into_iter().flatten());
        Ok(EqPresets {
            current,
            presets,
            warnings: reads.warnings,
        })
    }

    /// Built-in presets only; custom EQs come with W11. Spec: realme_t310.md#eq-write
    fn set_eq(&mut self, preset: &str) -> Result<String> {
        if codec::custom_eq_id(preset).is_some() {
            return self.set_custom_eq(preset, &[]).map(|now| format!("custom-{}", now.id));
        }
        let Some(id) = codec::eq_id(preset) else {
            return Err(Error::Usage(format!(
                "no EQ preset called {preset}; these buds have: {}, and the custom-<id> ones `budctl eq` lists",
                codec::eq_names().join(", ")
            )));
        };
        if !self.commands.allows(codec::EQ_SELECT) || !self.commands.allows(codec::EQ_PRESET) {
            return Err(Error::Unsupported("these buds do not offer EQ presets".to_owned()));
        }
        let reply = self.link.request(codec::EQ_SELECT, &[id])?;
        codec::check_write_reply(&reply, "the EQ change")?;

        self.read_back(
            READ_BACK,
            codec::EQ_PRESET,
            &[],
            |reply| {
                let now = codec::decode_eq(reply)?;
                Ok((now == preset, now))
            },
            |now| format!("the buds accepted EQ {preset}, but still report {now}"),
        )
    }

    /// Spec: realme_t310.md#custom-eq, #write-read-back
    fn set_custom_eq(&mut self, preset: &str, bands: &[EqBand]) -> Result<EqPreset> {
        let Some(id) = codec::custom_eq_id(preset) else {
            return Err(Error::Usage(format!(
                "{preset} is not a custom preset; those are named custom-<id>, as `budctl eq` lists them"
            )));
        };
        if !self.commands.allows(codec::CUSTOM_EQ_WRITE) || !self.commands.allows(codec::CUSTOM_EQ_LIST) {
            return Err(Error::Unsupported(
                "these buds do not offer custom EQ presets".to_owned(),
            ));
        }
        // A save carries the whole preset, so it starts from the list read just now.
        let list = codec::decode_custom_eq_list(&self.link.request(codec::CUSTOM_EQ_LIST, &[])?)?;
        let Some(current) = list.iter().find(|p| p.id == id) else {
            let have: Vec<String> = list.iter().map(|p| format!("custom-{}", p.id)).collect();
            return Err(Error::Usage(format!(
                "these buds have no {preset}; their custom presets: {}",
                if have.is_empty() {
                    "none".to_owned()
                } else {
                    have.join(", ")
                }
            )));
        };
        let wanted = codec::retune(current, bands)?;
        let reply = self
            .link
            .request(codec::CUSTOM_EQ_WRITE, &codec::custom_eq_save_payload(&wanted)?)?;
        codec::check_write_reply(&reply, "saving the custom EQ")?;

        let now = self.read_back(
            READ_BACK,
            codec::CUSTOM_EQ_LIST,
            &[],
            |reply| {
                let now = codec::decode_custom_eq_list(reply)?.into_iter().find(|p| p.id == id);
                let applied = now.as_ref().is_some_and(|p| p.bands == wanted.bands && p.selected);
                Ok((applied, now))
            },
            |now| match now {
                None => format!("the buds accepted {preset}, but their list no longer has it"),
                Some(p) if p.bands == wanted.bands => format!("the buds saved {preset}, but did not select it"),
                Some(_) => format!("the buds accepted {preset}, but still report the old bands"),
            },
        )?;
        now.map(|p| p.preset())
            .ok_or_else(|| Error::Protocol(format!("{preset} vanished from the list")))
    }

    /// Ack-only: the T310 never answers the bass-level read. Spec: realme_t310.md#bass-write
    /// Spec: realme_t310.md#custom-eq-create, #write-read-back
    fn create_custom_eq(&mut self, name: &str) -> Result<EqPreset> {
        if !self.commands.allows(codec::CUSTOM_EQ_WRITE) || !self.commands.allows(codec::CUSTOM_EQ_LIST) {
            return Err(Error::Unsupported(
                "these buds do not offer custom EQ presets".to_owned(),
            ));
        }
        let list = codec::decode_custom_eq_list(&self.link.request(codec::CUSTOM_EQ_LIST, &[])?)?;
        if list.len() >= codec::CUSTOM_EQ_MAX {
            return Err(Error::Usage(format!(
                "these buds keep at most {} custom presets, and have {}; change one of them instead",
                codec::CUSTOM_EQ_MAX,
                list.len()
            )));
        }
        if list.iter().any(|p| p.name == name.as_bytes()) {
            return Err(Error::Usage(format!("a custom preset is called {name} already")));
        }
        // The new preset copies the bands the buds use: the vendor's defaults do not hold here.
        let Some(model) = list.first() else {
            return Err(Error::Usage(
                "budctl copies a new preset's bands from one the buds have, and these buds have none".to_owned(),
            ));
        };
        let new = codec::new_custom_eq(name, model)?;
        let reply = self
            .link
            .request(codec::CUSTOM_EQ_WRITE, &codec::custom_eq_create_payload(&new)?)?;
        let id = codec::decode_custom_eq_write_reply(&reply)?;
        let created = self.read_back(
            READ_BACK,
            codec::CUSTOM_EQ_LIST,
            &[],
            |reply| {
                let now = codec::decode_custom_eq_list(reply)?;
                let created = codec::find_created(&now, &new.name, id).cloned();
                Ok((created.is_some(), created))
            },
            |_| format!("the buds accepted the new preset {name}, but their list does not have it"),
        )?;
        created
            .map(|p| p.preset())
            .ok_or_else(|| Error::Protocol(format!("the new preset {name} vanished from the list")))
    }

    /// Spec: realme_t310.md#custom-eq-rename, #write-read-back
    fn rename_custom_eq(&mut self, preset: &str, name: &str) -> Result<EqPreset> {
        let Some(id) = codec::custom_eq_id(preset) else {
            return Err(Error::Usage(format!(
                "{preset} is not a custom preset; those are named custom-<id>, as `budctl eq` lists them"
            )));
        };
        if !self.commands.allows(codec::CUSTOM_EQ_WRITE) || !self.commands.allows(codec::CUSTOM_EQ_LIST) {
            return Err(Error::Unsupported(
                "these buds do not offer custom EQ presets".to_owned(),
            ));
        }
        // A save carries the whole preset, so it starts from the list read just now.
        let list = codec::decode_custom_eq_list(&self.link.request(codec::CUSTOM_EQ_LIST, &[])?)?;
        let Some(current) = list.iter().find(|p| p.id == id) else {
            return Err(Error::Usage(format!("these buds have no {preset}")));
        };
        if list.iter().any(|p| p.id != id && p.name == name.as_bytes()) {
            return Err(Error::Usage(format!("a custom preset is called {name} already")));
        }
        let wanted = codec::renamed(current, name)?;
        let reply = self
            .link
            .request(codec::CUSTOM_EQ_WRITE, &codec::custom_eq_save_payload(&wanted)?)?;
        codec::check_write_reply(&reply, "renaming the custom EQ")?;
        let now = self.read_back(
            READ_BACK,
            codec::CUSTOM_EQ_LIST,
            &[],
            |reply| {
                let now = codec::decode_custom_eq_list(reply)?.into_iter().find(|p| p.id == id);
                let applied = now.as_ref().is_some_and(|p| p.name == wanted.name);
                Ok((applied, now))
            },
            |now| match now {
                None => format!("the buds accepted the new name, but their list no longer has {preset}"),
                Some(_) => format!("the buds accepted the new name, but still list {preset} under the old one"),
            },
        )?;
        now.map(|p| p.preset())
            .ok_or_else(|| Error::Protocol(format!("{preset} vanished from the list")))
    }

    /// Spec: realme_t310.md#bass-write
    fn bass_range(&self) -> Option<(i8, i8)> {
        self.commands
            .allows(codec::BASS_LEVEL_WRITE)
            .then_some(codec::BASS_RANGE)
    }

    fn set_bass_level(&mut self, level: i8) -> Result<()> {
        let payload = codec::bass_write_payload(level)?;
        if !self.commands.allows(codec::BASS_LEVEL_WRITE) {
            return Err(Error::Unsupported("these buds do not offer a bass level".to_owned()));
        }
        let reply = self.link.request(codec::BASS_LEVEL_WRITE, &payload)?;
        codec::check_write_reply(&reply, "the bass-level change")
    }

    /// Spec: realme_t310.md#find
    fn find(&mut self, ring: bool) -> Result<()> {
        if !self.commands.allows(codec::FIND) {
            return Err(Error::Unsupported("these buds cannot ring through budctl".to_owned()));
        }
        let reply = self.link.request(codec::FIND, &codec::find_payload(ring))?;
        codec::check_write_reply(&reply, if ring { "ringing" } else { "stopping the ringing" })
    }

    /// Spec: realme_t310.md#switch-write, #write-read-back
    fn set_switch(&mut self, key: &str, on: bool) -> Result<Switch> {
        let Some(id) = codec::switch_id(key) else {
            return Err(Error::Unsupported(format!("these buds have no switch called {key}")));
        };
        let Some(&(_, timing)) = WRITABLE_SWITCHES.iter().find(|(k, _)| *k == key) else {
            return Err(Error::Unsupported(format!("changing {key} is not supported yet")));
        };
        if !self.commands.allows(codec::SWITCH_WRITE) || !self.commands.allows(codec::SWITCHES) {
            return Err(Error::Unsupported("these buds do not offer switch settings".to_owned()));
        }
        let reply = self
            .link
            .request(codec::SWITCH_WRITE, &codec::switch_write_payload(id, on))?;
        codec::check_write_reply(&reply, "the switch change")?;

        self.read_back(
            timing,
            codec::SWITCHES,
            &codec::SWITCH_QUERY,
            |reply| {
                let switch = codec::decode_switches(reply)?
                    .into_iter()
                    .find(|s| s.key == key)
                    .ok_or_else(|| Error::Protocol(format!("the switch reply has no {key}")))?;
                Ok((switch.on == on, switch))
            },
            |now| {
                format!(
                    "the buds accepted {} {}, but still report {}",
                    now.label,
                    on_off(on),
                    on_off(now.on)
                )
            },
        )
    }

    fn touch_controls(&mut self) -> Result<TouchControls> {
        if !self.commands.allows(codec::GESTURES) {
            return Err(Error::Unsupported(
                "these buds do not offer touch-control settings".to_owned(),
            ));
        }
        let gestures = codec::decode_gestures(&self.link.request(codec::GESTURES, &[])?)?;
        let mut reads = Reads::default();
        let hold_cycle = self.read(
            &mut reads,
            "hold cycle",
            codec::NOISE_CONTROL,
            &codec::HOLD_CYCLE_QUERY,
            codec::decode_hold_cycle,
        )?;
        // The choices only when the buds take writes: the GUI offers what it shows here.
        let writable = |cmd| self.commands.allows(cmd);
        let choices = if writable(codec::GESTURE_WRITE) {
            codec::touch_choices()
        } else {
            Vec::new()
        };
        let cycle_modes = if hold_cycle.is_some() && writable(codec::NOISE_CONTROL_WRITE) {
            codec::cycle_modes()
        } else {
            Vec::new()
        };
        Ok(TouchControls {
            gestures,
            hold_cycle,
            choices,
            cycle_modes,
            warnings: reads.warnings,
        })
    }

    /// Spec: realme_t310.md#hold-cycle-write, #write-read-back
    fn set_hold_cycle(&mut self, modes: &[&str]) -> Result<Vec<String>> {
        let mask = codec::hold_cycle_mask(modes)?;
        if !self.commands.allows(codec::NOISE_CONTROL_WRITE) || !self.commands.allows(codec::NOISE_CONTROL) {
            return Err(Error::Unsupported(
                "these buds do not offer noise-control settings".to_owned(),
            ));
        }
        let reply = self
            .link
            .request(codec::NOISE_CONTROL_WRITE, &codec::hold_cycle_write_payload(mask))?;
        codec::check_write_reply(&reply, "the hold-cycle change")?;

        self.read_back(
            READ_BACK,
            codec::NOISE_CONTROL,
            &codec::HOLD_CYCLE_QUERY,
            |reply| {
                Ok((
                    codec::decode_hold_cycle_mask(reply)? == mask,
                    codec::decode_hold_cycle(reply)?,
                ))
            },
            |now| {
                format!(
                    "the buds accepted the hold cycle {}, but still report {}",
                    modes.join(" → "),
                    now.join(" → ")
                )
            },
        )
    }

    /// Spec: realme_t310.md#gesture-write, #gesture-choices
    fn set_touch(&mut self, side: &str, gesture: &str, action: &str, dry_run: bool) -> Result<TouchChange> {
        let target = codec::touch_target(side, gesture, action)?;
        if !self.commands.allows(codec::GESTURE_WRITE) || !self.commands.allows(codec::GESTURES) {
            return Err(Error::Unsupported(
                "these buds do not offer touch-control settings".to_owned(),
            ));
        }
        // The write carries the whole table, so it starts from the table read just now.
        let before = codec::decode_gesture_entries(&self.link.request(codec::GESTURES, &[])?)?;
        let after = codec::retarget_gestures(&before, &target)?;
        let changes: Vec<GestureChange> = before
            .iter()
            .zip(&after)
            .filter(|(old, new)| old != new)
            .map(|(old, new)| {
                let (old, new) = (codec::name_gesture(old), codec::name_gesture(new));
                GestureChange {
                    side: new.side,
                    gesture: new.gesture,
                    from: old.action,
                    to: new.action,
                }
            })
            .collect();
        let planned: Vec<_> = after.iter().map(codec::name_gesture).collect();
        if changes.is_empty() {
            return Ok(TouchChange {
                changes,
                write: None,
                sent: false,
                gestures: planned,
            });
        }
        let write = RawWrite {
            command: codec::GESTURE_WRITE,
            payload: codec::gesture_write_payload(&after)?,
        };
        if dry_run {
            return Ok(TouchChange {
                changes,
                write: Some(write),
                sent: false,
                gestures: planned,
            });
        }

        let reply = self.link.request(write.command, &write.payload)?;
        codec::check_write_reply(&reply, "the touch-control change")?;
        let gestures = self.read_back(
            TOUCH_READ_BACK,
            codec::GESTURES,
            &[],
            |reply| {
                let now = codec::decode_gesture_entries(reply)?;
                // Applied when retargeting the table read back would change nothing.
                let applied = codec::retarget_gestures(&now, &target).is_ok_and(|again| again == now);
                Ok((applied, now))
            },
            |now| {
                let still: Vec<String> = changes
                    .iter()
                    .filter_map(|change| {
                        now.iter()
                            .map(codec::name_gesture)
                            .find(|g| g.group.is_none() && g.side == change.side && g.gesture == change.gesture)
                    })
                    .filter(|g| g.action != action)
                    .map(|g| format!("{} {} {}", g.side, g.gesture, g.action))
                    .collect();
                if still.is_empty() {
                    format!("the buds accepted {side} {gesture} {action}, but the table read back does not show it")
                } else {
                    format!(
                        "the buds accepted {side} {gesture} {action}, but still report {}",
                        still.join(", ")
                    )
                }
            },
        )?;
        Ok(TouchChange {
            changes,
            write: Some(write),
            sent: true,
            gestures: gestures.iter().map(codec::name_gesture).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{AncLevel, Charge};
    use crate::transport::mock::{Script, Step, hex};

    fn tx(cmd: u16, seq: u8, payload: &str) -> Step {
        Step::Tx(frame::encode(cmd, seq, &hex(payload)).unwrap())
    }

    fn rx(cmd: u16, seq: u8, payload: &str) -> Step {
        Step::Rx(frame::encode(cmd, seq, &hex(payload)).unwrap())
    }

    fn fixed_clock() -> u32 {
        1_750_000_000
    }

    fn open(script: &Script) -> Result<T310Session> {
        T310Session::open(Link::new(script.transport(), fixed_clock))
    }

    /// The handshake of the owner's T310 (HW1); seq 01-04 are used up.
    fn handshake() -> Vec<Step> {
        vec![
            tx(0x0100, 0x01, ""),
            rx(0x8100, 0x01, codec::T310_BITMAP_REPLY),
            tx(0x0103, 0x02, ""),
            rx(0x8103, 0x02, "00 12 3C 06"),
            tx(0x0200, 0x03, ""),
            rx(0x8200, 0x03, "00 09 01 02 03 04 05 06 0D F1 F4"),
            tx(0x0205, 0x04, "04 01 02 03 04"),
            rx(0x8205, 0x04, "00 04 01 00 02 00 03 00 04 00"),
        ]
    }

    #[test]
    fn serve_answers_the_buds_at_once() {
        let mut steps = handshake();
        // The time request, at Unix time 1750000000. Spec: realme_t310.md#buds-requests
        steps.extend([rx(0x0500, 0x02, ""), tx(0x8500, 0x02, "00 80 E1 4E 68")]);
        let script = Script::new(steps);
        let mut session = open(&script).unwrap();
        session.serve().unwrap();
        script.assert_done();

        let mut steps = handshake();
        steps.push(Step::Close);
        let script = Script::new(steps);
        let mut session = open(&script).unwrap();
        assert!(matches!(session.serve(), Err(Error::NotConnected(_))));
    }

    #[test]
    fn full_status() {
        let mut steps = handshake();
        let mut firmware = String::from("00 03");
        for byte in b"1,2,120,2,2,120,3,2,105" {
            firmware.push_str(&format!(" {byte:02X}"));
        }
        steps.extend([
            tx(0x010D, 0x05, "06 05 06 11 1A 1B 1D"),
            rx(0x810D, 0x05, "00 06 05 00 06 00 11 01 1A 00 1B 01 1D 01"),
            tx(0x010C, 0x06, "01 01"),
            rx(0x810C, 0x06, "00 01 01 08"),
            tx(0x0106, 0x07, ""),
            rx(0x8106, 0x07, "00 03 01 50 02 E4 03 3C"),
            tx(0x010F, 0x08, ""),
            rx(0x810F, 0x08, "00 01"),
            tx(0x0114, 0x09, ""),
            rx(0x8114, 0x09, "00 02"),
            tx(0x0105, 0x0A, ""),
            rx(0x8105, 0x0A, &firmware),
        ]);
        let script = Script::new(steps);
        let status = open(&script).unwrap().status().unwrap();
        script.assert_done();

        assert_eq!(
            status.battery.left,
            Some(Charge {
                level: 80,
                charging: false
            })
        );
        assert_eq!(
            status.battery.right,
            Some(Charge {
                level: 100,
                charging: true
            })
        );
        assert_eq!(
            status.battery.case,
            Some(Charge {
                level: 60,
                charging: false
            })
        );
        let noise = status.noise_control.unwrap();
        assert_eq!((noise.mode.as_str(), noise.level.as_deref()), ("anc", Some("deep")));
        assert_eq!(status.eq.as_deref(), Some("deep-bass"));
        assert_eq!(status.bass_level, None);
        assert_eq!(status.codec.as_deref(), Some("AAC"));
        assert_eq!(status.firmware.as_deref(), Some("120.120.105"));
        let game = status.switches.iter().find(|s| s.key == "game").unwrap();
        assert!(!game.on);
        assert_eq!(status.switches.len(), 5);
        assert!(status.warnings.is_empty());
    }

    #[test]
    fn answers_the_time_request_during_the_settle() {
        let mut steps = vec![
            rx(0x0500, 0x01, ""),
            Step::Tx(hex("AA 0C 00 00 00 85 01 05 00 00 80 E1 4E 68")),
        ];
        steps.extend(handshake());
        // No 0x8205 ack this time: it may never arrive, and `open` does not wait for it.
        // Spec: realme_t310.md#subscribe
        steps.pop();
        let script = Script::new(steps);
        open(&script).unwrap();
        script.assert_done();
    }

    #[test]
    fn a_silent_handshake_reports_a_busy_channel() {
        let script = Script::new(vec![tx(0x0100, 0x01, "")]);
        assert!(matches!(open(&script), Err(Error::ChannelBusy(_))));
    }

    #[test]
    fn other_models_are_refused() {
        let script = Script::new(vec![
            tx(0x0100, 0x01, ""),
            rx(0x8100, 0x01, codec::T310_BITMAP_REPLY),
            tx(0x0103, 0x02, ""),
            rx(0x8103, 0x02, "00 14 54 06"),
        ]);
        match open(&script) {
            Err(Error::WrongModel { found, .. }) => assert_eq!(found, "065414"),
            other => panic!("expected WrongModel, got {:?}", other.err()),
        }
    }

    #[test]
    fn commands_missing_from_the_bitmap_are_never_sent() {
        // A bitmap with no bits: only always-allowed commands go out, and no subscription.
        let script = Script::new(vec![
            tx(0x0100, 0x01, ""),
            rx(0x8100, 0x01, "00"),
            tx(0x0103, 0x02, ""),
            rx(0x8103, 0x02, "00 12 3C 06"),
            tx(0x0200, 0x03, ""),
            rx(0x8200, 0x03, "00 02 01 02"),
            tx(0x010D, 0x04, "06 05 06 11 1A 1B 1D"),
            rx(0x810D, 0x04, "00 01 06 01"),
            tx(0x0106, 0x05, ""),
            // no battery reply: becomes a warning
        ]);
        let status = open(&script).unwrap().status().unwrap();
        script.assert_done();
        assert_eq!(status.switches.len(), 1);
        assert!(status.noise_control.is_none() && status.firmware.is_none());
        assert_eq!(status.warnings.len(), 1);
        assert!(status.warnings.first().unwrap().starts_with("battery"));
    }

    #[test]
    fn a_lost_link_stops_the_status() {
        let mut steps = handshake();
        steps.extend([tx(0x010D, 0x05, "06 05 06 11 1A 1B 1D"), Step::Close]);
        let script = Script::new(steps);
        assert!(matches!(open(&script).unwrap().status(), Err(Error::NotConnected(_))));
    }

    #[test]
    fn touch_controls_and_hold_cycle() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0108, 0x05, ""),
            rx(0x8108, 0x05, "00 02 01 01 02 01 02 01 04 08"),
            tx(0x010C, 0x06, "02 01"),
            rx(0x810C, 0x06, "00 02 01 0A"),
        ]);
        let script = Script::new(steps);
        let touch = open(&script).unwrap().touch_controls().unwrap();
        script.assert_done();
        assert_eq!(touch.gestures.len(), 2);
        assert_eq!(touch.hold_cycle.unwrap(), ["transparency", "anc"]);
        let gestures: Vec<&str> = touch.choices.iter().map(|c| c.gesture.as_str()).collect();
        assert_eq!(gestures, ["double-tap", "triple-tap", "hold"]);
        let hold = touch.choices.iter().find(|c| c.gesture == "hold").unwrap();
        assert_eq!(hold.actions, ["none", "noise-cycle", "voice-assistant", "game-mode"]);
        assert_eq!(touch.cycle_modes, ["off", "transparency", "anc"]);
    }

    /// The owner's `budctl --debug touch` (HW1), byte for byte. The buds' own frames (slice
    /// byte 08) arrive glued to replies and need no answer.
    #[test]
    fn replays_the_hw1_touch_session() {
        let script = Script::new(vec![
            Step::Tx(hex("AA 07 00 00 00 01 01 00 00")),
            Step::Rx(hex(
                "AA 10 00 00 00 81 01 09 00 00 BF 1F 5A 20 45 04 40 00 AA 07 08 00 01 05 0F 00 00",
            )),
            Step::Tx(hex("AA 07 00 00 03 01 02 00 00")),
            Step::Rx(hex("AA 0B 00 00 03 81 02 04 00 00 12 3C 06")),
            Step::Tx(hex("AA 07 00 00 00 02 03 00 00")),
            Step::Rx(hex("AA 12 00 00 00 82 03 0B 00 00 09 01 02 03 04 05 06 0D F1 F4")),
            Step::Tx(hex("AA 0C 00 00 05 02 04 05 00 04 01 02 03 04")),
            Step::Rx(hex(
                "AA 11 00 00 05 82 04 0A 00 00 04 01 00 02 00 03 00 04 00 AA 0F 08 00 04 02 10 08 00 01 03 02 64 01 64 03 00",
            )),
            Step::Tx(hex("AA 07 00 00 08 01 05 00 00")),
            Step::Rx(hex(
                "AA 21 00 00 08 81 05 1A 00 00 06 01 01 02 01 01 01 03 06 01 01 04 08 02 01 02 01 02 01 03 06 02 01 04 08",
            )),
            Step::Tx(hex("AA 09 00 00 0C 01 06 02 00 02 01")),
            Step::Rx(hex("AA 0B 00 00 0C 81 06 04 00 00 02 01 0A")),
        ]);
        let touch = open(&script).unwrap().touch_controls().unwrap();
        script.assert_done();
        let table: Vec<(&str, &str, &str)> = touch
            .gestures
            .iter()
            .map(|g| (g.side.as_str(), g.gesture.as_str(), g.action.as_str()))
            .collect();
        assert_eq!(
            table,
            [
                ("left", "double-tap", "play-pause"),
                ("left", "triple-tap", "next"),
                ("left", "hold", "noise-cycle"),
                ("right", "double-tap", "play-pause"),
                ("right", "triple-tap", "next"),
                ("right", "hold", "noise-cycle"),
            ]
        );
        assert_eq!(touch.hold_cycle.unwrap(), ["transparency", "anc"]);
        assert!(touch.warnings.is_empty());
    }

    #[test]
    fn sets_noise_control_and_reads_it_back() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0404, 0x05, "01 01 02"),
            rx(0x8404, 0x05, "00"),
            // The buds announce the change; it needs no answer.
            rx(0x0204, 0x10, "03 01 01 02"),
            tx(0x010C, 0x06, "01 01"),
            rx(0x810C, 0x06, "00 01 01 02"),
        ]);
        let script = Script::new(steps);
        let now = open(&script)
            .unwrap()
            .set_noise_control(NoiseMode::Transparency)
            .unwrap();
        script.assert_done();
        assert_eq!((now.mode.as_str(), now.level), ("transparency", None));
    }

    #[test]
    fn reads_back_again_until_the_change_shows() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0404, 0x05, "01 01 01"),
            rx(0x8404, 0x05, "00"),
            tx(0x010C, 0x06, "01 01"),
            rx(0x810C, 0x06, "00 01 01 08"),
            tx(0x010C, 0x07, "01 01"),
            rx(0x810C, 0x07, "00 01 01 01"),
        ]);
        let script = Script::new(steps);
        let now = open(&script).unwrap().set_noise_control(NoiseMode::Off).unwrap();
        script.assert_done();
        assert_eq!(now.mode, "off");
    }

    #[test]
    fn sets_a_noise_cancelling_level() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0404, 0x05, "01 01 04"),
            rx(0x8404, 0x05, "00"),
            tx(0x010C, 0x06, "01 01"),
            rx(0x810C, 0x06, "00 01 01 04"),
        ]);
        let script = Script::new(steps);
        let now = open(&script)
            .unwrap()
            .set_noise_control(NoiseMode::Anc(AncLevel::Light))
            .unwrap();
        script.assert_done();
        assert_eq!((now.mode.as_str(), now.level.as_deref()), ("anc", Some("light")));
    }

    #[test]
    fn a_rejected_noise_control_change_is_not_read_back() {
        let mut steps = handshake();
        steps.extend([tx(0x0404, 0x05, "01 01 08"), rx(0x8404, 0x05, "0E")]);
        let script = Script::new(steps);
        let result = open(&script).unwrap().set_noise_control(NoiseMode::Anc(AncLevel::Deep));
        script.assert_done();
        assert!(matches!(result, Err(Error::Rejected { status: 0x0E, .. })));
    }

    #[test]
    fn noise_control_is_not_written_without_its_bitmap_bit() {
        let script = Script::new(vec![
            tx(0x0100, 0x01, ""),
            rx(0x8100, 0x01, "00"),
            tx(0x0103, 0x02, ""),
            rx(0x8103, 0x02, "00 12 3C 06"),
            tx(0x0200, 0x03, ""),
            rx(0x8200, 0x03, "00 00"),
        ]);
        let result = open(&script).unwrap().set_noise_control(NoiseMode::Off);
        script.assert_done();
        assert!(matches!(result, Err(Error::Unsupported(_))));
    }

    #[test]
    fn sets_game_mode_and_reads_it_back() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0403, 0x05, "06 01"),
            rx(0x8403, 0x05, "00"),
            // Still the old value: budctl reads again.
            tx(0x010D, 0x06, "06 05 06 11 1A 1B 1D"),
            rx(0x810D, 0x06, "00 06 05 00 06 00 11 00 1A 01 1B 00 1D 00"),
            tx(0x010D, 0x07, "06 05 06 11 1A 1B 1D"),
            rx(0x810D, 0x07, "00 06 05 00 06 01 11 00 1A 01 1B 00 1D 00"),
        ]);
        let script = Script::new(steps);
        let game = open(&script).unwrap().set_switch("game", true).unwrap();
        script.assert_done();
        assert_eq!((game.key, game.on), ("game", true));
    }

    #[test]
    fn unknown_switches_are_never_written() {
        // Nothing is sent after the handshake, so the subscribe ack is left out.
        let mut steps = handshake();
        steps.pop();
        let script = Script::new(steps);
        let mut session = open(&script).unwrap();
        assert!(matches!(session.set_switch("turbo", true), Err(Error::Unsupported(_))));
        script.assert_done();
    }

    #[test]
    fn sets_bass_boost_and_reads_it_back_at_once() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0403, 0x05, "1D 01"),
            rx(0x8403, 0x05, "00"),
            tx(0x010D, 0x06, "06 05 06 11 1A 1B 1D"),
            rx(0x810D, 0x06, "00 06 05 00 06 00 11 00 1A 01 1B 00 1D 01"),
        ]);
        let script = Script::new(steps);
        let now = open(&script).unwrap().set_switch("bass-boost", true).unwrap();
        script.assert_done();
        assert_eq!((now.key, now.on), ("bass-boost", true));
    }

    #[test]
    fn a_switch_the_buds_lack_is_rejected() {
        let mut steps = handshake();
        steps.extend([tx(0x0403, 0x05, "06 00"), rx(0x8403, 0x05, "01")]);
        let script = Script::new(steps);
        let result = open(&script).unwrap().set_switch("game", false);
        script.assert_done();
        assert!(matches!(result, Err(Error::Rejected { status: 0x01, .. })));
    }

    #[test]
    fn selects_an_eq_preset_and_reads_it_back() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0406, 0x05, "01"),
            rx(0x8406, 0x05, "00"),
            tx(0x010F, 0x06, ""),
            rx(0x810F, 0x06, "00 01"),
        ]);
        let script = Script::new(steps);
        let now = open(&script).unwrap().set_eq("deep-bass").unwrap();
        script.assert_done();
        assert_eq!(now, "deep-bass");
    }

    #[test]
    fn an_unknown_eq_preset_is_a_usage_error() {
        let mut steps = handshake();
        steps.pop();
        let script = Script::new(steps);
        let result = open(&script).unwrap().set_eq("loudness");
        script.assert_done();
        match result {
            Err(Error::Usage(message)) => assert!(message.contains("pure-raw, deep-bass, pure-vocals, pure-bass")),
            other => panic!("expected a usage error, got {other:?}"),
        }
    }

    #[test]
    fn rings_and_stops() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0400, 0x05, "01"),
            rx(0x8400, 0x05, "00"),
            tx(0x0400, 0x06, "00"),
            rx(0x8400, 0x06, "00"),
        ]);
        let script = Script::new(steps);
        let mut session = open(&script).unwrap();
        session.find(true).unwrap();
        session.find(false).unwrap();
        script.assert_done();
    }

    #[test]
    fn find_is_not_sent_without_its_bitmap_bit() {
        // Bits 0-4 only: no bit 5.
        let script = Script::new(vec![
            tx(0x0100, 0x01, ""),
            rx(0x8100, 0x01, "00 1F"),
            tx(0x0103, 0x02, ""),
            rx(0x8103, 0x02, "00 12 3C 06"),
            tx(0x0200, 0x03, ""),
            rx(0x8200, 0x03, "00 00"),
        ]);
        let result = open(&script).unwrap().find(true);
        script.assert_done();
        assert!(matches!(result, Err(Error::Unsupported(_))));
    }

    /// The owner's gesture table (HW1) and the same table with the left triple tap on volume up.
    const HW1_TABLE: &str = "00 06 01 01 02 01 01 01 03 06 01 01 04 08 02 01 02 01 02 01 03 06 02 01 04 08";
    const LEFT_VOLUME_UP: &str = "06 01 01 02 01 01 01 03 0B 01 01 04 08 02 01 02 01 02 01 03 06 02 01 04 08";

    #[test]
    fn a_touch_dry_run_reads_the_table_and_sends_nothing() {
        let mut steps = handshake();
        steps.extend([tx(0x0108, 0x05, ""), rx(0x8108, 0x05, HW1_TABLE)]);
        let script = Script::new(steps);
        let change = open(&script)
            .unwrap()
            .set_touch("left", "triple-tap", "volume-up", true)
            .unwrap();
        script.assert_done();
        assert!(!change.sent);
        let write = change.write.unwrap();
        assert_eq!((write.command, write.payload), (0x0401, hex(LEFT_VOLUME_UP)));
        assert_eq!(
            change.changes,
            [GestureChange {
                side: "left".to_owned(),
                gesture: "triple-tap".to_owned(),
                from: "next".to_owned(),
                to: "volume-up".to_owned(),
            }]
        );
        let left_triple = change.gestures.get(1).unwrap();
        assert_eq!(left_triple.action, "volume-up");
    }

    #[test]
    fn sets_a_gesture_and_reads_the_table_back() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0108, 0x05, ""),
            rx(0x8108, 0x05, HW1_TABLE),
            tx(0x0401, 0x06, LEFT_VOLUME_UP),
            // The owner's ack (W8): 00, then the old table cut to the write's length.
            rx(
                0x8401,
                0x06,
                "00 06 01 01 02 01 01 01 03 06 01 01 04 08 02 01 02 01 02 01 03 06 02 01 04",
            ),
            tx(0x0108, 0x07, ""),
            rx(0x8108, 0x07, &format!("00 {LEFT_VOLUME_UP}")),
        ]);
        let script = Script::new(steps);
        let change = open(&script)
            .unwrap()
            .set_touch("left", "triple-tap", "volume-up", false)
            .unwrap();
        script.assert_done();
        assert!(change.sent);
        assert_eq!(change.changes.len(), 1);
        assert_eq!(change.gestures.get(1).unwrap().action, "volume-up");
    }

    #[test]
    fn a_gesture_that_already_has_the_action_is_not_written() {
        let mut steps = handshake();
        steps.extend([tx(0x0108, 0x05, ""), rx(0x8108, 0x05, HW1_TABLE)]);
        let script = Script::new(steps);
        let change = open(&script)
            .unwrap()
            .set_touch("both", "hold", "noise-cycle", false)
            .unwrap();
        script.assert_done();
        assert!(change.changes.is_empty() && change.write.is_none() && !change.sent);
    }

    #[test]
    fn a_bad_touch_action_is_refused_before_anything_is_sent() {
        let mut steps = handshake();
        steps.pop();
        let script = Script::new(steps);
        let result = open(&script)
            .unwrap()
            .set_touch("left", "double-tap", "volume-up", false);
        script.assert_done();
        assert!(matches!(result, Err(Error::Usage(_))));
    }

    #[test]
    fn sets_the_hold_cycle_and_reads_it_back() {
        let mut steps = handshake();
        steps.extend([
            tx(0x0404, 0x05, "02 01 0B"),
            rx(0x8404, 0x05, "00"),
            tx(0x010C, 0x06, "02 01"),
            rx(0x810C, 0x06, "00 02 01 0B"),
        ]);
        let script = Script::new(steps);
        let now = open(&script)
            .unwrap()
            .set_hold_cycle(&["off", "transparency", "anc"])
            .unwrap();
        script.assert_done();
        assert_eq!(now, ["off", "transparency", "anc"]);
    }

    #[test]
    fn a_one_mode_hold_cycle_is_refused_before_anything_is_sent() {
        let mut steps = handshake();
        steps.pop();
        let script = Script::new(steps);
        let result = open(&script).unwrap().set_hold_cycle(&["anc"]);
        script.assert_done();
        assert!(matches!(result, Err(Error::Usage(_))));
    }

    /// The custom EQ list of the owner's T310 (W11a): one flat preset, "Custom1", id 04.
    const OWNER_CUSTOM_EQS: &str =
        "00 01 01 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00";

    #[test]
    fn lists_built_in_and_custom_eq_presets() {
        let mut steps = handshake();
        steps.extend([
            tx(0x010F, 0x05, ""),
            rx(0x810F, 0x05, "00 04"),
            tx(0x0122, 0x06, ""),
            // The owner's T310 (W11a).
            rx(0x8122, 0x06, OWNER_CUSTOM_EQS),
        ]);
        let script = Script::new(steps);
        let eq = open(&script).unwrap().eq_presets().unwrap();
        script.assert_done();
        assert_eq!(eq.current.as_deref(), Some("custom-4"));
        let names: Vec<(&str, bool)> = eq.presets.iter().map(|p| (p.name.as_str(), p.custom)).collect();
        assert_eq!(
            names,
            [
                ("pure-raw", false),
                ("deep-bass", false),
                ("pure-vocals", false),
                ("pure-bass", false),
                ("Custom1", true)
            ]
        );
        let bands: Vec<u16> = eq.presets.last().unwrap().bands.iter().map(|b| b.hz).collect();
        assert_eq!(bands, [50, 250, 1000, 4000, 8000, 16000]);
        assert!(eq.warnings.is_empty());
    }

    #[test]
    fn sets_the_bass_level() {
        let mut steps = handshake();
        steps.extend([tx(0x041B, 0x05, "FB 05 FE"), rx(0x841B, 0x05, "00")]);
        let script = Script::new(steps);
        let mut session = open(&script).unwrap();
        assert_eq!(session.bass_range(), Some((-5, 5)));
        session.set_bass_level(-2).unwrap();
        script.assert_done();
    }

    #[test]
    fn a_bass_level_out_of_range_is_refused_before_anything_is_sent() {
        let mut steps = handshake();
        steps.pop();
        let script = Script::new(steps);
        let result = open(&script).unwrap().set_bass_level(6);
        script.assert_done();
        assert!(matches!(result, Err(Error::Usage(_))));
    }

    #[test]
    fn creates_a_custom_preset_and_finds_it_in_the_list() {
        const LIST: &str =
            "00 01 01 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00";
        const AFTER: &str = "00 02 01 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00 \
             00 FA 06 05 04 54 65 73 74 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00";
        let mut steps = handshake();
        steps.extend([
            tx(0x0122, 0x05, ""),
            rx(0x8122, 0x05, LIST),
            tx(
                0x0418,
                0x06,
                "01 FA 06 00 04 54 65 73 74 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00",
            ),
            rx(0x8418, 0x06, "00 05"),
            tx(0x0122, 0x07, ""),
            rx(0x8122, 0x07, AFTER),
        ]);
        let script = Script::new(steps);
        let created = open(&script).unwrap().create_custom_eq("Test").unwrap();
        script.assert_done();
        assert_eq!((created.id, created.name.as_str()), (5, "Test"));
        assert_eq!(created.bands.len(), 6);
    }

    #[test]
    fn renames_a_custom_preset_and_reads_it_back() {
        const LIST: &str = "00 02 00 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 06 FA 00 03 E8 03 03 A0 0F 04 40 1F 06 80 3E 06 \
             01 FA 06 05 04 54 65 73 74 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00";
        const AFTER: &str = "00 02 00 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 06 FA 00 03 E8 03 03 A0 0F 04 40 1F 06 80 3E 06 \
             01 FA 06 05 03 47 79 6D 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00";
        let mut steps = handshake();
        steps.extend([
            tx(0x0122, 0x05, ""),
            rx(0x8122, 0x05, LIST),
            tx(
                0x0418,
                0x06,
                "02 FA 06 05 03 47 79 6D 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00",
            ),
            rx(0x8418, 0x06, "00 05"),
            tx(0x0122, 0x07, ""),
            rx(0x8122, 0x07, AFTER),
        ]);
        let script = Script::new(steps);
        let now = open(&script).unwrap().rename_custom_eq("custom-5", "Gym").unwrap();
        script.assert_done();
        assert_eq!((now.id, now.name.as_str()), (5, "Gym"));

        // Another preset's name is refused before anything is written.
        let mut steps = handshake();
        steps.extend([tx(0x0122, 0x05, ""), rx(0x8122, 0x05, LIST)]);
        let script = Script::new(steps);
        let taken = open(&script).unwrap().rename_custom_eq("custom-5", "Custom1");
        script.assert_done();
        assert!(matches!(taken, Err(Error::Usage(_))));
    }

    #[test]
    fn refuses_a_fourth_preset_or_a_taken_name_before_writing() {
        let one = "00 FA 06 04 07 43 75 73 74 6F 6D 31 00";
        for (list, name) in [
            (format!("00 03 {one} {one} {one}"), "Test"),
            (format!("00 01 {one}"), "Custom1"),
            ("00 00".to_owned(), "Test"),
        ] {
            let mut steps = handshake();
            steps.extend([tx(0x0122, 0x05, ""), rx(0x8122, 0x05, &list)]);
            let script = Script::new(steps);
            let result = open(&script).unwrap().create_custom_eq(name);
            script.assert_done();
            assert!(matches!(result, Err(Error::Usage(_))), "{list}");
        }
    }

    #[test]
    fn retunes_a_custom_preset_and_reads_it_back() {
        let tuned =
            "00 01 01 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 03 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00";
        let mut steps = handshake();
        steps.extend([
            tx(0x0122, 0x05, ""),
            rx(0x8122, 0x05, OWNER_CUSTOM_EQS),
            tx(
                0x0418,
                0x06,
                "02 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 03 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00",
            ),
            rx(0x8418, 0x06, "00 04"),
            tx(0x0122, 0x07, ""),
            rx(0x8122, 0x07, tuned),
        ]);
        let script = Script::new(steps);
        let now = open(&script)
            .unwrap()
            .set_custom_eq("custom-4", &[EqBand { hz: 50, gain: 3 }])
            .unwrap();
        script.assert_done();
        assert_eq!((now.name.as_str(), now.id), ("Custom1", 4));
        assert_eq!(now.bands.first(), Some(&EqBand { hz: 50, gain: 3 }));
    }

    #[test]
    fn eq_custom_selects_by_saving_the_preset_unchanged() {
        let saved = "02 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00";
        let mut steps = handshake();
        steps.extend([
            tx(0x0122, 0x05, ""),
            rx(0x8122, 0x05, OWNER_CUSTOM_EQS),
            tx(0x0418, 0x06, saved),
            rx(0x8418, 0x06, "00 04"),
            tx(0x0122, 0x07, ""),
            rx(0x8122, 0x07, OWNER_CUSTOM_EQS),
        ]);
        let script = Script::new(steps);
        let now = open(&script).unwrap().set_eq("custom-4").unwrap();
        script.assert_done();
        assert_eq!(now, "custom-4");
    }

    #[test]
    fn a_custom_preset_the_buds_lack_is_never_written() {
        let mut steps = handshake();
        steps.extend([tx(0x0122, 0x05, ""), rx(0x8122, 0x05, OWNER_CUSTOM_EQS)]);
        let script = Script::new(steps);
        let result = open(&script)
            .unwrap()
            .set_custom_eq("custom-5", &[EqBand { hz: 50, gain: 3 }]);
        script.assert_done();
        match result {
            Err(Error::Usage(message)) => assert!(message.ends_with("custom-4")),
            other => panic!("expected a usage error, got {other:?}"),
        }
    }
}
