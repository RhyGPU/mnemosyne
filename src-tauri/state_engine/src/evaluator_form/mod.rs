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

pub mod types;
pub use types::*;

pub mod trace;
pub use trace::*;

pub mod raw_repair;
pub use raw_repair::*;

pub mod normalize;
pub use normalize::*;

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
    
    if normalized_raw == "user" || normalized_raw == "default_player" || normalized_raw == "player" {
        return "default_player".to_string();
    }
    
    clean_raw.to_string()
}



#[cfg(test)]
mod tests;
