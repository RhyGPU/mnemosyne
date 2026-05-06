use serde::{Deserialize, Serialize};

use crate::{
    arousal::ArousalSignal,
    hidden_state::HiddenState,
    memory::create_scored_memory,
    soul::{current_timestamp, Relationship, Soul},
};

pub const PATCH_PROTOCOL_VERSION: u32 = 1;
const MAX_RELATIONSHIP_DELTA: f32 = 10.0;
/// Clamp each relationship scalar after deltas (matches engine defaults and leaves headroom above 100).
const RELATIONSHIP_SCALAR_MIN: f32 = 0.0;
const RELATIONSHIP_SCALAR_MAX: f32 = 300.0;
const MAX_RECENT_MEMORIES: usize = 12;
const MAX_RECENT_EVENTS: usize = 12;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EnginePatch {
    pub schema_version: Option<u32>,
    pub soul_patch: Option<SoulPatch>,
    pub world_patch: Option<WorldPatch>,
    pub body_patch: Option<BodyPatch>,
    pub sensory_patch: Option<SensoryPatch>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SoulPatch {
    pub relationship_delta: Option<RelationshipDelta>,
    pub new_memories: Vec<MemoryPatch>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RelationshipDelta {
    pub target: Option<String>,
    pub trust: Option<f32>,
    pub affection: Option<f32>,
    pub intimacy: Option<f32>,
    pub passion: Option<f32>,
    pub commitment: Option<f32>,
    pub fear: Option<f32>,
    pub desire: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MemoryPatch {
    pub content: String,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WorldPatch {
    pub location: Option<String>,
    pub recent_event: Option<String>,
    pub recent_events: Vec<String>,
    pub active_plot_add: Vec<String>,
    pub active_plot_resolve: Vec<String>,
    pub key_object_add: Vec<String>,
    pub key_object_remove: Vec<String>,
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
    pub world_updated: bool,
    pub body_updated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatchError {
    UnsupportedSchemaVersion(u32),
}

impl EnginePatch {
    pub fn apply_to_soul(&self, soul: &mut Soul) -> Result<PatchReport, PatchError> {
        self.validate()?;
        if self.is_empty() {
            return Ok(PatchReport::default());
        }

        let mut report = PatchReport::default();
        if let Some(soul_patch) = &self.soul_patch {
            report.relationship_updated = soul_patch.apply_relationship(soul);
            report.memories_added = soul_patch.apply_memories(soul);
        }
        if let Some(world_patch) = &self.world_patch {
            report.world_updated = world_patch.apply(soul);
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
        }
    }
}

impl SoulPatch {
    fn is_empty(&self) -> bool {
        self.relationship_delta
            .as_ref()
            .map_or(true, RelationshipDelta::is_empty)
            && self.new_memories.iter().all(MemoryPatch::is_empty)
    }

    fn apply_relationship(&self, soul: &mut Soul) -> bool {
        let Some(delta) = &self.relationship_delta else {
            return false;
        };
        if delta.is_empty() {
            return false;
        }

        let target = delta.target_name();
        let relationship = soul
            .relationships
            .entry(target)
            .or_insert_with(default_relationship);
        apply_delta(&mut relationship.trust, delta.trust);
        apply_delta(&mut relationship.affection, delta.affection);
        apply_delta(&mut relationship.intimacy, delta.intimacy);
        apply_delta(&mut relationship.passion, delta.passion);
        apply_delta(&mut relationship.commitment, delta.commitment);
        apply_delta(&mut relationship.fear, delta.fear);
        apply_delta(&mut relationship.desire, delta.desire);
        true
    }

    fn apply_memories(&self, soul: &mut Soul) -> usize {
        let mut added = 0;
        for memory in &self.new_memories {
            let Some(content) = memory.content() else {
                continue;
            };
            let tag = memory.tag();
            let recent = create_scored_memory(soul, content, tag);
            soul.memory.recent.push(recent);
            added += 1;
        }
        if added > 0 {
            soul.memory.recent.sort_by(|left, right| {
                right
                    .salience
                    .partial_cmp(&left.salience)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            soul.memory.recent.truncate(MAX_RECENT_MEMORIES);
        }
        added
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
}

impl WorldPatch {
    fn is_empty(&self) -> bool {
        self.location
            .as_deref()
            .map_or(true, |location| location.trim().is_empty())
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
    }

    fn apply(&self, soul: &mut Soul) -> bool {
        let mut changed = false;
        if let Some(location) = cleaned(&self.location) {
            soul.world.location = location.to_string();
            changed = true;
        }

        if let Some(event) = cleaned(&self.recent_event) {
            push_recent_event(soul, event);
            changed = true;
        }
        for event in self
            .recent_events
            .iter()
            .filter_map(|event| clean_str(event))
        {
            push_recent_event(soul, event);
            changed = true;
        }

        for plot in self
            .active_plot_add
            .iter()
            .filter_map(|plot| clean_str(plot))
        {
            changed |= push_unique(&mut soul.world.active_plots, plot);
        }
        for plot in self
            .active_plot_resolve
            .iter()
            .filter_map(|plot| clean_str(plot))
        {
            changed |= remove_value(&mut soul.world.active_plots, plot);
        }
        for object in self
            .key_object_add
            .iter()
            .filter_map(|object| clean_str(object))
        {
            changed |= push_unique(&mut soul.world.key_objects, object);
        }
        for object in self
            .key_object_remove
            .iter()
            .filter_map(|object| clean_str(object))
        {
            changed |= remove_value(&mut soul.world.key_objects, object);
        }

        changed
    }
}

impl BodyPatch {
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

fn apply_delta(value: &mut f32, delta: Option<f32>) {
    *value = (*value
        + finite_delta(
            delta.unwrap_or(0.0),
            -MAX_RELATIONSHIP_DELTA,
            MAX_RELATIONSHIP_DELTA,
        ))
    .clamp(RELATIONSHIP_SCALAR_MIN, RELATIONSHIP_SCALAR_MAX);
}

fn finite_delta(value: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        0.0
    }
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
        love_type: String::new(),
    }
}

fn cleaned(value: &Option<String>) -> Option<&str> {
    value.as_deref().and_then(clean_str)
}

fn clean_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn push_recent_event(soul: &mut Soul, event: &str) {
    soul.world.recent_events.push(event.to_string());
    if soul.world.recent_events.len() > MAX_RECENT_EVENTS {
        let remove_count = soul.world.recent_events.len() - MAX_RECENT_EVENTS;
        soul.world.recent_events.drain(0..remove_count);
    }
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
        assert_eq!(soul.relationships["user"].affection, 190.0);
        assert_eq!(soul.relationships["user"].fear, 10.0);
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
