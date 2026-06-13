use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    evaluator::{EvaluatorConversionContext, EvaluatorConversionReport},
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

    let evidence_text = normalized_evidence_text(context);
    let mut patch = EnginePatch {
        schema_version: Some(PATCH_PROTOCOL_VERSION),
        ..EnginePatch::default()
    };
    let mut accepted_candidate_ids = Vec::new();

    for (index, op) in output.ops.iter().enumerate() {
        match op {
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
    }

    let no_op = patch.is_empty();
    Ok(EvaluatorConversionReport {
        patch,
        accepted_candidate_ids,
        rejected_candidates: Vec::new(),
        evidence_validations: Vec::new(),
        no_op,
    })
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
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
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
}
