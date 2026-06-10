use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    arousal::ArousalSignal,
    hidden_state::HiddenState,
    memory::create_scored_memory,
    setting::SessionWorld,
    soul::{
        current_timestamp, MemorySourceType, ObjectState, Relationship, SceneState, Soul,
        TruthStatus, WorldEventRecord, WorldLog,
    },
};

pub const PATCH_PROTOCOL_VERSION: u32 = 1;
const MAX_RELATIONSHIP_DELTA: f32 = 10.0;
/// Clamp each relationship scalar after deltas (matches engine defaults and leaves headroom above 100).
const RELATIONSHIP_SCALAR_MIN: f32 = 0.0;
const RELATIONSHIP_SCALAR_MAX: f32 = 300.0;
const MAX_RECENT_MEMORIES: usize = 12;
const MAX_STORED_MEMORIES: usize = 200;
const MAX_RECENT_EVENTS: usize = 12;
const MEMORY_HYGIENE_SCAN_LIMIT: usize = 30;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EnginePatch {
    pub schema_version: Option<u32>,
    pub soul_patch: Option<SoulPatch>,
    pub world_patch: Option<WorldPatch>,
    pub body_patch: Option<BodyPatch>,
    pub sensory_patch: Option<SensoryPatch>,
    pub memory_layer_reply: Option<MemoryLayerReplyPatch>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SoulPatch {
    pub relationship_delta: Option<RelationshipDelta>,
    pub relationship_deltas: Vec<RelationshipDelta>,
    pub new_memories: Vec<MemoryPatch>,
    pub memory_operations: Vec<MemoryPatch>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RelationshipDelta {
    pub relationship_event_id: Option<String>,
    pub from: Option<String>,
    pub target: Option<String>,
    pub trust: Option<f32>,
    pub affection: Option<f32>,
    pub intimacy: Option<f32>,
    pub passion: Option<f32>,
    pub commitment: Option<f32>,
    pub fear: Option<f32>,
    pub desire: Option<f32>,
    pub respect: Option<f32>,
    pub conflict: Option<f32>,
    pub dependency: Option<f32>,
    pub curiosity: Option<f32>,
    pub comfort: Option<f32>,
    pub boundary_pressure: Option<f32>,
    pub trustable_bias: Option<f32>,
    pub untrustworthy_bias: Option<f32>,
    pub asshole_bias: Option<f32>,
    pub care_bias: Option<f32>,
    pub danger_bias: Option<f32>,
    pub competence_bias: Option<f32>,
    pub autonomy_respect_bias: Option<f32>,
    pub attachment_pull: Option<f32>,
    pub schema_threat: Option<f32>,
    pub first_impression_strength: Option<f32>,
    pub first_impression_confidence: Option<f32>,
    pub reappraisal_debt: Option<f32>,
    pub reappraisal_state_code: Option<u8>,
    pub max_abs_delta: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MemoryPatch {
    pub operation: Option<String>,
    pub memory_id: Option<String>,
    pub target_memory_id: Option<String>,
    pub supersedes_memory_id: Option<String>,
    pub content: String,
    pub tag: Option<String>,
    pub source_type: Option<MemorySourceType>,
    pub source_session_id: Option<String>,
    pub source_conversation_id: Option<String>,
    pub source_message_id: Option<i64>,
    pub source_entity_id: Option<String>,
    pub is_lived_experience: Option<bool>,
    pub is_imported_context: Option<bool>,
    pub perceived_by_entity_id: Option<String>,
    pub target_entity_ids: Vec<String>,
    pub interpretation: Option<String>,
    pub confidence: Option<f32>,
    pub salience: Option<f32>,
    pub retrieval_strength: Option<f32>,
    pub objective_event_id: Option<String>,
    pub truth_status: Option<TruthStatus>,
    pub architecture_verified: Option<bool>,
    pub memory_slot: Option<String>,
    pub owner_soul_id: Option<String>,
    pub relevance_tags: HashMap<String, u8>,
    pub knowledge_scope: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MemoryLayerReplyPatch {
    pub nonce: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WorldPatch {
    pub location: Option<String>,
    pub time_elapsed: Option<String>,
    pub scene_state: Option<SceneStatePatch>,
    pub recent_event: Option<String>,
    pub recent_events: Vec<String>,
    pub active_plot_add: Vec<String>,
    pub active_plot_resolve: Vec<String>,
    pub key_object_add: Vec<String>,
    pub key_object_remove: Vec<String>,
    pub event_operations: Vec<WorldEventOperationPatch>,
    pub object_observation_operations: Vec<ObjectObservationOperationPatch>,
    pub corrected_object_states: Vec<ObjectState>,
    pub invalidated_recent_event_ids: Vec<String>,
    pub correction_note: Option<String>,
    pub retcon_scope: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SceneStatePatch {
    pub scene_state_id: Option<String>,
    pub current_scene: Option<String>,
    pub resolved_active_plot: Option<String>,
    pub scene_branch: Option<String>,
    pub focus: Option<String>,
    pub participants: Vec<String>,
    pub last_user_action: Option<String>,
    pub pressure_point: Option<String>,
    pub continuity_note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WorldEventOperationPatch {
    pub operation: String,
    pub recent_event_id: Option<String>,
    pub target_recent_event_id: Option<String>,
    pub supersedes_recent_event_id: Option<String>,
    pub content: Option<String>,
    pub match_text: Option<String>,
    pub invalidated_by_patch_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ObjectObservationOperationPatch {
    pub operation: String,
    pub object_observation_id: Option<String>,
    pub target_object_observation_id: Option<String>,
    pub object_state: Option<ObjectState>,
    pub invalidated_by_patch_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorldObjectConsistencyNotice {
    pub event: String,
    pub object_id: Option<String>,
    pub reason: String,
    pub rejected: bool,
    pub correction: bool,
}

impl WorldEventOperationPatch {
    fn is_empty(&self) -> bool {
        self.operation.trim().is_empty()
            && self
                .content
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            && self
                .recent_event_id
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            && self
                .target_recent_event_id
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
    }
}

impl ObjectObservationOperationPatch {
    fn is_empty(&self) -> bool {
        self.operation.trim().is_empty()
            && self.object_state.is_none()
            && self
                .object_observation_id
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            && self
                .target_object_observation_id
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
    }
}

impl SceneStatePatch {
    pub fn is_empty(&self) -> bool {
        self.scene_state_id
            .as_deref()
            .map_or(true, |value| value.trim().is_empty())
            && self
                .current_scene
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            && self
                .resolved_active_plot
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            && self
                .scene_branch
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            && self
                .focus
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            && self
                .participants
                .iter()
                .all(|value| value.trim().is_empty())
            && self
                .last_user_action
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            && self
                .pressure_point
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            && self
                .continuity_note
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
    }
}

/// Accepted for Patch Protocol V1 compatibility; arousal bridging uses the optional scalar fields only.
/// `region_updates` / `condition_updates` are placeholders (validated JSON, ignored on apply until Body V1 lands).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BodyPatch {
    pub activation_delta: Option<f32>,
    pub activation_blocked: Option<bool>,
    pub peak_allowed: Option<bool>,
    pub forced_peak: Option<bool>,
    pub region_updates: Vec<serde_json::Value>,
    pub condition_updates: Vec<serde_json::Value>,
}

/// Placeholder module: deserialization accepts narrator proposals; engine does not mutate sensory state yet.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SensoryPatch {
    pub association_updates: Vec<SensoryAssociationPatch>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SensoryAssociationPatch {
    pub sense: Option<String>,
    pub cue: Option<String>,
    pub association: Option<String>,
    pub strength_delta: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PatchReport {
    pub relationship_updated: bool,
    pub memories_added: usize,
    pub memory_events: Vec<MemoryApplyEvent>,
    pub world_updated: bool,
    pub body_updated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryApplyEvent {
    pub action: MemoryApplyAction,
    pub source_type: MemorySourceType,
    pub content: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryApplyAction {
    Added,
    RejectedDuplicate,
    RejectedGeneric,
    Merged,
    Deprioritized,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatchError {
    UnsupportedSchemaVersion(u32),
}

impl EnginePatch {
    pub fn apply_to_soul(&self, soul: &mut Soul) -> Result<PatchReport, PatchError> {
        self.apply_to_session(soul, None)
    }

    pub fn apply_to_session(
        &self,
        soul: &mut Soul,
        mut session_world: Option<&mut SessionWorld>,
    ) -> Result<PatchReport, PatchError> {
        self.validate()?;
        if self.is_empty() {
            return Ok(PatchReport::default());
        }

        let mut report = PatchReport::default();
        if let Some(soul_patch) = &self.soul_patch {
            report.relationship_updated = soul_patch.apply_relationship(soul);
            let memory_report = soul_patch.apply_memories(soul);
            report.memories_added = memory_report.added;
            report.memory_events = memory_report.events;
        }
        if let Some(world_patch) = &self.world_patch {
            report.world_updated = if let Some(session_world) = session_world.as_deref_mut() {
                world_patch.apply_to_session_world(session_world)
            } else {
                world_patch.apply(soul)
            };
        }
        if let Some(body_patch) = &self.body_patch {
            report.body_updated = body_patch.apply(soul);
        } else if self.should_decay_body() {
            soul.arousal.decay();
            report.body_updated = true;
        }

        // SensoryPatch V1 proposals deserialize successfully but state application is intentionally deferred.

        if report.relationship_updated
            || report.memories_added > 0
            || report.world_updated
            || report.body_updated
        {
            soul.last_updated = current_timestamp() as i64;
        }

        Ok(report)
    }

    pub fn validate(&self) -> Result<(), PatchError> {
        if let Some(version) = self.schema_version {
            if version != PATCH_PROTOCOL_VERSION {
                return Err(PatchError::UnsupportedSchemaVersion(version));
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.soul_patch.as_ref().map_or(true, SoulPatch::is_empty)
            && self.world_patch.as_ref().map_or(true, WorldPatch::is_empty)
            && self.body_patch.as_ref().map_or(true, BodyPatch::is_empty)
            && self
                .sensory_patch
                .as_ref()
                .map_or(true, SensoryPatch::is_empty)
            && self
                .memory_layer_reply
                .as_ref()
                .map_or(true, MemoryLayerReplyPatch::is_empty)
    }

    fn should_decay_body(&self) -> bool {
        self.soul_patch
            .as_ref()
            .map_or(false, |patch| !patch.is_empty())
            || self
                .world_patch
                .as_ref()
                .map_or(false, |patch| !patch.is_empty())
    }
}

impl From<&HiddenState> for EnginePatch {
    fn from(hidden_state: &HiddenState) -> Self {
        let mut soul_patch = SoulPatch::default();
        if hidden_state.trust_delta.is_some() || hidden_state.affection_delta.is_some() {
            soul_patch.relationship_delta = Some(RelationshipDelta {
                target: Some("user".into()),
                trust: hidden_state.trust_delta,
                affection: hidden_state.affection_delta,
                ..RelationshipDelta::default()
            });
        }
        if let Some(memory) = hidden_state
            .memory
            .as_deref()
            .map(str::trim)
            .filter(|memory| !memory.is_empty())
        {
            soul_patch.new_memories.push(MemoryPatch {
                content: memory.to_string(),
                tag: hidden_state.tag.clone(),
                ..MemoryPatch::default()
            });
        }

        let world_patch =
            if hidden_state.world_event.is_some() || hidden_state.new_location.is_some() {
                Some(WorldPatch {
                    location: hidden_state.new_location.clone(),
                    recent_event: hidden_state.world_event.clone(),
                    ..WorldPatch::default()
                })
            } else {
                None
            };

        let body_patch = if hidden_state.arousal_delta.is_some()
            || hidden_state.arousal_denied.is_some()
            || hidden_state.orgasm_allowed.is_some()
            || hidden_state.forced_orgasm.is_some()
        {
            Some(BodyPatch {
                activation_delta: hidden_state.arousal_delta,
                activation_blocked: hidden_state.arousal_denied,
                peak_allowed: hidden_state.orgasm_allowed,
                forced_peak: hidden_state.forced_orgasm,
                ..BodyPatch::default()
            })
        } else {
            None
        };

        Self {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: (!soul_patch.is_empty()).then_some(soul_patch),
            world_patch,
            body_patch,
            sensory_patch: None,
            memory_layer_reply: None,
        }
    }
}

impl SoulPatch {
    fn is_empty(&self) -> bool {
        self.relationship_delta
            .as_ref()
            .map_or(true, RelationshipDelta::is_empty)
            && self
                .relationship_deltas
                .iter()
                .all(RelationshipDelta::is_empty)
            && self.memory_operations.iter().all(MemoryPatch::is_empty)
            && self.new_memories.iter().all(MemoryPatch::is_empty)
    }

    fn apply_relationship(&self, soul: &mut Soul) -> bool {
        let mut changed = false;
        if let Some(delta) = &self.relationship_delta {
            changed |= apply_relationship_delta(soul, delta);
        }
        for delta in &self.relationship_deltas {
            changed |= apply_relationship_delta(soul, delta);
        }
        changed
    }

    fn apply_memories(&self, soul: &mut Soul) -> MemoryApplyReport {
        let mut report = MemoryApplyReport::default();
        for operation in &self.memory_operations {
            if apply_memory_operation(soul, operation) {
                report.events.push(MemoryApplyEvent {
                    action: MemoryApplyAction::Merged,
                    source_type: operation.resolved_source_type(&operation.content),
                    content: operation.content.clone(),
                    reason: operation.operation.clone(),
                });
            }
        }
        for memory in self
            .new_memories
            .iter()
            .take(memory_patch_limit(&self.new_memories))
        {
            if memory.operation.as_deref().is_some_and(|op| {
                matches!(
                    normalized_operation(op).as_str(),
                    "invalidate"
                        | "replace"
                        | "update"
                        | "mark_retconned"
                        | "supersede"
                        | "pin"
                        | "unpin"
                        | "restore_archived"
                        | "unarchive"
                )
            }) {
                if apply_memory_operation(soul, memory) {
                    report.events.push(MemoryApplyEvent {
                        action: MemoryApplyAction::Merged,
                        source_type: memory.resolved_source_type(&memory.content),
                        content: memory.content.clone(),
                        reason: memory.operation.clone(),
                    });
                }
                continue;
            }
            let Some(content) = memory.content() else {
                continue;
            };
            let cleaned_content = sanitize_memory_content(content);
            let content = cleaned_content.as_str();
            let source_type = memory.resolved_source_type(content);
            if is_generic_memory_content(content) {
                report.events.push(MemoryApplyEvent {
                    action: MemoryApplyAction::RejectedGeneric,
                    source_type,
                    content: content.to_string(),
                    reason: Some("generic filler memory".into()),
                });
                continue;
            }
            if source_type != MemorySourceType::ImportedLog {
                if let Some(existing_index) = duplicate_memory_index(soul, content, memory.tag()) {
                    report.events.push(MemoryApplyEvent {
                        action: MemoryApplyAction::RejectedDuplicate,
                        source_type,
                        content: content.to_string(),
                        reason: Some(format!(
                            "similar to existing memory {}",
                            soul.memory.recent[existing_index].id
                        )),
                    });
                    continue;
                }
            }
            if source_type != MemorySourceType::ImportedLog {
                if let Some(existing_index) = mergeable_memory_index(soul, content, memory.tag()) {
                    let existing = &mut soul.memory.recent[existing_index];
                    existing.salience = existing.salience.max(55.0).min(100.0);
                    existing.retrieval_strength = existing.retrieval_strength.max(55.0).min(100.0);
                    if existing.confidence.is_none() {
                        existing.confidence = memory
                            .confidence
                            .filter(|confidence| confidence.is_finite());
                    }
                    report.events.push(MemoryApplyEvent {
                        action: MemoryApplyAction::Merged,
                        source_type,
                        content: content.to_string(),
                        reason: Some(format!("merged into memory {}", existing.id)),
                    });
                    continue;
                }
            }
            let tag = memory.tag();
            let mut recent = create_scored_memory(soul, content, tag);
            if let Some(memory_id) = memory.cleaned_optional(&memory.memory_id) {
                recent.id = memory_id;
            }
            recent.source_type = source_type;
            recent.source_session_id = memory.cleaned_optional(&memory.source_session_id);
            recent.source_conversation_id = memory.cleaned_optional(&memory.source_conversation_id);
            recent.source_message_id = memory.source_message_id;
            recent.source_entity_id = memory.cleaned_optional(&memory.source_entity_id);
            recent.is_imported_context = memory
                .is_imported_context
                .unwrap_or_else(|| source_type.imported_or_cross_session());
            recent.is_lived_experience = memory.is_lived_experience.unwrap_or_else(|| {
                !matches!(
                    source_type,
                    MemorySourceType::ImportedLog
                        | MemorySourceType::PreviousSession
                        | MemorySourceType::CrossSessionBleed
                        | MemorySourceType::UserClaimed
                )
            });
            recent.perceived_by_entity_id = memory.cleaned_optional(&memory.perceived_by_entity_id);
            recent.target_entity_ids = memory.cleaned_targets();
            recent.interpretation = memory.cleaned_optional(&memory.interpretation);
            recent.confidence = memory
                .confidence
                .filter(|confidence| confidence.is_finite())
                .or_else(|| matches!(source_type, MemorySourceType::Unknown).then_some(0.45));
            recent.objective_event_id = memory.cleaned_optional(&memory.objective_event_id);
            recent.superseded_by_memory_id = memory.cleaned_optional(&memory.supersedes_memory_id);
            recent.truth_status = memory.truth_status.unwrap_or_default();
            recent.architecture_verified = memory.architecture_verified.unwrap_or(false)
                && recent.truth_status.is_engine_verified();
            recent.memory_slot = memory.cleaned_optional(&memory.memory_slot);
            recent.owner_soul_id = memory.cleaned_optional(&memory.owner_soul_id);
            recent.relevance_tags = memory
                .relevance_tags
                .iter()
                .map(|(key, score)| (key.trim().to_string(), (*score).min(100)))
                .filter(|(key, _)| !key.is_empty())
                .collect();
            recent.knowledge_scope = memory.cleaned_optional(&memory.knowledge_scope);
            let mut action = MemoryApplyAction::Added;
            if is_generic_emotional_reaction(content, tag) {
                recent.salience = (recent.salience * 0.55).min(40.0);
                recent.retrieval_strength = (recent.retrieval_strength * 0.55).min(40.0);
                action = MemoryApplyAction::Deprioritized;
            }
            boost_high_value_memory(
                &mut recent.salience,
                &mut recent.retrieval_strength,
                content,
                tag,
            );
            if let Some(salience) = clamped_memory_score(memory.salience) {
                recent.salience = salience;
            }
            if let Some(retrieval_strength) = clamped_memory_score(memory.retrieval_strength) {
                recent.retrieval_strength = retrieval_strength;
            }
            let event_content = recent.content.clone();
            soul.memory.recent.push(recent);
            report.added += 1;
            report.events.push(MemoryApplyEvent {
                action,
                source_type,
                content: event_content,
                reason: None,
            });
        }
        if report.added > 0 {
            soul.memory.recent.sort_by(|left, right| {
                right
                    .salience
                    .partial_cmp(&left.salience)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            // Archive overflow beyond the active cap instead of deleting it.
            // Archived memories leave the prompt-candidate pool but stay stored,
            // restorable, and exportable. Pinned memories are exempt and do not
            // consume cap slots.
            let mut active_seen = 0usize;
            for memory in soul.memory.recent.iter_mut() {
                if memory.is_pinned || memory.archived || !memory.is_active {
                    continue;
                }
                active_seen += 1;
                if active_seen > MAX_RECENT_MEMORIES {
                    memory.archived = true;
                    memory.is_active = false;
                }
            }
            // Hard ceiling so the stored pool cannot grow without bound; the vec is
            // salience-sorted, so this drops the lowest-salience unpinned overflow.
            if soul.memory.recent.len() > MAX_STORED_MEMORIES {
                let mut overflow = soul.memory.recent.len() - MAX_STORED_MEMORIES;
                let mut index = soul.memory.recent.len();
                while overflow > 0 && index > 0 {
                    index -= 1;
                    if !soul.memory.recent[index].is_pinned {
                        soul.memory.recent.remove(index);
                        overflow -= 1;
                    }
                }
            }
        }
        report
    }
}

fn memory_patch_limit(memories: &[MemoryPatch]) -> usize {
    if memories.iter().any(|memory| {
        memory.source_type == Some(MemorySourceType::ImportedLog)
            || infer_memory_source_type(&memory.content) == Some(MemorySourceType::ImportedLog)
    }) {
        3
    } else {
        memories.len()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct MemoryApplyReport {
    added: usize,
    events: Vec<MemoryApplyEvent>,
}

fn apply_memory_operation(soul: &mut Soul, memory: &MemoryPatch) -> bool {
    let operation = normalized_operation(memory.operation.as_deref().unwrap_or("add"));
    let target_id = memory
        .cleaned_optional(&memory.target_memory_id)
        .or_else(|| memory.cleaned_optional(&memory.memory_id));
    match operation.as_str() {
        "invalidate" | "mark_retconned" | "revert" => {
            let Some(target_id) = target_id else {
                return false;
            };
            let mut changed = false;
            for existing in &mut soul.memory.recent {
                if existing.id == target_id {
                    existing.is_active = false;
                    existing.is_retconned = true;
                    existing.invalidated_by_patch_id =
                        memory.cleaned_optional(&memory.objective_event_id);
                    changed = true;
                }
            }
            changed
        }
        "replace" | "supersede" | "update" => {
            let Some(target_id) = target_id else {
                return false;
            };
            let mut changed = false;
            for existing in &mut soul.memory.recent {
                if existing.id == target_id {
                    existing.is_active = false;
                    existing.is_retconned = true;
                    existing.superseded_by_memory_id = memory.cleaned_optional(&memory.memory_id);
                    changed = true;
                }
            }
            if operation == "update" {
                return changed;
            }
            changed
        }
        // Ledger-replayable curation operations: pin state and archive
        // restoration must flow through patches so a materialized rebuild
        // reproduces them.
        "pin" | "unpin" => {
            let Some(target_id) = target_id else {
                return false;
            };
            crate::memory::set_memory_pinned(soul, &target_id, operation == "pin")
        }
        "restore_archived" | "unarchive" => {
            let Some(target_id) = target_id else {
                return false;
            };
            crate::memory::restore_archived_memory(soul, &target_id)
        }
        _ => false,
    }
}

impl RelationshipDelta {
    fn is_empty(&self) -> bool {
        self.trust.is_none()
            && self.affection.is_none()
            && self.intimacy.is_none()
            && self.passion.is_none()
            && self.commitment.is_none()
            && self.fear.is_none()
            && self.desire.is_none()
            && self.respect.is_none()
            && self.conflict.is_none()
            && self.dependency.is_none()
            && self.curiosity.is_none()
            && self.comfort.is_none()
            && self.boundary_pressure.is_none()
            && self.trustable_bias.is_none()
            && self.untrustworthy_bias.is_none()
            && self.asshole_bias.is_none()
            && self.care_bias.is_none()
            && self.danger_bias.is_none()
            && self.competence_bias.is_none()
            && self.autonomy_respect_bias.is_none()
            && self.attachment_pull.is_none()
            && self.schema_threat.is_none()
            && self.first_impression_strength.is_none()
            && self.first_impression_confidence.is_none()
            && self.reappraisal_debt.is_none()
            && self.reappraisal_state_code.is_none()
    }

    fn target_name(&self) -> String {
        self.target
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .unwrap_or("user")
            .to_string()
    }
}

impl MemoryPatch {
    fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
            && self
                .operation
                .as_deref()
                .map_or(true, |op| op.trim().is_empty())
            && self
                .memory_id
                .as_deref()
                .map_or(true, |id| id.trim().is_empty())
            && self
                .target_memory_id
                .as_deref()
                .map_or(true, |id| id.trim().is_empty())
    }

    fn content(&self) -> Option<&str> {
        let content = self.content.trim();
        (!content.is_empty()).then_some(content)
    }

    fn tag(&self) -> &str {
        self.tag
            .as_deref()
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .unwrap_or("observation")
    }

    fn resolved_source_type(&self, content: &str) -> MemorySourceType {
        self.source_type.unwrap_or_else(|| {
            infer_memory_source_type(content).unwrap_or(MemorySourceType::CurrentSession)
        })
    }

    fn cleaned_optional(&self, value: &Option<String>) -> Option<String> {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    fn cleaned_targets(&self) -> Vec<String> {
        self.target_entity_ids
            .iter()
            .filter_map(|target| clean_str(target).map(str::to_string))
            .collect()
    }
}

impl MemoryLayerReplyPatch {
    fn is_empty(&self) -> bool {
        self.nonce.trim().is_empty() || self.content.trim().is_empty()
    }
}

fn infer_memory_source_type(text: &str) -> Option<MemorySourceType> {
    let lower = text.to_ascii_lowercase();
    if looks_like_imported_log(&lower) {
        Some(MemorySourceType::ImportedLog)
    } else if contains_any(
        &lower,
        &[
            "cross-session bleed",
            "cross session bleed",
            "another version",
            "parallel timeline",
            "imported memory",
            "memory bleed",
        ],
    ) {
        Some(MemorySourceType::CrossSessionBleed)
    } else if contains_any(
        &lower,
        &[
            "previous session",
            "prior session",
            "archived chat",
            "old chat log",
        ],
    ) {
        Some(MemorySourceType::PreviousSession)
    } else {
        None
    }
}

fn sanitize_memory_content(content: &str) -> String {
    let mut cleaned = content.trim().to_string();
    for marker in ["Context cue:", "context cue:"] {
        if let Some(index) = cleaned.find(marker) {
            cleaned.truncate(index);
        }
    }
    cleaned
        .trim()
        .trim_end_matches(['.', ';', ','])
        .trim()
        .to_string()
}

fn looks_like_imported_log(lower_text: &str) -> bool {
    lower_text.contains("# mnemosyne chat log")
        || (lower_text.contains("## user")
            && lower_text.contains("## narrator")
            && lower_text.contains("created:"))
        || lower_text.contains("# mnemosyne llm payload history")
}

fn is_generic_memory_content(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    let normalized = normalize_memory_text(content);
    normalized.split_whitespace().count() <= 3
        || contains_any(
            &lower,
            &[
                "neutral exchange added texture",
                "the exchange added emotional texture",
                "the scene continued",
                "she observed the user's words",
                "a recent chat provided context",
                "recent chat is available",
                "fresh scene context",
                "context cue",
            ],
        )
}

fn is_generic_emotional_reaction(content: &str, tag: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    matches!(
        tag,
        "observation" | "minor_observation" | "routine" | "small_talk"
    ) && contains_any(
        &lower,
        &[
            "felt tense",
            "felt nervous",
            "was tense",
            "stayed tense",
            "reacted emotionally",
            "listened to the user",
            "noticed the user's words",
            "watched the user",
        ],
    )
}

fn boost_high_value_memory(
    salience: &mut f32,
    retrieval_strength: &mut f32,
    content: &str,
    tag: &str,
) {
    let lower = content.to_ascii_lowercase();
    if matches!(
        tag,
        "identity_continuity"
            | "identity_change"
            | "promise"
            | "commitment"
            | "relationship_turning_point"
            | "breakthrough"
            | "threat"
            | "orientation"
    ) || contains_any(
        &lower,
        &[
            "promise",
            "commitment",
            "vow",
            "identity",
            "turning point",
            "confession",
            "learned the truth",
            "active plot",
            "warrant",
            "evacuation",
        ],
    ) {
        *salience = salience.max(70.0);
        *retrieval_strength = retrieval_strength.max(70.0);
    }
}

fn duplicate_memory_index(soul: &Soul, content: &str, tag: &str) -> Option<usize> {
    similar_memory_index(soul, content, tag, 0.82)
}

fn mergeable_memory_index(soul: &Soul, content: &str, tag: &str) -> Option<usize> {
    similar_memory_index(soul, content, tag, 0.64)
}

fn similar_memory_index(soul: &Soul, content: &str, tag: &str, threshold: f32) -> Option<usize> {
    let new_tokens = memory_token_set(content);
    if new_tokens.is_empty() {
        return None;
    }
    soul.memory
        .recent
        .iter()
        .enumerate()
        .skip(
            soul.memory
                .recent
                .len()
                .saturating_sub(MEMORY_HYGIENE_SCAN_LIMIT),
        )
        .filter(|(_, memory)| memory.tag == tag)
        .find(|(_, memory)| {
            memory_similarity(&new_tokens, &memory_token_set(&memory.content)) >= threshold
        })
        .map(|(index, _)| index)
}

fn memory_token_set(text: &str) -> Vec<String> {
    let mut tokens = normalize_memory_text(text)
        .split_whitespace()
        .filter(|token| token.len() > 2)
        .map(str::to_string)
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

fn normalize_memory_text(text: &str) -> String {
    text.to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character.is_whitespace() {
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

fn memory_similarity(left: &[String], right: &[String]) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.iter().filter(|token| right.contains(token)).count();
    let union = left.len() + right.len() - intersection;
    intersection as f32 / union as f32
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

impl WorldPatch {
    pub fn is_empty_for_commands(&self) -> bool {
        self.is_empty()
    }

    fn is_empty(&self) -> bool {
        self.location
            .as_deref()
            .map_or(true, |location| location.trim().is_empty())
            && self
                .time_elapsed
                .as_deref()
                .map_or(true, |time| time.trim().is_empty())
            && self
                .scene_state
                .as_ref()
                .map_or(true, SceneStatePatch::is_empty)
            && self
                .recent_event
                .as_deref()
                .map_or(true, |event| event.trim().is_empty())
            && self
                .recent_events
                .iter()
                .all(|event| event.trim().is_empty())
            && self
                .active_plot_add
                .iter()
                .all(|plot| plot.trim().is_empty())
            && self
                .active_plot_resolve
                .iter()
                .all(|plot| plot.trim().is_empty())
            && self
                .key_object_add
                .iter()
                .all(|object| object.trim().is_empty())
            && self
                .key_object_remove
                .iter()
                .all(|object| object.trim().is_empty())
            && self
                .event_operations
                .iter()
                .all(WorldEventOperationPatch::is_empty)
            && self
                .object_observation_operations
                .iter()
                .all(ObjectObservationOperationPatch::is_empty)
            && self
                .corrected_object_states
                .iter()
                .all(|object| object.object_id.trim().is_empty())
            && self.invalidated_recent_event_ids.is_empty()
            && self
                .correction_note
                .as_deref()
                .map_or(true, |note| note.trim().is_empty())
            && self
                .retcon_scope
                .as_deref()
                .map_or(true, |scope| scope.trim().is_empty())
    }

    fn apply(&self, soul: &mut Soul) -> bool {
        self.apply_to_world_log(&mut soul.world)
    }

    pub fn object_consistency_notices(
        &self,
        world: &crate::soul::WorldLog,
    ) -> Vec<WorldObjectConsistencyNotice> {
        let mut checked_world = world.clone();
        for object_state in &self.corrected_object_states {
            upsert_object_state(&mut checked_world, object_state);
        }
        let mut notices = Vec::new();
        if let Some(note) = cleaned(&self.correction_note) {
            if is_retcon_or_correction_text(note) {
                notices.push(WorldObjectConsistencyNotice {
                    event: note.to_string(),
                    object_id: None,
                    reason: "retcon/correction note routes through correction handling".into(),
                    rejected: false,
                    correction: true,
                });
            }
        }
        if let Some(event) = cleaned(&self.recent_event) {
            if let Some(notice) = world_event_consistency_notice(&checked_world, event) {
                notices.push(notice);
            }
        }
        notices.extend(
            self.recent_events
                .iter()
                .filter_map(|event| clean_str(event))
                .filter_map(|event| world_event_consistency_notice(&checked_world, event)),
        );
        notices
    }

    pub fn apply_to_world_log(&self, world: &mut WorldLog) -> bool {
        let mut changed = false;
        if let Some(location) = cleaned(&self.location) {
            world.location = location.to_string();
            changed = true;
        }
        if let Some(time_elapsed) = cleaned(&self.time_elapsed) {
            world.time_elapsed = normalize_time_elapsed(time_elapsed);
            changed = true;
        }
        if let Some(scene_state) = self.scene_state.as_ref() {
            changed |= apply_scene_state_patch(&mut world.scene_state, scene_state);
        }

        for operation in &self.event_operations {
            changed |= apply_world_event_operation(world, operation);
        }
        for operation in &self.object_observation_operations {
            changed |= apply_object_observation_operation(world, operation);
        }
        for object_state in &self.corrected_object_states {
            changed |= upsert_object_state(world, object_state);
        }
        if let Some(note) = cleaned(&self.correction_note) {
            changed |= apply_retcon_correction_note(world, note);
        }

        if let Some(event) = cleaned(&self.recent_event) {
            changed |= apply_recent_event_with_consistency_guard(world, event);
        }
        for event in self
            .recent_events
            .iter()
            .filter_map(|event| clean_str(event))
        {
            changed |= apply_recent_event_with_consistency_guard(world, event);
        }

        for plot in self
            .active_plot_add
            .iter()
            .filter_map(|plot| clean_str(plot))
        {
            changed |= push_unique(&mut world.active_plots, plot);
        }
        for plot in self
            .active_plot_resolve
            .iter()
            .filter_map(|plot| clean_str(plot))
        {
            changed |= remove_value(&mut world.active_plots, plot);
        }
        for object in self
            .key_object_add
            .iter()
            .filter_map(|object| clean_str(object))
        {
            changed |= push_unique(&mut world.key_objects, object);
        }
        for object in self
            .key_object_remove
            .iter()
            .filter_map(|object| clean_str(object))
        {
            changed |= remove_value(&mut world.key_objects, object);
        }

        changed
    }

    pub fn apply_to_session_world(&self, session_world: &mut SessionWorld) -> bool {
        let mut world = session_world.world_log();
        let changed = self.apply_to_world_log(&mut world);
        if changed {
            session_world.set_world_log(&world);
            session_world.last_updated = current_timestamp() as i64;
        }
        changed
    }
}

impl BodyPatch {
    pub fn is_empty_for_commands(&self) -> bool {
        self.is_empty()
    }

    fn has_arousal_bridge(&self) -> bool {
        self.activation_delta.is_some()
            || self.activation_blocked.is_some()
            || self.peak_allowed.is_some()
            || self.forced_peak.is_some()
    }

    fn is_empty(&self) -> bool {
        !self.has_arousal_bridge()
            && self.region_updates.is_empty()
            && self.condition_updates.is_empty()
    }

    fn apply(&self, soul: &mut Soul) -> bool {
        if !self.has_arousal_bridge() {
            return false;
        }
        soul.arousal.apply_signal(ArousalSignal {
            delta: finite_delta(self.activation_delta.unwrap_or(0.0), -30.0, 60.0),
            denied: self.activation_blocked.unwrap_or(false),
            orgasm_allowed: self.peak_allowed.unwrap_or(false),
            forced_orgasm: self.forced_peak.unwrap_or(false),
        });
        true
    }
}

impl SensoryPatch {
    fn is_empty(&self) -> bool {
        self.association_updates.iter().all(|update| {
            update
                .sense
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
                && update
                    .cue
                    .as_deref()
                    .map_or(true, |value| value.trim().is_empty())
                && update
                    .association
                    .as_deref()
                    .map_or(true, |value| value.trim().is_empty())
                && update.strength_delta.is_none()
        })
    }
}

fn apply_delta_with_cap(value: &mut f32, delta: Option<f32>, max_abs_delta: f32) {
    let max_abs_delta = finite_delta(max_abs_delta, 0.0, 30.0);
    *value = (*value + finite_delta(delta.unwrap_or(0.0), -max_abs_delta, max_abs_delta))
        .clamp(RELATIONSHIP_SCALAR_MIN, RELATIONSHIP_SCALAR_MAX);
}

fn finite_delta(value: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        0.0
    }
}

fn apply_relationship_delta(soul: &mut Soul, delta: &RelationshipDelta) -> bool {
    if delta.is_empty() {
        return false;
    }

    let target = relationship_storage_target(soul, &delta.target_name());
    if !soul.relationships.contains_key(&target) && is_player_persona_target(&target) {
        if let Some(existing) = soul
            .relationships
            .get("default_player")
            .or_else(|| soul.relationships.get("user"))
            .cloned()
        {
            soul.relationships.insert(target.clone(), existing);
        }
    }
    let relationship = soul
        .relationships
        .entry(target)
        .or_insert_with(default_relationship);
    let max_abs_delta = delta
        .max_abs_delta
        .unwrap_or(MAX_RELATIONSHIP_DELTA)
        .clamp(0.0, 30.0);
    apply_delta_with_cap(&mut relationship.trust, delta.trust, max_abs_delta);
    apply_delta_with_cap(&mut relationship.affection, delta.affection, max_abs_delta);
    apply_delta_with_cap(&mut relationship.intimacy, delta.intimacy, max_abs_delta);
    apply_delta_with_cap(&mut relationship.passion, delta.passion, max_abs_delta);
    apply_delta_with_cap(
        &mut relationship.commitment,
        delta.commitment,
        max_abs_delta,
    );
    apply_delta_with_cap(&mut relationship.fear, delta.fear, max_abs_delta);
    apply_delta_with_cap(&mut relationship.desire, delta.desire, max_abs_delta);
    apply_delta_with_cap(&mut relationship.respect, delta.respect, max_abs_delta);
    apply_delta_with_cap(&mut relationship.conflict, delta.conflict, max_abs_delta);
    apply_delta_with_cap(
        &mut relationship.dependency,
        delta.dependency,
        max_abs_delta,
    );
    apply_delta_with_cap(&mut relationship.curiosity, delta.curiosity, max_abs_delta);
    apply_delta_with_cap(&mut relationship.comfort, delta.comfort, max_abs_delta);
    apply_delta_with_cap(
        &mut relationship.boundary_pressure,
        delta.boundary_pressure,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.trustable_bias,
        delta.trustable_bias,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.untrustworthy_bias,
        delta.untrustworthy_bias,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.asshole_bias,
        delta.asshole_bias,
        max_abs_delta,
    );
    apply_delta_with_cap(&mut relationship.care_bias, delta.care_bias, max_abs_delta);
    apply_delta_with_cap(
        &mut relationship.danger_bias,
        delta.danger_bias,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.competence_bias,
        delta.competence_bias,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.autonomy_respect_bias,
        delta.autonomy_respect_bias,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.attachment_pull,
        delta.attachment_pull,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.schema_threat,
        delta.schema_threat,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.first_impression_strength,
        delta.first_impression_strength,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.first_impression_confidence,
        delta.first_impression_confidence,
        max_abs_delta,
    );
    apply_delta_with_cap(
        &mut relationship.reappraisal_debt,
        delta.reappraisal_debt,
        max_abs_delta,
    );
    if let Some(code) = delta.reappraisal_state_code {
        relationship.reappraisal_state_code = code.min(4);
    }
    true
}

fn relationship_storage_target(soul: &Soul, target: &str) -> String {
    if target.eq_ignore_ascii_case("default_player")
        && soul.relationships.contains_key("user")
        && !soul.relationships.contains_key("default_player")
    {
        "user".into()
    } else if target.eq_ignore_ascii_case("user")
        && soul.relationships.contains_key("default_player")
    {
        "default_player".into()
    } else {
        target.to_string()
    }
}

fn is_player_persona_target(target: &str) -> bool {
    let normalized = target.trim().to_ascii_lowercase();
    normalized.starts_with("preset_") || normalized.starts_with("persona_")
}

fn default_relationship() -> Relationship {
    Relationship {
        trust: 0.0,
        affection: 0.0,
        intimacy: 0.0,
        passion: 0.0,
        commitment: 0.0,
        fear: 0.0,
        desire: 0.0,
        respect: 0.0,
        conflict: 0.0,
        dependency: 0.0,
        curiosity: 0.0,
        comfort: 0.0,
        boundary_pressure: 0.0,
        love_type: String::new(),
        ..Relationship::default()
    }
}

fn cleaned(value: &Option<String>) -> Option<&str> {
    value.as_deref().and_then(clean_str)
}

fn clean_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn normalized_operation(operation: &str) -> String {
    operation
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .replace(' ', "_")
}

pub const MOCK_WORLD_FRAME_RUPTURE: &str = "The conversation continued without a major rupture";
pub const PENDING_USER_INPUT_PREFIX: &str = "pending_user_input:";

/// Returns true when an event records user input as completed world truth before narration.
pub fn is_premature_user_turn_event(event: &str, pending_user_text: Option<&str>) -> bool {
    let event = event.trim();
    if event.is_empty() {
        return true;
    }
    if event
        .to_ascii_lowercase()
        .starts_with(PENDING_USER_INPUT_PREFIX)
    {
        return true;
    }
    if event.contains(MOCK_WORLD_FRAME_RUPTURE) {
        return true;
    }
    let user = pending_user_text.map(str::trim).unwrap_or("");
    if user.is_empty() {
        return false;
    }
    if event == user {
        return true;
    }
    let suffix = format!(": {user}");
    event.ends_with(&suffix) || event.ends_with(&format!("{suffix}."))
}

pub fn purge_premature_recent_events_from_world(
    world: &mut WorldLog,
    pending_user_text: Option<&str>,
) {
    world
        .recent_events
        .retain(|event| !is_premature_user_turn_event(event, pending_user_text));
    world.recent_event_records.retain(|record| {
        !record.is_active || !is_premature_user_turn_event(&record.content, pending_user_text)
    });
    sync_recent_event_strings_from_records(world);
}

fn apply_recent_event_with_consistency_guard(world: &mut WorldLog, event: &str) -> bool {
    if is_premature_user_turn_event(event, None) {
        return false;
    }
    if is_retcon_or_correction_text(event) {
        return apply_retcon_correction_note(world, event);
    }
    if world_event_consistency_notice(world, event)
        .as_ref()
        .is_some_and(|notice| notice.rejected)
    {
        return false;
    }
    push_recent_event_to_world(world, event)
}

fn apply_scene_state_patch(scene_state: &mut SceneState, patch: &SceneStatePatch) -> bool {
    let before = scene_state.clone();
    if let Some(value) = patch.scene_state_id.as_deref().and_then(clean_str) {
        scene_state.scene_state_id = value.to_string();
    }
    if let Some(value) = patch.current_scene.as_deref().and_then(clean_str) {
        scene_state.current_scene = value.to_string();
    }
    if let Some(value) = patch.resolved_active_plot.as_deref().and_then(clean_str) {
        scene_state.resolved_active_plot = value.to_string();
    }
    if let Some(value) = patch.scene_branch.as_deref().and_then(clean_str) {
        scene_state.scene_branch = value.to_string();
    }
    if let Some(value) = patch.focus.as_deref().and_then(clean_str) {
        scene_state.focus = value.to_string();
    }
    let participants = patch
        .participants
        .iter()
        .filter_map(|participant| clean_str(participant))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !participants.is_empty() {
        scene_state.participants = participants;
    }
    if let Some(value) = patch.last_user_action.as_deref().and_then(clean_str) {
        scene_state.last_user_action = value.to_string();
    }
    if let Some(value) = patch.pressure_point.as_deref().and_then(clean_str) {
        scene_state.pressure_point = value.to_string();
    }
    if let Some(value) = patch.continuity_note.as_deref().and_then(clean_str) {
        scene_state.continuity_note = value.to_string();
    }
    *scene_state != before
}

fn apply_world_event_operation(world: &mut WorldLog, operation: &WorldEventOperationPatch) -> bool {
    let op = normalized_operation(&operation.operation);
    match op.as_str() {
        "add" | "add_recent_event" => {
            let Some(content) = operation.content.as_deref().and_then(clean_str) else {
                return false;
            };
            apply_recent_event_record_with_guard(
                world,
                operation
                    .recent_event_id
                    .as_deref()
                    .and_then(clean_str)
                    .map(str::to_string),
                content,
            )
        }
        "replace" | "replace_recent_event" | "supersede" => {
            let target = operation
                .target_recent_event_id
                .as_deref()
                .and_then(clean_str);
            let mut changed = target
                .map(|target| {
                    invalidate_recent_event(
                        world,
                        target,
                        operation.invalidated_by_patch_id.as_deref(),
                    )
                })
                .unwrap_or(false);
            if let Some(content) = operation.content.as_deref().and_then(clean_str) {
                changed |= apply_recent_event_record_with_guard(
                    world,
                    operation
                        .recent_event_id
                        .as_deref()
                        .and_then(clean_str)
                        .map(str::to_string),
                    content,
                );
            }
            changed
        }
        "invalidate" | "invalidate_recent_event" | "revert" => operation
            .target_recent_event_id
            .as_deref()
            .and_then(clean_str)
            .map(|target| {
                invalidate_recent_event(world, target, operation.invalidated_by_patch_id.as_deref())
            })
            .unwrap_or(false),
        "clear_recent_event_matching" => operation
            .match_text
            .as_deref()
            .and_then(clean_str)
            .map(|needle| {
                clear_recent_events_matching(
                    world,
                    needle,
                    operation.invalidated_by_patch_id.as_deref(),
                )
            })
            .unwrap_or(false),
        "add_correction_note" => operation
            .content
            .as_deref()
            .and_then(clean_str)
            .map(|content| apply_retcon_correction_note(world, content))
            .unwrap_or(false),
        "no_op" | "" => false,
        _ => false,
    }
}

fn apply_object_observation_operation(
    world: &mut WorldLog,
    operation: &ObjectObservationOperationPatch,
) -> bool {
    let op = normalized_operation(&operation.operation);
    match op.as_str() {
        "update" | "update_object_state" | "replace" | "replace_object_state" => operation
            .object_state
            .as_ref()
            .map(|state| {
                let mut state = state.clone();
                if state.object_observation_id.is_none() {
                    state.object_observation_id = operation
                        .object_observation_id
                        .as_deref()
                        .and_then(clean_str)
                        .map(str::to_string);
                }
                upsert_object_state(world, &state)
            })
            .unwrap_or(false),
        "invalidate" | "invalidate_object_observation" => operation
            .target_object_observation_id
            .as_deref()
            .and_then(clean_str)
            .map(|target| {
                let before = world.object_states.len();
                world.object_states.retain(|state| {
                    state.object_observation_id.as_deref() != Some(target)
                        && state.object_id != target
                });
                world.object_states.len() != before
            })
            .unwrap_or(false),
        "no_op" | "" => false,
        _ => false,
    }
}

fn apply_recent_event_record_with_guard(
    world: &mut WorldLog,
    recent_event_id: Option<String>,
    event: &str,
) -> bool {
    if is_premature_user_turn_event(event, None) {
        return false;
    }
    if is_retcon_or_correction_text(event) {
        return apply_retcon_correction_note(world, event);
    }
    if world_event_consistency_notice(world, event)
        .as_ref()
        .is_some_and(|notice| notice.rejected)
    {
        return false;
    }
    push_recent_event_record_to_world(world, recent_event_id, event)
}

fn apply_retcon_correction_note(world: &mut WorldLog, note: &str) -> bool {
    let mut changed = remove_contradictory_phone_events(world);
    let correction = if note.to_ascii_lowercase().contains("phone") {
        "Retcon: phone did not buzz because notifications were off / no vibration / screen wake disabled."
    } else {
        "Retcon: previous contradictory scene event was invalidated and should not be used as objective world state."
    };
    if !world.recent_events.iter().any(|event| event == correction) {
        push_recent_event_to_world(world, correction);
        changed = true;
    }
    changed
}

fn remove_contradictory_phone_events(world: &mut WorldLog) -> bool {
    let before = world.recent_events.len();
    let snapshot = world.clone();
    world.recent_events.retain(|event| {
        !world_event_consistency_notice(&snapshot, event)
            .as_ref()
            .is_some_and(|notice| notice.rejected)
    });
    let event_before = world.recent_event_records.len();
    world.recent_event_records.retain(|event| {
        !world_event_consistency_notice(&snapshot, &event.content)
            .as_ref()
            .is_some_and(|notice| notice.rejected)
    });
    world.recent_events.len() != before || world.recent_event_records.len() != event_before
}

fn upsert_object_state(world: &mut WorldLog, object_state: &ObjectState) -> bool {
    let object_id = object_state.object_id.trim();
    if object_id.is_empty() {
        return false;
    }
    if let Some(existing) = world
        .object_states
        .iter_mut()
        .find(|existing| existing.object_id.eq_ignore_ascii_case(object_id))
    {
        if existing == object_state {
            return false;
        }
        *existing = object_state.clone();
        true
    } else if let Some(index) = unknown_owner_object_alias_index(world, object_state) {
        if world.object_states[index] == *object_state {
            return false;
        }
        world.object_states[index] = object_state.clone();
        true
    } else {
        world.object_states.push(object_state.clone());
        true
    }
}

fn unknown_owner_object_alias_index(world: &WorldLog, object_state: &ObjectState) -> Option<usize> {
    let new_owner = object_state
        .owner_entity_id
        .as_deref()
        .and_then(clean_str)?;
    if new_owner.eq_ignore_ascii_case("unknown") {
        return None;
    }
    let object_kind = clean_str(&object_state.object_kind)?;
    world.object_states.iter().position(|existing| {
        clean_str(&existing.object_kind).is_some_and(|kind| kind.eq_ignore_ascii_case(object_kind))
            && existing
                .owner_entity_id
                .as_deref()
                .and_then(clean_str)
                .map(|owner| owner.eq_ignore_ascii_case("unknown"))
                .unwrap_or(true)
            && existing
                .object_id
                .to_ascii_lowercase()
                .starts_with(&format!("unknown_{}_", object_kind.to_ascii_lowercase()))
    })
}

fn world_event_consistency_notice(
    world: &WorldLog,
    event: &str,
) -> Option<WorldObjectConsistencyNotice> {
    if !event_claims_phone_notification_activity(event) {
        return None;
    }
    let Some((object_id, reason)) = phone_state_blocks_notification_activity(world) else {
        return None;
    };
    Some(WorldObjectConsistencyNotice {
        event: event.to_string(),
        object_id,
        reason,
        rejected: true,
        correction: false,
    })
}

fn event_claims_phone_notification_activity(event: &str) -> bool {
    let lower = event.to_ascii_lowercase();
    lower.contains("phone")
        && contains_any(
            &lower,
            &[
                "buzz",
                "vibrat",
                "ping",
                "notification",
                "screen woke",
                "screen wake",
                "screen lit",
                "lit up",
                "wake up",
            ],
        )
}

fn phone_state_blocks_notification_activity(world: &WorldLog) -> Option<(Option<String>, String)> {
    for object_state in &world.object_states {
        if !object_state
            .object_id
            .to_ascii_lowercase()
            .contains("phone")
        {
            continue;
        }
        let notification_mode = object_state.notification_mode.to_ascii_lowercase();
        if matches!(
            notification_mode.as_str(),
            "notifications_off" | "notifications off" | "do_not_disturb" | "silent"
        ) {
            return Some((
                Some(object_state.object_id.clone()),
                format!(
                    "phone notification mode is {}",
                    object_state.notification_mode
                ),
            ));
        }
        if object_state.vibrate_enabled == Some(false) {
            return Some((
                Some(object_state.object_id.clone()),
                "phone vibration is disabled".into(),
            ));
        }
        if object_state.screen_wake_enabled == Some(false) {
            return Some((
                Some(object_state.object_id.clone()),
                "phone screen wake is disabled".into(),
            ));
        }
    }
    for key_object in &world.key_objects {
        let lower = key_object.to_ascii_lowercase();
        if !lower.contains("phone") {
            continue;
        }
        if lower.contains("notifications off")
            || lower.contains("notification off")
            || lower.contains("do not disturb")
            || lower.contains("do_not_disturb")
            || lower.contains("silent")
        {
            return Some((
                Some(key_object.clone()),
                "legacy key object says phone notifications are off/silent".into(),
            ));
        }
        if lower.contains("vibration disabled") || lower.contains("no vibration") {
            return Some((
                Some(key_object.clone()),
                "legacy key object says no vibration".into(),
            ));
        }
        if lower.contains("screen wake disabled") || lower.contains("no screen wake") {
            return Some((
                Some(key_object.clone()),
                "legacy key object says screen wake is disabled".into(),
            ));
        }
    }
    None
}

pub fn is_retcon_or_correction_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    contains_any(
        &lower,
        &[
            "ooc: that's wrong",
            "ooc: that is wrong",
            "that's wrong",
            "that is wrong",
            "redo the scene",
            "retcon",
            "how does",
            "contradicts the setting",
            "contradict",
        ],
    )
}

fn invalidate_recent_event(
    world: &mut WorldLog,
    recent_event_id: &str,
    invalidated_by_patch_id: Option<&str>,
) -> bool {
    let mut changed = false;
    for event in &mut world.recent_event_records {
        if event.recent_event_id == recent_event_id && event.is_active {
            event.is_active = false;
            event.invalidated_by_patch_id = invalidated_by_patch_id.map(str::to_string);
            changed = true;
        }
    }
    if changed {
        sync_recent_event_strings_from_records(world);
    }
    changed
}

fn clear_recent_events_matching(
    world: &mut WorldLog,
    needle: &str,
    invalidated_by_patch_id: Option<&str>,
) -> bool {
    let needle = needle.to_ascii_lowercase();
    let mut changed = false;
    for event in &mut world.recent_event_records {
        if event.is_active && event.content.to_ascii_lowercase().contains(&needle) {
            event.is_active = false;
            event.invalidated_by_patch_id = invalidated_by_patch_id.map(str::to_string);
            changed = true;
        }
    }
    let before = world.recent_events.len();
    world
        .recent_events
        .retain(|event| !event.to_ascii_lowercase().contains(&needle));
    changed |= world.recent_events.len() != before;
    if changed {
        sync_recent_event_strings_from_records(world);
    }
    changed
}

fn push_recent_event_to_world(world: &mut WorldLog, event: &str) -> bool {
    push_recent_event_record_to_world(world, None, event)
}

fn push_recent_event_record_to_world(
    world: &mut WorldLog,
    recent_event_id: Option<String>,
    event: &str,
) -> bool {
    let normalized = normalize_recent_event_for_dedupe(event);
    if normalized.is_empty()
        || world
            .recent_event_records
            .iter()
            .filter(|record| record.is_active)
            .any(|record| normalize_recent_event_for_dedupe(&record.content) == normalized)
        || world
            .recent_events
            .iter()
            .any(|existing| normalize_recent_event_for_dedupe(existing) == normalized)
    {
        return false;
    }
    let recent_event_id = recent_event_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| stable_recent_event_id(event, world.recent_event_records.len()));
    world.recent_event_records.push(WorldEventRecord {
        recent_event_id,
        content: event.to_string(),
        is_active: true,
        invalidated_by_patch_id: None,
        superseded_by_event_id: None,
        created_at: 0,
    });
    world.recent_events.push(event.to_string());
    if world.recent_events.len() > MAX_RECENT_EVENTS {
        let remove_count = world.recent_events.len() - MAX_RECENT_EVENTS;
        world.recent_events.drain(0..remove_count);
    }
    if world.recent_event_records.len() > MAX_RECENT_EVENTS {
        let remove_count = world.recent_event_records.len() - MAX_RECENT_EVENTS;
        world.recent_event_records.drain(0..remove_count);
    }
    true
}

fn normalize_recent_event_for_dedupe(event: &str) -> String {
    let mut normalized = event
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character.is_ascii_whitespace() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>();
    for (from, to) in [
        ("aurora schwarz", "aurora"),
        ("the user", "user"),
        ("player", "user"),
        ("knocked", "knock"),
        ("knocking", "knock"),
        ("knocks", "knock"),
        ("responded", "respond"),
        ("responds", "respond"),
        ("answered", "answer"),
        ("answers", "answer"),
        ("apartment door", "door"),
        ("front door", "door"),
        ("the door", "door"),
    ] {
        normalized = normalized.replace(from, to);
    }
    let tokens = normalized
        .split_whitespace()
        .filter(|token| {
            !matches!(
                *token,
                "the"
                    | "a"
                    | "an"
                    | "s"
                    | "and"
                    | "then"
                    | "that"
                    | "to"
                    | "on"
                    | "at"
                    | "in"
                    | "into"
                    | "her"
                    | "his"
                    | "their"
                    | "with"
            )
        })
        .collect::<Vec<_>>();
    if tokens.contains(&"user") && tokens.contains(&"knock") && tokens.contains(&"door") {
        return "user knock door".to_string();
    }
    tokens.join(" ")
}

fn clamped_memory_score(value: Option<f32>) -> Option<f32> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 100.0))
}

fn sync_recent_event_strings_from_records(world: &mut WorldLog) {
    if world.recent_event_records.is_empty() {
        return;
    }
    world.recent_events = world
        .recent_event_records
        .iter()
        .filter(|event| event.is_active)
        .map(|event| event.content.clone())
        .collect();
    if world.recent_events.len() > MAX_RECENT_EVENTS {
        let remove_count = world.recent_events.len() - MAX_RECENT_EVENTS;
        world.recent_events.drain(0..remove_count);
    }
}

fn stable_recent_event_id(event: &str, ordinal: usize) -> String {
    format!(
        "recent_event_{:016x}",
        stable_hash(&format!("{ordinal}:{event}"))
    )
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn push_unique(values: &mut Vec<String>, value: &str) -> bool {
    if values.iter().any(|existing| existing == value) {
        return false;
    }
    values.push(value.to_string());
    true
}

fn remove_value(values: &mut Vec<String>, value: &str) -> bool {
    let before = values.len();
    values.retain(|existing| existing != value);
    values.len() != before
}

fn normalize_time_elapsed(raw: &str) -> String {
    let trimmed = raw.trim();
    const PREFIX: &str = "Session start";
    let Some(found) = trimmed.find(PREFIX) else {
        return trimmed.to_string();
    };
    let suffix = trimmed[found + PREFIX.len()..]
        .trim_start_matches(['.', ' ', '-', ':'])
        .trim();
    if suffix.is_empty() {
        PREFIX.into()
    } else {
        suffix.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::new_default_soul;

    #[test]
    fn hidden_state_converts_to_engine_patch() {
        let hidden_state = HiddenState {
            memory: Some("Aurora keeps the promise in mind.".into()),
            tag: Some("trust_building".into()),
            trust_delta: Some(3.0),
            affection_delta: Some(1.0),
            world_event: Some("A promise changed the room.".into()),
            new_location: Some("Safehouse".into()),
            present_characters: Some(vec!["Aurora".into()]),
            arousal_delta: None,
            arousal_denied: None,
            orgasm_allowed: None,
            forced_orgasm: None,
        };

        let patch = EnginePatch::from(&hidden_state);

        assert_eq!(patch.schema_version, Some(PATCH_PROTOCOL_VERSION));
        assert_eq!(
            patch
                .soul_patch
                .as_ref()
                .and_then(|patch| patch.relationship_delta.as_ref())
                .and_then(|delta| delta.trust),
            Some(3.0)
        );
        assert_eq!(
            patch
                .world_patch
                .as_ref()
                .and_then(|patch| patch.location.as_deref()),
            Some("Safehouse")
        );
    }

    #[test]
    fn relationship_deltas_are_clamped() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                relationship_delta: Some(RelationshipDelta {
                    target: Some("user".into()),
                    trust: Some(999.0),
                    affection: Some(-999.0),
                    fear: Some(f32::NAN),
                    ..RelationshipDelta::default()
                }),
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(soul.relationships["user"].trust, 20.0);
        assert_eq!(soul.relationships["user"].affection, 10.0);
        assert_eq!(soul.relationships["user"].fear, 10.0);
    }

    #[test]
    fn targeted_relationship_deltas_apply_separately() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                relationship_deltas: vec![
                    RelationshipDelta {
                        from: Some("aurora".into()),
                        target: Some("junhwa".into()),
                        trust: Some(-2.0),
                        fear: Some(5.0),
                        conflict: Some(8.0),
                        ..RelationshipDelta::default()
                    },
                    RelationshipDelta {
                        from: Some("aurora".into()),
                        target: Some("rhy".into()),
                        trust: Some(3.0),
                        curiosity: Some(5.0),
                        ..RelationshipDelta::default()
                    },
                ],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        let report = patch.apply_to_soul(&mut soul).expect("patch applies");

        assert!(report.relationship_updated);
        assert_eq!(soul.relationships["junhwa"].trust, 0.0);
        assert_eq!(soul.relationships["junhwa"].fear, 5.0);
        assert_eq!(soul.relationships["junhwa"].conflict, 8.0);
        assert_eq!(soul.relationships["rhy"].trust, 3.0);
        assert_eq!(soul.relationships["rhy"].curiosity, 5.0);
    }

    #[test]
    fn default_player_delta_updates_legacy_user_relationship() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                relationship_deltas: vec![RelationshipDelta {
                    target: Some("default_player".into()),
                    trust: Some(4.0),
                    ..RelationshipDelta::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(soul.relationships["user"].trust, 14.0);
        assert!(!soul.relationships.contains_key("default_player"));
    }

    #[test]
    fn player_persona_delta_preserves_legacy_relationship_values() {
        let mut soul = new_default_soul("Aurora");
        soul.relationships.get_mut("user").unwrap().trust = 27.0;
        soul.relationships.get_mut("user").unwrap().fear = 9.0;
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                relationship_deltas: vec![RelationshipDelta {
                    target: Some("persona_jun".into()),
                    trust: Some(3.0),
                    ..RelationshipDelta::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        let persona_relationship = soul
            .relationships
            .get("persona_jun")
            .expect("persona relationship");
        assert_eq!(persona_relationship.trust, 30.0);
        assert_eq!(persona_relationship.fear, 9.0);
        assert_eq!(soul.relationships["user"].trust, 27.0);
    }

    #[test]
    fn explicit_world_time_patch_applies() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            world_patch: Some(WorldPatch {
                time_elapsed: Some("Ten minutes later".into()),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        let report = patch.apply_to_soul(&mut soul).expect("patch applies");

        assert!(report.world_updated);
        assert_eq!(soul.world.time_elapsed, "Ten minutes later");
    }

    #[test]
    fn malformed_session_start_time_is_normalized_on_apply() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            world_patch: Some(WorldPatch {
                time_elapsed: Some("Session startLate evening, just after midnight.".into()),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(
            soul.world.time_elapsed,
            "Late evening, just after midnight."
        );
    }

    #[test]
    fn memory_patch_creates_scored_memory() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                new_memories: vec![MemoryPatch {
                    content: "Aurora remembers the hidden stairwell.".into(),
                    tag: Some("orientation".into()),
                    ..MemoryPatch::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        let report = patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(report.memories_added, 1);
        assert_eq!(soul.memory.recent.len(), 1);
        assert_eq!(soul.memory.recent[0].tag, "orientation");
        assert!(soul.memory.recent[0].salience > 0.0);
        assert_eq!(
            soul.memory.recent[0].source_type,
            MemorySourceType::CurrentSession
        );
        assert!(soul.memory.recent[0].is_lived_experience);
    }

    #[test]
    fn memory_overflow_is_archived_not_deleted() {
        let mut soul = new_default_soul("Aurora");
        // Contents must be lexically distinct or they merge instead of adding.
        let contents = [
            "Aurora found a brass key beneath the chapel flagstones.",
            "The northern bridge collapsed during the spring flood.",
            "Marlow keeps carrier pigeons trained for coded letters.",
            "A silver coin from Veldt buys passage on the ferry.",
            "The lighthouse keeper sleeps through every second watch.",
            "Wild thyme grows along the ruined aqueduct walls.",
            "The blacksmith owes a gambling debt to the harbor guild.",
            "Lantern oil is rationed after the warehouse fire.",
            "An old map marks a well hidden inside the orchard.",
            "The mayor's daughter studies astronomy in secret.",
            "Wolves avoid the valley when the mill wheel turns.",
            "A sealed letter waits under the tavern floorboard.",
            "The glassblower trades mirrors for foreign spices.",
            "Frost ruined the eastern vineyard two winters back.",
            "The chapel bell cracks slightly on every third toll.",
            "Smugglers use the tide caves below the white cliffs.",
            "A retired soldier tends the graves outside the gate.",
        ];
        assert!(contents.len() > MAX_RECENT_MEMORIES);
        for (index, content) in contents.iter().enumerate() {
            let patch = EnginePatch {
                schema_version: Some(PATCH_PROTOCOL_VERSION),
                soul_patch: Some(SoulPatch {
                    new_memories: vec![MemoryPatch {
                        content: (*content).into(),
                        tag: Some("orientation".into()),
                        salience: Some(40.0 + index as f32),
                        ..MemoryPatch::default()
                    }],
                    ..SoulPatch::default()
                }),
                ..EnginePatch::default()
            };
            patch.apply_to_soul(&mut soul).expect("patch applies");
        }

        let active = soul
            .memory
            .recent
            .iter()
            .filter(|memory| memory.is_active && !memory.archived)
            .count();
        let archived = soul
            .memory
            .recent
            .iter()
            .filter(|memory| memory.archived && !memory.is_active)
            .count();

        assert_eq!(active, MAX_RECENT_MEMORIES, "active pool stays capped");
        assert!(archived >= 1, "overflow is archived instead of deleted");
        assert_eq!(
            soul.memory.recent.len(),
            active + archived,
            "no memory is hard-deleted below the stored ceiling"
        );
        // Archived entries are the lower-salience ones.
        let min_active = soul
            .memory
            .recent
            .iter()
            .filter(|memory| memory.is_active)
            .map(|memory| memory.salience)
            .fold(f32::INFINITY, f32::min);
        let max_archived = soul
            .memory
            .recent
            .iter()
            .filter(|memory| memory.archived)
            .map(|memory| memory.salience)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(min_active >= max_archived);
    }

    #[test]
    fn pin_and_restore_flow_through_patch_operations() {
        // Pin state must be reproducible by replaying the ledger, so it is
        // applied via memory_operations rather than direct soul mutation.
        let add = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                new_memories: vec![MemoryPatch {
                    memory_id: Some("mem_oath".into()),
                    content: "Aurora swore the river oath at dawn.".into(),
                    tag: Some("promise".into()),
                    ..MemoryPatch::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };
        let pin = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                memory_operations: vec![MemoryPatch {
                    operation: Some("pin".into()),
                    target_memory_id: Some("mem_oath".into()),
                    ..MemoryPatch::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        // Apply on one soul, then replay the same ledger on a fresh soul:
        // both must agree.
        for replay in 0..2 {
            let mut soul = new_default_soul("Aurora");
            add.apply_to_soul(&mut soul).expect("add applies");
            pin.apply_to_soul(&mut soul).expect("pin applies");
            let memory = soul
                .memory
                .recent
                .iter()
                .find(|memory| memory.id == "mem_oath")
                .expect("memory exists");
            assert!(memory.is_pinned, "replay {replay}: memory is pinned");
        }
    }

    #[test]
    fn pinned_memory_is_exempt_from_archival_eviction() {
        let mut soul = new_default_soul("Aurora");
        let mut pinned =
            crate::memory::create_scored_memory(&soul, "Aurora swore the river oath.", "promise");
        pinned.id = "mem_pinned".into();
        pinned.salience = 1.0; // lowest salience: first in line for eviction
        pinned.is_pinned = true;
        soul.memory.recent.push(pinned);

        let contents = [
            "Aurora found a brass key beneath the chapel flagstones.",
            "The northern bridge collapsed during the spring flood.",
            "Marlow keeps carrier pigeons trained for coded letters.",
            "A silver coin from Veldt buys passage on the ferry.",
            "The lighthouse keeper sleeps through every second watch.",
            "Wild thyme grows along the ruined aqueduct walls.",
            "The blacksmith owes a gambling debt to the harbor guild.",
            "Lantern oil is rationed after the warehouse fire.",
            "An old map marks a well hidden inside the orchard.",
            "The mayor's daughter studies astronomy in secret.",
            "Wolves avoid the valley when the mill wheel turns.",
            "A sealed letter waits under the tavern floorboard.",
            "The glassblower trades mirrors for foreign spices.",
            "Frost ruined the eastern vineyard two winters back.",
        ];
        for (index, content) in contents.iter().enumerate() {
            let patch = EnginePatch {
                schema_version: Some(PATCH_PROTOCOL_VERSION),
                soul_patch: Some(SoulPatch {
                    new_memories: vec![MemoryPatch {
                        content: (*content).into(),
                        tag: Some("orientation".into()),
                        salience: Some(40.0 + index as f32),
                        ..MemoryPatch::default()
                    }],
                    ..SoulPatch::default()
                }),
                ..EnginePatch::default()
            };
            patch.apply_to_soul(&mut soul).expect("patch applies");
        }

        let pinned = soul
            .memory
            .recent
            .iter()
            .find(|memory| memory.id == "mem_pinned")
            .expect("pinned memory still stored");
        assert!(pinned.is_active, "pinned memory must never be archived");
        assert!(!pinned.archived);

        let active_unpinned = soul
            .memory
            .recent
            .iter()
            .filter(|memory| memory.is_active && !memory.is_pinned)
            .count();
        assert_eq!(
            active_unpinned, MAX_RECENT_MEMORIES,
            "unpinned active pool stays capped"
        );
    }

    #[test]
    fn imported_log_memory_is_not_lived_experience() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                new_memories: vec![MemoryPatch {
                    content: "# Mnemosyne Chat Log\n## User\nHello\n## Narrator\nAurora argued about ownership.\nCreated: 100".into(),
                    tag: Some("identity_continuity".into()),
                    ..MemoryPatch::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        let report = patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(report.memories_added, 1);
        assert_eq!(
            soul.memory.recent[0].source_type,
            MemorySourceType::ImportedLog
        );
        assert!(!soul.memory.recent[0].is_lived_experience);
        assert!(soul.memory.recent[0].is_imported_context);
    }

    #[test]
    fn duplicate_memory_is_rejected() {
        let mut soul = new_default_soul("Aurora");
        let first = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                new_memories: vec![MemoryPatch {
                    content: "Aurora accepted the user's careful promise to return.".into(),
                    tag: Some("promise".into()),
                    ..MemoryPatch::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };
        first.apply_to_soul(&mut soul).expect("first applies");
        let duplicate = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                new_memories: vec![MemoryPatch {
                    content: "Aurora accepted the user's careful promise to return.".into(),
                    tag: Some("promise".into()),
                    ..MemoryPatch::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        let report = duplicate
            .apply_to_soul(&mut soul)
            .expect("duplicate applies");

        assert_eq!(report.memories_added, 0);
        assert_eq!(soul.memory.recent.len(), 1);
        assert!(report
            .memory_events
            .iter()
            .any(|event| event.action == MemoryApplyAction::RejectedDuplicate));
    }

    #[test]
    fn generic_filler_memory_is_rejected() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                new_memories: vec![MemoryPatch {
                    content: "The scene continued.".into(),
                    tag: Some("observation".into()),
                    ..MemoryPatch::default()
                }],
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        let report = patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(report.memories_added, 0);
        assert!(soul.memory.recent.is_empty());
        assert!(report
            .memory_events
            .iter()
            .any(|event| event.action == MemoryApplyAction::RejectedGeneric));
    }

    #[test]
    fn imported_log_memory_patch_is_capped_to_three_summaries() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            soul_patch: Some(SoulPatch {
                new_memories: (0..8)
                    .map(|index| MemoryPatch {
                        content: format!(
                            "# Mnemosyne Chat Log\n## User\nentry {index}\n## Narrator\nimported durable fact {index}: {}.\nCreated: {index}",
                            ["red key", "blue door", "silver promise", "green lab", "old map", "black terminal", "white room", "gold thread"][index]
                        ),
                        tag: Some("identity_continuity".into()),
                        ..MemoryPatch::default()
                    })
                    .collect(),
                ..SoulPatch::default()
            }),
            ..EnginePatch::default()
        };

        let report = patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(report.memories_added, 3);
        assert_eq!(soul.memory.recent.len(), 3);
        assert!(soul.memory.recent.iter().all(|memory| {
            memory.source_type == MemorySourceType::ImportedLog
                && !memory.is_lived_experience
                && memory.is_imported_context
        }));
    }

    #[test]
    fn world_patch_updates_location_and_recent_events() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            world_patch: Some(WorldPatch {
                location: Some("Service tunnel".into()),
                recent_event: Some("Aurora found a locked gate.".into()),
                active_plot_add: vec!["Open the locked gate".into()],
                key_object_add: vec!["Rusty key".into()],
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(soul.world.location, "Service tunnel");
        assert_eq!(soul.world.recent_events.len(), 1);
        assert!(soul
            .world
            .active_plots
            .contains(&"Open the locked gate".to_string()));
        assert!(soul.world.key_objects.contains(&"Rusty key".to_string()));
    }

    #[test]
    fn world_patch_deduplicates_recent_events() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            world_patch: Some(WorldPatch {
                recent_event: Some("Aurora closed the door.".into()),
                recent_events: vec![
                    "Aurora closed the door".into(),
                    "  Aurora closed the door.  ".into(),
                ],
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(soul.world.recent_events.len(), 1);
        assert_eq!(soul.world.recent_event_records.len(), 1);
    }

    #[test]
    fn recent_event_semantic_dedup_knock() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            world_patch: Some(WorldPatch {
                recent_event: Some("User knocked on Aurora's apartment door.".into()),
                event_operations: vec![WorldEventOperationPatch {
                    operation: "add_recent_event".into(),
                    recent_event_id: Some("knock_response".into()),
                    content: Some(
                        "The user knock on the door; Aurora responded from inside.".into(),
                    ),
                    ..WorldEventOperationPatch::default()
                }],
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(soul.world.recent_events.len(), 1);
        assert_eq!(soul.world.recent_event_records.len(), 1);
    }

    #[test]
    fn world_patch_updates_resolved_scene_state() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            world_patch: Some(WorldPatch {
                scene_state: Some(SceneStatePatch {
                    current_scene: Some("Aurora is at the apartment door.".into()),
                    resolved_active_plot: Some("Door argument".into()),
                    scene_branch: Some("Door stayed closed".into()),
                    last_user_action: Some("User challenged the contradiction OOC.".into()),
                    ..SceneStatePatch::default()
                }),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(soul.world.scene_state.resolved_active_plot, "Door argument");
        assert_eq!(soul.world.scene_state.scene_branch, "Door stayed closed");
    }

    #[test]
    fn typed_object_state_supports_doors_without_phone_fields() {
        let mut soul = new_default_soul("Aurora");
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            world_patch: Some(WorldPatch {
                object_observation_operations: vec![ObjectObservationOperationPatch {
                    operation: "update_object_state".into(),
                    object_state: Some(ObjectState {
                        object_id: "apartment_door".into(),
                        object_kind: "door".into(),
                        open_state: Some("closed".into()),
                        lock_state: Some("locked".into()),
                        status: "closed and locked".into(),
                        ..ObjectState::default()
                    }),
                    ..ObjectObservationOperationPatch::default()
                }],
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        let door = soul.world.object_states.first().expect("door state");
        assert_eq!(door.object_kind, "door");
        assert_eq!(door.open_state.as_deref(), Some("closed"));
        assert_eq!(door.notification_mode, "unknown");
    }

    #[test]
    fn apply_to_session_routes_world_patch_to_session_world() {
        let mut soul = Soul::default_for_character("Echo-0");
        soul.world.location = "Legacy room".into();
        let mut session_world = crate::setting::session_world_from_legacy_world(
            "Testing Room",
            Some("world-source".into()),
            &crate::soul::WorldLog {
                location: "Session start room".into(),
                active_plots: Vec::new(),
                recent_events: Vec::new(),
                key_objects: Vec::new(),
                time_elapsed: "Session start".into(),
                ..crate::soul::WorldLog::default()
            },
        );
        let patch = EnginePatch {
            world_patch: Some(WorldPatch {
                location: Some("Session lab".into()),
                recent_event: Some("Echo-0 crossed the lab.".into()),
                active_plot_add: vec!["Verify routing".into()],
                key_object_add: vec!["debug terminal".into()],
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        let report = patch
            .apply_to_session(&mut soul, Some(&mut session_world))
            .expect("patch applies");

        assert!(report.world_updated);
        assert_eq!(soul.world.location, "Legacy room");
        assert_eq!(session_world.location, "Session lab");
        assert!(session_world
            .recent_events
            .contains(&"Echo-0 crossed the lab.".into()));
        assert!(session_world
            .active_plots
            .contains(&"Verify routing".into()));
        assert!(session_world.key_objects.contains(&"debug terminal".into()));
    }

    #[test]
    fn premature_recent_event_rejected_at_patch_apply() {
        let mut world = WorldLog::default();
        let applied = apply_recent_event_with_consistency_guard(
            &mut world,
            "The conversation continued without a major rupture: I knock on the door",
        );
        assert!(!applied);
        assert!(world.recent_events.is_empty());
    }

    fn apply_to_session_legacy_fallback_mutates_soul_world_without_session_world() {
        let mut soul = Soul::default_for_character("Echo-0");
        soul.world.location = "Legacy room".into();
        let patch = EnginePatch {
            world_patch: Some(WorldPatch {
                location: Some("Legacy fallback room".into()),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        let report = patch
            .apply_to_session(&mut soul, None)
            .expect("patch applies");

        assert!(report.world_updated);
        assert_eq!(soul.world.location, "Legacy fallback room");
    }

    #[test]
    fn invalid_or_empty_patch_does_not_corrupt_soul() {
        let mut soul = new_default_soul("Aurora");
        let original = soul.clone();

        EnginePatch::default()
            .apply_to_soul(&mut soul)
            .expect("empty patch is valid");
        assert_eq!(soul, original);

        let invalid = EnginePatch {
            schema_version: Some(999),
            world_patch: Some(WorldPatch {
                location: Some("Should not apply".into()),
                ..WorldPatch::default()
            }),
            ..EnginePatch::default()
        };

        assert_eq!(
            invalid.apply_to_soul(&mut soul),
            Err(PatchError::UnsupportedSchemaVersion(999))
        );
        assert_eq!(soul, original);
    }

    #[test]
    fn body_patch_placeholders_do_not_invoke_arousal_bridge() {
        let mut soul = new_default_soul("Aurora");
        soul.arousal.level = 42.0;
        let patch = EnginePatch {
            schema_version: Some(PATCH_PROTOCOL_VERSION),
            body_patch: Some(BodyPatch {
                region_updates: vec![serde_json::json!({ "region_id": "hand" })],
                condition_updates: vec![serde_json::json!({ "id": "bruised" })],
                ..BodyPatch::default()
            }),
            ..EnginePatch::default()
        };

        patch.apply_to_soul(&mut soul).expect("patch applies");

        assert_eq!(soul.arousal.level, 42.0);
    }

    #[test]
    fn sensory_patch_deserializes_without_mutating_soul() {
        let json = br#"{"schema_version":1,"sensory_patch":{"association_updates":[{"sense":"sound","cue":"rain","association":"comfort","strength_delta":10.0}]}}"#;
        let patch: EnginePatch = serde_json::from_slice(json).expect("deserialize");
        let mut soul = new_default_soul("Aurora");
        let snapshot = soul.clone();

        patch.apply_to_soul(&mut soul).expect("apply");

        assert_eq!(soul, snapshot);
    }
}
