use std::collections::HashSet;

use crate::evaluator_form::{
    clean, memory_candidate_id, EvalFormRowRejection, EvalFormSpec, EventRow, MemoryRow,
    ObjectRow, RelationshipRow, ReviewDecision, ReviewRow,
};

pub fn validate_event_row(
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

pub fn validate_object_row(
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

pub fn validate_relationship_row(
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

pub fn validate_memory_row(
    row: &MemoryRow,
    spec: &EvalFormSpec,
    event_ids: &HashSet<&str>,
    rejected: &mut Vec<EvalFormRowRejection>,
) -> bool {
    let row_id = memory_candidate_id(row);
    if matches!(row.row_enabled, Some(value) if value > 1) {
        return reject_row(rejected, "memory", &row_id, "row_enabled must be 0 or 1");
    }
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

pub fn validate_review_row(
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

pub fn reject_row(
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

pub fn existing_id_allowed(spec: &EvalFormSpec, id: &str) -> bool {
    spec.existing_memories
        .iter()
        .chain(spec.existing_events.iter())
        .chain(spec.existing_object_observations.iter())
        .chain(spec.existing_relationship_facts.iter())
        .any(|row| row.existing_id == id)
}

pub fn validate_evaluator_contract(
    raw_response: &str,
    user_text: &str,
    narrator_text: &str,
) -> Result<(), String> {
    let stripped = crate::evaluator_form::strip_json_fences(raw_response);
    let extracted = crate::evaluator_form::extract_first_balanced_json_object(&stripped)
        .unwrap_or_else(|| stripped.clone());

    let root = serde_json::from_str::<serde_json::Value>(&extracted)
        .map_err(|err| format!("Invalid JSON: {err}"))?;

    let root_obj = root
        .as_object()
        .ok_or_else(|| "Root of response must be a JSON object".to_string())?;

    if root_obj.len() != 1 || !root_obj.contains_key("evaluator_form_v1") {
        return Err("Root JSON envelope must contain exactly one key: 'evaluator_form_v1'".to_string());
    }

    let form_v1 = &root_obj["evaluator_form_v1"];
    let form_obj = form_v1
        .as_object()
        .ok_or_else(|| "'evaluator_form_v1' must be a JSON object".to_string())?;

    let allowed_envelope_keys = [
        "event_rows",
        "object_rows",
        "relationship_rows",
        "relationship_event_rows",
        "memory_rows",
        "review_rows",
    ];

    for key in form_obj.keys() {
        if !allowed_envelope_keys.contains(&key.as_str()) {
            return Err(format!("Unknown key inside envelope: '{key}'"));
        }
    }

    let mut has_rel_delta = false;
    if let Some(rows) = form_obj.get("relationship_event_rows").and_then(|v| v.as_array()) {
        for row in rows {
            let row_obj = row
                .as_object()
                .ok_or_else(|| "relationship_event_row must be an object".to_string())?;
            if !row_obj.contains_key("row_enabled") {
                return Err("relationship_event_row is missing 'row_enabled'".to_string());
            }

            use crate::evaluator_form::relationship_event::{
                FORBIDDEN_RELATIONSHIP_SURFACE_KEYS, RELATIONSHIP_EVENT_REQUIRED_KEYS,
            };

            for key in row_obj.keys() {
                if !RELATIONSHIP_EVENT_REQUIRED_KEYS.contains(&key.as_str()) {
                    return Err(format!("relationship_event_row contains unknown key: '{key}'"));
                }
                if FORBIDDEN_RELATIONSHIP_SURFACE_KEYS.contains(&key.as_str()) {
                    return Err(format!(
                        "relationship_event_row contains forbidden surface dimension key: '{key}'"
                    ));
                }
            }

            let enabled = row_obj.get("row_enabled").and_then(|v| v.as_i64()).unwrap_or(0);
            if enabled == 1 {
                has_rel_delta = true;
            }
        }
    }

    if let Some(rel_rows) = form_obj.get("relationship_rows") {
        let arr = rel_rows
            .as_array()
            .ok_or_else(|| "relationship_rows must be an array".to_string())?;
        if !arr.is_empty() {
            return Err("relationship_rows must remain empty".to_string());
        }
    }

    let allowed_slots = [
        "relationship_memory",
        "current_plot_memory",
        "character_identity_memory",
        "unresolved_tension",
        "world_location_memory",
        "recent_emotional_state",
    ];
    let mut has_memory_delta = false;
    if let Some(mem_rows) = form_obj.get("memory_rows").and_then(|v| v.as_array()) {
        for row in mem_rows {
            let row_obj = row
                .as_object()
                .ok_or_else(|| "memory_row must be an object".to_string())?;
            if let Some(slot_val) = row_obj.get("slot").and_then(|v| v.as_str()) {
                if !allowed_slots.contains(&slot_val) {
                    return Err(format!("memory_row contains invalid slot: '{slot_val}'"));
                }
            }

            let enabled = row_obj.get("row_enabled").and_then(|v| v.as_i64()).unwrap_or(1);
            if enabled == 1 {
                has_memory_delta = true;
            }
        }
    }

    let evidence_text = format!("{}\n{}", user_text, narrator_text);
    let row_fields = [
        "event_rows",
        "object_rows",
        "relationship_rows",
        "relationship_event_rows",
        "memory_rows",
        "review_rows",
    ];

    for field in &row_fields {
        if let Some(rows) = form_obj.get(*field).and_then(|v| v.as_array()) {
            for row in rows {
                if let Some(row_obj) = row.as_object() {
                    let enabled = row_obj.get("row_enabled").and_then(|v| v.as_i64()).unwrap_or(1);
                    if enabled == 0 {
                        continue;
                    }
                    if let Some(quote_val) = row_obj.get("evidence_quote").and_then(|v| v.as_str()) {
                        if !quote_val.trim().is_empty()
                            && !crate::evaluator::claim_has_evidence(Some(quote_val), &evidence_text)
                        {
                            return Err(format!(
                                "evidence_quote not found in scene context: '{quote_val}'"
                            ));
                        }
                    }
                }
            }
        }
    }

    if !has_rel_delta && !has_memory_delta {
        return Err("Nontrivial test scene did not produce any memory or relationship delta".to_string());
    }

    Ok(())
}
