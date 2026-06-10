use serde_json::{json, Value};

use crate::{evaluator::MemorySlot, evaluator_form::EvalFormSpec};

pub fn build_hard_eval_form_template(spec: &EvalFormSpec) -> Value {
    let source_soul_id = primary_soul_id(spec);
    let target_entity_id = default_player_id(spec);

    json!({
        "evaluator_form_v1": {
            "scene_participants": scene_participant_template(spec),
            "new_character_rows": [{
                "row_enabled": 0,
                "temporary_label": "",
                "display_name": "",
                "role_code": 0,
                "evidence_quote": ""
            }],
            "event_rows": [],
            "object_rows": [{
                "row_enabled": 0,
                "linked_event_id": "event_latest_turn",
                "object_id": "",
                "object_label": "",
                "object_type": "",
                "owner_entity_id": target_entity_id,
                "status": "",
                "location": "",
                "last_observed_state": "",
                "evidence_quote": ""
            }],
            "relationship_rows": [],
            "relationship_event_rows": [{
                "row_enabled": 0,
                "event_id": "event_latest_turn",
                "actor_entity_id": target_entity_id,
                "relationship_source_soul_id": source_soul_id,
                "relationship_target_entity_id": target_entity_id,
                "perceived_by_entity_id": source_soul_id,
                "evidence_quote": "",
                "intent": 0,
                "honesty": 0,
                "reliability": 0,
                "boundary_treatment": 0,
                "responsiveness": 0,
                "power_use": 0,
                "evaluation_tone": 0,
                "competence": 0,
                "disclosure": 0,
                "reciprocity": 0,
                "repair": 0,
                "predictability": 0,
                "salience": 0,
                "certainty": 0,
                "directness": 0,
                "costliness": 0,
                "stakes": 0,
                "repetition": 0,
                "event_flags_u64": 0
            }],
            "memory_rows": [{
                "row_enabled": 0,
                "owner_soul_id": source_soul_id,
                "slot": MemorySlot::RelationshipMemory.as_label(),
                "importance": "medium",
                "evidence_quote": "",
                "content": "",
                "linked_event_id": "event_latest_turn"
            }],
            "review_rows": []
        }
    })
}

fn scene_participant_template(spec: &EvalFormSpec) -> Vec<Value> {
    spec.active_entities
        .iter()
        .map(|entity| {
            json!({
                "entity_id": entity.entity_id,
                "display_name": entity.display_name,
                "present": 1,
                "newly_introduced": 0
            })
        })
        .collect()
}

fn primary_soul_id(spec: &EvalFormSpec) -> &str {
    spec.active_soul_ids
        .first()
        .map(String::as_str)
        .or_else(|| {
            spec.active_entities
                .iter()
                .find(|entity| entity.entity_type == "soul")
                .map(|entity| entity.entity_id.as_str())
        })
        .unwrap_or("active_soul")
}

fn default_player_id(spec: &EvalFormSpec) -> &str {
    spec.active_entities
        .iter()
        .find(|entity| entity.entity_type == "player_persona")
        .map(|entity| entity.entity_id.as_str())
        .or_else(|| {
            spec.active_entities
                .iter()
                .find(|entity| entity.entity_id == "default_player")
                .map(|entity| entity.entity_id.as_str())
        })
        .or_else(|| {
            spec.active_entities
                .iter()
                .find(|entity| entity.entity_type == "user")
                .map(|entity| entity.entity_id.as_str())
        })
        .unwrap_or("default_player")
}
