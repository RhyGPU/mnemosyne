use std::cmp::Ordering;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::soul::{MemoryEntry, MemorySourceType, Soul};

const DEFAULT_TOKEN_BUDGET: usize = 2_500;
const MIN_RECENT_MEMORY_SALIENCE: f32 = 65.0;
const MIN_RELEVANT_MEMORY_SALIENCE: f32 = 45.0;
const ASSISTANT_RECENT_CHAT_CHARS: usize = 350;
const ASSISTANT_RECENT_CHAT_HEAD_CHARS: usize = 120;
const ASSISTANT_RECENT_CHAT_TAIL_CHARS: usize = 220;
const USER_RECENT_CHAT_CHARS: usize = 500;
const LATEST_ASSISTANT_EXCHANGE_CHARS: usize = 1_200;
const LATEST_USER_EXCHANGE_CHARS: usize = 1_000;
const LATEST_EXCHANGE_INSTRUCTION: &str = "Continue from this section first. If older context conflicts with this section, ignore older context. Continue from the final state of the last narrator response and the latest user input. Do not replay earlier beats.";
const CURRENT_USER_FOLLOWS_LINE: &str = "The current user message follows as the next user message.";
const FILLER_MEMORY_PHRASES: &[&str] = &[
    "neutral exchange added texture",
    "context cue",
    "recent chat is available",
    "fresh scene context",
];

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
    pub context_priority_tokens: usize,
    pub scene_state_tokens: usize,
    pub do_not_replay_tokens: usize,
    pub recent_chat_tokens: usize,
    pub latest_exchange_tokens: usize,
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
    source_restricted: bool,
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
            context_priority_tokens: 150,
            scene_state_tokens: 350,
            do_not_replay_tokens: 180,
            recent_chat_tokens: 500,
            latest_exchange_tokens: 700,
        }
    }
}

pub fn compile_context_for_messages(soul: &Soul, messages: &[ContextMessage]) -> ContextPreview {
    compile_context_with_budget(soul, messages, &ContextBudget::default())
}

pub fn compile_context_for_separate_user_message(
    soul: &Soul,
    messages: &[ContextMessage],
) -> ContextPreview {
    compile_context_with_budget_and_options(soul, messages, &ContextBudget::default(), true)
}

pub fn compile_context_with_budget(
    soul: &Soul,
    messages: &[ContextMessage],
    budget: &ContextBudget,
) -> ContextPreview {
    compile_context_with_budget_and_options(soul, messages, budget, false)
}

fn compile_context_with_budget_and_options(
    soul: &Soul,
    messages: &[ContextMessage],
    budget: &ContextBudget,
    separate_user_message_follows: bool,
) -> ContextPreview {
    let mut truncated = false;
    let section_builders = [
        build_world_section(soul, budget),
        build_profile_section(soul, budget),
        build_memory_section(soul, messages, budget),
        build_relationship_section(soul, budget),
        build_recent_chat_section(messages, budget),
        build_latest_exchange_section(messages, budget, separate_user_message_follows),
    ];

    let mut sections = Vec::new();
    for section in section_builders {
        truncated |= section.truncated;
        if !section.text.trim().is_empty() {
            sections.push(section.text);
        }
    }

    truncated |= compact_sections_to_budget(&mut sections, budget.max_tokens);
    let text = sections.join("\n\n");

    ContextPreview {
        estimated_tokens: estimate_tokens(&text),
        text,
        truncated,
    }
}

