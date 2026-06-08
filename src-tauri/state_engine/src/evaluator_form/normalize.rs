use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::{
    evaluator::{EvaluatorConversionContext, MemorySlot},
    evaluator_form::{
        active_player_entity_id, clean, resolve_active_entity_id, slugify, stable_id, EvalFormRepairTrace,
        EvalFormResponse, EvalFormSpec, EventRow, EventType, ImportanceTier, MagnitudeTier,
        MemoryRow, ObjectRow, RelationshipDimension, RelationshipDirection, RelationshipRow,
    },
};

pub fn normalize_eval_form_value(value: &mut Value, trace: &mut EvalFormRepairTrace) {
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

pub fn normalize_row_array(
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

pub fn normalize_child_row_array(
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

pub fn normalize_event_row_value(
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

pub fn normalize_object_row_value(
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

pub fn normalize_relationship_row_value(
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

    if let Some(val) = row.get("direction").and_then(Value::as_str) {
        if val.contains("->") {
            let parts: Vec<&str> = val.split("->").collect();
            if parts.len() == 2 {
                let left = parts[0].trim().to_string();
                let right = parts[1].trim().to_string();
                row.insert("source_soul_id".into(), Value::String(left));
                row.insert("target_entity_id".into(), Value::String(right));
                row.remove("direction");
                trace.raw_form_repair_warnings.push("arrow direction parsed as source and target".into());
                trace.raw_form_repair_applied = true;
            }
        }
    }

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

pub fn normalize_relationship_tags_value(
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

pub fn infer_relationship_dimension_from_tags(
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

pub fn infer_relationship_direction_from_shift(
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

pub fn infer_relationship_direction_from_summary(
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

pub fn normalize_relationship_magnitude_from_importance(
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

pub fn relationship_tag_values(value: &Value) -> Vec<String> {
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

pub fn relationship_dimension_label(raw: &str) -> Option<&'static str> {
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

pub fn normalize_relationship_dimension_value(
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

pub fn normalize_memory_row_value(
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

pub fn normalize_review_row_value(
    row: &mut serde_json::Map<String, Value>,
    trace: &mut EvalFormRepairTrace,
) {
    move_alias(row, "event_id", "linked_event_id", trace);
    move_alias(row, "memory_id", "candidate_id", trace);
    move_alias(row, "review_id", "candidate_id", trace);
}

pub fn move_alias(
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

pub fn collect_event_ids(value: &Value) -> Vec<String> {
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

pub fn normalize_linked_event_id_value(
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

pub fn normalize_event_type_value(
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

pub fn normalize_relationship_direction_value(
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

pub fn normalize_memory_slot_value(
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

pub fn memory_slot_from_value(value: &Value) -> Option<&'static str> {
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

pub fn normalize_tags_value(row: &mut serde_json::Map<String, Value>, trace: &mut EvalFormRepairTrace) {
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

pub fn split_relationship_dimensions(value: &mut Value, trace: &mut EvalFormRepairTrace) {
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

pub fn normalize_token(raw: &str) -> String {
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

pub fn normalize_eval_form_response(
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
    for row in &mut normalized.relationship_event_rows {
        normalize_relationship_event_entities(row, spec);
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

pub fn normalize_relationship_event_entities(row: &mut Value, spec: &EvalFormSpec) {
    let Some(object) = row.as_object_mut() else {
        return;
    };
    for key in [
        "actor_entity_id",
        "source_entity_id",
        "target_entity_id",
        "relationship_source_soul_id",
        "relationship_target_entity_id",
        "perceived_by_entity_id",
    ] {
        let Some(raw) = object.get(key).and_then(Value::as_str) else {
            continue;
        };
        let resolved = normalize_player_id(&resolve_active_entity_id(raw, spec));
        object.insert(key.into(), Value::String(resolved));
    }
}

pub fn default_participants(spec: &EvalFormSpec) -> Vec<String> {
    let mut participants = spec.active_soul_ids.clone();
    let player_id = active_player_entity_id(spec).unwrap_or_else(|| "default_player".into());
    if !participants.iter().any(|id| id == &player_id) {
        participants.push(player_id);
    }
    participants
}

pub fn compact_latest_turn_summary(context: &EvaluatorConversionContext<'_>) -> String {
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

pub fn choose_main_event_id(rows: &[EventRow]) -> Option<String> {
    rows.iter()
        .max_by_key(|row| {
            importance_rank(row.importance_tier.unwrap_or(ImportanceTier::Medium))
        })
        .and_then(|row| clean(&row.event_id).map(str::to_string))
}

pub fn importance_rank(tier: ImportanceTier) -> u8 {
    match tier {
        ImportanceTier::Trivial => 0,
        ImportanceTier::Low => 1,
        ImportanceTier::Medium => 2,
        ImportanceTier::High => 3,
        ImportanceTier::Critical => 4,
    }
}

pub fn normalize_child_link(
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

pub fn normalize_relationship_aliases(row: &mut RelationshipRow, spec: &EvalFormSpec) {
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

    let active_player_id = active_player_entity_id(spec).unwrap_or_else(|| "default_player".into());
    if spec.active_soul_ids.len() == 1 {
        let active_soul_id = &spec.active_soul_ids[0];
        if row.source_soul_id.trim().is_empty() {
            row.source_soul_id = active_soul_id.clone();
        }
        if row.target_entity_id.trim().is_empty() {
            row.target_entity_id = active_player_id.clone();
        }
    } else if !active_player_id.trim().is_empty() {
        if row.target_entity_id.trim().is_empty() {
            row.target_entity_id = active_player_id.clone();
        }
    }

    row.source_soul_id = normalize_player_id(&row.source_soul_id);
    row.target_entity_id = normalize_player_id(&row.target_entity_id);

    if spec.active_soul_ids.len() == 1 {
        let active_soul_id = &spec.active_soul_ids[0];
        if row.source_soul_id == active_player_id || row.source_soul_id == "default_player" {
            row.source_soul_id = active_soul_id.clone();
            row.target_entity_id = active_player_id;
        }
    }
}

pub fn normalize_relationship_defaults(row: &mut RelationshipRow) {
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
        if let Some(dim) = row.dimension {
            if let Some(inferred) = infer_relationship_direction_from_evidence(
                dim,
                &row.evidence_quote,
                row.shift.as_deref(),
                row.summary.as_deref(),
            ) {
                row.direction = Some(inferred);
            }
        }
    }

    if row.direction.is_none() {
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
            let change_type_lower = row.change_type.as_deref().unwrap_or("").to_ascii_lowercase();

            let has_escalation_keyword = evidence_lower.contains("pressure")
                || evidence_lower.contains("conflict")
                || evidence_lower.contains("fear")
                || evidence_lower.contains("tension")
                || evidence_lower.contains("wary")
                || evidence_lower.contains("guarded")
                || evidence_lower.contains("edge")
                || evidence_lower.contains("escalation")
                || evidence_lower.contains("positioned just inside the doorway")
                || evidence_lower.contains("positioned yourself just inside the doorway")
                || evidence_lower.contains("just inside the doorway")
                || evidence_lower.contains("doorway")
                || evidence_lower.contains("positioned")
                || evidence_lower.contains("not quite entering")
                || evidence_lower.contains("giving me time to process")
                || change_type_lower.contains("pressure")
                || change_type_lower.contains("conflict")
                || change_type_lower.contains("fear")
                || change_type_lower.contains("tension")
                || change_type_lower.contains("wary")
                || change_type_lower.contains("guarded")
                || change_type_lower.contains("edge")
                || change_type_lower.contains("escalation");

            let has_deescalation_keyword = evidence_lower.contains("softened")
                || evidence_lower.contains("eased")
                || evidence_lower.contains("reduced")
                || evidence_lower.contains("decreased")
                || evidence_lower.contains("less")
                || evidence_lower.contains("lower")
                || change_type_lower.contains("softened")
                || change_type_lower.contains("eased")
                || change_type_lower.contains("reduced")
                || change_type_lower.contains("decreased")
                || change_type_lower.contains("less")
                || change_type_lower.contains("lower");

            let has_increase_keyword = evidence_lower.contains("increase")
                || evidence_lower.contains("grew")
                || evidence_lower.contains("grow")
                || evidence_lower.contains("intensified")
                || evidence_lower.contains("warmer")
                || evidence_lower.contains("closer")
                || evidence_lower.contains("more")
                || evidence_lower.contains("strengthen")
                || evidence_lower.contains("deepen")
                || evidence_lower.contains("higher")
                || evidence_lower.contains("up")
                || evidence_lower.contains("improving")
                || evidence_lower.contains("improved")
                || evidence_lower.contains("building")
                || evidence_lower.contains("built")
                || evidence_lower.contains("enhanced")
                || evidence_lower.contains("drop")
                || evidence_lower.contains("whisper")
                || evidence_lower.contains("soft")
                || evidence_lower.contains("trust")
                || evidence_lower.contains("vulnerable")
                || evidence_lower.contains("careful posture")
                || evidence_lower.contains("giving me time to process")
                || evidence_lower.contains("affect")
                || evidence_lower.contains("intim")
                || evidence_lower.contains("passion")
                || evidence_lower.contains("commit")
                || evidence_lower.contains("desir")
                || evidence_lower.contains("respect")
                || evidence_lower.contains("curios")
                || evidence_lower.contains("interest")
                || evidence_lower.contains("comfort")
                || change_type_lower.contains("increase")
                || change_type_lower.contains("grew")
                || change_type_lower.contains("grow")
                || change_type_lower.contains("intensified")
                || change_type_lower.contains("warmer")
                || change_type_lower.contains("closer")
                || change_type_lower.contains("more")
                || change_type_lower.contains("strengthen")
                || change_type_lower.contains("deepen")
                || change_type_lower.contains("higher")
                || change_type_lower.contains("up")
                || change_type_lower.contains("improving")
                || change_type_lower.contains("improved")
                || change_type_lower.contains("building")
                || change_type_lower.contains("built")
                || change_type_lower.contains("enhanced")
                || change_type_lower.contains("drop")
                || change_type_lower.contains("whisper")
                || change_type_lower.contains("soft")
                || change_type_lower.contains("trust")
                || change_type_lower.contains("affect")
                || change_type_lower.contains("intim")
                || change_type_lower.contains("passion")
                || change_type_lower.contains("commit")
                || change_type_lower.contains("desir")
                || change_type_lower.contains("respect")
                || change_type_lower.contains("curios")
                || change_type_lower.contains("interest")
                || change_type_lower.contains("comfort");

            let strongly_implied =
                has_increase_keyword || has_escalation_keyword || has_deescalation_keyword;

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

pub fn normalize_object_aliases(row: &mut ObjectRow) {
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

    // Gap 2: both object_id and new_object_label missing — try conservative noun extraction
    if row.object_id.as_deref().and_then(clean).is_none()
        && row.new_object_label.as_deref().and_then(clean).is_none()
    {
        let search_text = format!("{} {}", row.evidence_quote, row.new_value);
        if let Some(noun) = infer_physical_object_from_evidence(&search_text) {
            row.new_object_label = Some(noun.replace(' ', "_"));
            row.object_id = Some(slugify(noun));
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

    // Gap 1: property_changed is already "state" but new_value is still empty —
    // derive from evidence_quote as a last resort.
    if row.new_value.trim().is_empty() && row.property_changed == "state" {
        if !row.evidence_quote.trim().is_empty() {
            row.new_value = row.evidence_quote.clone();
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

pub fn normalize_memory_aliases(
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

pub fn normalize_player_id(value: &str) -> String {
    if value == "user" {
        "default_player".into()
    } else {
        value.to_string()
    }
}

/// Try to extract a single conservative physical-object noun from `evidence`.
///
/// Returns `None` if no known physical noun is found, so that abstract-only
/// evidence ("warmth increased", "nervous energy", …) never produces a fake
/// object ID.
///
/// The noun list is intentionally small and concrete.  Add entries only for
/// objects that appear regularly in scene narration.
pub(crate) fn infer_physical_object_from_evidence(evidence: &str) -> Option<&'static str> {
    let lower = evidence.to_ascii_lowercase();

    // Ordered: longer / more-specific phrases first so they beat single words.
    const PHYSICAL_NOUNS: &[&str] = &[
        // multi-word first
        "wet jacket",
        "wine glass",
        "chain lock",
        "cigarette",
        // single-word
        "jacket",
        "coat",
        "door",
        "chair",
        "table",
        "glass",
        "mug",
        "phone",
        "window",
        "lock",
        "bag",
        "book",
        "candle",
        "bottle",
        "key",
        "knife",
        "lamp",
        "pen",
        "cup",
    ];

    for &noun in PHYSICAL_NOUNS {
        if lower.contains(noun) {
            return Some(noun);
        }
    }
    None
}

pub fn infer_relationship_direction_from_evidence(
    dimension: RelationshipDimension,
    evidence_quote: &str,
    _shift: Option<&str>,
    _summary: Option<&str>,
) -> Option<RelationshipDirection> {
    let evidence_lower = evidence_quote.to_ascii_lowercase();

    match dimension {
        RelationshipDimension::BoundaryPressure => {
            let strong_boundary = [
                "chain is still on the door",
                "expecting someone or preparing for a stranger",
                "preparing for a stranger",
                "chain still holds the door",
                "keeps the door chained",
                "keeps the chain on",
                "hesitates before opening",
                "holds the door an inch",
                "holds the door partly closed",
                "keeps distance",
                "backs away",
                "sets a boundary",
                "refuses entry",
                "door chain",
                "chain on the door",
                "guarded door chain",
                "guarded",
                "uncertain entry",
            ];
            if strong_boundary.iter().any(|&p| evidence_lower.contains(p)) {
                return Some(RelationshipDirection::Increase);
            }
        }
        RelationshipDimension::Trust => {
            let strong_distrust = [
                "doesn't trust",
                "does not trust",
                "distrust",
                "backs away suspiciously",
                "keeps the chain on",
                "chain is still on the door",
                "asks who sent you",
                "refuses to open",
            ];
            if strong_distrust.iter().any(|&p| evidence_lower.contains(p)) {
                return Some(RelationshipDirection::Decrease);
            }
        }
        RelationshipDimension::Comfort => {
            let strong_discomfort = [
                "chain is still on the door",
                "keeps the chain on",
                "doesn't trust",
                "does not trust",
                "distrust",
                "backs away suspiciously",
                "asks who sent you",
                "refuses to open",
                "hesitates before opening",
                "holds the door partly closed",
                "keeps distance",
                "backs away",
                "sets a boundary",
                "refuses entry",
                "expecting someone or preparing for a stranger",
                "preparing for a stranger",
                "tension",
                "discomfort",
                "unease",
                "uneasy",
                "guarded",
                "stiffens",
            ];
            let strong_comfort = [
                "relax",
                "soften",
                "invites closer",
                "inviting closer",
                "opens the door fully",
                "opening the door fully",
                "comfortable familiarity",
                "comfortable",
                "familiar",
                "welcom",
            ];
            if strong_comfort.iter().any(|&p| evidence_lower.contains(p)) {
                return Some(RelationshipDirection::Increase);
            } else if strong_discomfort.iter().any(|&p| evidence_lower.contains(p)) {
                return Some(RelationshipDirection::Decrease);
            }
        }
        RelationshipDimension::Fear => {
            let strong_fear = [
                "stiffens",
                "pulse thrumming",
                "taste copper",
                "startled",
                "flinches",
                "fear spikes",
                "panic",
                "scared",
                "frightened",
                "terror",
            ];
            if strong_fear.iter().any(|&p| evidence_lower.contains(p)) {
                return Some(RelationshipDirection::Increase);
            }
        }
        _ => {}
    }

    None
}
