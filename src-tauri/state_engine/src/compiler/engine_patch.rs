use serde::{Deserialize, Serialize};

use crate::{
    patch::{
        EnginePatch, MemoryPatch, ObjectObservationOperationPatch, RelationshipDelta,
        SceneStatePatch, SoulPatch, WorldEventOperationPatch, WorldPatch, PATCH_PROTOCOL_VERSION,
    },
    soul::{MemorySourceType, ObjectState, TruthStatus},
};

use super::{
    CompilerDiagnostic, CompilerStage, DiagnosticSeverity, EvidenceSource, MemoryFormationKind,
    RelationshipEvidenceSignal, SourceEnvelope, StateEffect, StateEffectKind,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EnginePatchLoweringReport {
    pub source_hash: String,
    pub patch: EnginePatch,
    pub diagnostics: Vec<CompilerDiagnostic>,
    pub unsupported_effect_ids: Vec<String>,
}

pub fn lower_state_effects_to_engine_patch(
    source: &SourceEnvelope,
    effects: &[StateEffect],
) -> EnginePatchLoweringReport {
    let mut patch = EnginePatch {
        schema_version: Some(PATCH_PROTOCOL_VERSION),
        ..EnginePatch::default()
    };
    let mut diagnostics = Vec::new();
    let mut unsupported_effect_ids = Vec::new();

    for effect in effects {
        if effect.provenance.source_hash != source.source_hash() {
            diagnostics.push(diagnostic(
                effect,
                "effect_source_mismatch",
                "effect source does not match the active source envelope",
            ));
            continue;
        }
        match &effect.effect {
            StateEffectKind::AppendWorldEvent {
                summary,
                participant_entity_ids: _,
                location_entity_id: _,
            } => {
                patch
                    .world_patch
                    .get_or_insert_with(WorldPatch::default)
                    .event_operations
                    .push(WorldEventOperationPatch {
                        operation: "add_recent_event".into(),
                        recent_event_id: Some(effect.provenance.effect_id.clone()),
                        content: Some(summary.clone()),
                        ..WorldEventOperationPatch::default()
                    });
            }
            StateEffectKind::RecordObjectObservation {
                object_entity_id,
                observer_entity_id: _,
                status,
                location_entity_id,
            } => {
                patch
                    .world_patch
                    .get_or_insert_with(WorldPatch::default)
                    .object_observation_operations
                    .push(ObjectObservationOperationPatch {
                        operation: "update_object_state".into(),
                        object_observation_id: Some(effect.provenance.effect_id.clone()),
                        object_state: Some(ObjectState {
                            object_id: object_entity_id.clone(),
                            object_kind: "object".into(),
                            location: location_entity_id.clone().unwrap_or_default(),
                            status: status.clone(),
                            last_observed_state: status.clone(),
                            confidence: 0.8,
                            ..ObjectState::default()
                        }),
                        ..ObjectObservationOperationPatch::default()
                    });
            }
            StateEffectKind::FormMemory {
                owner_soul_id,
                memory_kind,
                content,
                target_entity_ids,
            } => {
                patch
                    .soul_patch
                    .get_or_insert_with(SoulPatch::default)
                    .new_memories
                    .push(memory_patch(
                        source,
                        effect,
                        owner_soul_id,
                        *memory_kind,
                        content,
                        target_entity_ids,
                    ));
            }
            StateEffectKind::RecordIntention {
                owner_entity_id,
                content,
                target_entity_ids,
            } => {
                patch
                    .soul_patch
                    .get_or_insert_with(SoulPatch::default)
                    .new_memories
                    .push(memory_patch(
                        source,
                        effect,
                        owner_entity_id,
                        MemoryFormationKind::Intention,
                        content,
                        target_entity_ids,
                    ));
            }
            StateEffectKind::ApplyRelationshipEvidence {
                source_soul_id,
                target_entity_id,
                signal,
            } => {
                patch
                    .soul_patch
                    .get_or_insert_with(SoulPatch::default)
                    .relationship_deltas
                    .push(relationship_delta(
                        effect,
                        source_soul_id,
                        target_entity_id,
                        signal,
                    ));
            }
            StateEffectKind::UpdateSceneProjection {
                scene,
                focus,
                participant_entity_ids,
                pressure_point,
            } => {
                patch
                    .world_patch
                    .get_or_insert_with(WorldPatch::default)
                    .scene_state = Some(SceneStatePatch {
                    scene_state_id: Some(effect.provenance.effect_id.clone()),
                    current_scene: Some(scene.clone()),
                    focus: Some(focus.clone()),
                    participants: participant_entity_ids.clone(),
                    pressure_point: pressure_point.clone(),
                    ..SceneStatePatch::default()
                });
            }
            StateEffectKind::RecordCorrection { .. } => {
                unsupported_effect_ids.push(effect.provenance.effect_id.clone());
                diagnostics.push(diagnostic(
                    effect,
                    "correction_requires_ledger_invalidation",
                    "correction effects require the M6 invalidation projection and cannot lower to a V1 patch",
                ));
            }
        }
    }

    EnginePatchLoweringReport {
        source_hash: source.source_hash().into(),
        patch,
        diagnostics,
        unsupported_effect_ids,
    }
}

fn memory_patch(
    source: &SourceEnvelope,
    effect: &StateEffect,
    owner_soul_id: &str,
    memory_kind: MemoryFormationKind,
    content: &str,
    target_entity_ids: &[String],
) -> MemoryPatch {
    let (tag, truth_status) = match memory_kind {
        MemoryFormationKind::Episode => ("episode", TruthStatus::SceneEvent),
        MemoryFormationKind::Testimony => ("testimony", TruthStatus::CharacterBelief),
        MemoryFormationKind::Perception => ("perception", TruthStatus::SceneEvent),
        MemoryFormationKind::Affect => ("affect", TruthStatus::CharacterBelief),
        MemoryFormationKind::Intention => ("intention", TruthStatus::CharacterBelief),
        MemoryFormationKind::Belief => ("belief", TruthStatus::CharacterBelief),
    };
    let source_message_id = match effect.provenance.evidence.source {
        EvidenceSource::UserMessage => source.identity().user_message_id,
        EvidenceSource::AssistantMessage => source.identity().assistant_message_id,
    };
    MemoryPatch {
        memory_id: Some(effect.provenance.effect_id.clone()),
        content: content.into(),
        tag: Some(tag.into()),
        source_type: Some(MemorySourceType::CurrentSession),
        source_session_id: Some(source.identity().branch_id.clone()),
        source_conversation_id: Some(source.identity().conversation_id.clone()),
        source_message_id: Some(source_message_id),
        source_quote: Some(effect.provenance.evidence.quote.clone()),
        perceived_by_entity_id: Some(owner_soul_id.into()),
        target_entity_ids: target_entity_ids.to_vec(),
        confidence: Some(0.8),
        salience: Some(60.0),
        retrieval_strength: Some(60.0),
        truth_status: Some(truth_status),
        memory_slot: Some(tag.into()),
        owner_soul_id: Some(owner_soul_id.into()),
        architecture_verified: Some(false),
        ..MemoryPatch::default()
    }
}

fn relationship_delta(
    effect: &StateEffect,
    source_soul_id: &str,
    target_entity_id: &str,
    signal: &RelationshipEvidenceSignal,
) -> RelationshipDelta {
    let direction = signal.valence.signum() as f32;
    let intensity = (signal.valence.unsigned_abs() as f32
        * (0.35
            + signal.directness as f32 / 250.0
            + signal.stakes as f32 / 400.0
            + signal.costliness as f32 / 500.0
            + signal.repetition as f32 / 500.0))
        .clamp(0.0, 5.0);
    let signed = direction * intensity;
    let has = |behavior| signal.behaviors.contains(&behavior);
    use super::BehaviorEvidenceKind as Behavior;
    let trust = if has(Behavior::PromiseKept)
        || has(Behavior::HonestDisclosure)
        || has(Behavior::SupportOffered)
        || has(Behavior::RepairAccepted)
    {
        Some(intensity)
    } else if has(Behavior::PromiseBroken) || has(Behavior::Deception) {
        Some(-intensity)
    } else {
        nonzero(signed * 0.5)
    };
    let boundary_pressure = if has(Behavior::BoundaryViolated) || has(Behavior::HarmThreatened) {
        Some(intensity)
    } else if has(Behavior::BoundaryRespected) {
        Some(-intensity)
    } else {
        None
    };
    let conflict = if has(Behavior::PromiseBroken)
        || has(Behavior::Deception)
        || has(Behavior::BoundaryViolated)
        || has(Behavior::HarmThreatened)
        || has(Behavior::Abandonment)
    {
        Some(intensity)
    } else if has(Behavior::RepairAccepted) {
        Some(-intensity)
    } else {
        None
    };
    RelationshipDelta {
        relationship_event_id: Some(effect.provenance.effect_id.clone()),
        from: Some(source_soul_id.into()),
        target: Some(target_entity_id.into()),
        trust,
        comfort: nonzero(signed * 0.6),
        respect: nonzero(signed * 0.5),
        conflict,
        boundary_pressure,
        intimacy: has(Behavior::HonestDisclosure)
            .then_some(intensity * 0.5)
            .or_else(|| has(Behavior::Abandonment).then_some(-intensity * 0.5)),
        max_abs_delta: Some(5.0),
        ..RelationshipDelta::default()
    }
}

fn nonzero(value: f32) -> Option<f32> {
    (value.abs() >= 0.001).then_some(value.clamp(-5.0, 5.0))
}

fn diagnostic(effect: &StateEffect, code: &str, message: &str) -> CompilerDiagnostic {
    CompilerDiagnostic {
        stage: CompilerStage::Lowering,
        severity: DiagnosticSeverity::Error,
        code: code.into(),
        message: message.into(),
        candidate_id: Some(effect.provenance.candidate_id.clone()),
        field_path: None,
    }
}