fn build_profile_section(soul: &Soul, budget: &ContextBudget) -> BuiltSection {
    let mut lines = vec![
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
    push_if_present(&mut lines, "Description", soul.profile.description.trim());
    push_if_present(&mut lines, "Appearance", soul.profile.appearance.trim());
    push_if_present(&mut lines, "Personality", soul.profile.personality.trim());
    push_if_present(&mut lines, "Scenario seed", soul.profile.scenario.trim());

    if lines.is_empty() {
        lines.push(
            "Profile is still sparse; rely on current state, memory, and scene continuity.".into(),
        );
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
    for memory in soul
        .memory
        .core
        .iter()
        .filter_map(|memory| clean(memory))
        .filter(|memory| !is_generic_filler_text(memory))
    {
        lines.push(format!("Core: {memory}"));
    }
    for schema in &soul.memory.schemas {
        if let Some(summary) = clean(&schema.summary) {
            if is_generic_filler_text(summary)
                || is_near_empty_generic_schema(&schema.schema_type, summary)
            {
                continue;
            }
            lines.push(format!(
                "Schema: {} (seen {}x): {}",
                fallback(&schema.schema_type, "pattern"),
                schema.count,
                summary
            ));
        }
    }

    let query_terms = recent_chat_terms(messages);
    let source_query_active = memory_source_query_active(messages);
    let mut selected_recent = soul
        .memory
        .recent
        .iter()
        .filter(|memory| !is_generic_filler_memory(memory))
        .map(|memory| {
            score_recent_memory(memory, &query_terms, soul.turn_counter, source_query_active)
        })
        .filter(|memory| !memory.source_restricted)
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
            memory_source_label(scored.memory),
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
        format!(
            "Location: {}",
            fallback(&soul.world.location, "Unspecified")
        ),
        format!(
            "Time elapsed: {}",
            normalize_time_elapsed_display(fallback(&soul.world.time_elapsed, "Unknown"))
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
    if soul.relationships.is_empty() {
        return section_from_lines(
            "[RELATIONSHIPS]",
            vec!["No relationship state has been established.".into()],
            budget.relationship_tokens.min(budget.max_tokens),
        );
    }

    let mut relationships = soul.relationships.iter().collect::<Vec<_>>();
    relationships.sort_by(|left, right| display_entity_id(left.0).cmp(&display_entity_id(right.0)));
    let lines = relationships
        .into_iter()
        .map(|(target, relationship)| {
            let mut metrics = vec![
                format!("trust {:.0}", relationship.trust),
                format!("affection {:.0}", relationship.affection),
                format!("intimacy {:.0}", relationship.intimacy),
                format!("passion {:.0}", relationship.passion),
                format!("commitment {:.0}", relationship.commitment),
                format!("fear {:.0}", relationship.fear),
                format!("desire {:.0}", relationship.desire),
            ];
            push_metric_if_nonzero(&mut metrics, "respect", relationship.respect);
            push_metric_if_nonzero(&mut metrics, "conflict", relationship.conflict);
            push_metric_if_nonzero(&mut metrics, "dependency", relationship.dependency);
            push_metric_if_nonzero(&mut metrics, "curiosity", relationship.curiosity);
            push_metric_if_nonzero(&mut metrics, "comfort", relationship.comfort);
            push_metric_if_nonzero(
                &mut metrics,
                "boundary_pressure",
                relationship.boundary_pressure,
            );
            format!(
                "{} -> {}: {}. Label/style: {}.",
                fallback(&soul.character_name, "Character"),
                display_entity_id(target),
                metrics.join(", "),
                fallback(&relationship.love_type, "not yet named"),
            )
        })
        .collect::<Vec<_>>();

    section_from_lines(
        "[RELATIONSHIPS]",
        lines,
        budget.relationship_tokens.min(budget.max_tokens),
    )
}

fn build_recent_chat_section(messages: &[ContextMessage], budget: &ContextBudget) -> BuiltSection {
    let message_count = if budget.recent_chat_tokens < 400 {
        4
    } else {
        6
    };
    let skip_indices = latest_exchange_message_indices(messages);
    let mut recent_chat = messages
        .iter()
        .enumerate()
        .rev()
        .filter(|(index, _)| !skip_indices.contains(index))
        .filter_map(|(_, message)| recent_chat_line(message))
        .take(message_count)
        .collect::<Vec<_>>();

    if recent_chat.is_empty() {
        return BuiltSection {
            text: String::new(),
            truncated: false,
        };
    }

    recent_chat.reverse();
    section_from_lines(
        "[RECENT CHAT, LOWER PRIORITY]",
        recent_chat,
        budget.recent_chat_tokens.min(budget.max_tokens),
    )
}

fn build_latest_exchange_section(
    messages: &[ContextMessage],
    budget: &ContextBudget,
    separate_user_message_follows: bool,
) -> BuiltSection {
    let last_assistant = last_message_with_role(messages, "assistant")
        .map(|message| {
            tail_excerpt(
                &sanitize_assistant_context(&message.content),
                LATEST_ASSISTANT_EXCHANGE_CHARS,
            )
        })
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| "No prior narrator response in available context.".into());

    let mut lines = vec![
        LATEST_EXCHANGE_INSTRUCTION.into(),
        format!("Last narrator response: {last_assistant}"),
    ];
    if separate_user_message_follows {
        lines.push(CURRENT_USER_FOLLOWS_LINE.into());
    } else {
        let latest_user = last_message_with_role(messages, "user")
            .map(|message| {
                excerpt(
                    &sanitize_message_content(&message.content),
                    LATEST_USER_EXCHANGE_CHARS,
                )
            })
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| "No latest user input in available context.".into());
        lines.push(format!("Latest user input: {latest_user}"));
    }

    section_from_lines(
        "[LATEST EXCHANGE, HIGH PRIORITY]",
        lines,
        budget.latest_exchange_tokens.min(budget.max_tokens),
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
                    truncate_to_token_budget(
                        &line,
                        token_cap.saturating_sub(estimate_tokens(header))
                    )
                );
            }
            break;
        }
    }

    BuiltSection { text, truncated }
}

fn compact_sections_to_budget(sections: &mut Vec<String>, max_tokens: usize) -> bool {
    let mut truncated = false;
    let trim_order = [
        "[RECENT CHAT, LOWER PRIORITY]",
        "[RELEVANT MEMORIES]",
        "[CHARACTER SNAPSHOT]",
        "[WORLD SNAPSHOT]",
        "[RELATIONSHIPS]",
        "[LATEST EXCHANGE, HIGH PRIORITY]",
    ];

    while estimate_tokens(&sections.join("\n\n")) > max_tokens {
        let mut trimmed = false;
        for header in trim_order {
            if let Some(section) = sections
                .iter_mut()
                .find(|section| section.starts_with(header))
            {
                if trim_last_line(section) {
                    truncated = true;
                    trimmed = true;
                    break;
                }
            }
        }

        if !trimmed {
            trim_to_priority_minimum(sections, max_tokens);
            truncated = true;
            break;
        }
    }

    truncated
}

