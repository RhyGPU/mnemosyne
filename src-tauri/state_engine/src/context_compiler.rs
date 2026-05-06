use std::cmp::Ordering;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::soul::{MemoryEntry, Soul};

const DEFAULT_TOKEN_BUDGET: usize = 2_500;
const MIN_RECENT_MEMORY_SALIENCE: f32 = 65.0;
const MIN_RELEVANT_MEMORY_SALIENCE: f32 = 45.0;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub max_tokens: usize,
    pub current_state_tokens: usize,
    pub profile_tokens: usize,
    pub memory_tokens: usize,
    pub world_tokens: usize,
    pub relationship_tokens: usize,
    pub recent_chat_tokens: usize,
}

#[derive(Debug, Clone)]
struct BuiltSection {
    text: String,
    truncated: bool,
}

#[derive(Debug, Clone)]
struct ScoredMemory<'a> {
    memory: &'a MemoryEntry,
    score: f32,
    repetitive: bool,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_tokens: DEFAULT_TOKEN_BUDGET,
            current_state_tokens: 300,
            profile_tokens: 350,
            memory_tokens: 650,
            world_tokens: 450,
            relationship_tokens: 250,
            recent_chat_tokens: 750,
        }
    }
}

pub fn compile_context_for_messages(soul: &Soul, messages: &[ContextMessage]) -> ContextPreview {
    compile_context_with_budget(soul, messages, &ContextBudget::default())
}

pub fn compile_context_with_budget(
    soul: &Soul,
    messages: &[ContextMessage],
    budget: &ContextBudget,
) -> ContextPreview {
    let mut truncated = false;
    let section_builders = [
        build_current_state_section(soul, budget),
        build_profile_section(soul, budget),
        build_world_section(soul, budget),
        build_memory_section(soul, messages, budget),
        build_relationship_section(soul, budget),
        build_recent_chat_section(messages, budget),
    ];

    let mut sections = Vec::new();
    for section in section_builders {
        truncated |= section.truncated;
        if !section.text.trim().is_empty() {
            sections.push(section.text);
        }
    }

    let mut text = sections.join("\n\n");
    while estimate_tokens(&text) > budget.max_tokens {
        truncated = true;
        if let Some(last_break) = text.rfind('\n') {
            text.truncate(last_break);
        } else {
            text = truncate_to_token_budget(&text, budget.max_tokens);
            break;
        }
    }

    ContextPreview {
        estimated_tokens: estimate_tokens(&text),
        text,
        truncated,
    }
}

fn build_current_state_section(soul: &Soul, budget: &ContextBudget) -> BuiltSection {
    let lines = vec![
        format!("Character: {}", fallback(&soul.character_name, "Unnamed Character")),
        format!("Turn: {}", soul.turn_counter),
        format!(
            "Psyche: dev stage {}, attachment style {}, fear baseline {:.0}, resolve {:.0}, shame {:.0}, openness {:.0}.",
            soul.global.dev_stage,
            soul.global.attach_style,
            soul.global.fear_baseline,
            soul.global.resolve,
            soul.global.shame,
            soul.global.openness,
        ),
        format!(
            "Needs: physiological {:.0}, safety {:.0}, belonging {:.0}, esteem {:.0}, actualization {:.0}.",
            soul.global.maslow[0],
            soul.global.maslow[1],
            soul.global.maslow[2],
            soul.global.maslow[3],
            soul.global.maslow[4],
        ),
        format!(
            "Trauma: phase {}, hypervigilance {:.0}, flashbacks {:.0}, numbing {:.0}, avoidance {:.0}.",
            soul.trauma.phase,
            soul.trauma.symptoms.hypervigilance,
            soul.trauma.symptoms.flashbacks,
            soul.trauma.symptoms.numbing,
            soul.trauma.symptoms.avoidance,
        ),
        format!("Body/arousal continuity: {}", soul.arousal.summary()),
    ];

    section_from_lines(
        "[CURRENT STATE]",
        lines,
        budget.current_state_tokens.min(budget.max_tokens),
    )
}

