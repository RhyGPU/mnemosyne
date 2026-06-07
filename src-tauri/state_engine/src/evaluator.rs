use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    patch::{
        EnginePatch, MemoryPatch, ObjectObservationOperationPatch, RelationshipDelta,
        SceneStatePatch, SoulPatch, WorldEventOperationPatch, WorldPatch, PATCH_PROTOCOL_VERSION,
    },
    setting::SessionWorld,
    soul::{MemorySourceType, ObjectState, Soul, TruthStatus},
};

pub const EVALUATOR_SCHEMA_VERSION: u32 = 1;

pub mod turn_flags {
    pub const SCENE_TURN: u64 = 1 << 0;
    pub const PURE_OOC: u64 = 1 << 1;
    pub const RETCON_OR_CORRECTION: u64 = 1 << 2;
    pub const WORLD_CHANGE: u64 = 1 << 3;
    pub const OBJECT_CHANGE: u64 = 1 << 4;
    pub const RELATIONSHIP_SHIFT: u64 = 1 << 5;
    pub const UNRESOLVED_TENSION: u64 = 1 << 6;
    pub const CURRENT_PLOT_ADVANCED: u64 = 1 << 7;
    pub const CHARACTER_IDENTITY_CHANGE: u64 = 1 << 8;
    pub const RECENT_EMOTIONAL_STATE: u64 = 1 << 9;
    pub const CONTRADICTION_DETECTED: u64 = 1 << 10;
    pub const USER_ACTION_PRESENT: u64 = 1 << 11;
    pub const CHARACTER_BOUNDARY_ASSERTED: u64 = 1 << 12;
    pub const USER_BOUNDARY_PRESSURE: u64 = 1 << 13;
    pub const MULTI_SOUL_SCENE: u64 = 1 << 14;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EvaluatorOutputV1 {
    pub schema_version: u32,
    #[serde(default)]
    pub thought_process: Option<String>,
    pub turn_flags_u64: u64,
    pub turn_classification: TurnClassification,
    pub global_scene_evaluation: GlobalSceneEvaluation,
    pub per_soul_evaluations: Vec<PerSoulEvaluation>,
    pub world_changes: Vec<WorldChangeEvaluation>,
    pub object_changes: Vec<ObjectChangeEvaluation>,
    pub relationship_evaluations: Vec<RelationshipEvaluation>,
    pub memory_candidates: Vec<MemoryCandidate>,
    pub relevance_tags: RelevanceTags,
    pub no_op_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct TurnClassification {
    pub is_pure_ooc: bool,
    pub scene_event_occurred: bool,
    pub is_retcon_or_correction: bool,
    pub human_summary: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GlobalSceneEvaluation {
    pub scene_event_occurred: bool,
    pub location_changed: bool,
    pub object_state_changed: bool,
    pub relationship_changed: bool,
    pub unresolved_tension: bool,
    pub current_plot_advanced: bool,
    pub character_identity_changed: bool,
    pub recent_emotional_state_changed: bool,
    pub contradiction_detected: bool,
    pub evidence_quote: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct PerSoulEvaluation {
    pub soul_id: String,
    pub observed: bool,
    pub knowledge_scope: KnowledgeScope,
    pub subjective_interpretation: String,
    pub emotional_state: Option<String>,
    pub relationship_deltas: Vec<RelationshipEvaluation>,
    pub memory_candidates: Vec<MemoryCandidate>,
    pub relevance_tags: RelevanceTags,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeScope {
    DirectlyObserved,
    HeardAbout,
    Inferred,
    #[default]
    NotKnown,
}

impl KnowledgeScope {
    pub fn as_label(self) -> &'static str {
        match self {
            KnowledgeScope::DirectlyObserved => "directly_observed",
            KnowledgeScope::HeardAbout => "heard_about",
            KnowledgeScope::Inferred => "inferred",
            KnowledgeScope::NotKnown => "not_known",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WorldChangeEvaluation {
    pub change_id: Option<String>,
    pub location: Option<String>,
    pub event_summary: Option<String>,
    pub scene_state: Option<SceneStatePatch>,
    pub active_plot_add: Vec<String>,
    pub active_plot_resolve: Vec<String>,
    pub evidence_quote: Option<String>,
    pub confidence: f32,
    pub relevance_tags: RelevanceTags,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ObjectChangeEvaluation {
    pub change_id: Option<String>,
    pub object_state: ObjectState,
    pub evidence_quote: Option<String>,
    pub confidence: f32,
    pub relevance_tags: RelevanceTags,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RelationshipEvaluation {
    pub source_soul_id: String,
    pub target_entity_id: String,
    pub trust: Option<f32>,
    pub affection: Option<f32>,
    pub intimacy: Option<f32>,
    pub passion: Option<f32>,
    pub commitment: Option<f32>,
    pub fear: Option<f32>,
    pub desire: Option<f32>,
    pub respect: Option<f32>,
    pub conflict: Option<f32>,
    pub dependency: Option<f32>,
    pub curiosity: Option<f32>,
    pub comfort: Option<f32>,
    pub boundary_pressure: Option<f32>,
    pub trustable_bias: Option<f32>,
    pub untrustworthy_bias: Option<f32>,
    pub asshole_bias: Option<f32>,
    pub care_bias: Option<f32>,
    pub danger_bias: Option<f32>,
    pub competence_bias: Option<f32>,
    pub autonomy_respect_bias: Option<f32>,
    pub attachment_pull: Option<f32>,
    pub schema_threat: Option<f32>,
    pub first_impression_strength: Option<f32>,
    pub first_impression_confidence: Option<f32>,
    pub reappraisal_debt: Option<f32>,
    pub reappraisal_state_code: Option<u8>,
    pub max_abs_delta: Option<f32>,
    pub evidence_quote: Option<String>,
    pub criterion_met: bool,
    pub confidence: f32,
    pub relevance_tags: RelevanceTags,
    /// When true, this relationship delta was produced by the form evaluator
    /// and its evidence quote was already validated at the row level during
    /// form parsing. Skips the secondary claim_has_evidence substring check
    /// in evaluator_output_to_engine_patch.
    pub evidence_validated_by_form: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MemorySlot {
    RelationshipMemory,
    CurrentPlotMemory,
    CharacterIdentityMemory,
    UnresolvedTension,
    WorldLocationMemory,
    RecentEmotionalState,
    #[default]
    Unknown,
}

impl MemorySlot {
    pub fn as_label(self) -> &'static str {
        match self {
            MemorySlot::RelationshipMemory => "relationship_memory",
            MemorySlot::CurrentPlotMemory => "current_plot_memory",
            MemorySlot::CharacterIdentityMemory => "character_identity_memory",
            MemorySlot::UnresolvedTension => "unresolved_tension",
            MemorySlot::WorldLocationMemory => "world_location_memory",
            MemorySlot::RecentEmotionalState => "recent_emotional_state",
            MemorySlot::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MemoryCandidate {
    pub candidate_id: String,
    pub owner_soul_id: String,
    pub slot: MemorySlot,
    pub content: String,
    pub evidence_quote: String,
    pub criterion_met: bool,
    pub confidence: f32,
    pub salience: Option<f32>,
    pub retrieval_strength: Option<f32>,
    pub perceived_by_entity_id: Option<String>,
    pub target_entity_ids: Vec<String>,
    pub source_type: MemorySourceType,
    pub truth_status: TruthStatus,
    pub relevance_tags: Vec<String>,
    pub knowledge_scope: KnowledgeScope,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RelevanceTags {
    pub setting_tags: HashMap<String, u8>,
    pub location_tags: HashMap<String, u8>,
    pub interacted_entities: HashMap<String, u8>,
    pub event_type_tags: HashMap<String, u8>,
    pub object_tags: HashMap<String, u8>,
    pub emotional_tags: HashMap<String, u8>,
    pub memory_slot_tags: HashMap<String, u8>,
    pub per_soul_relevance: HashMap<String, u8>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvaluatorConversionReport {
    pub patch: EnginePatch,
    pub accepted_candidate_ids: Vec<String>,
    pub rejected_candidates: Vec<EvaluatorCandidateRejection>,
    pub evidence_validations: Vec<EvidenceValidationTrace>,
    pub no_op: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatorCandidateRejection {
    pub candidate_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceValidationTrace {
    pub candidate_id: String,
    pub evidence_validation_raw: String,
    pub evidence_validation_normalized: String,
    pub evidence_validation_match_source: String,
    pub evidence_validation_result: bool,
}

#[derive(Debug, Clone)]
pub struct EvaluatorConversionContext<'a> {
    pub active_soul_id: &'a str,
    pub active_soul_ids: Vec<String>,
    pub latest_user_message: &'a str,
    pub latest_narrator_response: &'a str,
    pub session_world: Option<&'a SessionWorld>,
    pub baseline_recent_event_id: Option<String>,
}

impl EvaluatorOutputV1 {
    pub fn is_pure_ooc(&self) -> bool {
        self.turn_classification.is_pure_ooc || self.turn_flags_u64 & turn_flags::PURE_OOC != 0
    }
}

pub fn evaluator_output_to_engine_patch(
    output: &EvaluatorOutputV1,
    context: &EvaluatorConversionContext<'_>,
) -> EvaluatorConversionReport {
    let mut report = EvaluatorConversionReport::default();
    if output.schema_version != EVALUATOR_SCHEMA_VERSION {
        report.no_op = true;
        report
            .rejected_candidates
            .push(EvaluatorCandidateRejection {
                candidate_id: "schema_version".into(),
                reason: format!("unsupported evaluator schema {}", output.schema_version),
            });
        return report;
    }
    if output.is_pure_ooc() {
        report.no_op = true;
        return report;
    }

    let evidence_text = format!(
        "{}\n{}",
        context.latest_user_message, context.latest_narrator_response
    );
    let user_evidence_text = context.latest_user_message.to_string();
    let narrator_evidence_text = strip_status_block_lines(context.latest_narrator_response);
    let active_souls = context
        .active_soul_ids
        .iter()
        .map(|id| id.as_str())
        .collect::<HashSet<_>>();

    let mut patch = EnginePatch {
        schema_version: Some(PATCH_PROTOCOL_VERSION),
        ..EnginePatch::default()
    };
    let mut world_patch = WorldPatch::default();
    for change in &output.world_changes {
        if !claim_has_evidence(change.evidence_quote.as_deref(), &evidence_text) {
            continue;
        }
        if change.confidence < 0.45 {
            continue;
        }
        if let Some(location) = clean(&change.location) {
            world_patch.location = Some(location.to_string());
        }
        if let Some(event) = clean(&change.event_summary) {
            world_patch.event_operations.push(WorldEventOperationPatch {
                operation: "add_recent_event".into(),
                recent_event_id: Some(stable_id(
                    "event",
                    change.change_id.as_deref().unwrap_or(event),
                )),
                content: Some(event.to_string()),
                ..WorldEventOperationPatch::default()
            });
        }
        if let Some(scene_state) = &change.scene_state {
            world_patch.scene_state = Some(scene_state.clone());
        }
        world_patch
            .active_plot_add
            .extend(change.active_plot_add.iter().filter_map(|v| clean_str(v)));
        world_patch.active_plot_resolve.extend(
            change
                .active_plot_resolve
                .iter()
                .filter_map(|v| clean_str(v)),
        );
    }
    for change in &output.object_changes {
        let objective_allowed = objective_object_change_has_structured_evidence(change);
        if !objective_allowed
            && !claim_has_evidence(change.evidence_quote.as_deref(), &evidence_text)
        {
            continue;
        }
        if change.confidence < 0.5 || change.object_state.object_id.trim().is_empty() {
            continue;
        }
        let mut object_state = change.object_state.clone();
        object_state.object_observation_id = Some(stable_id(
            "object",
            change
                .change_id
                .as_deref()
                .unwrap_or(object_state.object_id.as_str()),
        ));
        world_patch
            .object_observation_operations
            .push(ObjectObservationOperationPatch {
                operation: "update_object_state".into(),
                object_observation_id: object_state.object_observation_id.clone(),
                object_state: Some(object_state),
                ..ObjectObservationOperationPatch::default()
            });
    }
    if !world_patch.is_empty_for_commands() {
        patch.world_patch = Some(world_patch);
    }

    let mut soul_patch = SoulPatch::default();
    for relation in output.relationship_evaluations.iter().chain(
        output
            .per_soul_evaluations
            .iter()
            .flat_map(|s| s.relationship_deltas.iter()),
    ) {
        if relation.source_soul_id != context.active_soul_id {
            continue;
        }
        if !relation.criterion_met || relation.confidence < 0.55 {
            continue;
        }
        if !relation.evidence_validated_by_form
            && !claim_has_evidence(relation.evidence_quote.as_deref(), &evidence_text)
        {
            continue;
        }
        soul_patch.relationship_deltas.push(RelationshipDelta {
            relationship_event_id: Some(stable_id(
                "rel",
                &format!("{}:{}", relation.source_soul_id, relation.target_entity_id),
            )),
            from: Some(relation.source_soul_id.clone()),
            target: Some(relation.target_entity_id.clone()),
            trust: relation.trust,
            affection: relation.affection,
            intimacy: relation.intimacy,
            passion: relation.passion,
            commitment: relation.commitment,
            fear: relation.fear,
            desire: relation.desire,
            respect: relation.respect,
            conflict: relation.conflict,
            dependency: relation.dependency,
            curiosity: relation.curiosity,
            comfort: relation.comfort,
            boundary_pressure: relation.boundary_pressure,
            trustable_bias: relation.trustable_bias,
            untrustworthy_bias: relation.untrustworthy_bias,
            asshole_bias: relation.asshole_bias,
            care_bias: relation.care_bias,
            danger_bias: relation.danger_bias,
            competence_bias: relation.competence_bias,
            autonomy_respect_bias: relation.autonomy_respect_bias,
            attachment_pull: relation.attachment_pull,
            schema_threat: relation.schema_threat,
            first_impression_strength: relation.first_impression_strength,
            first_impression_confidence: relation.first_impression_confidence,
            reappraisal_debt: relation.reappraisal_debt,
            reappraisal_state_code: relation.reappraisal_state_code,
            max_abs_delta: relation.max_abs_delta,
        });
    }

    let candidates = output
        .memory_candidates
        .iter()
        .map(|candidate| (candidate, false))
        .chain(output.per_soul_evaluations.iter().flat_map(|s| {
            s.memory_candidates
                .iter()
                .map(|candidate| (candidate, true))
        }));
    let mut candidates = candidates.collect::<Vec<_>>();
    candidates.sort_by(|(left, left_nested), (right, right_nested)| {
        candidate_quality_score(right, *right_nested, context, &active_souls, &evidence_text).cmp(
            &candidate_quality_score(left, *left_nested, context, &active_souls, &evidence_text),
        )
    });
    let mut seen_candidates = HashSet::new();
    for candidate in candidates {
        let (candidate, _nested_origin) = candidate;
        if !seen_candidates.insert(candidate.candidate_id.clone()) {
            reject(&mut report, candidate, "duplicate candidate id");
            continue;
        }
        if candidate.owner_soul_id != context.active_soul_id {
            if active_souls.contains(candidate.owner_soul_id.as_str()) {
                reject(
                    &mut report,
                    candidate,
                    "candidate belongs to a different active Soul",
                );
            } else {
                reject(
                    &mut report,
                    candidate,
                    "candidate owner_soul_id is not active",
                );
            }
            continue;
        }
        match validate_memory_candidate(candidate, &evidence_text) {
            Ok(()) => {
                report
                    .evidence_validations
                    .push(evidence_validation_trace_for_candidate(
                        candidate,
                        &user_evidence_text,
                        &narrator_evidence_text,
                    ));
                let tags = candidate
                    .relevance_tags
                    .iter()
                    .map(|tag| (tag.clone(), 100u8))
                    .chain(flatten_relevance_tags(&output.relevance_tags))
                    .collect::<HashMap<_, _>>();
                soul_patch.new_memories.push(MemoryPatch {
                    operation: Some("add".into()),
                    memory_id: Some(stable_id("memory", &candidate.candidate_id)),
                    content: candidate.content.trim().to_string(),
                    tag: Some(candidate.slot.as_label().into()),
                    source_type: Some(candidate.source_type),
                    source_entity_id: Some(candidate.owner_soul_id.clone()),
                    perceived_by_entity_id: candidate
                        .perceived_by_entity_id
                        .clone()
                        .or_else(|| Some(candidate.owner_soul_id.clone())),
                    target_entity_ids: candidate.target_entity_ids.clone(),
                    interpretation: (!candidate.content.trim().is_empty())
                        .then(|| candidate.content.trim().to_string()),
                    confidence: Some(candidate.confidence.clamp(0.0, 1.0)),
                    salience: clamp_optional_score(candidate.salience),
                    retrieval_strength: clamp_optional_score(candidate.retrieval_strength),
                    truth_status: Some(candidate.truth_status),
                    architecture_verified: Some(false),
                    memory_slot: Some(candidate.slot.as_label().into()),
                    owner_soul_id: Some(candidate.owner_soul_id.clone()),
                    relevance_tags: tags,
                    knowledge_scope: Some(candidate.knowledge_scope.as_label().into()),
                    ..MemoryPatch::default()
                });
                report
                    .accepted_candidate_ids
                    .push(candidate.candidate_id.clone());
            }
            Err(reason) => {
                report
                    .evidence_validations
                    .push(evidence_validation_trace_for_candidate(
                        candidate,
                        &user_evidence_text,
                        &narrator_evidence_text,
                    ));
                reject(&mut report, candidate, reason);
            }
        }
    }

    if !soul_patch.is_empty_for_evaluator() {
        patch.soul_patch = Some(soul_patch);
    }
    report.no_op = patch.is_empty();
    report.patch = patch;
    report
}

fn candidate_quality_score(
    candidate: &MemoryCandidate,
    nested_origin: bool,
    context: &EvaluatorConversionContext<'_>,
    active_souls: &HashSet<&str>,
    evidence_text: &str,
) -> i32 {
    let mut score = 0;
    if candidate.owner_soul_id == context.active_soul_id {
        score += 100;
    } else if active_souls.contains(candidate.owner_soul_id.as_str()) {
        score += 50;
    }
    if !candidate.content.trim().is_empty() {
        score += 40;
    }
    if claim_has_evidence(Some(&candidate.evidence_quote), evidence_text) {
        score += 30;
    }
    if candidate.criterion_met {
        score += 20;
    }
    score += (candidate.confidence.clamp(0.0, 1.0) * 10.0) as i32;
    score += (candidate.salience.unwrap_or_default().clamp(0.0, 100.0) / 10.0) as i32;
    if nested_origin && candidate.owner_soul_id == context.active_soul_id {
        score += 12;
    }
    if !nested_origin && candidate.owner_soul_id.trim().is_empty() {
        score += 4;
    }
    score
}

trait SoulPatchEvaluatorExt {
    fn is_empty_for_evaluator(&self) -> bool;
}

impl SoulPatchEvaluatorExt for SoulPatch {
    fn is_empty_for_evaluator(&self) -> bool {
        self.relationship_delta.is_none()
            && self.relationship_deltas.is_empty()
            && self.new_memories.is_empty()
            && self.memory_operations.is_empty()
    }
}

fn validate_memory_candidate(
    candidate: &MemoryCandidate,
    evidence_text: &str,
) -> Result<(), &'static str> {
    if !candidate.criterion_met {
        return Err("criterion not met");
    }
    if candidate.confidence < 0.55 {
        return Err("confidence below threshold");
    }
    if candidate.content.trim().is_empty() {
        return Err("empty content");
    }
    if candidate.owner_soul_id.trim().is_empty() {
        return Err("missing owner_soul_id");
    }
    if candidate.slot == MemorySlot::Unknown {
        return Err("unknown memory slot");
    }
    if candidate.slot == MemorySlot::WorldLocationMemory
        && looks_like_emotional_fact(&candidate.content)
        && !looks_like_location_fact(&candidate.content)
    {
        return Err("world_location_memory cannot store a purely emotional fact");
    }
    if is_low_value_memory(&candidate.content) {
        return Err("generic low-value memory");
    }
    if !claim_has_evidence(Some(&candidate.evidence_quote), evidence_text) {
        return Err("missing or invalid evidence_quote");
    }
    Ok(())
}

fn reject(
    report: &mut EvaluatorConversionReport,
    candidate: &MemoryCandidate,
    reason: impl Into<String>,
) {
    report
        .rejected_candidates
        .push(EvaluatorCandidateRejection {
            candidate_id: candidate.candidate_id.clone(),
            reason: reason.into(),
        });
}

fn claim_has_evidence(quote: Option<&str>, evidence_text: &str) -> bool {
    let Some(quote) = quote.map(str::trim).filter(|quote| !quote.is_empty()) else {
        return false;
    };
    let quote_lower = normalize_evidence_quote(quote);
    if quote_lower.len() < 4 {
        return false;
    }
    let evidence_lower = normalize_evidence(evidence_text);
    evidence_contains_quote(&evidence_lower, &quote_lower)
}

fn evidence_validation_trace_for_candidate(
    candidate: &MemoryCandidate,
    user_text: &str,
    narrator_text_without_status: &str,
) -> EvidenceValidationTrace {
    let normalized = normalize_evidence_quote(&candidate.evidence_quote);
    let user = normalize_evidence(user_text);
    let narrator = normalize_evidence(narrator_text_without_status);
    let match_source = if evidence_contains_quote(&user, &normalized) {
        "user"
    } else if evidence_contains_quote(&narrator, &normalized) {
        "narrator"
    } else {
        "none"
    };
    EvidenceValidationTrace {
        candidate_id: candidate.candidate_id.clone(),
        evidence_validation_raw: candidate.evidence_quote.clone(),
        evidence_validation_normalized: normalized,
        evidence_validation_match_source: match_source.into(),
        evidence_validation_result: match_source != "none",
    }
}

fn evidence_contains_quote(evidence_lower: &str, quote_lower: &str) -> bool {
    if quote_lower.len() < 4 {
        return false;
    }
    if evidence_lower.contains(quote_lower) {
        return true;
    }
    if quote_lower.len() < 24 {
        return false;
    }
    quote_lower
        .split("...")
        .flat_map(|part| part.split('…'))
        .map(str::trim)
        .filter(|part| part.len() >= 24)
        .any(|part| evidence_lower.contains(part))
}

fn normalize_evidence_quote(text: &str) -> String {
    let mut cleaned = text
        .trim()
        .replace("\\\"", "\"")
        .replace(['“', '”'], "\"")
        .replace(['‘', '’'], "'");
    loop {
        let trimmed = cleaned.trim();
        if trimmed.len() >= 2
            && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
                || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
        {
            cleaned = trimmed[1..trimmed.len() - 1].to_string();
        } else {
            break;
        }
    }
    normalize_evidence(&cleaned)
}

fn normalize_evidence(text: &str) -> String {
    strip_status_block_lines(text)
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_status_block_lines(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            let lower = trimmed.to_ascii_lowercase();
            !lower.starts_with("scene |")
                && !lower.starts_with("status |")
                && !lower.starts_with("physical state:")
                && !lower.starts_with("atmosphere:")
                && !lower.starts_with("focus:")
                && !lower.starts_with("```status")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_low_value_memory(content: &str) -> bool {
    let lower = normalize_evidence(content);
    [
        "she looked tense",
        "she listened carefully",
        "she narrowed her eyes",
        "she watched the user",
        "looked tense",
        "listened carefully",
        "narrowed her eyes",
        "watched the user",
    ]
    .iter()
    .any(|phrase| lower == *phrase || lower.contains(phrase))
}

fn looks_like_emotional_fact(content: &str) -> bool {
    let lower = normalize_evidence(content);
    [
        "felt",
        "afraid",
        "angry",
        "tense",
        "comforted",
        "ashamed",
        "relieved",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn looks_like_location_fact(content: &str) -> bool {
    let lower = normalize_evidence(content);
    [
        "bar", "room", "location", "door", "kitchen", "street", "where", "at ",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn objective_object_change_has_structured_evidence(change: &ObjectChangeEvaluation) -> bool {
    change.object_state.object_id.trim().len() >= 2
        && (!change.object_state.last_observed_state.trim().is_empty()
            || change.object_state.notification_mode != "unknown"
            || change.object_state.power_state != "unknown"
            || change.object_state.open_state.is_some()
            || change.object_state.lock_state.is_some())
}

fn clamp_optional_score(value: Option<f32>) -> Option<f32> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 100.0))
}

fn clean(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn clean_str(value: &String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn stable_id(prefix: &str, source: &str) -> String {
    let mut hash: u64 = 1469598103934665603;
    for byte in source.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{prefix}_{hash:016x}")
}

fn flatten_relevance_tags(tags: &RelevanceTags) -> impl Iterator<Item = (String, u8)> + '_ {
    tags.setting_tags
        .iter()
        .chain(tags.location_tags.iter())
        .chain(tags.interacted_entities.iter())
        .chain(tags.event_type_tags.iter())
        .chain(tags.object_tags.iter())
        .chain(tags.emotional_tags.iter())
        .chain(tags.memory_slot_tags.iter())
        .map(|(key, value)| (key.clone(), (*value).min(100)))
}

pub fn active_souls_for_v1(primary: &Soul) -> Vec<String> {
    vec![primary.character_id.clone()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{setting::session_world_from_legacy_world, soul::new_default_soul};

    fn base_output(soul_id: &str) -> EvaluatorOutputV1 {
        EvaluatorOutputV1 {
            schema_version: EVALUATOR_SCHEMA_VERSION,
            per_soul_evaluations: vec![PerSoulEvaluation {
                soul_id: soul_id.into(),
                observed: true,
                knowledge_scope: KnowledgeScope::DirectlyObserved,
                ..PerSoulEvaluation::default()
            }],
            ..EvaluatorOutputV1::default()
        }
    }

    fn context<'a>(
        soul: &'a Soul,
        user: &'a str,
        narrator: &'a str,
        world: &'a SessionWorld,
    ) -> EvaluatorConversionContext<'a> {
        EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: user,
            latest_narrator_response: narrator,
            session_world: Some(world),
            baseline_recent_event_id: None,
        }
    }

    fn memory_candidate(soul_id: &str, id: &str, content: &str, quote: &str) -> MemoryCandidate {
        MemoryCandidate {
            candidate_id: id.into(),
            owner_soul_id: soul_id.into(),
            slot: MemorySlot::RelationshipMemory,
            content: content.into(),
            evidence_quote: quote.into(),
            criterion_met: true,
            confidence: 0.82,
            salience: Some(82.0),
            retrieval_strength: Some(82.0),
            perceived_by_entity_id: Some(soul_id.into()),
            target_entity_ids: vec!["default_player".into()],
            source_type: MemorySourceType::CurrentSession,
            truth_status: TruthStatus::SceneEvent,
            relevance_tags: vec!["boundary_pressure".into()],
            knowledge_scope: KnowledgeScope::DirectlyObserved,
        }
    }

    #[test]
    fn pure_ooc_evaluator_noop() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Test", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output.turn_flags_u64 = turn_flags::PURE_OOC;
        output.turn_classification.is_pure_ooc = true;
        output.no_op_reason = Some("GM-only note".into());

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(&soul, "(OOC) pause", "Paused.", &world),
        );

        assert!(report.no_op);
        assert!(report.patch.is_empty());
    }

    #[test]
    fn scene_turn_sets_scene_flag() {
        let mut output = base_output("aurora");
        output.turn_flags_u64 = turn_flags::SCENE_TURN | turn_flags::USER_ACTION_PRESENT;
        output.turn_classification.scene_event_occurred = true;

        assert_ne!(output.turn_flags_u64 & turn_flags::SCENE_TURN, 0);
        assert!(output.turn_classification.scene_event_occurred);
    }

    #[test]
    fn door_knock_creates_world_event() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output.turn_flags_u64 = turn_flags::SCENE_TURN | turn_flags::WORLD_CHANGE;
        output.world_changes.push(WorldChangeEvaluation {
            change_id: Some("door_knock".into()),
            event_summary: Some("Someone knocked on Aurora's door.".into()),
            evidence_quote: Some("knock lands on Aurora's door".into()),
            confidence: 0.8,
            ..WorldChangeEvaluation::default()
        });

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(&soul, "I knock.", "A knock lands on Aurora's door.", &world),
        );

        assert!(report.patch.world_patch.unwrap().event_operations[0]
            .content
            .as_deref()
            .unwrap()
            .contains("knocked"));
    }

    #[test]
    fn phone_notifications_off_blocks_chime_screenwake() {
        let soul = new_default_soul("Aurora");
        let mut world = session_world_from_legacy_world("Apartment", None, &soul.world);
        world.object_states.push(ObjectState {
            object_id: "aurora_phone".into(),
            object_kind: "phone".into(),
            notification_mode: "notifications_off".into(),
            vibrate_enabled: Some(false),
            screen_wake_enabled: Some(false),
            last_observed_state: "notifications off".into(),
            confidence: 0.9,
            ..ObjectState::default()
        });
        let mut output = base_output(&soul.character_id);
        output.object_changes.push(ObjectChangeEvaluation {
            change_id: Some("phone_off".into()),
            object_state: world.object_states[0].clone(),
            confidence: 0.9,
            ..ObjectChangeEvaluation::default()
        });

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(
                &soul,
                "I turn notifications off.",
                "The phone stays dark and silent.",
                &world,
            ),
        );

        let object = report
            .patch
            .world_patch
            .unwrap()
            .object_observation_operations[0]
            .object_state
            .as_ref()
            .unwrap()
            .clone();
        assert_eq!(object.notification_mode, "notifications_off");
        assert_eq!(object.screen_wake_enabled, Some(false));
    }

    #[test]
    fn boundary_scene_creates_unresolved_tension_candidate() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        let mut candidate = memory_candidate(
            &soul.character_id,
            "boundary",
            "Aurora remembers the user's boundary pressure remained unresolved.",
            "don't push past that boundary",
        );
        candidate.slot = MemorySlot::UnresolvedTension;
        output.memory_candidates.push(candidate);

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(
                &soul,
                "Don't push past that boundary.",
                "Aurora lets the boundary hang unresolved.",
                &world,
            ),
        );

        assert_eq!(report.accepted_candidate_ids, vec!["boundary"]);
        assert_eq!(
            report.patch.soul_patch.unwrap().new_memories[0]
                .memory_slot
                .as_deref(),
            Some("unresolved_tension")
        );
    }

    #[test]
    fn relationship_shift_creates_relationship_candidate() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output
            .relationship_evaluations
            .push(RelationshipEvaluation {
                source_soul_id: soul.character_id.clone(),
                target_entity_id: "default_player".into(),
                trust: Some(2.0),
                comfort: Some(1.0),
                evidence_quote: Some("I promise I won't leave".into()),
                criterion_met: true,
                confidence: 0.8,
                ..RelationshipEvaluation::default()
            });

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(
                &soul,
                "I promise I won't leave.",
                "Aurora visibly trusts the promise a little.",
                &world,
            ),
        );

        assert_eq!(
            report.patch.soul_patch.unwrap().relationship_deltas[0].trust,
            Some(2.0)
        );
    }

    #[test]
    fn repeated_objectification_creates_boundary_pressure() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output
            .relationship_evaluations
            .push(RelationshipEvaluation {
                source_soul_id: soul.character_id.clone(),
                target_entity_id: "default_player".into(),
                boundary_pressure: Some(3.0),
                conflict: Some(1.0),
                evidence_quote: Some("ignores her no again".into()),
                criterion_met: true,
                confidence: 0.85,
                ..RelationshipEvaluation::default()
            });

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(
                &soul,
                "I ignore her no again.",
                "The user ignores her no again.",
                &world,
            ),
        );

        assert_eq!(
            report.patch.soul_patch.unwrap().relationship_deltas[0].boundary_pressure,
            Some(3.0)
        );
    }

    #[test]
    fn generic_body_language_memory_rejected() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output.memory_candidates.push(memory_candidate(
            &soul.character_id,
            "generic",
            "She looked tense.",
            "looked tense",
        ));

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(&soul, "I wait.", "She looked tense.", &world),
        );

        assert_eq!(
            report.rejected_candidates[0].reason,
            "generic low-value memory"
        );
    }

    #[test]
    fn world_location_memory_rejects_emotional_fact() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        let mut candidate = memory_candidate(
            &soul.character_id,
            "bad_location",
            "Aurora felt afraid.",
            "felt afraid",
        );
        candidate.slot = MemorySlot::WorldLocationMemory;
        output.memory_candidates.push(candidate);

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(&soul, "I step closer.", "Aurora felt afraid.", &world),
        );

        assert_eq!(
            report.rejected_candidates[0].reason,
            "world_location_memory cannot store a purely emotional fact"
        );
    }

    #[test]
    fn multi_soul_same_event_different_interpretations() {
        let mut output = base_output("aurora");
        output.per_soul_evaluations = vec![
            PerSoulEvaluation {
                soul_id: "a".into(),
                observed: true,
                subjective_interpretation: "A meant it as criticism.".into(),
                ..PerSoulEvaluation::default()
            },
            PerSoulEvaluation {
                soul_id: "b".into(),
                observed: true,
                subjective_interpretation: "B took it as helpful advice.".into(),
                ..PerSoulEvaluation::default()
            },
        ];
        assert_ne!(
            output.per_soul_evaluations[0].subjective_interpretation,
            output.per_soul_evaluations[1].subjective_interpretation
        );
    }

    #[test]
    fn evaluator_output_converts_to_engine_patch() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output.memory_candidates.push(memory_candidate(
            &soul.character_id,
            "promise",
            "Aurora treats the user's promise as a relationship turning point.",
            "I promise",
        ));

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(&soul, "I promise.", "Aurora hears: I promise.", &world),
        );

