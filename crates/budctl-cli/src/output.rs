//! Printing results, as text for people or as JSON (`--json`) for scripts and status bars.

use std::io::{self, Write};

use budctl_core::bluez::PairedDevice;
use budctl_core::devices::Device;
use budctl_core::status::{Charge, JSON_SCHEMA, NoiseControl, Status, Switch, TouchControls};
use budctl_core::{Error, Found};
use serde_json::Value;

/// Why a command did not finish: budctl failed, or stdout did.
pub enum Failure {
    Budctl(Error),
    Io(io::Error),
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Failure::Budctl(error)
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Failure::Io(error)
    }
}

impl From<serde_json::Error> for Failure {
    fn from(error: serde_json::Error) -> Self {
        Failure::Io(io::Error::from(error))
    }
}

/// Print `body` inside the JSON envelope every result shares: `schema`, plus `device` and
/// `address` when the result is about one pair of buds.
fn json(out: &mut impl Write, target: Option<(&PairedDevice, &dyn Device)>, body: Value) -> Result<(), Failure> {
    let mut envelope = serde_json::Map::new();
    envelope.insert("schema".to_owned(), JSON_SCHEMA.into());
    if let Some((device, model)) = target {
        envelope.insert("device".to_owned(), model.name().into());
        envelope.insert("address".to_owned(), device.address.to_string().into());
    }
    if let Value::Object(fields) = body {
        envelope.extend(fields);
    }
    serde_json::to_writer(&mut *out, &envelope)?;
    writeln!(out)?;
    Ok(())
}

pub fn list(out: &mut impl Write, found: &[Found], as_json: bool) -> Result<(), Failure> {
    if as_json {
        let devices: Vec<Value> = found
            .iter()
            .map(|f| {
                serde_json::json!({
                    "address": f.device.address,
                    "name": f.device.name,
                    "alias": f.device.alias,
                    "connected": f.device.connected,
                    // The model budctl recognized, or null.
                    "model": f.model.map(|m| m.name()),
                })
            })
            .collect();
        return json(out, None, serde_json::json!({ "devices": devices }));
    }

    if found.is_empty() {
        writeln!(out, "No paired Bluetooth devices.")?;
        return Ok(());
    }
    for f in found {
        let state = if f.device.connected {
            "connected"
        } else {
            "not connected"
        };
        let model = match f.model {
            Some(model) => format!("supported: {}", model.name()),
            None => "not supported".to_owned(),
        };
        writeln!(out, "{}  {}  ({state}, {model})", f.device.address, f.device.alias)?;
    }
    Ok(())
}

fn charge(charge: Option<Charge>) -> String {
    match charge {
        Some(Charge { level, charging: true }) => format!("{level}% (charging)"),
        Some(Charge { level, charging: false }) => format!("{level}%"),
        None => "—".to_owned(),
    }
}

fn header(out: &mut impl Write, device: &PairedDevice, model: &dyn Device) -> io::Result<()> {
    writeln!(out, "{} ({})", model.name(), device.address)
}

fn warnings(out: &mut impl Write, warnings: &[String]) -> io::Result<()> {
    for warning in warnings {
        writeln!(out, "  warning: {warning}")?;
    }
    Ok(())
}

pub fn status(
    out: &mut impl Write,
    device: &PairedDevice,
    model: &dyn Device,
    status: &Status,
    as_json: bool,
) -> Result<(), Failure> {
    if as_json {
        return json(out, Some((device, model)), serde_json::to_value(status)?);
    }

    header(out, device, model)?;
    let battery = &status.battery;
    let mut rows: Vec<(&str, String)> = vec![(
        "Battery",
        format!(
            "L {}  R {}  case {}",
            charge(battery.left),
            charge(battery.right),
            charge(battery.case)
        ),
    )];
    if let Some(noise) = &status.noise_control {
        let value = match &noise.level {
            Some(level) => format!("{} ({level})", noise.mode),
            None => noise.mode.clone(),
        };
        rows.push(("Noise control", value));
    }
    if let Some(eq) = &status.eq {
        rows.push(("EQ", eq.clone()));
    }
    if let Some(bass) = status.bass_level {
        rows.push(("Bass level", bass.to_string()));
    }
    for switch in &status.switches {
        rows.push((switch.label, if switch.on { "on" } else { "off" }.to_owned()));
    }
    if let Some(codec) = &status.codec {
        rows.push(("Codec", codec.clone()));
    }
    if let Some(firmware) = &status.firmware {
        rows.push(("Firmware", firmware.clone()));
    }
    // Labels are as wide as the longest one, so values line up whatever the device names.
    let width = rows.iter().map(|(label, _)| label.chars().count()).max().unwrap_or(0);
    for (label, value) in &rows {
        writeln!(out, "  {label:<width$}  {value}")?;
    }
    warnings(out, &status.warnings)?;
    Ok(())
}

