use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::evaluator::{
    EvaluatorOutputV1, TurnClassification, GlobalSceneEvaluation, PerSoulEvaluation,
    KnowledgeScope, WorldChangeEvaluation, ObjectChangeEvaluation, RelationshipEvaluation,
    MemorySlot, MemoryCandidate, RelevanceTags,
};
use crate::patch::SceneStatePatch;
use crate::soul::{MemorySourceType, TruthStatus, ObjectState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorParseResult {
    pub output: EvaluatorOutputV1,
    pub normalized_json: String,
    pub normalized: bool,
    pub warnings: Vec<String>,
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

#[derive(Debug, Clone, Deserialize)]
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
                warnings.push(format!("{field_name} (Number) > 1.0 ({f}) normalized to {}", f / 100.0));
                Some((f / 100.0).clamp(0.0, 1.0))
            } else {
                Some(f.clamp(0.0, 1.0))
            }
        }
        serde_json::Value::String(s) => {
            let s_trim = s.trim();
            if s_trim.ends_with('%') {
                if let Ok(pct) = s_trim.trim_end_matches('%').trim().parse::<f32>() {
                    warnings.push(format!("{field_name} percentage string {s:?} normalized to {}", pct / 100.0));
                    return Some((pct / 100.0).clamp(0.0, 1.0));
                }
            }
            if let Ok(f) = s_trim.parse::<f32>() {
                if f > 1.0 {
                    warnings.push(format!("{field_name} numeric string {s:?} > 1.0 normalized to {}", f / 100.0));
                    return Some((f / 100.0).clamp(0.0, 1.0));
                } else {
                    warnings.push(format!("{field_name} numeric string {s:?} normalized to float {f}"));
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
                warnings.push(format!("{field_name} semantic string {s:?} normalized to {m}"));
            } else {
                warnings.push(format!("Failed to parse float for field {field_name} from value {val:?}"));
            }
            mapped
        }
        _ => {
            warnings.push(format!("Failed to parse float for field {field_name} from value {val:?}"));
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
                    warnings.push(format!("{field_name} percentage string {s:?} normalized to unscaled {pct}"));
                    return Some(pct);
                }
            }
            if let Ok(f) = s_trim.parse::<f32>() {
                warnings.push(format!("{field_name} numeric string {s:?} parsed as float {f}"));
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
                warnings.push(format!("{field_name} semantic string {s:?} normalized to unscaled {m}"));
            } else {
                warnings.push(format!("Failed to parse unscaled float for field {field_name} from value {val:?}"));
            }
            mapped
        }
        _ => {
            warnings.push(format!("Failed to parse unscaled float for field {field_name} from value {val:?}"));
            None
        }
    }
}

fn parse_lax_slot(val: Option<&serde_json::Value>, warnings: &mut Vec<String>, path: &str) -> MemorySlot {
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

fn parse_lax_knowledge_scope(val: Option<&serde_json::Value>, warnings: &mut Vec<String>, path: &str) -> KnowledgeScope {
    let Some(val) = val else {
        return KnowledgeScope::NotKnown;
    };
    let s = match val {
        serde_json::Value::String(s) => s.as_str(),
        _ => return KnowledgeScope::NotKnown,
    };
    let normalized = s.trim().to_ascii_lowercase().replace("-", "_");
    let mapped = match normalized.as_str() {
        "full" | "observed" | "direct" | "full_observation" => Some(KnowledgeScope::DirectlyObserved),
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
            warnings.push(format!("{path} knowledge_scope alias {s:?} normalized to {expected}"));
        }
        ks
    } else {
        warnings.push(format!("{path} unknown knowledge scope {s:?}"));
        KnowledgeScope::NotKnown
    }
}

