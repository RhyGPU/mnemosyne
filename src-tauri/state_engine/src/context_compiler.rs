use serde::{Deserialize, Serialize};

use crate::soul::Soul;

const TARGET_TOKEN_BUDGET: usize = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPreview {
    pub text: String,
    pub estimated_tokens: usize,
    pub truncated: bool,
}

pub fn compile_context_for_messages(soul: &Soul, messages: &[ContextMessage]) -> ContextPreview {
    let relationship = soul.relationships.get("user");
    let mut sections = Vec::new();

    sections.push(format!(
        "[CURRENT STATE]\nLocation: {}\nActive Plot: {}\nTime: {}.",
        soul.world.location,
        soul.world.active_plots.join(". "),
        soul.world.time_elapsed
    ));

    let mut memory_lines = Vec::new();
    for memory in soul.memory.core.iter().take(5) {
        memory_lines.push(format!("Core: {memory}"));
    }
    for schema in soul.memory.schemas.iter().take(4) {
        memory_lines.push(format!(
            "Schema: {}: {}",
            schema.schema_type, schema.summary
        ));
    }
    sections.push(format!("[CHARACTER MEMORY]\n{}", memory_lines.join("\n")));

    let mut recent_lines = soul
        .world
        .recent_events
        .iter()
        .rev()
        .take(5)
        .map(|event| format!("- {event}"))
        .collect::<Vec<_>>();
    recent_lines.reverse();
    if recent_lines.is_empty() {
        recent_lines.push("- No major recent events yet.".into());
    }
    sections.push(format!("[RECENT EVENTS]\n{}", recent_lines.join("\n")));

    if let Some(relationship) = relationship {
        sections.push(format!(
            "[RELATIONSHIP]\nTrust toward user: {}. Affection: {}. Fear: {}. Desire: {}.",
            relationship.trust, relationship.affection, relationship.fear, relationship.desire
        ));
    }

    let recent_chat = messages
        .iter()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>();
    if !recent_chat.is_empty() {
        sections.push(format!("[RECENT CHAT]\n{}", recent_chat.join("\n")));
    }

    let mut text = sections.join("\n\n");
    let mut truncated = false;
    while estimate_tokens(&text) > TARGET_TOKEN_BUDGET {
        truncated = true;
        if let Some(last_break) = text.rfind('\n') {
            text.truncate(last_break);
        } else {
            text.truncate(TARGET_TOKEN_BUDGET * 4);
            break;
        }
    }

    ContextPreview {
        estimated_tokens: estimate_tokens(&text),
        text,
        truncated,
    }
}

pub fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() / 4).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::new_default_soul;

    #[test]
    fn context_contains_required_sections() {
        let soul = new_default_soul("Aurora");
        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.text.contains("[CURRENT STATE]"));
        assert!(preview.text.contains("[CHARACTER MEMORY]"));
        assert!(preview.text.contains("[RECENT EVENTS]"));
        assert!(preview.text.contains("[RELATIONSHIP]"));
    }

    #[test]
    fn context_respects_budget() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.core = (0..100)
            .map(|index| format!("Long memory {index} {}", "x".repeat(500)))
            .collect();
        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.estimated_tokens <= 2_000);
        assert!(preview.truncated);
    }
}

