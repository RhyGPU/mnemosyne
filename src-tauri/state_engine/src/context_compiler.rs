use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::patch::is_premature_user_turn_event;
use crate::relationship_surface::relationship_surface_summary;
use crate::setting::SessionWorld;
use crate::soul::{MemoryEntry, MemorySourceType, PlotStatus, Soul, TruthStatus};

const DEFAULT_TOKEN_BUDGET: usize = 2_500;
const MIN_RECENT_MEMORY_SALIENCE: f32 = 65.0;
const ASSISTANT_RECENT_CHAT_CHARS: usize = 350;
const ASSISTANT_RECENT_CHAT_HEAD_CHARS: usize = 120;
const ASSISTANT_RECENT_CHAT_TAIL_CHARS: usize = 220;
const USER_RECENT_CHAT_CHARS: usize = 500;
const LATEST_ASSISTANT_EXCHANGE_CHARS: usize = 1_200;
const LATEST_USER_EXCHANGE_CHARS: usize = 1_000;
const LATEST_EXCHANGE_INSTRUCTION: &str = "Continue from this section first. If older context conflicts with this section, ignore older context. Continue from the final state of the last narrator response and the latest user input. Do not replay earlier beats.";
const CURRENT_USER_FOLLOWS_LINE: &str =
    "The current user message follows as the next user message.";