pub fn touch(
    out: &mut impl Write,
    device: &PairedDevice,
    model: &dyn Device,
    touch: &TouchControls,
    as_json: bool,
) -> Result<(), Failure> {
    if as_json {
        return json(out, Some((device, model)), serde_json::to_value(touch)?);
    }

    header(out, device, model)?;
    if touch.gestures.is_empty() {
        writeln!(out, "  (no gestures reported)")?;
    }
    for g in &touch.gestures {
        match g.group {
            Some(group) => writeln!(out, "  {:<6} {:<12} {}  [group {group}]", g.side, g.gesture, g.action)?,
            None => writeln!(out, "  {:<6} {:<12} {}", g.side, g.gesture, g.action)?,
        }
    }
    if let Some(cycle) = &touch.hold_cycle {
        writeln!(out, "  Hold cycles through: {}", cycle.join(" → "))?;
    }
    warnings(out, &touch.warnings)?;
    Ok(())
}

/// The noise-control mode after a change, as the buds report it on reading back.
pub fn noise_control(
    out: &mut impl Write,
    device: &PairedDevice,
    model: &dyn Device,
    now: &NoiseControl,
    as_json: bool,
) -> Result<(), Failure> {
    if as_json {
        return json(
            out,
            Some((device, model)),
            serde_json::json!({ "noise_control": serde_json::to_value(now)? }),
        );
    }
    header(out, device, model)?;
    match &now.level {
        Some(level) => writeln!(out, "  Noise control  {} ({level})", now.mode)?,
        None => writeln!(out, "  Noise control  {}", now.mode)?,
    }
    Ok(())
}

/// One switch after a change, as the buds report it on reading back. JSON uses the same
/// `switches` map as `status`.
pub fn switch(
    out: &mut impl Write,
    device: &PairedDevice,
    model: &dyn Device,
    now: &Switch,
    as_json: bool,
) -> Result<(), Failure> {
    if as_json {
        return json(
            out,
            Some((device, model)),
            serde_json::json!({ "switches": { now.key: now.on } }),
        );
    }
    header(out, device, model)?;
    writeln!(out, "  {}  {}", now.label, if now.on { "on" } else { "off" })?;
    Ok(())
}

/// The EQ preset after a change, as the buds report it on reading back.
pub fn eq(
    out: &mut impl Write,
    device: &PairedDevice,
    model: &dyn Device,
    now: &str,
    as_json: bool,
) -> Result<(), Failure> {
    if as_json {
        return json(out, Some((device, model)), serde_json::json!({ "eq": now }));
    }
    header(out, device, model)?;
    writeln!(out, "  EQ  {now}")?;
    Ok(())
}

/// After `find start` or `find stop`: the buds acked; there is nothing to read back.
pub fn find(
    out: &mut impl Write,
    device: &PairedDevice,
    model: &dyn Device,
    ring: bool,
    as_json: bool,
) -> Result<(), Failure> {
    if as_json {
        return json(out, Some((device, model)), serde_json::json!({ "ringing": ring }));
    }
    header(out, device, model)?;
    if ring {
        writeln!(out, "  Ringing. Stop with: budctl find stop")?;
    } else {
        writeln!(out, "  Stopped ringing.")?;
    }
    Ok(())
}