fn build_profile_section(soul: &Soul, budget: &ContextBudget) -> BuiltSection {
    let mut lines = Vec::new();
    push_if_present(
        &mut lines,
        "Description",
        soul.profile.description.trim(),
    );
    push_if_present(&mut lines, "Appearance", soul.profile.appearance.trim());
    push_if_present(
        &mut lines,
        "Personality",
        soul.profile.personality.trim(),
    );
    push_if_present(&mut lines, "Scenario seed", soul.profile.scenario.trim());

    if lines.is_empty() {
        lines.push("Profile is still sparse; rely on current state, memory, and scene continuity.".into());
    }

    section_from_lines(
        "[CHARACTER SNAPSHOT]",
        lines,
        budget.profile_tokens.min(budget.max_tokens),
    )
}

fn build_memory_section(
    soul: &Soul,
    messages: &[ContextMessage],
    budget: &ContextBudget,
) -> BuiltSection {
    let mut lines = Vec::new();
    for memory in soul.memory.core.iter().filter_map(|memory| clean(memory)) {
        lines.push(format!("Core: {memory}"));
    }
    for schema in &soul.memory.schemas {
        if let Some(summary) = clean(&schema.summary) {
            lines.push(format!(
                "Schema: {} (seen {}x): {}",
                fallback(&schema.schema_type, "pattern"),
                schema.count,
                summary
            ));
        }
    }

    let query_terms = recent_chat_terms(messages);
    let mut selected_recent = soul
        .memory
        .recent
        .iter()
        .map(|memory| score_recent_memory(memory, &query_terms, soul.turn_counter))
        .filter(|memory| {
            memory.memory.salience >= MIN_RECENT_MEMORY_SALIENCE
                || memory.score >= 80.0
                || (!memory.repetitive && memory.memory.salience >= MIN_RELEVANT_MEMORY_SALIENCE)
        })
        .collect::<Vec<_>>();

    selected_recent.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
    });

    for scored in selected_recent {
        lines.push(format!(
            "Recent: [{} / salience {:.0}] {}",
            fallback(&scored.memory.tag, "memory"),
            scored.memory.salience,
            scored.memory.content.trim()
        ));
    }

    if lines.is_empty() {
        lines.push("No durable memories have been selected yet.".into());
    }

    section_from_lines(
        "[RELEVANT MEMORIES]",
        lines,
        budget.memory_tokens.min(budget.max_tokens),
    )
}

fn build_world_section(soul: &Soul, budget: &ContextBudget) -> BuiltSection {
    let mut lines = vec![
        format!("Location: {}", fallback(&soul.world.location, "Unspecified")),
        format!(
            "Time elapsed: {}",
            fallback(&soul.world.time_elapsed, "Unknown")
        ),
    ];

    lines.push(format_list(
        "Active plots",
        &soul.world.active_plots,
        "No active plot has been established.",
    ));
    lines.push(format_list(
        "Key objects",
        &soul.world.key_objects,
        "No key objects are being tracked.",
    ));

    let mut recent_events = soul
        .world
        .recent_events
        .iter()
        .rev()
        .take(8)
        .filter_map(|event| clean(event))
        .map(|event| format!("- {event}"))
        .collect::<Vec<_>>();
    recent_events.reverse();
    if recent_events.is_empty() {
        lines.push("Recent events: No major recent events yet.".into());
    } else {
        lines.push(format!("Recent events:\n{}", recent_events.join("\n")));
    }

    section_from_lines(
        "[WORLD SNAPSHOT]",
        lines,
        budget.world_tokens.min(budget.max_tokens),
    )
}