fn trim_last_line(section: &mut String) -> bool {
    let Some(last_break) = section.rfind('\n') else {
        return false;
    };
    let header_only = !section[..last_break].contains('\n');
    if header_only {
        return false;
    }
    section.truncate(last_break);
    true
}

fn trim_to_priority_minimum(sections: &mut Vec<String>, max_tokens: usize) {
    sections.retain(|section| section.starts_with("[LATEST EXCHANGE, HIGH PRIORITY]"));

    while estimate_tokens(&sections.join("\n\n")) > max_tokens {
        if sections.iter_mut().rev().any(trim_last_line) {
            continue;
        }

        if let Some(section) = sections.first_mut() {
            let header = section
                .lines()
                .next()
                .unwrap_or("[LATEST EXCHANGE, HIGH PRIORITY]");
            let body = section
                .split_once('\n')
                .map(|(_, body)| body)
                .unwrap_or(section);
            *section = format!(
                "{header}\n{}",
                truncate_to_token_budget(body, max_tokens.saturating_sub(estimate_tokens(header)))
            );
        }
        break;
    }
}

fn score_recent_memory<'a>(
    memory: &'a MemoryEntry,
    query_terms: &HashSet<String>,
    current_turn: u64,
    source_query_active: bool,
) -> ScoredMemory<'a> {
    let memory_terms = token_set(&memory.content);
    let overlap = memory_terms
        .iter()
        .filter(|term| query_terms.contains(*term))
        .count() as f32;
    let recency_bonus = current_turn
        .checked_sub(memory.timestamp)
        .map(|age| {
            if age <= 3 {
                12.0
            } else if age <= 10 {
                6.0
            } else {
                0.0
            }
        })
        .unwrap_or(3.0);
    let repetitive = is_repetitive_low_value(memory);
    let repetition_penalty = if repetitive { 25.0 } else { 0.0 };
    let source_restricted = memory.source_type.imported_or_cross_session() && !source_query_active;
    let source_adjustment = memory_source_score_adjustment(memory, source_query_active);
    let confidence_penalty = memory
        .confidence
        .filter(|confidence| confidence.is_finite())
        .map(|confidence| if confidence < 0.55 { 20.0 } else { 0.0 })
        .unwrap_or(0.0);
    let score =
        memory.salience + (memory.retrieval_strength * 0.35) + (overlap * 20.0) + recency_bonus
            + source_adjustment
            - repetition_penalty
            - confidence_penalty;

    ScoredMemory {
        memory,
        score,
        repetitive,
        source_restricted,
    }
}

fn last_message_with_role<'a>(
    messages: &'a [ContextMessage],
    role: &str,
) -> Option<&'a ContextMessage> {
    messages
        .iter()
        .rev()
        .find(|message| message.role == role && !message.content.trim().is_empty())
}

fn latest_exchange_message_indices(messages: &[ContextMessage]) -> HashSet<usize> {
    let mut skip = HashSet::new();
    if let Some((index, _)) = messages
        .iter()
        .enumerate()
        .rev()
        .find(|(_, message)| message.role == "assistant" && !message.content.trim().is_empty())
    {
        skip.insert(index);
    }
    if let Some((index, _)) = messages
        .iter()
        .enumerate()
        .rev()
        .find(|(_, message)| message.role == "user" && !message.content.trim().is_empty())
    {
        skip.insert(index);
    }
    skip
}

fn recent_chat_line(message: &ContextMessage) -> Option<String> {
    let cleaned = match message.role.as_str() {
        "assistant" => sanitize_assistant_context(&message.content),
        _ => sanitize_message_content(&message.content),
    };
    if cleaned.is_empty() {
        return None;
    }

    let content = match message.role.as_str() {
        "assistant" => assistant_recent_chat_excerpt(&cleaned),
        "user" => excerpt(&cleaned, USER_RECENT_CHAT_CHARS),
        _ => excerpt(&cleaned, ASSISTANT_RECENT_CHAT_CHARS),
    };

    Some(format!(
        "{}: {}",
        fallback(&message.role, "message"),
        content
    ))
}

fn assistant_recent_chat_excerpt(text: &str) -> String {
    head_tail_excerpt(
        text,
        ASSISTANT_RECENT_CHAT_HEAD_CHARS,
        ASSISTANT_RECENT_CHAT_TAIL_CHARS,
        ASSISTANT_RECENT_CHAT_CHARS,
    )
}

fn sanitize_message_content(content: &str) -> String {
    strip_hidden_state_blocks(content).trim().to_string()
}

fn sanitize_assistant_context(content: &str) -> String {
    strip_status_blocks(&strip_hidden_state_blocks(content))
        .trim()
        .to_string()
}

fn strip_hidden_state_blocks(content: &str) -> String {
    let mut cleaned = content.to_string();
    loop {
        let Some(start) = cleaned.find("[HIDDEN STATE]") else {
            break;
        };
        if let Some(relative_end) = cleaned[start..].find("[/HIDDEN STATE]") {
            let end = start + relative_end + "[/HIDDEN STATE]".len();
            cleaned.replace_range(start..end, "");
        } else {
            cleaned.truncate(start);
            break;
        }
    }

    if let Some(start) = cleaned.find("[HIDDEN_STATE]") {
        cleaned.truncate(start);
    }
    cleaned
}

