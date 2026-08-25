use serde::{Deserialize, Serialize};

use super::{
    diagnostics::{CompilerContractError, CompilerDiagnostic},
    perception::{
        BehaviorEvidenceKind, ClaimValue, EpistemicMode, EvidenceSpan, PerceptionCandidate,
        PerceptionKind,
    },
    semantic::{bound_entity, SemanticDisposition, SemanticReport, ValidatedCandidate},
    source::{stable_digest, SourceEnvelope},
    MEMORY_COMPILER_CONTRACT_VERSION,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectProvenance {
    pub effect_id: String,
    pub candidate_id: String,
    pub source_hash: String,
    pub compiler_version: u32,
    pub evidence: EvidenceSpan,
}

impl EffectProvenance {
    pub fn from_candidate(
        source: &SourceEnvelope,
        candidate: &PerceptionCandidate,
        effect_index: usize,
    ) -> Result<Self, CompilerContractError> {
        source.validate()?;
        if candidate.source_hash != source.source_hash() {
            return Err(CompilerContractError::new(
                "candidate_source_mismatch",
                "candidate source hash does not match the active source envelope",
            ));
        }
        let effect_index = effect_index.to_string();
        Ok(Self {
            effect_id: stable_digest(
                "state_effect",
                [
                    source.source_hash(),
                    candidate.candidate_id.as_str(),
                    effect_index.as_str(),
                ],
            ),
            candidate_id: candidate.candidate_id.clone(),
            source_hash: source.source_hash().to_string(),
            compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
            evidence: candidate.perception.evidence.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StateEffect {
    pub provenance: EffectProvenance,
    pub effect: StateEffectKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StateEffectKind {
    AppendWorldEvent {
        summary: String,
        participant_entity_ids: Vec<String>,
        location_entity_id: Option<String>,
    },
    RecordObjectObservation {
        object_entity_id: String,
        observer_entity_id: String,
        status: String,
        location_entity_id: Option<String>,
    },
    FormMemory {
        owner_soul_id: String,
        memory_kind: MemoryFormationKind,
        content: String,
        target_entity_ids: Vec<String>,
    },
    RecordIntention {
        owner_entity_id: String,
        content: String,
        target_entity_ids: Vec<String>,
    },
    ApplyRelationshipEvidence {
        source_soul_id: String,
        target_entity_id: String,
        signal: RelationshipEvidenceSignal,
    },
    UpdateSceneProjection {
        scene: String,
        focus: String,
        participant_entity_ids: Vec<String>,
        pressure_point: Option<String>,
    },
    RecordCorrection {
        subject_entity_id: String,
        predicate: String,
        replacement: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryFormationKind {
    Episode,
    Testimony,
    Perception,
    Affect,
    Intention,
    Belief,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RelationshipEvidenceSignal {
    pub behaviors: Vec<BehaviorEvidenceKind>,
    pub valence: i8,
    pub directness: u8,
    pub stakes: u8,
    pub costliness: u8,
    pub repetition: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LoweringReport {
    pub source_hash: String,
    pub effects: Vec<StateEffect>,
    pub diagnostics: Vec<CompilerDiagnostic>,
}

pub trait EffectLowerer {
    fn lower(&self, source: &SourceEnvelope, semantics: &SemanticReport) -> LoweringReport;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicEffectLowerer;

impl EffectLowerer for DeterministicEffectLowerer {
    fn lower(&self, source: &SourceEnvelope, semantics: &SemanticReport) -> LoweringReport {
        let mut diagnostics = semantics.diagnostics.clone();
        if semantics.source_hash != source.source_hash() {
            diagnostics.push(CompilerDiagnostic {
                stage: super::diagnostics::CompilerStage::Lowering,
                severity: super::diagnostics::DiagnosticSeverity::Error,
                code: "semantic_source_mismatch".into(),
                message: "semantic report does not belong to active source".into(),
                candidate_id: None,
                field_path: None,
            });
            return LoweringReport {
                source_hash: source.source_hash().into(),
                effects: Vec::new(),
                diagnostics,
            };
        }

        let mut effects = Vec::new();
        for validated in semantics
            .candidates
            .iter()
            .filter(|candidate| candidate.disposition == SemanticDisposition::Accepted)
        {
            let candidate = &validated.candidate.candidate;
            let kinds = lower_candidate(source, validated);
            for (effect_index, effect) in kinds.into_iter().enumerate() {
                match EffectProvenance::from_candidate(source, candidate, effect_index) {
                    Ok(provenance) => effects.push(StateEffect { provenance, effect }),
                    Err(error) => diagnostics.push(CompilerDiagnostic {
                        stage: super::diagnostics::CompilerStage::Lowering,
                        severity: super::diagnostics::DiagnosticSeverity::Error,
                        code: error.code.into(),
                        message: error.message,
                        candidate_id: Some(candidate.candidate_id.clone()),
                        field_path: None,
                    }),
                }
            }
        }
        LoweringReport {
            source_hash: source.source_hash().into(),
            effects,
            diagnostics,
        }
    }
}

fn lower_candidate(
    source: &SourceEnvelope,
    validated: &ValidatedCandidate,
) -> Vec<StateEffectKind> {
    let bound = &validated.candidate;
    let perception = &bound.candidate.perception;
    let subject = bound_entity(bound, "subject_ref")
        .unwrap_or(perception.subject_ref.as_str())
        .to_string();
    let actor = bound_entity(bound, "actor_ref").map(str::to_string);
    let perceiver = bound_entity(bound, "perceiver_ref").map(str::to_string);
    let targets = bound
        .bindings
        .iter()
        .filter(|binding| binding.field_path.starts_with("target_refs["))
        .filter_map(|binding| binding.resolved_entity_id.clone())
        .collect::<Vec<_>>();
    let object_entity = bound_entity(bound, "object.entity_ref").map(str::to_string);
    let summary = claim_summary(
        &subject,
        &perception.predicate,
        perception.object.as_ref(),
        object_entity.as_deref(),
    );
    let owner_soul_id = perceiver
        .as_ref()
        .filter(|id| source.active_soul_ids().contains(id))
        .cloned()
        .or_else(|| {
            source
                .active_soul_ids()
                .iter()
                .find(|id| id.as_str() == subject)
                .cloned()
        })
        .or_else(|| source.active_soul_ids().first().cloned())
        .unwrap_or_else(|| subject.clone());

    match perception.kind {
        PerceptionKind::Event => {
            if matches!(
                perception.epistemic_mode,
                EpistemicMode::StatedBy | EpistemicMode::Inferred | EpistemicMode::RememberedBy
            ) {
                vec![StateEffectKind::FormMemory {
                    owner_soul_id,
                    memory_kind: memory_kind_for(perception.kind, perception.epistemic_mode),
                    content: summary,
                    target_entity_ids: targets,
                }]
            } else {
                let mut participants = vec![subject];
                participants.extend(actor);
                participants.extend(targets);
                participants.sort();
                participants.dedup();
                vec![StateEffectKind::AppendWorldEvent {
                    summary,
                    participant_entity_ids: participants,
                    location_entity_id: None,
                }]
            }
        }
        PerceptionKind::Utterance
        | PerceptionKind::AffectCue
        | PerceptionKind::BeliefExpression => vec![StateEffectKind::FormMemory {
            owner_soul_id,
            memory_kind: memory_kind_for(perception.kind, perception.epistemic_mode),
            content: summary,
            target_entity_ids: targets,
        }],
        PerceptionKind::ObjectObservation => {
            if matches!(
                perception.epistemic_mode,
                EpistemicMode::StatedBy | EpistemicMode::Inferred | EpistemicMode::RememberedBy
            ) {
                vec![StateEffectKind::FormMemory {
                    owner_soul_id,
                    memory_kind: memory_kind_for(perception.kind, perception.epistemic_mode),
                    content: summary,
                    target_entity_ids: targets,
                }]
            } else {
                vec![StateEffectKind::RecordObjectObservation {
                    object_entity_id: object_entity.unwrap_or(subject),
                    observer_entity_id: perceiver.unwrap_or(owner_soul_id),
                    status: summary,
                    location_entity_id: None,
                }]
            }
        }
        PerceptionKind::RelationshipEvidence => perception
            .relationship_signal
            .as_ref()
            .map(|signal| {
                let target_entity_id = targets
                    .iter()
                    .find(|target| target.as_str() != owner_soul_id)
                    .cloned()
                    .or_else(|| {
                        actor
                            .as_ref()
                            .filter(|actor| actor.as_str() != owner_soul_id)
                            .cloned()
                    })
                    .unwrap_or_else(|| subject.clone());
                vec![StateEffectKind::ApplyRelationshipEvidence {
                    source_soul_id: owner_soul_id,
                    target_entity_id,
                    signal: RelationshipEvidenceSignal {
                        behaviors: signal.behaviors.clone(),
                        valence: signal.valence,
                        directness: signal.directness,
                        stakes: signal.stakes,
                        costliness: signal.costliness,
                        repetition: signal.repetition,
                    },
                }]
            })
            .unwrap_or_default(),
        PerceptionKind::Intention => vec![StateEffectKind::RecordIntention {
            owner_entity_id: actor.unwrap_or(subject),
            content: summary,
            target_entity_ids: targets,
        }],
        PerceptionKind::Correction => vec![StateEffectKind::RecordCorrection {
            subject_entity_id: subject,
            predicate: perception.predicate.clone(),
            replacement: claim_value_text(perception.object.as_ref(), object_entity.as_deref()),
        }],
    }
}

fn memory_kind_for(kind: PerceptionKind, epistemic: EpistemicMode) -> MemoryFormationKind {
    match kind {
        PerceptionKind::AffectCue => MemoryFormationKind::Affect,
        PerceptionKind::Intention => MemoryFormationKind::Intention,
        PerceptionKind::BeliefExpression => MemoryFormationKind::Belief,
        _ => match epistemic {
            EpistemicMode::StatedBy => MemoryFormationKind::Testimony,
            EpistemicMode::Inferred | EpistemicMode::RememberedBy => MemoryFormationKind::Belief,
            EpistemicMode::DirectlyObserved | EpistemicMode::NarratorDescribed => {
                MemoryFormationKind::Episode
            }
        },
    }
}

fn claim_summary(
    subject: &str,
    predicate: &str,
    value: Option<&ClaimValue>,
    bound_object: Option<&str>,
) -> String {
    match claim_value_text(value, bound_object) {
        Some(value) => format!("{subject} {predicate} {value}"),
        None => format!("{subject} {predicate}"),
    }
}

fn claim_value_text(value: Option<&ClaimValue>, bound_object: Option<&str>) -> Option<String> {
    match value {
        Some(ClaimValue::EntityRef { entity_ref }) => {
            Some(bound_object.unwrap_or(entity_ref).to_string())
        }
        Some(ClaimValue::Text { text }) => Some(text.clone()),
        Some(ClaimValue::Number { value, unit }) => Some(match unit {
            Some(unit) => format!("{value} {unit}"),
            None => value.to_string(),
        }),
        Some(ClaimValue::Boolean { value }) => Some(value.to_string()),
        None => None,
    }
}
