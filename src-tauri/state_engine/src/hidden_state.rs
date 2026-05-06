use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};

use crate::{patch::EnginePatch, soul::Soul};

const HIDDEN_STATE_MARKER: &str = "[HIDDEN_STATE]";
const HIDDEN_STATE_JSON_START: &str = "[HIDDEN STATE]";
const HIDDEN_STATE_JSON_END: &str = "[/HIDDEN STATE]";
const HIDDEN_STATE_ENCODING_PREFIX: &str = "mne1.";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HiddenState {
    pub memory: Option<String>,
    pub tag: Option<String>,
    pub trust_delta: Option<f32>,
    pub affection_delta: Option<f32>,
    pub world_event: Option<String>,
    pub new_location: Option<String>,
    pub present_characters: Option<Vec<String>>,
    pub arousal_delta: Option<f32>,
    pub arousal_denied: Option<bool>,
    pub orgasm_allowed: Option<bool>,
    pub forced_orgasm: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ParsedProviderResponse {
    pub visible_text: String,
    pub hidden_state: HiddenState,
    pub engine_patch: EnginePatch,
}

impl ParsedProviderResponse {
    pub fn apply_to_soul(&self, soul: &mut Soul) {
        let _ = self.engine_patch.apply_to_soul(soul);
    }

    pub fn has_patch(&self) -> bool {
        !self.engine_patch.is_empty()
    }
}

/// Public wrapper that applies a parsed hidden-state payload to the soul.
pub fn apply_hidden_state(hidden_state: &HiddenState, soul: &mut Soul) {
    hidden_state.apply_to_soul(soul);
}

impl HiddenState {
    pub fn apply_to_soul(&self, soul: &mut Soul) {
        let patch = EnginePatch::from(self);
        let _ = patch.apply_to_soul(soul);
    }
}

pub fn encode_hidden_state(hidden_state: &HiddenState) -> String {
    let bytes = serde_json::to_vec(hidden_state).expect("hidden state should serialize");
    format!(
        "{}{}",
        HIDDEN_STATE_ENCODING_PREFIX,
        URL_SAFE_NO_PAD.encode(bytes)
    )
}

pub fn parse_hidden_state(raw: &str) -> Result<ParsedProviderResponse, serde_json::Error> {
    if let Some(start) = raw.find(HIDDEN_STATE_JSON_START) {
        let visible_text = raw[..start].trim().to_string();
        let hidden_start = start + HIDDEN_STATE_JSON_START.len();
        let hidden_part = if let Some(end) = raw[hidden_start..].find(HIDDEN_STATE_JSON_END) {
            &raw[hidden_start..hidden_start + end]
        } else {
            &raw[hidden_start..]
        }
        .trim();
        let (hidden_state, engine_patch) = decode_hidden_payload(hidden_part)?;
        return Ok(ParsedProviderResponse {
            visible_text,
            hidden_state,
            engine_patch,
        });
    }

    let Some(start) = raw.find(HIDDEN_STATE_MARKER) else {
        return Ok(ParsedProviderResponse {
            visible_text: raw.trim().to_string(),
            hidden_state: HiddenState::default(),
            engine_patch: EnginePatch::default(),
        });
    };
    let visible_text = raw[..start].trim().to_string();
    let hidden_part = raw[start + HIDDEN_STATE_MARKER.len()..].trim();
    let (hidden_state, engine_patch) = decode_hidden_payload(hidden_part)?;
    Ok(ParsedProviderResponse {
        visible_text,
        hidden_state,
        engine_patch,
    })
}

fn decode_hidden_payload(payload: &str) -> Result<(HiddenState, EnginePatch), serde_json::Error> {
    let value = decode_hidden_value(payload)?;
    if looks_like_engine_patch(&value) {
        let engine_patch = serde_json::from_value(value)?;
        return Ok((HiddenState::default(), engine_patch));
    }

    let hidden_state: HiddenState = serde_json::from_value(value)?;
    let engine_patch = EnginePatch::from(&hidden_state);
    Ok((hidden_state, engine_patch))
}

fn decode_hidden_value(payload: &str) -> Result<serde_json::Value, serde_json::Error> {
    if let Some(encoded) = payload.strip_prefix(HIDDEN_STATE_ENCODING_PREFIX) {
        if let Ok(bytes) = URL_SAFE_NO_PAD.decode(encoded) {
            return serde_json::from_slice(&bytes);
        }
    }

    if !payload.trim_start().starts_with('{') {
        if let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload) {
            return serde_json::from_slice(&bytes);
        }
    }

    serde_json::from_str(payload)
}