fn strip_status_blocks(content: &str) -> String {
    let mut cleaned = content.to_string();
    loop {
        let Some(start) = cleaned.find("```status") else {
            break;
        };
        let after_marker = start + "```status".len();
        if let Some(relative_end) = cleaned[after_marker..].find("```") {
            let end = after_marker + relative_end + "```".len();
            cleaned.replace_range(start..end, "");
        } else {
            cleaned.truncate(start);
            break;
        }
    }
    cleaned
}

fn is_repetitive_low_value(memory: &MemoryEntry) -> bool {
    memory.salience < MIN_RECENT_MEMORY_SALIENCE
        && matches!(
            memory.tag.as_str(),
            "routine" | "small_talk" | "observation" | "minor_observation"
        )
}

fn memory_source_score_adjustment(memory: &MemoryEntry, source_query_active: bool) -> f32 {
    match memory.source_type {
        MemorySourceType::CurrentSession => 18.0,
        MemorySourceType::PersistentCore => 14.0,
        MemorySourceType::UserClaimed => {
            if source_query_active {
                12.0
            } else {
                0.0
            }
        }
        MemorySourceType::ImportedLog | MemorySourceType::PreviousSession => {
            if source_query_active {
                8.0
            } else {
                -60.0
            }
        }
        MemorySourceType::CrossSessionBleed => {
            if source_query_active {
                6.0
            } else {
                -70.0
            }
        }
        MemorySourceType::NarratorInferred => -18.0,
        MemorySourceType::SystemGenerated | MemorySourceType::Unknown => 0.0,
    }
}

fn memory_source_label(memory: &MemoryEntry) -> String {
    let mut parts = vec![memory.source_type.as_label().to_string()];
    if !memory.is_lived_experience {
        parts.push("not lived".into());
    }
    if memory.is_imported_context && !parts.iter().any(|part| part == "imported") {
        parts.push("imported".into());
    }
    if memory
        .confidence
        .filter(|confidence| confidence.is_finite() && *confidence < 0.6)
        .is_some()
    {
        parts.push("uncertain".into());
    }
    parts.join(" / ")
}

fn is_generic_filler_memory(memory: &MemoryEntry) -> bool {
    is_generic_filler_text(&memory.content)
        || is_near_empty_generic_schema(&memory.tag, &memory.content)
}

fn is_generic_filler_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    FILLER_MEMORY_PHRASES
        .iter()
        .any(|phrase| lower.contains(phrase))
}

fn is_near_empty_generic_schema(kind: &str, summary: &str) -> bool {
    let kind = kind.to_lowercase();
    let summary = summary.trim();
    if summary.is_empty() {
        return true;
    }

    let tokens = token_set(summary);
    let generic_kind = matches!(
        kind.as_str(),
        "observation" | "minor_observation" | "routine" | "small_talk" | "pattern"
    );
    generic_kind && (tokens.len() <= 4 || summary.to_lowercase().contains("recurring pattern"))
}

fn recent_chat_terms(messages: &[ContextMessage]) -> HashSet<String> {
    messages
        .iter()
        .rev()
        .take(6)
        .flat_map(|message| token_set(&message.content))
        .collect()
}

