use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    evaluator::{
        evaluator_output_to_engine_patch, turn_flags, EvaluatorConversionContext,
        EvaluatorConversionReport, EvaluatorOutputV1, GlobalSceneEvaluation, MemoryCandidate,
        MemorySlot, ObjectChangeEvaluation, RelationshipEvaluation, RelevanceTags,
        TurnClassification, WorldChangeEvaluation, EVALUATOR_SCHEMA_VERSION,
    },
    evaluator_ingest::NormalizedEvaluationDraft,
    patch::{MemoryPatch, SceneStatePatch, PATCH_PROTOCOL_VERSION},
    setting::SessionWorld,
    soul::{MemorySourceType, ObjectState, Soul, TruthStatus},
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

pub fn build_eval_form_spec(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    latest_user_message: &str,
    latest_narrator_response: &str,
    top_k: usize,
) -> EvalFormSpec {
    let world = session_world
        .map(SessionWorld::world_log)
        .unwrap_or_else(|| soul.world.clone());
    let mut known_object_ids = world
        .object_states
        .iter()
        .map(|object| object.object_id.clone())
        .filter(|id| !id.trim().is_empty())
        .collect::<Vec<_>>();
    known_object_ids.sort();
    known_object_ids.dedup();

    EvalFormSpec {
        form_version: EVALUATOR_FORM_VERSION.into(),
        active_entities: vec![
            FormEntityOption {
                entity_id: soul.character_id.clone(),
                display_name: soul.character_name.clone(),
                entity_type: "soul".into(),
            },
            FormEntityOption {
                entity_id: "default_player".into(),
                display_name: "User".into(),
                entity_type: "user".into(),
            },
        ],
        active_soul_ids: vec![soul.character_id.clone()],
        known_object_ids,
        allowed_memory_slots: vec![
            MemorySlot::RelationshipMemory,
            MemorySlot::CurrentPlotMemory,
            MemorySlot::CharacterIdentityMemory,
            MemorySlot::UnresolvedTension,
            MemorySlot::WorldLocationMemory,
            MemorySlot::RecentEmotionalState,
        ],
        allowed_relationship_dimensions: all_relationship_dimensions(),
        allowed_event_types: all_event_types(),
        allowed_importance_tiers: vec![
            ImportanceTier::Trivial,
            ImportanceTier::Low,
            ImportanceTier::Medium,
            ImportanceTier::High,
            ImportanceTier::Critical,
        ],
        allowed_tag_vocabularies: default_tag_vocabularies(),
        existing_memories: select_relevant_memories(
            soul,
            latest_user_message,
            latest_narrator_response,
            top_k,
        ),
        existing_events: select_relevant_events(
            &world.recent_events,
            &world.recent_event_records,
            top_k,
        ),
        existing_object_observations: world
            .object_states
            .iter()
            .take(top_k)
            .map(|object| ExistingStateRow {
                existing_id: object
                    .object_observation_id
                    .clone()
                    .unwrap_or_else(|| object.object_id.clone()),
                kind: ExistingStateKind::ObjectObservation,
                summary: format!(
                    "{}: {}",
                    object.object_id,
                    if object.last_observed_state.trim().is_empty() {
                        object.status.as_str()
                    } else {
                        object.last_observed_state.as_str()
                    }
                ),
            })
            .collect(),
        existing_relationship_facts: soul
            .relationships
            .iter()
            .take(top_k)
            .map(|(target, relation)| ExistingStateRow {
                existing_id: format!("rel:{}:{}", soul.character_id, normalize_player_id(target)),
                kind: ExistingStateKind::RelationshipFact,
                summary: format!(
                    "{} -> {} trust {:.1}, affection {:.1}, comfort {:.1}, conflict {:.1}",
                    soul.character_name,
                    normalize_player_id(target),
                    relation.trust,
                    relation.affection,
                    relation.comfort,
                    relation.conflict
                ),
            })
            .collect(),
    }
}

pub fn parse_eval_form_response(raw_json: &str) -> Result<EvalFormResponse, String> {
    parse_eval_form_response_with_trace(raw_json).map(|(response, _)| response)
}

pub fn parse_eval_form_response_with_trace(
    raw_json: &str,
) -> Result<(EvalFormResponse, EvalFormRepairTrace), String> {
    let mut trace = EvalFormRepairTrace {
        json_extract_status: "not_started".into(),
        ..EvalFormRepairTrace::default()
    };
    let stripped = strip_json_fences(raw_json);
    let (extracted, extracted_status) = extract_first_balanced_json_object(&stripped)
        .map(|json| (json, "success".to_string()))
        .unwrap_or_else(|| (stripped.clone(), "not_found_used_full_text".to_string()));
    trace.json_extract_status = extracted_status;

    match serde_json::from_str::<Value>(&extracted) {
        Ok(mut value) => {
            normalize_eval_form_value(&mut value, &mut trace);
            let response = serde_json::from_value(value).map_err(|err| {
                format!("invalid EvalFormResponse JSON after normalization: {err}")
            })?;
            trace.salvage_success = true;
            Ok((response, trace))
        }
        Err(first_err) => {
            trace.strict_parse_failed_but_salvage_attempted = true;
            let repaired = repair_common_json_drift(&extracted, &mut trace);
            let mut value = serde_json::from_str::<Value>(&repaired).map_err(|err| {
                format!("invalid EvalFormResponse JSON: {first_err}; repair failed: {err}")
            })?;
            normalize_eval_form_value(&mut value, &mut trace);
            let response = serde_json::from_value(value)
                .map_err(|err| format!("invalid EvalFormResponse JSON after repair: {err}"))?;
            trace.salvage_success = true;
            Ok((response, trace))
        }
    }
}

fn strip_json_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    stripped
        .strip_suffix("```")
        .unwrap_or(stripped)
        .trim()
        .to_string()
}

fn extract_first_balanced_json_object(raw: &str) -> Option<String> {
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in raw.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => {
                if start.is_none() {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let start = start?;
                    return Some(raw[start..=index].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn repair_unescaped_quotes_in_field(raw: &str, field_name: &str, trace: &mut EvalFormRepairTrace) -> String {
    let mut out = String::new();
    let mut current_pos = 0;
    
    while let Some(field_pos) = raw[current_pos..].find(field_name) {
        let absolute_field_pos = current_pos + field_pos;
        out.push_str(&raw[current_pos..absolute_field_pos]);
        
        let after_field = absolute_field_pos + field_name.len();
        if let Some(colon_offset) = raw[after_field..].find(':') {
            let colon_pos = after_field + colon_offset;
            if let Some(quote_offset) = raw[colon_pos..].find('"') {
                let start_quote_pos = colon_pos + quote_offset;
                
                let search_start = start_quote_pos + 1;
                let mut end_quote_pos = None;
                
                let bytes = raw.as_bytes();
                for i in search_start..bytes.len() {
                    if bytes[i] == b'"' {
                        let mut is_escaped = false;
                        let mut k = i;
                        while k > search_start && bytes[k - 1] == b'\\' {
                            is_escaped = !is_escaped;
                            k -= 1;
                        }
                        if !is_escaped {
                            let mut next = i + 1;
                            while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                                next += 1;
                            }
                            if next < bytes.len() && (bytes[next] == b',' || bytes[next] == b'}' || bytes[next] == b']') {
                                end_quote_pos = Some(i);
                                break;
                            }
                        }
                    }
                }
                
                if let Some(end_pos) = end_quote_pos {
                    let content = &raw[search_start..end_pos];
                    
                    let mut escaped_content = String::new();
                    let content_bytes = content.as_bytes();
                    let mut idx = 0;
                    while idx < content_bytes.len() {
                        if content_bytes[idx] == b'"' {
                            let mut backslash_count = 0;
                            let mut k = idx;
                            while k > 0 && content_bytes[k - 1] == b'\\' {
                                backslash_count += 1;
                                k -= 1;
                            }
                            let is_escaped = backslash_count % 2 == 1;
                            if !is_escaped {
                                escaped_content.push('\\');
                            }
                            escaped_content.push('"');
                        } else {
                            escaped_content.push(content_bytes[idx] as char);
                        }
                        idx += 1;
                    }
                    
                    if escaped_content != content {
                        trace.raw_form_repair_warnings.push(format!("unescaped quotes in {} repaired", field_name));
                        trace.raw_form_repair_applied = true;
                    }
                    
                    out.push_str(&raw[absolute_field_pos..=start_quote_pos]);
                    out.push_str(&escaped_content);
                    out.push('"');
                    
                    current_pos = end_pos + 1;
                    continue;
                }
            }
        }
        
        out.push_str(&raw[absolute_field_pos..after_field]);
        current_pos = after_field;
    }
    
    out.push_str(&raw[current_pos..]);
    out
}

fn repair_common_json_drift(raw: &str, trace: &mut EvalFormRepairTrace) -> String {
    let mut repaired = raw.replace(['“', '”'], "\"").replace(['‘', '’'], "'");
    if repaired != raw {
        trace
            .raw_form_repair_warnings
            .push("smart quotes normalized".into());
        trace.raw_form_repair_applied = true;
    }
    let without_trailing_commas = remove_trailing_commas(&repaired);
    if without_trailing_commas != repaired {
        trace
            .raw_form_repair_warnings
            .push("trailing commas removed".into());
        trace.raw_form_repair_applied = true;
        repaired = without_trailing_commas;
    }
    let quoted_and = repair_quoted_string_and_string(&repaired);
    if quoted_and != repaired {
        trace
            .raw_form_repair_warnings
            .push("quoted string-and-string evidence repaired".into());
        trace.raw_form_repair_applied = true;
        repaired = quoted_and;
    }
    let repaired_evidence = repair_unescaped_quotes_in_field(&repaired, "evidence_quote", trace);
    if repaired_evidence != repaired {
        repaired = repaired_evidence;
    }
    repaired
}

fn remove_trailing_commas(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == ',' {
            let mut lookahead = chars.clone();
            while matches!(lookahead.peek(), Some(next) if next.is_whitespace()) {
                lookahead.next();
            }
            if matches!(lookahead.peek(), Some('}' | ']')) {
                continue;
            }
        }
        out.push(ch);
    }
    out
}

fn repair_quoted_string_and_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            out.push(bytes[index] as char);
            index += 1;
            continue;
        }
        let Some((first, after_first)) = read_json_string(raw, index) else {
            out.push(bytes[index] as char);
            index += 1;
            continue;
        };
        let mut probe = after_first;
        while probe < bytes.len() && bytes[probe].is_ascii_whitespace() {
            probe += 1;
        }
        if !raw[probe..].starts_with("and") {
            out.push_str(&raw[index..after_first]);
            index = after_first;
            continue;
        }
        let after_and = probe + 3;
        let is_word_continuation = bytes
            .get(after_and)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
        if is_word_continuation {
            out.push_str(&raw[index..after_first]);
            index = after_first;
            continue;
        }
        let mut second_start = after_and;
        while second_start < bytes.len() && bytes[second_start].is_ascii_whitespace() {
            second_start += 1;
        }
        if bytes.get(second_start) != Some(&b'"') {
            out.push_str(&raw[index..after_first]);
            index = after_first;
            continue;
        }
        let Some((second, after_second)) = read_json_string(raw, second_start) else {
            out.push_str(&raw[index..after_first]);
            index = after_first;
            continue;
        };
        out.push('"');
        out.push_str(&escape_json_string(&format!("{first}; {second}")));
        out.push('"');
        index = after_second;
    }
    out
}

fn read_json_string(raw: &str, start: usize) -> Option<(String, usize)> {
    let bytes = raw.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut escaped = false;
    let mut value = String::new();
    let mut index = start + 1;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if escaped {
            value.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some((value, index + 1));
        } else {
            value.push(ch);
        }
        index += 1;
    }
    None
}

fn escape_json_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn normalize_eval_form_value(value: &mut Value, trace: &mut EvalFormRepairTrace) {
    normalize_row_array(value, "event_rows", normalize_event_row_value, trace);
    let event_ids = collect_event_ids(value);
    normalize_child_row_array(
        value,
        "object_rows",
        &event_ids,
        normalize_object_row_value,
        trace,
    );
    normalize_child_row_array(
        value,
        "relationship_rows",
        &event_ids,
        normalize_relationship_row_value,
        trace,
    );
    split_relationship_dimensions(value, trace);
    normalize_child_row_array(
        value,
        "relationship_rows",
        &event_ids,
        normalize_relationship_row_value,
        trace,
    );
    normalize_child_row_array(
        value,
        "memory_rows",
        &event_ids,
        normalize_memory_row_value,
        trace,
    );
    normalize_child_row_array(
        value,
        "review_rows",
        &event_ids,
        normalize_review_row_value,
        trace,
    );
}

fn normalize_row_array(
    value: &mut Value,
    key: &str,
    normalize: fn(&mut serde_json::Map<String, Value>, &mut EvalFormRepairTrace),
    trace: &mut EvalFormRepairTrace,
) {
    let Some(rows) = value.get_mut(key).and_then(Value::as_array_mut) else {
        return;
    };
    for row in rows {
        if let Some(object) = row.as_object_mut() {
            normalize(object, trace);
        }
    }
}

fn normalize_child_row_array(
    value: &mut Value,
    key: &str,
    event_ids: &[String],
    normalize: fn(&mut serde_json::Map<String, Value>, &mut EvalFormRepairTrace),
    trace: &mut EvalFormRepairTrace,
) {
    let Some(rows) = value.get_mut(key).and_then(Value::as_array_mut) else {
        return;
    };
    for row in rows {
        if let Some(object) = row.as_object_mut() {
            normalize(object, trace);
            normalize_linked_event_id_value(object, event_ids, trace);
        }
    }
}

fn normalize_event_row_value(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    move_alias(row, "summary", "objective_summary", trace);
    move_alias(row, "kind", "event_type", trace);
    move_alias(row, "type", "event_type", trace);
    move_alias(row, "importance", "importance_tier", trace);
    normalize_event_type_value(row, "event_type", trace);
    if !row.contains_key("event_id") {
        row.insert("event_id".into(), Value::String("event_latest_turn".into()));
        trace
            .raw_form_repair_warnings
            .push("missing event_id defaulted".into());
        trace.raw_form_repair_applied = true;
    }
    if !row.contains_key("objective_summary") {
        if let Some(quote) = row.get("evidence_quote").and_then(Value::as_str) {
            let summary = quote.chars().take(120).collect::<String>();
            row.insert("objective_summary".into(), Value::String(summary));
            trace
                .raw_form_repair_warnings
                .push("missing objective_summary derived from evidence_quote".into());
            trace.raw_form_repair_applied = true;
        }
    }
}

fn normalize_object_row_value(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    move_alias(row, "event_id", "linked_event_id", trace);
    move_alias(row, "property", "property_changed", trace);
    move_alias(row, "change", "property_changed", trace);
    move_alias(row, "changed_property", "property_changed", trace);
    move_alias(row, "value", "new_value", trace);
    move_alias(row, "summary", "new_value", trace);
    move_alias(row, "state_change", "new_value", trace);
    move_alias(row, "new_state", "new_value", trace);
    move_alias(row, "object_state", "new_value", trace);
    move_alias(row, "status", "new_value", trace);
    move_alias(row, "old_state", "old_value", trace);
    move_alias(row, "previous_status", "old_value", trace);
    move_alias(row, "object_label", "new_object_label", trace);
    move_alias(row, "new_object_label", "new_object_label", trace);
    move_alias(row, "object_change_type", "change_type", trace);

    if let Some(val) = row.get_mut("change_type") {
        if let Some(s) = val.as_str() {
            let s_norm = s.trim().to_ascii_lowercase();
            if s_norm == "object_change" {
                *val = Value::String("state_change".into());
                trace.raw_form_repair_warnings.push("change_type object_change normalized to state_change".into());
                trace.raw_form_repair_applied = true;
            }
        }
    }

    // Part A: change_type-only normalizations
    let change_type = row.get("change_type").and_then(Value::as_str).map(|s| s.trim().to_ascii_lowercase());
    let prop_empty = row.get("property_changed").and_then(Value::as_str).unwrap_or("").trim().is_empty();
    let val_empty = row.get("new_value").and_then(Value::as_str).unwrap_or("").trim().is_empty();
    let evidence_quote = row.get("evidence_quote").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let new_object_label = row.get("new_object_label").and_then(Value::as_str).unwrap_or("").trim().to_string();

    if prop_empty {
        if let Some(ref ct) = change_type {
            if ct == "state_change" {
                row.insert("property_changed".into(), Value::String("state".into()));
                trace.raw_form_repair_warnings.push("property_changed derived as state".into());
                trace.raw_form_repair_applied = true;
            } else if ct == "new_object_observation" {
                row.insert("property_changed".into(), Value::String("presence".into()));
                trace.raw_form_repair_warnings.push("property_changed derived as presence".into());
                trace.raw_form_repair_applied = true;
            }
        }
    }

    if val_empty {
        if let Some(ref ct) = change_type {
            if ct == "state_change" {
                let nv = if !evidence_quote.is_empty() {
                    evidence_quote.clone()
                } else {
                    "state_changed".to_string()
                };
                row.insert("new_value".into(), Value::String(nv));
                trace.raw_form_repair_warnings.push("new_value derived for state_change".into());
                trace.raw_form_repair_applied = true;
            } else if ct == "new_object_observation" {
                let nv = if !new_object_label.is_empty() {
                    new_object_label.clone()
                } else if !evidence_quote.is_empty() {
                    evidence_quote.clone()
                } else {
                    "presence_observed".to_string()
                };
                row.insert("new_value".into(), Value::String(nv));
                trace.raw_form_repair_warnings.push("new_value derived for new_object_observation".into());
                trace.raw_form_repair_applied = true;
            }
        }
    }

    let obj_id_empty = row.get("object_id").and_then(Value::as_str).unwrap_or("").trim().is_empty();
    if obj_id_empty {
        if !new_object_label.is_empty() {
            let slug = slugify(&new_object_label);
            row.insert("object_id".into(), Value::String(slug));
            trace.raw_form_repair_warnings.push("object_id canonicalized from new_object_label".into());
            trace.raw_form_repair_applied = true;
        }
    }

    if let Some(object_id) = row.get("object_id").and_then(Value::as_str) {
        if let Some(stripped) = object_id.strip_prefix("obj:") {
            row.insert("object_id".into(), Value::String(stripped.to_string()));
            trace
                .raw_form_repair_warnings
                .push("obj: object_id canonicalized".into());
            trace.raw_form_repair_applied = true;
        } else if let Some(stripped) = object_id.strip_prefix("obj_") {
            row.insert("object_id".into(), Value::String(stripped.to_string()));
            trace
                .raw_form_repair_warnings
                .push("obj_ object_id canonicalized".into());
            trace.raw_form_repair_applied = true;
        }
    }
}

fn normalize_relationship_row_value(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    move_alias(row, "event_id", "linked_event_id", trace);
    move_alias(row, "source_entity_id", "source_soul_id", trace);
    move_alias(row, "relationship_dimension", "dimension", trace);
    move_alias(row, "relationship_dim", "dimension", trace);
    move_alias(row, "relationship_metric", "dimension", trace);
    move_alias(row, "change_direction", "direction", trace);
    move_alias(row, "shift_direction", "direction", trace);
    move_alias(row, "tag_vocabularies", "selected_tags", trace);
    move_alias(row, "relevance_tags", "selected_tags", trace);
    move_alias(row, "tags", "selected_tags", trace);

    if let Some(val) = row.get_mut("change_type") {
        if let Some(s) = val.as_str() {
            let s_norm = s.trim().to_ascii_lowercase();
            if s_norm == "relationship_shift" {
                *val = Value::String("shift".into());
                trace.raw_form_repair_warnings.push("change_type relationship_shift normalized to shift".into());
                trace.raw_form_repair_applied = true;
            }
        }
    }

    infer_relationship_dimension_from_tags(row, trace);
    infer_relationship_direction_from_shift(row, trace);
    infer_relationship_direction_from_summary(row, trace);
    normalize_relationship_direction_value(row, trace);
    normalize_relationship_dimension_value(row, trace);
    normalize_relationship_magnitude_from_importance(row, trace);
    normalize_relationship_tags_value(row, trace);
    if let Some(relationship_id) = row
        .get("relationship_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        let parts = relationship_id
            .split(':')
            .map(str::to_string)
            .collect::<Vec<_>>();
        if parts.len() == 3 && parts[0] == "rel" {
            row.entry("source_soul_id")
                .or_insert_with(|| Value::String(parts[1].clone()));
            row.entry("target_entity_id")
                .or_insert_with(|| Value::String(parts[2].clone()));
            trace
                .raw_form_repair_warnings
                .push("relationship_id split into source and target".into());
            trace.raw_form_repair_applied = true;
        }
    }
}

