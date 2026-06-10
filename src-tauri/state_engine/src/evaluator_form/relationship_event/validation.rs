use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::evaluator_form::{
    clean, reject_row, EvalFormRowRejection, EvalFormSpec, ValidatedRelationshipEventRow,
};

pub const RELATIONSHIP_EVENT_TEMPLATE_VERSION: &str = "relationship_event_v2b_hard_template";

pub const RELATIONSHIP_EVENT_REQUIRED_KEYS: [&str; 26] = [
    "row_enabled",
    "event_id",
    "actor_entity_id",
    "relationship_source_soul_id",
    "relationship_target_entity_id",
    "perceived_by_entity_id",
    "evidence_quote",
    "intent",
    "honesty",
    "reliability",
    "boundary_treatment",
    "responsiveness",
    "power_use",
    "evaluation_tone",
    "competence",
    "disclosure",
    "reciprocity",
    "repair",
    "predictability",
    "salience",
    "certainty",
    "directness",
    "costliness",
    "stakes",
    "repetition",
    "event_flags_u64",
];

const RELATIONSHIP_EVENT_STRING_KEYS: [&str; 6] = [
    "event_id",
    "actor_entity_id",
    "relationship_source_soul_id",
    "relationship_target_entity_id",
    "perceived_by_entity_id",
    "evidence_quote",
];

const FORBIDDEN_LEGACY_NUMERIC_KEYS: [&str; 8] = [
    "axis_trust",
    "axis_fear",
    "axis_comfort",
    "axis_boundary_pressure",
    "modifier_trust",
    "modifier_fear",
    "modifier_comfort",
    "modifier_boundary_pressure",
];

