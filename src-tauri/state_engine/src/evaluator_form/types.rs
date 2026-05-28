use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    evaluator::{
        EvaluatorConversionReport, EvaluatorOutputV1, GlobalSceneEvaluation, MemoryCandidate,
        MemorySlot, RelevanceTags, TurnClassification,
    },
    evaluator_ingest::NormalizedEvaluationDraft,
};

pub const EVALUATOR_FORM_VERSION: &str = "evaluator_form_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalFormSpec {
    pub form_version: String,
    pub active_entities: Vec<FormEntityOption>,
    pub active_soul_ids: Vec<String>,
    pub known_object_ids: Vec<String>,
    pub allowed_memory_slots: Vec<MemorySlot>,
    pub allowed_relationship_dimensions: Vec<RelationshipDimension>,
    pub allowed_event_types: Vec<EventType>,
    pub allowed_importance_tiers: Vec<ImportanceTier>,
    pub allowed_tag_vocabularies: Vec<String>,
    pub existing_memories: Vec<ExistingStateRow>,
    pub existing_events: Vec<ExistingStateRow>,
    pub existing_object_observations: Vec<ExistingStateRow>,
    pub existing_relationship_facts: Vec<ExistingStateRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormEntityOption {
    pub entity_id: String,
    pub display_name: String,
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExistingStateRow {
    pub existing_id: String,
    pub kind: ExistingStateKind,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExistingStateKind {
    Memory,
    Event,
    ObjectObservation,
    RelationshipFact,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    SceneEvent,
    LocationChange,
    ObjectChange,
    RelationshipShift,
    CurrentPlotAdvanced,
    UnresolvedTension,
    RecentEmotionalState,
    Correction,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImportanceTier {
    Trivial,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceTier {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipDirection {
    Increase,
    Decrease,
    NoChange,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MagnitudeTier {
    Tiny,
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipDimension {
    Trust,
    Affection,
    Intimacy,
    Passion,
    Commitment,
    Fear,
    Desire,
    Respect,
    Conflict,
    Dependency,
    Curiosity,
    Comfort,
    BoundaryPressure,
}

impl RelationshipDimension {
    pub fn as_label(self) -> &'static str {
        match self {
            RelationshipDimension::Trust => "trust",
            RelationshipDimension::Affection => "affection",
            RelationshipDimension::Intimacy => "intimacy",
            RelationshipDimension::Passion => "passion",
            RelationshipDimension::Commitment => "commitment",
            RelationshipDimension::Fear => "fear",
            RelationshipDimension::Desire => "desire",
            RelationshipDimension::Respect => "respect",
            RelationshipDimension::Conflict => "conflict",
            RelationshipDimension::Dependency => "dependency",
            RelationshipDimension::Curiosity => "curiosity",
            RelationshipDimension::Comfort => "comfort",
            RelationshipDimension::BoundaryPressure => "boundary_pressure",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    New,
    DuplicateOfExisting,
    UpdateExisting,
    SupersedeExisting,
    ContradictsExisting,
    TooMinorNoOp,
    NotSupportedByEvidence,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EvalFormResponse {
    pub event_rows: Vec<EventRow>,
    pub object_rows: Vec<ObjectRow>,
    pub relationship_rows: Vec<RelationshipRow>,
    pub memory_rows: Vec<MemoryRow>,
    pub review_rows: Vec<ReviewRow>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EventRow {
    pub event_id: String,
    pub event_type: Option<EventType>,
    #[serde(alias = "summary")]
    pub objective_summary: String,
    pub participants: Vec<String>,
    pub location: Option<String>,
    pub evidence_quote: String,
    pub importance_tier: Option<ImportanceTier>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ObjectRow {
    #[serde(alias = "event_id")]
    pub linked_event_id: String,
    pub object_id: Option<String>,
    #[serde(alias = "object_label")]
    pub new_object_label: Option<String>,
    #[serde(alias = "change_type", alias = "object_change_type")]
    pub change_type: Option<String>,
    #[serde(alias = "property", alias = "changed_property")]
    pub property_changed: String,
    #[serde(alias = "old_state", alias = "previous_status")]
    pub old_value: Option<String>,
    #[serde(alias = "value", alias = "object_state", alias = "new_state", alias = "status")]
    pub new_value: String,
    pub evidence_quote: String,
    pub confidence_tier: Option<ConfidenceTier>,
    #[serde(alias = "object_type", alias = "object_kind")]
    pub object_kind: Option<String>,
    #[serde(alias = "location_observed")]
    pub location: Option<String>,
    #[serde(skip_serializing)]
    pub summary: Option<String>,
    #[serde(skip_serializing)]
    pub change: Option<String>,
    #[serde(skip_serializing)]
    pub state_change: Option<String>,
    #[serde(skip_serializing)]
    pub location_observation: Option<String>,
    #[serde(skip_serializing)]
    pub associated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RelationshipRow {
    #[serde(alias = "event_id")]
    pub linked_event_id: String,
    #[serde(alias = "source_entity_id")]
    pub source_soul_id: String,
    pub target_entity_id: String,
    #[serde(alias = "relationship_id", skip_serializing)]
    pub relationship_id: Option<String>,
    #[serde(alias = "relationship_dimension", alias = "relationship_dim", alias = "relationship_metric")]
    pub dimension: Option<RelationshipDimension>,
    #[serde(alias = "change_direction", alias = "shift_direction")]
    pub direction: Option<RelationshipDirection>,
    pub magnitude_tier: Option<MagnitudeTier>,
    pub importance_tier: Option<ImportanceTier>,
    pub evidence_quote: String,
    pub summary: Option<String>,
    #[serde(alias = "tags", alias = "tag_vocabularies", alias = "relevance_tags")]
    pub selected_tags: Vec<String>,
    #[serde(alias = "shift")]
    pub shift: Option<String>,
    #[serde(alias = "change_type")]
    pub change_type: Option<String>,
    #[serde(skip_serializing)]
    pub associated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MemoryRow {
    #[serde(alias = "event_id")]
    pub linked_event_id: String,
    pub owner_soul_id: String,
    #[serde(alias = "slot_id", alias = "memory_slot")]
    pub slot: Option<MemorySlot>,
    #[serde(alias = "candidate_memory", alias = "candidate_summary", alias = "content_summary")]
    pub content: String,
    pub evidence_quote: String,
    #[serde(alias = "salience", alias = "importance", alias = "importance_tier")]
    pub importance_tier: Option<ImportanceTier>,
    pub retrieval_cues: Vec<String>,
    #[serde(alias = "tags", alias = "tag_vocabularies", alias = "relevance_tags")]
    pub selected_tags: Vec<String>,
    #[serde(skip_serializing)]
    pub summary: Option<String>,
    #[serde(skip_serializing)]
    pub associated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ReviewRow {
    pub candidate_id: String,
    pub decision: Option<ReviewDecision>,
    pub existing_id: Option<String>,
    pub reason: String,
    pub evidence_quote: String,
    #[serde(skip_serializing)]
    pub associated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalRowTrace {
    pub row_kind: String, // event | object | relationship | memory | review
    pub row_index: usize,
    pub raw_row: Value,
    pub normalized_row: Value,
    pub validation_status: String, // accepted | rejected
    pub rejection_reason: Option<String>,
    pub compiler_result: String, // world_event_created | object_patch_created | relationship_delta_created | memory_candidate_created | non_delta_no_change | advisory_only | rejected
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct EvalFormTrace {
    pub form_spec_event_option_count: usize,
    pub form_existing_memory_option_count: usize,
    pub form_rows_submitted: usize,
    pub form_rows_accepted: usize,
    pub form_rows_rejected: usize,
    pub form_dedupe_decisions: Vec<FormDedupeDecisionTrace>,
    pub compiled_turn_flags_u64: u64,
    pub code_assigned_decay_profile: HashMap<String, String>,
    pub code_assigned_tag_weights: HashMap<String, u8>,
    pub raw_form_repair_applied: bool,
    pub raw_form_repair_warnings: Vec<String>,
    pub json_extract_status: String,
    pub strict_parse_failed_but_salvage_attempted: bool,
    pub salvage_success: bool,
    pub relationship_dimension_inferred_from: Vec<String>,
    pub relationship_direction_inferred_from: Vec<String>,
    pub relationship_rows_split_count: usize,
    #[serde(default)]
    pub relationship_row_results: HashMap<String, String>,
    #[serde(default)]
    pub relationship_non_delta_count: usize,
    #[serde(default)]
    pub evaluator_row_traces: Vec<EvalRowTrace>,
    #[serde(default)]
    pub object_row_results: HashMap<String, String>,
    #[serde(default)]
    pub memory_row_results: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormDedupeDecisionTrace {
    pub candidate_id: String,
    pub decision: ReviewDecision,
    pub existing_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalFormRowRejection {
    pub row_kind: String,
    pub row_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct EvalFormCompileResult {
    pub output: EvaluatorOutputV1,
    pub draft: NormalizedEvaluationDraft,
    pub conversion: EvaluatorConversionReport,
    pub trace: EvalFormTrace,
    pub rejected_rows: Vec<EvalFormRowRejection>,
    pub normalized_response: EvalFormResponse,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalFormRepairTrace {
    pub raw_form_repair_applied: bool,
    pub raw_form_repair_warnings: Vec<String>,
    pub json_extract_status: String,
    pub strict_parse_failed_but_salvage_attempted: bool,
    pub salvage_success: bool,
}