fn parse_lax_bool(val: Option<&serde_json::Value>, field_name: &str, warnings: &mut Vec<String>) -> bool {
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
            warnings.push(format!("{field_name} string {s:?} parsed as bool: {is_true}"));
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
                    serde_json::Value::Bool(b) => if *b { 1 } else { 0 },
                    _ => 1,
                };
                map.insert(k.clone(), u_val);
            }
        }
        serde_json::Value::Array(arr) => {
            warnings.push(format!("{field_name} was parsed from Array instead of Object"));
            for item in arr {
                if let Some(s) = item.as_str() {
                    map.insert(s.to_string(), 1);
                }
            }
        }
        serde_json::Value::String(s) => {
            warnings.push(format!("{field_name} was parsed from String instead of Object"));
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
        is_pure_ooc: parse_lax_bool(lax.is_pure_ooc.as_ref(), "turn_classification.is_pure_ooc", warnings),
        scene_event_occurred: parse_lax_bool(lax.scene_event_occurred.as_ref(), "turn_classification.scene_event_occurred", warnings),
        is_retcon_or_correction: parse_lax_bool(lax.is_retcon_or_correction.as_ref(), "turn_classification.is_retcon_or_correction", warnings),
        human_summary: lax.human_summary.as_ref()
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
        scene_event_occurred: parse_lax_bool(lax.scene_event_occurred.as_ref(), "global_scene_evaluation.scene_event_occurred", warnings),
        location_changed: parse_lax_bool(lax.location_changed.as_ref(), "global_scene_evaluation.location_changed", warnings),
        object_state_changed: parse_lax_bool(lax.object_state_changed.as_ref(), "global_scene_evaluation.object_state_changed", warnings),
        relationship_changed: parse_lax_bool(lax.relationship_changed.as_ref(), "global_scene_evaluation.relationship_changed", warnings),
        unresolved_tension: parse_lax_bool(lax.unresolved_tension.as_ref(), "global_scene_evaluation.unresolved_tension", warnings),
        current_plot_advanced: parse_lax_bool(lax.current_plot_advanced.as_ref(), "global_scene_evaluation.current_plot_advanced", warnings),
        character_identity_changed: parse_lax_bool(lax.character_identity_changed.as_ref(), "global_scene_evaluation.character_identity_changed", warnings),
        recent_emotional_state_changed: parse_lax_bool(lax.recent_emotional_state_changed.as_ref(), "global_scene_evaluation.recent_emotional_state_changed", warnings),
        contradiction_detected: parse_lax_bool(lax.contradiction_detected.as_ref(), "global_scene_evaluation.contradiction_detected", warnings),
        evidence_quote: lax.evidence_quote.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())),
        summary: lax.summary.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default(),
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
        setting_tags: parse_lax_tag_map(lax.setting_tags.as_ref(), &format!("{path}.setting_tags"), warnings),
        location_tags: parse_lax_tag_map(lax.location_tags.as_ref(), &format!("{path}.location_tags"), warnings),
        interacted_entities: parse_lax_tag_map(lax.interacted_entities.as_ref(), &format!("{path}.interacted_entities"), warnings),
        event_type_tags: parse_lax_tag_map(lax.event_type_tags.as_ref(), &format!("{path}.event_type_tags"), warnings),
        object_tags: parse_lax_tag_map(lax.object_tags.as_ref(), &format!("{path}.object_tags"), warnings),
        emotional_tags: parse_lax_tag_map(lax.emotional_tags.as_ref(), &format!("{path}.emotional_tags"), warnings),
        memory_slot_tags: parse_lax_tag_map(lax.memory_slot_tags.as_ref(), &format!("{path}.memory_slot_tags"), warnings),
        per_soul_relevance: parse_lax_tag_map(lax.per_soul_relevance.as_ref(), &format!("{path}.per_soul_relevance"), warnings),
    }
}