pub const FORBIDDEN_RELATIONSHIP_SURFACE_KEYS: [&str; 7] = [
    "trust",
    "comfort",
    "intimacy",
    "respect",
    "fear",
    "conflict",
    "boundary_pressure",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationshipEventValidation {
    Enabled(ValidatedRelationshipEventRow),
    Disabled,
}

pub fn relationship_event_row_id(row: &Value) -> String {
    let object = row.as_object();
    format!(
        "{}:{}:{}:{}",
        object
            .and_then(|o| o.get("event_id"))
            .and_then(Value::as_str)
            .unwrap_or("unknown_event"),
        object
            .and_then(|o| o.get("relationship_source_soul_id"))
            .and_then(Value::as_str)
            .unwrap_or("unknown_source"),
        object
            .and_then(|o| o.get("relationship_target_entity_id"))
            .and_then(Value::as_str)
            .unwrap_or("unknown_target"),
        object
            .and_then(|o| o.get("perceived_by_entity_id"))
            .and_then(Value::as_str)
            .unwrap_or("unknown_perceiver")
    )
}

pub fn validate_relationship_event_row(
    row: &Value,
    spec: &EvalFormSpec,
    allowed_entities: &HashSet<&str>,
    event_ids: &HashSet<&str>,
    rejected: &mut Vec<EvalFormRowRejection>,
) -> Option<RelationshipEventValidation> {
    let Some(object) = row.as_object() else {
        reject_row(
            rejected,
            "relationship_event",
            "relationship_event:invalid",
            "entity_resolution_failed",
        );
        return None;
    };

    let row_id = relationship_event_row_id(row);
    if !validate_relationship_event_keys(object, rejected, &row_id) {
        return None;
    }

    let row_enabled = required_row_enabled(object, rejected, &row_id)?;
    for field in RELATIONSHIP_EVENT_STRING_KEYS {
        required_string(object, field, rejected, &row_id)?;
    }
    let intent = required_axis(object, "intent", rejected, &row_id)?;
    let honesty = required_axis(object, "honesty", rejected, &row_id)?;
    let reliability = required_axis(object, "reliability", rejected, &row_id)?;
    let boundary_treatment = required_axis(object, "boundary_treatment", rejected, &row_id)?;
    let responsiveness = required_axis(object, "responsiveness", rejected, &row_id)?;
    let power_use = required_axis(object, "power_use", rejected, &row_id)?;
    let evaluation_tone = required_axis(object, "evaluation_tone", rejected, &row_id)?;
    let competence = required_axis(object, "competence", rejected, &row_id)?;
    let disclosure = required_axis(object, "disclosure", rejected, &row_id)?;
    let reciprocity = required_axis(object, "reciprocity", rejected, &row_id)?;
    let repair = required_axis(object, "repair", rejected, &row_id)?;
    let predictability = required_axis(object, "predictability", rejected, &row_id)?;
    let salience = required_modifier(object, "salience", rejected, &row_id)?;
    let certainty = required_modifier(object, "certainty", rejected, &row_id)?;
    let directness = required_modifier(object, "directness", rejected, &row_id)?;
    let costliness = required_modifier(object, "costliness", rejected, &row_id)?;
    let stakes = required_modifier(object, "stakes", rejected, &row_id)?;
    let repetition = required_modifier(object, "repetition", rejected, &row_id)?;
    let event_flags_u64 = required_event_flags(object, rejected, &row_id)?;

    if row_enabled == 0 {
        return Some(RelationshipEventValidation::Disabled);
    }

    let event_id = required_nonempty_string(object, "event_id").unwrap_or_default();
    let actor_entity_id = required_nonempty_string(object, "actor_entity_id").unwrap_or_default();
    let relationship_source_soul_id =
        required_nonempty_string(object, "relationship_source_soul_id").unwrap_or_default();
    let relationship_target_entity_id =
        required_nonempty_string(object, "relationship_target_entity_id").unwrap_or_default();
    let perceived_by_entity_id =
        required_nonempty_string(object, "perceived_by_entity_id").unwrap_or_default();

    if event_id.is_empty() || !event_ids.contains(event_id.as_str()) {
        reject_row(
            rejected,
            "relationship_event",
            &row_id,
            "event_id_resolution_failed",
        );
        return None;
    }

    if actor_entity_id.is_empty() || !allowed_entities.contains(actor_entity_id.as_str()) {
        reject_row(
            rejected,
            "relationship_event",
            &row_id,
            "actor_entity_resolution_failed",
        );
        return None;
    }

    if relationship_target_entity_id.is_empty()
        || !allowed_entities.contains(relationship_target_entity_id.as_str())
    {
        reject_row(
            rejected,
            "relationship_event",
            &row_id,
            "relationship_target_entity_resolution_failed",
        );
        return None;
    }

    if relationship_source_soul_id.is_empty()
        || !spec
            .active_soul_ids
            .iter()
            .any(|id| id == &relationship_source_soul_id)
    {
        reject_row(
            rejected,
            "relationship_event",
            &row_id,
            "relationship_source_soul_resolution_failed",
        );
        return None;
    }

    if perceived_by_entity_id.is_empty()
        || !spec
            .active_soul_ids
            .iter()
            .any(|id| id == &perceived_by_entity_id)
    {
        reject_row(
            rejected,
            "relationship_event",
            &row_id,
            "perceived_by_entity_resolution_failed",
        );
        return None;
    }

    let evidence_quote = required_nonempty_string(object, "evidence_quote").unwrap_or_default();
    if evidence_quote.is_empty() {
        reject_row(
            rejected,
            "relationship_event",
            &row_id,
            "evidence_quote_missing",
        );
        return None;
    }

    Some(RelationshipEventValidation::Enabled(
        ValidatedRelationshipEventRow {
            event_id,
            source_entity_id: actor_entity_id,
            target_entity_id: relationship_target_entity_id.clone(),
            perceived_by_entity_id,
            relationship_source_soul_id,
            relationship_target_entity_id,
            evidence_quote,
            intent,
            honesty,
            reliability,
            boundary_treatment,
            responsiveness,
            power_use,
            evaluation_tone,
            competence,
            disclosure,
            reciprocity,
            repair,
            predictability,
            salience,
            certainty,
            directness,
            costliness,
            stakes,
            repetition,
            event_flags_u64,
        },
    ))
}

fn validate_relationship_event_keys(
    object: &Map<String, Value>,
    rejected: &mut Vec<EvalFormRowRejection>,
    row_id: &str,
) -> bool {
    for key in FORBIDDEN_LEGACY_NUMERIC_KEYS {
        if object.contains_key(key) {
            return reject_row(
                rejected,
                "relationship_event",
                row_id,
                &format!("relationship_event_unknown_key:{key}"),
            );
        }
    }
    for key in FORBIDDEN_RELATIONSHIP_SURFACE_KEYS {
        if object.contains_key(key) {
            return reject_row(
                rejected,
                "relationship_event",
                row_id,
                &format!("relationship_event_forbidden_surface_field:{key}"),
            );
        }
    }
    if let Some(key) = object
        .keys()
        .find(|key| !RELATIONSHIP_EVENT_REQUIRED_KEYS.contains(&key.as_str()))
    {
        return reject_row(
            rejected,
            "relationship_event",
            row_id,
            &format!("relationship_event_unknown_key:{key}"),
        );
    }
    for key in RELATIONSHIP_EVENT_REQUIRED_KEYS {
        if !object.contains_key(key) {
            return reject_row(
                rejected,
                "relationship_event",
                row_id,
                &format!("relationship_event_missing_key:{key}"),
            );
        }
    }
    true
}

fn required_string(
    object: &Map<String, Value>,
    field: &str,
    rejected: &mut Vec<EvalFormRowRejection>,
    row_id: &str,
) -> Option<String> {
    let Some(value) = object.get(field).and_then(Value::as_str) else {
        reject_row(
            rejected,
            "relationship_event",
            row_id,
            &format!("relationship_event_string_field_non_string:{field}"),
        );
        return None;
    };
    Some(value.to_string())
}

fn required_nonempty_string(object: &Map<String, Value>, field: &str) -> Option<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .and_then(clean)
        .map(str::to_string)
}

