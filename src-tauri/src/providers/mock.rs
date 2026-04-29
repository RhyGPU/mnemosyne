use state_engine::{
    hidden_state::{encode_hidden_state, HiddenState},
    soul::Soul,
};

#[derive(Debug, Default)]
pub struct MockProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NarrativeMode {
    Reader,
    Realistic,
    God,
    Custom,
}

#[derive(Debug, Clone, Copy)]
struct MockTemplate {
    tag: &'static str,
    trust_delta: f32,
    affection_delta: f32,
    reader_line: &'static str,
    realistic_line: &'static str,
    god_line: &'static str,
    memory_frame: &'static str,
    world_frame: &'static str,
}

impl MockProvider {
    pub fn complete(&self, soul: &Soul, context: &str, user_text: &str, mode: &str) -> String {
        let trimmed = user_text.trim();
        let template = template_for(classify_tag(trimmed));
        let mode = NarrativeMode::from_label(mode);
        let relationship_hint = relationship_hint(soul);
        let context_hint = context_hint(context);
        let response = render_visible_response(soul, trimmed, mode, template, relationship_hint);
        let memory = render_memory(soul, trimmed, template, context_hint);
        let world_event = render_world_event(trimmed, template);
        let hidden_state = HiddenState {
            memory: Some(memory),
            tag: Some(template.tag.into()),
            trust_delta: Some(template.trust_delta),
            affection_delta: Some(template.affection_delta),
            world_event: Some(world_event),
        };

        format!(
            "{response}\n\n[HIDDEN_STATE]\n{}",
            encode_hidden_state(&hidden_state)
        )
    }
}

impl NarrativeMode {
    fn from_label(label: &str) -> Self {
        match label.trim().to_lowercase().as_str() {
            "realistic" => Self::Realistic,
            "god" => Self::God,
            "custom" => Self::Custom,
            _ => Self::Reader,
        }
    }
}

fn classify_tag(text: &str) -> MockTag {
    let lower = text.to_lowercase();
    if lower.contains("trust") || lower.contains("promise") || lower.contains("safe") {
        MockTag::TrustBuilding
    } else if lower.contains("hurt") || lower.contains("blood") || lower.contains("danger") {
        MockTag::Threat
    } else if lower.contains("remember")
        || lower.contains("childhood")
        || lower.contains("together")
    {
        MockTag::Bonding
    } else if lower.contains("where") || lower.contains("look") || lower.contains("room") {
        MockTag::Orientation
    } else {
        MockTag::Observation
    }
}

#[derive(Debug, Clone, Copy)]
enum MockTag {
    TrustBuilding,
    Threat,
    Bonding,
    Orientation,
    Observation,
}

fn template_for(tag: MockTag) -> MockTemplate {
    match tag {
        MockTag::TrustBuilding => MockTemplate {
            tag: "trust_building",
            trust_delta: 3.0,
            affection_delta: 1.0,
            reader_line: "The promise lands softly; she tests whether it can hold weight.",
            realistic_line: "She studies the promise for a long second before letting her shoulders loosen.",
            god_line: "Trust advances, but only by a careful increment; the scene remains emotionally fragile.",
            memory_frame: "A safety promise shifted the emotional baseline",
            world_frame: "A small promise of safety changed the room's emotional pressure",
        },
        MockTag::Threat => MockTemplate {
            tag: "threat",
            trust_delta: 0.0,
            affection_delta: 0.0,
            reader_line: "Her attention snaps sharp, and old alarm-patterns wake behind her eyes.",
            realistic_line: "She goes still and starts cataloging exits, distance, and anything that could become cover.",
            god_line: "Threat pressure rises; immediate survival logic begins overriding softer goals.",
            memory_frame: "A perceived danger forced a defensive read of the scene",
            world_frame: "The scene tightened around a possible danger",
        },
        MockTag::Bonding => MockTemplate {
            tag: "bonding",
            trust_delta: 1.0,
            affection_delta: 3.0,
            reader_line: "The shared thread of memory draws guarded warmth into her posture.",
            realistic_line: "She lets the memory sit between you, guarded but visibly affected by it.",
            god_line: "Bonding increases; shared memory becomes a usable emotional anchor.",
            memory_frame: "A shared memory created a warmer bond",
            world_frame: "The exchange became more intimate through remembered detail",
        },
        MockTag::Orientation => MockTemplate {
            tag: "orientation",
            trust_delta: 1.0,
            affection_delta: 0.5,
            reader_line: "She follows the details carefully, building a map from each concrete cue.",
            realistic_line: "She asks for specifics, anchoring herself in location, exits, and visible objects.",
            god_line: "Orientation improves; the character has more usable scene information.",
            memory_frame: "New scene information improved orientation",
            world_frame: "The scene gained clearer spatial definition",
        },
        MockTag::Observation => MockTemplate {
            tag: "observation",
            trust_delta: 1.0,
            affection_delta: 1.0,
            reader_line: "She listens, not fully relaxed, but present enough to stay in the exchange.",
            realistic_line: "She acknowledges the turn with measured focus and keeps the exchange grounded.",
            god_line: "A neutral exchange is recorded; no major state axis shifts dramatically.",
            memory_frame: "A neutral exchange added texture to the relationship",
            world_frame: "The conversation continued without a major rupture",
        },
    }
}