fn map_memory_candidate(
    lax: &LaxMemoryCandidate,
    parent_soul_id: Option<&str>,
    warnings: &mut Vec<String>,
    path: &str,
) -> MemoryCandidate {
    // 1. owner_soul_id mapping
    let mut owner = lax.owner_soul_id.as_ref()
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
            if let Some(first) = target_souls.as_array().and_then(|a| a.first()).and_then(|v| v.as_str()) {
                owner = Some(first.to_string());
                warnings.push(format!("{path} target_souls[0] normalized to owner_soul_id"));
            }
        }
    }
    
    // Inherit from parent soul ID if empty
    let owner_soul_id = match owner {
        Some(o) if !o.trim().is_empty() => o,
        _ => {
            if let Some(parent) = parent_soul_id {
                warnings.push(format!("{path} owner_soul_id was missing; inherited parent soul_id {parent:?}"));
                parent.to_string()
            } else {
                "".to_string()
            }
        }
    };

    // 2. estimated_strength / confidence / salience / retrieval_strength mapping
    let mut confidence = parse_lax_float_with_warning(lax.confidence.as_ref(), &format!("{path}.confidence"), warnings);
    let mut salience = parse_lax_float_unscaled_with_warning(lax.salience.as_ref(), &format!("{path}.salience"), warnings);
    let mut retrieval_strength = parse_lax_float_unscaled_with_warning(lax.retrieval_strength.as_ref(), &format!("{path}.retrieval_strength"), warnings);

    if let Some(est_val) = &lax.estimated_strength {
        if confidence.is_none() {
            confidence = parse_lax_float_with_warning(Some(est_val), &format!("{path}.estimated_strength"), warnings);
        }
        if salience.is_none() {
            salience = parse_lax_float_unscaled_with_warning(Some(est_val), &format!("{path}.estimated_strength"), warnings);
        }
        if retrieval_strength.is_none() {
            retrieval_strength = parse_lax_float_unscaled_with_warning(Some(est_val), &format!("{path}.estimated_strength"), warnings);
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

    // 4. specifics / payload -> content mapping
    let content = if let Some(c) = lax.content.as_ref().and_then(|v| v.as_str()) {
        c.to_string()
    } else {
        let mut found_content = None;
        if let Some(specifics) = &lax.specifics {
            if let Some(s) = specifics.as_str() {
                found_content = Some(s.to_string());
                warnings.push(format!("{path}.specifics normalized to content"));
            }
        }
        if found_content.is_none() {
            if let Some(payload) = &lax.payload {
                if let Some(payload_obj) = payload.as_object() {
                    for key in ["action", "interpretation", "content", "specifics"] {
                        if let Some(val) = payload_obj.get(key) {
                            if let Some(s) = val.as_str() {
                                found_content = Some(s.to_string());
                                warnings.push(format!("{path}.payload.{key} normalized to content"));
                                break;
                            }
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
    let mut target_entity_ids = parse_lax_string_array_or_keys(lax.target_entity_ids.as_ref(), &format!("{path}.target_entity_ids"), warnings);
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
    let mut relevance_tags = parse_lax_string_array_or_keys(lax.relevance_tags.as_ref(), &format!("{path}.relevance_tags"), warnings);
    if relevance_tags.is_empty() {
        if let Some(tags) = &lax.tags {
            relevance_tags = parse_lax_string_array_or_keys(Some(tags), &format!("{path}.tags"), warnings);
        }
    }

    // 7. memory_id / candidate_id mapping
    let candidate_id = if let Some(cid) = lax.candidate_id.as_ref().and_then(|v| v.as_str()) {
        cid.to_string()
    } else if let Some(mid) = lax.memory_id.as_ref().and_then(|v| v.as_str()) {
        warnings.push(format!("{path}.memory_id normalized to candidate_id"));
        mid.to_string()
    } else {
        let evidence_str = lax.evidence_quote.as_ref().and_then(|v| v.as_str()).unwrap_or("");
        let fb = format!("mem_norm_{}", rand_str_from_evidence(evidence_str));
        warnings.push(format!("{path}.candidate_id generated from evidence hash: {fb}"));
        fb
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
                            warnings.push(format!("{path}.source_type unknown: {s:?}; defaulting to CurrentSession"));
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
                            warnings.push(format!("{path}.truth_status unknown: {s:?}; defaulting to SceneEvent"));
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
    let knowledge_scope = parse_lax_knowledge_scope(lax.knowledge_scope.as_ref(), warnings, &format!("{path}.knowledge_scope"));

    MemoryCandidate {
        candidate_id,
        owner_soul_id,
        slot,
        content,
        evidence_quote: lax.evidence_quote.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default(),
        criterion_met: parse_lax_bool(lax.criterion_met.as_ref(), &format!("{path}.criterion_met"), warnings),
        confidence: confidence.unwrap_or(0.7),
        salience,
        retrieval_strength,
        perceived_by_entity_id: lax.perceived_by_entity_id.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())),
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
        warnings.push(format!("{path}.object normalized to object_state.object_id"));
        if !state_obj.contains_key("object_id") {
            state_obj.insert("object_id".into(), obj_val.clone());
        }
        if !state_obj.contains_key("object_kind") {
            state_obj.insert("object_kind".into(), obj_val.clone());
        }
    }

    if let Some(change_val) = &lax.change {
        warnings.push(format!("{path}.change normalized to object_state.last_observed_state"));
        if !state_obj.contains_key("last_observed_state") {
            state_obj.insert("last_observed_state".into(), change_val.clone());
        }
    }

    if let Some(prev_val) = &lax.previous_state {
        warnings.push(format!("{path}.previous_state preserved under object_state.properties.previous_state"));
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
        warnings.push(format!("{path}.entity_id normalized to object_state.owner_entity_id"));
        if !state_obj.contains_key("owner_entity_id") {
            state_obj.insert("owner_entity_id".into(), ent_val.clone());
        }
    }

    // Apply defaults to state_obj
    if !state_obj.contains_key("object_id") {
        state_obj.insert("object_id".into(), serde_json::Value::String("unknown_object".to_string()));
    }
    if !state_obj.contains_key("object_kind") {
        state_obj.insert("object_kind".into(), serde_json::Value::String("unknown".to_string()));
    }
    if !state_obj.contains_key("status") {
        state_obj.insert("status".into(), serde_json::Value::String("unknown".to_string()));
    }
    if !state_obj.contains_key("power_state") {
        state_obj.insert("power_state".into(), serde_json::Value::String("unknown".to_string()));
    }
    if !state_obj.contains_key("notification_mode") {
        state_obj.insert("notification_mode".into(), serde_json::Value::String("unknown".to_string()));
    }
    if !state_obj.contains_key("last_observed_state") {
        state_obj.insert("last_observed_state".into(), serde_json::Value::String("".to_string()));
    }
    if !state_obj.contains_key("location") {
        state_obj.insert("location".into(), serde_json::Value::String("".to_string()));
    }
    if !state_obj.contains_key("confidence") {
        state_obj.insert("confidence".into(), serde_json::json!(0.7));
    }

    // Deserialize state_obj into ObjectState
    let object_state: ObjectState = match serde_json::from_value(serde_json::Value::Object(state_obj)) {
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
        let evidence_str = lax.evidence_quote.as_ref().and_then(|v| v.as_str()).unwrap_or("");
        let fb = format!("obj_norm_{}", rand_str_from_evidence(evidence_str));
        warnings.push(format!("{path}.change_id generated from evidence hash: {fb}"));
        Some(fb)
    };

    let confidence = parse_lax_float_with_warning(lax.confidence.as_ref(), &format!("{path}.confidence"), warnings)
        .unwrap_or(0.7);

    let relevance_tags = map_relevance_tags(lax.relevance_tags.as_ref(), warnings, &format!("{path}.relevance_tags"));

    ObjectChangeEvaluation {
        change_id,
        object_state,
        evidence_quote: lax.evidence_quote.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())),
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
            warnings.push(format!("{path} changes/deltas nested object flattened to relationship evaluation"));
            changes_obj = Some(m.clone());
            break;
        }
    }

    let get_lax_val = |field: &str, primary: Option<&serde_json::Value>| -> Option<serde_json::Value> {
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

    let parse_field = |field: &str, primary: Option<&serde_json::Value>, warnings: &mut Vec<String>| -> Option<f32> {
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
    let boundary_pressure = parse_field("boundary_pressure", lax.boundary_pressure.as_ref(), warnings);

    // 2. source_soul_id Mapping
    let mut source_soul_id = lax.source_soul_id.as_ref()
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
    let mut target_entity_id = lax.target_entity_id.as_ref()
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
    let evidence_quote = lax.evidence_quote.as_ref().and_then(|v| v.as_str().map(|s| s.to_string()));

    // 5. criterion met
    let criterion_met = if let Some(cm) = &lax.criterion_met {
        parse_lax_bool(Some(cm), &format!("{path}.criterion_met"), warnings)
    } else {
        evidence_quote.as_ref().map(|s| !s.is_empty()).unwrap_or(false)
    };

    // 6. confidence
    let confidence = parse_lax_float_with_warning(lax.confidence.as_ref(), &format!("{path}.confidence"), warnings)
        .unwrap_or(0.65);

    // 7. relevance tags
    let relevance_tags = map_relevance_tags(lax.relevance_tags.as_ref(), warnings, &format!("{path}.relevance_tags"));

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
        let evidence_str = lax.evidence_quote.as_ref().and_then(|v| v.as_str()).unwrap_or("");
        let fb = format!("world_norm_{}", rand_str_from_evidence(evidence_str));
        warnings.push(format!("{path}.change_id generated from evidence hash: {fb}"));
        Some(fb)
    };

    let confidence = parse_lax_float_with_warning(lax.confidence.as_ref(), &format!("{path}.confidence"), warnings)
        .unwrap_or(0.7);

    let active_plot_add = parse_lax_string_array_or_keys(lax.active_plot_add.as_ref(), &format!("{path}.active_plot_add"), warnings);
    let active_plot_resolve = parse_lax_string_array_or_keys(lax.active_plot_resolve.as_ref(), &format!("{path}.active_plot_resolve"), warnings);

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

    let relevance_tags = map_relevance_tags(lax.relevance_tags.as_ref(), warnings, &format!("{path}.relevance_tags"));

    WorldChangeEvaluation {
        change_id,
        location: lax.location.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())),
        event_summary: lax.event_summary.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())),
        scene_state,
        active_plot_add,
        active_plot_resolve,
        evidence_quote: lax.evidence_quote.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())),
        confidence,
        relevance_tags,
    }
}