fn required_row_enabled(
    object: &Map<String, Value>,
    rejected: &mut Vec<EvalFormRowRejection>,
    row_id: &str,
) -> Option<i64> {
    let Some(value) = object.get("row_enabled").and_then(Value::as_i64) else {
        reject_row(
            rejected,
            "relationship_event",
            row_id,
            "relationship_event_row_enabled_invalid",
        );
        return None;
    };
    if value != 0 && value != 1 {
        reject_row(
            rejected,
            "relationship_event",
            row_id,
            "relationship_event_row_enabled_invalid",
        );
        return None;
    }
    Some(value)
}

fn required_axis(
    object: &Map<String, Value>,
    field: &str,
    rejected: &mut Vec<EvalFormRowRejection>,
    row_id: &str,
) -> Option<i32> {
    let value = required_integer(object, field, rejected, row_id)?;
    if !(-5..=5).contains(&value) {
        reject_row(
            rejected,
            "relationship_event",
            row_id,
            &format!("axis_out_of_range:{field}"),
        );
        return None;
    }
    Some(value)
}

fn required_modifier(
    object: &Map<String, Value>,
    field: &str,
    rejected: &mut Vec<EvalFormRowRejection>,
    row_id: &str,
) -> Option<u32> {
    let value = required_integer(object, field, rejected, row_id)?;
    if !(0..=100).contains(&value) {
        reject_row(
            rejected,
            "relationship_event",
            row_id,
            &format!("modifier_out_of_range:{field}"),
        );
        return None;
    }
    Some(value as u32)
}

fn required_integer(
    object: &Map<String, Value>,
    field: &str,
    rejected: &mut Vec<EvalFormRowRejection>,
    row_id: &str,
) -> Option<i32> {
    let Some(value) = object.get(field) else {
        reject_row(
            rejected,
            "relationship_event",
            row_id,
            &format!("numeric_field_missing:{field}"),
        );
        return None;
    };
    let Some(value) = value.as_i64() else {
        reject_row(
            rejected,
            "relationship_event",
            row_id,
            &format!("numeric_field_non_numeric:{field}"),
        );
        return None;
    };
    i32::try_from(value).ok().or_else(|| {
        reject_row(
            rejected,
            "relationship_event",
            row_id,
            &format!("numeric_field_non_numeric:{field}"),
        );
        None
    })
}

fn required_event_flags(
    object: &Map<String, Value>,
    rejected: &mut Vec<EvalFormRowRejection>,
    row_id: &str,
) -> Option<u64> {
    let Some(flags) = object.get("event_flags_u64").and_then(Value::as_u64) else {
        reject_row(
            rejected,
            "relationship_event",
            row_id,
            "event_flags_non_numeric",
        );
        return None;
    };
    Some(flags)
}