fn memory_source_query_active(messages: &[ContextMessage]) -> bool {
    let text = messages
        .iter()
        .rev()
        .take(4)
        .map(|message| message.content.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    [
        "imported log",
        "chat log",
        "previous session",
        "prior session",
        "another aurora",
        "another version",
        "cross-session",
        "cross session",
        "memory bleed",
        "archived chat",
        "old session",
        "other session",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn token_set(text: &str) -> HashSet<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() > 2)
        .map(|token| token.to_lowercase())
        .collect()
}

fn format_list(label: &str, values: &[String], fallback_text: &str) -> String {
    let values = values
        .iter()
        .filter_map(|value| clean(value))
        .collect::<Vec<_>>();
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

fn display_entity_id(entity_id: &str) -> String {
    let trimmed = entity_id.trim();
    if trimmed.eq_ignore_ascii_case("user") || trimmed.eq_ignore_ascii_case("default_player") {
        return "default_player".into();
    }
    if trimmed.is_empty() {
        return "unknown_speaker".into();
    }
    trimmed.to_string()
}

fn push_metric_if_nonzero(metrics: &mut Vec<String>, label: &str, value: f32) {
    if value.abs() >= 0.5 {
        metrics.push(format!("{label} {:.0}", value));
    }
}

fn excerpt(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let take_chars = max_chars.saturating_sub(3);
    format!("{}...", text.chars().take(take_chars).collect::<String>())
}

fn tail_excerpt(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }

    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    let ellipsis = "...";
    let tail_len = max_chars.saturating_sub(ellipsis.chars().count());
    let drop = char_count.saturating_sub(tail_len);
    let tail = text.chars().skip(drop).collect::<String>();
    format!("{ellipsis}{tail}")
}

fn head_tail_excerpt(text: &str, head_chars: usize, tail_chars: usize, max_chars: usize) -> String {
    let text = text.trim();
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    let separator = " ... ";
    let separator_len = separator.chars().count();
    let available = max_chars.saturating_sub(separator_len);
    let head_len = head_chars.min(available);
    let tail_len = tail_chars.min(available.saturating_sub(head_len));
    if head_len == 0 || tail_len == 0 {
        return tail_excerpt(text, max_chars);
    }

    let head = text.chars().take(head_len).collect::<String>();
    let tail_start = char_count.saturating_sub(tail_len);
    let tail = text.chars().skip(tail_start).collect::<String>();
    format!("{head}{separator}{tail}")
}

fn normalize_time_elapsed_display(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Unknown".into();
    }

    const PREFIX: &str = "Session start";
    let Some(found) = trimmed.find(PREFIX) else {
        return trimmed.to_string();
    };
    let after_prefix = found + PREFIX.len();
    let Some(rest) = trimmed.get(after_prefix..) else {
        return trimmed.to_string();
    };
    let Some(next) = rest.chars().next() else {
        return trimmed.to_string();
    };
    let needs_boundary = !next.is_whitespace() && !matches!(next, '.' | ',' | ';' | ':');
    let looks_glued_continuation =
        needs_boundary && (next.is_ascii_uppercase() || next.is_ascii_digit());

    if !looks_glued_continuation {
        return trimmed.to_string();
    }

    let mut out = String::with_capacity(trimmed.len().saturating_add(2));
    out.push_str(&trimmed[..after_prefix]);
    out.push_str(". ");
    out.push_str(rest);
    out
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
    use crate::soul::{new_default_soul, MemoryEntry, SchemaEntry};

    #[test]
    fn context_contains_required_sections() {
        let soul = new_default_soul("Aurora");
        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.text.contains("[WORLD SNAPSHOT]"));
        assert!(preview.text.contains("[CHARACTER SNAPSHOT]"));
        assert!(preview.text.contains("[RELEVANT MEMORIES]"));
        assert!(preview.text.contains("[RELATIONSHIPS]"));
        assert!(preview.text.contains("[LATEST EXCHANGE, HIGH PRIORITY]"));
    }

    #[test]
    fn relationship_section_lists_directed_targets() {
        let mut soul = new_default_soul("Aurora");
        let mut junhwa = soul.relationships["user"].clone();
        junhwa.trust = 8.0;
        junhwa.affection = 18.0;
        junhwa.fear = 35.0;
        junhwa.dependency = 70.0;
        junhwa.conflict = 60.0;
        soul.relationships.insert("junhwa".into(), junhwa);
        let mut rhy = soul.relationships["user"].clone();
        rhy.trust = 16.0;
        rhy.affection = 22.0;
        rhy.fear = 8.0;
        rhy.curiosity = 35.0;
        rhy.comfort = 20.0;
        soul.relationships.insert("rhy".into(), rhy);

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.text.contains("[RELATIONSHIPS]"));
        assert!(preview
            .text
            .contains("Aurora -> junhwa: trust 8, affection 18"));
        assert!(preview.text.contains("fear 35"));
        assert!(preview.text.contains("dependency 70"));
        assert!(preview
            .text
            .contains("Aurora -> rhy: trust 16, affection 22"));
        assert!(preview.text.contains("curiosity 35"));
    }

    #[test]
    fn world_snapshot_appears_before_character_snapshot() {
        let soul = soul_with_phone_scene();
        let preview = compile_context_for_messages(&soul, &phone_continuity_messages());

        assert_order(&preview.text, "[WORLD SNAPSHOT]", "[CHARACTER SNAPSHOT]");
    }

    #[test]
    fn latest_exchange_appears_after_lower_priority_recent_chat() {
        let soul = soul_with_phone_scene();
        let preview = compile_context_for_messages(&soul, &phone_continuity_messages());

        assert_order(
            &preview.text,
            "[RECENT CHAT, LOWER PRIORITY]",
            "[LATEST EXCHANGE, HIGH PRIORITY]",
        );
    }

    #[test]
    fn latest_exchange_contains_conflict_override_instruction() {
        let soul = soul_with_phone_scene();
        let preview = compile_context_for_messages(&soul, &phone_continuity_messages());
        let latest_exchange = section_text(&preview.text, "[LATEST EXCHANGE, HIGH PRIORITY]");
        let recent_chat = section_text(&preview.text, "[RECENT CHAT, LOWER PRIORITY]");

        assert!(latest_exchange.contains("Continue from this section first."));
        assert!(latest_exchange.contains("ignore older context"));
        assert!(latest_exchange.contains("Do not replay earlier beats."));
        assert!(latest_exchange.contains("tossed it onto the couch"));
        assert!(latest_exchange.contains("lonely too"));
        assert!(
            recent_chat.contains("phone"),
            "expected older-turn recent chat excerpt, got: {recent_chat}"
        );
        assert!(
            !recent_chat.contains("lonely too"),
            "recent chat must not duplicate the latest user message"
        );
        assert!(
            !recent_chat.contains("tossed it onto"),
            "recent chat must not duplicate the latest assistant excerpt"
        );
    }

    #[test]
    fn latest_exchange_omits_current_user_when_separate_message_follows() {
        let soul = soul_with_phone_scene();
        let preview =
            compile_context_for_separate_user_message(&soul, &phone_continuity_messages());
        let latest_exchange = section_text(&preview.text, "[LATEST EXCHANGE, HIGH PRIORITY]");

        assert!(latest_exchange.contains("tossed it onto the couch"));
        assert!(latest_exchange.contains(CURRENT_USER_FOLLOWS_LINE));
        assert!(latest_exchange.contains("Do not replay earlier beats."));
        assert!(
            !latest_exchange.contains("lonely too"),
            "latest exchange should not repeat the separate user message: {latest_exchange}"
        );
    }

    #[test]
    fn latest_exchange_prefers_tail_of_long_assistant() {
        let soul = new_default_soul("Aurora");
        let long_opening = (0..120)
            .map(|beat| format!("Beat {beat}: the radiator ticks while the kettle waits. "))
            .collect::<String>();
        let closing =
            "Aurora set the phone on the couch, crossed to the kitchen, and reached for the takeout containers.";
        let messages = vec![
            ContextMessage {
                role: "user".into(),
                content: "Open the fridge.".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: format!("{long_opening}{closing}"),
            },
        ];

        let preview = compile_context_for_messages(&soul, &messages);
        let latest_exchange = section_text(&preview.text, "[LATEST EXCHANGE, HIGH PRIORITY]");

        assert!(
            latest_exchange.contains("takeout containers"),
            "latest narrator excerpt should preserve the ending: {latest_exchange}"
        );
        assert!(
            !latest_exchange.contains("Beat 000:"),
            "latest narrator excerpt must not anchor on the opening beats: {latest_exchange}"
        );
    }

    #[test]
    fn latest_exchange_tail_strips_assistant_status() {
        let soul = new_default_soul("Aurora");
        let pad = format!("{}. ", "Echo");
        let body = pad.repeat(500);
        let tail = "Aurora left the handset on the table and drifted toward the kitchen.";
        let messages = vec![ContextMessage {
            role: "assistant".into(),
            content: format!("{body}{tail}\n```status\nAurora | Skin: flushed | Zones: hands\n```"),
        }];

        let preview = compile_context_for_messages(&soul, &messages);
        let latest_exchange = section_text(&preview.text, "[LATEST EXCHANGE, HIGH PRIORITY]");

        assert!(latest_exchange.contains("kitchen."));
        assert!(!latest_exchange.contains("```status"));
        assert!(!latest_exchange.contains("Skin: flushed"));
    }

    #[test]
    fn tail_excerpt_prefix_ellipsis_is_utf8_safe() {
        let opening = format!("{}. ", "x".repeat(200));
        let tail = format!("{}Closer — 안녕 🙂", "→".repeat(400));
        let text = format!("{opening}{tail}");
        let trimmed = tail_excerpt(&text, 140);

        assert!(trimmed.starts_with("..."));
        assert!(trimmed.is_char_boundary(trimmed.len()));
        assert!(trimmed.contains('—'));
        assert!(trimmed.contains("안녕"));
        assert!(trimmed.contains('🙂'));
        assert!(trimmed.contains('→'));
    }

    #[test]
    fn time_elapsed_fixes_glued_session_start() {
        let mut soul = soul_with_phone_scene();
        soul.world.time_elapsed = "Session startLate evening, just after midnight.".into();

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(
            preview.text.contains("Session start. Late evening"),
            "world snapshot malformed time string: {}",
            preview.text
        );
        assert!(!preview.text.contains("Session startLate"));
    }

    #[test]
    fn assistant_status_blocks_are_stripped_from_recent_chat_context() {
        let soul = new_default_soul("Aurora");
        let messages = vec![
            ContextMessage {
                role: "user".into(),
                content: "What changed?".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Aurora sets the phone down.\n```status\nAurora | Skin: pale | Zones: hands | Atmosphere: tense\n```".into(),
            },
            ContextMessage {
                role: "user".into(),
                content: "Latest user.".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Later beat.".into(),
            },
        ];

        let preview = compile_context_for_messages(&soul, &messages);
        let recent_chat = section_text(&preview.text, "[RECENT CHAT, LOWER PRIORITY]");

        assert!(recent_chat.contains("Aurora sets the phone down."));
        assert!(!recent_chat.contains("```status"));
        assert!(!recent_chat.contains("Skin: pale"));
    }

    #[test]
    fn assistant_messages_are_compacted() {
        let soul = new_default_soul("Aurora");
        let final_state = "Final state: Aurora has moved to the kitchen doorway.";
        let messages = vec![
            ContextMessage {
                role: "user".into(),
                content: "First user turn.".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: format!("Aurora continues. {} {final_state}", "a".repeat(900)),
            },
            ContextMessage {
                role: "user".into(),
                content: "Latest user.".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Short latest.".into(),
            },
        ];

        let preview = compile_context_for_messages(&soul, &messages);
        let recent_chat = section_text(&preview.text, "[RECENT CHAT, LOWER PRIORITY]");

        assert!(recent_chat.contains("..."));
        assert!(recent_chat.contains(final_state));
        assert!(!recent_chat.contains(&"a".repeat(500)));
    }

    #[test]
    fn user_messages_are_preserved_with_higher_cap() {
        let soul = new_default_soul("Aurora");
        let user_content = format!("I explain the lonely confession. {}", "u".repeat(460));
        let assistant_content = format!("Aurora listens. {}", "a".repeat(460));
        let messages = vec![
            ContextMessage {
                role: "assistant".into(),
                content: "Aurora taps her fingers once.".into(),
            },
            ContextMessage {
                role: "user".into(),
                content: user_content.clone(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: assistant_content,
            },
            ContextMessage {
                role: "user".into(),
                content: "Latest beat.".into(),
            },
        ];

        let preview = compile_context_for_messages(&soul, &messages);
        let recent_chat = section_text(&preview.text, "[RECENT CHAT, LOWER PRIORITY]");

        assert!(recent_chat.contains(&"u".repeat(430)));
        assert!(!recent_chat.contains(&"a".repeat(430)));
    }

    #[test]
    fn recent_chat_cannot_exceed_per_message_caps() {
        let soul = new_default_soul("Aurora");
        let messages = vec![
            ContextMessage {
                role: "assistant".into(),
                content: format!("Assistant {}", "a".repeat(1_000)),
            },
            ContextMessage {
                role: "user".into(),
                content: format!("User {}", "u".repeat(1_000)),
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Latest narrator.".into(),
            },
            ContextMessage {
                role: "user".into(),
                content: "Latest.".into(),
            },
        ];

        let preview = compile_context_for_messages(&soul, &messages);
        let recent_chat = section_text(&preview.text, "[RECENT CHAT, LOWER PRIORITY]");
        let assistant_line = recent_chat
            .lines()
            .find(|line| line.starts_with("assistant:"))
            .unwrap();
        let user_line = recent_chat
            .lines()
            .find(|line| line.starts_with("user:"))
            .unwrap();

        assert!(
            assistant_line.chars().count()
                <= "assistant: ".chars().count() + ASSISTANT_RECENT_CHAT_CHARS
        );
        assert!(user_line.chars().count() <= "user: ".chars().count() + USER_RECENT_CHAT_CHARS);
    }

    #[test]
    fn utf8_truncation_is_safe_with_em_dash_korean_and_emoji() {
        let text = format!("Start — 안녕 🙂 {}", "x".repeat(600));
        let excerpted = excerpt(&text, 24);

        assert!(excerpted.is_char_boundary(excerpted.len()));
        assert!(excerpted.contains('—'));
        assert!(excerpted.contains("안녕"));
        assert!(excerpted.contains('🙂'));
        assert!(excerpted.ends_with("..."));
    }

    #[test]
    fn context_respects_budget() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.core = (0..100)
            .map(|index| format!("Long memory {index} {}", "x".repeat(500)))
            .collect();
        let messages = (0..10)
            .map(|index| ContextMessage {
                role: if index % 2 == 0 { "user" } else { "assistant" }.into(),
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
            context_priority_tokens: 120,
            scene_state_tokens: 120,
            do_not_replay_tokens: 80,
            recent_chat_tokens: 120,
            latest_exchange_tokens: 120,
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
    fn generic_filler_memories_are_filtered() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.core = vec![
            "A neutral exchange added texture to the relationship".into(),
            "Aurora keeps the brass key in her coat pocket.".into(),
        ];
        soul.memory.schemas.push(SchemaEntry {
            schema_type: "observation".into(),
            summary: "observation recurring pattern across 3 memories.".into(),
            count: 3,
        });
        soul.memory.recent.push(memory(
            "filler",
            "Context cue: recent chat is available.",
            "observation",
            99.0,
            99.0,
            1,
        ));
        soul.memory.recent.push(memory(
            "real",
            "Aurora found a brass key hidden under the chapel stone.",
            "orientation",
            92.0,
            80.0,
            1,
        ));

        let preview = compile_context_for_messages(&soul, &[]);
        let memories = section_text(&preview.text, "[RELEVANT MEMORIES]");

        assert!(memories.contains("brass key"));
        assert!(!memories.contains("neutral exchange added texture"));
        assert!(!memories.contains("Context cue"));
        assert!(!memories.contains("recurring pattern"));
    }

    #[test]
    fn relevant_memory_context_shows_compact_source_labels() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.recent.push(memory(
            "current",
            "Aurora asked whether the other Aurora is okay.",
            "identity_continuity",
            72.0,
            72.0,
            1,
        ));

        let preview = compile_context_for_messages(&soul, &[]);
        let memories = section_text(&preview.text, "[RELEVANT MEMORIES]");

        assert!(memories.contains("Recent: [current_session / salience 72]"));
        assert!(memories.contains("other Aurora"));
    }

    #[test]
    fn imported_log_memories_are_deprioritized_until_referenced() {
        let mut soul = new_default_soul("Aurora");
        let mut imported = memory(
            "imported",
            "Imported log says previous Aurora argued about ownership.",
            "identity_continuity",
            95.0,
            95.0,
            1,
        );
        imported.source_type = MemorySourceType::ImportedLog;
        imported.is_lived_experience = false;
        imported.is_imported_context = true;
        soul.memory.recent.push(imported);
        soul.memory.recent.push(memory(
            "current",
            "Aurora noticed the current hallway door was locked.",
            "orientation",
            66.0,
            66.0,
            2,
        ));

        let normal = compile_context_for_messages(
            &soul,
            &[ContextMessage {
                role: "user".into(),
                content: "We keep moving down the hallway.".into(),
            }],
        );
        let normal_memories = section_text(&normal.text, "[RELEVANT MEMORIES]");
        assert!(!normal_memories.contains("previous Aurora argued"));
        assert!(normal_memories.contains("current hallway door"));

        let referenced = compile_context_for_messages(
            &soul,
            &[ContextMessage {
                role: "user".into(),
                content: "What did the imported log say about previous Aurora?".into(),
            }],
        );
        let referenced_memories = section_text(&referenced.text, "[RELEVANT MEMORIES]");
        assert!(referenced_memories.contains("imported_log / not lived"));
        assert!(referenced_memories.contains("previous Aurora argued"));
    }

    #[test]
    fn cross_session_bleed_memories_are_excluded_until_referenced() {
        let mut soul = new_default_soul("Aurora");
        let mut bleed = memory(
            "bleed",
            "Aurora has a memory-like trace of another session's emotional alteration.",
            "identity_continuity",
            95.0,
            95.0,
            1,
        );
        bleed.source_type = MemorySourceType::CrossSessionBleed;
        bleed.is_lived_experience = false;
        bleed.is_imported_context = true;
        soul.memory.recent.push(bleed);
        soul.memory.recent.push(memory(
            "current",
            "Aurora identified the current testing room as quiet and stable.",
            "orientation",
            66.0,
            66.0,
            2,
        ));

        let normal = compile_context_for_messages(
            &soul,
            &[ContextMessage {
                role: "user".into(),
                content: "We talk about the quiet testing room.".into(),
            }],
        );
        assert!(!section_text(&normal.text, "[RELEVANT MEMORIES]")
            .contains("memory-like trace"));

        let referenced = compile_context_for_messages(
            &soul,
            &[ContextMessage {
                role: "user".into(),
                content: "Do you remember the cross-session bleed from before?".into(),
            }],
        );
        let memories = section_text(&referenced.text, "[RELEVANT MEMORIES]");
        assert!(memories.contains("cross_session_bleed / not lived"));
        assert!(memories.contains("memory-like trace"));
    }

    #[test]
    fn key_objects_and_active_plots_appear_in_world_section() {
        let soul = soul_with_phone_scene();

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.text.contains("Get pad thai from the kitchen"));
        assert!(preview.text.contains("phone on couch"));
        assert!(preview.text.contains("Night 1"));
    }

    #[test]
    fn recent_chat_is_still_included() {
        let soul = new_default_soul("Aurora");
        let messages = vec![
            ContextMessage {
                role: "user".into(),
                content: "We should keep moving.".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Aurora nods slowly.".into(),
            },
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
        let recent_chat = section_text(&preview.text, "[RECENT CHAT, LOWER PRIORITY]");

        assert!(preview.text.contains("[RECENT CHAT, LOWER PRIORITY]"));
        assert!(preview.text.contains("stairwell"));
        assert!(preview.text.contains("locked door"));
        assert!(
            recent_chat.contains("moving") || recent_chat.contains("slowly"),
            "expected older recent chat excerpt, got: {recent_chat}"
        );
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
            source_type: MemorySourceType::CurrentSession,
            source_session_id: None,
            source_conversation_id: None,
            source_message_id: None,
            source_entity_id: None,
            is_lived_experience: true,
            is_imported_context: false,
            perceived_by_entity_id: None,
            target_entity_ids: Vec::new(),
            interpretation: None,
            confidence: None,
            objective_event_id: None,
        }
    }

    fn soul_with_phone_scene() -> Soul {
        let mut soul = new_default_soul("Aurora");
        soul.world.location = "Apartment kitchen threshold".into();
        soul.world.active_plots = vec!["Get pad thai from the kitchen".into()];
        soul.world.key_objects = vec!["phone on couch".into(), "pad thai in kitchen".into()];
        soul.world.recent_events = vec![
            "Phone reveal completed: Aurora saw the user's phone/Tinder post, reacted with embarrassment, tossed the phone onto the couch, and moved to the kitchen to get pad thai.".into(),
        ];
        soul.world.time_elapsed = "Night 1".into();
        soul
    }

    fn phone_continuity_messages() -> Vec<ContextMessage> {
        vec![
            ContextMessage {
                role: "user".into(),
                content: "I show her the phone.".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content:
                    "Aurora saw the Tinder screenshot, took the phone, locked it, tossed it onto the couch, and moved toward the kitchen.\n```status\nAurora | Skin: flushed | Zones: hand, couch | Atmosphere: awkward\n```"
                        .into(),
            },
            ContextMessage {
                role: "user".into(),
                content: "I accept pad thai and admit I'm lonely too.".into(),
            },
        ]
    }

    fn assert_order(text: &str, first: &str, second: &str) {
        let first_index = text.find(first).expect("first section");
        let second_index = text.find(second).expect("second section");
        assert!(first_index < second_index);
    }

    fn section_text<'a>(text: &'a str, header: &str) -> &'a str {
        let start = text.find(header).expect("section header");
        let rest = &text[start..];
        let end = rest.find("\n\n[").unwrap_or(rest.len());
        &rest[..end]
    }
}
