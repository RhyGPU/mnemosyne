use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    diagnostics::CompilerContractError,
    source::{stable_digest, SourceEnvelope},
    MEMORY_COMPILER_CONTRACT_VERSION,
};

pub const PERCEPTION_IR_SCHEMA_VERSION: u32 = 2;
pub const PERCEPTION_IR_SCHEMA_NAME: &str = "mnemosyne_perception_ir_v2";

/// Transport-neutral identity for the model invocation that produced a draft.
/// This is attached by application code, never read from the draft JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelProvenance {
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub schema_name: String,
}

/// The complete LLM-writable surface. It intentionally has no conversation,
/// branch, turn, message, source hash, compiler version, truth status, or effect
/// fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerceptionBatchDraft {
    pub schema_version: u32,
    pub candidates: Vec<PerceptionCandidateDraft>,
    pub no_op_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerceptionCandidateDraft {
    pub kind: PerceptionKind,
    pub subject_ref: String,
    pub predicate: String,
    pub object: Option<ClaimValue>,
    pub actor_ref: Option<String>,
    pub perceiver_ref: Option<String>,
    pub target_refs: Vec<String>,
    pub evidence: EvidenceSpan,
    pub epistemic_mode: EpistemicMode,
    pub extraction_confidence: f32,
    pub temporal: TemporalExpression,
    pub durability_hint: DurabilityHint,
    #[serde(default)]
    pub relationship_signal: Option<RelationshipSignalDraft>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerceptionKind {
    Event,
    Utterance,
    ObjectObservation,
    AffectCue,
    RelationshipEvidence,
    Intention,
    BeliefExpression,
    Correction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClaimValue {
    EntityRef { entity_ref: String },
    Text { text: String },
    Number { value: f64, unit: Option<String> },
    Boolean { value: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSpan {
    pub source: EvidenceSource,
    pub quote: String,
    pub start_char: Option<u32>,
    pub end_char: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    UserMessage,
    AssistantMessage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicMode {
    DirectlyObserved,
    StatedBy,
    NarratorDescribed,
    Inferred,
    RememberedBy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalExpression {
    pub anchor: TemporalAnchor,
    pub expression: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TemporalAnchor {
    CurrentTurn,
    BeforeCurrentTurn,
    AfterCurrentTurn,
    Absolute,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DurabilityHint {
    Transient,
    Turn,
    Session,
    LongTerm,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RelationshipSignalDraft {
    pub behaviors: Vec<BehaviorEvidenceKind>,
    pub valence: i8,
    pub directness: u8,
    pub stakes: u8,
    pub costliness: u8,
    pub repetition: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorEvidenceKind {
    PromiseKept,
    PromiseBroken,
    HonestDisclosure,
    Deception,
    BoundaryRespected,
    BoundaryViolated,
    SupportOffered,
    HarmThreatened,
    RepairAttempted,
    RepairAccepted,
    Abandonment,
    Reciprocity,
    CompetenceDisplayed,
}

/// Code-sealed candidate with deterministic identity and source authority.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerceptionCandidate {
    pub candidate_id: String,
    pub source_hash: String,
    pub perception: PerceptionCandidateDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerceptionBatch {
    pub schema_version: u32,
    pub compiler_version: u32,
    pub source_hash: String,
    pub producer: ModelProvenance,
    pub candidates: Vec<PerceptionCandidate>,
    pub no_op_reason: Option<String>,
}

pub fn seal_perception_batch(
    source: &SourceEnvelope,
    draft: PerceptionBatchDraft,
    producer: ModelProvenance,
) -> Result<PerceptionBatch, CompilerContractError> {
    source.validate()?;
    if draft.schema_version != PERCEPTION_IR_SCHEMA_VERSION {
        return Err(CompilerContractError::new(
            "unsupported_perception_schema",
            format!(
                "unsupported perception schema {}, expected {}",
                draft.schema_version, PERCEPTION_IR_SCHEMA_VERSION
            ),
        ));
    }
    if producer.provider.trim().is_empty()
        || producer.model.trim().is_empty()
        || producer.prompt_version.trim().is_empty()
        || producer.schema_name.trim().is_empty()
    {
        return Err(CompilerContractError::new(
            "missing_model_provenance",
            "provider, model, prompt version, and schema name are required",
        ));
    }

    let candidates = draft
        .candidates
        .into_iter()
        .enumerate()
        .map(|(index, perception)| {
            if perception.subject_ref.trim().is_empty()
                || perception.predicate.trim().is_empty()
                || perception.evidence.quote.trim().is_empty()
            {
                return Err(CompilerContractError::new(
                    "incomplete_perception_candidate",
                    format!(
                        "candidate {index} requires non-empty subject, predicate, and evidence"
                    ),
                ));
            }
            if !perception.extraction_confidence.is_finite()
                || !(0.0..=1.0).contains(&perception.extraction_confidence)
            {
                return Err(CompilerContractError::new(
                    "invalid_extraction_confidence",
                    format!("candidate {index} confidence must be finite and between 0 and 1"),
                ));
            }
            match (
                perception.evidence.start_char,
                perception.evidence.end_char,
            ) {
                (None, None) => {}
                (Some(start), Some(end)) if start < end => {}
                _ => {
                    return Err(CompilerContractError::new(
                        "invalid_evidence_span",
                        format!(
                            "candidate {index} evidence offsets must be absent or an increasing pair"
                        ),
                    ));
                }
            }
            match (&perception.kind, &perception.relationship_signal) {
                (PerceptionKind::RelationshipEvidence, Some(signal))
                    if !signal.behaviors.is_empty()
                        && (-5..=5).contains(&signal.valence)
                        && signal.directness <= 100
                        && signal.stakes <= 100
                        && signal.costliness <= 100
                        && signal.repetition <= 100 => {}
                (PerceptionKind::RelationshipEvidence, _) => {
                    return Err(CompilerContractError::new(
                        "invalid_relationship_signal",
                        format!(
                            "candidate {index} relationship evidence requires bounded behavior signal"
                        ),
                    ));
                }
                (_, None) => {}
                (_, Some(_)) => {
                    return Err(CompilerContractError::new(
                        "unexpected_relationship_signal",
                        format!(
                            "candidate {index} relationship signal is only valid for relationship evidence"
                        ),
                    ));
                }
            }
            let serialized = serde_json::to_string(&perception).map_err(|error| {
                CompilerContractError::new(
                    "perception_serialization_failed",
                    format!("candidate {index} could not be serialized: {error}"),
                )
            })?;
            let index = index.to_string();
            let candidate_id = stable_digest(
                "perception_candidate",
                [source.source_hash(), index.as_str(), serialized.as_str()],
            );
            Ok(PerceptionCandidate {
                candidate_id,
                source_hash: source.source_hash().to_string(),
                perception,
            })
        })
        .collect::<Result<Vec<_>, CompilerContractError>>()?;

    Ok(PerceptionBatch {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
        source_hash: source.source_hash().to_string(),
        producer,
        candidates,
        no_op_reason: draft.no_op_reason,
    })
}

/// Provider-enforced schema for the complete LLM-writable V2 surface.
///
/// Optional semantic values are represented as required nullable fields so the
/// schema is accepted by strict structured-output providers while still
/// remaining a single canonical shape.
pub fn perception_ir_json_schema() -> serde_json::Value {
    let nullable_string = json!({ "type": ["string", "null"] });
    let nullable_u32 = json!({ "type": ["integer", "null"], "minimum": 0 });
    let string_array = json!({ "type": "array", "items": { "type": "string" } });
    let claim_value = json!({
        "anyOf": [
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "entity_ref"],
                "properties": {
                    "type": { "type": "string", "enum": ["entity_ref"] },
                    "entity_ref": { "type": "string", "minLength": 1 }
                }
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "text"],
                "properties": {
                    "type": { "type": "string", "enum": ["text"] },
                    "text": { "type": "string", "minLength": 1 }
                }
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "value", "unit"],
                "properties": {
                    "type": { "type": "string", "enum": ["number"] },
                    "value": { "type": "number" },
                    "unit": nullable_string
                }
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "value"],
                "properties": {
                    "type": { "type": "string", "enum": ["boolean"] },
                    "value": { "type": "boolean" }
                }
            }
        ]
    });
    let nullable_claim = json!({
        "anyOf": [
            claim_value,
            { "type": "null" }
        ]
    });
    let candidate = json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "kind", "subject_ref", "predicate", "object", "actor_ref",
            "perceiver_ref", "target_refs", "evidence", "epistemic_mode",
            "extraction_confidence", "temporal", "durability_hint", "relationship_signal"
        ],
        "properties": {
            "kind": {
                "type": "string",
                "enum": [
                    "event", "utterance", "object_observation", "affect_cue",
                    "relationship_evidence", "intention", "belief_expression", "correction"
                ]
            },
            "subject_ref": { "type": "string", "minLength": 1 },
            "predicate": { "type": "string", "minLength": 1 },
            "object": nullable_claim,
            "actor_ref": nullable_string,
            "perceiver_ref": nullable_string,
            "target_refs": string_array,
            "evidence": {
                "type": "object",
                "additionalProperties": false,
                "required": ["source", "quote", "start_char", "end_char"],
                "properties": {
                    "source": {
                        "type": "string",
                        "enum": ["user_message", "assistant_message"]
                    },
                    "quote": { "type": "string", "minLength": 1 },
                    "start_char": nullable_u32,
                    "end_char": nullable_u32
                }
            },
            "epistemic_mode": {
                "type": "string",
                "enum": [
                    "directly_observed", "stated_by", "narrator_described",
                    "inferred", "remembered_by"
                ]
            },
            "extraction_confidence": {
                "type": "number",
                "minimum": 0,
                "maximum": 1
            },
            "temporal": {
                "type": "object",
                "additionalProperties": false,
                "required": ["anchor", "expression"],
                "properties": {
                    "anchor": {
                        "type": "string",
                        "enum": [
                            "current_turn", "before_current_turn", "after_current_turn",
                            "absolute", "unknown"
                        ]
                    },
                    "expression": nullable_string
                }
            },
            "durability_hint": {
                "type": "string",
                "enum": ["transient", "turn", "session", "long_term", "unknown"]
            },
            "relationship_signal": {
                "anyOf": [
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": [
                            "behaviors", "valence", "directness", "stakes",
                            "costliness", "repetition"
                        ],
                        "properties": {
                            "behaviors": {
                                "type": "array",
                                "items": {
                                    "type": "string",
                                    "enum": [
                                        "promise_kept", "promise_broken", "honest_disclosure",
                                        "deception", "boundary_respected", "boundary_violated",
                                        "support_offered", "harm_threatened", "repair_attempted",
                                        "repair_accepted", "abandonment", "reciprocity",
                                        "competence_displayed"
                                    ]
                                }
                            },
                            "valence": { "type": "integer", "minimum": -5, "maximum": 5 },
                            "directness": { "type": "integer", "minimum": 0, "maximum": 100 },
                            "stakes": { "type": "integer", "minimum": 0, "maximum": 100 },
                            "costliness": { "type": "integer", "minimum": 0, "maximum": 100 },
                            "repetition": { "type": "integer", "minimum": 0, "maximum": 100 }
                        }
                    },
                    { "type": "null" }
                ]
            }
        }
    });

    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "candidates", "no_op_reason"],
        "properties": {
            "schema_version": {
                "type": "integer",
                "enum": [PERCEPTION_IR_SCHEMA_VERSION]
            },
            "candidates": {
                "type": "array",
                "items": candidate
            },
            "no_op_reason": nullable_string
        }
    })
}
