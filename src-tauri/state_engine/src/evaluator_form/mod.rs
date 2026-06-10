pub mod types;
pub use types::*;

pub mod trace;
pub use trace::*;

pub mod raw_repair;
pub use raw_repair::*;

pub mod normalize;
pub use normalize::*;

pub mod relationship_event;
pub use relationship_event::*;

pub mod template;
pub use template::*;

pub mod validate;
pub use validate::*;

pub mod compile;
pub use compile::*;

pub(crate) fn slugify(label: &str) -> String {
    label
        .trim()
        .to_ascii_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

pub(crate) fn clean(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

pub(crate) fn resolve_active_entity_id(raw_id: &str, spec: &EvalFormSpec) -> String {
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

    if normalized_raw == "user" || normalized_raw == "default_player" || normalized_raw == "player"
    {
        return active_player_entity_id(spec).unwrap_or_else(|| "default_player".to_string());
    }

    clean_raw.to_string()
}

pub(crate) fn active_player_entity_id(spec: &EvalFormSpec) -> Option<String> {
    spec.active_entities
        .iter()
        .find(|entity| entity.entity_type == "player_persona" || entity.entity_type == "user")
        .map(|entity| entity.entity_id.clone())
}

#[cfg(test)]
mod tests;