fn looks_like_engine_patch(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.contains_key("schema_version")
        || object.contains_key("soul_patch")
        || object.contains_key("world_patch")
        || object.contains_key("body_patch")
        || object.contains_key("sensory_patch")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::new_default_soul;

    #[test]
    fn strips_and_parses_encoded_hidden_state() {
        let hidden_state = HiddenState {
            memory: Some("A promise mattered.".into()),
            tag: Some("trust_building".into()),
            trust_delta: Some(3.0),
            affection_delta: Some(2.0),
            world_event: Some("A promise landed.".into()),
            new_location: None,
            present_characters: Some(vec!["Aurora".into()]),
            arousal_delta: None,
            arousal_denied: None,
            orgasm_allowed: None,
            forced_orgasm: None,
        };
        let raw = format!(
            "Visible line.\n\n{HIDDEN_STATE_MARKER}\n{}",
            encode_hidden_state(&hidden_state)
        );

        let parsed = parse_hidden_state(&raw).expect("parsed");
        assert_eq!(parsed.visible_text, "Visible line.");
        assert_eq!(parsed.hidden_state.tag.as_deref(), Some("trust_building"));
        assert_eq!(parsed.hidden_state.trust_delta, Some(3.0));
        assert!(parsed.has_patch());
        assert!(raw.contains(HIDDEN_STATE_ENCODING_PREFIX));
        assert!(!raw.contains("\"tag\""));
    }

    #[test]
    fn accepts_legacy_plain_json_hidden_state() {
        let raw = r#"Visible line.

[HIDDEN_STATE]
{"memory":"A promise mattered.","tag":"trust_building","trust_delta":3,"affection_delta":2,"world_event":"A promise landed."}"#;

        let parsed = parse_hidden_state(raw).expect("parsed");
        assert_eq!(parsed.hidden_state.tag.as_deref(), Some("trust_building"));
    }

    #[test]
    fn accepts_prompt_json_hidden_state_block() {
        let raw = r#"Visible line.

[HIDDEN STATE]{"memory":"A promise mattered.","tag":"trust_building","trust_delta":3,"affection_delta":2,"world_event":"A promise landed.","new_location":"Safehouse","present_characters":["Aurora"]}[/HIDDEN STATE]"#;

        let parsed = parse_hidden_state(raw).expect("parsed");
        assert_eq!(parsed.visible_text, "Visible line.");
        assert_eq!(parsed.hidden_state.tag.as_deref(), Some("trust_building"));
        assert_eq!(
            parsed.hidden_state.new_location.as_deref(),
            Some("Safehouse")
        );
        assert_eq!(
            parsed.hidden_state.present_characters.as_deref(),
            Some(&["Aurora".to_string()][..])
        );
        assert!(parsed.has_patch());
    }

    #[test]
    fn accepts_structured_engine_patch_block() {
        let raw = r#"Visible line.

[HIDDEN STATE]{"schema_version":1,"soul_patch":{"relationship_delta":{"trust":4},"new_memories":[{"content":"Aurora marks the stairwell.","tag":"orientation"}]},"world_patch":{"location":"Stairwell","recent_event":"Aurora found the stairwell."}}[/HIDDEN STATE]"#;

        let parsed = parse_hidden_state(raw).expect("parsed");
        let mut soul = new_default_soul("Aurora");
        parsed.apply_to_soul(&mut soul);

        assert_eq!(parsed.visible_text, "Visible line.");
        assert!(parsed.hidden_state.memory.is_none());
        assert!(parsed.has_patch());
        assert_eq!(soul.relationships["user"].trust, 14.0);
        assert_eq!(soul.memory.recent.len(), 1);
        assert_eq!(soul.world.location, "Stairwell");
    }

    #[test]
    fn accepts_visible_only_response() {
        let parsed = parse_hidden_state("Only visible.").expect("parsed");
        assert_eq!(parsed.visible_text, "Only visible.");
        assert!(parsed.hidden_state.memory.is_none());
        assert!(!parsed.has_patch());
    }

    #[test]
    fn hidden_state_application_updates_soul() {
        let mut soul = new_default_soul("Aurora");
        let state = HiddenState {
            memory: Some("Aurora notices a safer rhythm in the exchange.".into()),
            tag: Some("trust_building".into()),
            trust_delta: Some(4.0),
            affection_delta: Some(2.0),
            world_event: Some("A small trust-building exchange changed the mood.".into()),
            new_location: Some("Safehouse".into()),
            present_characters: None,
            arousal_delta: Some(25.0),
            arousal_denied: None,
            orgasm_allowed: None,
            forced_orgasm: None,
        };

        state.apply_to_soul(&mut soul);

        assert_eq!(soul.relationships["user"].trust, 14.0);
        assert_eq!(soul.memory.recent.len(), 1);
        assert_eq!(soul.world.recent_events.len(), 1);
        assert_eq!(soul.world.location, "Safehouse");
        assert_eq!(soul.arousal.level, 25.0);
    }
}
