use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    evaluator::{
        EvaluatorCandidateRejection, EvaluatorConversionContext, EvaluatorConversionReport,
    },
    patch::{
        EnginePatch, MemoryPatch, ObjectObservationOperationPatch, RelationshipDelta,
        SceneStatePatch, SoulPatch, WorldEventOperationPatch, WorldPatch, PATCH_PROTOCOL_VERSION,
    },
    soul::{MemorySourceType, ObjectState, Soul, TruthStatus},
};

pub const EVALUATOR_STRUCTURED_SCHEMA_VERSION: u32 = 1;
pub const EVALUATOR_OPS_SCHEMA_NAME: &str = "evaluator_structured_ops_v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvaluatorStructuredOutputV1 {
    pub schema_version: u32,
    pub ops: Vec<EvaluatorOp>,
    pub no_op_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvaluatorOp {
    AddMemory(AddMemoryOp),
    RelationshipEvent(RelationshipEventOp),
    UpdateObjectState(UpdateObjectStateOp),
    UpdateSceneState(UpdateSceneStateOp),
    AddWorldEvent(AddWorldEventOp),
    NoOp(NoOp),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AddMemoryOp {
    pub owner_soul_id: String,
    pub slot: MemorySlotOp,
    pub content: String,
    pub evidence_quote: String,
    pub confidence: f32,
    pub salience: u32,
    pub source_message_id: Option<i64>,
    pub target_entity_ids: Vec<String>,
    pub truth_status: TruthStatusOp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RelationshipEventOp {
    pub source_soul_id: String,
    pub target_entity_id: String,
    pub actor_entity_id: String,
    pub perceived_by_entity_id: String,
    pub evidence_quote: String,
    pub axes: RelationshipAxes,
    pub modifiers: RelationshipModifiers,
    pub event_flags_u64: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RelationshipAxes {
    pub intent: i32,
    pub honesty: i32,
    pub reliability: i32,
    pub boundary_treatment: i32,
    pub responsiveness: i32,
    pub power_use: i32,
    pub evaluation_tone: i32,
    pub competence: i32,
    pub disclosure: i32,
    pub reciprocity: i32,
    pub repair: i32,
    pub predictability: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RelationshipModifiers {
    pub salience: u32,
    pub certainty: u32,
    pub directness: u32,
    pub costliness: u32,
    pub stakes: u32,
    pub repetition: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdateObjectStateOp {
    pub object_label: String,
    pub object_type: String,
    pub owner_entity_id: String,
    pub status: String,
    pub location: String,
    pub last_observed_state: String,
    pub evidence_quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdateSceneStateOp {
    pub current_scene: String,
    pub focus: String,
    pub participants: Vec<String>,
    pub last_user_action: String,
    pub pressure_point: String,
    pub continuity_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AddWorldEventOp {
    pub content: String,
    pub evidence_quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NoOp {
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySlotOp {
    RelationshipMemory,
    CurrentPlotMemory,
    CharacterIdentityMemory,
    UnresolvedTension,
    WorldLocationMemory,
    RecentEmotionalState,
}

impl MemorySlotOp {
    fn as_label(self) -> &'static str {
        match self {
            Self::RelationshipMemory => "relationship_memory",
            Self::CurrentPlotMemory => "current_plot_memory",
            Self::CharacterIdentityMemory => "character_identity_memory",
            Self::UnresolvedTension => "unresolved_tension",
            Self::WorldLocationMemory => "world_location_memory",
            Self::RecentEmotionalState => "recent_emotional_state",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TruthStatusOp {
    Fiction,
    SceneEvent,
    CharacterBelief,
    NarratorClaim,
    UserClaimed,
    VerifiedEngine,
    ActualSystemEvent,
    Unknown,
}

impl From<TruthStatusOp> for TruthStatus {
    fn from(value: TruthStatusOp) -> Self {
        match value {
            TruthStatusOp::Fiction => TruthStatus::Fiction,
            TruthStatusOp::SceneEvent => TruthStatus::SceneEvent,
            TruthStatusOp::CharacterBelief => TruthStatus::CharacterBelief,
            TruthStatusOp::NarratorClaim => TruthStatus::NarratorClaim,
            TruthStatusOp::UserClaimed => TruthStatus::UserClaimed,
            TruthStatusOp::VerifiedEngine => TruthStatus::VerifiedEngine,
            TruthStatusOp::ActualSystemEvent => TruthStatus::ActualSystemEvent,
            TruthStatusOp::Unknown => TruthStatus::Unknown,
        }
    }
}

pub fn compile_evaluator_ops_to_engine_patch(
    output: &EvaluatorStructuredOutputV1,
    context: &EvaluatorConversionContext<'_>,
    soul: &Soul,
) -> Result<EvaluatorConversionReport, String> {
    if output.schema_version != EVALUATOR_STRUCTURED_SCHEMA_VERSION {
        return Err(format!(
            "unsupported evaluator structured schema {}",
            output.schema_version
        ));
    }

    let mut alias_trace = EntityAliasTrace::default();
    let evidence_text = normalized_evidence_text(context);
    let mut patch = EnginePatch {
        schema_version: Some(PATCH_PROTOCOL_VERSION),
        ..EnginePatch::default()
    };
    let mut accepted_candidate_ids = Vec::new();
    // Partial-accept: resolve + validate each op independently. A single bad op
    // is dropped and logged (rejected_candidates) instead of discarding the whole
    // turn's extraction. Only a *total* miss (below) falls through to the form-v1
    // fallback, preserving the recovery ladder.
    let mut rejected_candidates: Vec<EvaluatorCandidateRejection> = Vec::new();

    for (index, raw_op) in output.ops.iter().enumerate() {
        let mut op = raw_op.clone();
        let outcome: Result<(), String> = (|| {
            resolve_op_entity_aliases(&mut op, index, context, soul, &mut alias_trace)?;
            match &op {
            EvaluatorOp::NoOp(_) => {}
            EvaluatorOp::AddMemory(op) => {
                validate_soul_id(&op.owner_soul_id, context)?;
                validate_entities(&op.target_entity_ids, context, soul)?;
                validate_evidence(&op.evidence_quote, &evidence_text)?;
                let soul_patch = patch.soul_patch.get_or_insert_with(SoulPatch::default);
                soul_patch.new_memories.push(MemoryPatch {
                    memory_id: Some(stable_id("memory_ops", &format!("{index}:{}", op.content))),
                    content: op.content.clone(),
                    tag: Some(op.slot.as_label().to_string()),
                    source_type: Some(MemorySourceType::CurrentSession),
                    source_message_id: op.source_message_id,
                    // Persist the validated evidence quote as the memory's source
                    // line (the "quote" half of address/quote provenance).
                    source_quote: non_empty(&op.evidence_quote),
                    perceived_by_entity_id: Some(op.owner_soul_id.clone()),
                    target_entity_ids: op.target_entity_ids.clone(),
                    confidence: Some(op.confidence.clamp(0.0, 1.0)),
                    salience: Some(op.salience.min(100) as f32),
                    retrieval_strength: Some(op.salience.min(100) as f32),
                    truth_status: Some(op.truth_status.into()),
                    memory_slot: Some(op.slot.as_label().to_string()),
                    owner_soul_id: Some(op.owner_soul_id.clone()),
                    architecture_verified: Some(false),
                    ..MemoryPatch::default()
                });
                accepted_candidate_ids.push(format!("op:{index}:add_memory"));
            }
            EvaluatorOp::RelationshipEvent(op) => {
                validate_soul_id(&op.source_soul_id, context)?;
                validate_entity(&op.target_entity_id, context, soul)?;
                validate_entity(&op.actor_entity_id, context, soul)?;
                validate_soul_id(&op.perceived_by_entity_id, context)?;
                validate_evidence(&op.evidence_quote, &evidence_text)?;
                let delta = relationship_delta_from_op(op, soul);
                if !relationship_delta_is_empty(&delta) {
                    let soul_patch = patch.soul_patch.get_or_insert_with(SoulPatch::default);
                    soul_patch.relationship_deltas.push(delta);
                }
                accepted_candidate_ids.push(format!("op:{index}:relationship_event"));
            }
            EvaluatorOp::UpdateObjectState(op) => {
                validate_entity(&op.owner_entity_id, context, soul)?;
                validate_evidence(&op.evidence_quote, &evidence_text)?;
                let object_id = stable_object_id(op, soul);
                if object_id == slugify(&op.object_label) {
                    return Err("object identity cannot be raw condition label".into());
                }
                let world_patch = patch.world_patch.get_or_insert_with(WorldPatch::default);
                world_patch
                    .object_observation_operations
                    .push(ObjectObservationOperationPatch {
                        operation: "update_object_state".into(),
                        object_observation_id: Some(stable_id(
                            "object_obs",
                            &format!("{index}:{object_id}:{}", op.last_observed_state),
                        )),
                        object_state: Some(ObjectState {
                            object_id,
                            object_kind: slugify(&op.object_type),
                            owner_entity_id: Some(op.owner_entity_id.clone()),
                            location: op.location.clone(),
                            status: op.status.clone(),
                            last_observed_state: op.last_observed_state.clone(),
                            confidence: 0.8,
                            ..ObjectState::default()
                        }),
                        ..ObjectObservationOperationPatch::default()
                    });
                accepted_candidate_ids.push(format!("op:{index}:update_object_state"));
            }
            EvaluatorOp::UpdateSceneState(op) => {
                validate_entities(&op.participants, context, soul)?;
                let world_patch = patch.world_patch.get_or_insert_with(WorldPatch::default);
                world_patch.scene_state = Some(SceneStatePatch {
                    scene_state_id: Some(stable_id(
                        "scene_ops",
                        &format!("{index}:{}", op.current_scene),
                    )),
                    current_scene: non_empty(&op.current_scene),
                    focus: non_empty(&op.focus),
                    participants: op.participants.clone(),
                    last_user_action: non_empty(&op.last_user_action),
                    pressure_point: non_empty(&op.pressure_point),
                    continuity_note: non_empty(&op.continuity_note),
                    ..SceneStatePatch::default()
                });
                accepted_candidate_ids.push(format!("op:{index}:update_scene_state"));
            }
            EvaluatorOp::AddWorldEvent(op) => {
                validate_evidence(&op.evidence_quote, &evidence_text)?;
                let world_patch = patch.world_patch.get_or_insert_with(WorldPatch::default);
                world_patch.event_operations.push(WorldEventOperationPatch {
                    operation: "add_recent_event".into(),
                    recent_event_id: Some(stable_id(
                        "event_ops",
                        &format!("{index}:{}", op.content),
                    )),
                    content: Some(op.content.clone()),
                    ..WorldEventOperationPatch::default()
                });
                accepted_candidate_ids.push(format!("op:{index}:add_world_event"));
            }
            }
            Ok(())
        })();
        if let Err(reason) = outcome {
            rejected_candidates.push(EvaluatorCandidateRejection {
                candidate_id: format!("op:{index}"),
                reason,
            });
        }
    }

    // Total miss: nothing valid compiled but ops were present and failed — let
    // the caller fall through to evaluator_form_v1 instead of saving nothing.
    if accepted_candidate_ids.is_empty() && !rejected_candidates.is_empty() {
        let reasons = rejected_candidates
            .iter()
            .map(|rejection| format!("{}: {}", rejection.candidate_id, rejection.reason))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "all {} evaluator op(s) failed: {reasons}",
            rejected_candidates.len()
        ));
    }

    let no_op = patch.is_empty();
    Ok(EvaluatorConversionReport {
        patch,
        accepted_candidate_ids,
        rejected_candidates,
        evidence_validations: Vec::new(),
        entity_aliases_resolved: alias_trace.resolved,
        entity_alias_resolution_warnings: alias_trace.warnings,
        no_op,
    })
}

#[derive(Debug, Default)]
struct EntityAliasTrace {
    resolved: Vec<String>,
    warnings: Vec<String>,
}

fn resolve_op_entity_aliases(
    op: &mut EvaluatorOp,
    index: usize,
    context: &EvaluatorConversionContext<'_>,
    soul: &Soul,
    trace: &mut EntityAliasTrace,
) -> Result<(), String> {
    match op {
            EvaluatorOp::AddMemory(op) => {
                resolve_soul_alias_field(
                    &mut op.owner_soul_id,
                    "add_memory.owner_soul_id",
                    index,
                    context,
                    soul,
                    trace,
                )?;
                resolve_entity_alias_vec(
                    &mut op.target_entity_ids,
                    "add_memory.target_entity_ids",
                    index,
                    context,
                    soul,
                    trace,
                )?;
            }
            EvaluatorOp::RelationshipEvent(op) => {
                resolve_soul_alias_field(
                    &mut op.source_soul_id,
                    "relationship_event.source_soul_id",
                    index,
                    context,
                    soul,
                    trace,
                )?;
                resolve_entity_alias_field(
                    &mut op.target_entity_id,
                    "relationship_event.target_entity_id",
                    index,
                    context,
                    soul,
                    trace,
                )?;
                resolve_entity_alias_field(
                    &mut op.actor_entity_id,
                    "relationship_event.actor_entity_id",
                    index,
                    context,
                    soul,
                    trace,
                )?;
                resolve_soul_alias_field(
                    &mut op.perceived_by_entity_id,
                    "relationship_event.perceived_by_entity_id",
                    index,
                    context,
                    soul,
                    trace,
                )?;
            }
            EvaluatorOp::UpdateObjectState(op) => {
                resolve_entity_alias_field(
                    &mut op.owner_entity_id,
                    "update_object_state.owner_entity_id",
                    index,
                    context,
                    soul,
                    trace,
                )?;
            }
            EvaluatorOp::UpdateSceneState(op) => {
                resolve_entity_alias_vec(
                    &mut op.participants,
                    "update_scene_state.participants",
                    index,
                    context,
                    soul,
                    trace,
                )?;
            }
            EvaluatorOp::AddWorldEvent(_) | EvaluatorOp::NoOp(_) => {}
    }
    Ok(())
}

fn resolve_soul_alias_field(
    value: &mut String,
    field: &str,
    op_index: usize,
    context: &EvaluatorConversionContext<'_>,
    soul: &Soul,
    trace: &mut EntityAliasTrace,
) -> Result<(), String> {
    let original = value.trim();
    let replacement = match original {
        "active_soul" => Some(context.active_soul_id.to_string()),
        "session_world" => context
            .session_world
            .map(|world| world.world_id.clone())
            .or_else(|| Some(context.active_soul_id.to_string())),
        // Narrator-first: a relationship event is recorded and perceived by the
        // active Soul — never a player. Weak evaluators routinely mis-assign
        // these soul-only fields (source_soul_id / perceived_by_entity_id) to the
        // player, which used to reject the ENTIRE patch (→ noop, no memory).
        // Coerce to the active Soul instead, and log it so the model's mistake
        // stays visible in the trace rather than being silently lost.
        "active_player" | "latest_speaker" => {
            trace.warnings.push(format!(
                "op:{op_index}:{field}: coerced player alias '{original}' to active soul (narrator-first)"
            ));
            Some(context.active_soul_id.to_string())
        }
        alias if is_known_entity_alias(alias) => {
            return Err(format!(
                "entity alias '{alias}' is not valid for soul id field {field}"
            ));
        }
        other if is_player_entity_id(other) => {
            trace.warnings.push(format!(
                "op:{op_index}:{field}: coerced player id '{other}' to active soul (narrator-first)"
            ));
            Some(context.active_soul_id.to_string())
        }
        _ => None,
    };
    if let Some(replacement) = replacement {
        trace.resolved.push(format!(
            "op:{op_index}:{field}:{original}->{replacement}"
        ));
        *value = replacement;
    } else {
        reject_unknown_alias(original, field)?;
        if !context.active_soul_ids.iter().any(|id| id == original) && original != soul.character_id
        {
            trace.warnings.push(format!(
                "op:{op_index}:{field}: raw soul id '{original}' will be validated without alias correction"
            ));
        }
    }
    Ok(())
}

fn resolve_entity_alias_field(
    value: &mut String,
    field: &str,
    op_index: usize,
    context: &EvaluatorConversionContext<'_>,
    soul: &Soul,
    trace: &mut EntityAliasTrace,
) -> Result<(), String> {
    let original = value.trim();
    let replacement = match original {
        "latest_speaker" if is_ooc_operator_context(context) && !field.starts_with("update_scene_state") => {
            return Err(format!(
                "entity alias 'latest_speaker' is ambiguous in OOC/operator context for field {field}"
            ));
        }
        "active_player" | "latest_speaker" => Some(active_player_entity_id(context, soul)),
        "active_soul" => Some(context.active_soul_id.to_string()),
        "session_world" => context.session_world.map(|world| world.world_id.clone()),
        alias if is_known_entity_alias(alias) => {
            return Err(format!(
                "entity alias '{alias}' is not valid for field {field}"
            ));
        }
        _ => None,
    };
    if let Some(replacement) = replacement {
        trace.resolved.push(format!(
            "op:{op_index}:{field}:{original}->{replacement}"
        ));
        *value = replacement;
    } else {
        reject_unknown_alias(original, field)?;
    }
    Ok(())
}

fn resolve_entity_alias_vec(
    values: &mut [String],
    field: &str,
    op_index: usize,
    context: &EvaluatorConversionContext<'_>,
    soul: &Soul,
    trace: &mut EntityAliasTrace,
) -> Result<(), String> {
    for value in values {
        resolve_entity_alias_field(value, field, op_index, context, soul, trace)?;
    }
    Ok(())
}

fn active_player_entity_id(context: &EvaluatorConversionContext<'_>, soul: &Soul) -> String {
    if soul.relationships.contains_key("preset_male") {
        return "preset_male".into();
    }
    if let Some(world) = context.session_world {
        if let Some(participant) = world
            .scene_state
            .participants
            .iter()
            .find(|participant| participant.starts_with("preset_") && *participant != "default_player")
        {
            return participant.clone();
        }
        if let Some(owner) = world
            .object_states
            .iter()
            .filter_map(|object| object.owner_entity_id.as_deref())
            .find(|owner| owner.starts_with("preset_") && *owner != "default_player")
        {
            return owner.to_string();
        }
    }
    soul.relationships
        .keys()
        .find(|key| key.starts_with("preset_") && key.as_str() != "default_player")
        .cloned()
        .unwrap_or_else(|| "preset_male".into())
}

fn is_known_entity_alias(value: &str) -> bool {
    matches!(
        value,
        "active_soul" | "active_player" | "latest_speaker" | "session_world"
    )
}

/// Player entity ids follow the `preset_*` / `default_player` convention and are
/// never valid in a soul-only field. Used to coerce a mis-assigned raw player id
/// to the active Soul (narrator-first) rather than rejecting the whole patch.
fn is_player_entity_id(value: &str) -> bool {
    value == "default_player" || value.starts_with("preset_")
}

fn reject_unknown_alias(value: &str, field: &str) -> Result<(), String> {
    if value.ends_with("_soul")
        || value.ends_with("_player")
        || value.ends_with("_speaker")
        || value == "active_character"
        || value == "current_soul"
    {
        return Err(format!("unknown entity alias '{value}' in field {field}"));
    }
    Ok(())
}

fn is_ooc_operator_context(context: &EvaluatorConversionContext<'_>) -> bool {
    let lower = context.latest_user_message.trim_start().to_ascii_lowercase();
    lower.starts_with("ooc:")
        || lower.starts_with("[ooc")
        || lower.starts_with("(ooc")
        || lower.starts_with("operator:")
        || lower.starts_with("/ooc")
}

fn relationship_delta_from_op(op: &RelationshipEventOp, soul: &Soul) -> RelationshipDelta {
    let current = soul
        .relationships
        .get(&op.target_entity_id)
        .cloned()
        .unwrap_or_default();
    let strength = (op.modifiers.salience.min(100) as f32 / 100.0)
        * (op.modifiers.certainty.min(100) as f32 / 100.0)
        * (op.modifiers.directness.min(100) as f32 / 100.0)
        * (0.5 + op.modifiers.stakes.min(100) as f32 / 100.0);
    let cap = (2.0 + 12.0 * strength).clamp(1.0, 14.0);
    let axes = &op.axes;
    let trust_target = (current.trust
        + 2.0 * axes.honesty as f32
        + 1.5 * axes.reliability as f32
        + axes.intent as f32)
        .clamp(0.0, 100.0);
    let comfort_target = (current.comfort
        + 1.5 * axes.responsiveness as f32
        + axes.predictability as f32
        + axes.boundary_treatment as f32)
        .clamp(0.0, 100.0);
    let conflict_target = (current.conflict
        + (-axes.evaluation_tone.min(0)) as f32 * 3.0
        + (-axes.boundary_treatment.min(0)) as f32 * 2.0
        - axes.repair.max(0) as f32)
        .clamp(0.0, 100.0);
    let boundary_target = (current.boundary_pressure
        + (-axes.boundary_treatment.min(0)) as f32 * 4.0
        + (-axes.power_use.min(0)) as f32 * 3.0
        - axes.boundary_treatment.max(0) as f32)
        .clamp(0.0, 100.0);
    RelationshipDelta {
        relationship_event_id: Some(stable_id(
            "relationship_event_ops",
            &format!(
                "{}:{}:{}",
                op.source_soul_id, op.target_entity_id, op.evidence_quote
            ),
        )),
        from: Some(op.source_soul_id.clone()),
        target: Some(op.target_entity_id.clone()),
        trust: bounded_delta(current.trust, trust_target, cap),
        comfort: bounded_delta(current.comfort, comfort_target, cap),
        conflict: bounded_delta(current.conflict, conflict_target, cap),
        boundary_pressure: bounded_delta(current.boundary_pressure, boundary_target, cap),
        intimacy: bounded_delta(
            current.intimacy,
            (current.intimacy + axes.disclosure.max(0) as f32 + axes.reciprocity.max(0) as f32)
                .clamp(0.0, 100.0),
            cap,
        ),
        respect: bounded_delta(
            current.respect,
            (current.respect + axes.competence as f32 + axes.evaluation_tone as f32)
                .clamp(0.0, 100.0),
            cap,
        ),
        max_abs_delta: Some(cap),
        ..RelationshipDelta::default()
    }
}

fn relationship_delta_is_empty(delta: &RelationshipDelta) -> bool {
    delta.trust.is_none()
        && delta.comfort.is_none()
        && delta.conflict.is_none()
        && delta.boundary_pressure.is_none()
        && delta.intimacy.is_none()
        && delta.respect.is_none()
}

fn bounded_delta(current: f32, target: f32, cap: f32) -> Option<f32> {
    let delta = (target - current).clamp(-cap, cap);
    (delta.abs() >= 0.001).then_some(delta)
}

fn validate_soul_id(id: &str, context: &EvaluatorConversionContext<'_>) -> Result<(), String> {
    if context.active_soul_ids.iter().any(|active| active == id) {
        Ok(())
    } else {
        Err(format!("unknown or inactive source soul id: {id}"))
    }
}

fn validate_entity(
    id: &str,
    context: &EvaluatorConversionContext<'_>,
    soul: &Soul,
) -> Result<(), String> {
    if id == soul.character_id
        || id == context.active_soul_id
        || id == "unknown"
        || (id != "default_player" && id.starts_with("preset_"))
        || soul.relationships.contains_key(id)
    {
        Ok(())
    } else {
        Err(format!("unknown entity id: {id}"))
    }
}

fn validate_entities(
    ids: &[String],
    context: &EvaluatorConversionContext<'_>,
    soul: &Soul,
) -> Result<(), String> {
    for id in ids {
        validate_entity(id, context, soul)?;
    }
    Ok(())
}

fn validate_evidence(quote: &str, evidence_text: &str) -> Result<(), String> {
    let quote = quote.trim();
    if quote.is_empty() {
        return Err("evidence_quote is required".into());
    }
    if !evidence_text.contains(&normalize_for_match(quote)) {
        return Err(format!(
            "evidence quote not found in latest exchange: {quote}"
        ));
    }
    Ok(())
}

fn normalized_evidence_text(context: &EvaluatorConversionContext<'_>) -> String {
    normalize_for_match(&format!(
        "{}\n{}",
        context.latest_user_message, context.latest_narrator_response
    ))
}

fn normalize_for_match(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for character in value.chars() {
        let mapped = match character {
            '\u{2018}' | '\u{2019}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201F}' => '"',
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => ' ',
            '*' | '_' | '`' | '~' => continue,
            character if character.is_ascii_punctuation() => ' ',
            character if character.is_whitespace() => ' ',
            character => character.to_ascii_lowercase(),
        };
        normalized.push(mapped);
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn stable_object_id(op: &UpdateObjectStateOp, soul: &Soul) -> String {
    let object_type = slugify(&op.object_type);
    let owner = slugify(&op.owner_entity_id);
    let label = slugify(&op.object_label);
    if let Some(existing) = soul.world.object_states.iter().find(|object| {
        object.object_kind == object_type
            && object.owner_entity_id.as_deref() == Some(op.owner_entity_id.as_str())
    }) {
        return existing.object_id.clone();
    }
    if label.contains(&object_type) || object_type != "object" {
        format!("{owner}_{object_type}_1")
    } else {
        format!("{owner}_{label}_1")
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn slugify(label: &str) -> String {
    label
        .trim()
        .to_ascii_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| {
            !part.is_empty()
                && !matches!(
                    *part,
                    "wet"
                        | "dry"
                        | "soaked"
                        | "damp"
                        | "broken"
                        | "open"
                        | "closed"
                        | "locked"
                        | "unlocked"
                )
        })
        .collect::<Vec<_>>()
        .join("_")
}

fn stable_id(prefix: &str, source: &str) -> String {
    let mut hash: u64 = 1469598103934665603;
    for byte in source.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{prefix}_{hash:016x}")
}

pub fn evaluator_ops_json_schema() -> serde_json::Value {
    let nullable_string = json!({ "type": ["string", "null"] });
    let nullable_i64 = json!({ "type": ["integer", "null"] });
    let string_array = json!({ "type": "array", "items": { "type": "string" } });
    let evidence_string = json!({ "type": "string", "minLength": 1 });
    let bounded_axis = json!({ "type": "integer", "minimum": -5, "maximum": 5 });
    let pct = json!({ "type": "integer", "minimum": 0, "maximum": 100 });
    let truth_status = json!({
        "type": "string",
        "enum": ["fiction", "scene_event", "character_belief", "narrator_claim", "user_claimed", "verified_engine", "actual_system_event", "unknown"]
    });
    let memory_slot = json!({
        "type": "string",
        "enum": ["relationship_memory", "current_plot_memory", "character_identity_memory", "unresolved_tension", "world_location_memory", "recent_emotional_state"]
    });
    let axes = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["intent", "honesty", "reliability", "boundary_treatment", "responsiveness", "power_use", "evaluation_tone", "competence", "disclosure", "reciprocity", "repair", "predictability"],
        "properties": {
            "intent": bounded_axis, "honesty": bounded_axis, "reliability": bounded_axis,
            "boundary_treatment": bounded_axis, "responsiveness": bounded_axis,
            "power_use": bounded_axis, "evaluation_tone": bounded_axis, "competence": bounded_axis,
            "disclosure": bounded_axis, "reciprocity": bounded_axis, "repair": bounded_axis,
            "predictability": bounded_axis
        }
    });
    let modifiers = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["salience", "certainty", "directness", "costliness", "stakes", "repetition"],
        "properties": {
            "salience": pct, "certainty": pct, "directness": pct,
            "costliness": pct, "stakes": pct, "repetition": pct
        }
    });
    let add_memory = op_schema(
        "add_memory",
        json!({
            "owner_soul_id": { "type": "string" }, "slot": memory_slot,
            "content": { "type": "string", "minLength": 1 },
            "evidence_quote": evidence_string, "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
            "salience": pct, "source_message_id": nullable_i64,
            "target_entity_ids": string_array, "truth_status": truth_status
        }),
        &[
            "owner_soul_id",
            "slot",
            "content",
            "evidence_quote",
            "confidence",
            "salience",
            "source_message_id",
            "target_entity_ids",
            "truth_status",
        ],
    );
    let relationship_event = op_schema(
        "relationship_event",
        json!({
            "source_soul_id": { "type": "string" }, "target_entity_id": { "type": "string" },
            "actor_entity_id": { "type": "string" }, "perceived_by_entity_id": { "type": "string" },
            "evidence_quote": evidence_string, "axes": axes, "modifiers": modifiers,
            "event_flags_u64": { "type": "integer", "minimum": 0 }
        }),
        &[
            "source_soul_id",
            "target_entity_id",
            "actor_entity_id",
            "perceived_by_entity_id",
            "evidence_quote",
            "axes",
            "modifiers",
            "event_flags_u64",
        ],
    );
    let object_update = op_schema(
        "update_object_state",
        json!({
            "object_label": { "type": "string", "minLength": 1 },
            "object_type": { "type": "string", "minLength": 1 },
            "owner_entity_id": { "type": "string" },
            "status": { "type": "string", "minLength": 1 },
            "location": { "type": "string" },
            "last_observed_state": { "type": "string", "minLength": 1 },
            "evidence_quote": evidence_string
        }),
        &[
            "object_label",
            "object_type",
            "owner_entity_id",
            "status",
            "location",
            "last_observed_state",
            "evidence_quote",
        ],
    );
    let scene_update = op_schema(
        "update_scene_state",
        json!({
            "current_scene": { "type": "string" }, "focus": { "type": "string" },
            "participants": string_array, "last_user_action": { "type": "string" },
            "pressure_point": { "type": "string" }, "continuity_note": { "type": "string" }
        }),
        &[
            "current_scene",
            "focus",
            "participants",
            "last_user_action",
            "pressure_point",
            "continuity_note",
        ],
    );
    let world_event = op_schema(
        "add_world_event",
        json!({ "content": { "type": "string", "minLength": 1 }, "evidence_quote": evidence_string }),
        &["content", "evidence_quote"],
    );
    let no_op = op_schema(
        "no_op",
        json!({ "reason": { "type": "string", "minLength": 1 } }),
        &["reason"],
    );

    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "ops", "no_op_reason"],
        "properties": {
            "schema_version": { "type": "integer", "enum": [1] },
            "ops": {
                "type": "array",
                "items": { "anyOf": [add_memory, relationship_event, object_update, scene_update, world_event, no_op] }
            },
            "no_op_reason": nullable_string
        }
    })
}

/// Schema name for the repair variant, distinct so providers that cache
/// response_format definitions by name never mix the two.
pub const EVALUATOR_OPS_REPAIR_SCHEMA_NAME: &str = "evaluator_structured_ops_repair_v1";

/// The REPAIR variant of the ops schema. Repair only fires on turns the system
/// already judged to contain durable change, so the `no_op` escape hatch is
/// removed and at least one real op is required — small local models otherwise
/// reason correctly about the scene and then punt into `no_op` anyway; the
/// grammar makes that impossible. Fabrication risk is backstopped by Rust-side
/// validation: ops with invented evidence quotes or bad entity ids are rejected
/// and commit nothing, exactly as before.
pub fn evaluator_ops_repair_json_schema() -> serde_json::Value {
    let mut schema = evaluator_ops_json_schema();
    let ops = &mut schema["properties"]["ops"];
    ops["minItems"] = json!(1);
    if let Some(variants) = ops["items"]["anyOf"].as_array_mut() {
        variants.retain(|variant| {
            variant["properties"]["op"]["enum"][0].as_str() != Some("no_op")
        });
    }
    // No prose escape valve either: the reason slot must stay empty.
    schema["properties"]["no_op_reason"] = json!({ "type": "null" });
    schema
}

fn op_schema(
    op_name: &str,
    properties: serde_json::Value,
    extra_required: &[&str],
) -> serde_json::Value {
    let mut props = properties.as_object().cloned().unwrap_or_default();
    props.insert("op".into(), json!({ "type": "string", "enum": [op_name] }));
    let mut required = vec!["op".to_string()];
    required.extend(extra_required.iter().map(|key| key.to_string()));
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": props
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(user: &'a str, narrator: &'a str) -> EvaluatorConversionContext<'a> {
        EvaluatorConversionContext {
            active_soul_id: "aurora",
            active_soul_ids: vec!["aurora".into()],
            latest_user_message: user,
            latest_narrator_response: narrator,
            session_world: None,
            baseline_recent_event_id: None,
        }
    }

    #[test]
    fn unknown_top_level_key_rejected_by_serde() {
        let err = serde_json::from_str::<EvaluatorStructuredOutputV1>(
            r#"{"schema_version":1,"ops":[],"no_op_reason":null,"relationship_rows":[]}"#,
        )
        .expect_err("unknown key rejected");
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn unknown_relationship_axis_rejected_by_serde() {
        let raw = r#"{"schema_version":1,"ops":[{"op":"relationship_event","source_soul_id":"aurora","target_entity_id":"preset_male","actor_entity_id":"preset_male","perceived_by_entity_id":"aurora","evidence_quote":"hello","axes":{"intent":0,"honesty":0,"reliability":0,"boundary_treatment":0,"responsiveness":0,"power_use":0,"evaluation_tone":0,"competence":0,"disclosure":0,"reciprocity":0,"repair":0,"predictability":0,"trust":5},"modifiers":{"salience":50,"certainty":50,"directness":50,"costliness":0,"stakes":0,"repetition":0},"event_flags_u64":0}],"no_op_reason":null}"#;
        assert!(serde_json::from_str::<EvaluatorStructuredOutputV1>(raw).is_err());
    }

    #[test]
    fn update_scene_state_confidence_rejected_by_serde() {
        let raw = r#"{"schema_version":1,"ops":[{"op":"update_scene_state","current_scene":"Apartment doorway","focus":"Aurora and preset_male","participants":["aurora","preset_male"],"last_user_action":"I wait.","pressure_point":"Aurora decides whether to open the door.","continuity_note":"Rain continues.","confidence":0.8}],"no_op_reason":null}"#;
        let err = serde_json::from_str::<EvaluatorStructuredOutputV1>(raw)
            .expect_err("confidence is not valid on update_scene_state");
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn no_op_output_accepted() {
        let output: EvaluatorStructuredOutputV1 = serde_json::from_str(
            r#"{"schema_version":1,"ops":[],"no_op_reason":"nothing durable"}"#,
        )
        .expect("no op parses");
        assert!(output.ops.is_empty());
    }

    #[test]
    fn ops_schema_excludes_old_form_and_surface_fields() {
        let schema_text = serde_json::to_string(&evaluator_ops_json_schema()).unwrap();
        for forbidden in [
            "relationship_rows",
            "trust",
            "comfort",
            "intimacy",
            "respect",
            "fear",
            "conflict",
            "boundary_pressure",
        ] {
            assert!(
                !schema_text.contains(&format!("\"{forbidden}\"")),
                "{forbidden} must not be part of evaluator ops schema"
            );
        }
        assert!(schema_text.contains("\"additionalProperties\":false"));
        assert!(schema_text.contains("relationship_event"));
    }

    #[test]
    fn repair_schema_forces_at_least_one_real_op() {
        let schema = evaluator_ops_repair_json_schema();
        // At least one op is mandatory.
        assert_eq!(schema["properties"]["ops"]["minItems"], json!(1));
        // The no_op escape hatch is gone; every real op variant remains.
        let text = serde_json::to_string(&schema).unwrap();
        assert!(!text.contains("\"no_op\""));
        for kept in [
            "add_memory",
            "relationship_event",
            "update_object_state",
            "update_scene_state",
            "add_world_event",
        ] {
            assert!(text.contains(kept), "{kept} missing from repair schema");
        }
        // The prose escape valve is closed too.
        assert_eq!(
            schema["properties"]["no_op_reason"],
            json!({ "type": "null" })
        );
        // Distinct schema name so provider-side caching never mixes variants.
        assert_ne!(EVALUATOR_OPS_REPAIR_SCHEMA_NAME, EVALUATOR_OPS_SCHEMA_NAME);
    }

    #[test]
    fn object_update_compiles_stable_object_identity() {
        let soul = Soul::default_for_character("Aurora");
        let output = EvaluatorStructuredOutputV1 {
            schema_version: 1,
            ops: vec![EvaluatorOp::UpdateObjectState(UpdateObjectStateOp {
                object_label: "wet jacket".into(),
                object_type: "jacket".into(),
                owner_entity_id: "preset_male".into(),
                status: "wet".into(),
                location: "near door".into(),
                last_observed_state: "wet jacket near door".into(),
                evidence_quote: "wet jacket near door".into(),
            })],
            no_op_reason: None,
        };
        let report = compile_evaluator_ops_to_engine_patch(
            &output,
            &context("", "He leaves a wet jacket near door."),
            &soul,
        )
        .expect("compiles");
        let world_patch = report.patch.world_patch.unwrap();
        let object_id = &world_patch.object_observation_operations[0]
            .object_state
            .as_ref()
            .unwrap()
            .object_id;
        assert_eq!(object_id, "preset_male_jacket_1");
        assert_ne!(object_id, "wet_jacket");
    }

    #[test]
    fn evidence_quote_accepts_markdown_and_smart_punctuation_normalization() {
        let soul = Soul::default_for_character("Aurora");
        let output = EvaluatorStructuredOutputV1 {
            schema_version: 1,
            ops: vec![EvaluatorOp::AddWorldEvent(AddWorldEventOp {
                content: "Aurora repeats that the changes were intentional.".into(),
                evidence_quote: "I changed those. I changed those specifically because—".into(),
            })],
            no_op_reason: None,
        };
        compile_evaluator_ops_to_engine_patch(
            &output,
            &context("", "**I changed those.** I changed those specifically because"),
            &soul,
        )
        .expect("normalized literal substring should validate");
    }

    #[test]
    fn invalid_entity_fails_semantic_validation() {
        let soul = Soul::default_for_character("Aurora");
        let output = EvaluatorStructuredOutputV1 {
            schema_version: 1,
            ops: vec![EvaluatorOp::AddMemory(AddMemoryOp {
                owner_soul_id: "missing".into(),
                slot: MemorySlotOp::RelationshipMemory,
                content: "Aurora noticed him.".into(),
                evidence_quote: "noticed him".into(),
                confidence: 0.8,
                salience: 60,
                source_message_id: None,
                target_entity_ids: vec!["preset_male".into()],
                truth_status: TruthStatusOp::SceneEvent,
            })],
            no_op_reason: None,
        };
        assert!(compile_evaluator_ops_to_engine_patch(
            &output,
            &context("", "Aurora noticed him."),
            &soul
        )
        .is_err());
    }

    fn neutral_axes() -> RelationshipAxes {
        RelationshipAxes {
            intent: 1,
            honesty: 1,
            reliability: 1,
            boundary_treatment: 1,
            responsiveness: 1,
            power_use: 0,
            evaluation_tone: 1,
            competence: 0,
            disclosure: 0,
            reciprocity: 0,
            repair: 0,
            predictability: 1,
        }
    }

    fn neutral_modifiers() -> RelationshipModifiers {
        RelationshipModifiers {
            salience: 60,
            certainty: 80,
            directness: 80,
            costliness: 0,
            stakes: 20,
            repetition: 0,
        }
    }

    fn relationship_output(
        source_soul_id: &str,
        target_entity_id: &str,
        actor_entity_id: &str,
        perceived_by_entity_id: &str,
    ) -> EvaluatorStructuredOutputV1 {
        EvaluatorStructuredOutputV1 {
            schema_version: 1,
            ops: vec![EvaluatorOp::RelationshipEvent(RelationshipEventOp {
                source_soul_id: source_soul_id.into(),
                target_entity_id: target_entity_id.into(),
                actor_entity_id: actor_entity_id.into(),
                perceived_by_entity_id: perceived_by_entity_id.into(),
                evidence_quote: "I wait at the doorway.".into(),
                axes: neutral_axes(),
                modifiers: neutral_modifiers(),
                event_flags_u64: 0,
            })],
            no_op_reason: None,
        }
    }

    #[test]
    fn relationship_event_aliases_resolve_before_validation() {
        let soul = Soul::default_for_character("Aurora");
        let report = compile_evaluator_ops_to_engine_patch(
            &relationship_output("active_soul", "active_player", "latest_speaker", "active_soul"),
            &context("I wait at the doorway.", ""),
            &soul,
        )
        .expect("aliases compile");
        let delta = &report
            .patch
            .soul_patch
            .as_ref()
            .unwrap()
            .relationship_deltas[0];
        assert_eq!(delta.from.as_deref(), Some("aurora"));
        assert_eq!(delta.target.as_deref(), Some("preset_male"));
        assert!(
            report
                .entity_aliases_resolved
                .iter()
                .any(|entry| entry.contains("relationship_event.actor_entity_id:latest_speaker->preset_male"))
        );
    }

    #[test]
    fn relationship_event_player_in_soul_field_coerces_instead_of_rejecting() {
        // Regression: laguna/owl-class evaluators put the player in
        // perceived_by_entity_id (a soul-only field). That used to reject the
        // whole patch → noop → memory never grew. It must now coerce the
        // soul-only fields to the active Soul and still apply the relationship
        // delta. (perceived_by AND source_soul_id both set to the player here.)
        let soul = Soul::default_for_character("Aurora");
        let report = compile_evaluator_ops_to_engine_patch(
            &relationship_output("active_player", "active_player", "active_player", "active_player"),
            &context("I wait at the doorway.", ""),
            &soul,
        )
        .expect("player in soul-only fields coerces to active soul, not reject");
        let delta = &report
            .patch
            .soul_patch
            .as_ref()
            .expect("soul patch present")
            .relationship_deltas[0];
        assert_eq!(delta.from.as_deref(), Some("aurora"));
        assert_eq!(delta.target.as_deref(), Some("preset_male"));
    }

    #[test]
    fn relationship_event_raw_player_id_in_soul_field_coerces() {
        // Same fix for a raw player id (preset_male) rather than the alias.
        let soul = Soul::default_for_character("Aurora");
        let report = compile_evaluator_ops_to_engine_patch(
            &relationship_output("active_soul", "active_player", "active_player", "preset_male"),
            &context("I wait at the doorway.", ""),
            &soul,
        )
        .expect("raw player id in perceived_by coerces to active soul");
        assert_eq!(
            report.patch.soul_patch.as_ref().unwrap().relationship_deltas[0]
                .from
                .as_deref(),
            Some("aurora")
        );
    }

    #[test]
    fn add_memory_persists_evidence_quote_as_source_quote() {
        // The "quote" half of address/quote: the validated evidence quote is now
        // carried onto the memory instead of being discarded after validation.
        let soul = Soul::default_for_character("Aurora");
        let output = EvaluatorStructuredOutputV1 {
            schema_version: 1,
            ops: vec![EvaluatorOp::AddMemory(AddMemoryOp {
                owner_soul_id: "active_soul".into(),
                slot: MemorySlotOp::RelationshipMemory,
                content: "Aurora noted the user waited at the doorway.".into(),
                evidence_quote: "I wait at the doorway.".into(),
                confidence: 0.8,
                salience: 60,
                source_message_id: None,
                target_entity_ids: vec!["active_player".into()],
                truth_status: TruthStatusOp::SceneEvent,
            })],
            no_op_reason: None,
        };
        let report = compile_evaluator_ops_to_engine_patch(
            &output,
            &context("I wait at the doorway.", ""),
            &soul,
        )
        .expect("compiles");
        let memory = &report.patch.soul_patch.as_ref().unwrap().new_memories[0];
        assert_eq!(
            memory.source_quote.as_deref(),
            Some("I wait at the doorway."),
            "the memory carries its exact source line"
        );
    }

    #[test]
    fn partial_accept_keeps_valid_ops_and_drops_the_bad_one() {
        // The whole point: one fumbled op no longer discards the turn's whole
        // extraction. The valid memory is kept; the unsupported one is dropped
        // and logged in rejected_candidates (visible in the trace).
        let soul = Soul::default_for_character("Aurora");
        let output = EvaluatorStructuredOutputV1 {
            schema_version: 1,
            ops: vec![
                EvaluatorOp::AddMemory(AddMemoryOp {
                    owner_soul_id: "active_soul".into(),
                    slot: MemorySlotOp::RelationshipMemory,
                    content: "Aurora noticed the user wait patiently at the doorway.".into(),
                    evidence_quote: "I wait at the doorway.".into(),
                    confidence: 0.8,
                    salience: 60,
                    source_message_id: None,
                    target_entity_ids: vec!["active_player".into()],
                    truth_status: TruthStatusOp::SceneEvent,
                }),
                EvaluatorOp::AddMemory(AddMemoryOp {
                    owner_soul_id: "active_soul".into(),
                    slot: MemorySlotOp::RelationshipMemory,
                    content: "A fabricated event with no support in the turn.".into(),
                    // Not present in the evidence text -> validation rejects it.
                    evidence_quote: "Dragons circled the tower at dusk.".into(),
                    confidence: 0.8,
                    salience: 60,
                    source_message_id: None,
                    target_entity_ids: vec!["active_player".into()],
                    truth_status: TruthStatusOp::SceneEvent,
                }),
            ],
            no_op_reason: None,
        };
        let report = compile_evaluator_ops_to_engine_patch(
            &output,
            &context("I wait at the doorway.", ""),
            &soul,
        )
        .expect("the valid op compiles even though a sibling op fails");
        assert_eq!(
            report.patch.soul_patch.as_ref().unwrap().new_memories.len(),
            1,
            "the valid memory is kept"
        );
        assert_eq!(
            report.rejected_candidates.len(),
            1,
            "the unsupported op is dropped and logged, not silently lost"
        );
        assert!(report.rejected_candidates[0].candidate_id.contains("op:1"));
    }

    #[test]
    fn add_memory_aliases_resolve_before_validation() {
        let soul = Soul::default_for_character("Aurora");
        let output = EvaluatorStructuredOutputV1 {
            schema_version: 1,
            ops: vec![EvaluatorOp::AddMemory(AddMemoryOp {
                owner_soul_id: "active_soul".into(),
                slot: MemorySlotOp::RelationshipMemory,
                content: "Aurora noticed the player's patience.".into(),
                evidence_quote: "I wait at the doorway.".into(),
                confidence: 0.8,
                salience: 60,
                source_message_id: None,
                target_entity_ids: vec!["active_player".into()],
                truth_status: TruthStatusOp::SceneEvent,
            })],
            no_op_reason: None,
        };
        let report = compile_evaluator_ops_to_engine_patch(
            &output,
            &context("I wait at the doorway.", ""),
            &soul,
        )
        .expect("memory aliases compile");
        let memory = &report.patch.soul_patch.as_ref().unwrap().new_memories[0];
        assert_eq!(memory.owner_soul_id.as_deref(), Some("aurora"));
        assert_eq!(memory.target_entity_ids, vec!["preset_male"]);
    }

    #[test]
    fn update_object_state_active_player_alias_creates_stable_player_object_id() {
        let soul = Soul::default_for_character("Aurora");
        let output = EvaluatorStructuredOutputV1 {
            schema_version: 1,
            ops: vec![EvaluatorOp::UpdateObjectState(UpdateObjectStateOp {
                object_label: "wet jacket".into(),
                object_type: "jacket".into(),
                owner_entity_id: "active_player".into(),
                status: "wet".into(),
                location: "near door".into(),
                last_observed_state: "wet jacket near door".into(),
                evidence_quote: "wet jacket near door".into(),
            })],
            no_op_reason: None,
        };
        let report = compile_evaluator_ops_to_engine_patch(
            &output,
            &context("", "He leaves a wet jacket near door."),
            &soul,
        )
        .expect("object owner alias compiles");
        let object = report.patch.world_patch.unwrap().object_observation_operations[0]
            .object_state
            .clone()
            .unwrap();
        assert_eq!(object.owner_entity_id.as_deref(), Some("preset_male"));
        assert_eq!(object.object_id, "preset_male_jacket_1");
    }

    #[test]
    fn latest_speaker_does_not_resolve_to_active_player_for_ooc_relationship_ops() {
        let soul = Soul::default_for_character("Aurora");
        let err = compile_evaluator_ops_to_engine_patch(
            &relationship_output("active_soul", "active_player", "latest_speaker", "active_soul"),
            &context("OOC: please summarize the state.", ""),
            &soul,
        )
        .expect_err("OOC latest_speaker must not silently become player");
        assert!(err.contains("latest_speaker"));
        assert!(err.contains("OOC"));
    }

    #[test]
    fn unknown_alias_fails_clearly() {
        let soul = Soul::default_for_character("Aurora");
        let err = compile_evaluator_ops_to_engine_patch(
            &relationship_output("current_soul", "active_player", "active_player", "active_soul"),
            &context("I wait at the doorway.", ""),
            &soul,
        )
        .expect_err("unknown alias rejected");
        assert!(err.contains("unknown entity alias"));
        assert!(err.contains("current_soul"));
    }
}
