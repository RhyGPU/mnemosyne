use serde::{Deserialize, Serialize};

use crate::{
    memory::create_scored_memory,
    soul::{current_timestamp, Relationship, Soul},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HiddenState {
    pub memory: Option<String>,
    pub tag: Option<String>,
    pub trust_delta: Option<f32>,
    pub affection_delta: Option<f32>,
    pub world_event: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedProviderResponse {
    pub visible_text: String,
    pub hidden_state: HiddenState,
}

impl ParsedProviderResponse {
    pub fn apply_to_soul(&self, soul: &mut Soul) {
        self.hidden_state.apply_to_soul(soul);
    }
}

impl HiddenState {
    pub fn apply_to_soul(&self, soul: &mut Soul) {
        let relationship = soul
            .relationships
            .entry("user".into())
            .or_insert_with(default_relationship);
        relationship.trust = clamp_stat(relationship.trust + self.trust_delta.unwrap_or(0.0));
        relationship.affection =
            clamp_stat(relationship.affection + self.affection_delta.unwrap_or(0.0));

        if let Some(memory) = self
            .memory
            .as_deref()
            .filter(|memory| !memory.trim().is_empty())
        {
            let tag = self.tag.as_deref().unwrap_or("observation");
            let recent = create_scored_memory(soul, memory, tag);
            soul.memory.recent.push(recent);
            soul.memory.recent.sort_by(|left, right| {
                right
                    .salience
                    .partial_cmp(&left.salience)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            soul.memory.recent.truncate(12);
        }

        if let Some(event) = self
            .world_event
            .as_deref()
            .filter(|event| !event.trim().is_empty())
        {
            soul.world.recent_events.push(event.trim().to_string());
            if soul.world.recent_events.len() > 12 {
                let remove_count = soul.world.recent_events.len() - 12;
                soul.world.recent_events.drain(0..remove_count);
            }
        }

        soul.last_updated = current_timestamp() as i64;
    }
}

pub fn parse_hidden_state(raw: &str) -> Result<ParsedProviderResponse, serde_json::Error> {
    let Some(start) = raw.find("[HIDDEN_STATE]") else {
        return Ok(ParsedProviderResponse {
            visible_text: raw.trim().to_string(),
            hidden_state: HiddenState::default(),
        });
    };
    let visible_text = raw[..start].trim().to_string();
    let hidden_part = raw[start + "[HIDDEN_STATE]".len()..].trim();
    let hidden_state = serde_json::from_str(hidden_part)?;
    Ok(ParsedProviderResponse {
        visible_text,
        hidden_state,
    })
}

fn default_relationship() -> Relationship {
    Relationship {
        trust: 0.0,
        affection: 0.0,
        intimacy: 0.0,
        passion: 0.0,
        commitment: 0.0,
        fear: 0.0,
        desire: 0.0,
        love_type: String::new(),
    }
}

fn clamp_stat(value: f32) -> f32 {
    value.clamp(0.0, 300.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::new_default_soul;

    #[test]
    fn strips_and_parses_hidden_state() {
        let raw = r#"Visible line.

[HIDDEN_STATE]
{"memory":"A promise mattered.","tag":"trust_building","trust_delta":3,"affection_delta":2,"world_event":"A promise landed."}"#;

        let parsed = parse_hidden_state(raw).expect("parsed");
        assert_eq!(parsed.visible_text, "Visible line.");
        assert_eq!(parsed.hidden_state.tag.as_deref(), Some("trust_building"));
        assert_eq!(parsed.hidden_state.trust_delta, Some(3.0));
    }

    #[test]
    fn accepts_visible_only_response() {
        let parsed = parse_hidden_state("Only visible.").expect("parsed");
        assert_eq!(parsed.visible_text, "Only visible.");
        assert!(parsed.hidden_state.memory.is_none());
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
        };

        state.apply_to_soul(&mut soul);

        assert_eq!(soul.relationships["user"].trust, 14.0);
        assert_eq!(soul.memory.recent.len(), 1);
        assert_eq!(soul.world.recent_events.len(), 1);
    }
}

