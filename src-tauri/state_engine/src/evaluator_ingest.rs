use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::evaluator::{
    EvaluatorOutputV1, GlobalSceneEvaluation, KnowledgeScope, MemoryCandidate, MemorySlot,
    ObjectChangeEvaluation, PerSoulEvaluation, RelationshipEvaluation, RelevanceTags,
    TurnClassification, WorldChangeEvaluation,
};
use crate::patch::SceneStatePatch;
use crate::soul::{MemorySourceType, ObjectState, TruthStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorParseResult {
    pub output: EvaluatorOutputV1,
    pub draft: NormalizedEvaluationDraft,
    pub normalized_json: String,
    pub normalized: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvaluatorDraftContext {
    pub active_soul_id: String,
    pub active_soul_display_name: String,
    pub active_soul_ids: Vec<String>,
    pub latest_user_message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NormalizedEvaluationDraft {
    pub scene_evaluation: GlobalSceneEvaluation,
    pub memory_candidate_count: usize,
    pub world_event_count: usize,
    pub scene_state_present: bool,
    pub relationship_delta_count: usize,
    pub per_soul_interpretation_count: usize,
    pub object_observation_count: usize,
    pub warnings: Vec<String>,
    pub candidate_quality_decisions: Vec<String>,
    pub candidate_routing_decisions: Vec<String>,
    pub state_effect_guarantee_applied: bool,
    pub state_effect_guarantee_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaxEvaluatorOutput {
    pub schema_version: Option<serde_json::Value>,
    pub thought_process: Option<serde_json::Value>,
    pub turn_flags_u64: Option<serde_json::Value>,
    pub turn_classification: Option<LaxTurnClassification>,
    pub global_scene_evaluation: Option<LaxGlobalSceneEvaluation>,
    pub per_soul_evaluations: Option<Vec<LaxPerSoulEvaluation>>,
    pub world_changes: Option<Vec<LaxWorldChangeEvaluation>>,
    pub object_changes: Option<Vec<LaxObjectChangeEvaluation>>,
    pub relationship_evaluations: Option<Vec<LaxRelationshipEvaluation>>,
    pub memory_candidates: Option<Vec<LaxMemoryCandidate>>,
    pub relevance_tags: Option<LaxRelevanceTags>,
    pub no_op_reason: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaxTurnClassification {
    pub is_pure_ooc: Option<serde_json::Value>,
    pub scene_event_occurred: Option<serde_json::Value>,
    pub is_retcon_or_correction: Option<serde_json::Value>,
    pub human_summary: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaxGlobalSceneEvaluation {
    pub scene_event_occurred: Option<serde_json::Value>,
    pub location_changed: Option<serde_json::Value>,
    pub object_state_changed: Option<serde_json::Value>,
    pub relationship_changed: Option<serde_json::Value>,
    pub unresolved_tension: Option<serde_json::Value>,
    pub current_plot_advanced: Option<serde_json::Value>,
    pub character_identity_changed: Option<serde_json::Value>,
    pub recent_emotional_state_changed: Option<serde_json::Value>,
    pub contradiction_detected: Option<serde_json::Value>,
    pub evidence_quote: Option<serde_json::Value>,
    pub summary: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaxPerSoulEvaluation {
    pub soul_id: Option<serde_json::Value>,
    pub observed: Option<serde_json::Value>,
    pub knowledge_scope: Option<serde_json::Value>,
    pub subjective_interpretation: Option<serde_json::Value>,
    pub emotional_state: Option<serde_json::Value>,
    pub relationship_deltas: Option<Vec<LaxRelationshipEvaluation>>,
    pub memory_candidates: Option<Vec<LaxMemoryCandidate>>,
    pub relevance_tags: Option<LaxRelevanceTags>,

    // Aliases!
    pub primary_soul: Option<serde_json::Value>,
    pub soul: Option<serde_json::Value>,
    pub owner: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaxWorldChangeEvaluation {
    pub change_id: Option<serde_json::Value>,
    pub location: Option<serde_json::Value>,
    pub event_summary: Option<serde_json::Value>,
    pub change_type: Option<serde_json::Value>,
    pub target: Option<serde_json::Value>,
    pub new_state: Option<serde_json::Value>,
    pub scene_state: Option<serde_json::Value>,
    pub active_plot_add: Option<serde_json::Value>,
    pub active_plot_resolve: Option<serde_json::Value>,
    pub evidence_quote: Option<serde_json::Value>,
    pub confidence: Option<serde_json::Value>,
    pub relevance_tags: Option<LaxRelevanceTags>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaxObjectChangeEvaluation {
    pub change_id: Option<serde_json::Value>,
    pub object_state: Option<serde_json::Value>,
    pub evidence_quote: Option<serde_json::Value>,
    pub confidence: Option<serde_json::Value>,
    pub relevance_tags: Option<LaxRelevanceTags>,

    // Allow top-level aliases!
    pub object: Option<serde_json::Value>,
    pub change: Option<serde_json::Value>,
    pub previous_state: Option<serde_json::Value>,
    pub entity_id: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LaxRelationshipEvaluation {
    pub source_soul_id: Option<serde_json::Value>,
    pub target_entity_id: Option<serde_json::Value>,
    pub trust: Option<serde_json::Value>,
    pub affection: Option<serde_json::Value>,
    pub intimacy: Option<serde_json::Value>,
    pub passion: Option<serde_json::Value>,
    pub commitment: Option<serde_json::Value>,
    pub fear: Option<serde_json::Value>,
    pub desire: Option<serde_json::Value>,
    pub respect: Option<serde_json::Value>,
    pub conflict: Option<serde_json::Value>,
    pub dependency: Option<serde_json::Value>,
    pub curiosity: Option<serde_json::Value>,
    pub comfort: Option<serde_json::Value>,
    pub boundary_pressure: Option<serde_json::Value>,
    pub evidence_quote: Option<serde_json::Value>,
    pub criterion_met: Option<serde_json::Value>,
    pub confidence: Option<serde_json::Value>,
    pub relevance_tags: Option<LaxRelevanceTags>,

    // Allow alternate names & nesting:
    pub soul_id: Option<serde_json::Value>,
    pub source: Option<serde_json::Value>,
    pub target: Option<serde_json::Value>,
    pub entity_id: Option<serde_json::Value>,
    pub actor: Option<serde_json::Value>,
    pub changes: Option<serde_json::Value>,
    pub deltas: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaxMemoryCandidate {
    pub candidate_id: Option<serde_json::Value>,
    pub owner_soul_id: Option<serde_json::Value>,
    pub slot: Option<serde_json::Value>,
    pub content: Option<serde_json::Value>,
    pub details: Option<serde_json::Value>,
    pub summary: Option<serde_json::Value>,
    pub action: Option<serde_json::Value>,
    pub evidence_quote: Option<serde_json::Value>,
    pub criterion_met: Option<serde_json::Value>,
    pub confidence: Option<serde_json::Value>,
    pub salience: Option<serde_json::Value>,
    pub retrieval_strength: Option<serde_json::Value>,
    pub perceived_by_entity_id: Option<serde_json::Value>,
    pub target_entity_ids: Option<serde_json::Value>,
    pub source_type: Option<serde_json::Value>,
    pub truth_status: Option<serde_json::Value>,
    pub relevance_tags: Option<serde_json::Value>,
    pub knowledge_scope: Option<serde_json::Value>,

    // Aliases!
    pub soul_id: Option<serde_json::Value>,
    pub primary_soul: Option<serde_json::Value>,
    pub target_souls: Option<serde_json::Value>,
    pub estimated_strength: Option<serde_json::Value>,
    pub proposed_memory_slot: Option<serde_json::Value>,
    pub memory_type: Option<serde_json::Value>,
    pub slots: Option<serde_json::Value>,
    pub specifics: Option<serde_json::Value>,
    pub payload: Option<serde_json::Value>,
    pub actor: Option<serde_json::Value>,
    pub target: Option<serde_json::Value>,
    pub relationship_delta: Option<serde_json::Value>,
    pub changes: Option<serde_json::Value>,
    pub deltas: Option<serde_json::Value>,
    pub tags: Option<serde_json::Value>,
    pub memory_id: Option<serde_json::Value>,
    pub soul: Option<serde_json::Value>,
    pub owner: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaxRelevanceTags {
    pub setting_tags: Option<serde_json::Value>,
    pub location_tags: Option<serde_json::Value>,
    pub interacted_entities: Option<serde_json::Value>,
    pub event_type_tags: Option<serde_json::Value>,
    pub object_tags: Option<serde_json::Value>,
    pub emotional_tags: Option<serde_json::Value>,
    pub memory_slot_tags: Option<serde_json::Value>,
    pub per_soul_relevance: Option<serde_json::Value>,
}

// Helper Functions
fn rand_str_from_evidence(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn clean_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|value| match value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        serde_json::Value::Number(_) | serde_json::Value::Bool(_) => Some(value.to_string()),
        _ => None,
    })
}

fn payload_string(payload: Option<&serde_json::Value>, key: &str) -> Option<String> {
    payload
        .and_then(|payload| payload.as_object())
        .and_then(|payload| payload.get(key))
        .and_then(|value| clean_value_string(Some(value)))
}

fn joined_action_details(action: Option<String>, details: Option<String>) -> Option<String> {
    match (action, details) {
        (Some(action), Some(details)) => Some(format!("{action}: {details}")),
        (Some(action), None) => Some(action),
        (None, Some(details)) => Some(details),
        (None, None) => None,
    }
}

fn stable_candidate_id(
    owner_or_scope: &str,
    slot: MemorySlot,
    evidence: &str,
    content: &str,
) -> String {
    let source = format!(
        "{}|{}|{}|{}",
        owner_or_scope.trim(),
        slot.as_label(),
        evidence.trim(),
        content.trim()
    );
    format!("mem_norm_{}", rand_str_from_evidence(&source))
}

fn parse_lax_float_with_warning(
    val: Option<&serde_json::Value>,
    field_name: &str,
    warnings: &mut Vec<String>,
) -> Option<f32> {
    let val = val?;
    if val.is_null() {
        return None;
    }
    match val {
        serde_json::Value::Number(n) => {
            let f = n.as_f64()? as f32;
            if f > 1.0 {
                warnings.push(format!(
                    "{field_name} (Number) > 1.0 ({f}) normalized to {}",
                    f / 100.0
                ));
                Some((f / 100.0).clamp(0.0, 1.0))
            } else {
                Some(f.clamp(0.0, 1.0))
            }
        }
        serde_json::Value::String(s) => {
            let s_trim = s.trim();
            if s_trim.ends_with('%') {
                if let Ok(pct) = s_trim.trim_end_matches('%').trim().parse::<f32>() {
                    warnings.push(format!(
                        "{field_name} percentage string {s:?} normalized to {}",
                        pct / 100.0
                    ));
                    return Some((pct / 100.0).clamp(0.0, 1.0));
                }
            }
            if let Ok(f) = s_trim.parse::<f32>() {
                if f > 1.0 {
                    warnings.push(format!(
                        "{field_name} numeric string {s:?} > 1.0 normalized to {}",
                        f / 100.0
                    ));
                    return Some((f / 100.0).clamp(0.0, 1.0));
                } else {
                    warnings.push(format!(
                        "{field_name} numeric string {s:?} normalized to float {f}"
                    ));
                    return Some(f.clamp(0.0, 1.0));
                }
            }
            let s_lower = s_trim.to_lowercase();
            let mapped = match s_lower.as_str() {
                "high" | "strong" | "very_high" | "very_strong" => Some(0.85),
                "medium" | "moderate" | "average" => Some(0.6),
                "low" | "weak" => Some(0.35),
                "none" | "very_low" | "zero" => Some(0.05),
                _ => None,
            };
            if let Some(m) = mapped {
                warnings.push(format!(
                    "{field_name} semantic string {s:?} normalized to {m}"
                ));
            } else {
                warnings.push(format!(
                    "Failed to parse float for field {field_name} from value {val:?}"
                ));
            }
            mapped
        }
        _ => {
            warnings.push(format!(
                "Failed to parse float for field {field_name} from value {val:?}"
            ));
            None
        }
    }
}

fn parse_lax_float_unscaled_with_warning(
    val: Option<&serde_json::Value>,
    field_name: &str,
    warnings: &mut Vec<String>,
) -> Option<f32> {
    let val = val?;
    if val.is_null() {
        return None;
    }
    match val {
        serde_json::Value::Number(n) => {
            let f = n.as_f64()? as f32;
            Some(f)
        }
        serde_json::Value::String(s) => {
            let s_trim = s.trim();
            if s_trim.ends_with('%') {
                if let Ok(pct) = s_trim.trim_end_matches('%').trim().parse::<f32>() {
                    warnings.push(format!(
                        "{field_name} percentage string {s:?} normalized to unscaled {pct}"
                    ));
                    return Some(pct);
                }
            }
            if let Ok(f) = s_trim.parse::<f32>() {
                warnings.push(format!(
                    "{field_name} numeric string {s:?} parsed as float {f}"
                ));
                return Some(f);
            }
            let s_lower = s_trim.to_lowercase();
            let mapped = match s_lower.as_str() {
                "high" | "strong" | "very_high" | "very_strong" => Some(85.0),
                "medium" | "moderate" | "average" => Some(60.0),
                "low" | "weak" => Some(35.0),
                "none" | "very_low" | "zero" => Some(5.0),
                _ => None,
            };
            if let Some(m) = mapped {
                warnings.push(format!(
                    "{field_name} semantic string {s:?} normalized to unscaled {m}"
                ));
            } else {
                warnings.push(format!(
                    "Failed to parse unscaled float for field {field_name} from value {val:?}"
                ));
            }
            mapped
        }
        _ => {
            warnings.push(format!(
                "Failed to parse unscaled float for field {field_name} from value {val:?}"
            ));
            None
        }
    }
}

fn parse_lax_slot(
    val: Option<&serde_json::Value>,
    warnings: &mut Vec<String>,
    path: &str,
) -> MemorySlot {
    let Some(val) = val else {
        return MemorySlot::Unknown;
    };
    let s = match val {
        serde_json::Value::String(s) => s.as_str(),
        _ => {
            warnings.push(format!("{path} slot is not a string, got {val:?}"));
            return MemorySlot::Unknown;
        }
    };
    let s_clean = s.trim().to_ascii_lowercase().replace("-", "_");
    let mapped = if s_clean.contains("relationship") {
        MemorySlot::RelationshipMemory
    } else if s_clean.contains("plot") {
        MemorySlot::CurrentPlotMemory
    } else if s_clean.contains("identity") || s_clean.contains("character") {
        MemorySlot::CharacterIdentityMemory
    } else if s_clean.contains("tension") {
        MemorySlot::UnresolvedTension
    } else if s_clean.contains("location") || s_clean.contains("world") {
        MemorySlot::WorldLocationMemory
    } else if s_clean.contains("emotional") || s_clean.contains("emotion") {
        MemorySlot::RecentEmotionalState
    } else {
        MemorySlot::Unknown
    };

    if mapped != MemorySlot::Unknown {
        let expected = mapped.as_label();
        if s_clean != expected {
            warnings.push(format!("{path} slot alias {s:?} normalized to {expected}"));
        }
    } else {
        warnings.push(format!("{path} unknown memory slot name {s:?}"));
    }
    mapped
}

fn parse_lax_knowledge_scope(
    val: Option<&serde_json::Value>,
    warnings: &mut Vec<String>,
    path: &str,
) -> KnowledgeScope {
    let Some(val) = val else {
        return KnowledgeScope::NotKnown;
    };
    let s = match val {
        serde_json::Value::String(s) => s.as_str(),
        _ => return KnowledgeScope::NotKnown,
    };
    let normalized = s.trim().to_ascii_lowercase().replace("-", "_");
    let mapped = match normalized.as_str() {
        "full" | "observed" | "direct" | "full_observation" => {
            Some(KnowledgeScope::DirectlyObserved)
        }
        "hearsay" => Some(KnowledgeScope::HeardAbout),
        "unknown" | "none" => Some(KnowledgeScope::NotKnown),
        "partial" | "partial_knowledge" => Some(KnowledgeScope::Inferred),
        "directly_observed" => Some(KnowledgeScope::DirectlyObserved),
        "heard_about" => Some(KnowledgeScope::HeardAbout),
        "inferred" => Some(KnowledgeScope::Inferred),
        "not_known" => Some(KnowledgeScope::NotKnown),
        _ => None,
    };
    if let Some(ks) = mapped {
        let expected = ks.as_label();
        if normalized != expected {
            warnings.push(format!(
                "{path} knowledge_scope alias {s:?} normalized to {expected}"
            ));
        }
        ks
    } else {
        warnings.push(format!("{path} unknown knowledge scope {s:?}"));
        KnowledgeScope::NotKnown
    }
}

fn parse_lax_bool(
    val: Option<&serde_json::Value>,
    field_name: &str,
    warnings: &mut Vec<String>,
) -> bool {
    let Some(val) = val else {
        return false;
    };
    match val {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => {
            let is_true = n.as_f64().unwrap_or(0.0) != 0.0;
            warnings.push(format!("{field_name} (Number) parsed as bool: {is_true}"));
            is_true
        }
        serde_json::Value::String(s) => {
            let s_clean = s.trim().to_lowercase();
            let is_true = s_clean == "true" || s_clean == "yes" || s_clean == "1";
            warnings.push(format!(
                "{field_name} string {s:?} parsed as bool: {is_true}"
            ));
            is_true
        }
        _ => false,
    }
}

fn parse_lax_tag_map(
    val: Option<&serde_json::Value>,
    field_name: &str,
    warnings: &mut Vec<String>,
) -> HashMap<String, u8> {
    let mut map = HashMap::new();
    let Some(val) = val else {
        return map;
    };
    match val {
        serde_json::Value::Object(obj) => {
            for (k, v) in obj {
                let u_val = match v {
                    serde_json::Value::Number(n) => n.as_u64().unwrap_or(1) as u8,
                    serde_json::Value::String(s) => s.trim().parse::<u8>().unwrap_or(1),
                    serde_json::Value::Bool(b) => {
                        if *b {
                            1
                        } else {
                            0
                        }
                    }
                    _ => 1,
                };
                map.insert(k.clone(), u_val);
            }
        }
        serde_json::Value::Array(arr) => {
            warnings.push(format!(
                "{field_name} was parsed from Array instead of Object"
            ));
            for item in arr {
                if let Some(s) = item.as_str() {
                    map.insert(s.to_string(), 1);
                }
            }
        }
        serde_json::Value::String(s) => {
            warnings.push(format!(
                "{field_name} was parsed from String instead of Object"
            ));
            map.insert(s.to_string(), 1);
        }
        _ => {}
    }
    map
}

fn parse_lax_string_array_or_keys(
    val: Option<&serde_json::Value>,
    field_name: &str,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let mut vec = Vec::new();
    let Some(val) = val else {
        return vec;
    };
    match val {
        serde_json::Value::Array(arr) => {
            for item in arr {
                match item {
                    serde_json::Value::String(s) => vec.push(s.clone()),
                    other => vec.push(other.to_string()),
                }
            }
        }
        serde_json::Value::Object(obj) => {
            warnings.push(format!("{field_name} was parsed from Object keys"));
            for k in obj.keys() {
                vec.push(k.clone());
            }
        }
        serde_json::Value::String(s) => {
            warnings.push(format!("{field_name} was parsed from String"));
            vec.push(s.clone());
        }
        _ => {}
    }
    vec
}

// Struct-Level Mappers
fn map_turn_classification(
    lax: Option<&LaxTurnClassification>,
    warnings: &mut Vec<String>,
) -> TurnClassification {
    let Some(lax) = lax else {
        return TurnClassification::default();
    };
    TurnClassification {
        is_pure_ooc: parse_lax_bool(
            lax.is_pure_ooc.as_ref(),
            "turn_classification.is_pure_ooc",
            warnings,
        ),
        scene_event_occurred: parse_lax_bool(
            lax.scene_event_occurred.as_ref(),
            "turn_classification.scene_event_occurred",
            warnings,
        ),
        is_retcon_or_correction: parse_lax_bool(
            lax.is_retcon_or_correction.as_ref(),
            "turn_classification.is_retcon_or_correction",
            warnings,
        ),
        human_summary: lax
            .human_summary
            .as_ref()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default(),
    }
}

fn map_global_scene_evaluation(
    lax: Option<&LaxGlobalSceneEvaluation>,
    warnings: &mut Vec<String>,
) -> GlobalSceneEvaluation {
    let Some(lax) = lax else {
        return GlobalSceneEvaluation::default();
    };
    GlobalSceneEvaluation {
        scene_event_occurred: parse_lax_bool(
            lax.scene_event_occurred.as_ref(),
            "global_scene_evaluation.scene_event_occurred",
            warnings,
        ),
        location_changed: parse_lax_bool(
            lax.location_changed.as_ref(),
            "global_scene_evaluation.location_changed",
            warnings,
        ),
        object_state_changed: parse_lax_bool(
            lax.object_state_changed.as_ref(),
            "global_scene_evaluation.object_state_changed",
            warnings,
        ),
        relationship_changed: parse_lax_bool(
            lax.relationship_changed.as_ref(),
            "global_scene_evaluation.relationship_changed",
            warnings,
        ),
        unresolved_tension: parse_lax_bool(
            lax.unresolved_tension.as_ref(),
            "global_scene_evaluation.unresolved_tension",
            warnings,
        ),
        current_plot_advanced: parse_lax_bool(
            lax.current_plot_advanced.as_ref(),
            "global_scene_evaluation.current_plot_advanced",
            warnings,
        ),
        character_identity_changed: parse_lax_bool(
            lax.character_identity_changed.as_ref(),
            "global_scene_evaluation.character_identity_changed",
            warnings,
        ),
        recent_emotional_state_changed: parse_lax_bool(
            lax.recent_emotional_state_changed.as_ref(),
            "global_scene_evaluation.recent_emotional_state_changed",
            warnings,
        ),
        contradiction_detected: parse_lax_bool(
            lax.contradiction_detected.as_ref(),
            "global_scene_evaluation.contradiction_detected",
            warnings,
        ),
        evidence_quote: lax
            .evidence_quote
            .as_ref()
            .and_then(|v| v.as_str().map(|s| s.to_string())),
        summary: lax
            .summary
            .as_ref()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default(),
    }
}

fn map_relevance_tags(
    lax: Option<&LaxRelevanceTags>,
    warnings: &mut Vec<String>,
    path: &str,
) -> RelevanceTags {
    let Some(lax) = lax else {
        return RelevanceTags::default();
    };
    RelevanceTags {
        setting_tags: parse_lax_tag_map(
            lax.setting_tags.as_ref(),
            &format!("{path}.setting_tags"),
            warnings,
        ),
        location_tags: parse_lax_tag_map(
            lax.location_tags.as_ref(),
            &format!("{path}.location_tags"),
            warnings,
        ),
        interacted_entities: parse_lax_tag_map(
            lax.interacted_entities.as_ref(),
            &format!("{path}.interacted_entities"),
            warnings,
        ),
        event_type_tags: parse_lax_tag_map(
            lax.event_type_tags.as_ref(),
            &format!("{path}.event_type_tags"),
            warnings,
        ),
        object_tags: parse_lax_tag_map(
            lax.object_tags.as_ref(),
            &format!("{path}.object_tags"),
            warnings,
        ),
        emotional_tags: parse_lax_tag_map(
            lax.emotional_tags.as_ref(),
            &format!("{path}.emotional_tags"),
            warnings,
        ),
        memory_slot_tags: parse_lax_tag_map(
            lax.memory_slot_tags.as_ref(),
            &format!("{path}.memory_slot_tags"),
            warnings,
        ),
        per_soul_relevance: parse_lax_tag_map(
            lax.per_soul_relevance.as_ref(),
            &format!("{path}.per_soul_relevance"),
            warnings,
        ),
    }
}

fn map_memory_candidate(
    lax: &LaxMemoryCandidate,
    parent_soul_id: Option<&str>,
    warnings: &mut Vec<String>,
    path: &str,
) -> MemoryCandidate {
    // 1. owner_soul_id mapping
    let mut owner = lax
        .owner_soul_id
        .as_ref()
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    if owner.is_none() {
        for alias in [&lax.soul_id, &lax.primary_soul, &lax.soul, &lax.owner] {
            if let Some(val) = alias {
                if let Some(s) = val.as_str() {
                    owner = Some(s.to_string());
                    warnings.push(format!("{path} soul_id normalized to owner_soul_id"));
                    break;
                }
            }
        }
    }
    if owner.is_none() {
        if let Some(target_souls) = &lax.target_souls {
            if let Some(first) = target_souls
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
            {
                owner = Some(first.to_string());
                warnings.push(format!(
                    "{path} target_souls[0] normalized to owner_soul_id"
                ));
            }
        }
    }

    // Inherit from parent soul ID if empty
    let owner_soul_id = match owner {
        Some(o) if !o.trim().is_empty() => o,
        _ => {
            if let Some(parent) = parent_soul_id {
                warnings.push(format!(
                    "{path} owner_soul_id was missing; inherited parent soul_id {parent:?}"
                ));
                parent.to_string()
            } else {
                "".to_string()
            }
        }
    };

    // 2. estimated_strength / confidence / salience / retrieval_strength mapping
    let mut confidence = parse_lax_float_with_warning(
        lax.confidence.as_ref(),
        &format!("{path}.confidence"),
        warnings,
    );
    let mut salience = parse_lax_float_unscaled_with_warning(
        lax.salience.as_ref(),
        &format!("{path}.salience"),
        warnings,
    );
    let mut retrieval_strength = parse_lax_float_unscaled_with_warning(
        lax.retrieval_strength.as_ref(),
        &format!("{path}.retrieval_strength"),
        warnings,
    );

    if let Some(est_val) = &lax.estimated_strength {
        if confidence.is_none() {
            confidence = parse_lax_float_with_warning(
                Some(est_val),
                &format!("{path}.estimated_strength"),
                warnings,
            );
        }
        if salience.is_none() {
            salience = parse_lax_float_unscaled_with_warning(
                Some(est_val),
                &format!("{path}.estimated_strength"),
                warnings,
            );
        }
        if retrieval_strength.is_none() {
            retrieval_strength = parse_lax_float_unscaled_with_warning(
                Some(est_val),
                &format!("{path}.estimated_strength"),
                warnings,
            );
        }
    }

    // 3. slot mapping
    let slot = if lax.slot.is_some() {
        parse_lax_slot(lax.slot.as_ref(), warnings, &format!("{path}.slot"))
    } else {
        let mut found_slot = MemorySlot::Unknown;
        for alias_val in [&lax.proposed_memory_slot, &lax.memory_type] {
            if let Some(val) = alias_val {
                found_slot = parse_lax_slot(Some(val), warnings, &format!("{path}.slot"));
                if found_slot != MemorySlot::Unknown {
                    break;
                }
            }
        }
        if found_slot == MemorySlot::Unknown {
            if let Some(slots) = &lax.slots {
                if let Some(first) = slots.as_array().and_then(|a| a.first()) {
                    found_slot = parse_lax_slot(Some(first), warnings, &format!("{path}.slot"));
                }
            }
        }
        found_slot
    };

    // 4. semantic aliases -> content mapping
    let content = if let Some(c) = lax.content.as_ref().and_then(|v| v.as_str()) {
        c.to_string()
    } else {
        let mut found_content = None;
        if lax.action.is_some() {
            found_content = joined_action_details(
                clean_value_string(lax.action.as_ref()),
                clean_value_string(lax.details.as_ref()),
            );
            if found_content.is_some() {
                warnings.push(format!("{path}.action/details normalized to content"));
            }
        }
        for (key, value) in [
            ("details", &lax.details),
            ("summary", &lax.summary),
            ("specifics", &lax.specifics),
        ] {
            if found_content.is_some() {
                break;
            }
            if let Some(s) = clean_value_string(value.as_ref()) {
                found_content = Some(s);
                warnings.push(format!("{path}.{key} normalized to content"));
                break;
            }
        }
        if found_content.is_none() {
            if let Some(payload) = &lax.payload {
                if let Some(payload_obj) = payload.as_object() {
                    found_content = joined_action_details(
                        payload_string(Some(payload), "action"),
                        payload_string(Some(payload), "details"),
                    );
                    if found_content.is_some() {
                        warnings.push(format!(
                            "{path}.payload.action/details normalized to content"
                        ));
                    }
                    for key in [
                        "content",
                        "details",
                        "summary",
                        "specifics",
                        "interpretation",
                        "action",
                    ] {
                        if found_content.is_some() {
                            break;
                        }
                        if let Some(s) = payload_obj
                            .get(key)
                            .and_then(|v| clean_value_string(Some(v)))
                        {
                            found_content = Some(s);
                            warnings.push(format!("{path}.payload.{key} normalized to content"));
                            break;
                        }
                    }
                } else if let Some(s) = payload.as_str() {
                    found_content = Some(s.to_string());
                    warnings.push(format!("{path}.payload string normalized to content"));
                }
            }
        }
        found_content.unwrap_or_default()
    };

    // 5. actor / target_entity_ids mapping
    let mut target_entity_ids = parse_lax_string_array_or_keys(
        lax.target_entity_ids.as_ref(),
        &format!("{path}.target_entity_ids"),
        warnings,
    );
    if target_entity_ids.is_empty() {
        if let Some(actor) = &lax.actor {
            match actor {
                serde_json::Value::String(s) => {
                    target_entity_ids.push(s.clone());
                    warnings.push(format!("{path}.actor normalized to target_entity_ids"));
                }
                serde_json::Value::Array(arr) => {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            target_entity_ids.push(s.to_string());
                        }
                    }
                    warnings.push(format!("{path}.actor normalized to target_entity_ids"));
                }
                _ => {}
            }
        }
    }

    // 6. tags / relevance_tags mapping
    let mut relevance_tags = parse_lax_string_array_or_keys(
        lax.relevance_tags.as_ref(),
        &format!("{path}.relevance_tags"),
        warnings,
    );
    if relevance_tags.is_empty() {
        if let Some(tags) = &lax.tags {
            relevance_tags =
                parse_lax_string_array_or_keys(Some(tags), &format!("{path}.tags"), warnings);
        }
    }

    // 7. memory_id / candidate_id mapping
    let evidence_quote = lax
        .evidence_quote
        .as_ref()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    let candidate_id = if let Some(cid) = lax.candidate_id.as_ref().and_then(|v| v.as_str()) {
        cid.to_string()
    } else if let Some(mid) = lax.memory_id.as_ref().and_then(|v| v.as_str()) {
        warnings.push(format!("{path}.memory_id normalized to candidate_id"));
        mid.to_string()
    } else {
        let candidate_id = stable_candidate_id(&owner_soul_id, slot, &evidence_quote, &content);
        warnings.push(format!(
            "{path}.candidate_id generated from owner/slot/evidence/content: {candidate_id}"
        ));
        candidate_id
    };

    // 8. source_type
    let source_type = if let Some(st_val) = &lax.source_type {
        match serde_json::from_value::<MemorySourceType>(st_val.clone()) {
            Ok(st) => st,
            Err(_) => {
                if let Some(s) = st_val.as_str() {
                    let s_norm = s.trim().to_ascii_lowercase().replace("-", "_");
                    match s_norm.as_str() {
                        "current_session" => MemorySourceType::CurrentSession,
                        "previous_session" => MemorySourceType::PreviousSession,
                        "imported_log" => MemorySourceType::ImportedLog,
                        "cross_session_bleed" => MemorySourceType::CrossSessionBleed,
                        "user_claimed" => MemorySourceType::UserClaimed,
                        "narrator_inferred" => MemorySourceType::NarratorInferred,
                        "system_generated" => MemorySourceType::SystemGenerated,
                        "persistent_core" => MemorySourceType::PersistentCore,
                        _ => {
                            warnings.push(format!(
                                "{path}.source_type unknown: {s:?}; defaulting to CurrentSession"
                            ));
                            MemorySourceType::CurrentSession
                        }
                    }
                } else {
                    MemorySourceType::CurrentSession
                }
            }
        }
    } else {
        MemorySourceType::CurrentSession
    };

    // 9. truth_status
    let truth_status = if let Some(ts_val) = &lax.truth_status {
        match serde_json::from_value::<TruthStatus>(ts_val.clone()) {
            Ok(ts) => ts,
            Err(_) => {
                if let Some(s) = ts_val.as_str() {
                    let s_norm = s.trim().to_ascii_lowercase().replace("-", "_");
                    match s_norm.as_str() {
                        "fiction" => TruthStatus::Fiction,
                        "scene_event" => TruthStatus::SceneEvent,
                        "character_belief" => TruthStatus::CharacterBelief,
                        "narrator_claim" => TruthStatus::NarratorClaim,
                        "user_claimed" => TruthStatus::UserClaimed,
                        "verified_engine" => TruthStatus::VerifiedEngine,
                        "actual_system_event" => TruthStatus::ActualSystemEvent,
                        _ => {
                            warnings.push(format!(
                                "{path}.truth_status unknown: {s:?}; defaulting to SceneEvent"
                            ));
                            TruthStatus::SceneEvent
                        }
                    }
                } else {
                    TruthStatus::SceneEvent
                }
            }
        }
    } else {
        TruthStatus::SceneEvent
    };

    // 10. knowledge_scope
    let knowledge_scope = parse_lax_knowledge_scope(
        lax.knowledge_scope.as_ref(),
        warnings,
        &format!("{path}.knowledge_scope"),
    );

    MemoryCandidate {
        candidate_id,
        owner_soul_id,
        slot,
        content,
        evidence_quote,
        criterion_met: parse_lax_bool(
            lax.criterion_met.as_ref(),
            &format!("{path}.criterion_met"),
            warnings,
        ),
        confidence: confidence.unwrap_or(0.7),
        salience,
        retrieval_strength,
        perceived_by_entity_id: lax
            .perceived_by_entity_id
            .as_ref()
            .and_then(|v| v.as_str().map(|s| s.to_string())),
        target_entity_ids,
        source_type,
        truth_status,
        relevance_tags,
        knowledge_scope,
    }
}

fn map_object_change(
    lax: &LaxObjectChangeEvaluation,
    warnings: &mut Vec<String>,
    path: &str,
) -> ObjectChangeEvaluation {
    // 1. Get or create object_state value
    let mut state_obj = match &lax.object_state {
        Some(serde_json::Value::Object(obj)) => obj.clone(),
        _ => serde_json::Map::new(),
    };

    // 2. Map top-level aliases to state_obj
    if let Some(obj_val) = &lax.object {
        warnings.push(format!(
            "{path}.object normalized to object_state.object_id"
        ));
        if !state_obj.contains_key("object_id") {
            state_obj.insert("object_id".into(), obj_val.clone());
        }
        if !state_obj.contains_key("object_kind") {
            state_obj.insert("object_kind".into(), obj_val.clone());
        }
    }

    if let Some(change_val) = &lax.change {
        warnings.push(format!(
            "{path}.change normalized to object_state.last_observed_state"
        ));
        if !state_obj.contains_key("last_observed_state") {
            state_obj.insert("last_observed_state".into(), change_val.clone());
        }
    }

    if let Some(prev_val) = &lax.previous_state {
        warnings.push(format!(
            "{path}.previous_state preserved under object_state.properties.previous_state"
        ));
        let mut props_map = match state_obj.get("properties") {
            Some(serde_json::Value::Object(m)) => m.clone(),
            _ => serde_json::Map::new(),
        };
        let prev_str = match prev_val {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        props_map.insert("previous_state".into(), serde_json::Value::String(prev_str));
        state_obj.insert("properties".into(), serde_json::Value::Object(props_map));
    }

    if let Some(ent_val) = &lax.entity_id {
        warnings.push(format!(
            "{path}.entity_id normalized to object_state.owner_entity_id"
        ));
        if !state_obj.contains_key("owner_entity_id") {
            state_obj.insert("owner_entity_id".into(), ent_val.clone());
        }
    }

    // Apply defaults to state_obj
    if !state_obj.contains_key("object_id") {
        state_obj.insert(
            "object_id".into(),
            serde_json::Value::String("unknown_object".to_string()),
        );
    }
    if !state_obj.contains_key("object_kind") {
        state_obj.insert(
            "object_kind".into(),
            serde_json::Value::String("unknown".to_string()),
        );
    }
    if !state_obj.contains_key("status") {
        state_obj.insert(
            "status".into(),
            serde_json::Value::String("unknown".to_string()),
        );
    }
    if !state_obj.contains_key("power_state") {
        state_obj.insert(
            "power_state".into(),
            serde_json::Value::String("unknown".to_string()),
        );
    }
    if !state_obj.contains_key("notification_mode") {
        state_obj.insert(
            "notification_mode".into(),
            serde_json::Value::String("unknown".to_string()),
        );
    }
    if !state_obj.contains_key("last_observed_state") {
        state_obj.insert(
            "last_observed_state".into(),
            serde_json::Value::String("".to_string()),
        );
    }
    if !state_obj.contains_key("location") {
        state_obj.insert("location".into(), serde_json::Value::String("".to_string()));
    }
    if !state_obj.contains_key("confidence") {
        state_obj.insert("confidence".into(), serde_json::json!(0.7));
    }

    // Deserialize state_obj into ObjectState
    let object_state: ObjectState =
        match serde_json::from_value(serde_json::Value::Object(state_obj)) {
            Ok(os) => os,
            Err(e) => {
                warnings.push(format!("Failed to deserialize ObjectState: {e:?}"));
                ObjectState {
                    object_observation_id: None,
                    object_id: "unknown_object".to_string(),
                    object_kind: "unknown".to_string(),
                    owner_entity_id: None,
                    location: "".to_string(),
                    status: "unknown".to_string(),
                    open_state: None,
                    lock_state: None,
                    sealed: None,
                    contents_known: None,
                    contents_summary: None,
                    properties: HashMap::new(),
                    power_state: "unknown".to_string(),
                    notification_mode: "unknown".to_string(),
                    vibrate_enabled: None,
                    screen_wake_enabled: None,
                    can_receive_calls: None,
                    can_receive_texts: None,
                    last_observed_state: "".to_string(),
                    confidence: 0.7,
                }
            }
        };

    let change_id = if let Some(cid) = lax.change_id.as_ref().and_then(|v| v.as_str()) {
        Some(cid.to_string())
    } else {
        let evidence_str = lax
            .evidence_quote
            .as_ref()
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let fb = format!("obj_norm_{}", rand_str_from_evidence(evidence_str));
        warnings.push(format!(
            "{path}.change_id generated from evidence hash: {fb}"
        ));
        Some(fb)
    };

    let confidence = parse_lax_float_with_warning(
        lax.confidence.as_ref(),
        &format!("{path}.confidence"),
        warnings,
    )
    .unwrap_or(0.7);

    let relevance_tags = map_relevance_tags(
        lax.relevance_tags.as_ref(),
        warnings,
        &format!("{path}.relevance_tags"),
    );

    ObjectChangeEvaluation {
        change_id,
        object_state,
        evidence_quote: lax
            .evidence_quote
            .as_ref()
            .and_then(|v| v.as_str().map(|s| s.to_string())),
        confidence,
        relevance_tags,
    }
}

fn map_relationship_evaluation(
    lax: &LaxRelationshipEvaluation,
    warnings: &mut Vec<String>,
    path: &str,
) -> RelationshipEvaluation {
    // 1. Resolve nested Changes/Deltas
    let mut changes_obj = None;
    for changes_alias in [&lax.changes, &lax.deltas] {
        if let Some(serde_json::Value::Object(m)) = changes_alias {
            warnings.push(format!(
                "{path} changes/deltas nested object flattened to relationship evaluation"
            ));
            changes_obj = Some(m.clone());
            break;
        }
    }

    let get_lax_val =
        |field: &str, primary: Option<&serde_json::Value>| -> Option<serde_json::Value> {
            if let Some(v) = primary {
                if !v.is_null() {
                    return Some(v.clone());
                }
            }
            if let Some(changes) = &changes_obj {
                if let Some(v) = changes.get(field) {
                    if !v.is_null() {
                        return Some(v.clone());
                    }
                }
            }
            None
        };

    let parse_field = |field: &str,
                       primary: Option<&serde_json::Value>,
                       warnings: &mut Vec<String>|
     -> Option<f32> {
        let val = get_lax_val(field, primary);
        parse_lax_float_unscaled_with_warning(val.as_ref(), &format!("{path}.{field}"), warnings)
    };

    let trust = parse_field("trust", lax.trust.as_ref(), warnings);
    let affection = parse_field("affection", lax.affection.as_ref(), warnings);
    let intimacy = parse_field("intimacy", lax.intimacy.as_ref(), warnings);
    let passion = parse_field("passion", lax.passion.as_ref(), warnings);
    let commitment = parse_field("commitment", lax.commitment.as_ref(), warnings);
    let fear = parse_field("fear", lax.fear.as_ref(), warnings);
    let desire = parse_field("desire", lax.desire.as_ref(), warnings);
    let respect = parse_field("respect", lax.respect.as_ref(), warnings);
    let conflict = parse_field("conflict", lax.conflict.as_ref(), warnings);
    let dependency = parse_field("dependency", lax.dependency.as_ref(), warnings);
    let curiosity = parse_field("curiosity", lax.curiosity.as_ref(), warnings);
    let comfort = parse_field("comfort", lax.comfort.as_ref(), warnings);
    let boundary_pressure = parse_field(
        "boundary_pressure",
        lax.boundary_pressure.as_ref(),
        warnings,
    );

    // 2. source_soul_id Mapping
    let mut source_soul_id = lax
        .source_soul_id
        .as_ref()
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    if source_soul_id.is_none() {
        for alias in [&lax.soul_id, &lax.source] {
            if let Some(val) = alias {
                if let Some(s) = val.as_str() {
                    source_soul_id = Some(s.to_string());
                    warnings.push(format!("{path} source_soul_id mapped from alias {s:?}"));
                    break;
                }
            }
        }
    }
    let source_soul_id = source_soul_id.unwrap_or_default();

    // 3. target_entity_id Mapping
    let mut target_entity_id = lax
        .target_entity_id
        .as_ref()
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    if target_entity_id.is_none() {
        for alias in [&lax.target, &lax.entity_id, &lax.actor] {
            if let Some(val) = alias {
                if let Some(s) = val.as_str() {
                    target_entity_id = Some(s.to_string());
                    warnings.push(format!("{path} target_entity_id mapped from alias {s:?}"));
                    break;
                }
            }
        }
    }
    let target_entity_id = target_entity_id.unwrap_or_default();

    // 4. evidence quote
    let evidence_quote = lax
        .evidence_quote
        .as_ref()
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    // 5. criterion met
    let criterion_met = if let Some(cm) = &lax.criterion_met {
        parse_lax_bool(Some(cm), &format!("{path}.criterion_met"), warnings)
    } else {
        evidence_quote
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    };

    // 6. confidence
    let confidence = parse_lax_float_with_warning(
        lax.confidence.as_ref(),
        &format!("{path}.confidence"),
        warnings,
    )
    .unwrap_or(0.65);

    // 7. relevance tags
    let relevance_tags = map_relevance_tags(
        lax.relevance_tags.as_ref(),
        warnings,
        &format!("{path}.relevance_tags"),
    );

    RelationshipEvaluation {
        source_soul_id,
        target_entity_id,
        trust,
        affection,
        intimacy,
        passion,
        commitment,
        fear,
        desire,
        respect,
        conflict,
        dependency,
        curiosity,
        comfort,
        boundary_pressure,
        evidence_quote,
        criterion_met,
        confidence,
        relevance_tags,
        evidence_validated_by_form: false,
        ..RelationshipEvaluation::default()
    }
}

fn map_world_change(
    lax: &LaxWorldChangeEvaluation,
    warnings: &mut Vec<String>,
    path: &str,
) -> WorldChangeEvaluation {
    let change_id = if let Some(cid) = lax.change_id.as_ref().and_then(|v| v.as_str()) {
        Some(cid.to_string())
    } else {
        let evidence_str = lax
            .evidence_quote
            .as_ref()
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let fb = format!("world_norm_{}", rand_str_from_evidence(evidence_str));
        warnings.push(format!(
            "{path}.change_id generated from evidence hash: {fb}"
        ));
        Some(fb)
    };

    let confidence = parse_lax_float_with_warning(
        lax.confidence.as_ref(),
        &format!("{path}.confidence"),
        warnings,
    )
    .unwrap_or(0.7);

    let active_plot_add = parse_lax_string_array_or_keys(
        lax.active_plot_add.as_ref(),
        &format!("{path}.active_plot_add"),
        warnings,
    );
    let active_plot_resolve = parse_lax_string_array_or_keys(
        lax.active_plot_resolve.as_ref(),
        &format!("{path}.active_plot_resolve"),
        warnings,
    );

    let scene_state: Option<SceneStatePatch> = if let Some(ss_val) = &lax.scene_state {
        match serde_json::from_value(ss_val.clone()) {
            Ok(ss) => Some(ss),
            Err(e) => {
                warnings.push(format!("{path}.scene_state deserialization failed: {e:?}"));
                None
            }
        }
    } else {
        None
    };

    let relevance_tags = map_relevance_tags(
        lax.relevance_tags.as_ref(),
        warnings,
        &format!("{path}.relevance_tags"),
    );

    let event_summary = lax
        .event_summary
        .as_ref()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .or_else(|| {
            let parts = [
                clean_value_string(lax.change_type.as_ref()),
                clean_value_string(lax.target.as_ref()),
                clean_value_string(lax.new_state.as_ref()),
                clean_value_string(lax.evidence_quote.as_ref()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            (!parts.is_empty()).then(|| {
                warnings.push(format!(
                    "{path}.event_summary synthesized from change_type/target/new_state/evidence"
                ));
                parts.join(": ")
            })
        });

    WorldChangeEvaluation {
        change_id,
        location: lax
            .location
            .as_ref()
            .and_then(|v| v.as_str().map(|s| s.to_string())),
        event_summary,
        scene_state,
        active_plot_add,
        active_plot_resolve,
        evidence_quote: lax
            .evidence_quote
            .as_ref()
            .and_then(|v| v.as_str().map(|s| s.to_string())),
        confidence,
        relevance_tags,
    }
}

fn map_per_soul_evaluation(
    lax: &LaxPerSoulEvaluation,
    warnings: &mut Vec<String>,
    path: &str,
) -> PerSoulEvaluation {
    let mut soul_id_opt = lax
        .soul_id
        .as_ref()
        .and_then(|v| v.as_str().map(|s| s.to_string()));
    if soul_id_opt.is_none() {
        for alias in [&lax.primary_soul, &lax.soul, &lax.owner] {
            if let Some(val) = alias {
                if let Some(s) = val.as_str() {
                    soul_id_opt = Some(s.to_string());
                    warnings.push(format!("{path} soul_id mapped from alias {s:?}"));
                    break;
                }
            }
        }
    }
    let soul_id = soul_id_opt.unwrap_or_default();

    let observed = parse_lax_bool(lax.observed.as_ref(), &format!("{path}.observed"), warnings);

    let knowledge_scope = parse_lax_knowledge_scope(
        lax.knowledge_scope.as_ref(),
        warnings,
        &format!("{path}.knowledge_scope"),
    );

    let relationship_deltas = if let Some(deltas) = &lax.relationship_deltas {
        deltas
            .iter()
            .enumerate()
            .map(|(idx, d)| {
                map_relationship_evaluation(
                    d,
                    warnings,
                    &format!("{path}.relationship_deltas[{idx}]"),
                )
            })
            .collect()
    } else {
        Vec::new()
    };

    let parent_soul_id_ref = if !soul_id.is_empty() {
        Some(soul_id.as_str())
    } else {
        None
    };
    let memory_candidates = if let Some(candidates) = &lax.memory_candidates {
        candidates
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                map_memory_candidate(
                    c,
                    parent_soul_id_ref,
                    warnings,
                    &format!("{path}.memory_candidates[{idx}]"),
                )
            })
            .collect()
    } else {
        Vec::new()
    };

    let relevance_tags = map_relevance_tags(
        lax.relevance_tags.as_ref(),
        warnings,
        &format!("{path}.relevance_tags"),
    );

    PerSoulEvaluation {
        soul_id,
        observed,
        knowledge_scope,
        subjective_interpretation: lax
            .subjective_interpretation
            .as_ref()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default(),
        emotional_state: lax
            .emotional_state
            .as_ref()
            .and_then(|v| v.as_str().map(|s| s.to_string())),
        relationship_deltas,
        memory_candidates,
        relevance_tags,
    }
}

pub fn map_evaluator_output(
    lax: LaxEvaluatorOutput,
    warnings: &mut Vec<String>,
) -> EvaluatorOutputV1 {
    let schema_version = lax
        .schema_version
        .as_ref()
        .and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_u64().map(|x| x as u32),
            serde_json::Value::String(s) => s.trim().parse::<u32>().ok(),
            _ => None,
        })
        .unwrap_or(1);

    let thought_process = lax
        .thought_process
        .as_ref()
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let turn_flags_u64 = lax
        .turn_flags_u64
        .as_ref()
        .and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_u64(),
            serde_json::Value::String(s) => s.trim().parse::<u64>().ok(),
            _ => None,
        })
        .unwrap_or(0);

    let turn_classification = map_turn_classification(lax.turn_classification.as_ref(), warnings);

    let global_scene_evaluation =
        map_global_scene_evaluation(lax.global_scene_evaluation.as_ref(), warnings);

    let per_soul_evaluations = if let Some(pse) = &lax.per_soul_evaluations {
        pse.iter()
            .enumerate()
            .map(|(idx, se)| {
                map_per_soul_evaluation(se, warnings, &format!("per_soul_evaluations[{idx}]"))
            })
            .collect()
    } else {
        Vec::new()
    };

    let world_changes = if let Some(wc) = &lax.world_changes {
        wc.iter()
            .enumerate()
            .map(|(idx, c)| map_world_change(c, warnings, &format!("world_changes[{idx}]")))
            .collect()
    } else {
        Vec::new()
    };

    let object_changes = if let Some(oc) = &lax.object_changes {
        oc.iter()
            .enumerate()
            .map(|(idx, c)| map_object_change(c, warnings, &format!("object_changes[{idx}]")))
            .collect()
    } else {
        Vec::new()
    };

    let relationship_evaluations = if let Some(re) = &lax.relationship_evaluations {
        re.iter()
            .enumerate()
            .map(|(idx, e)| {
                map_relationship_evaluation(
                    e,
                    warnings,
                    &format!("relationship_evaluations[{idx}]"),
                )
            })
            .collect()
    } else {
        Vec::new()
    };

    let memory_candidates = if let Some(mc) = &lax.memory_candidates {
        mc.iter()
            .enumerate()
            .map(|(idx, c)| {
                map_memory_candidate(c, None, warnings, &format!("memory_candidates[{idx}]"))
            })
            .collect()
    } else {
        Vec::new()
    };

    let relevance_tags =
        map_relevance_tags(lax.relevance_tags.as_ref(), warnings, "relevance_tags");

    let no_op_reason = lax
        .no_op_reason
        .as_ref()
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    EvaluatorOutputV1 {
        schema_version,
        thought_process,
        turn_flags_u64,
        turn_classification,
        global_scene_evaluation,
        per_soul_evaluations,
        world_changes,
        object_changes,
        relationship_evaluations,
        memory_candidates,
        relevance_tags,
        no_op_reason,
    }
}

fn is_active_soul(context: Option<&EvaluatorDraftContext>, entity_id: &str) -> bool {
    let id = entity_id.trim();
    !id.is_empty()
        && context
            .map(|context| context.active_soul_ids.iter().any(|active| active == id))
            .unwrap_or(true)
}

fn has_world_effect(output: &EvaluatorOutputV1) -> bool {
    output.world_changes.iter().any(|change| {
        change
            .event_summary
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
            || change.scene_state.is_some()
            || !change.active_plot_add.is_empty()
            || !change.active_plot_resolve.is_empty()
    })
}

fn scene_turn_detected(output: &EvaluatorOutputV1) -> bool {
    output.turn_classification.scene_event_occurred
        || output.global_scene_evaluation.scene_event_occurred
        || output.global_scene_evaluation.current_plot_advanced
        || output.turn_flags_u64 & crate::evaluator::turn_flags::SCENE_TURN != 0
        || output.turn_flags_u64 & crate::evaluator::turn_flags::CURRENT_PLOT_ADVANCED != 0
}

fn minimal_scene_state(context: &EvaluatorDraftContext, summary: &str) -> SceneStatePatch {
    let current_scene = if summary.trim().is_empty() {
        format!(
            "{} and default_player are continuing the current scene.",
            context.active_soul_display_name
        )
    } else {
        summary.trim().to_string()
    };
    SceneStatePatch {
        scene_state_id: Some(stable_candidate_id(
            "session_world",
            MemorySlot::CurrentPlotMemory,
            &context.latest_user_message,
            &current_scene,
        )),
        current_scene: Some(current_scene),
        focus: Some(format!(
            "{} and default_player",
            context.active_soul_display_name
        )),
        participants: vec![context.active_soul_id.clone(), "default_player".into()],
        last_user_action: (!context.latest_user_message.trim().is_empty())
            .then(|| context.latest_user_message.trim().to_string()),
        continuity_note: Some("The scene advanced on the latest user action.".into()),
        ..SceneStatePatch::default()
    }
}

fn route_top_level_world_memories(
    output: &mut EvaluatorOutputV1,
    draft: &mut NormalizedEvaluationDraft,
    context: Option<&EvaluatorDraftContext>,
) {
    let Some(context) = context else {
        return;
    };
    let mut retained = Vec::new();
    for candidate in output.memory_candidates.drain(..) {
        let owner = candidate.owner_soul_id.trim();
        let should_route = owner.is_empty() || owner == "default_player";
        if should_route && !candidate.content.trim().is_empty() {
            draft.candidate_routing_decisions.push(format!(
                "{} routed to SessionWorld recent_event because owner was {:?}",
                candidate.candidate_id, owner
            ));
            output.world_changes.push(WorldChangeEvaluation {
                change_id: Some(stable_candidate_id(
                    if owner.is_empty() {
                        "session_world"
                    } else {
                        owner
                    },
                    candidate.slot,
                    &candidate.evidence_quote,
                    &candidate.content,
                )),
                event_summary: Some(candidate.content.clone()),
                evidence_quote: (!candidate.evidence_quote.trim().is_empty())
                    .then(|| candidate.evidence_quote.clone())
                    .or_else(|| Some(context.latest_user_message.clone())),
                confidence: candidate.confidence.max(0.6),
                ..WorldChangeEvaluation::default()
            });
        } else {
            if candidate.content.trim().is_empty() {
                draft.candidate_quality_decisions.push(format!(
                    "{} marked low quality: empty content",
                    candidate.candidate_id
                ));
            }
            retained.push(candidate);
        }
    }
    output.memory_candidates = retained;
}

fn add_relationship_delta_from_memory(
    output: &mut EvaluatorOutputV1,
    lax: &LaxMemoryCandidate,
    candidate: &MemoryCandidate,
    draft: &mut NormalizedEvaluationDraft,
    context: Option<&EvaluatorDraftContext>,
    path: &str,
) {
    let relation_value = lax
        .relationship_delta
        .as_ref()
        .or(lax.changes.as_ref())
        .or(lax.deltas.as_ref());
    let Some(relation_value) = relation_value else {
        return;
    };
    if !is_active_soul(context, &candidate.owner_soul_id) {
        draft.candidate_routing_decisions.push(format!(
            "{} relationship_delta ignored because owner is not an active Soul",
            candidate.candidate_id
        ));
        return;
    }
    let mut relation = serde_json::from_value::<LaxRelationshipEvaluation>(relation_value.clone())
        .unwrap_or_else(|_| LaxRelationshipEvaluation {
            changes: Some(relation_value.clone()),
            ..LaxRelationshipEvaluation::default()
        });
    relation.source_soul_id = Some(serde_json::Value::String(candidate.owner_soul_id.clone()));
    relation.evidence_quote = Some(serde_json::Value::String(candidate.evidence_quote.clone()));
    if relation.target_entity_id.is_none() && relation.target.is_none() && relation.actor.is_none()
    {
        relation.target_entity_id = clean_value_string(lax.target.as_ref())
            .or_else(|| clean_value_string(lax.actor.as_ref()))
            .or_else(|| Some("default_player".into()))
            .map(serde_json::Value::String);
    }
    relation.criterion_met = Some(serde_json::Value::Bool(
        !candidate.owner_soul_id.trim().is_empty()
            && !candidate.evidence_quote.trim().is_empty()
            && relation.target_entity_id.is_some(),
    ));
    let mut warnings = Vec::new();
    let mapped = map_relationship_evaluation(&relation, &mut warnings, path);
    draft.warnings.extend(warnings);
    output.relationship_evaluations.push(mapped);
}

fn add_recent_emotional_state_candidate(
    output: &mut EvaluatorOutputV1,
    draft: &mut NormalizedEvaluationDraft,
    context: Option<&EvaluatorDraftContext>,
) {
    if !output
        .global_scene_evaluation
        .recent_emotional_state_changed
    {
        return;
    }
    let Some(context) = context else {
        return;
    };
    for per_soul in &mut output.per_soul_evaluations {
        if per_soul.soul_id != context.active_soul_id {
            continue;
        }
        let Some(emotional_state) = per_soul
            .emotional_state
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(evidence) = output
            .global_scene_evaluation
            .evidence_quote
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            draft
                .candidate_quality_decisions
                .push("recent emotional state rejected: missing evidence_quote".into());
            continue;
        };
        if emotional_state.len() < 12 || emotional_state.eq_ignore_ascii_case("tense") {
            draft.candidate_quality_decisions.push(
                "recent emotional state rejected: generic emotional/body-language content".into(),
            );
            continue;
        }
        let content = format!(
            "{} recent emotional state: {}",
            context.active_soul_display_name, emotional_state
        );
        per_soul.memory_candidates.push(MemoryCandidate {
            candidate_id: stable_candidate_id(
                &context.active_soul_id,
                MemorySlot::RecentEmotionalState,
                evidence,
                &content,
            ),
            owner_soul_id: context.active_soul_id.clone(),
            slot: MemorySlot::RecentEmotionalState,
            content,
            evidence_quote: evidence.to_string(),
            criterion_met: true,
            confidence: 0.7,
            salience: Some(50.0),
            retrieval_strength: Some(45.0),
            perceived_by_entity_id: Some(context.active_soul_id.clone()),
            target_entity_ids: vec!["default_player".into()],
            source_type: MemorySourceType::CurrentSession,
            truth_status: TruthStatus::SceneEvent,
            relevance_tags: vec!["recent_emotional_state".into()],
            knowledge_scope: KnowledgeScope::DirectlyObserved,
        });
        draft
            .candidate_quality_decisions
            .push("recent emotional state candidate created with evidence".into());
    }
}

fn create_normalized_draft(
    lax: &LaxEvaluatorOutput,
    output: &mut EvaluatorOutputV1,
    warnings: &[String],
    context: Option<&EvaluatorDraftContext>,
) -> NormalizedEvaluationDraft {
    let mut draft = NormalizedEvaluationDraft {
        scene_evaluation: output.global_scene_evaluation.clone(),
        warnings: warnings.to_vec(),
        ..NormalizedEvaluationDraft::default()
    };

    route_top_level_world_memories(output, &mut draft, context);

    if let Some(memories) = &lax.memory_candidates {
        for (idx, lax_candidate) in memories.iter().enumerate() {
            if let Some(candidate) = output.memory_candidates.get(idx).cloned() {
                add_relationship_delta_from_memory(
                    output,
                    lax_candidate,
                    &candidate,
                    &mut draft,
                    context,
                    &format!("memory_candidates[{idx}].relationship_delta"),
                );
            }
        }
    }
    if let Some(per_soul_lax) = &lax.per_soul_evaluations {
        for (soul_idx, lax_soul) in per_soul_lax.iter().enumerate() {
            if let Some(lax_memories) = &lax_soul.memory_candidates {
                let candidates = output
                    .per_soul_evaluations
                    .get(soul_idx)
                    .map(|soul| soul.memory_candidates.clone())
                    .unwrap_or_default();
                for (idx, lax_candidate) in lax_memories.iter().enumerate() {
                    if let Some(candidate) = candidates.get(idx) {
                        add_relationship_delta_from_memory(
                            output,
                            lax_candidate,
                            candidate,
                            &mut draft,
                            context,
                            &format!("per_soul_evaluations[{soul_idx}].memory_candidates[{idx}].relationship_delta"),
                        );
                    }
                }
            }
        }
    }

    add_recent_emotional_state_candidate(output, &mut draft, context);

    if scene_turn_detected(output) && !has_world_effect(output) {
        if let Some(context) = context {
            let summary = output.global_scene_evaluation.summary.trim().to_string();
            let fallback_summary = if summary.is_empty() {
                context.latest_user_message.clone()
            } else {
                summary
            };
            output.world_changes.push(WorldChangeEvaluation {
                change_id: Some(stable_candidate_id(
                    "session_world",
                    MemorySlot::CurrentPlotMemory,
                    output
                        .global_scene_evaluation
                        .evidence_quote
                        .as_deref()
                        .unwrap_or(""),
                    &fallback_summary,
                )),
                event_summary: Some(fallback_summary.clone()),
                scene_state: Some(minimal_scene_state(context, &fallback_summary)),
                evidence_quote: output
                    .global_scene_evaluation
                    .evidence_quote
                    .clone()
                    .or_else(|| Some(context.latest_user_message.clone())),
                confidence: 0.7,
                ..WorldChangeEvaluation::default()
            });
            draft.state_effect_guarantee_applied = true;
            draft.state_effect_guarantee_reason = Some(
                "scene turn had no usable world effect; synthesized recent_event and scene_state"
                    .into(),
            );
        }
    }

    draft.memory_candidate_count = output.memory_candidates.len()
        + output
            .per_soul_evaluations
            .iter()
            .map(|soul| soul.memory_candidates.len())
            .sum::<usize>();
    draft.world_event_count = output.world_changes.len();
    draft.scene_state_present = output
        .world_changes
        .iter()
        .any(|change| change.scene_state.is_some());
    draft.relationship_delta_count = output.relationship_evaluations.len()
        + output
            .per_soul_evaluations
            .iter()
            .map(|soul| soul.relationship_deltas.len())
            .sum::<usize>();
    draft.per_soul_interpretation_count = output
        .per_soul_evaluations
        .iter()
        .filter(|soul| !soul.subjective_interpretation.trim().is_empty())
        .count();
    draft.object_observation_count = output.object_changes.len();
    draft
}

pub fn parse_evaluator_output(raw_json: &str) -> Result<EvaluatorParseResult, String> {
    parse_evaluator_output_with_context(raw_json, None)
}

pub fn parse_evaluator_output_with_context(
    raw_json: &str,
    context: Option<&EvaluatorDraftContext>,
) -> Result<EvaluatorParseResult, String> {
    let trimmed = raw_json.trim();
    let json = if let Some(stripped) = trimmed.strip_prefix("```json") {
        stripped.trim_end_matches("```").trim()
    } else if let Some(stripped) = trimmed.strip_prefix("```") {
        stripped.trim_end_matches("```").trim()
    } else {
        trimmed
    };

    let mut warnings = Vec::new();
    let lax_output: LaxEvaluatorOutput = serde_json::from_str(json)
        .map_err(|err| format!("Evaluator returned invalid LaxEvaluatorOutput JSON: {err}"))?;

    let mut output = map_evaluator_output(lax_output.clone(), &mut warnings);
    let draft = create_normalized_draft(&lax_output, &mut output, &warnings, context);

    let normalized = !warnings.is_empty()
        || draft.state_effect_guarantee_applied
        || !draft.candidate_routing_decisions.is_empty()
        || !draft.candidate_quality_decisions.is_empty();
    let normalized_json = serde_json::to_string(&output)
        .map_err(|err| format!("Evaluator output serialization failed: {err}"))?;

    Ok(EvaluatorParseResult {
        output,
        draft,
        normalized_json,
        normalized,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> EvaluatorDraftContext {
        EvaluatorDraftContext {
            active_soul_id: "aurora_soul".into(),
            active_soul_display_name: "Aurora Schwarz".into(),
            active_soul_ids: vec!["aurora_soul".into()],
            latest_user_message: "I walk in. Long time no see, Aurora.".into(),
        }
    }

    fn base_json(memory: serde_json::Value) -> String {
        serde_json::json!({
            "schema_version": 1,
            "turn_flags_u64": 1,
            "turn_classification": {
                "scene_event_occurred": true
            },
            "global_scene_evaluation": {
                "scene_event_occurred": true,
                "evidence_quote": "I walk in",
                "summary": "The visitor entered Aurora's apartment."
            },
            "per_soul_evaluations": [],
            "world_changes": [],
            "object_changes": [],
            "relationship_evaluations": [],
            "memory_candidates": [memory],
            "relevance_tags": {},
            "no_op_reason": null
        })
        .to_string()
    }

    #[test]
    fn details_maps_to_memory_content() {
        let raw = base_json(serde_json::json!({
            "owner_soul_id": "aurora_soul",
            "slot": "relationship_memory",
            "details": "Aurora noticed the visitor came inside.",
            "evidence_quote": "I walk in",
            "criterion_met": true
        }));
        let parsed = parse_evaluator_output_with_context(&raw, Some(&ctx())).unwrap();
        assert_eq!(
            parsed.output.memory_candidates[0].content,
            "Aurora noticed the visitor came inside."
        );
    }

    #[test]
    fn action_details_maps_to_memory_content() {
        let raw = base_json(serde_json::json!({
            "owner_soul_id": "aurora_soul",
            "slot": "relationship_memory",
            "action": "visitor entered",
            "details": "Aurora let them in after the doorway greeting.",
            "evidence_quote": "I walk in",
            "criterion_met": true
        }));
        let parsed = parse_evaluator_output_with_context(&raw, Some(&ctx())).unwrap();
        assert!(parsed.output.memory_candidates[0]
            .content
            .contains("visitor entered"));
    }

    #[test]
    fn default_player_memory_routes_to_session_world_not_soul() {
        let raw = base_json(serde_json::json!({
            "owner_soul_id": "default_player",
            "slot": "current_plot_memory",
            "content": "The visitor entered Aurora's apartment.",
            "evidence_quote": "I walk in",
            "criterion_met": true
        }));
        let parsed = parse_evaluator_output_with_context(&raw, Some(&ctx())).unwrap();
        assert!(parsed.output.memory_candidates.is_empty());
        assert_eq!(
            parsed.output.world_changes[0].event_summary.as_deref(),
            Some("The visitor entered Aurora's apartment.")
        );
    }

    #[test]
    fn active_soul_nested_candidate_inherits_owner() {
        let raw = serde_json::json!({
            "schema_version": 1,
            "per_soul_evaluations": [{
                "soul_id": "aurora_soul",
                "memory_candidates": [{
                    "slot": "relationship_memory",
                    "content": "Aurora recognized the visitor's return.",
                    "evidence_quote": "Long time no see",
                    "criterion_met": true
                }]
            }]
        })
        .to_string();
        let parsed = parse_evaluator_output_with_context(&raw, Some(&ctx())).unwrap();
        assert_eq!(
            parsed.output.per_soul_evaluations[0].memory_candidates[0].owner_soul_id,
            "aurora_soul"
        );
    }

    #[test]
    fn candidate_id_includes_owner_slot_evidence_content_when_generated() {
        let first = serde_json::json!({
            "owner_soul_id": "aurora_soul",
            "slot": "relationship_memory",
            "content": "Same quote, Aurora owner.",
            "evidence_quote": "same",
            "criterion_met": true
        });
        let mut second = first.clone();
        second["owner_soul_id"] = serde_json::json!("other_soul");
        let first = parse_evaluator_output(&base_json(first)).unwrap();
        let second = parse_evaluator_output(&base_json(second)).unwrap();
        assert_ne!(
            first.output.memory_candidates[0].candidate_id,
            second.output.memory_candidates[0].candidate_id
        );
    }

    #[test]
    fn relationship_delta_inherits_candidate_evidence() {
        let raw = base_json(serde_json::json!({
            "owner_soul_id": "aurora_soul",
            "slot": "relationship_memory",
            "content": "Aurora felt more comfortable with the visitor.",
            "evidence_quote": "Long time no see",
            "criterion_met": true,
            "relationship_delta": {
                "target": "default_player",
                "changes": { "comfort": 2.0, "curiosity": 1.0 }
            }
        }));
        let parsed = parse_evaluator_output_with_context(&raw, Some(&ctx())).unwrap();
        let relation = &parsed.output.relationship_evaluations[0];
        assert_eq!(relation.source_soul_id, "aurora_soul");
        assert_eq!(relation.evidence_quote.as_deref(), Some("Long time no see"));
        assert_eq!(relation.comfort, Some(2.0));
    }

    #[test]
    fn scene_turn_synthesizes_minimal_scene_state() {
        let raw = serde_json::json!({
            "schema_version": 1,
            "turn_classification": { "scene_event_occurred": true },
            "global_scene_evaluation": {
                "scene_event_occurred": true,
                "summary": "The visitor entered Aurora's apartment.",
                "evidence_quote": "I walk in"
            }
        })
        .to_string();
        let parsed = parse_evaluator_output_with_context(&raw, Some(&ctx())).unwrap();
        assert!(parsed.draft.state_effect_guarantee_applied);
        assert!(parsed.output.world_changes[0].scene_state.is_some());
    }

    #[test]
    fn recent_emotional_state_candidate_requires_evidence() {
        let raw = serde_json::json!({
            "schema_version": 1,
            "global_scene_evaluation": {
                "recent_emotional_state_changed": true
            },
            "per_soul_evaluations": [{
                "soul_id": "aurora_soul",
                "emotional_state": "playfully engaged after the visitor entered"
            }]
        })
        .to_string();
        let parsed = parse_evaluator_output_with_context(&raw, Some(&ctx())).unwrap();
        assert!(parsed.output.per_soul_evaluations[0]
            .memory_candidates
            .is_empty());
        assert!(parsed
            .draft
            .candidate_quality_decisions
            .iter()
            .any(|decision| decision.contains("missing evidence_quote")));
    }
}