fn render_visible_response(
    soul: &Soul,
    user_text: &str,
    mode: NarrativeMode,
    template: MockTemplate,
    relationship_hint: &'static str,
) -> String {
    let mode_line = match mode {
        NarrativeMode::Reader => template.reader_line,
        NarrativeMode::Realistic => template.realistic_line,
        NarrativeMode::God => template.god_line,
        NarrativeMode::Custom => template.reader_line,
    };
    let response_note = if user_text.ends_with('?') {
        "The question narrows her attention; uncertainty stays visible, but she keeps tracking the scene."
    } else if user_text.is_empty() {
        "The silence leaves her guarded, waiting for a clearer cue."
    } else {
        "The turn lands; she absorbs it without retreating from the moment."
    };

    match mode {
        NarrativeMode::God => format!(
            "{mode_line}\n\n{} steadies in the scene. Relationship read: {relationship_hint}. {response_note}",
            soul.character_name
        ),
        NarrativeMode::Realistic => format!(
            "{mode_line}\n\n{} answers only through visible restraint: a controlled breath, a lowered chin, eyes measuring the room. {response_note}",
            soul.character_name
        ),
        NarrativeMode::Reader | NarrativeMode::Custom => format!(
            "{mode_line}\n\n{}'s awareness stays close to the surface of the scene. {response_note}",
            soul.character_name
        ),
    }
}

fn render_memory(
    soul: &Soul,
    user_text: &str,
    template: MockTemplate,
    context_hint: &'static str,
) -> String {
    format!(
        "{} for {}. User turn: {}. Context cue: {}.",
        template.memory_frame, soul.character_name, user_text, context_hint
    )
}

fn render_world_event(user_text: &str, template: MockTemplate) -> String {
    format!("{}: {}", template.world_frame, user_text)
}

fn relationship_hint(soul: &Soul) -> &'static str {
    let Some(relationship) = soul.relationships.get("user") else {
        return "unestablished";
    };
    if relationship.trust >= 45.0 {
        "warming trust"
    } else if relationship.fear >= 40.0 {
        "guarded fear"
    } else {
        "fragile neutrality"
    }
}

fn context_hint(context: &str) -> &'static str {
    if context.contains("[RECENT CHAT]") {
        "recent chat is available"
    } else {
        "fresh scene context"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state_engine::{hidden_state::parse_hidden_state, soul::new_default_soul};

    #[test]
    fn mock_provider_outputs_hidden_state() {
        let soul = new_default_soul("Aurora");
        let provider = MockProvider;
        let raw = provider.complete(
            &soul,
            "[CURRENT STATE]",
            "I promise this is safe.",
            "Reader",
        );
        let parsed = parse_hidden_state(&raw).expect("hidden state");

        assert!(parsed.visible_text.contains("Aurora"));
        assert_eq!(parsed.hidden_state.tag.as_deref(), Some("trust_building"));
        assert_eq!(parsed.hidden_state.trust_delta, Some(3.0));
    }

    #[test]
    fn god_mode_marks_gm_response() {
        let soul = new_default_soul("Aurora");
        let provider = MockProvider;
        let raw = provider.complete(&soul, "[CURRENT STATE]", "Where are we?", "God");
        let parsed = parse_hidden_state(&raw).expect("hidden state");

        assert!(!parsed.visible_text.starts_with("[GM]"));
        assert!(parsed.visible_text.contains("Orientation improves"));
        assert!(!parsed.visible_text.contains("\""));
        assert_eq!(parsed.hidden_state.tag.as_deref(), Some("orientation"));
        assert!(!raw.contains("\"tag\""));
    }

    #[test]
    fn mock_provider_uses_third_person_narration() {
        let soul = new_default_soul("Aurora");
        let provider = MockProvider;
        let raw = provider.complete(
            &soul,
            "[CURRENT STATE]",
            "I promise this is safe.",
            "Reader",
        );
        let parsed = parse_hidden_state(&raw).expect("hidden state");

        assert!(parsed.visible_text.contains("Aurora"));
        assert!(!parsed.visible_text.contains("\"I "));
        assert!(!parsed.visible_text.contains("I heard you"));
    }
}