fn build_relationship_section(soul: &Soul, budget: &ContextBudget) -> BuiltSection {
    let Some(relationship) = soul.relationships.get("user") else {
        return section_from_lines(
            "[RELATIONSHIP]",
            vec!["No relationship state for the user has been established.".into()],
            budget.relationship_tokens.min(budget.max_tokens),
        );
    };

    let lines = vec![format!(
        "Toward user: trust {:.0}, affection {:.0}, intimacy {:.0}, passion {:.0}, commitment {:.0}, fear {:.0}, desire {:.0}. Label/style: {}.",
        relationship.trust,
        relationship.affection,
        relationship.intimacy,
        relationship.passion,
        relationship.commitment,
        relationship.fear,
        relationship.desire,
        fallback(&relationship.love_type, "not yet named"),
    )];

    section_from_lines(
        "[RELATIONSHIP]",
        lines,
        budget.relationship_tokens.min(budget.max_tokens),
    )
}

fn build_recent_chat_section(messages: &[ContextMessage], budget: &ContextBudget) -> BuiltSection {
    let recent_chat = messages
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .filter(|message| !message.content.trim().is_empty())
        .map(|message| {
            format!(
                "{}: {}",
                fallback(&message.role, "message"),
                message.content.trim()
            )
        })
        .collect::<Vec<_>>();

    if recent_chat.is_empty() {
        return BuiltSection {
            text: String::new(),
            truncated: false,
        };
    }

    section_from_lines(
        "[RECENT CHAT]",
        recent_chat,
        budget.recent_chat_tokens.min(budget.max_tokens),
    )
}

fn section_from_lines(header: &str, lines: Vec<String>, token_cap: usize) -> BuiltSection {
    let mut text = header.to_string();
    let mut truncated = false;

    for line in lines.into_iter().filter(|line| !line.trim().is_empty()) {
        let candidate = format!("{text}\n{line}");
        if estimate_tokens(&candidate) <= token_cap {
            text = candidate;
        } else {
            truncated = true;
            if text == header {
                text = format!(
                    "{header}\n{}",
                    truncate_to_token_budget(&line, token_cap.saturating_sub(estimate_tokens(header)))
                );
            }
            break;
        }
    }

    BuiltSection { text, truncated }
}

fn score_recent_memory<'a>(
    memory: &'a MemoryEntry,
    query_terms: &HashSet<String>,
    current_turn: u64,
) -> ScoredMemory<'a> {
    let memory_terms = token_set(&memory.content);
    let overlap = memory_terms
        .iter()
        .filter(|term| query_terms.contains(*term))
        .count() as f32;
    let recency_bonus = current_turn
        .checked_sub(memory.timestamp)
        .map(|age| if age <= 3 { 12.0 } else if age <= 10 { 6.0 } else { 0.0 })
        .unwrap_or(3.0);
    let repetitive = is_repetitive_low_value(memory);
    let repetition_penalty = if repetitive { 25.0 } else { 0.0 };
    let score =
        memory.salience + (memory.retrieval_strength * 0.35) + (overlap * 20.0) + recency_bonus
            - repetition_penalty;

    ScoredMemory {
        memory,
        score,
        repetitive,
    }
}

fn is_repetitive_low_value(memory: &MemoryEntry) -> bool {
    memory.salience < MIN_RECENT_MEMORY_SALIENCE
        && matches!(
            memory.tag.as_str(),
            "routine" | "small_talk" | "observation" | "minor_observation"
        )
}

fn recent_chat_terms(messages: &[ContextMessage]) -> HashSet<String> {
    messages
        .iter()
        .rev()
        .take(6)
        .flat_map(|message| token_set(&message.content))
        .collect()
}

fn token_set(text: &str) -> HashSet<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() > 2)
        .map(|token| token.to_lowercase())
        .collect()
}

fn format_list(label: &str, values: &[String], fallback_text: &str) -> String {
    let values = values.iter().filter_map(|value| clean(value)).collect::<Vec<_>>();
    if values.is_empty() {
        format!("{label}: {fallback_text}")
    } else {
        format!("{label}: {}", values.join("; "))
    }
}