fn map_per_soul_evaluation(
    lax: &LaxPerSoulEvaluation,
    warnings: &mut Vec<String>,
    path: &str,
) -> PerSoulEvaluation {
    let mut soul_id_opt = lax.soul_id.as_ref().and_then(|v| v.as_str().map(|s| s.to_string()));
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

    let knowledge_scope = parse_lax_knowledge_scope(lax.knowledge_scope.as_ref(), warnings, &format!("{path}.knowledge_scope"));

    let relationship_deltas = if let Some(deltas) = &lax.relationship_deltas {
        deltas.iter().enumerate().map(|(idx, d)| {
            map_relationship_evaluation(d, warnings, &format!("{path}.relationship_deltas[{idx}]"))
        }).collect()
    } else {
        Vec::new()
    };

    let parent_soul_id_ref = if !soul_id.is_empty() { Some(soul_id.as_str()) } else { None };
    let memory_candidates = if let Some(candidates) = &lax.memory_candidates {
        candidates.iter().enumerate().map(|(idx, c)| {
            map_memory_candidate(c, parent_soul_id_ref, warnings, &format!("{path}.memory_candidates[{idx}]"))
        }).collect()
    } else {
        Vec::new()
    };

    let relevance_tags = map_relevance_tags(lax.relevance_tags.as_ref(), warnings, &format!("{path}.relevance_tags"));

    PerSoulEvaluation {
        soul_id,
        observed,
        knowledge_scope,
        subjective_interpretation: lax.subjective_interpretation.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default(),
        emotional_state: lax.emotional_state.as_ref().and_then(|v| v.as_str().map(|s| s.to_string())),
        relationship_deltas,
        memory_candidates,
        relevance_tags,
    }
}