        assert!(!report.patch.is_empty());
        assert_eq!(report.accepted_candidate_ids, vec!["promise"]);
    }

    #[test]
    fn evaluator_candidate_salience_is_preserved() {
        let mut soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        let mut candidate = memory_candidate(
            &soul.character_id,
            "salience",
            "Aurora treats the user's boundary promise as highly important.",
            "boundary promise",
        );
        candidate.salience = Some(97.5);
        candidate.retrieval_strength = None;
        output.memory_candidates.push(candidate);

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(
                &soul,
                "I make a boundary promise.",
                "Aurora hears the boundary promise.",
                &world,
            ),
        );
        report
            .patch
            .apply_to_soul(&mut soul)
            .expect("patch applies");

        let memory = soul
            .memory
            .recent
            .iter()
            .find(|memory| memory.id.contains("memory_"))
            .unwrap();
        assert_eq!(memory.salience, 97.5);
    }

    #[test]
    fn evaluator_candidate_retrieval_strength_is_preserved() {
        let mut soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        let mut candidate = memory_candidate(
            &soul.character_id,
            "retrieval",
            "Aurora remembers the user's promise as easy to recall.",
            "easy to recall",
        );
        candidate.salience = None;
        candidate.retrieval_strength = Some(91.0);
        output.memory_candidates.push(candidate);

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(
                &soul,
                "This should be easy to recall.",
                "Aurora marks it easy to recall.",
                &world,
            ),
        );
        report
            .patch
            .apply_to_soul(&mut soul)
            .expect("patch applies");

        let memory = soul
            .memory
            .recent
            .iter()
            .find(|memory| memory.id.contains("memory_"))
            .unwrap();
        assert_eq!(memory.retrieval_strength, 91.0);
    }

    #[test]
    fn omitted_salience_uses_existing_fallback() {
        let mut soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        let mut candidate = memory_candidate(
            &soul.character_id,
            "fallback",
            "Aurora remembers the user's promise without evaluator-provided scoring.",
            "without evaluator-provided scoring",
        );
        candidate.salience = None;
        candidate.retrieval_strength = None;
        output.memory_candidates.push(candidate);

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(
                &soul,
                "This is without evaluator-provided scoring.",
                "Aurora stores it without evaluator-provided scoring.",
                &world,
            ),
        );
        let patch_memory = &report.patch.soul_patch.as_ref().unwrap().new_memories[0];
        assert_eq!(patch_memory.salience, None);
        assert_eq!(patch_memory.retrieval_strength, None);

        report
            .patch
            .apply_to_soul(&mut soul)
            .expect("patch applies");
        let memory = soul
            .memory
            .recent
            .iter()
            .find(|memory| memory.id.contains("memory_"))
            .unwrap();
        assert!(memory.salience > 0.0);
        assert!(memory.retrieval_strength > 0.0);
    }

    #[test]
    fn invalid_evidence_quote_rejected() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output.memory_candidates.push(memory_candidate(
            &soul.character_id,
            "invented",
            "Aurora remembers an invented betrayal.",
            "this quote is absent",
        ));

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(&soul, "I sit down.", "Aurora nods.", &world),
        );

        assert_eq!(
            report.rejected_candidates[0].reason,
            "missing or invalid evidence_quote"
        );
    }

    #[test]
    fn memory_quote_with_escaped_outer_quotes_validates() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output.memory_candidates.push(memory_candidate(
            &soul.character_id,
            "escaped",
            "Aurora remembers the visitor's return as emotionally charged.",
            "\\\"Long time no see, Aurora.\\\"",
        ));

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(&soul, "I walk in. Long time no see, Aurora.", "", &world),
        );

        assert_eq!(report.accepted_candidate_ids, vec!["escaped"]);
        assert_eq!(
            report.evidence_validations[0].evidence_validation_match_source,
            "user"
        );
    }

    #[test]
    fn memory_quote_from_narrator_without_status_block_validates() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output.memory_candidates.push(memory_candidate(
            &soul.character_id,
            "statusless",
            "Aurora remembers opening the door after the knock.",
            "She unlocks the door and pulls it open just enough to stand in the gap.",
        ));

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(
                &soul,
                "I knock.",
                "She unlocks the door and pulls it open just enough to stand in the gap.\nScene | Focus: Aurora | Physical state: Not specified",
                &world,
            ),
        );

        assert_eq!(report.accepted_candidate_ids, vec!["statusless"]);
        assert_eq!(
            report.evidence_validations[0].evidence_validation_match_source,
            "narrator"
        );
    }

    #[test]
    fn duplicate_candidate_prefers_valid_nested_owner() {
        let soul = new_default_soul("Aurora");
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let mut output = base_output(&soul.character_id);
        output.memory_candidates.push(MemoryCandidate {
            candidate_id: "same".into(),
            owner_soul_id: "".into(),
            slot: MemorySlot::RelationshipMemory,
            content: "".into(),
            evidence_quote: "I promise".into(),
            criterion_met: false,
            confidence: 0.2,
            ..MemoryCandidate::default()
        });
        output.per_soul_evaluations[0]
            .memory_candidates
            .push(memory_candidate(
                &soul.character_id,
                "same",
                "Aurora treats the user's promise as meaningful.",
                "I promise",
            ));

        let report = evaluator_output_to_engine_patch(
            &output,
            &context(&soul, "I promise.", "Aurora hears: I promise.", &world),
        );

        assert_eq!(report.accepted_candidate_ids, vec!["same"]);
        assert!(report
            .rejected_candidates
            .iter()
            .any(|rejection| rejection.reason == "duplicate candidate id"));
    }
}
