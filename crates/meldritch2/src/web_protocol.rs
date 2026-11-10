//! Typed implementation of meldritch-web-protocol 1.0.0 (wire schema 3).

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: &str = "1.0.0";
pub const SUPPORTED_PROTOCOLS: &str = ">=1.0.0 <2.0.0";
pub const WIRE_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ClientMessage {
    #[serde(rename = "hello")]
    Hello {
        protocol: String,
        #[serde(default)]
        client: Option<String>,
    },
    #[serde(rename = "authenticate")]
    Authenticate { token: String },
    #[serde(rename = "control.set")]
    ControlSet { name: String, value: Value },
    #[serde(rename = "step.toggle")]
    StepToggle {
        plane: String,
        control: String,
        step: usize,
    },
    #[serde(rename = "trigger.fire")]
    TriggerFire { plane: String, control: String },
    #[serde(rename = "note.create")]
    NoteCreate {
        plane: String,
        control: String,
        pattern: usize,
        step: u8,
        #[serde(deserialize_with = "midi_data_byte")]
        pitch: u8,
        #[serde(deserialize_with = "positive_midi_data_byte")]
        velocity: u8,
        #[serde(deserialize_with = "positive_byte")]
        duration: u8,
        #[serde(default)]
        legato: bool,
    },
    #[serde(rename = "note.update")]
    NoteUpdate {
        plane: String,
        control: String,
        pattern: usize,
        id: u32,
        step: u8,
        #[serde(deserialize_with = "midi_data_byte")]
        pitch: u8,
        #[serde(deserialize_with = "positive_midi_data_byte")]
        velocity: u8,
        #[serde(deserialize_with = "positive_byte")]
        duration: u8,
        #[serde(default)]
        legato: bool,
    },
    #[serde(rename = "note.delete")]
    NoteDelete {
        plane: String,
        control: String,
        pattern: usize,
        id: u32,
    },
    #[serde(rename = "drum.hit.toggle")]
    DrumHitToggle {
        plane: String,
        control: String,
        pattern: usize,
        row: String,
        step: u8,
    },
    #[serde(rename = "drum.hit.velocity")]
    DrumHitVelocity {
        plane: String,
        control: String,
        pattern: usize,
        row: String,
        step: u8,
        #[serde(deserialize_with = "positive_midi_data_byte")]
        velocity: u8,
    },
    #[serde(rename = "drum.mute")]
    DrumMute {
        plane: String,
        control: String,
        row: String,
        muted: bool,
    },
    #[serde(rename = "pattern.select")]
    PatternSelect {
        plane: String,
        control: String,
        pattern: usize,
        boundary: PatternBoundary,
    },
    #[serde(rename = "adsr.set")]
    AdsrSet {
        plane: String,
        control: String,
        parameter: AdsrParameter,
        value: f64,
    },
    #[serde(rename = "preset.select")]
    PresetSelect {
        plane: String,
        control: String,
        preset: Value,
    },
}

fn bounded_byte<'de, D>(deserializer: D, minimum: u8, maximum: u8) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u8::deserialize(deserializer)?;
    (minimum..=maximum)
        .contains(&value)
        .then_some(value)
        .ok_or_else(|| serde::de::Error::custom(format!("value must be in {minimum}..={maximum}")))
}

fn midi_data_byte<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_byte(deserializer, 0, 127)
}

fn positive_midi_data_byte<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_byte(deserializer, 1, 127)
}

fn positive_byte<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_byte(deserializer, 1, u8::MAX)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatternBoundary {
    Immediate,
    Beat,
    Bar,
    Cycle,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdsrParameter {
    Attack,
    Decay,
    Sustain,
    Release,
}

impl ClientMessage {
    pub fn into_value(self) -> Value {
        serde_json::to_value(self).expect("typed Web messages serialize")
    }
}

pub fn known_client_type(kind: &str) -> bool {
    matches!(
        kind,
        "hello"
            | "authenticate"
            | "control.set"
            | "step.toggle"
            | "trigger.fire"
            | "note.create"
            | "note.update"
            | "note.delete"
            | "drum.hit.toggle"
            | "drum.hit.velocity"
            | "drum.mute"
            | "pattern.select"
            | "adsr.set"
            | "preset.select"
    )
}

/// Version 1 is the only current major. This deliberately accepts the exact
/// version and the documented common range spellings without implementing a
/// second package-manager range language in the audio engine.
pub fn negotiate(requested: &str) -> Option<&'static str> {
    let compact = requested.replace(' ', "");
    (requested == PROTOCOL_VERSION
        || requested == "1"
        || requested == "^1"
        || compact == ">=1.0.0<2.0.0")
        .then_some(PROTOCOL_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_fixtures_deserialize_and_round_trip() {
        let message: ClientMessage = serde_json::from_value(serde_json::json!({
            "type":"note.create", "plane":"notes", "control":"roll",
            "pattern":0, "step":0, "pitch":60, "velocity":100, "duration":2
        }))
        .unwrap();
        assert_eq!(message.into_value()["type"], "note.create");
        assert_eq!(negotiate(">=1.0.0 <2.0.0"), Some("1.0.0"));
        assert_eq!(negotiate("^2"), None);
    }

    #[test]
    fn malformed_known_commands_are_rejected() {
        let malformed = serde_json::json!({
            "type":"drum.hit.velocity", "plane":"drums", "control":"grid",
            "pattern":0, "row":"kick", "step":0, "velocity":200
        });
        assert!(serde_json::from_value::<ClientMessage>(malformed).is_err());
    }
}