pub fn map_evaluator_output(
    lax: LaxEvaluatorOutput,
    warnings: &mut Vec<String>,
) -> EvaluatorOutputV1 {
    let schema_version = lax.schema_version.as_ref()
        .and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_u64().map(|x| x as u32),
            serde_json::Value::String(s) => s.trim().parse::<u32>().ok(),
            _ => None,
        })
        .unwrap_or(1);
    
    let thought_process = lax.thought_process.as_ref().and_then(|v| v.as_str().map(|s| s.to_string()));

    let turn_flags_u64 = lax.turn_flags_u64.as_ref()
        .and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_u64(),
            serde_json::Value::String(s) => s.trim().parse::<u64>().ok(),
            _ => None,
        })
        .unwrap_or(0);

    let turn_classification = map_turn_classification(lax.turn_classification.as_ref(), warnings);

    let global_scene_evaluation = map_global_scene_evaluation(lax.global_scene_evaluation.as_ref(), warnings);

    let per_soul_evaluations = if let Some(pse) = &lax.per_soul_evaluations {
        pse.iter().enumerate().map(|(idx, se)| {
            map_per_soul_evaluation(se, warnings, &format!("per_soul_evaluations[{idx}]"))
        }).collect()
    } else {
        Vec::new()
    };

    let world_changes = if let Some(wc) = &lax.world_changes {
        wc.iter().enumerate().map(|(idx, c)| {
            map_world_change(c, warnings, &format!("world_changes[{idx}]"))
        }).collect()
    } else {
        Vec::new()
    };

    let object_changes = if let Some(oc) = &lax.object_changes {
        oc.iter().enumerate().map(|(idx, c)| {
            map_object_change(c, warnings, &format!("object_changes[{idx}]"))
        }).collect()
    } else {
        Vec::new()
    };

    let relationship_evaluations = if let Some(re) = &lax.relationship_evaluations {
        re.iter().enumerate().map(|(idx, e)| {
            map_relationship_evaluation(e, warnings, &format!("relationship_evaluations[{idx}]"))
        }).collect()
    } else {
        Vec::new()
    };

    let memory_candidates = if let Some(mc) = &lax.memory_candidates {
        mc.iter().enumerate().map(|(idx, c)| {
            map_memory_candidate(c, None, warnings, &format!("memory_candidates[{idx}]"))
        }).collect()
    } else {
        Vec::new()
    };

    let relevance_tags = map_relevance_tags(lax.relevance_tags.as_ref(), warnings, "relevance_tags");

    let no_op_reason = lax.no_op_reason.as_ref().and_then(|v| v.as_str().map(|s| s.to_string()));

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

pub fn parse_evaluator_output(raw_json: &str) -> Result<EvaluatorParseResult, String> {
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

    let output = map_evaluator_output(lax_output, &mut warnings);

    let normalized = !warnings.is_empty();
    let normalized_json = serde_json::to_string(&output)
        .map_err(|err| format!("Evaluator output serialization failed: {err}"))?;

    Ok(EvaluatorParseResult {
        output,
        normalized_json,
        normalized,
        warnings,
    })
}