fn push_if_present(lines: &mut Vec<String>, label: &str, value: &str) {
    if !value.trim().is_empty() {
        lines.push(format!("{label}: {}", value.trim()));
    }
}

fn clean(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn fallback<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    clean(value).unwrap_or(fallback)
}

fn truncate_to_token_budget(text: &str, token_cap: usize) -> String {
    if token_cap == 0 {
        return String::new();
    }
    text.chars().take(token_cap * 4).collect()
}

pub fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() / 4).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::{new_default_soul, MemoryEntry};

    #[test]
    fn context_contains_required_sections() {
        let soul = new_default_soul("Aurora");
        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.text.contains("[CURRENT STATE]"));
        assert!(preview.text.contains("[CHARACTER SNAPSHOT]"));
        assert!(preview.text.contains("[WORLD SNAPSHOT]"));
        assert!(preview.text.contains("[RELEVANT MEMORIES]"));
        assert!(preview.text.contains("[RELATIONSHIP]"));
    }

    #[test]
    fn context_respects_budget() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.core = (0..100)
            .map(|index| format!("Long memory {index} {}", "x".repeat(500)))
            .collect();
        let messages = (0..10)
            .map(|index| ContextMessage {
                role: "user".into(),
                content: format!("Long chat turn {index} {}", "x".repeat(2_000)),
            })
            .collect::<Vec<_>>();
        let budget = ContextBudget {
            max_tokens: 500,
            current_state_tokens: 100,
            profile_tokens: 80,
            memory_tokens: 120,
            world_tokens: 100,
            relationship_tokens: 80,
            recent_chat_tokens: 120,
        };

        let preview = compile_context_with_budget(&soul, &messages, &budget);

        assert!(preview.estimated_tokens <= budget.max_tokens);
        assert!(preview.truncated);
    }

    #[test]
    fn high_salience_recent_memories_are_included() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.recent.push(memory(
            "high",
            "Aurora found the brass key hidden under the chapel stone.",
            "orientation",
            92.0,
            80.0,
            1,
        ));

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.text.contains("brass key"));
    }

    #[test]
    fn low_salience_repetitive_memories_are_deprioritized() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.recent.push(memory(
            "low",
            "Aurora quietly noticed the room remained quiet again.",
            "observation",
            25.0,
            20.0,
            1,
        ));

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(!preview.text.contains("room remained quiet again"));
    }

    #[test]
    fn key_objects_and_active_plots_appear_in_world_section() {
        let mut soul = new_default_soul("Aurora");
        soul.world.location = "Carver City service tunnel".into();
        soul.world.active_plots = vec!["Open the locked gate".into(), "Avoid the patrol".into()];
        soul.world.key_objects = vec!["Rusty key".into(), "Signal lantern".into()];
        soul.world.recent_events = vec!["The gate mechanism clicked once.".into()];
        soul.world.time_elapsed = "Night 1, forty minutes after entry".into();

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.text.contains("Open the locked gate"));
        assert!(preview.text.contains("Rusty key"));
        assert!(preview.text.contains("Night 1"));
    }

    #[test]
    fn recent_chat_is_still_included() {
        let soul = new_default_soul("Aurora");
        let messages = vec![
            ContextMessage {
                role: "user".into(),
                content: "Do you remember the stairwell?".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Aurora glanced toward the locked door.".into(),
            },
        ];

        let preview = compile_context_for_messages(&soul, &messages);

        assert!(preview.text.contains("[RECENT CHAT]"));
        assert!(preview.text.contains("stairwell"));
        assert!(preview.text.contains("locked door"));
    }

    fn memory(
        id: &str,
        content: &str,
        tag: &str,
        salience: f32,
        retrieval_strength: f32,
        timestamp: u64,
    ) -> MemoryEntry {
        MemoryEntry {
            id: id.into(),
            timestamp,
            content: content.into(),
            salience,
            tag: tag.into(),
            retrieval_strength,
        }
    }
}