fn normalize_relationship_tags_value(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    let Some(value) = row.get("selected_tags").cloned() else {
        return;
    };
    let mut tags = relationship_tag_values(&value)
        .into_iter()
        .filter_map(|tag| relationship_dimension_label(&tag).map(str::to_string))
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    row.insert(
        "selected_tags".into(),
        Value::Array(tags.into_iter().map(Value::String).collect()),
    );
    trace
        .raw_form_repair_warnings
        .push("unknown relationship tags dropped".into());
    trace.raw_form_repair_applied = true;
}

fn infer_relationship_dimension_from_tags(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    if row
        .get("dimension")
        .and_then(Value::as_str)
        .and_then(clean)
        .is_some()
    {
        return;
    }
    let Some(tag_value) = row.get("selected_tags").or_else(|| row.get("tags")) else {
        return;
    };
    let tags = relationship_tag_values(tag_value);
    if let Some(dimension) = tags
        .iter()
        .find_map(|tag| relationship_dimension_label(tag))
    {
        row.insert("dimension".into(), Value::String(dimension.into()));
        trace.raw_form_repair_warnings.push(format!(
            "relationship dimension inferred from tag {dimension}"
        ));
        trace.raw_form_repair_applied = true;
    }
}

fn infer_relationship_direction_from_shift(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    if row
        .get("direction")
        .and_then(Value::as_str)
        .and_then(clean)
        .is_some()
    {
        return;
    }

    let mut traditional_inferred = false;
    if let Some(shift) = row.get("shift").and_then(Value::as_str) {
        let normalized = normalize_token(shift);
        let direction = if normalized.contains("increase")
            || normalized.contains("escalation")
            || normalized.contains("pressure_increased")
            || normalized.contains("intensified")
            || normalized.contains("grew")
            || normalized.contains("growing")
            || normalized.contains("more")
        {
            Some("increase")
        } else if normalized.contains("decrease")
            || normalized.contains("softened")
            || normalized.contains("eased")
            || normalized.contains("reduced")
            || normalized.contains("less")
            || normalized.contains("lower")
        {
            Some("decrease")
        } else {
            None
        };
        if let Some(direction) = direction {
            row.insert("direction".into(), Value::String(direction.into()));
            trace.raw_form_repair_warnings.push(format!(
                "relationship direction inferred from shift {direction}"
            ));
            trace.raw_form_repair_applied = true;
            traditional_inferred = true;
        }
    }

    if !traditional_inferred {
        let change_type = row.get("change_type").and_then(Value::as_str).unwrap_or("").trim().to_ascii_lowercase();
        if change_type == "shift" {
            let dimension = row.get("dimension").and_then(Value::as_str).unwrap_or("").trim().to_ascii_lowercase();
            let evidence = row.get("evidence_quote").and_then(Value::as_str).unwrap_or("").trim().to_ascii_lowercase();

            let has_increase_word = evidence.contains("increase")
                || evidence.contains("grew")
                || evidence.contains("grow")
                || evidence.contains("intensified")
                || evidence.contains("warmer")
                || evidence.contains("closer")
                || evidence.contains("more")
                || evidence.contains("strengthen")
                || evidence.contains("deepen")
                || evidence.contains("higher")
                || evidence.contains("up")
                || evidence.contains("escalat")
                || evidence.contains("improv")
                || evidence.contains("build")
                || evidence.contains("built")
                || evidence.contains("enhanc")
                || evidence.contains("whisper")
                || evidence.contains("soft")
                || evidence.contains("drop") // drop/dropping voice
                || evidence.contains("trust")
                || evidence.contains("affect")
                || evidence.contains("intim")
                || evidence.contains("passion")
                || evidence.contains("commit")
                || evidence.contains("desir")
                || evidence.contains("respect")
                || evidence.contains("curios")
                || evidence.contains("interest")
                || evidence.contains("comfort");

            let has_decrease_word = evidence.contains("decrease")
                || evidence.contains("soften")
                || evidence.contains("ease")
                || evidence.contains("reduc")
                || evidence.contains("less")
                || evidence.contains("lower")
                || evidence.contains("wither")
                || evidence.contains("diminish")
                || evidence.contains("fad")
                || evidence.contains("cool")
                || evidence.contains("pull")
                || evidence.contains("withdraw")
                || (evidence.contains("drop") && !evidence.contains("voice"));

            if has_increase_word || has_decrease_word {
                let has_decrease = has_decrease_word && !has_increase_word;
                let direction = if has_decrease {
                    Some("decrease")
                } else if dimension == "trust" || dimension == "affection" || dimension == "intimacy" || dimension == "passion" || dimension == "commitment" || dimension == "desire" || dimension == "respect" || dimension == "curiosity" || dimension == "interest" || dimension == "comfort" {
                    Some("increase")
                } else if dimension == "boundary_pressure" || dimension == "boundarypressure" || dimension == "conflict" || dimension == "fear" {
                    let has_escalation = evidence.contains("pressure") || evidence.contains("conflict") || evidence.contains("fear") || evidence.contains("tension") || evidence.contains("wary") || evidence.contains("guarded") || evidence.contains("edge") || evidence.contains("escalation")
                        || change_type.contains("pressure") || change_type.contains("conflict") || change_type.contains("fear") || change_type.contains("tension") || change_type.contains("wary") || change_type.contains("guarded") || change_type.contains("edge") || change_type.contains("escalation");
                    if has_escalation {
                        Some("increase")
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(dir) = direction {
                    row.insert("direction".into(), Value::String(dir.into()));
                    trace.raw_form_repair_warnings.push(format!(
                        "relationship direction inferred as {dir} for shift"
                    ));
                    trace.raw_form_repair_applied = true;
                }
            }
        }
    }
}

fn infer_relationship_direction_from_summary(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    if row
        .get("direction")
        .and_then(Value::as_str)
        .and_then(clean)
        .is_some()
    {
        return;
    }
    let Some(summary) = row.get("summary").and_then(Value::as_str) else {
        return;
    };
    let normalized = normalize_token(summary);
    let direction = if normalized.contains("increase")
        || normalized.contains("increases")
        || normalized.contains("increased")
    {
        Some("increase")
    } else if normalized.contains("decrease")
        || normalized.contains("decreases")
        || normalized.contains("decreased")
    {
        Some("decrease")
    } else {
        None
    };
    if let Some(direction) = direction {
        row.insert("direction".into(), Value::String(direction.into()));
        trace.raw_form_repair_warnings.push(format!(
            "relationship direction inferred from summary {direction}"
        ));
        trace.raw_form_repair_applied = true;
    }
}

fn normalize_relationship_magnitude_from_importance(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    if row
        .get("magnitude_tier")
        .and_then(Value::as_str)
        .and_then(clean)
        .is_some()
    {
        return;
    }
    let Some(importance) = row.get("importance_tier").and_then(Value::as_str) else {
        return;
    };
    let magnitude = match normalize_token(importance).as_str() {
        "trivial" | "low" => "small",
        "medium" => "small",
        "high" => "medium",
        "critical" => "large",
        _ => "small",
    };
    row.insert("magnitude_tier".into(), Value::String(magnitude.into()));
    trace.raw_form_repair_warnings.push(format!(
        "relationship magnitude inferred from importance_tier {magnitude}"
    ));
    trace.raw_form_repair_applied = true;
}

fn relationship_tag_values(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items.iter().flat_map(relationship_tag_values).collect(),
        Value::String(tag) => vec![tag.clone()],
        Value::Object(map) => map
            .get("value")
            .or_else(|| map.get("tag"))
            .or_else(|| map.get("name"))
            .and_then(Value::as_str)
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn relationship_dimension_label(raw: &str) -> Option<&'static str> {
    match normalize_token(raw).as_str() {
        "trust" => Some("trust"),
        "affection" => Some("affection"),
        "intimacy" => Some("intimacy"),
        "passion" => Some("passion"),
        "commitment" => Some("commitment"),
        "fear" => Some("fear"),
        "desire" => Some("desire"),
        "respect" => Some("respect"),
        "conflict" => Some("conflict"),
        "dependency" => Some("dependency"),
        "curiosity" | "interest" => Some("curiosity"),
        "comfort" => Some("comfort"),
        "boundary_pressure" | "boundarypressure" => Some("boundary_pressure"),
        _ => None,
    }
}

fn normalize_relationship_dimension_value(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    let Some(value) = row.get_mut("dimension") else {
        return;
    };
    let Some(raw) = value.as_str().map(str::to_string) else {
        return;
    };
    let normalized = normalize_token(&raw);
    let mapped = match normalized.as_str() {
        "trust" => "trust",
        "affection" => "affection",
        "intimacy" => "intimacy",
        "passion" => "passion",
        "commitment" => "commitment",
        "fear" => "fear",
        "desire" => "desire",
        "respect" => "respect",
        "conflict" => "conflict",
        "dependency" => "dependency",
        "curiosity" | "interest" => "curiosity",
        "comfort" => "comfort",
        "boundarypressure" | "boundary_pressure" => "boundary_pressure",
        _ => "curiosity",
    };
    if mapped != raw.as_str() {
        *value = Value::String(mapped.into());
        trace.raw_form_repair_warnings.push(format!(
            "relationship dimension {raw} normalized to {mapped}"
        ));
        trace.raw_form_repair_applied = true;
    }
}

fn normalize_memory_row_value(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    move_alias(row, "event_id", "linked_event_id", trace);
    move_alias(row, "memory_slot", "slot", trace);
    move_alias(row, "slot_id", "slot", trace);
    move_alias(row, "slot_type", "slot", trace);
    if let Some(kind) = row.get("kind").cloned() {
        if row.get("slot").is_none() && memory_slot_from_value(&kind).is_some() {
            row.insert("slot".into(), kind);
            trace
                .raw_form_repair_warnings
                .push("kind normalized to memory slot".into());
            trace.raw_form_repair_applied = true;
        }
    }
    move_alias(row, "candidate_memory", "content", trace);
    if row.get("content").is_none() {
        move_alias(row, "candidate_summary", "content", trace);
    }
    if row.get("content").is_none() {
        move_alias(row, "content_summary", "content", trace);
    }
    if row.get("content").is_none() {
        move_alias(row, "summary", "content", trace);
    }
    move_alias(row, "importance_tier", "importance", trace);
    move_alias(row, "salience", "importance", trace);
    move_alias(row, "tag_vocabularies", "selected_tags", trace);
    move_alias(row, "relevance_tags", "selected_tags", trace);
    move_alias(row, "tags", "selected_tags", trace);
    normalize_memory_slot_value(row, "slot", trace);
    normalize_tags_value(row, trace);
}

fn normalize_review_row_value(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    move_alias(row, "event_id", "linked_event_id", trace);
    move_alias(row, "memory_id", "candidate_id", trace);
    move_alias(row, "review_id", "candidate_id", trace);
}

fn move_alias(
    row: &mut serde_json::Map<String, Value>,
    from: &str,
    to: &str,
    trace: &mut EvalFormRepairTrace,
) {
    if row.contains_key(to) {
        return;
    }
    if let Some(value) = row.remove(from) {
        row.insert(to.into(), value);
        trace
            .raw_form_repair_warnings
            .push(format!("{from} normalized to {to}"));
        trace.raw_form_repair_applied = true;
    }
}

fn collect_event_ids(value: &Value) -> Vec<String> {
    value
        .get("event_rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("event_id").and_then(Value::as_str))
        .filter_map(clean)
        .map(str::to_string)
        .collect()
}

fn normalize_linked_event_id_value(
    row: &mut serde_json::Map<String, Value>,
    event_ids: &[String],
    trace: &mut EvalFormRepairTrace,
) {
    if row
        .get("linked_event_id")
        .and_then(Value::as_str)
        .and_then(clean)
        .is_some()
    {
        return;
    }
    if let Some(associated) = row
        .get("associated_event_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find(|id| event_ids.iter().any(|event_id| event_id == id))
    {
        row.insert(
            "linked_event_id".into(),
            Value::String(associated.to_string()),
        );
        trace
            .raw_form_repair_warnings
            .push("associated_event_ids linked child row".into());
        trace.raw_form_repair_applied = true;
        return;
    }
    if event_ids.len() == 1 {
        row.insert(
            "linked_event_id".into(),
            Value::String(event_ids[0].clone()),
        );
        trace
            .raw_form_repair_warnings
            .push("missing linked_event_id used single event".into());
        trace.raw_form_repair_applied = true;
    } else if let Some(event_id) = event_ids.first() {
        row.insert("linked_event_id".into(), Value::String(event_id.clone()));
        trace
            .raw_form_repair_warnings
            .push("missing linked_event_id used main event".into());
        trace.raw_form_repair_applied = true;
    } else {
        row.insert(
            "linked_event_id".into(),
            Value::String("event_latest_turn".into()),
        );
        trace
            .raw_form_repair_warnings
            .push("missing linked_event_id used synthesized event".into());
        trace.raw_form_repair_applied = true;
    }
}

fn normalize_event_type_value(
    row: &mut serde_json::Map<String, Value>,
    key: &str,
    trace: &mut EvalFormRepairTrace,
) {
    let Some(value) = row.get_mut(key) else {
        return;
    };
    let Some(raw) = value.as_str().map(str::to_string) else {
        return;
    };
    let normalized = normalize_token(&raw);
    let mapped = match normalized.as_str() {
        "scene" | "scene_turn" | "scene_event" => "scene_event",
        "location" | "location_change" => "location_change",
        "object" | "object_change" => "object_change",
        "relationship" | "relationship_shift" => "relationship_shift",
        "plot" | "current_plot" | "current_plot_advanced" => "current_plot_advanced",
        "tension" | "unresolved_tension" => "unresolved_tension",
        "emotion" | "emotional_state" | "recent_emotional_state" => "recent_emotional_state",
        "correction" | "retcon" => "correction",
        _ => "scene_event",
    };
    if mapped != raw.as_str() {
        *value = Value::String(mapped.into());
        trace
            .raw_form_repair_warnings
            .push(format!("event_type {raw} normalized to {mapped}"));
        trace.raw_form_repair_applied = true;
    }
}

fn normalize_relationship_direction_value(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    let Some(value) = row.get_mut("direction") else {
        return;
    };
    let Some(raw) = value.as_str().map(str::to_string) else {
        return;
    };
    let normalized = normalize_token(&raw);
    let mapped = if normalized.contains("increase")
        || normalized.contains("increased")
        || normalized.contains("interest")
        || normalized.contains("warmer")
        || normalized.contains("closer")
    {
        "increase"
    } else if normalized.contains("decrease")
        || normalized.contains("decreased")
        || normalized.contains("less")
        || normalized.contains("lower")
    {
        "decrease"
    } else if normalized.contains("mixed")
        || normalized.contains("no_change")
        || normalized.contains("unchanged")
    {
        "no_change"
    } else {
        "no_change"
    };
    if mapped != raw.as_str() {
        *value = Value::String(mapped.into());
        trace
            .raw_form_repair_warnings
            .push(format!("direction {raw} normalized to {mapped}"));
        trace.raw_form_repair_applied = true;
    }
}

fn normalize_memory_slot_value(
    row: &mut serde_json::Map<String, Value>,
    key: &str,
    trace: &mut EvalFormRepairTrace,
) {
    let Some(value) = row.get_mut(key) else {
        return;
    };
    let Some(mapped) = memory_slot_from_value(value) else {
        *value = Value::String("unknown".into());
        trace
            .raw_form_repair_warnings
            .push("unknown memory slot normalized to unknown".into());
        trace.raw_form_repair_applied = true;
        return;
    };
    if value.as_str() != Some(mapped) {
        *value = Value::String(mapped.into());
        trace
            .raw_form_repair_warnings
            .push(format!("memory slot normalized to {mapped}"));
        trace.raw_form_repair_applied = true;
    }
}

fn memory_slot_from_value(value: &Value) -> Option<&'static str> {
    let raw = value.as_str()?;
    let normalized = normalize_token(raw);
    match normalized.as_str() {
        "relationship" | "relationship_memory" => Some("relationship_memory"),
        "current_plot" | "plot" | "current_plot_memory" => Some("current_plot_memory"),
        "character_identity" | "character_identity_memory" => Some("character_identity_memory"),
        "unresolved_tension" | "tension" => Some("unresolved_tension"),
        "world_location" | "location" | "world_location_memory" => Some("world_location_memory"),
        "recent_emotional_state" | "emotional_state" | "emotion" => Some("recent_emotional_state"),
        "unknown" => Some("unknown"),
        _ => None,
    }
}

fn normalize_tags_value(row: &mut serde_json::Map<String, Value>, trace: &mut EvalFormRepairTrace) {
    let Some(tags) = row.get_mut("selected_tags").and_then(Value::as_array_mut) else {
        return;
    };
    let before = tags.len();
    let mut seen = HashSet::new();
    tags.retain_mut(|tag| {
        let Some(raw) = tag.as_str() else {
            return false;
        };
        let normalized = normalize_token(raw);
        let canonical = match normalized.as_str() {
            "sceneevent" | "scene_event" => "scene_event",
            "relationship" => "relationship",
            "currentplot" | "current_plot" => "current_plot",
            "location" => "location",
            "object" => "object",
            "emotionalstate" | "emotional_state" => "emotional_state",
            "boundary" => "boundary",
            "doorway" => "doorway",
            "reunion" => "reunion",
            _ => return false,
        };
        *tag = Value::String(canonical.into());
        seen.insert(canonical.to_string())
    });
    if tags.len() != before {
        trace
            .raw_form_repair_warnings
            .push("unknown tags dropped".into());
        trace.raw_form_repair_applied = true;
    }
}

fn split_relationship_dimensions(value: &mut Value, trace: &mut EvalFormRepairTrace) {
    let Some(rows) = value
        .get_mut("relationship_rows")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let mut expanded = Vec::new();
    for row in rows.drain(..) {
        let Some(dimensions) = row.get("dimensions_changed").and_then(Value::as_array) else {
            expanded.push(row);
            continue;
        };
        if dimensions.is_empty() {
            expanded.push(row);
            continue;
        }
        for dimension in dimensions {
            let mut next = row.clone();
            if let Some(object) = next.as_object_mut() {
                object.insert("dimension".into(), dimension.clone());
            }
            expanded.push(next);
        }
        trace
            .raw_form_repair_warnings
            .push("dimensions_changed split into relationship rows".into());
        trace.raw_form_repair_applied = true;
    }
    *rows = expanded;
}

fn normalize_token(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut previous_underscore = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            previous_underscore = false;
        } else if !previous_underscore {
            out.push('_');
            previous_underscore = true;
        }
    }
    out.trim_matches('_').to_string()
}

pub fn compile_eval_form_response(
    spec: &EvalFormSpec,
    response: &EvalFormResponse,
    context: &EvaluatorConversionContext<'_>,
) -> EvalFormCompileResult {
    let response = normalize_eval_form_response(spec, response, context);
    let response = &response;
    let mut rejected_rows = Vec::new();
    let mut output = EvaluatorOutputV1 {
        schema_version: EVALUATOR_SCHEMA_VERSION,
        ..EvaluatorOutputV1::default()
    };
    let mut trace = EvalFormTrace {
        form_spec_event_option_count: spec.allowed_event_types.len(),
        form_existing_memory_option_count: spec.existing_memories.len(),
        form_rows_submitted: response.event_rows.len()
            + response.object_rows.len()
            + response.relationship_rows.len()
            + response.memory_rows.len()
            + response.review_rows.len(),
        ..EvalFormTrace::default()
    };
    trace.relationship_dimension_inferred_from = response
        .relationship_rows
        .iter()
        .filter(|row| row.dimension.is_some() && !row.selected_tags.is_empty())
        .map(|row| format!("tags:{}", row.selected_tags.join(",")))
        .collect();
    trace.relationship_direction_inferred_from = response
        .relationship_rows
        .iter()
        .filter(|row| row.direction.is_some() && row.summary.as_deref().and_then(clean).is_some())
        .map(|_| "summary".to_string())
        .collect();
    let allowed_entities = spec
        .active_entities
        .iter()
        .map(|entity| entity.entity_id.as_str())
        .collect::<HashSet<_>>();
    let mut event_ids = response
        .event_rows
        .iter()
        .filter_map(|row| clean(&row.event_id))
        .collect::<HashSet<_>>();
    if let Some(ref baseline_id) = context.baseline_recent_event_id {
        event_ids.insert(baseline_id.as_str());
    }
    let review_map = response
        .review_rows
        .iter()
        .filter_map(|row| {
            let decision = row.decision?;
            clean(&row.candidate_id).map(|id| (id.to_string(), (decision, row)))
        })
        .collect::<HashMap<_, _>>();

    for row in &response.event_rows {
        if validate_event_row(row, spec, &allowed_entities, &mut rejected_rows) {
            trace.form_rows_accepted += 1;
            output.world_changes.push(WorldChangeEvaluation {
                change_id: Some(row.event_id.clone()),
                location: row
                    .location
                    .as_ref()
                    .and_then(|value| clean(value).map(str::to_string)),
                event_summary: clean(&row.objective_summary).map(str::to_string),
                scene_state: Some(scene_state_from_event(row, context)),
                evidence_quote: clean(&row.evidence_quote).map(str::to_string),
                confidence: confidence_from_importance(
                    row.importance_tier.unwrap_or(ImportanceTier::Medium),
                ),
                relevance_tags: relevance_from_event(row),
                ..WorldChangeEvaluation::default()
            });
            apply_event_flags(&mut output, row);
        }
    }

    for row in &response.object_rows {
        if validate_object_row(row, spec, &event_ids, &mut rejected_rows) {
            trace.form_rows_accepted += 1;
            let object_id = row
                .object_id
                .as_ref()
                .and_then(|id| clean(id).map(str::to_string))
                .or_else(|| {
                    row.new_object_label
                        .as_ref()
                        .and_then(|id| clean(id).map(slugify))
                })
                .unwrap_or_else(|| "unknown_object".into());
            output.object_changes.push(ObjectChangeEvaluation {
                change_id: Some(stable_id(
                    "object_form",
                    &format!(
                        "{}:{}:{}",
                        row.linked_event_id, object_id, row.property_changed
                    ),
                )),
                object_state: ObjectState {
                    object_id: object_id.clone(),
                    object_kind: row
                        .object_kind
                        .clone()
                        .and_then(|k| clean(&k).map(str::to_string))
                        .unwrap_or_else(|| infer_object_kind(&object_id)),
                    status: row.new_value.clone(),
                    last_observed_state: format!("{}: {}", row.property_changed, row.new_value),
                    confidence: confidence_from_confidence_tier(
                        row.confidence_tier.unwrap_or(ConfidenceTier::Medium),
                    ),
                    location: row
                        .location
                        .clone()
                        .and_then(|l| clean(&l).map(str::to_string))
                        .unwrap_or_default(),
                    ..ObjectState::default()
                },
                evidence_quote: clean(&row.evidence_quote).map(str::to_string),
                confidence: confidence_from_confidence_tier(
                    row.confidence_tier.unwrap_or(ConfidenceTier::Medium),
                ),
                ..ObjectChangeEvaluation::default()
            });
            output.turn_flags_u64 |= turn_flags::OBJECT_CHANGE | turn_flags::WORLD_CHANGE;
        }
    }

    for row in &response.relationship_rows {
        if validate_relationship_row(row, spec, &allowed_entities, &event_ids, &mut rejected_rows) {
            trace.form_rows_accepted += 1;
            if row.direction != Some(RelationshipDirection::NoChange) {
                output
                    .relationship_evaluations
                    .push(relationship_from_row(row));
                output.turn_flags_u64 |= turn_flags::RELATIONSHIP_SHIFT;
            }
        }
    }

    for row in &response.memory_rows {
        let candidate_id = memory_candidate_id(row);
        let review = review_map.get(&candidate_id).copied();
        if let Some((decision, review_row)) = review {
            trace.form_dedupe_decisions.push(FormDedupeDecisionTrace {
                candidate_id: candidate_id.clone(),
                decision,
                existing_id: review_row.existing_id.clone(),
                reason: review_row.reason.clone(),
            });
            if !review_row.evidence_quote.trim().is_empty() {
                trace.form_rows_accepted += 1;
            }
        }
        if !validate_memory_row(row, spec, &event_ids, &mut rejected_rows) {
            continue;
        }
        if matches!(
            review.map(|(decision, _)| decision),
            Some(
                ReviewDecision::DuplicateOfExisting
                    | ReviewDecision::TooMinorNoOp
                    | ReviewDecision::NotSupportedByEvidence
            )
        ) {
            trace.form_rows_accepted += 1;
            continue;
        }
        trace.form_rows_accepted += 1;
        let candidate = memory_candidate_from_row(row, &candidate_id);
        if row.owner_soul_id == "session_world" {
            output.world_changes.push(WorldChangeEvaluation {
                change_id: Some(candidate_id),
                event_summary: Some(row.content.clone()),
                evidence_quote: Some(row.evidence_quote.clone()),
                confidence: candidate.confidence,
                ..WorldChangeEvaluation::default()
            });
            output.turn_flags_u64 |= turn_flags::WORLD_CHANGE;
        } else {
            output.memory_candidates.push(candidate);
        }
    }

    for row in &response.review_rows {
        if validate_review_row(row, spec, &mut rejected_rows) {
            trace.form_rows_accepted += 1;
        }
    }

    trace.form_rows_rejected = rejected_rows.len();
    output.turn_flags_u64 = compute_turn_flags(&output);
    output.turn_classification = TurnClassification {
        is_pure_ooc: false,
        scene_event_occurred: output.turn_flags_u64 & turn_flags::SCENE_TURN != 0,
        is_retcon_or_correction: output.turn_flags_u64 & turn_flags::RETCON_OR_CORRECTION != 0,
        human_summary: response
            .event_rows
            .iter()
            .filter_map(|row| clean(&row.objective_summary))
            .next()
            .unwrap_or_default()
            .to_string(),
    };
    output.global_scene_evaluation = global_scene_from_output(&output);
    output.relevance_tags = aggregate_relevance_tags(&output);
    trace.compiled_turn_flags_u64 = output.turn_flags_u64;
    trace.code_assigned_decay_profile = response
        .memory_rows
        .iter()
        .map(|row| {
            (
                memory_candidate_id(row),
                decay_profile(row.importance_tier.unwrap_or(ImportanceTier::Medium)).to_string(),
            )
        })
        .collect();
    trace.code_assigned_tag_weights = flatten_tag_weights(&output.relevance_tags);

    let draft = draft_from_output(&output, &rejected_rows, &trace);
    let mut conversion = evaluator_output_to_engine_patch(&output, context);
    apply_review_memory_operations(&mut conversion, response, &review_map);
    EvalFormCompileResult {
        output,
        draft,
        conversion,
        trace,
        rejected_rows,
        normalized_response: response.clone(),
    }
}

fn normalize_eval_form_response(
    spec: &EvalFormSpec,
    response: &EvalFormResponse,
    context: &EvaluatorConversionContext<'_>,
) -> EvalFormResponse {
    let mut normalized = response.clone();
    if normalized.event_rows.is_empty()
        && context.baseline_recent_event_id.is_none()
        && (!context.latest_user_message.trim().is_empty()
            || !context.latest_narrator_response.trim().is_empty())
    {
        let summary = compact_latest_turn_summary(context);
        normalized.event_rows.push(EventRow {
            event_id: "event_latest_turn".into(),
            event_type: Some(EventType::SceneEvent),
            objective_summary: summary,
            participants: default_participants(spec),
            evidence_quote: context.latest_user_message.to_string(),
            importance_tier: Some(ImportanceTier::Medium),
            ..EventRow::default()
        });
    }
    let event_summaries = normalized
        .event_rows
        .iter()
        .filter_map(|row| {
            let id = clean(&row.event_id)?;
            let summary = clean(&row.objective_summary)?;
            Some((id.to_string(), summary.to_string()))
        })
        .collect::<HashMap<_, _>>();
    let event_ids = normalized
        .event_rows
        .iter()
        .filter_map(|row| clean(&row.event_id).map(str::to_string))
        .collect::<Vec<_>>();
    let main_event_id = choose_main_event_id(&normalized.event_rows)
        .or_else(|| event_ids.first().cloned())
        .or_else(|| context.baseline_recent_event_id.clone())
        .unwrap_or_else(|| "event_latest_turn".into());

    for row in &mut normalized.relationship_rows {
        normalize_child_link(
            &mut row.linked_event_id,
            &row.associated_event_ids,
            &event_ids,
            &main_event_id,
        );
        normalize_relationship_aliases(row, spec);
        normalize_relationship_defaults(row);
    }
    for row in &mut normalized.object_rows {
        normalize_child_link(
            &mut row.linked_event_id,
            &row.associated_event_ids,
            &event_ids,
            &main_event_id,
        );
        normalize_object_aliases(row);
    }
    for row in &mut normalized.memory_rows {
        normalize_child_link(
            &mut row.linked_event_id,
            &row.associated_event_ids,
            &event_ids,
            &main_event_id,
        );
        normalize_memory_aliases(row, &event_summaries, spec);
    }
    for row in &mut normalized.review_rows {
        if row.candidate_id.trim().is_empty() {
            row.candidate_id = stable_id(
                "review_form",
                &format!("{}:{}", row.reason, row.evidence_quote),
            );
        }
    }

    normalized
}

fn default_participants(spec: &EvalFormSpec) -> Vec<String> {
    let mut participants = spec.active_soul_ids.clone();
    if !participants.iter().any(|id| id == "default_player") {
        participants.push("default_player".into());
    }
    participants
}

fn compact_latest_turn_summary(context: &EvaluatorConversionContext<'_>) -> String {
    let narrator = context.latest_narrator_response.trim();
    if !narrator.is_empty() {
        return narrator.chars().take(220).collect();
    }
    let user = context.latest_user_message.trim();
    if !user.is_empty() {
        return format!(
            "Latest user action: {}",
            user.chars().take(180).collect::<String>()
        );
    }
    "The current scene advanced.".into()
}

fn choose_main_event_id(rows: &[EventRow]) -> Option<String> {
    rows.iter()
        .max_by_key(|row| importance_rank(row.importance_tier.unwrap_or(ImportanceTier::Medium)))
        .and_then(|row| clean(&row.event_id).map(str::to_string))
}

fn importance_rank(tier: ImportanceTier) -> u8 {
    match tier {
        ImportanceTier::Trivial => 0,
        ImportanceTier::Low => 1,
        ImportanceTier::Medium => 2,
        ImportanceTier::High => 3,
        ImportanceTier::Critical => 4,
    }
}

fn normalize_child_link(
    linked_event_id: &mut String,
    associated_event_ids: &[String],
    event_ids: &[String],
    main_event_id: &str,
) {
    if clean(linked_event_id).is_some() {
        return;
    }
    if let Some(associated) = associated_event_ids
        .iter()
        .find(|id| event_ids.iter().any(|event_id| event_id == *id))
    {
        *linked_event_id = associated.clone();
    } else if event_ids.len() == 1 {
        *linked_event_id = event_ids[0].clone();
    } else {
        *linked_event_id = main_event_id.to_string();
    }
}

fn normalize_relationship_aliases(row: &mut RelationshipRow, spec: &EvalFormSpec) {
    if let Some(relationship_id) = row.relationship_id.as_deref().and_then(clean) {
        let clean_rel = relationship_id
            .strip_prefix("rel:")
            .unwrap_or(relationship_id);
        let parts = clean_rel
            .split(':')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>();
        if parts.len() == 2 {
            if row.source_soul_id.trim().is_empty() {
                row.source_soul_id = parts[0].clone();
            }
            if row.target_entity_id.trim().is_empty() {
                row.target_entity_id = parts[1].clone();
            }
        } else if parts.len() == 3 && clean_rel.starts_with("rel:") {
            if row.source_soul_id.trim().is_empty() {
                row.source_soul_id = parts[1].clone();
            }
            if row.target_entity_id.trim().is_empty() {
                row.target_entity_id = parts[2].clone();
            }
        } else if parts.len() == 1 {
            let split_dash = clean_rel.split('-').collect::<Vec<_>>();
            if split_dash.len() == 2 {
                if row.source_soul_id.trim().is_empty() {
                    row.source_soul_id = split_dash[0].trim().to_string();
                }
                if row.target_entity_id.trim().is_empty() {
                    row.target_entity_id = split_dash[1].trim().to_string();
                }
            }
        }
    }

    if !row.source_soul_id.trim().is_empty() {
        row.source_soul_id = resolve_active_entity_id(&row.source_soul_id, spec);
    }
    if !row.target_entity_id.trim().is_empty() {
        row.target_entity_id = resolve_active_entity_id(&row.target_entity_id, spec);
    }

    let has_default_player = spec.active_entities.iter().any(|e| e.entity_id == "default_player");
    if spec.active_soul_ids.len() == 1 {
        let active_soul_id = &spec.active_soul_ids[0];
        if row.source_soul_id.trim().is_empty() {
            row.source_soul_id = active_soul_id.clone();
        }
        if row.target_entity_id.trim().is_empty() {
            row.target_entity_id = "default_player".to_string();
        }
    } else if has_default_player {
        if row.target_entity_id.trim().is_empty() {
            row.target_entity_id = "default_player".to_string();
        }
    }

    row.source_soul_id = normalize_player_id(&row.source_soul_id);
    row.target_entity_id = normalize_player_id(&row.target_entity_id);

    if spec.active_soul_ids.len() == 1 {
        let active_soul_id = &spec.active_soul_ids[0];
        if row.source_soul_id == "default_player" {
            row.source_soul_id = active_soul_id.clone();
            row.target_entity_id = "default_player".to_string();
        }
    }
}

fn normalize_relationship_defaults(row: &mut RelationshipRow) {
    if let Some(shift_str) = &row.shift {
        let clean_shift = shift_str.trim().trim_start_matches('+');
        if let Ok(val) = clean_shift.parse::<f32>() {
            if val > 0.0 {
                row.direction = Some(RelationshipDirection::Increase);
            } else if val < 0.0 {
                row.direction = Some(RelationshipDirection::Decrease);
            } else {
                row.direction = Some(RelationshipDirection::NoChange);
            }
        }
    }

    if row.direction.is_none() {
        if let Some(ref ct) = row.change_type {
            if ct.trim().to_ascii_lowercase() == "shift" {
                if let Some(dim) = row.dimension {
                    let is_positive = matches!(
                        dim,
                        RelationshipDimension::Trust
                            | RelationshipDimension::Affection
                            | RelationshipDimension::Intimacy
                            | RelationshipDimension::Passion
                            | RelationshipDimension::Commitment
                            | RelationshipDimension::Desire
                            | RelationshipDimension::Respect
                            | RelationshipDimension::Curiosity
                            | RelationshipDimension::Comfort
                    );
                    
                    let evidence_lower = row.evidence_quote.to_ascii_lowercase();
                    let change_type_lower = ct.to_ascii_lowercase();
                    
                    let has_escalation_keyword =
                        evidence_lower.contains("pressure") || evidence_lower.contains("conflict") ||
                        evidence_lower.contains("fear") || evidence_lower.contains("tension") ||
                        evidence_lower.contains("wary") || evidence_lower.contains("guarded") ||
                        evidence_lower.contains("edge") || evidence_lower.contains("escalation") ||
                        change_type_lower.contains("pressure") || change_type_lower.contains("conflict") ||
                        change_type_lower.contains("fear") || change_type_lower.contains("tension") ||
                        change_type_lower.contains("wary") || change_type_lower.contains("guarded") ||
                        change_type_lower.contains("edge") || change_type_lower.contains("escalation");
                        
                    let has_deescalation_keyword =
                        evidence_lower.contains("softened") || evidence_lower.contains("eased") ||
                        evidence_lower.contains("reduced") || evidence_lower.contains("decreased") ||
                        evidence_lower.contains("less") || evidence_lower.contains("lower") ||
                        change_type_lower.contains("softened") || change_type_lower.contains("eased") ||
                        change_type_lower.contains("reduced") || change_type_lower.contains("decreased") ||
                        change_type_lower.contains("less") || change_type_lower.contains("lower");

                    let has_increase_keyword =
                        evidence_lower.contains("increase") || evidence_lower.contains("grew") ||
                        evidence_lower.contains("grow") || evidence_lower.contains("intensified") ||
                        evidence_lower.contains("warmer") || evidence_lower.contains("closer") ||
                        evidence_lower.contains("more") || evidence_lower.contains("strengthen") ||
                        evidence_lower.contains("deepen") || evidence_lower.contains("higher") ||
                        evidence_lower.contains("up") || evidence_lower.contains("improving") ||
                        evidence_lower.contains("improved") || evidence_lower.contains("building") ||
                        evidence_lower.contains("built") || evidence_lower.contains("enhanced") ||
                        evidence_lower.contains("drop") || evidence_lower.contains("whisper") ||
                        evidence_lower.contains("soft") || evidence_lower.contains("trust") ||
                        evidence_lower.contains("affect") || evidence_lower.contains("intim") ||
                        evidence_lower.contains("passion") || evidence_lower.contains("commit") ||
                        evidence_lower.contains("desir") || evidence_lower.contains("respect") ||
                        evidence_lower.contains("curios") || evidence_lower.contains("interest") ||
                        evidence_lower.contains("comfort") ||
                        change_type_lower.contains("increase") || change_type_lower.contains("grew") ||
                        change_type_lower.contains("grow") || change_type_lower.contains("intensified") ||
                        change_type_lower.contains("warmer") || change_type_lower.contains("closer") ||
                        change_type_lower.contains("more") || change_type_lower.contains("strengthen") ||
                        change_type_lower.contains("deepen") || change_type_lower.contains("higher") ||
                        change_type_lower.contains("up") || change_type_lower.contains("improving") ||
                        change_type_lower.contains("improved") || change_type_lower.contains("building") ||
                        change_type_lower.contains("built") || change_type_lower.contains("enhanced") ||
                        change_type_lower.contains("drop") || change_type_lower.contains("whisper") ||
                        change_type_lower.contains("soft") || change_type_lower.contains("trust") ||
                        change_type_lower.contains("affect") || change_type_lower.contains("intim") ||
                        change_type_lower.contains("passion") || change_type_lower.contains("commit") ||
                        change_type_lower.contains("desir") || change_type_lower.contains("respect") ||
                        change_type_lower.contains("curios") || change_type_lower.contains("interest") ||
                        change_type_lower.contains("comfort");

                    let strongly_implied = has_increase_keyword || has_escalation_keyword || has_deescalation_keyword;

                    if strongly_implied {
                        if is_positive {
                            if has_deescalation_keyword && !has_escalation_keyword {
                                row.direction = Some(RelationshipDirection::Decrease);
                            } else {
                                row.direction = Some(RelationshipDirection::Increase);
                            }
                        } else {
                            if has_escalation_keyword {
                                row.direction = Some(RelationshipDirection::Increase);
                            } else if has_deescalation_keyword {
                                row.direction = Some(RelationshipDirection::Decrease);
                            }
                        }
                    }
                }
            }
        }
    }

    if row.magnitude_tier.is_none() {
        if let Some(ref shift_str) = row.shift {
            let clean_shift = shift_str.trim().trim_start_matches('+');
            if let Ok(val) = clean_shift.parse::<f32>() {
                let abs_val = val.abs();
                row.magnitude_tier = Some(if abs_val >= 3.0 {
                    MagnitudeTier::Large
                } else if abs_val >= 2.0 {
                    MagnitudeTier::Medium
                } else {
                    MagnitudeTier::Small
                });
            }
        }
        
        if row.magnitude_tier.is_none() {
            if let Some(importance) = row.importance_tier {
                row.magnitude_tier = Some(match importance {
                    ImportanceTier::Trivial | ImportanceTier::Low => MagnitudeTier::Small,
                    ImportanceTier::Medium => MagnitudeTier::Small,
                    ImportanceTier::High => MagnitudeTier::Medium,
                    ImportanceTier::Critical => MagnitudeTier::Large,
                });
            }
        }
    }

    if row.magnitude_tier.is_none() {
        row.magnitude_tier = Some(MagnitudeTier::Small);
    }
}

fn normalize_object_aliases(row: &mut ObjectRow) {
    let summary = row.summary.as_deref().and_then(clean);
    let change = row.change.as_deref().and_then(clean);
    let state_change = row.state_change.as_deref().and_then(clean);
    let location_observation = row.location_observation.as_deref().and_then(clean);

    if row.property_changed.trim().is_empty() {
        if let Some(ref ct) = row.change_type {
            let ct_norm = ct.trim().to_ascii_lowercase();
            if ct_norm == "state_change" {
                row.property_changed = "state".to_string();
            } else if ct_norm == "new_object_observation" {
                row.property_changed = "presence".to_string();
            }
        }
    }

    if row.new_value.trim().is_empty() {
        if let Some(ref ct) = row.change_type {
            let ct_norm = ct.trim().to_ascii_lowercase();
            if ct_norm == "state_change" {
                row.new_value = if !row.evidence_quote.trim().is_empty() {
                    row.evidence_quote.clone()
                } else {
                    "state_changed".to_string()
                };
            } else if ct_norm == "new_object_observation" {
                row.new_value = if let Some(ref label) = row.new_object_label {
                    label.clone()
                } else if !row.evidence_quote.trim().is_empty() {
                    row.evidence_quote.clone()
                } else {
                    "presence_observed".to_string()
                };
            }
        }
    }

    if row.object_id.as_deref().and_then(clean).is_none() {
        if let Some(label) = row.new_object_label.as_deref().and_then(clean) {
            row.object_id = Some(slugify(label));
        }
    }

    if row.property_changed.trim().is_empty() {
        if !row.new_value.trim().is_empty() {
            row.property_changed = "state".to_string();
        } else if let Some(value) = change.or(state_change).or(location_observation).or(summary) {
            row.property_changed = value.to_string();
        } else {
            row.property_changed = "state".to_string();
        }
    }
    if row.new_value.trim().is_empty() {
        if let Some(value) = state_change.or(summary).or(change).or(location_observation) {
            row.new_value = value.to_string();
        }
    }
    if let Some(object_id) = row.object_id.as_mut() {
        if let Some(stripped) = object_id.strip_prefix("obj:") {
            *object_id = stripped.to_string();
        } else if let Some(stripped) = object_id.strip_prefix("obj_") {
            *object_id = stripped.to_string();
        }
    }
}

fn normalize_memory_aliases(
    row: &mut MemoryRow,
    event_summaries: &HashMap<String, String>,
    spec: &EvalFormSpec,
) {
    if row.content.trim().is_empty() {
        if let Some(summary) = row.summary.as_deref().and_then(clean) {
            row.content = summary.to_string();
        } else if let Some(evidence) = Some(&row.evidence_quote).and_then(|e| clean(e)) {
            row.content = evidence.to_string();
        } else if let Some(summary) = event_summaries.get(row.linked_event_id.trim()) {
            let slot = row.slot.map(|slot| slot.as_label()).unwrap_or("unknown");
            row.content = format!("{slot}: {summary}");
        }
    }
    if row.owner_soul_id.trim().is_empty() {
        row.owner_soul_id = match row.slot.unwrap_or(MemorySlot::Unknown) {
            MemorySlot::WorldLocationMemory => "session_world".into(),
            MemorySlot::RelationshipMemory
            | MemorySlot::CurrentPlotMemory
            | MemorySlot::UnresolvedTension
            | MemorySlot::RecentEmotionalState
            | MemorySlot::CharacterIdentityMemory => spec
                .active_soul_ids
                .first()
                .cloned()
                .unwrap_or_else(|| "session_world".into()),
            _ => spec
                .active_soul_ids
                .first()
                .cloned()
                .unwrap_or_else(|| "session_world".into()),
        };
    }
    if row.slot == Some(MemorySlot::WorldLocationMemory) {
        row.owner_soul_id = "session_world".into();
    }
    row.selected_tags = row
        .selected_tags
        .iter()
        .filter_map(|tag| {
            let normalized = normalize_token(tag);
            match normalized.as_str() {
                "sceneevent" | "scene_event" => Some("scene_event".to_string()),
                "relationship" => Some("relationship".to_string()),
                "currentplot" | "current_plot" => Some("current_plot".to_string()),
                "location" => Some("location".to_string()),
                "object" => Some("object".to_string()),
                "emotionalstate" | "emotional_state" => Some("emotional_state".to_string()),
                "boundary" => Some("boundary".to_string()),
                "doorway" => Some("doorway".to_string()),
                "reunion" => Some("reunion".to_string()),
                _ => None,
            }
        })
        .collect();
}

fn validate_event_row(
    row: &EventRow,
    spec: &EvalFormSpec,
    allowed_entities: &HashSet<&str>,
    rejected: &mut Vec<EvalFormRowRejection>,
) -> bool {
    let row_id = row.event_id.clone();
    if clean(&row.event_id).is_none() || clean(&row.objective_summary).is_none() {
        return reject_row(
            rejected,
            "event",
            &row_id,
            "event_id and objective_summary are required",
        );
    }
    if clean(&row.evidence_quote).is_none() {
        return reject_row(rejected, "event", &row_id, "evidence_quote is required");
    }
    if !row
        .event_type
        .is_some_and(|event_type| spec.allowed_event_types.contains(&event_type))
    {
        return reject_row(rejected, "event", &row_id, "event_type is not allowed");
    }
    for participant in &row.participants {
        if !allowed_entities.contains(participant.as_str()) {
            return reject_row(rejected, "event", &row_id, "unknown participant entity id");
        }
    }
    true
}

fn validate_object_row(
    row: &ObjectRow,
    _spec: &EvalFormSpec,
    event_ids: &HashSet<&str>,
    rejected: &mut Vec<EvalFormRowRejection>,
) -> bool {
    let row_id = format!("{}:{}", row.linked_event_id, row.property_changed);
    if !event_ids.contains(row.linked_event_id.as_str()) {
        return reject_row(rejected, "object", &row_id, "linked_event_id is unknown");
    }
    if clean(&row.evidence_quote).is_none() {
        return reject_row(rejected, "object", &row_id, "evidence_quote is required");
    }
    if clean(&row.property_changed).is_none() || clean(&row.new_value).is_none() {
        return reject_row(
            rejected,
            "object",
            &row_id,
            "property_changed and new_value are required",
        );
    }
    if row.object_id.as_deref().and_then(clean).is_none()
        && row.new_object_label.as_deref().and_then(clean).is_none()
    {
        return reject_row(
            rejected,
            "object",
            &row_id,
            "object_id or new_object_label is required",
        );
    }
    true
}

fn validate_relationship_row(
    row: &RelationshipRow,
    spec: &EvalFormSpec,
    allowed_entities: &HashSet<&str>,
    event_ids: &HashSet<&str>,
    rejected: &mut Vec<EvalFormRowRejection>,
) -> bool {
    let row_id = format!(
        "{}:{}:{}",
        row.linked_event_id, row.source_soul_id, row.target_entity_id
    );
    if !event_ids.contains(row.linked_event_id.as_str()) {
        return reject_row(
            rejected,
            "relationship",
            &row_id,
            "linked_event_id is unknown",
        );
    }
    if clean(&row.evidence_quote).is_none() {
        return reject_row(
            rejected,
            "relationship",
            &row_id,
            "evidence_quote is required",
        );
    }
    if !spec
        .active_soul_ids
        .iter()
        .any(|id| id == &row.source_soul_id)
    {
        return reject_row(
            rejected,
            "relationship",
            &row_id,
            "source_soul_id is not an active Soul",
        );
    }
    if !allowed_entities.contains(row.target_entity_id.as_str()) {
        return reject_row(
            rejected,
            "relationship",
            &row_id,
            "unknown target_entity_id",
        );
    }
    if !row
        .dimension
        .is_some_and(|dimension| spec.allowed_relationship_dimensions.contains(&dimension))
    {
        return reject_row(
            rejected,
            "relationship",
            &row_id,
            "relationship dimension is not allowed",
        );
    }
    if row.direction.is_none() {
        return reject_row(
            rejected,
            "relationship",
            &row_id,
            "direction_missing_uncertain",
        );
    }
    true
}

fn validate_memory_row(
    row: &MemoryRow,
    spec: &EvalFormSpec,
    event_ids: &HashSet<&str>,
    rejected: &mut Vec<EvalFormRowRejection>,
) -> bool {
    let row_id = memory_candidate_id(row);
    if !event_ids.contains(row.linked_event_id.as_str()) {
        return reject_row(rejected, "memory", &row_id, "linked_event_id is unknown");
    }
    if clean(&row.evidence_quote).is_none() {
        return reject_row(rejected, "memory", &row_id, "evidence_quote is required");
    }
    if clean(&row.content).is_none() {
        return reject_row(rejected, "memory", &row_id, "content is required");
    }
    if row.owner_soul_id != "session_world"
        && !spec
            .active_soul_ids
            .iter()
            .any(|id| id == &row.owner_soul_id)
    {
        return reject_row(
            rejected,
            "memory",
            &row_id,
            "owner_soul_id is neither active Soul nor session_world",
        );
    }
    if !row
        .slot
        .is_some_and(|slot| spec.allowed_memory_slots.contains(&slot))
    {
        return reject_row(rejected, "memory", &row_id, "memory slot is not allowed");
    }
    for tag in &row.selected_tags {
        if !spec
            .allowed_tag_vocabularies
            .iter()
            .any(|allowed| allowed == tag)
        {
            return reject_row(rejected, "memory", &row_id, "selected tag is not allowed");
        }
    }
    true
}

fn validate_review_row(
    row: &ReviewRow,
    spec: &EvalFormSpec,
    rejected: &mut Vec<EvalFormRowRejection>,
) -> bool {
    let row_id = row.candidate_id.clone();
    if clean(&row.candidate_id).is_none() {
        return reject_row(rejected, "review", &row_id, "candidate_id is required");
    }
    if clean(&row.evidence_quote).is_none() {
        // Do not let review row rejection poison the whole evaluator result.
        // Ignore review_rows without evidence_quote as advisory only.
        return true;
    }
    if matches!(
        row.decision,
        Some(
            ReviewDecision::DuplicateOfExisting
                | ReviewDecision::UpdateExisting
                | ReviewDecision::SupersedeExisting
                | ReviewDecision::ContradictsExisting
        )
    ) {
        let Some(existing_id) = row.existing_id.as_deref().and_then(clean) else {
            return reject_row(
                rejected,
                "review",
                &row_id,
                "existing_id is required for this decision",
            );
        };
        if !existing_id_allowed(spec, existing_id) {
            return reject_row(
                rejected,
                "review",
                &row_id,
                "existing_id is not in form spec",
            );
        }
    }
    true
}

fn reject_row(
    rejected: &mut Vec<EvalFormRowRejection>,
    row_kind: &str,
    row_id: &str,
    reason: &str,
) -> bool {
    rejected.push(EvalFormRowRejection {
        row_kind: row_kind.into(),
        row_id: row_id.into(),
        reason: reason.into(),
    });
    false
}

fn relationship_from_row(row: &RelationshipRow) -> RelationshipEvaluation {
    let magnitude = if let Some(ref shift_str) = row.shift {
        let clean_shift = shift_str.trim().trim_start_matches('+');
        clean_shift.parse::<f32>().unwrap_or_else(|_| {
            magnitude_value(
                row.direction.unwrap_or(RelationshipDirection::NoChange),
                row.magnitude_tier.unwrap_or(MagnitudeTier::Small),
            )
        })
    } else {
        magnitude_value(
            row.direction.unwrap_or(RelationshipDirection::NoChange),
            row.magnitude_tier.unwrap_or(MagnitudeTier::Small),
        )
    };
    let mut relation = RelationshipEvaluation {
        source_soul_id: row.source_soul_id.clone(),
        target_entity_id: row.target_entity_id.clone(),
        evidence_quote: Some(row.evidence_quote.clone()),
        criterion_met: row.direction != Some(RelationshipDirection::NoChange),
        confidence: 0.75,
        evidence_validated_by_form: true,
        ..RelationshipEvaluation::default()
    };
    match row.dimension.unwrap_or(RelationshipDimension::Trust) {
        RelationshipDimension::Trust => relation.trust = Some(magnitude),
        RelationshipDimension::Affection => relation.affection = Some(magnitude),
        RelationshipDimension::Intimacy => relation.intimacy = Some(magnitude),
        RelationshipDimension::Passion => relation.passion = Some(magnitude),
        RelationshipDimension::Commitment => relation.commitment = Some(magnitude),
        RelationshipDimension::Fear => relation.fear = Some(magnitude),
        RelationshipDimension::Desire => relation.desire = Some(magnitude),
        RelationshipDimension::Respect => relation.respect = Some(magnitude),
        RelationshipDimension::Conflict => relation.conflict = Some(magnitude),
        RelationshipDimension::Dependency => relation.dependency = Some(magnitude),
        RelationshipDimension::Curiosity => relation.curiosity = Some(magnitude),
        RelationshipDimension::Comfort => relation.comfort = Some(magnitude),
        RelationshipDimension::BoundaryPressure => relation.boundary_pressure = Some(magnitude),
    }
    relation
}

fn memory_candidate_from_row(row: &MemoryRow, candidate_id: &str) -> MemoryCandidate {
    let importance = row.importance_tier.unwrap_or(ImportanceTier::Medium);
    MemoryCandidate {
        candidate_id: candidate_id.into(),
        owner_soul_id: row.owner_soul_id.clone(),
        slot: row.slot.unwrap_or(MemorySlot::Unknown),
        content: row.content.clone(),
        evidence_quote: row.evidence_quote.clone(),
        criterion_met: true,
        confidence: confidence_from_importance(importance),
        salience: Some(salience_from_importance(importance)),
        retrieval_strength: Some(retrieval_from_importance(importance)),
        perceived_by_entity_id: Some(row.owner_soul_id.clone()),
        target_entity_ids: vec!["default_player".into()],
        source_type: MemorySourceType::CurrentSession,
        truth_status: TruthStatus::SceneEvent,
        relevance_tags: row.selected_tags.clone(),
        knowledge_scope: crate::evaluator::KnowledgeScope::DirectlyObserved,
    }
}

fn apply_review_memory_operations(
    conversion: &mut EvaluatorConversionReport,
    response: &EvalFormResponse,
    review_map: &HashMap<String, (ReviewDecision, &ReviewRow)>,
) {
    let mut operations = Vec::new();
    for row in &response.memory_rows {
        let candidate_id = memory_candidate_id(row);
        let Some((decision, review)) = review_map.get(&candidate_id).copied() else {
            continue;
        };
        let Some(existing_id) = review.existing_id.as_deref().and_then(clean) else {
            continue;
        };
        let operation = match decision {
            ReviewDecision::UpdateExisting => "update",
            ReviewDecision::SupersedeExisting => "supersede",
            ReviewDecision::ContradictsExisting => "invalidate",
            _ => continue,
        };
        operations.push(MemoryPatch {
            operation: Some(operation.into()),
            memory_id: Some(stable_id("memory_form", &candidate_id)),
            target_memory_id: Some(existing_id.to_string()),
            supersedes_memory_id: Some(existing_id.to_string()),
            content: row.content.clone(),
            tag: row.slot.map(|slot| slot.as_label().to_string()),
            ..MemoryPatch::default()
        });
    }
    if operations.is_empty() {
        return;
    }
    let mut patch = conversion.patch.clone();
    patch.schema_version = Some(PATCH_PROTOCOL_VERSION);
    let soul_patch = patch.soul_patch.get_or_insert_with(Default::default);
    soul_patch.memory_operations.extend(operations);
    conversion.patch = patch;
    conversion.no_op = false;
}

fn scene_state_from_event(
    row: &EventRow,
    context: &EvaluatorConversionContext<'_>,
) -> SceneStatePatch {
    SceneStatePatch {
        scene_state_id: Some(stable_id("scene_form", &row.event_id)),
        current_scene: clean(&row.objective_summary).map(str::to_string),
        focus: Some(row.participants.join(" and ")),
        participants: row.participants.clone(),
        last_user_action: clean(context.latest_user_message).map(str::to_string),
        continuity_note: clean(&row.objective_summary).map(str::to_string),
        ..SceneStatePatch::default()
    }
}

fn apply_event_flags(output: &mut EvaluatorOutputV1, row: &EventRow) {
    output.turn_flags_u64 |= turn_flags::SCENE_TURN | turn_flags::USER_ACTION_PRESENT;
    match row.event_type.unwrap_or(EventType::SceneEvent) {
        EventType::LocationChange => output.turn_flags_u64 |= turn_flags::WORLD_CHANGE,
        EventType::ObjectChange => {
            output.turn_flags_u64 |= turn_flags::OBJECT_CHANGE | turn_flags::WORLD_CHANGE
        }
        EventType::RelationshipShift => output.turn_flags_u64 |= turn_flags::RELATIONSHIP_SHIFT,
        EventType::CurrentPlotAdvanced => {
            output.turn_flags_u64 |= turn_flags::CURRENT_PLOT_ADVANCED
        }
        EventType::UnresolvedTension => output.turn_flags_u64 |= turn_flags::UNRESOLVED_TENSION,
        EventType::RecentEmotionalState => {
            output.turn_flags_u64 |= turn_flags::RECENT_EMOTIONAL_STATE
        }
        EventType::Correction => output.turn_flags_u64 |= turn_flags::RETCON_OR_CORRECTION,
        EventType::SceneEvent => {}
    }
}

fn compute_turn_flags(output: &EvaluatorOutputV1) -> u64 {
    let mut flags = output.turn_flags_u64;
    if !output.world_changes.is_empty() {
        flags |= turn_flags::SCENE_TURN | turn_flags::WORLD_CHANGE;
    }
    if !output.object_changes.is_empty() {
        flags |= turn_flags::OBJECT_CHANGE;
    }
    if !output.relationship_evaluations.is_empty() {
        flags |= turn_flags::RELATIONSHIP_SHIFT;
    }
    if output
        .memory_candidates
        .iter()
        .any(|candidate| candidate.slot == MemorySlot::UnresolvedTension)
    {
        flags |= turn_flags::UNRESOLVED_TENSION;
    }
    flags
}

fn global_scene_from_output(output: &EvaluatorOutputV1) -> GlobalSceneEvaluation {
    GlobalSceneEvaluation {
        scene_event_occurred: output.turn_flags_u64 & turn_flags::SCENE_TURN != 0,
        location_changed: output.world_changes.iter().any(|change| {
            change
                .location
                .as_ref()
                .is_some_and(|location| !location.trim().is_empty())
        }),
        object_state_changed: !output.object_changes.is_empty(),
        relationship_changed: !output.relationship_evaluations.is_empty(),
        unresolved_tension: output.turn_flags_u64 & turn_flags::UNRESOLVED_TENSION != 0,
        current_plot_advanced: output.turn_flags_u64 & turn_flags::CURRENT_PLOT_ADVANCED != 0,
        recent_emotional_state_changed: output.turn_flags_u64 & turn_flags::RECENT_EMOTIONAL_STATE
            != 0,
        evidence_quote: output
            .world_changes
            .first()
            .and_then(|change| change.evidence_quote.clone()),
        summary: output
            .world_changes
            .first()
            .and_then(|change| change.event_summary.clone())
            .unwrap_or_default(),
        ..GlobalSceneEvaluation::default()
    }
}

fn draft_from_output(
    output: &EvaluatorOutputV1,
    rejected_rows: &[EvalFormRowRejection],
    trace: &EvalFormTrace,
) -> NormalizedEvaluationDraft {
    NormalizedEvaluationDraft {
        scene_evaluation: output.global_scene_evaluation.clone(),
        memory_candidate_count: output.memory_candidates.len(),
        world_event_count: output.world_changes.len(),
        scene_state_present: output
            .world_changes
            .iter()
            .any(|change| change.scene_state.is_some()),
        relationship_delta_count: output.relationship_evaluations.len(),
        object_observation_count: output.object_changes.len(),
        warnings: rejected_rows
            .iter()
            .map(|row| format!("{} {} rejected: {}", row.row_kind, row.row_id, row.reason))
            .collect(),
        candidate_quality_decisions: rejected_rows.iter().map(|row| row.reason.clone()).collect(),
        candidate_routing_decisions: trace
            .form_dedupe_decisions
            .iter()
            .map(|decision| format!("{} {:?}", decision.candidate_id, decision.decision))
            .collect(),
        state_effect_guarantee_applied: false,
        state_effect_guarantee_reason: None,
        per_soul_interpretation_count: 0,
    }
}

fn aggregate_relevance_tags(output: &EvaluatorOutputV1) -> RelevanceTags {
    let mut tags = RelevanceTags::default();
    for change in &output.world_changes {
        tags.event_type_tags
            .extend(change.relevance_tags.event_type_tags.clone());
    }
    for candidate in &output.memory_candidates {
        for tag in &candidate.relevance_tags {
            tags.memory_slot_tags.insert(tag.clone(), 80);
        }
        tags.memory_slot_tags
            .insert(candidate.slot.as_label().into(), 80);
    }
    tags
}

fn relevance_from_event(row: &EventRow) -> RelevanceTags {
    let mut tags = RelevanceTags::default();
    tags.event_type_tags.insert(
        format!("{:?}", row.event_type.unwrap_or(EventType::SceneEvent)).to_ascii_lowercase(),
        80,
    );
    tags
}

fn flatten_tag_weights(tags: &RelevanceTags) -> HashMap<String, u8> {
    tags.setting_tags
        .iter()
        .chain(tags.location_tags.iter())
        .chain(tags.interacted_entities.iter())
        .chain(tags.event_type_tags.iter())
        .chain(tags.object_tags.iter())
        .chain(tags.emotional_tags.iter())
        .chain(tags.memory_slot_tags.iter())
        .map(|(key, value)| (key.clone(), *value))
        .collect()
}

fn existing_id_allowed(spec: &EvalFormSpec, id: &str) -> bool {
    spec.existing_memories
        .iter()
        .chain(spec.existing_events.iter())
        .chain(spec.existing_object_observations.iter())
        .chain(spec.existing_relationship_facts.iter())
        .any(|row| row.existing_id == id)
}

fn select_relevant_memories(
    soul: &Soul,
    latest_user_message: &str,
    latest_narrator_response: &str,
    top_k: usize,
) -> Vec<ExistingStateRow> {
    let query = token_set(&format!("{latest_user_message} {latest_narrator_response}"));
    let mut rows = soul
        .memory
        .recent
        .iter()
        .map(|memory| {
            let overlap = token_set(&memory.content)
                .iter()
                .filter(|token| query.contains(token))
                .count();
            (overlap, memory)
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| right.0.cmp(&left.0));
    rows.into_iter()
        .take(top_k)
        .map(|(_, memory)| ExistingStateRow {
            existing_id: memory.id.clone(),
            kind: ExistingStateKind::Memory,
            summary: memory.content.clone(),
        })
        .collect()
}

fn select_relevant_events(
    recent_events: &[String],
    records: &[crate::soul::WorldEventRecord],
    top_k: usize,
) -> Vec<ExistingStateRow> {
    if !records.is_empty() {
        return records
            .iter()
            .rev()
            .take(top_k)
            .map(|record| ExistingStateRow {
                existing_id: record.recent_event_id.clone(),
                kind: ExistingStateKind::Event,
                summary: record.content.clone(),
            })
            .collect();
    }
    recent_events
        .iter()
        .rev()
        .take(top_k)
        .enumerate()
        .map(|(idx, event)| ExistingStateRow {
            existing_id: format!("recent_event_{idx}"),
            kind: ExistingStateKind::Event,
            summary: event.clone(),
        })
        .collect()
}

fn all_event_types() -> Vec<EventType> {
    vec![
        EventType::SceneEvent,
        EventType::LocationChange,
        EventType::ObjectChange,
        EventType::RelationshipShift,
        EventType::CurrentPlotAdvanced,
        EventType::UnresolvedTension,
        EventType::RecentEmotionalState,
        EventType::Correction,
    ]
}

fn all_relationship_dimensions() -> Vec<RelationshipDimension> {
    vec![
        RelationshipDimension::Trust,
        RelationshipDimension::Affection,
        RelationshipDimension::Intimacy,
        RelationshipDimension::Passion,
        RelationshipDimension::Commitment,
        RelationshipDimension::Fear,
        RelationshipDimension::Desire,
        RelationshipDimension::Respect,
        RelationshipDimension::Conflict,
        RelationshipDimension::Dependency,
        RelationshipDimension::Curiosity,
        RelationshipDimension::Comfort,
        RelationshipDimension::BoundaryPressure,
    ]
}

fn default_tag_vocabularies() -> Vec<String> {
    [
        "scene_event",
        "relationship",
        "current_plot",
        "location",
        "object",
        "emotional_state",
        "boundary",
        "doorway",
        "reunion",
    ]
    .iter()
    .map(|tag| (*tag).to_string())
    .collect()
}

fn memory_candidate_id(row: &MemoryRow) -> String {
    stable_id(
        "form_memory",
        &format!(
            "{}|{}|{}|{}",
            row.linked_event_id,
            row.owner_soul_id,
            row.slot.map(|slot| slot.as_label()).unwrap_or("unknown"),
            row.content
        ),
    )
}

fn stable_id(prefix: &str, source: &str) -> String {
    let mut hash: u64 = 1469598103934665603;
    for byte in source.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{prefix}_{hash:016x}")
}

fn magnitude_value(direction: RelationshipDirection, tier: MagnitudeTier) -> f32 {
    let amount = match tier {
        MagnitudeTier::Tiny => 0.5,
        MagnitudeTier::Small => 1.0,
        MagnitudeTier::Medium => 2.0,
        MagnitudeTier::Large => 4.0,
    };
    match direction {
        RelationshipDirection::Increase => amount,
        RelationshipDirection::Decrease => -amount,
        RelationshipDirection::NoChange => 0.0,
    }
}

fn confidence_from_importance(tier: ImportanceTier) -> f32 {
    match tier {
        ImportanceTier::Trivial => 0.45,
        ImportanceTier::Low => 0.6,
        ImportanceTier::Medium => 0.75,
        ImportanceTier::High => 0.88,
        ImportanceTier::Critical => 0.95,
    }
}

fn confidence_from_confidence_tier(tier: ConfidenceTier) -> f32 {
    match tier {
        ConfidenceTier::Low => 0.5,
        ConfidenceTier::Medium => 0.72,
        ConfidenceTier::High => 0.9,
    }
}

fn salience_from_importance(tier: ImportanceTier) -> f32 {
    match tier {
        ImportanceTier::Trivial => 20.0,
        ImportanceTier::Low => 40.0,
        ImportanceTier::Medium => 60.0,
        ImportanceTier::High => 82.0,
        ImportanceTier::Critical => 95.0,
    }
}

fn retrieval_from_importance(tier: ImportanceTier) -> f32 {
    match tier {
        ImportanceTier::Trivial => 15.0,
        ImportanceTier::Low => 35.0,
        ImportanceTier::Medium => 55.0,
        ImportanceTier::High => 78.0,
        ImportanceTier::Critical => 92.0,
    }
}

fn decay_profile(tier: ImportanceTier) -> &'static str {
    match tier {
        ImportanceTier::Trivial => "fast",
        ImportanceTier::Low => "normal",
        ImportanceTier::Medium => "normal",
        ImportanceTier::High => "slow",
        ImportanceTier::Critical => "pinned",
    }
}

fn infer_object_kind(object_id: &str) -> String {
    if object_id.contains("door") {
        "door".into()
    } else if object_id.contains("phone") {
        "phone".into()
    } else {
        "unknown".into()
    }
}

fn normalize_player_id(value: &str) -> String {
    if value == "user" {
        "default_player".into()
    } else {
        value.to_string()
    }
}

fn token_set(text: &str) -> Vec<String> {
    let mut tokens = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() > 2)
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

fn slugify(label: &str) -> String {
    label
        .trim()
        .to_ascii_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn clean(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn resolve_active_entity_id(raw_id: &str, spec: &EvalFormSpec) -> String {
    let clean_raw = raw_id.trim();
    if clean_raw.is_empty() {
        return clean_raw.to_string();
    }
    
    let normalized_raw = normalize_token(clean_raw);
    
    for entity in &spec.active_entities {
        if entity.entity_id == clean_raw {
            return entity.entity_id.clone();
        }
        
        if normalize_token(&entity.display_name) == normalized_raw {
            return entity.entity_id.clone();
        }
        
        if normalize_token(&entity.entity_id) == normalized_raw {
            return entity.entity_id.clone();
        }
    }
    
    if normalized_raw == "user" || normalized_raw == "default_player" || normalized_raw == "player" {
        return "default_player".to_string();
    }
    
    clean_raw.to_string()
}

pub fn format_honest_ui_status(
    patch_applied: bool,
    materialized_soul_updated: bool,
    materialized_session_world_updated: bool,
    rejected_rows: &[EvalFormRowRejection],
) -> String {
    let was_applied = patch_applied && materialized_soul_updated && materialized_session_world_updated;
    let rows_rejected = rejected_rows.len();

    if was_applied {
        if rows_rejected == 0 {
            "State updated".to_string()
        } else {
            let mut object_count = 0;
            let mut relationship_count = 0;
            let mut memory_count = 0;
            let mut event_count = 0;
            let mut review_count = 0;

            for r in rejected_rows {
                match r.row_kind.as_str() {
                    "object" => object_count += 1,
                    "relationship" => relationship_count += 1,
                    "memory" => memory_count += 1,
                    "event" => event_count += 1,
                    "review" => review_count += 1,
                    _ => {}
                }
            }

            let mut kinds = Vec::new();
            if object_count > 0 {
                kinds.push(format!("{} object row{}", object_count, if object_count == 1 { "" } else { "s" }));
            }
            if relationship_count > 0 {
                kinds.push(format!("{} relationship row{}", relationship_count, if relationship_count == 1 { "" } else { "s" }));
            }
            if memory_count > 0 {
                kinds.push(format!("{} memory row{}", memory_count, if memory_count == 1 { "" } else { "s" }));
            }
            if event_count > 0 {
                kinds.push(format!("{} event row{}", event_count, if event_count == 1 { "" } else { "s" }));
            }
            if review_count > 0 {
                kinds.push(format!("{} review row{}", review_count, if review_count == 1 { "" } else { "s" }));
            }

            if kinds.len() == 1 {
                format!("State updated; {} skipped", kinds[0])
            } else {
                format!("State updated; {} evaluator rows skipped", rows_rejected)
            }
        }
    } else {
        "State update failed".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        setting::session_world_from_legacy_world,
        soul::{new_default_soul, MemoryEntry},
    };

    fn soul_and_world() -> (Soul, SessionWorld) {
        let mut soul = new_default_soul("Aurora");
        soul.character_id = "aurora_soul".into();
        soul.memory.recent.push(MemoryEntry {
            id: "mem_existing".into(),
            timestamp: 1,
            content: "Aurora remembers that the visitor knocked before entering.".into(),
            salience: 70.0,
            tag: "current_plot_memory".into(),
            retrieval_strength: 70.0,
            source_type: MemorySourceType::CurrentSession,
            source_session_id: None,
            source_conversation_id: None,
            source_message_id: None,
            source_entity_id: None,
            is_lived_experience: true,
            is_imported_context: false,
            perceived_by_entity_id: Some("aurora_soul".into()),
            target_entity_ids: vec!["default_player".into()],
            interpretation: None,
            confidence: Some(0.8),
            objective_event_id: None,
            truth_status: TruthStatus::SceneEvent,
            architecture_verified: false,
            memory_slot: Some("current_plot_memory".into()),
            owner_soul_id: Some("aurora_soul".into()),
            relevance_tags: HashMap::new(),
            knowledge_scope: Some("directly_observed".into()),
            is_active: true,
            invalidated_by_patch_id: None,
            superseded_by_memory_id: None,
            is_retconned: false,
        });
        soul.world.object_states.push(ObjectState {
            object_id: "apartment_door".into(),
            object_kind: "door".into(),
            status: "closed".into(),
            last_observed_state: "closed".into(),
            confidence: 0.9,
            ..ObjectState::default()
        });
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        (soul, world)
    }

    fn spec_and_context<'a>(
        soul: &'a Soul,
        world: &'a SessionWorld,
        user: &'a str,
        narrator: &'a str,
    ) -> (EvalFormSpec, EvaluatorConversionContext<'a>) {
        (
            build_eval_form_spec(soul, Some(world), user, narrator, 8),
            EvaluatorConversionContext {
                active_soul_id: &soul.character_id,
                active_soul_ids: vec![soul.character_id.clone()],
                latest_user_message: user,
                latest_narrator_response: narrator,
                session_world: Some(world),
                baseline_recent_event_id: None,
            },
        )
    }

    fn event(id: &str, summary: &str, quote: &str) -> EventRow {
        EventRow {
            event_id: id.into(),
            event_type: Some(EventType::SceneEvent),
            objective_summary: summary.into(),
            participants: vec!["aurora_soul".into(), "default_player".into()],
            evidence_quote: quote.into(),
            importance_tier: Some(ImportanceTier::Medium),
            ..EventRow::default()
        }
    }

    fn memory(event_id: &str, content: &str, quote: &str) -> MemoryRow {
        MemoryRow {
            linked_event_id: event_id.into(),
            owner_soul_id: "aurora_soul".into(),
            slot: Some(MemorySlot::CurrentPlotMemory),
            content: content.into(),
            evidence_quote: quote.into(),
            importance_tier: Some(ImportanceTier::High),
            retrieval_cues: vec!["entry".into()],
            selected_tags: vec!["current_plot".into()],
            ..MemoryRow::default()
        }
    }

    #[test]
    fn form_supports_multiple_events_in_one_turn() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "I walk in and close the door.",
            "The visitor walks in and closes the door.",
        );
        let response = EvalFormResponse {
            event_rows: vec![
                event(
                    "entry",
                    "The visitor entered Aurora's apartment.",
                    "walks in",
                ),
                event("close", "The visitor closed the door.", "closes the door"),
            ],
            ..EvalFormResponse::default()
        };
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.output.world_changes.len(), 2);
        assert_eq!(result.trace.form_rows_rejected, 0);
    }

    #[test]
    fn form_rejects_unknown_entity_id() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "I enter.", "The visitor enters.");
        let mut bad = event("entry", "A stranger enters.", "enters");
        bad.participants.push("mystery_entity".into());
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![bad],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert!(result
            .rejected_rows
            .iter()
            .any(|row| row.reason.contains("unknown participant")));
    }

    #[test]
    fn form_requires_evidence_for_non_empty_rows() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "I enter.", "The visitor enters.");
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event("entry", "The visitor entered.", "")],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert!(result
            .rejected_rows
            .iter()
            .any(|row| row.reason == "evidence_quote is required"));
    }

    #[test]
    fn form_dedupe_marks_duplicate_of_existing() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "I walk in.",
            "The visitor walks in after knocking.",
        );
        let memory = memory(
            "entry",
            "Aurora remembers that the visitor knocked before entering.",
            "walks in",
        );
        let candidate_id = memory_candidate_id(&memory);
        let response = EvalFormResponse {
            event_rows: vec![event("entry", "The visitor entered.", "walks in")],
            memory_rows: vec![memory],
            review_rows: vec![ReviewRow {
                candidate_id: candidate_id.clone(),
                decision: Some(ReviewDecision::DuplicateOfExisting),
                existing_id: Some("mem_existing".into()),
                reason: "same remembered beat".into(),
                evidence_quote: "walks in".into(),
                ..ReviewRow::default()
            }],
            ..EvalFormResponse::default()
        };
        let result = compile_eval_form_response(&spec, &response, &context);
        assert!(result.output.memory_candidates.is_empty());
        assert_eq!(
            result.trace.form_dedupe_decisions[0].candidate_id,
            candidate_id
        );
    }

    #[test]
    fn form_dedupe_marks_update_existing() {
        let (soul, world) = soul_and_world();
        let (spec, context) =
            spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
        let memory = memory(
            "entry",
            "Aurora updates the entry beat with the visitor inside.",
            "walks in",
        );
        let candidate_id = memory_candidate_id(&memory);
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event("entry", "The visitor entered.", "walks in")],
                memory_rows: vec![memory],
                review_rows: vec![ReviewRow {
                    candidate_id,
                    decision: Some(ReviewDecision::UpdateExisting),
                    existing_id: Some("mem_existing".into()),
                    reason: "more current version".into(),
                    evidence_quote: "walks in".into(),
                    ..ReviewRow::default()
                }],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert!(result
            .conversion
            .patch
            .soul_patch
            .as_ref()
            .unwrap()
            .memory_operations
            .iter()
            .any(|operation| operation.operation.as_deref() == Some("update")));
    }

    #[test]
    fn form_memory_row_compiles_to_normalized_draft() {
        let (soul, world) = soul_and_world();
        let (spec, context) =
            spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event("entry", "The visitor entered.", "walks in")],
                memory_rows: vec![memory(
                    "entry",
                    "Aurora remembers the visitor came inside.",
                    "walks in",
                )],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert_eq!(result.draft.memory_candidate_count, 1);
        assert_eq!(result.output.memory_candidates.len(), 1);
    }

    #[test]
    fn form_relationship_row_compiles_to_delta() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "Long time no see.",
            "Aurora warms at the greeting.",
        );
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event(
                    "greeting",
                    "The visitor greeted Aurora.",
                    "Long time no see",
                )],
                relationship_rows: vec![RelationshipRow {
                    linked_event_id: "greeting".into(),
                    source_soul_id: "aurora_soul".into(),
                    target_entity_id: "default_player".into(),
                    dimension: Some(RelationshipDimension::Comfort),
                    direction: Some(RelationshipDirection::Increase),
                    magnitude_tier: Some(MagnitudeTier::Small),
                    evidence_quote: "Long time no see".into(),
                    ..RelationshipRow::default()
                }],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert_eq!(result.output.relationship_evaluations[0].comfort, Some(1.0));
    }

    #[test]
    fn form_object_row_compiles_to_object_observation() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "I close the door.",
            "The apartment door clicks closed.",
        );
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event("door", "The door closed.", "door clicks closed")],
                object_rows: vec![ObjectRow {
                    linked_event_id: "door".into(),
                    object_id: Some("apartment_door".into()),
                    property_changed: "open_state".into(),
                    old_value: Some("open".into()),
                    new_value: "closed".into(),
                    evidence_quote: "door clicks closed".into(),
                    confidence_tier: Some(ConfidenceTier::High),
                    ..ObjectRow::default()
                }],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert_eq!(
            result.output.object_changes[0].object_state.object_id,
            "apartment_door"
        );
        assert!(result
            .conversion
            .patch
            .world_patch
            .as_ref()
            .unwrap()
            .object_observation_operations
            .iter()
            .any(|operation| operation.operation == "update_object_state"));
    }

    #[test]
    fn code_computes_turn_flags_not_llm() {
        let (soul, world) = soul_and_world();
        let (spec, context) =
            spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event("entry", "The visitor entered.", "walks in")],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert_ne!(result.trace.compiled_turn_flags_u64, 0);
        assert_ne!(result.output.turn_flags_u64 & turn_flags::SCENE_TURN, 0);
    }

    #[test]
    fn code_assigns_decay_profile_not_llm() {
        let (soul, world) = soul_and_world();
        let (spec, context) =
            spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
        let memory = memory(
            "entry",
            "Aurora remembers the visitor came inside.",
            "walks in",
        );
        let candidate_id = memory_candidate_id(&memory);
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event("entry", "The visitor entered.", "walks in")],
                memory_rows: vec![memory],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert_eq!(
            result
                .trace
                .code_assigned_decay_profile
                .get(&candidate_id)
                .map(String::as_str),
            Some("slow")
        );
    }

    #[test]
    fn form_path_door_entry_creates_scene_state_or_recent_event() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "I walk in. Long time no see, Aurora.",
            "The visitor walks into Aurora's apartment.",
        );
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event(
                    "entry",
                    "The visitor entered Aurora's apartment.",
                    "walks into Aurora's apartment",
                )],
                ..EvalFormResponse::default()
            },
            &context,
        );
        let world_patch = result.conversion.patch.world_patch.as_ref().unwrap();
        assert!(world_patch.scene_state.is_some() || !world_patch.event_operations.is_empty());
    }

    #[test]
    fn form_path_can_review_existing_memory_before_writing_duplicate() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "I walk in.",
            "The visitor walks in after knocking.",
        );
        assert_eq!(spec.existing_memories.len(), 1);
        let memory = memory(
            "entry",
            "Aurora remembers that the visitor knocked before entering.",
            "walks in",
        );
        let candidate_id = memory_candidate_id(&memory);
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event("entry", "The visitor entered.", "walks in")],
                memory_rows: vec![memory],
                review_rows: vec![ReviewRow {
                    candidate_id,
                    decision: Some(ReviewDecision::DuplicateOfExisting),
                    existing_id: Some("mem_existing".into()),
                    reason: "already captured".into(),
                    evidence_quote: "walks in".into(),
                    ..ReviewRow::default()
                }],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert!(result.conversion.patch.soul_patch.is_none());
        assert_eq!(
            result.trace.form_dedupe_decisions[0].decision,
            ReviewDecision::DuplicateOfExisting
        );
    }

    #[test]
    fn form_accepts_summary_alias_for_objective_summary() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "I knock.", "A knock sounds.");
        let response = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "knock",
                    "event_type": "scene_event",
                    "summary": "The visitor knocked at Aurora's door.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "I knock."
                }]
            }"#,
        )
        .expect("parse aliases");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(
            result.output.world_changes[0].event_summary.as_deref(),
            Some("The visitor knocked at Aurora's door.")
        );
    }

    #[test]
    fn form_accepts_event_id_alias_for_linked_event_id() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "Long time no see.",
            "Aurora relaxes at the familiar greeting.",
        );
        let response = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "greeting",
                    "event_type": "scene_event",
                    "summary": "The visitor greeted Aurora.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "Long time no see."
                }],
                "relationship_rows": [{
                    "event_id": "greeting",
                    "source_soul_id": "aurora_soul",
                    "target_entity_id": "default_player",
                    "dimension": "comfort",
                    "change_direction": "increase",
                    "magnitude_tier": "small",
                    "evidence_quote": "Long time no see."
                }]
            }"#,
        )
        .expect("parse aliases");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.relationship_evaluations[0].comfort, Some(1.0));
    }

    #[test]
    fn form_accepts_slot_id_alias_for_slot() {
        let (soul, world) = soul_and_world();
        let (spec, context) =
            spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
        let response = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "entry",
                    "event_type": "scene_event",
                    "summary": "The visitor entered Aurora's apartment.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "I walk in."
                }],
                "memory_rows": [{
                    "event_id": "entry",
                    "owner_soul_id": "aurora_soul",
                    "slot_id": "current_plot_memory",
                    "content": "Aurora saw the visitor enter.",
                    "evidence_quote": "I walk in.",
                    "importance_tier": "medium",
                    "selected_tags": ["current_plot"]
                }]
            }"#,
        )
        .expect("parse aliases");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(
            result.output.memory_candidates[0].slot,
            MemorySlot::CurrentPlotMemory
        );
    }

    #[test]
    fn form_relationship_id_parses_source_and_target() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "Long time no see.",
            "Aurora relaxes at the familiar greeting.",
        );
        let response = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "greeting",
                    "event_type": "scene_event",
                    "summary": "The visitor greeted Aurora.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "Long time no see."
                }],
                "relationship_rows": [{
                    "event_id": "greeting",
                    "relationship_id": "rel:aurora_soul:default_player",
                    "dimension": "comfort",
                    "change_direction": "increase",
                    "magnitude_tier": "small",
                    "evidence_quote": "Long time no see."
                }]
            }"#,
        )
        .expect("parse relationship id");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(
            result.output.relationship_evaluations[0].source_soul_id,
            "aurora_soul"
        );
        assert_eq!(
            result.output.relationship_evaluations[0].target_entity_id,
            "default_player"
        );
    }

    #[test]
    fn form_memory_content_can_derive_from_linked_event() {
        let (soul, world) = soul_and_world();
        let (spec, context) =
            spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
        let response = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "entry",
                    "event_type": "scene_event",
                    "summary": "The visitor entered Aurora's apartment.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "I walk in."
                }],
                "memory_rows": [{
                    "event_id": "entry",
                    "owner_soul_id": "aurora_soul",
                    "slot_id": "current_plot_memory",
                    "evidence_quote": "I walk in.",
                    "importance_tier": "medium",
                    "selected_tags": ["current_plot"]
                }]
            }"#,
        )
        .expect("parse memory aliases");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert!(result.output.memory_candidates[0]
            .content
            .contains("I walk in."));
    }

    #[test]
    fn form_object_row_accepts_summary_and_change_aliases() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "I open the door.",
            "The apartment door opens.",
        );
        let response = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "door_opened",
                    "event_type": "object_change",
                    "summary": "The apartment door opened.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "I open the door."
                }],
                "object_rows": [{
                    "event_id": "door_opened",
                    "object_id": "apartment_door",
                    "change": "open_state",
                    "summary": "open",
                    "evidence_quote": "I open the door.",
                    "confidence_tier": "medium"
                }]
            }"#,
        )
        .expect("parse object aliases");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(
            result.output.object_changes[0]
                .object_state
                .last_observed_state,
            "open_state: open"
        );
    }

    #[test]
    fn strips_markdown_json_fence_before_form_parse() {
        let parsed = parse_eval_form_response(
            r#"```json
            {"event_rows":[{"event_id":"entry","event_type":"scene_event","summary":"Entry.","participants":["aurora_soul"],"evidence_quote":"I enter."}]}
            ```"#,
        )
        .expect("fenced json");

        assert_eq!(parsed.event_rows[0].event_id, "entry");
    }

    #[test]
    fn repairs_evidence_quote_string_and_string() {
        let (parsed, trace) = parse_eval_form_response_with_trace(
            r#"{
                "event_rows": [{
                    "event_id": "watchful",
                    "event_type": "scene_event",
                    "summary": "Aurora stays watchful.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "her body a casual barrier" and "her eyes remain watchful"
                }]
            }"#,
        )
        .expect("repair quote and quote");

        assert!(trace.raw_form_repair_applied);
        assert_eq!(
            parsed.event_rows[0].evidence_quote,
            "her body a casual barrier; her eyes remain watchful"
        );
    }

    #[test]
    fn maps_increased_interest_with_undercurrent_to_increase() {
        let parsed = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "greeting",
                    "event_type": "scene_event",
                    "summary": "The visitor greeted Aurora.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "Long time no see."
                }],
                "relationship_rows": [{
                    "event_id": "greeting",
                    "relationship_id": "rel:aurora_soul:default_player",
                    "dimension": "curiosity",
                    "direction": "increased_interest_with_undercurrent",
                    "evidence_quote": "Long time no see."
                }]
            }"#,
        )
        .expect("direction drift");

        assert_eq!(
            parsed.relationship_rows[0].direction,
            Some(RelationshipDirection::Increase)
        );
    }

    #[test]
    fn dimensions_changed_array_splits_relationship_rows() {
        let parsed = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "greeting",
                    "event_type": "scene_event",
                    "summary": "The visitor greeted Aurora.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "Long time no see."
                }],
                "relationship_rows": [{
                    "event_id": "greeting",
                    "relationship_id": "rel:aurora_soul:default_player",
                    "dimensions_changed": ["comfort", "curiosity"],
                    "direction": "increased",
                    "evidence_quote": "Long time no see."
                }]
            }"#,
        )
        .expect("dimensions split");

        assert_eq!(parsed.relationship_rows.len(), 2);
        assert_eq!(
            parsed.relationship_rows[0].dimension,
            Some(RelationshipDimension::Comfort)
        );
        assert_eq!(
            parsed.relationship_rows[1].dimension,
            Some(RelationshipDimension::Curiosity)
        );
    }

    #[test]
    fn missing_linked_event_id_uses_single_event() {
        let parsed = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "entry",
                    "event_type": "scene_event",
                    "summary": "The visitor entered.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "I enter."
                }],
                "memory_rows": [{
                    "owner_soul_id": "aurora_soul",
                    "slot_id": "current_plot_memory",
                    "summary": "The visitor entered.",
                    "evidence_quote": "I enter."
                }]
            }"#,
        )
        .expect("single event link");

        assert_eq!(parsed.memory_rows[0].linked_event_id, "entry");
    }

    #[test]
    fn missing_linked_event_id_uses_highest_importance_event() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "I enter.", "The visitor enters.");
        let response = EvalFormResponse {
            event_rows: vec![
                EventRow {
                    event_id: "minor".into(),
                    event_type: Some(EventType::SceneEvent),
                    objective_summary: "A small glance happens.".into(),
                    participants: vec!["aurora_soul".into()],
                    evidence_quote: "glance".into(),
                    importance_tier: Some(ImportanceTier::Low),
                    ..EventRow::default()
                },
                EventRow {
                    event_id: "major".into(),
                    event_type: Some(EventType::SceneEvent),
                    objective_summary: "The visitor enters.".into(),
                    participants: vec!["aurora_soul".into(), "default_player".into()],
                    evidence_quote: "I enter.".into(),
                    importance_tier: Some(ImportanceTier::High),
                    ..EventRow::default()
                },
            ],
            memory_rows: vec![MemoryRow {
                owner_soul_id: "aurora_soul".into(),
                slot: Some(MemorySlot::CurrentPlotMemory),
                content: "The visitor entered.".into(),
                evidence_quote: "I enter.".into(),
                ..MemoryRow::default()
            }],
            ..EvalFormResponse::default()
        };
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(
            result.output.memory_candidates[0]
                .candidate_id
                .contains("major"),
            false
        );
        assert_eq!(result.trace.form_rows_rejected, 0);
    }

    #[test]
    fn memory_summary_becomes_content() {
        let parsed = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "entry",
                    "event_type": "scene_event",
                    "summary": "The visitor entered.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "I enter."
                }],
                "memory_rows": [{
                    "event_id": "entry",
                    "owner_soul_id": "aurora_soul",
                    "slot_id": "current_plot_memory",
                    "summary": "Aurora saw the visitor enter.",
                    "evidence_quote": "I enter."
                }]
            }"#,
        )
        .expect("summary content");

        assert_eq!(
            parsed.memory_rows[0].content,
            "Aurora saw the visitor enter."
        );
    }

    #[test]
    fn memory_id_becomes_candidate_id() {
        let parsed = parse_eval_form_response(
            r#"{
                "review_rows": [{
                    "memory_id": "mem-1",
                    "decision": "new",
                    "reason": "new memory",
                    "evidence_quote": "I enter."
                }]
            }"#,
        )
        .expect("memory id alias");

        assert_eq!(parsed.review_rows[0].candidate_id, "mem-1");
    }

    #[test]
    fn unknown_tags_are_dropped_not_fatal() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "I enter.", "The visitor enters.");
        let response = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "entry",
                    "event_type": "scene_event",
                    "summary": "The visitor entered.",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "I enter."
                }],
                "memory_rows": [{
                    "event_id": "entry",
                    "owner_soul_id": "aurora_soul",
                    "slot_id": "current_plot_memory",
                    "content": "Aurora saw the visitor enter.",
                    "evidence_quote": "I enter.",
                    "selected_tags": ["Scene Event", "very_weird_tag"]
                }]
            }"#,
        )
        .expect("unknown tag");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.memory_candidates.len(), 1);
    }

    #[test]
    fn form_door_knock_accepts_at_least_one_row() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "I knock at the door",
            "A knock sounds at Aurora's apartment door.",
        );
        let response = parse_eval_form_response(
            r#"{
                "event_rows": [{
                    "event_id": "door_knock",
                    "event_type": "scene_event",
                    "summary": "The visitor knocked at Aurora's apartment door.",
                    "timestamp": "now",
                    "participants": ["aurora_soul", "default_player"],
                    "evidence_quote": "I knock at the door",
                    "importance_tier": "medium"
                }],
                "memory_rows": [{
                    "event_id": "door_knock",
                    "owner_soul_id": "session_world",
                    "slot_id": "current_plot_memory",
                    "evidence_quote": "I knock at the door",
                    "importance_tier": "medium",
                    "selected_tags": ["scene_event"]
                }]
            }"#,
        )
        .expect("parse knock");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert!(result.trace.form_rows_accepted > 0);
        assert!(result.draft.world_event_count > 0 || result.draft.scene_state_present);
        assert!(!result.conversion.patch.is_empty());
    }

    #[test]
    fn enrichment_relationship_without_linked_event_uses_baseline_event() {
        let (soul, world) = soul_and_world();
        let spec = build_eval_form_spec(&soul, Some(&world), "I enter.", "The visitor enters.", 8);
        let context = EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: "I enter.",
            latest_narrator_response: "The visitor enters.",
            session_world: Some(&world),
            baseline_recent_event_id: Some("event_baseline_xyz".into()),
        };
        let response = EvalFormResponse {
            relationship_rows: vec![RelationshipRow {
                linked_event_id: "".into(),
                source_soul_id: "aurora_soul".into(),
                target_entity_id: "default_player".into(),
                dimension: Some(RelationshipDimension::Comfort),
                direction: Some(RelationshipDirection::Increase),
                magnitude_tier: Some(MagnitudeTier::Small),
                evidence_quote: "Long time no see".into(),
                ..RelationshipRow::default()
            }],
            ..EvalFormResponse::default()
        };
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(
            result.normalized_response.relationship_rows[0].linked_event_id,
            "event_baseline_xyz"
        );
    }

    #[test]
    fn enrichment_memory_without_linked_event_uses_baseline_event() {
        let (soul, world) = soul_and_world();
        let spec = build_eval_form_spec(&soul, Some(&world), "I enter.", "The visitor enters.", 8);
        let context = EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: "I enter.",
            latest_narrator_response: "The visitor enters.",
            session_world: Some(&world),
            baseline_recent_event_id: Some("event_baseline_xyz".into()),
        };
        let response = EvalFormResponse {
            memory_rows: vec![MemoryRow {
                linked_event_id: "".into(),
                owner_soul_id: "aurora_soul".into(),
                slot: Some(MemorySlot::CurrentPlotMemory),
                content: "Aurora remembers the visitor enters.".into(),
                evidence_quote: "enters".into(),
                ..MemoryRow::default()
            }],
            ..EvalFormResponse::default()
        };
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(
            result.normalized_response.memory_rows[0].linked_event_id,
            "event_baseline_xyz"
        );
    }

    fn soul_aurora() -> Soul {
        let mut soul = new_default_soul("Aurora Schwarz");
        soul.character_id = "e0ee4936-2e71-4ab9-8631-4c22be68ec72".into();
        soul
    }

    fn live_fixture_context<'a>(
        soul: &'a Soul,
        world: &'a SessionWorld,
        user: &'a str,
        narrator: &'a str,
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

    #[test]
    fn relationship_dimension_infers_from_curiosity_tag() {
        let parsed = parse_eval_form_response(
            r#"{
              "event_rows":[{"event_id":"evt","event_type":"scene_event","summary":"Aurora grows curious.","participants":["aurora_soul","default_player"],"evidence_quote":"Long time no see."}],
              "relationship_rows":[{
                "relationship_id":"rel:aurora_soul:default_player",
                "summary":"Aurora's cautious curiosity towards User increases",
                "tags":[{"vocabulary":"relationship","value":"curiosity"},{"vocabulary":"relationship","value":"unknown_tag"}],
                "evidence_quote":"Long time no see."
              }]
            }"#,
        )
        .expect("parse");

        assert_eq!(
            parsed.relationship_rows[0].dimension,
            Some(RelationshipDimension::Curiosity)
        );
        assert_eq!(parsed.relationship_rows[0].selected_tags, vec!["curiosity"]);
    }

    #[test]
    fn relationship_direction_infers_from_summary_increases() {
        let parsed = parse_eval_form_response(
            r#"{
              "event_rows":[{"event_id":"evt","event_type":"scene_event","summary":"Aurora grows curious.","participants":["aurora_soul","default_player"],"evidence_quote":"Long time no see."}],
              "relationship_rows":[{
                "relationship_id":"rel:aurora_soul:default_player",
                "summary":"Aurora's cautious curiosity towards User increases",
                "tags":["curiosity"],
                "importance_tier":"high",
                "evidence_quote":"Long time no see."
              }]
            }"#,
        )
        .expect("parse");

        assert_eq!(
            parsed.relationship_rows[0].direction,
            Some(RelationshipDirection::Increase)
        );
        assert_eq!(
            parsed.relationship_rows[0].magnitude_tier,
            Some(MagnitudeTier::Medium)
        );
    }

    #[test]
    fn relationship_unknown_tag_dropped_not_fatal() {
        let (soul, world) = soul_and_world();
        let spec = build_eval_form_spec(
            &soul,
            Some(&world),
            "Long time no see.",
            "Aurora studies the visitor with cautious curiosity.",
            8,
        );
        let context = live_fixture_context(
            &soul,
            &world,
            "Long time no see.",
            "Aurora studies the visitor with cautious curiosity.",
        );
        let response = parse_eval_form_response(
            r#"{
              "event_rows":[{"event_id":"evt","event_type":"scene_event","summary":"Aurora studies the visitor.","participants":["aurora_soul","default_player"],"evidence_quote":"Long time no see."}],
              "relationship_rows":[{
                "relationship_id":"rel:aurora_soul:default_player",
                "summary":"Aurora's cautious curiosity towards User increases",
                "tags":["curiosity","totally_unknown"],
                "evidence_quote":"Aurora studies the visitor with cautious curiosity."
              }]
            }"#,
        )
        .expect("parse");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert!(result.rejected_rows.is_empty());
        assert_eq!(
            result.output.relationship_evaluations[0].curiosity,
            Some(1.0)
        );
    }

    #[test]
    fn payload_fixture_applies_curiosity_delta() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let user = "I walk in. Long time no see, Aurora.";
        let narrator = "Aurora's cautious curiosity towards User increases as she steps aside. She studies the visitor with cautious curiosity.";
        let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
        let context = live_fixture_context(&soul, &world, user, narrator);
        let response = parse_eval_form_response(&format!(
            r#"{{
              "event_rows":[{{"event_id":"evt","event_type":"scene_event","summary":"Aurora lets the visitor in.","participants":["{}","default_player"],"evidence_quote":"I walk in. Long time no see, Aurora."}}],
              "relationship_rows":[{{
                "relationship_id":"rel:{}:default_player",
                "summary":"Aurora's cautious curiosity towards User increases",
                "tags":["curiosity","fear"],
                "importance_tier":"medium",
                "evidence_quote":"Aurora's cautious curiosity towards User increases"
              }}]
            }}"#,
            soul.character_id, soul.character_id
        ))
        .expect("parse");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(
            result
                .conversion
                .patch
                .soul_patch
                .as_ref()
                .unwrap()
                .relationship_deltas[0]
                .curiosity,
            Some(1.0)
        );
    }

    #[test]
    fn payload_fixture_writes_unresolved_tension_memory() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let user = "I walk in. Long time no see, Aurora.";
        let narrator = "Aurora smiles, but her nerves remain visible; the reunion leaves unresolved tension in the room.";
        let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
        let context = live_fixture_context(&soul, &world, user, narrator);
        let response = parse_eval_form_response(&format!(
            r#"{{
              "event_rows":[{{"event_id":"evt","event_type":"scene_event","summary":"The visitor enters Aurora's apartment.","participants":["{}","default_player"],"evidence_quote":"I walk in. Long time no see, Aurora."}}],
              "memory_rows":[{{
                "owner_soul_id":"{}",
                "slot_id":"unresolved_tension",
                "candidate_memory":"Aurora's nerves make the reunion feel unresolved.",
                "salience":"medium",
                "evidence_quote":"her nerves remain visible; the reunion leaves unresolved tension in the room"
              }}]
            }}"#,
            soul.character_id, soul.character_id
        ))
        .expect("parse");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert!(result
            .conversion
            .patch
            .soul_patch
            .as_ref()
            .unwrap()
            .new_memories
            .iter()
            .any(|memory| memory.memory_slot.as_deref() == Some("unresolved_tension")));
    }

    #[test]
    fn payload_fixture_writes_recent_emotional_state_memory() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let user = "I walk in. Long time no see, Aurora.";
        let narrator =
            "Aurora shifts from waiting alone to playful engagement after the visitor enters.";
        let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
        let context = live_fixture_context(&soul, &world, user, narrator);
        let response = parse_eval_form_response(&format!(
            r#"{{
              "event_rows":[{{"event_id":"evt","event_type":"scene_event","summary":"The visitor enters Aurora's apartment.","participants":["{}","default_player"],"evidence_quote":"I walk in. Long time no see, Aurora."}}],
              "memory_rows":[{{
                "owner_soul_id":"{}",
                "slot_id":"recent_emotional_state",
                "candidate_memory":"Aurora shifts from waiting alone to playful engagement after the visitor enters.",
                "salience":"medium",
                "evidence_quote":"Aurora shifts from waiting alone to playful engagement after the visitor enters."
              }}]
            }}"#,
            soul.character_id, soul.character_id
        ))
        .expect("parse");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert!(result
            .conversion
            .patch
            .soul_patch
            .as_ref()
            .unwrap()
            .new_memories
            .iter()
            .any(|memory| memory.memory_slot.as_deref() == Some("recent_emotional_state")));
    }

    const PAYLOAD4_JSON: &str = r#"{
  "event_rows": [
    {
      "event_id": "evt_knock_response",
      "event_type": "scene_event",
      "importance_tier": "medium",
      "timestamp": "latest",
      "summary": "User knocks on door, Aurora responds",
      "evidence_quote": "The knock is soft but distinct in the quiet apartment. Aurora, who had been standing at her window watching rain streak the neon-lit glass, flinches slightly... She unlocks the door and pulls it open just enough to stand in the gap, one hand still on the knob."
    }
  ],
  "object_rows": [
    {
      "object_id": "obj_cigarette_mug",
      "object_label": "mug used as ashtray",
      "object_type": "consumable",
      "location_observed": "Aurora's apartment",
      "state_change": "cigarette stubbed out",
      "evidence_quote": "stubs the cigarette out in a nearby mug"
    }
  ],
  "relationship_rows": [
    {
      "relationship_id": "rel:e0ee4936-2e71-4ab9-8631-4c22be68ec72:default_player",
      "dimension": "affection",
      "shift": "+2",
      "evidence_quote": "A faint smile touches her mouth—half anticipation, half nerves... 'Hey,' she says, her voice husky and warm. 'You’re here.'"
    },
    {
      "relationship_id": "rel:e0ee4936-2e71-4ab9-8631-4c22be68ec72:default_player",
      "dimension": "comfort",
      "shift": "+3",
      "evidence_quote": "She unlocks the door and pulls it open just enough to stand in the gap"
    }
  ],
  "memory_rows": [
    {
      "slot_id": "relationship_memory",
      "candidate_memory": "Aurora welcomes User at door with warm, slightly nervous greeting, showing growing affection and comfort",
      "salience": "high",
      "evidence_quote": "A faint smile touches her mouth—half anticipation, half nerves... 'Hey,' she says, her voice husky and warm. 'You’re here.'"
    },
    {
      "slot_id": "current_plot_memory",
      "candidate_memory": "User arrives at Aurora's apartment after knocking; scene shifts from solitude to interaction",
      "salience": "high",
      "evidence_quote": "The knock is soft but distinct in the quiet apartment... She unlocks the door and pulls it open just enough to stand in the gap."
    },
    {
      "slot_id": "character_identity_memory",
      "candidate_memory": "Aurora experiences nervous anticipation when User arrives, revealing emotional investment",
      "salience": "medium",
      "evidence_quote": "A faint smile touches her mouth—half anticipation, half nerves."
    },
    {
      "slot_id": "unresolved_tension",
      "candidate_memory": "Aurora's nerves and anticipation create unresolved tension as she greets User",
      "salience": "medium",
      "evidence_quote": "half anticipation, half nerves"
    },
    {
      "slot_id": "recent_emotional_state",
      "candidate_memory": "Aurora shifts from thoughtful solitude to nervous anticipation upon hearing knock",
      "salience": "medium",
      "evidence_quote": "Aurora, who had been standing at her window watching rain streak the neon-lit glass, flinches slightly... Now she exhales a plume of smoke... moves quickly across the room."
    }
  ],
  "review_rows": [
    {
      "soul_id": "e0ee4936-2e71-4ab9-8631-4c22be68ec72",
      "soul_name": "Aurora Schwarz",
      "perceptions": [
        {
          "event": "evt_knock_response",
          "what_soul_knew": "Aurora knows User knocked and has arrived at her door",
          "evidence_quote": "She unlocks the door and pulls it open just enough to stand in the gap... 'You’re here.'"
        }
      ],
      "misunderstandings": []
    },
    {
      "soul_id": "default_player",
      "soul_name": "User",
      "perceptions": [
        {
          "event": "evt_knock_response",
          "what_soul_knew": "User knows they knocked and Aurora answered the door",
          "evidence_quote": "I knock at the door"
        }
      ],
      "misunderstandings": []
    }
  ]
}"#;

    #[test]
    fn payload4_fixture_applies_object_state() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
        let context = EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: "I knock",
            latest_narrator_response: "Door opens",
            session_world: Some(&world),
            baseline_recent_event_id: None,
        };
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert!(result.rejected_rows.is_empty());
        assert_eq!(result.output.object_changes.len(), 1);
        let object_change = &result.output.object_changes[0];
        assert_eq!(object_change.object_state.object_id, "cigarette_mug");
        assert_eq!(object_change.object_state.status, "cigarette stubbed out");
        assert_eq!(object_change.object_state.location, "Aurora's apartment");
    }

    #[test]
    fn payload4_fixture_applies_relationship_affection_comfort() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
        let context = EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: "I knock",
            latest_narrator_response: "Door opens",
            session_world: Some(&world),
            baseline_recent_event_id: None,
        };
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.output.relationship_evaluations.len(), 2);

        let rel_affection = result
            .output
            .relationship_evaluations
            .iter()
            .find(|r| r.affection.is_some())
            .unwrap();
        assert_eq!(rel_affection.affection, Some(2.0));
        assert_eq!(
            rel_affection.source_soul_id,
            "e0ee4936-2e71-4ab9-8631-4c22be68ec72"
        );
        assert_eq!(rel_affection.target_entity_id, "default_player");
        assert!(rel_affection
            .evidence_quote
            .as_ref()
            .unwrap()
            .contains("A faint smile"));

        let rel_comfort = result
            .output
            .relationship_evaluations
            .iter()
            .find(|r| r.comfort.is_some())
            .unwrap();
        assert_eq!(rel_comfort.comfort, Some(3.0));
        assert_eq!(
            rel_comfort.source_soul_id,
            "e0ee4936-2e71-4ab9-8631-4c22be68ec72"
        );
        assert_eq!(rel_comfort.target_entity_id, "default_player");
    }

    #[test]
    fn payload4_fixture_writes_soul_memory_recent() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
        let context = EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: "I knock",
            latest_narrator_response: "Door opens",
            session_world: Some(&world),
            baseline_recent_event_id: None,
        };
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert!(result.output.memory_candidates.len() > 0);
        let rel_mem = result
            .output
            .memory_candidates
            .iter()
            .find(|m| m.slot == MemorySlot::RelationshipMemory)
            .unwrap();
        assert_eq!(
            rel_mem.owner_soul_id,
            "e0ee4936-2e71-4ab9-8631-4c22be68ec72"
        );
        assert_eq!(
            rel_mem.target_entity_ids,
            vec!["default_player".to_string()]
        );
    }

    #[test]
    fn payload4_fixture_does_not_turn_subjective_memory_into_world_event() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
        let context = EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: "I knock",
            latest_narrator_response: "Door opens",
            session_world: Some(&world),
            baseline_recent_event_id: None,
        };
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        for change in &result.output.world_changes {
            if let Some(ref summary) = change.event_summary {
                assert!(!summary.contains("Aurora welcomes User"));
                assert!(!summary.contains("Aurora's nerves"));
            }
        }
    }

    #[test]
    fn payload4_fixture_exports_nonempty_memory_object_relationship() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
        let context = EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: "I knock",
            latest_narrator_response: "Door opens",
            session_world: Some(&world),
            baseline_recent_event_id: None,
        };
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert!(!result.output.memory_candidates.is_empty());
        assert!(!result.output.object_changes.is_empty());
        assert!(!result.output.relationship_evaluations.is_empty());
    }

    #[test]
    fn payload4_relationship_comfort_boundary_pressure_compiles() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let user = "I walk in. Long time no see, Aurora.";
        let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
        let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
        let context = live_fixture_context(&soul, &world, user, narrator);
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        let rel_comfort = result
            .output
            .relationship_evaluations
            .iter()
            .find(|r| r.comfort.is_some())
            .unwrap();
        assert_eq!(rel_comfort.comfort, Some(3.0));
        assert_eq!(rel_comfort.source_soul_id, soul.character_id);
        assert_eq!(rel_comfort.target_entity_id, "default_player");
        assert!(rel_comfort.criterion_met);
    }

    #[test]
    fn payload4_relationship_conflict_potential_escalation_compiles() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let user = "I walk in. Long time no see, Aurora.";
        let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
        let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
        let context = live_fixture_context(&soul, &world, user, narrator);
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        let payload_json: serde_json::Value = serde_json::from_str(PAYLOAD4_JSON).unwrap();
        let has_conflict = payload_json["relationship_rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| {
                row["dimension"]
                    .as_str()
                    .map(|d| d == "conflict")
                    .unwrap_or(false)
            });

        if has_conflict {
            let rel_conflict = result
                .output
                .relationship_evaluations
                .iter()
                .find(|r| r.conflict.is_some());
            assert!(
                rel_conflict.is_some(),
                "conflict relationship should be compiled"
            );
            let rel_conflict = rel_conflict.unwrap();
            assert!(
                rel_conflict.conflict.unwrap() > 0.0,
                "conflict should be positive"
            );
        }
    }

    #[test]
    fn payload4_memory_preserves_candidate_text() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let user = "I walk in. Long time no see, Aurora.";
        let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
        let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
        let context = live_fixture_context(&soul, &world, user, narrator);
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        let rel_mem = result
            .output
            .memory_candidates
            .iter()
            .find(|m| m.slot == MemorySlot::RelationshipMemory)
            .unwrap();
        assert!(
            !rel_mem.content.is_empty(),
            "memory content should not be empty"
        );
        assert!(
            rel_mem.content.contains("Aurora")
                && (rel_mem.content.contains("welcomes")
                    || rel_mem.content.contains("warm")
                    || rel_mem.content.contains("nervous")),
            "memory content should preserve candidate text, got: {}",
            rel_mem.content
        );
    }

    #[test]
    fn payload4_exports_relationship_delta_or_changed_summary() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let user = "I walk in. Long time no see, Aurora.";
        let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
        let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
        let context = live_fixture_context(&soul, &world, user, narrator);
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert!(
            !result.output.relationship_evaluations.is_empty(),
            "relationship evaluations should not be empty"
        );
        assert_eq!(
            result.draft.relationship_delta_count,
            result.output.relationship_evaluations.len(),
            "draft relationship_delta_count should match evaluations count"
        );
    }

    #[test]
    fn payload4_next_prompt_retrieves_written_memory() {
        let soul = soul_aurora();
        let world = session_world_from_legacy_world("Apartment", None, &soul.world);
        let user = "I walk in. Long time no see, Aurora.";
        let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
        let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
        let context = live_fixture_context(&soul, &world, user, narrator);
        let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert!(
            result.draft.memory_candidate_count > 0,
            "draft should have memory candidates"
        );
        assert!(
            !result.output.memory_candidates.is_empty(),
            "output should have memory candidates"
        );
        let has_relationship_memory = result
            .output
            .memory_candidates
            .iter()
            .any(|m| m.slot == MemorySlot::RelationshipMemory);
        assert!(
            has_relationship_memory,
            "should have at least one relationship memory candidate"
        );
    }

    #[test]
    fn form_validated_relationship_bypasses_second_evidence_check_only_for_that_row() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "I knock.", "Door opens.");
        let result = compile_eval_form_response(
            &spec,
            &EvalFormResponse {
                event_rows: vec![event("evt_knock", "The visitor knocks", "I knock.")],
                relationship_rows: vec![RelationshipRow {
                    linked_event_id: "evt_knock".into(),
                    source_soul_id: soul.character_id.clone(),
                    target_entity_id: "default_player".into(),
                    dimension: Some(RelationshipDimension::Comfort),
                    direction: Some(RelationshipDirection::Increase),
                    magnitude_tier: Some(MagnitudeTier::Small),
                    importance_tier: Some(ImportanceTier::Medium),
                    evidence_quote: "A faint smile touches her mouth, half anticipation".into(),
                    ..RelationshipRow::default()
                }],
                ..EvalFormResponse::default()
            },
            &context,
        );
        assert_eq!(
            result.output.relationship_evaluations.len(),
            1,
            "form-validated relationship should compile"
        );
        assert!(
            result.output.relationship_evaluations[0].evidence_validated_by_form,
            "flag should be true for form path"
        );
        let conversion = evaluator_output_to_engine_patch(
            &result.output,
            &EvaluatorConversionContext {
                active_soul_id: &soul.character_id,
                active_soul_ids: vec![soul.character_id.clone()],
                latest_user_message: "I knock.",
                latest_narrator_response: "Door opens.",
                session_world: Some(&world),
                baseline_recent_event_id: None,
            },
        );
        assert!(
            conversion.patch.soul_patch.is_some(),
            "soul patch should exist from form-validated row"
        );
        assert!(
            !conversion
                .patch
                .soul_patch
                .as_ref()
                .unwrap()
                .relationship_deltas
                .is_empty(),
            "relationship delta should survive evidence check"
        );
        let mut bad_output = result.output.clone();
        bad_output
            .relationship_evaluations
            .push(RelationshipEvaluation {
                source_soul_id: soul.character_id.clone(),
                target_entity_id: "default_player".into(),
                comfort: Some(3.0),
                evidence_quote: Some("A faint smile touches her mouth, half anticipation".into()),
                criterion_met: true,
                confidence: 0.75,
                evidence_validated_by_form: false,
                ..RelationshipEvaluation::default()
            });
        let bad_conversion = evaluator_output_to_engine_patch(
            &bad_output,
            &EvaluatorConversionContext {
                active_soul_id: &soul.character_id,
                active_soul_ids: vec![soul.character_id.clone()],
                latest_user_message: "I knock.",
                latest_narrator_response: "Door opens.",
                session_world: Some(&world),
                baseline_recent_event_id: None,
            },
        );
        assert_eq!(bad_conversion.patch.soul_patch.as_ref().unwrap().relationship_deltas.len(), 1, "only the form-validated row should survive; non-form row with same bad quote should be rejected");
    }

    #[test]
    fn latest_payload_event_row_without_id_or_summary_compiles() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "I knock.",
            "A firm, three-rap knock echoes in the hallway.",
        );
        let response = parse_eval_form_response(
            r#"{
              "event_rows": [{
                "type": "scene_event",
                "importance": "medium",
                "tags": ["scene_event", "doorway"],
                "evidence_quote": "A firm, three-rap knock echoes in the hallway, cutting through the low hum of the ambient music."
              }],
              "relationship_rows": [],
              "memory_rows": [],
              "review_rows": []
            }"#,
        ).expect("parse should succeed");
        let event_row = &response.event_rows[0];
        assert_eq!(
            event_row.event_type,
            Some(EventType::SceneEvent),
            "type should normalize to event_type"
        );
        assert_eq!(
            event_row.importance_tier,
            Some(ImportanceTier::Medium),
            "importance should normalize to importance_tier"
        );
        assert_eq!(
            event_row.event_id, "event_latest_turn",
            "missing event_id should default"
        );
        assert!(
            event_row.objective_summary.contains("knock echoes"),
            "missing objective_summary should derive from evidence_quote"
        );
    }

    #[test]
    fn latest_payload_object_property_new_state_compiles() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(
            &soul,
            &world,
            "I set it down.",
            "She sets her wine glass down on the table.",
        );
        let response = parse_eval_form_response(
            r#"{
              "event_rows": [{
                "event_id": "evt_wine_glass",
                "event_type": "scene_event",
                "summary": "User sets wine glass down",
                "evidence_quote": "She sets her wine glass down on the table."
              }],
              "object_rows": [{
                "object_id": "wine_glass",
                "property": "location",
                "old_state": "being held",
                "new_state": "on surface",
                "evidence_quote": "She sets her wine glass down on the table."
              }],
              "relationship_rows": [],
              "memory_rows": [],
              "review_rows": []
            }"#,
        )
        .expect("parse should succeed");
        let object_row = &response.object_rows[0];
        assert_eq!(
            object_row.property_changed, "location",
            "property should normalize to property_changed"
        );
        assert_eq!(
            object_row.new_value, "on surface",
            "new_state should normalize to new_value"
        );
        assert_eq!(
            object_row.old_value,
            Some("being held".into()),
            "old_state should normalize to old_value"
        );
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(
            result.output.object_changes.len(),
            1,
            "object change should compile"
        );
        assert_eq!(
            result.output.object_changes[0].object_state.object_id,
            "wine_glass"
        );
    }

    const LIVE_POOLSIDE_JSON: &str = r#"{
      "event_rows": [
        {
          "event_id": "event_latest_turn",
          "event_type": "scene_event",
          "objective_summary": "Aurora and User reunite warm",
          "participants": ["aurora_soul", "default_player"],
          "evidence_quote": "She smiles warm and lets you inside."
        }
      ],
      "relationship_rows": [
        {
          "relationship_dimension": "trust",
          "change_direction": "increased",
          "evidence_quote": "She smiles warm and lets you inside.",
          "tag_vocabularies": ["relationship", "reunion"]
        },
        {
          "relationship_dimension": "fear",
          "change_direction": "decreased",
          "evidence_quote": "She smiles warm and lets you inside.",
          "tag_vocabularies": ["relationship", "reunion"]
        }
      ],
      "memory_rows": [
        {
          "memory_slot": "relationship_memory",
          "importance_tier": "high",
          "evidence_quote": "She smiles warm and lets you inside.",
          "candidate_summary": "Reunion with someone familiar who previously inspired Aurora's sketching"
        }
      ],
      "review_rows": [
        {
          "per_soul_evaluation": {
            "soul_id": "aurora_soul",
            "soul_perceived_event": true,
            "soul_knows": []
          }
        }
      ]
    }"#;

    #[test]
    fn live_poolside_relationship_dimension_alias_compiles() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
        let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.relationship_evaluations.len(), 2);
        
        let trust_delta = result.output.relationship_evaluations.iter().find(|r| r.trust.is_some()).unwrap();
        assert!(trust_delta.trust.unwrap() > 0.0);

        let fear_delta = result.output.relationship_evaluations.iter().find(|r| r.fear.is_some()).unwrap();
        assert!(fear_delta.fear.unwrap() < 0.0);
    }

    #[test]
    fn live_poolside_memory_slot_alias_compiles() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
        let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.memory_candidates.len(), 1);
        assert_eq!(result.output.memory_candidates[0].slot, MemorySlot::RelationshipMemory);
    }

    #[test]
    fn live_poolside_candidate_summary_becomes_content() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
        let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.output.memory_candidates[0].content, "Reunion with someone familiar who previously inspired Aurora's sketching");
    }

    #[test]
    fn live_poolside_missing_owner_defaults_to_active_soul() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
        let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.output.memory_candidates[0].owner_soul_id, "aurora_soul");
    }

    #[test]
    fn live_poolside_review_without_evidence_is_advisory_not_fatal() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
        let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert_eq!(result.trace.form_rows_rejected, 0);
        assert!(result.rejected_rows.is_empty());
    }

    #[test]
    fn live_poolside_fixture_accepts_more_than_one_row() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
        let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
        let result = compile_eval_form_response(&spec, &response, &context);

        // 1 event row + 2 relationship rows + 1 memory row + 1 review row = 5 rows accepted
        assert_eq!(result.trace.form_rows_accepted, 5);
        assert_eq!(result.trace.form_rows_rejected, 0);
    }

    #[test]
    fn live_poolside_fixture_writes_memory_and_relationship_delta() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
        let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
        let result = compile_eval_form_response(&spec, &response, &context);

        assert!(result.draft.memory_candidate_count > 0);
        assert!(result.draft.relationship_delta_count > 0);
        
        // Also verify the converted report writes memories and relationship deltas
        let patch = &result.conversion.patch;
        let soul_patch = patch.soul_patch.as_ref().unwrap();
        assert_eq!(soul_patch.new_memories.len(), 1);
        assert_eq!(soul_patch.relationship_deltas.len(), 2);
    }

    #[test]
    fn live_unescaped_evidence_quote_repairs_and_parses() {
        let raw_json = r#"{
          "event_rows": [{
            "event_id": "evt_dialogue",
            "event_type": "scene_event",
            "objective_summary": "Aurora warns User",
            "participants": ["aurora_soul", "default_player"],
            "evidence_quote": "You better not be late" she calls back, voice low and rough with wine"
          }]
        }"#;
        let response = parse_eval_form_response(raw_json).expect("should repair unescaped quotes and parse successfully");
        assert_eq!(response.event_rows[0].evidence_quote, "You better not be late\" she calls back, voice low and rough with wine");
    }

    #[test]
    fn live_shift_direction_relationship_compiles_to_delta() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Warm smiles.");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "source_entity_id": "aurora_soul",
                "target_entity_id": "default_player",
                "dimension": "trust",
                "shift_direction": "increased",
                "evidence_quote": "Warm smiles."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.output.relationship_evaluations.len(), 1);
        assert!(result.output.relationship_evaluations[0].trust.unwrap() > 0.0);
    }

    #[test]
    fn live_source_entity_id_maps_to_source_soul_id() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Warm smiles.");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "source_entity_id": "aurora_soul",
                "target_entity_id": "default_player",
                "dimension": "trust",
                "shift_direction": "increased",
                "evidence_quote": "Warm smiles."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.normalized_response.relationship_rows[0].source_soul_id, "aurora_soul");
    }

    #[test]
    fn live_relationship_change_type_without_direction_infers_increase_when_dimension_boundary_pressure() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "increased tension, pressure, and conflict");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "source_entity_id": "aurora_soul",
                "target_entity_id": "default_player",
                "dimension": "boundary_pressure",
                "change_type": "shift",
                "evidence_quote": "increased tension, pressure, and conflict"
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.output.relationship_evaluations.len(), 1);
        assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
    }

    #[test]
    fn live_object_state_new_object_label_compiles_to_object_patch() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
        let response = parse_eval_form_response(
            r#"{
              "object_rows": [{
                "entity_id": "jacket",
                "new_object_label": "wet jacket",
                "object_state": "placed on chair",
                "evidence_quote": "Fabric drips."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.output.object_changes.len(), 1);
        assert_eq!(result.output.object_changes[0].object_state.object_id, "wet_jacket");
        assert_eq!(result.output.object_changes[0].object_state.status, "placed on chair");
    }

    #[test]
    fn live_object_state_defaults_property_changed_state() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
        let response = parse_eval_form_response(
            r#"{
              "object_rows": [{
                "entity_id": "jacket",
                "new_object_label": "wet jacket",
                "object_state": "placed on chair",
                "evidence_quote": "Fabric drips."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.normalized_response.object_rows[0].property_changed, "state");
    }

    #[test]
    fn live_content_summary_preserved_in_memory_content() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
        let response = parse_eval_form_response(
            r#"{
              "memory_rows": [{
                "memory_slot": "relationship_memory",
                "importance_tier": "high",
                "evidence_quote": "Fabric drips.",
                "content_summary": "Reunion with someone familiar"
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.output.memory_candidates[0].content, "Reunion with someone familiar");
    }

    #[test]
    fn live_candidate_summary_preserved_in_memory_content() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
        let response = parse_eval_form_response(
            r#"{
              "memory_rows": [{
                "memory_slot": "relationship_memory",
                "importance_tier": "high",
                "evidence_quote": "Fabric drips.",
                "candidate_summary": "Reunion with someone familiar who previously inspired Aurora"
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.output.memory_candidates[0].content, "Reunion with someone familiar who previously inspired Aurora");
    }

    #[test]
    fn live_memory_does_not_fallback_to_event_summary_when_candidate_exists() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
        let response = parse_eval_form_response(
            r#"{
              "memory_rows": [{
                "memory_slot": "relationship_memory",
                "importance_tier": "high",
                "evidence_quote": "Fabric drips.",
                "candidate_summary": "Reunion with someone familiar who previously inspired Aurora"
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.output.memory_candidates[0].content, "Reunion with someone familiar who previously inspired Aurora");
    }

    #[test]
    fn live_full_poolside_payload_writes_memory_relationship_and_object() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "I knock at the door", "Fabric drips.");
        let response = parse_eval_form_response(
            r#"{
              "event_rows": [{
                "event_id": "event_latest_turn",
                "event_type": "scene_event",
                "objective_summary": "User arrives wet at Aurora's door",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "Fabric drips."
              }],
              "relationship_rows": [{
                "source_entity_id": "aurora_soul",
                "target_entity_id": "default_player",
                "dimension": "boundary_pressure",
                "shift_direction": "increased",
                "evidence_quote": "Fabric drips."
              }],
              "object_rows": [{
                "entity_id": "jacket",
                "new_object_label": "wet jacket",
                "object_state": "placed on chair",
                "evidence_quote": "Fabric drips."
              }],
              "memory_rows": [{
                "memory_slot": "relationship_memory",
                "importance_tier": "high",
                "evidence_quote": "Fabric drips.",
                "candidate_summary": "Reunion with someone familiar who previously inspired Aurora's sketching"
              }],
              "review_rows": [{
                "candidate_id": "review_test",
                "reason": "already verified",
                "evidence_quote": "Fabric drips."
              }]
            }"#
        ).expect("parse should succeed");
        
        let result = compile_eval_form_response(&spec, &response, &context);
        
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert!(result.trace.form_rows_accepted > 1);
        assert!(result.draft.memory_candidate_count > 0);
        assert!(result.draft.relationship_delta_count > 0);
        
        let patch = &result.conversion.patch;
        let soul_patch = patch.soul_patch.as_ref().unwrap();
        let world_patch = patch.world_patch.as_ref().unwrap();
        
        assert!(soul_patch.new_memories.len() > 0);
        assert!(soul_patch.relationship_deltas.len() > 0);
        assert!(world_patch.object_observation_operations.len() > 0);
    }

    #[test]
    fn relationship_source_slug_aurora_schwarz_resolves_to_active_soul_uuid() {
        let (mut soul, world) = soul_and_world();
        soul.character_name = "Aurora Schwarz".into();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Warm smiles.");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "source_entity_id": "aurora_schwarz",
                "target_entity_id": "default_player",
                "dimension": "trust",
                "shift_direction": "increased",
                "evidence_quote": "Warm smiles."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.relationship_evaluations.len(), 1);
        assert_eq!(result.output.relationship_evaluations[0].source_soul_id, "aurora_soul");
    }

    #[test]
    fn relationship_source_display_name_resolves_to_active_soul_uuid() {
        let (mut soul, world) = soul_and_world();
        soul.character_name = "Aurora Schwarz".into();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Warm smiles.");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "source_entity_id": "Aurora Schwarz",
                "target_entity_id": "default_player",
                "dimension": "trust",
                "shift_direction": "increased",
                "evidence_quote": "Warm smiles."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.relationship_evaluations.len(), 1);
        assert_eq!(result.output.relationship_evaluations[0].source_soul_id, "aurora_soul");
    }

    #[test]
    fn live_payload6_relationship_row_no_longer_rejected() {
        let (mut soul, world) = soul_and_world();
        soul.character_name = "Aurora Schwarz".into();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Didn't expect you to actually show up");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "source_entity_id": "aurora_schwarz",
                "target_entity_id": "default_player",
                "dimension": "boundary_pressure",
                "shift_direction": "increased",
                "evidence_quote": "Didn't expect you to actually show up"
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.relationship_evaluations.len(), 1);
        assert_eq!(result.output.relationship_evaluations[0].source_soul_id, "aurora_soul");
        assert_eq!(result.output.relationship_evaluations[0].target_entity_id, "default_player");
    }

    #[test]
    fn object_status_alias_becomes_new_value() {
        let response = parse_eval_form_response(
            r#"{
              "object_rows": [{
                "object_id": "wet_jacket",
                "new_object_label": "wet jacket",
                "status": "placed_on_chair",
                "evidence_quote": "and place a wet jacket over the chair."
              }]
            }"#
        ).expect("parse should succeed");
        assert_eq!(response.object_rows[0].new_value, "placed_on_chair");
    }

    #[test]
    fn object_status_defaults_property_changed_state() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "and place a wet jacket over the chair.");
        let response = parse_eval_form_response(
            r#"{
              "object_rows": [{
                "object_id": "wet_jacket",
                "new_object_label": "wet jacket",
                "status": "placed_on_chair",
                "evidence_quote": "and place a wet jacket over the chair."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.object_changes.len(), 1);
        assert_eq!(result.normalized_response.object_rows[0].property_changed, "state");
    }

    #[test]
    fn live_payload8_object_rows_compile() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "water darkens the fabric");
        let response = parse_eval_form_response(
            r#"{
              "object_rows": [
                {
                  "object_id": "wet_jacket",
                  "new_object_label": "wet jacket",
                  "status": "placed_on_chair",
                  "evidence_quote": "and place a wet jacket over the chair."
                },
                {
                  "object_id": "chair",
                  "status": "wet_from_jacket_drip",
                  "evidence_quote": "water darkens the fabric before dripping onto the chair's arm"
                }
              ]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.object_changes.len(), 2);
        assert_eq!(result.output.object_changes[0].object_state.object_id, "wet_jacket");
        assert_eq!(result.output.object_changes[0].object_state.status, "placed_on_chair");
        assert_eq!(result.output.object_changes[1].object_state.object_id, "chair");
        assert_eq!(result.output.object_changes[1].object_state.status, "wet_from_jacket_drip");
    }

    #[test]
    fn memory_summary_alias_no_unreachable_pattern_warning() {
        let response = parse_eval_form_response(
            r#"{
              "memory_rows": [{
                "linked_event_id": "evt",
                "owner_soul_id": "aurora_soul",
                "memory_slot": "relationship_memory",
                "summary": "Reunion with someone familiar",
                "evidence_quote": "fabric drips"
              }]
            }"#
        ).expect("parse should succeed");
        assert_eq!(response.memory_rows[0].content, "Reunion with someone familiar");
    }

    #[test]
    fn patch_applied_with_rejected_rows_is_not_failure_status() {
        let rejected = vec![EvalFormRowRejection {
            row_kind: "object".into(),
            row_id: "obj1".into(),
            reason: "invalid".into(),
        }];
        let status = format_honest_ui_status(true, true, true, &rejected);
        assert_eq!(status, "State updated; 1 object row skipped");
    }

    #[test]
    fn ui_status_reports_rows_skipped_not_enrichment_failed() {
        let rejected = vec![
            EvalFormRowRejection {
                row_kind: "object".into(),
                row_id: "obj1".into(),
                reason: "invalid".into(),
            },
            EvalFormRowRejection {
                row_kind: "relationship".into(),
                row_id: "rel1".into(),
                reason: "invalid".into(),
            }
        ];
        let status = format_honest_ui_status(true, true, true, &rejected);
        assert_eq!(status, "State updated; 2 evaluator rows skipped");
    }

    #[test]
    fn live_payload11_chain_lock_change_type_state_change_compiles() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "The chain lock still clicks softly...");
        let response = parse_eval_form_response(
            r#"{
              "object_rows": [{
                "object_id": "chain_lock",
                "change_type": "state_change",
                "importance_tier": "medium",
                "evidence_quote": "The chain lock still clicks softly against the doorframe..."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.object_changes.len(), 1);
        assert_eq!(result.output.object_changes[0].object_state.object_id, "chain_lock");
        assert_eq!(result.output.object_changes[0].object_state.status, "The chain lock still clicks softly against the doorframe...");
    }

    #[test]
    fn live_payload11_wet_jacket_new_object_observation_compiles() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "You hang your soaked jacket...");
        let response = parse_eval_form_response(
            r#"{
              "object_rows": [{
                "object_id": "wet_jacket",
                "change_type": "new_object_observation",
                "importance_tier": "low",
                "evidence_quote": "You hang your soaked jacket on the back of the kitchen chair...",
                "new_object_label": "wet_jacket"
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.object_changes.len(), 1);
        assert_eq!(result.output.object_changes[0].object_state.object_id, "wet_jacket");
        assert_eq!(result.output.object_changes[0].object_state.status, "wet_jacket");
    }

    #[test]
    fn object_change_type_defaults_property_and_value() {
        let response = parse_eval_form_response(
            r#"{
              "object_rows": [
                {
                  "object_id": "chain_lock",
                  "change_type": "state_change",
                  "evidence_quote": "clicks softly"
                },
                {
                  "object_id": "wet_jacket",
                  "change_type": "new_object_observation",
                  "new_object_label": "wet_jacket",
                  "evidence_quote": "hang soaked jacket"
                }
              ]
            }"#
        ).expect("parse should succeed");
        assert_eq!(response.object_rows[0].property_changed, "state");
        assert_eq!(response.object_rows[0].new_value, "clicks softly");
        assert_eq!(response.object_rows[1].property_changed, "presence");
        assert_eq!(response.object_rows[1].new_value, "wet_jacket");
    }

    #[test]
    fn live_payload11_intimacy_shift_infers_increase() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Suit yourself");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "relationship_dimension": "intimacy",
                "change_type": "shift",
                "importance_tier": "high",
                "evidence_quote": "\"Suit yourself,\" she says, voice dropping..."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.relationship_evaluations.len(), 1);
        assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
        assert_eq!(result.output.relationship_evaluations[0].intimacy.is_some(), true);
    }

    #[test]
    fn live_payload11_trust_shift_infers_increase() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Suit yourself");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "relationship_dimension": "trust",
                "change_type": "shift",
                "importance_tier": "high",
                "evidence_quote": "\"Suit yourself,\" she says, voice dropping..."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.relationship_evaluations.len(), 1);
        assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
        assert_eq!(result.output.relationship_evaluations[0].trust.is_some(), true);
    }

    #[test]
    fn relationship_shift_without_direction_compiles_for_positive_dimension() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Suit yourself");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "relationship_dimension": "affection",
                "change_type": "shift",
                "importance_tier": "high",
                "evidence_quote": "closer connection and deeper affection"
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
    }

    #[test]
    fn relationship_missing_direction_rejects_as_uncertain() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Suit yourself");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "relationship_dimension": "trust",
                "change_type": "shift",
                "importance_tier": "high",
                "evidence_quote": "generic uninformative quote"
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 1);
        assert_eq!(result.rejected_rows[0].reason, "direction_missing_uncertain");
    }

    #[test]
    fn live_exact_object_change_row_compiles() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "draping it over the back of the chair");
        let response = parse_eval_form_response(
            r#"{
              "object_rows": [{
                "object_id": "wet_jacket",
                "change_type": "object_change",
                "importance_tier": "low",
                "evidence_quote": "I watch as you shake water from your jacket before draping it over the back of the chair nearest the couch."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.object_changes.len(), 1);
        
        let report = evaluator_output_to_engine_patch(&result.output, &context);
        assert!(!report.patch.is_empty());
        let world_patch = report.patch.world_patch.as_ref().unwrap();
        assert!(world_patch.object_observation_operations.len() > 0);
    }

    #[test]
    fn live_exact_relationship_shift_row_compiles() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Figured you'd never show... Sorry. Been... been a long time since anyone came by. Your presence brings comfort.");
        let response = parse_eval_form_response(
            r#"{
              "relationship_rows": [{
                "relationship_dimension": "comfort",
                "change_type": "relationship_shift",
                "importance_tier": "medium",
                "evidence_quote": "Figured you'd never show... Sorry. Been... been a long time since anyone came by. Your presence brings comfort."
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.relationship_evaluations.len(), 1);
        assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));

        let report = evaluator_output_to_engine_patch(&result.output, &context);
        assert!(!report.patch.is_empty());
        let soul_patch = report.patch.soul_patch.as_ref().unwrap();
        assert!(soul_patch.relationship_deltas.len() > 0);
    }

    #[test]
    fn live_exact_slot_type_memory_row_compiles() {
        let (soul, world) = soul_and_world();
        let (spec, context) = spec_and_context(&soul, &world, "Hey", "Aurora's cigarette trembles slightly in her fingers");
        let response = parse_eval_form_response(
            r#"{
              "memory_rows": [{
                "slot_type": "recent_emotional_state",
                "importance_tier": "medium",
                "evidence_quote": "Aurora's cigarette trembles slightly in her fingers",
                "content_summary": "Aurora is feeling highly anxious and emotional"
              }]
            }"#
        ).expect("parse should succeed");
        let result = compile_eval_form_response(&spec, &response, &context);
        assert_eq!(result.trace.form_rows_rejected, 0);
        assert_eq!(result.output.memory_candidates.len(), 1);

        let report = evaluator_output_to_engine_patch(&result.output, &context);
        assert!(!report.patch.is_empty());
        let soul_patch = report.patch.soul_patch.as_ref().unwrap();
        assert!(soul_patch.new_memories.len() > 0);
    }

    #[test]
    fn patch_applied_with_row_skips_does_not_show_hard_failure() {
        let rejected = vec![
            EvalFormRowRejection {
                row_kind: "relationship".into(),
                row_id: "event_latest_turn:aurora_soul:default_player".into(),
                reason: "direction_missing_uncertain".into(),
            }
        ];
        let honest_status = format_honest_ui_status(
            true, // patch_applied
            true, // materialized_soul_updated
            true, // materialized_session_world_updated
            &rejected,
        );
        assert_eq!(honest_status, "State updated; 1 relationship row skipped");
    }
}
