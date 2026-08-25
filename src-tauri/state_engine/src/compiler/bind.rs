use serde::{Deserialize, Serialize};

use super::{
    diagnostics::{CompilerDiagnostic, CompilerStage, DiagnosticSeverity},
    perception::{ClaimValue, PerceptionBatch, PerceptionCandidate},
    source::SourceEnvelope,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityRole {
    Soul,
    ActivePlayer,
    World,
    Object,
    Location,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntityDescriptor {
    pub entity_id: String,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub role: EntityRole,
    pub active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntityCatalog {
    pub entities: Vec<EntityDescriptor>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BindingStatus {
    Bound,
    Unresolved,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntityBinding {
    pub field_path: String,
    pub raw_ref: String,
    pub resolved_entity_id: Option<String>,
    pub status: BindingStatus,
    pub candidate_entity_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BoundCandidate {
    pub candidate: PerceptionCandidate,
    pub bindings: Vec<EntityBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BindingReport {
    pub source_hash: String,
    pub candidates: Vec<BoundCandidate>,
    pub diagnostics: Vec<CompilerDiagnostic>,
}

pub trait EntityBinder {
    fn bind(&self, source: &SourceEnvelope, batch: &PerceptionBatch) -> BindingReport;
}

#[derive(Debug, Clone)]
pub struct DeterministicEntityBinder {
    catalog: EntityCatalog,
}

impl DeterministicEntityBinder {
    pub fn new(catalog: EntityCatalog) -> Self {
        Self { catalog }
    }

    fn bind_ref(
        &self,
        source: &SourceEnvelope,
        field_path: String,
        raw_ref: &str,
    ) -> EntityBinding {
        let raw_ref = raw_ref.trim();
        let alias_role = match raw_ref {
            "active_soul" => Some(EntityRole::Soul),
            "active_player" | "latest_speaker" => Some(EntityRole::ActivePlayer),
            "session_world" => Some(EntityRole::World),
            _ => None,
        };
        let mut matches = self
            .catalog
            .entities
            .iter()
            .filter(|entity| entity.active)
            .filter(|entity| {
                if let Some(role) = alias_role {
                    if role == EntityRole::Soul {
                        return entity.role == role
                            && source.active_soul_ids().contains(&entity.entity_id);
                    }
                    return entity.role == role;
                }
                entity.entity_id == raw_ref
                    || normalized_entity_ref(&entity.display_name) == normalized_entity_ref(raw_ref)
                    || entity
                        .aliases
                        .iter()
                        .any(|alias| normalized_entity_ref(alias) == normalized_entity_ref(raw_ref))
            })
            .map(|entity| entity.entity_id.clone())
            .collect::<Vec<_>>();
        matches.sort();
        matches.dedup();
        let status = match matches.len() {
            1 => BindingStatus::Bound,
            0 => BindingStatus::Unresolved,
            _ => BindingStatus::Ambiguous,
        };
        EntityBinding {
            field_path,
            raw_ref: raw_ref.to_string(),
            resolved_entity_id: (status == BindingStatus::Bound).then(|| matches[0].clone()),
            status,
            candidate_entity_ids: matches,
        }
    }
}

impl EntityBinder for DeterministicEntityBinder {
    fn bind(&self, source: &SourceEnvelope, batch: &PerceptionBatch) -> BindingReport {
        let mut diagnostics = Vec::new();
        let candidates = batch
            .candidates
            .iter()
            .cloned()
            .map(|candidate| {
                let mut bindings = vec![self.bind_ref(
                    source,
                    "subject_ref".into(),
                    &candidate.perception.subject_ref,
                )];
                if let Some(actor_ref) = candidate.perception.actor_ref.as_deref() {
                    bindings.push(self.bind_ref(source, "actor_ref".into(), actor_ref));
                }
                if let Some(perceiver_ref) = candidate.perception.perceiver_ref.as_deref() {
                    bindings.push(self.bind_ref(source, "perceiver_ref".into(), perceiver_ref));
                }
                for (index, target_ref) in candidate.perception.target_refs.iter().enumerate() {
                    bindings.push(self.bind_ref(
                        source,
                        format!("target_refs[{index}]"),
                        target_ref,
                    ));
                }
                if let Some(ClaimValue::EntityRef { entity_ref }) =
                    candidate.perception.object.as_ref()
                {
                    bindings.push(self.bind_ref(source, "object.entity_ref".into(), entity_ref));
                }
                for binding in &bindings {
                    if binding.status != BindingStatus::Bound {
                        diagnostics.push(CompilerDiagnostic {
                            stage: CompilerStage::Binding,
                            severity: DiagnosticSeverity::Error,
                            code: match binding.status {
                                BindingStatus::Unresolved => "entity_unresolved",
                                BindingStatus::Ambiguous => "entity_ambiguous",
                                BindingStatus::Bound => unreachable!(),
                            }
                            .into(),
                            message: format!(
                                "{} entity reference {:?}",
                                match binding.status {
                                    BindingStatus::Unresolved => "unresolved",
                                    BindingStatus::Ambiguous => "ambiguous",
                                    BindingStatus::Bound => unreachable!(),
                                },
                                binding.raw_ref
                            ),
                            candidate_id: Some(candidate.candidate_id.clone()),
                            field_path: Some(binding.field_path.clone()),
                        });
                    }
                }
                BoundCandidate {
                    candidate,
                    bindings,
                }
            })
            .collect();
        BindingReport {
            source_hash: source.source_hash().into(),
            candidates,
            diagnostics,
        }
    }
}

fn normalized_entity_ref(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