const FILLER_MEMORY_PHRASES: &[&str] = &[
    "neutral exchange added texture",
    "context cue",
    "recent chat is available",
    "fresh scene context",
    "felt tense",
    "looked at the user",
    "listened carefully",
    "looked carefully",
    "watched the user",
    "said nothing",
    "remained quiet",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerPersonaContext {
    pub persona_id: String,
    pub display_name: String,
    pub gender_code: String,
    pub pronouns: String,
    pub description: String,
}

impl Default for PlayerPersonaContext {
    fn default() -> Self {
        Self {
            persona_id: "preset_male".into(),
            display_name: "Male Persona".into(),
            gender_code: "male".into(),
            pronouns: "he/him".into(),
            description: "User-controlled male RP persona. No additional traits specified.".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPreview {
    pub text: String,
    pub estimated_tokens: usize,
    pub truncated: bool,
    #[serde(default)]
    pub memory_slot_debug: Vec<MemorySlotTrace>,
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
    memory_slot_debug: Vec<MemorySlotTrace>,
}

#[derive(Debug, Clone)]
struct ScoredMemory<'a> {
    memory: &'a MemoryEntry,
    score: f32,
    repetitive: bool,
    source_restricted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemorySlotTrace {
    pub slot: String,
    pub memory_id: String,
    pub action: String,
    pub reason: String,
    pub source_type: String,
    pub truth_status: String,
    pub entity_match: bool,
    pub plot_match: bool,
    pub salience: f32,
    pub final_score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemorySlot {
    Relationship,
    CurrentPlot,
    CharacterIdentity,
    UnresolvedTension,
    WorldLocation,
    RecentEmotionalState,
}

impl MemorySlot {
    fn all() -> [Self; 6] {
        [
            Self::Relationship,
            Self::CurrentPlot,
            Self::CharacterIdentity,
            Self::UnresolvedTension,
            Self::WorldLocation,
            Self::RecentEmotionalState,
        ]
    }

    fn header(self) -> &'static str {
        match self {
            Self::Relationship => "[RELATIONSHIP MEMORY]",
            Self::CurrentPlot => "[CURRENT PLOT MEMORY]",
            Self::CharacterIdentity => "[CHARACTER IDENTITY MEMORY]",
            Self::UnresolvedTension => "[UNRESOLVED TENSION]",
            Self::WorldLocation => "[WORLD / LOCATION MEMORY]",
            Self::RecentEmotionalState => "[RECENT EMOTIONAL STATE]",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Relationship => "relationship_memories",
            Self::CurrentPlot => "current_plot_memories",
            Self::CharacterIdentity => "character_identity_memories",
            Self::UnresolvedTension => "unresolved_tension_memories",
            Self::WorldLocation => "world_location_memories",
            Self::RecentEmotionalState => "recent_emotional_state_memories",
        }
    }

    fn cap(self) -> usize {
        match self {
            Self::RecentEmotionalState => 1,
            _ => 2,
        }
    }

    fn fallback(self) -> &'static str {
        match self {
            Self::Relationship => "No relationship-specific durable memory selected.",
            Self::CurrentPlot => "No current-plot memory selected.",
            Self::CharacterIdentity => "No identity memory selected.",
            Self::UnresolvedTension => "No unresolved tension selected.",
            Self::WorldLocation => "No world/location memory selected.",
            Self::RecentEmotionalState => "No recent emotional-state memory selected.",
        }
    }
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
    compile_context_with_budget_and_world(soul, None, messages, &ContextBudget::default())
}

pub fn compile_context_for_session(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
) -> ContextPreview {
    compile_context_with_budget_and_world(soul, session_world, messages, &ContextBudget::default())
}

pub fn compile_context_for_session_with_player_persona(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
    player_persona: &PlayerPersonaContext,
) -> ContextPreview {
    compile_context_with_budget_and_options(
        soul,
        session_world,
        messages,
        &ContextBudget::default(),
        false,
        false,
        None,
        Some(player_persona),
    )
}

pub fn compile_context_for_separate_user_message(
    soul: &Soul,
    messages: &[ContextMessage],
) -> ContextPreview {
    compile_context_with_budget_and_options(
        soul,
        None,
        messages,
        &ContextBudget::default(),
        true,
        false,
        None,
        None,
    )
}

pub fn compile_context_for_session_separate_user_message(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
) -> ContextPreview {
    compile_context_for_session_separate_user_message_with_pending(
        soul,
        session_world,
        messages,
        None,
    )
}

pub fn compile_context_for_session_separate_user_message_with_pending(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
    pending_user_text: Option<&str>,
) -> ContextPreview {
    compile_context_for_session_separate_user_message_with_player_persona_pending(
        soul,
        session_world,
        messages,
        pending_user_text,
        None,
    )
}

pub fn compile_context_for_session_separate_user_message_with_player_persona_pending(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
    pending_user_text: Option<&str>,
    player_persona: Option<&PlayerPersonaContext>,
) -> ContextPreview {
    compile_context_with_budget_and_options(
        soul,
        session_world,
        messages,
        &ContextBudget::default(),
        true,
        false,
        pending_user_text,
        player_persona,
    )
}

pub fn compile_context_with_budget(
    soul: &Soul,
    messages: &[ContextMessage],
    budget: &ContextBudget,
) -> ContextPreview {
    compile_context_with_budget_and_world(soul, None, messages, budget)
}

pub fn compile_context_with_budget_and_world(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
    budget: &ContextBudget,
) -> ContextPreview {
    compile_context_with_budget_and_options(
        soul,
        session_world,
        messages,
        budget,
        false,
        false,
        None,
        None,
    )
}

pub fn compile_context_for_session_with_debug_replies(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
) -> ContextPreview {
    compile_context_with_budget_and_options(
        soul,
        session_world,
        messages,
        &ContextBudget::default(),
        false,
        true,
        None,
        None,
    )
}

fn compile_context_with_budget_and_options(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
    budget: &ContextBudget,
    separate_user_message_follows: bool,
    include_debug_replies: bool,
    pending_user_text: Option<&str>,
    player_persona: Option<&PlayerPersonaContext>,
) -> ContextPreview {
    let default_persona;
    let player_persona = if let Some(player_persona) = player_persona {
        player_persona
    } else {
        default_persona = PlayerPersonaContext::default();
        &default_persona
    };
    let mut truncated = false;
    let mut section_builders = vec![
        build_controlled_entities_section(soul, player_persona, budget),
        build_world_section(soul, session_world, budget, pending_user_text),
        build_profile_section(soul, budget),
        build_memory_section(soul, messages, budget),
        build_relationship_section(soul, budget, player_persona),
        build_recent_chat_section(messages, budget),
        build_latest_exchange_section(messages, budget, separate_user_message_follows),
    ];
    if include_debug_replies {
        section_builders.insert(3, build_verified_memory_layer_reply_section(soul, budget));
    }

    let mut sections = Vec::new();
    let mut memory_slot_debug = Vec::new();
    for section in section_builders {
        truncated |= section.truncated;
        memory_slot_debug.extend(section.memory_slot_debug);
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
        memory_slot_debug,
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

fn build_controlled_entities_section(
    soul: &Soul,
    player_persona: &PlayerPersonaContext,
    budget: &ContextBudget,
) -> BuiltSection {
    let character_name = fallback(&soul.character_name, "Unnamed Character");
    let lines = vec![
        "Narrator-controlled Souls:".into(),
        format!(
            "- {character_name} = engine-controlled character. The user is not {character_name}."
        ),
        "User-controlled player persona:".into(),
        format!("- persona_id: {}", player_persona.persona_id),
        format!("- display_name: {}", player_persona.display_name),
        format!("- gender_code: {}", player_persona.gender_code),
        format!("- pronouns: {}", player_persona.pronouns),
        format!("- description: {}", player_persona.description),
        "- controlled_by: user".into(),
        "Operator: the real app user outside RP, appears only through slash commands".into(),
        format!(
            "If the user says \"I\", resolve \"I\" to {}, not to {character_name}.",
            player_persona.display_name
        ),
    ];
    section_from_lines(
        "[CONTROLLED ENTITIES]",
        lines,
        budget.context_priority_tokens.min(budget.max_tokens),
    )
}

fn build_memory_section(
    soul: &Soul,
    messages: &[ContextMessage],
    budget: &ContextBudget,
) -> BuiltSection {
    let query_terms = recent_chat_terms(messages);
    let source_query_active = memory_source_query_active(messages);
    let world_terms = world_memory_terms(soul);
    let mut debug = Vec::new();
    let mut section_texts = Vec::new();

    let core_identity = soul
        .memory
        .core
        .iter()
        .filter_map(|memory| clean(memory))
        .filter(|memory| !is_generic_filler_text(memory))
        .take(2)
        .map(|memory| format!("- Core: {memory}"))
        .collect::<Vec<_>>();

    let schema_identity = soul
        .memory
        .schemas
        .iter()
        .filter_map(|schema| {
            clean(&schema.summary).and_then(|summary| {
                if is_generic_filler_text(summary)
                    || is_near_empty_generic_schema(&schema.schema_type, summary)
                {
                    None
                } else {
                    Some(format!(
                        "- Schema: {} (seen {}x): {}",
                        fallback(&schema.schema_type, "pattern"),
                        schema.reinforcement_count.max(schema.count),
                        summary
                    ))
                }
            })
        })
        .take(2)
        .collect::<Vec<_>>();

    let all_recent = soul
        .memory
        .recent
        .iter()
        .filter(|memory| memory.is_active && !memory.is_retconned)
        .filter(|memory| !is_generic_filler_memory(memory))
        .collect::<Vec<_>>();

    // Assign each memory one primary slot (its best-scoring eligible slot) so the
    // same memory cannot fill multiple prompt sections.
    let mut primary_slots: HashMap<&str, MemorySlot> = HashMap::new();
    for memory in &all_recent {
        let mut best: Option<((u8, u8, f32), MemorySlot)> = None;
        for slot in MemorySlot::all() {
            let scored = score_memory_for_slot(
                memory,
                slot,
                &query_terms,
                &world_terms,
                soul,
                source_query_active,
            );
            let eligible = !scored.source_restricted
                && !scored.repetitive
                && slot_matches_memory(memory, slot, &query_terms, &world_terms, soul)
                && scored.score >= slot_min_score(slot);
            if !eligible {
                continue;
            }
            // Rank: evaluator-assigned slot > explicit tag affinity > score; ties keep
            // the earlier slot in MemorySlot::all() order.
            let rank = (
                u8::from(evaluator_slot_matches(memory, slot)),
                u8::from(slot_tag_affinity(memory, slot)),
                scored.score,
            );
            if best
                .as_ref()
                .is_none_or(|(best_rank, _)| rank.partial_cmp(best_rank) == Some(Ordering::Greater))
            {
                best = Some((rank, slot));
            }
        }
        if let Some((_, slot)) = best {
            primary_slots.insert(memory.id.as_str(), slot);
        }
    }

    for slot in MemorySlot::all() {
        let mut lines = Vec::new();
        if slot == MemorySlot::CharacterIdentity {
            lines.extend(core_identity.iter().cloned());
            lines.extend(schema_identity.iter().cloned());
        }

        let mut candidates = all_recent
            .iter()
            .map(|memory| {
                score_memory_for_slot(
                    memory,
                    slot,
                    &query_terms,
                    &world_terms,
                    soul,
                    source_query_active,
                )
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
        });

        let mut selected = 0;
        for scored in candidates {
            let reason = slot_reason(scored.memory, slot, &query_terms, &world_terms, soul);
            let assigned_elsewhere = primary_slots
                .get(scored.memory.id.as_str())
                .is_some_and(|primary| *primary != slot);
            let selected_for_slot = !assigned_elsewhere
                && !scored.source_restricted
                && !scored.repetitive
                && slot_matches_memory(scored.memory, slot, &query_terms, &world_terms, soul)
                && scored.score >= slot_min_score(slot);
            debug.push(MemorySlotTrace {
                slot: slot.key().into(),
                memory_id: scored.memory.id.clone(),
                action: if selected_for_slot && selected < slot.cap() {
                    "selected".into()
                } else if assigned_elsewhere {
                    "deduplicated_primary_slot_elsewhere".into()
                } else if scored.source_restricted {
                    "downranked_source".into()
                } else if scored.repetitive {
                    "rejected_low_value".into()
                } else {
                    "not_selected".into()
                },
                reason: reason.clone(),
                source_type: scored.memory.source_type.as_label().into(),
                truth_status: scored.memory.truth_status.as_label().into(),
                entity_match: entity_matches_active_soul(scored.memory, soul)
                    || target_matches_query(scored.memory, &query_terms),
                plot_match: plot_matches_memory(scored.memory, &query_terms, &world_terms),
                salience: scored.memory.salience,
                final_score: scored.score,
            });

            if selected_for_slot && selected < slot.cap() {
                lines.push(format!(
                    "- [{} / salience {:.0}] {}",
                    memory_context_label(scored.memory),
                    scored.memory.salience,
                    scored.memory.content.trim()
                ));
                selected += 1;
            }
            if selected >= slot.cap() {
                break;
            }
        }

        if lines.is_empty() {
            lines.push(slot.fallback().into());
        }
        section_texts.push(section_from_lines(
            slot.header(),
            lines,
            slot_token_cap(slot, budget).min(budget.max_tokens),
        ));
    }

    let truncated = section_texts.iter().any(|section| section.truncated);
    let text = section_texts
        .into_iter()
        .map(|section| section.text)
        .collect::<Vec<_>>()
        .join("\n\n");

    BuiltSection {
        text,
        truncated,
        memory_slot_debug: debug,
    }
}

fn slot_token_cap(slot: MemorySlot, budget: &ContextBudget) -> usize {
    match slot {
        MemorySlot::RecentEmotionalState => 90,
        _ => (budget.memory_tokens / 5).max(100),
    }
}

fn slot_min_score(slot: MemorySlot) -> f32 {
    match slot {
        MemorySlot::RecentEmotionalState => 58.0,
        MemorySlot::Relationship => 54.0,
        _ => 50.0,
    }
}

fn score_memory_for_slot<'a>(
    memory: &'a MemoryEntry,
    slot: MemorySlot,
    query_terms: &HashSet<String>,
    world_terms: &HashSet<String>,
    soul: &Soul,
    source_query_active: bool,
) -> ScoredMemory<'a> {
    let mut scored =
        score_recent_memory(memory, query_terms, soul.turn_counter, source_query_active);
    if evaluator_slot_matches(memory, slot) {
        scored.score += 36.0;
    }
    if evaluator_owner_matches(memory, soul) {
        scored.score += 28.0;
    }
    if evaluator_relevance_matches(memory, query_terms, world_terms) {
        scored.score += 22.0;
    }
    if matches!(memory.knowledge_scope.as_deref(), Some("not_known")) {
        scored.score -= 80.0;
    }
    if slot_matches_memory(memory, slot, query_terms, world_terms, soul) {
        scored.score += 24.0;
    }
    if entity_matches_active_soul(memory, soul) {
        scored.score += 14.0;
    }
    if target_matches_query(memory, query_terms) {
        scored.score += 18.0;
    }
    if plot_matches_memory(memory, query_terms, world_terms) {
        scored.score += 16.0;
    }
    if is_durable_memory_text(&memory.content) {
        scored.score += 10.0;
    }
    if matches!(
        memory.truth_status,
        TruthStatus::NarratorClaim | TruthStatus::CharacterBelief | TruthStatus::UserClaimed
    ) && is_architecture_related_memory(&memory.content)
    {
        scored.score -= 35.0;
    }
    scored
}

fn slot_matches_memory(
    memory: &MemoryEntry,
    slot: MemorySlot,
    query_terms: &HashSet<String>,
    world_terms: &HashSet<String>,
    soul: &Soul,
) -> bool {
    if evaluator_slot_matches(memory, slot) {
        return evaluator_owner_matches(memory, soul);
    }
    let lower = memory.content.to_ascii_lowercase();
    let tag = memory.tag.to_ascii_lowercase();
    match slot {
        MemorySlot::Relationship => {
            let relationship_text = contains_any(
                &lower,
                &[
                    "trust",
                    "distrust",
                    "affection",
                    "fear of",
                    "bond",
                    "betray",
                    "relationship",
                    "promise",
                    "boundary",
                    "owes",
                    "forgave",
                    "conflict",
                    "argued",
                ],
            ) || tag.contains("relationship")
                || !memory.target_entity_ids.is_empty();
            relationship_text
                && (target_matches_query(memory, query_terms)
                    || memory.target_entity_ids.iter().any(|target| {
                        let target = display_entity_id(target);
                        target == "default_player" || target == "user"
                    })
                    || memory.target_entity_ids.is_empty())
                && entity_matches_active_soul(memory, soul)
        }
        MemorySlot::CurrentPlot => {
            contains_any(
                &lower,
                &[
                    "plot",
                    "goal",
                    "testing",
                    "developing",
                    "trying to",
                    "investigate",
                    "current",
                    "mission",
                    "task",
                    "intention",
                    "future",
                ],
            ) || tag.contains("plot")
                || plot_matches_memory(memory, query_terms, world_terms)
        }
        MemorySlot::CharacterIdentity => {
            contains_any(
                &lower,
                &[
                    "identity",
                    "is an",
                    "is a",
                    "role",
                    "self-concept",
                    "name",
                    "analysis agent",
                    "test subject",
                    "purpose",
                    "belongs to",
                ],
            ) || tag.contains("identity")
                || tag.contains("schema")
        }
        MemorySlot::UnresolvedTension => {
            contains_any(
                &lower,
                &[
                    "unresolved",
                    "tension",
                    "betray",
                    "promise",
                    "commitment",
                    "boundary",
                    "conflict",
                    "guilt",
                    "concern",
                    "afraid",
                    "owed",
                    "not resolved",
                    "argued",
                ],
            ) || tag.contains("conflict")
                || tag.contains("boundary")
        }
        MemorySlot::WorldLocation => {
            contains_any(
                &lower,
                &[
                    "location",
                    "world",
                    "room",
                    "cell",
                    "lab",
                    "testing room",
                    "kitchen",
                    "door",
                    "hallway",
                    "object",
                    "terminal",
                    "white room",
                ],
            ) || plot_matches_memory(memory, query_terms, world_terms)
                || tag.contains("orientation")
        }
        MemorySlot::RecentEmotionalState => {
            contains_any(
                &lower,
                &[
                    "feels",
                    "felt",
                    "afraid",
                    "angry",
                    "concerned",
                    "guilt",
                    "ashamed",
                    "diagnostic mode",
                    "guarded",
                    "distressed",
                    "relieved",
                ],
            ) || tag.contains("emotion")
                || tag.contains("trauma")
        }
    }
}

fn slot_reason(
    memory: &MemoryEntry,
    slot: MemorySlot,
    query_terms: &HashSet<String>,
    world_terms: &HashSet<String>,
    soul: &Soul,
) -> String {
    let mut reasons = Vec::new();
    if evaluator_slot_matches(memory, slot) {
        reasons.push("evaluator_slot_match");
    }
    if evaluator_owner_matches(memory, soul) {
        reasons.push("owner_soul_match");
    }
    if evaluator_relevance_matches(memory, query_terms, world_terms) {
        reasons.push("evaluator_relevance_match");
    }
    if entity_matches_active_soul(memory, soul) {
        reasons.push("entity_match");
    }
    if target_matches_query(memory, query_terms) {
        reasons.push("relationship_target_match");
    }
    if plot_matches_memory(memory, query_terms, world_terms) {
        reasons.push("plot_or_world_match");
    }
    if is_durable_memory_text(&memory.content) {
        reasons.push("durable_content");
    }
    if slot_matches_memory(memory, slot, query_terms, world_terms, soul) {
        reasons.push("slot_keyword_match");
    }
    if reasons.is_empty() {
        reasons.push("low_relevance");
    }
    reasons.join(" + ")
}

fn entity_matches_active_soul(memory: &MemoryEntry, soul: &Soul) -> bool {
    if !evaluator_owner_matches(memory, soul) {
        return false;
    }
    let active = soul.character_id.trim();
    let active_name = soul.character_name.trim();
    memory
        .perceived_by_entity_id
        .as_deref()
        .map(|entity| {
            entity.eq_ignore_ascii_case(active) || entity.eq_ignore_ascii_case(active_name)
        })
        .unwrap_or(true)
}

fn evaluator_owner_matches(memory: &MemoryEntry, soul: &Soul) -> bool {
    memory
        .owner_soul_id
        .as_deref()
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        .map(|owner| {
            owner.eq_ignore_ascii_case(&soul.character_id)
                || owner.eq_ignore_ascii_case(&soul.character_name)
        })
        .unwrap_or(true)
}

fn evaluator_slot_matches(memory: &MemoryEntry, slot: MemorySlot) -> bool {
    let Some(memory_slot) = memory
        .memory_slot
        .as_deref()
        .map(|slot| slot.trim().to_ascii_lowercase())
        .filter(|slot| !slot.is_empty())
    else {
        return false;
    };
    memory_slot == evaluator_slot_label(slot)
}

/// Explicit tag affinity outranks generic content-keyword matches when picking a
/// memory's primary slot. Mirrors the tag checks inside `slot_matches_memory`.
fn slot_tag_affinity(memory: &MemoryEntry, slot: MemorySlot) -> bool {
    let tag = memory.tag.to_ascii_lowercase();
    match slot {
        MemorySlot::Relationship => tag.contains("relationship"),
        MemorySlot::CurrentPlot => tag.contains("plot"),
        MemorySlot::CharacterIdentity => tag.contains("identity") || tag.contains("schema"),
        MemorySlot::UnresolvedTension => tag.contains("conflict") || tag.contains("boundary"),
        MemorySlot::WorldLocation => tag.contains("orientation"),
        MemorySlot::RecentEmotionalState => tag.contains("emotion") || tag.contains("trauma"),
    }
}

fn evaluator_slot_label(slot: MemorySlot) -> &'static str {
    match slot {
        MemorySlot::Relationship => "relationship_memory",
        MemorySlot::CurrentPlot => "current_plot_memory",
        MemorySlot::CharacterIdentity => "character_identity_memory",
        MemorySlot::UnresolvedTension => "unresolved_tension",
        MemorySlot::WorldLocation => "world_location_memory",
        MemorySlot::RecentEmotionalState => "recent_emotional_state",
    }
}

fn evaluator_relevance_matches(
    memory: &MemoryEntry,
    query_terms: &HashSet<String>,
    world_terms: &HashSet<String>,
) -> bool {
    memory.relevance_tags.iter().any(|(tag, score)| {
        *score >= 50
            && token_set(tag)
                .iter()
                .any(|term| query_terms.contains(term) || world_terms.contains(term))
    })
}

fn target_matches_query(memory: &MemoryEntry, query_terms: &HashSet<String>) -> bool {
    memory.target_entity_ids.iter().any(|target| {
        let target = display_entity_id(target).to_ascii_lowercase();
        query_terms.contains(&target)
    })
}

fn plot_matches_memory(
    memory: &MemoryEntry,
    query_terms: &HashSet<String>,
    world_terms: &HashSet<String>,
) -> bool {
    let memory_terms = token_set(&memory.content);
    memory_terms
        .iter()
        .any(|term| world_terms.contains(term) || query_terms.contains(term))
}

fn world_memory_terms(soul: &Soul) -> HashSet<String> {
    let mut text = format!(
        "{} {} {} {}",
        soul.world.location,
        soul.world.active_plots.join(" "),
        soul.world.key_objects.join(" "),
        soul.world
            .dominant_current_plot
            .as_ref()
            .map(|plot| format!("{} {}", plot.title, plot.summary))
            .unwrap_or_default()
    );
    for plot in &soul.world.background_plots {
        if plot.status != PlotStatus::Resolved {
            text.push(' ');
            text.push_str(&plot.title);
            text.push(' ');
            text.push_str(&plot.summary);
        }
    }
    token_set(&text)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn is_durable_memory_text(text: &str) -> bool {
    contains_any(
        &text.to_ascii_lowercase(),
        &[
            "promise",
            "betray",
            "identity",
            "boundary",
            "future intention",
            "resolved",
            "unresolved",
            "turning point",
            "important",
            "prefers",
            "refuses",
            "commitment",
            "testing memory",
            "building mnemosyne",
        ],
    )
}

fn build_verified_memory_layer_reply_section(soul: &Soul, budget: &ContextBudget) -> BuiltSection {
    let Some(reply) = soul.debug_memory_layer_replies.iter().find(|reply| {
        reply.architecture_verified
            && !reply.nonce.trim().is_empty()
            && !reply.content.trim().is_empty()
    }) else {
        return BuiltSection {
            text: String::new(),
            truncated: false,
            memory_slot_debug: Vec::new(),
        };
    };

    section_from_lines(
        "[MEMORY LAYER REPLY - VERIFIED DEBUG]",
        vec![
            format!("nonce: {}", reply.nonce.trim()),
            format!("content: {}", reply.content.trim()),
        ],
        budget.memory_tokens.min(budget.max_tokens),
    )
}

fn build_world_section(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    budget: &ContextBudget,
    pending_user_text: Option<&str>,
) -> BuiltSection {
    let (source, setting_name, scenario, world) = if let Some(session_world) = session_world {
        (
            "session_world",
            session_world.setting_name.as_str(),
            session_world.scenario.as_str(),
            session_world.world_log(),
        )
    } else {
        (
            "legacy character_soul.world",
            "Legacy character world",
            "",
            soul.world.clone(),
        )
    };
    let mut lines = vec![
        format!("Source: {source}"),
        format!("World: {}", fallback(setting_name, "Unnamed World")),
        format!("Location: {}", fallback(&world.location, "Unspecified")),
        format!(
            "Time elapsed: {}",
            normalize_time_elapsed_display(fallback(&world.time_elapsed, "Unknown"))
        ),
    ];
    if !scenario.trim().is_empty() {
        lines.push(format!("Scenario: {}", scenario.trim()));
    }

    lines.push(format_list(
        "Active plots",
        &world.active_plots,
        "No active plot has been established.",
    ));
    if let Some(plot) = world.dominant_current_plot.as_ref().filter(|plot| {
        matches!(plot.status, PlotStatus::Dominant | PlotStatus::Unknown)
            && !plot.title.trim().is_empty()
    }) {
        lines.push(format!(
            "Dominant current plot: {} - {}",
            plot.title.trim(),
            fallback(&plot.summary, "No summary")
        ));
    }
    let background = world
        .background_plots
        .iter()
        .filter(|plot| {
            matches!(
                plot.status,
                PlotStatus::Background | PlotStatus::Stale | PlotStatus::Unknown
            ) && !plot.title.trim().is_empty()
        })
        .take(3)
        .map(|plot| format!("{} ({})", plot.title.trim(), plot.status.as_label()))
        .collect::<Vec<_>>();
    if !background.is_empty() {
        lines.push(format!("Background/stale plots: {}", background.join("; ")));
    }
    let resolved = world
        .resolved_plots
        .iter()
        .filter(|plot| matches!(plot.status, PlotStatus::Resolved) && !plot.title.trim().is_empty())
        .rev()
        .take(2)
        .map(|plot| {
            format!(
                "{} resolved: {}",
                plot.title.trim(),
                plot.resolution_summary
                    .as_deref()
                    .map(str::trim)
                    .filter(|summary| !summary.is_empty())
                    .unwrap_or("resolution recorded")
            )
        })
        .collect::<Vec<_>>();
    if !resolved.is_empty() {
        lines.push(format!("Resolved plots: {}", resolved.join("; ")));
    }
    lines.push(format_list(
        "Key objects",
        &world.key_objects,
        "No key objects are being tracked.",
    ));
    let object_state_lines = world
        .object_states
        .iter()
        .filter(|object| !object.object_id.trim().is_empty())
        .take(6)
        .map(|object| {
            let mut parts = vec![
                format!("power {}", fallback(&object.power_state, "unknown")),
                format!(
                    "notifications {}",
                    fallback(&object.notification_mode, "unknown")
                ),
            ];
            if let Some(vibrate) = object.vibrate_enabled {
                parts.push(format!("vibrate {}", bool_label(vibrate)));
            }
            if let Some(screen_wake) = object.screen_wake_enabled {
                parts.push(format!("screen_wake {}", bool_label(screen_wake)));
            }
            if let Some(owner) = object.owner_entity_id.as_deref().and_then(clean) {
                parts.push(format!("owner {owner}"));
            }
            format!("- {} ({})", object.object_id.trim(), parts.join(", "))
        })
        .collect::<Vec<_>>();
    if !object_state_lines.is_empty() {
        lines.push(format!("Object states:\n{}", object_state_lines.join("\n")));
    }

    let active_record_events = world
        .recent_event_records
        .iter()
        .filter(|event| event.is_active)
        .map(|event| event.content.as_str())
        .collect::<Vec<_>>();
    let recent_event_source = if active_record_events.is_empty() {
        world
            .recent_events
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    } else {
        active_record_events
    };
    let mut recent_events = recent_event_source
        .iter()
        .rev()
        .take(8)
        .filter_map(|event| clean(event))
        .filter(|event| !is_premature_user_turn_event(event, pending_user_text))
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

fn build_relationship_section(
    soul: &Soul,
    budget: &ContextBudget,
    player_persona: &PlayerPersonaContext,
) -> BuiltSection {
    if soul.relationships.is_empty() {
        return section_from_lines(
            "[RELATIONSHIPS]",
            vec!["No relationship state has been established.".into()],
            budget.relationship_tokens.min(budget.max_tokens),
        );
    }

    let mut relationships = soul.relationships.iter().collect::<Vec<_>>();
    relationships.sort_by(|left, right| {
        display_entity_id_for_persona(left.0, player_persona)
            .cmp(&display_entity_id_for_persona(right.0, player_persona))
    });
    let lines = relationships
        .into_iter()
        .map(|(target, relationship)| {
            format!(
                "{} -> {}: {} Label/style: {}.",
                fallback(&soul.character_name, "Character"),
                display_entity_id_for_persona(target, player_persona),
                relationship_surface_summary(relationship),
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
            memory_slot_debug: Vec::new(),
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

    BuiltSection {
        text,
        truncated,
        memory_slot_debug: Vec::new(),
    }
}

fn compact_sections_to_budget(sections: &mut Vec<String>, max_tokens: usize) -> bool {
    let mut truncated = false;
    let trim_order = [
        "[RECENT CHAT, LOWER PRIORITY]",
        "[RECENT EMOTIONAL STATE]",
        "[WORLD / LOCATION MEMORY]",
        "[UNRESOLVED TENSION]",
        "[CURRENT PLOT MEMORY]",
        "[RELATIONSHIP MEMORY]",
        "[CHARACTER IDENTITY MEMORY]",
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
    let score = memory.salience
        + (memory.retrieval_strength * 0.35)
        + (overlap * 20.0)
        + recency_bonus
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

fn memory_context_label(memory: &MemoryEntry) -> String {
    let truth_status = memory.truth_status;
    if memory.architecture_verified && truth_status.is_engine_verified() {
        return truth_status.as_label().to_string();
    }
    if !memory.architecture_verified
        && (truth_status != TruthStatus::Unknown || is_architecture_related_memory(&memory.content))
    {
        return format!("{} / unverified", truth_status.as_label());
    }
    memory_source_label(memory)
}

fn is_architecture_related_memory(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "memory layer",
        "state updater",
        "hidden system",
        "backend",
        "provider",
        " api",
        "system responded",
        "direct state injection",
        "model spoke",
        "not fiction",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
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

fn bool_label(value: bool) -> &'static str {
    if value {
        "enabled"
    } else {
        "disabled"
    }
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

fn display_entity_id_for_persona(entity_id: &str, player_persona: &PlayerPersonaContext) -> String {
    let trimmed = entity_id.trim();
    if trimmed.eq_ignore_ascii_case("user")
        || trimmed.eq_ignore_ascii_case("default_player")
        || trimmed.eq_ignore_ascii_case(&player_persona.persona_id)
    {
        return format!(
            "{} ({})",
            fallback(&player_persona.display_name, "Player Persona"),
            fallback(&player_persona.persona_id, "active_player_persona")
        );
    }
    display_entity_id(trimmed)
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
        assert!(preview.text.contains("[RELATIONSHIP MEMORY]"));
        assert!(preview.text.contains("[CURRENT PLOT MEMORY]"));
        assert!(preview.text.contains("[CHARACTER IDENTITY MEMORY]"));
        assert!(preview.text.contains("[UNRESOLVED TENSION]"));
        assert!(preview.text.contains("[WORLD / LOCATION MEMORY]"));
        assert!(preview.text.contains("[RECENT EMOTIONAL STATE]"));
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
            .contains("Aurora -> junhwa: Conflict is strong"));
        assert!(!preview.text.contains("trust 8"));
        assert!(!preview.text.contains("dependency 70"));
        assert!(preview
            .text
            .contains("Aurora -> rhy: Trust feels faint and comfort feels faint"));
        assert!(!preview.text.contains("curiosity 35"));
    }

    #[test]
    fn active_player_persona_replaces_default_player_in_context_surface() {
        let mut soul = new_default_soul("Aurora");
        soul.relationships.get_mut("user").unwrap().trust = 42.0;
        let persona = PlayerPersonaContext {
            persona_id: "persona_jun".into(),
            display_name: "Jun Persona".into(),
            gender_code: "custom".into(),
            pronouns: "they/them".into(),
            description: "User-controlled custom RP persona.".into(),
        };
        let preview = compile_context_for_session_with_player_persona(&soul, None, &[], &persona);

        assert!(preview.text.contains("[CONTROLLED ENTITIES]"));
        assert!(preview.text.contains("- persona_id: persona_jun"));
        assert!(preview.text.contains("- display_name: Jun Persona"));
        assert!(preview
            .text
            .contains("If the user says \"I\", resolve \"I\" to Jun Persona"));
        assert!(preview
            .text
            .contains("Aurora -> Jun Persona (persona_jun):"));
        assert!(!preview.text.contains("Aurora -> default_player:"));
    }

    #[test]
    fn narrator_surface_hides_raw_scores() {
        let mut soul = new_default_soul("Aurora");
        let relationship = soul.relationships.get_mut("user").unwrap();
        relationship.trust = 77.0;
        relationship.comfort = 55.0;
        relationship.boundary_pressure = 12.0;
        relationship.autonomy_respect_bias = 45.0;

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.text.contains("[RELATIONSHIPS]"));
        assert!(!preview.text.contains("trust 77"));
        assert!(!preview.text.contains("comfort 55"));
        assert!(!preview.text.contains("boundary_pressure"));
    }

    #[test]
    fn narrator_surface_describes_high_asshole_and_high_trustable() {
        let mut soul = new_default_soul("Aurora");
        let relationship = soul.relationships.get_mut("user").unwrap();
        relationship.asshole_bias = 70.0;
        relationship.trustable_bias = 65.0;

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview
            .text
            .contains("abrasive and difficult, but increasingly reliable"));
    }

    #[test]
    fn narrator_surface_describes_boundary_pressure_drop() {
        let mut soul = new_default_soul("Aurora");
        let relationship = soul.relationships.get_mut("user").unwrap();
        relationship.boundary_pressure = 5.0;
        relationship.autonomy_respect_bias = 50.0;

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview.text.contains("sense of being cornered has eased"));
    }

    #[test]
    fn narrator_surface_describes_reappraisal_under_review() {
        let mut soul = new_default_soul("Aurora");
        soul.relationships
            .get_mut("user")
            .unwrap()
            .reappraisal_state_code = 2;

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(preview
            .text
            .contains("earlier impression is under pressure"));
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
    fn first_turn_no_premature_recent_event_in_context() {
        let soul = new_default_soul("Aurora");
        let mut session_world = crate::setting::session_world_from_setting(
            &crate::setting::new_default_setting("Aurora Apartment"),
        );
        session_world.recent_events =
            vec!["The conversation continued without a major rupture: I knock on the door".into()];
        let preview = compile_context_for_session_separate_user_message_with_pending(
            &soul,
            Some(&session_world),
            &[],
            Some("I knock on the door"),
        );
        assert!(!preview.text.contains("I knock on the door"));
        assert!(!preview
            .text
            .contains("The conversation continued without a major rupture"));
    }

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
            schema_id: "observation-schema".into(),
            owner_soul_id: Some(soul.character_id.clone()),
            target_entity_ids: Vec::new(),
            trigger_tags: vec!["observation".into()],
            salience: 20.0,
            reinforcement_count: 3,
            decay: 0.0,
            last_reinforced_turn: soul.turn_counter,
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
        let memories = section_text(&preview.text, "[WORLD / LOCATION MEMORY]");

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
        let memories = section_text(&preview.text, "[CHARACTER IDENTITY MEMORY]");

        assert!(memories.contains("[current_session / salience 72]"));
        assert!(memories.contains("other Aurora"));
    }

    #[test]
    fn relevant_memory_context_labels_unverified_architecture_claims() {
        let mut soul = new_default_soul("Echo-0");
        let mut claim = memory(
            "claim",
            "Echo-0 believes it contacted the memory layer.",
            "identity_continuity",
            92.0,
            92.0,
            1,
        );
        claim.truth_status = TruthStatus::CharacterBelief;
        claim.architecture_verified = false;
        soul.memory.recent.push(claim);

        let preview = compile_context_for_messages(&soul, &[]);
        let memories = section_text(&preview.text, "[CHARACTER IDENTITY MEMORY]");

        assert!(memories.contains("[character_belief / unverified"));
        assert!(memories.contains("memory layer"));
    }

    #[test]
    fn world_snapshot_uses_session_world_when_present() {
        let mut soul = new_default_soul("Echo-0");
        soul.world.location = "Wrong character-embedded room".into();
        let mut world = crate::setting::session_world_from_legacy_world(
            "Testing Room",
            Some("testing-room".into()),
            &crate::soul::WorldLog {
                location: "Session lab".into(),
                active_plots: vec!["Run Echo-0 verification".into()],
                recent_events: vec!["Echo-0 entered the testing room.".into()],
                key_objects: vec!["debug terminal".into()],
                time_elapsed: "Session start".into(),
                ..crate::soul::WorldLog::default()
            },
        );
        world.scenario = "Objective debug room.".into();

        let preview = compile_context_for_session(&soul, Some(&world), &[]);
        let world_section = section_text(&preview.text, "[WORLD SNAPSHOT]");

        assert!(world_section.contains("Source: session_world"));
        assert!(world_section.contains("World: Testing Room"));
        assert!(world_section.contains("Session lab"));
        assert!(world_section.contains("Objective debug room."));
        assert!(!world_section.contains("Wrong character-embedded room"));
    }

    #[test]
    fn legacy_world_snapshot_is_labeled_when_no_session_world_exists() {
        let mut soul = new_default_soul("Echo-0");
        soul.world.location = "Legacy embedded room".into();

        let preview = compile_context_for_messages(&soul, &[]);
        let world_section = section_text(&preview.text, "[WORLD SNAPSHOT]");

        assert!(world_section.contains("Source: legacy character_soul.world"));
        assert!(world_section.contains("Legacy embedded room"));
    }

    #[test]
    fn verified_memory_layer_reply_appears_in_debug_section() {
        let mut soul = new_default_soul("Echo-0");
        soul.debug_memory_layer_replies
            .push(crate::soul::MemoryLayerReply {
                nonce: "nonce-123".into(),
                content: "Debug memory-layer nonce reply received.".into(),
                created_at: 10,
                architecture_verified: true,
            });

        let preview = compile_context_for_messages(&soul, &[]);
        assert!(!preview
            .text
            .contains("[MEMORY LAYER REPLY - VERIFIED DEBUG]"));

        let debug_preview = compile_context_for_session_with_debug_replies(&soul, None, &[]);
        assert!(debug_preview
            .text
            .contains("[MEMORY LAYER REPLY - VERIFIED DEBUG]"));
        assert!(debug_preview.text.contains("nonce: nonce-123"));
        assert!(debug_preview
            .text
            .contains("content: Debug memory-layer nonce reply received."));
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
        let normal_memories = section_text(&normal.text, "[WORLD / LOCATION MEMORY]");
        assert!(!normal_memories.contains("previous Aurora argued"));
        assert!(normal_memories.contains("current hallway door"));

        let referenced = compile_context_for_messages(
            &soul,
            &[ContextMessage {
                role: "user".into(),
                content: "What did the imported log say about previous Aurora?".into(),
            }],
        );
        // Each memory now has one primary slot, so assert visibility and labeling
        // across the whole compiled context instead of pinning a specific section.
        assert!(referenced.text.contains("imported_log / not lived"));
        assert!(referenced.text.contains("previous Aurora argued"));
    }

    #[test]
    fn cross_session_bleed_memories_are_excluded_until_referenced() {
        let mut soul = new_default_soul("Aurora");
        let mut bleed = memory(
            "bleed",
            "Aurora felt distressed by a memory-like trace of another session's emotional alteration.",
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
        assert!(
            !section_text(&normal.text, "[RECENT EMOTIONAL STATE]").contains("memory-like trace")
        );

        let referenced = compile_context_for_messages(
            &soul,
            &[ContextMessage {
                role: "user".into(),
                content: "Do you remember the cross-session bleed from before?".into(),
            }],
        );
        // Each memory now has one primary slot, so assert visibility and labeling
        // across the whole compiled context instead of pinning a specific section.
        assert!(referenced.text.contains("cross_session_bleed / not lived"));
        assert!(referenced.text.contains("memory-like trace"));
    }

    #[test]
    fn high_salience_emotional_memory_does_not_crowd_relationship_slot() {
        let mut soul = new_default_soul("Echo-0");
        soul.memory.recent.push(memory(
            "emotion",
            "Echo-0 felt distressed while staring at the debug terminal.",
            "emotion",
            99.0,
            99.0,
            1,
        ));
        let mut relationship = memory(
            "relationship",
            "Echo-0 trusts default_player as the operator building Mnemosyne memory behavior.",
            "relationship",
            55.0,
            45.0,
            2,
        );
        relationship.target_entity_ids = vec!["default_player".into()];
        soul.memory.recent.push(relationship);

        let preview = compile_context_for_messages(&soul, &[]);
        let relationship_slot = section_text(&preview.text, "[RELATIONSHIP MEMORY]");
        let emotion_slot = section_text(&preview.text, "[RECENT EMOTIONAL STATE]");

        assert!(relationship_slot.contains("building Mnemosyne"));
        assert!(!relationship_slot.contains("felt distressed"));
        assert!(emotion_slot.contains("felt distressed"));
    }

    #[test]
    fn memory_appears_in_at_most_one_prompt_section() {
        let mut soul = new_default_soul("Aurora");
        // Content deliberately matches several slot keyword lists at once:
        // "promise"/"argued" (relationship, unresolved tension), "current"/"goal"
        // (current plot), "door" (world/location), "afraid" (emotional state).
        let mut tangled = memory(
            "tangled",
            "Aurora argued about a broken promise near the current door and is afraid the goal is lost.",
            "relationship",
            95.0,
            95.0,
            1,
        );
        tangled.target_entity_ids = vec!["default_player".into()];
        soul.memory.recent.push(tangled);

        let preview = compile_context_for_messages(
            &soul,
            &[ContextMessage {
                role: "user".into(),
                content: "We argued about the promise at the door.".into(),
            }],
        );

        let occurrences = preview
            .text
            .matches("argued about a broken promise")
            .count();
        assert_eq!(
            occurrences, 1,
            "memory must be selected into exactly one prompt section, found {occurrences}"
        );
        let selected_slots = preview
            .memory_slot_debug
            .iter()
            .filter(|trace| trace.memory_id == "tangled" && trace.action == "selected")
            .count();
        assert_eq!(selected_slots, 1);
        assert!(preview
            .memory_slot_debug
            .iter()
            .any(|trace| trace.memory_id == "tangled"
                && trace.action == "deduplicated_primary_slot_elsewhere"));
    }

    #[test]
    fn relationship_slot_respects_directed_entity_pair() {
        let mut soul = new_default_soul("Aurora");
        let mut junhwa = memory(
            "junhwa",
            "Aurora distrusts Junhwa after a betrayal.",
            "relationship",
            98.0,
            98.0,
            1,
        );
        junhwa.target_entity_ids = vec!["junhwa".into()];
        let mut rhy = memory(
            "rhy",
            "Aurora trusts Rhy with cautious collaboration.",
            "relationship",
            58.0,
            50.0,
            2,
        );
        rhy.target_entity_ids = vec!["rhy".into()];
        soul.memory.recent.push(junhwa);
        soul.memory.recent.push(rhy);

        let preview = compile_context_for_messages(
            &soul,
            &[ContextMessage {
                role: "user".into(),
                content: "Talk to Rhy about collaboration.".into(),
            }],
        );
        let relationship_slot = section_text(&preview.text, "[RELATIONSHIP MEMORY]");

        assert!(relationship_slot.contains("trusts Rhy"));
        assert!(!relationship_slot.contains("distrusts Junhwa"));
    }

    #[test]
    fn bar_location_triggers_prior_aurora_memory_about_x() {
        let mut soul = new_default_soul("Aurora");
        let mut bar_memory = memory(
            "bar_x",
            "Aurora remembers that X betrayed her trust at the Blue Lantern bar.",
            "relationship_memory",
            70.0,
            70.0,
            1,
        );
        bar_memory.owner_soul_id = Some(soul.character_id.clone());
        bar_memory.perceived_by_entity_id = Some(soul.character_id.clone());
        bar_memory.memory_slot = Some("relationship_memory".into());
        bar_memory.knowledge_scope = Some("directly_observed".into());
        bar_memory.target_entity_ids = vec!["x".into()];
        bar_memory
            .relevance_tags
            .insert("Blue Lantern bar".into(), 95);
        bar_memory.relevance_tags.insert("x".into(), 90);
        soul.memory.recent.push(bar_memory);
        soul.world.location = "Blue Lantern bar".into();

        let preview = compile_context_for_messages(
            &soul,
            &[ContextMessage {
                role: "user".into(),
                content: "We return to the Blue Lantern bar where X used to wait.".into(),
            }],
        );
        let relationship_slot = section_text(&preview.text, "[RELATIONSHIP MEMORY]");

        assert!(relationship_slot.contains("X betrayed her trust"));
    }

    #[test]
    fn persona_b_does_not_know_aurora_x_bar_memory() {
        let aurora = new_default_soul("Aurora");
        let mut persona_b = new_default_soul("Persona B");
        let mut bar_memory = memory(
            "bar_x",
            "Aurora remembers that X betrayed her trust at the Blue Lantern bar.",
            "relationship_memory",
            99.0,
            99.0,
            1,
        );
        bar_memory.owner_soul_id = Some(aurora.character_id.clone());
        bar_memory.perceived_by_entity_id = Some(aurora.character_id.clone());
        bar_memory.memory_slot = Some("relationship_memory".into());
        bar_memory.knowledge_scope = Some("directly_observed".into());
        bar_memory.target_entity_ids = vec!["x".into()];
        bar_memory
            .relevance_tags
            .insert("Blue Lantern bar".into(), 95);
        persona_b.memory.recent.push(bar_memory);
        persona_b.world.location = "Blue Lantern bar".into();

        let preview = compile_context_for_messages(
            &persona_b,
            &[ContextMessage {
                role: "user".into(),
                content: "Persona B enters the Blue Lantern bar.".into(),
            }],
        );
        let relationship_slot = section_text(&preview.text, "[RELATIONSHIP MEMORY]");

        assert!(!relationship_slot.contains("X betrayed her trust"));
    }

    #[test]
    fn identity_and_current_plot_memories_use_separate_slots() {
        let mut soul = new_default_soul("Echo-0");
        soul.world.active_plots = vec!["Test memory retrieval slots".into()];
        soul.memory.recent.push(memory(
            "identity",
            "Echo-0 is an analysis agent for system questions.",
            "identity",
            62.0,
            55.0,
            1,
        ));
        soul.memory.recent.push(memory(
            "plot",
            "Echo-0 is testing memory retrieval, compression, and bleed control.",
            "plot",
            62.0,
            55.0,
            2,
        ));

        let preview = compile_context_for_messages(&soul, &[]);

        assert!(
            section_text(&preview.text, "[CHARACTER IDENTITY MEMORY]").contains("analysis agent")
        );
        assert!(section_text(&preview.text, "[CURRENT PLOT MEMORY]").contains("memory retrieval"));
    }

    #[test]
    fn resolved_and_stale_plots_do_not_become_dominant() {
        let mut soul = new_default_soul("Echo-0");
        soul.world.dominant_current_plot = Some(crate::soul::PlotEntry {
            plot_id: "resolved".into(),
            title: "Old export crash".into(),
            summary: "Already resolved.".into(),
            status: PlotStatus::Resolved,
            salience: 90.0,
            started_turn: 1,
            last_touched_turn: 2,
            related_entities: vec!["echo_0".into()],
            related_world_id: None,
            unresolved_questions: Vec::new(),
            resolution_summary: Some("Export crash was fixed.".into()),
        });
        soul.world.background_plots.push(crate::soul::PlotEntry {
            plot_id: "stale".into(),
            title: "Old hallway search".into(),
            summary: "No longer active unless mentioned.".into(),
            status: PlotStatus::Stale,
            salience: 30.0,
            started_turn: 1,
            last_touched_turn: 1,
            related_entities: vec!["echo_0".into()],
            related_world_id: None,
            unresolved_questions: Vec::new(),
            resolution_summary: None,
        });
        soul.world.resolved_plots.push(crate::soul::PlotEntry {
            plot_id: "resolved-list".into(),
            title: "Image profile bug".into(),
            summary: "Resolved profile-image ownership issue.".into(),
            status: PlotStatus::Resolved,
            salience: 60.0,
            started_turn: 1,
            last_touched_turn: 4,
            related_entities: vec!["echo_0".into()],
            related_world_id: None,
            unresolved_questions: Vec::new(),
            resolution_summary: Some("Avatar replacement stopped deleting old Soul images.".into()),
        });

        let preview = compile_context_for_messages(&soul, &[]);
        let world = section_text(&preview.text, "[WORLD SNAPSHOT]");

        assert!(!world.contains("Dominant current plot: Old export crash"));
        assert!(world.contains("Old hallway search (stale)"));
        assert!(world.contains("Image profile bug resolved"));
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
            archived: false,
            is_pinned: false,
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
            truth_status: TruthStatus::Unknown,
            architecture_verified: false,
            memory_slot: None,
            owner_soul_id: None,
            relevance_tags: Default::default(),
            knowledge_scope: None,
            is_active: true,
            invalidated_by_patch_id: None,
            superseded_by_memory_id: None,
            is_retconned: false,
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
