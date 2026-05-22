use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{arousal::ArousalState, schema::CURRENT_SCHEMA_VERSION};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Soul {
    pub schema_version: u32,
    pub character_id: String,
    pub character_name: String,
    #[serde(default = "default_soul_kind")]
    pub soul_kind: String,
    #[serde(default)]
    pub source_soul_id: Option<String>,
    #[serde(default)]
    pub source_savepoint_id: Option<String>,
    #[serde(default)]
    pub created_from_name: Option<String>,
    #[serde(default)]
    pub profile: CharacterProfile,
    pub last_updated: i64,
    pub turn_counter: u64,
    pub turns_since_consolidation: u64,
    pub global: GlobalState,
    pub trauma: TraumaState,
    pub relationships: HashMap<String, Relationship>,
    #[serde(default)]
    pub arousal: ArousalState,
    pub memory: MemoryStore,
    #[serde(default)]
    pub debug_memory_layer_replies: Vec<MemoryLayerReply>,
    #[serde(default)]
    pub world: WorldLog,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CharacterProfile {
    pub description: String,
    pub appearance: String,
    pub personality: String,
    pub scenario: String,
    #[serde(default)]
    pub opening_narrator_message: String,
    #[serde(default)]
    pub avatar_image_id: Option<String>,
}

fn default_soul_kind() -> String {
    "savepoint".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobalState {
    pub dev_stage: u8,
    pub attach_style: u8,
    pub fear_baseline: f32,
    pub resolve: f32,
    pub shame: f32,
    pub openness: f32,
    pub maslow: [f32; 5],
    pub sdt: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraumaState {
    pub phase: u8,
    pub symptoms: TraumaSymptoms,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraumaSymptoms {
    pub hypervigilance: f32,
    pub flashbacks: f32,
    pub numbing: f32,
    pub avoidance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    pub trust: f32,
    pub affection: f32,
    pub intimacy: f32,
    pub passion: f32,
    pub commitment: f32,
    pub fear: f32,
    pub desire: f32,
    #[serde(default)]
    pub respect: f32,
    #[serde(default)]
    pub conflict: f32,
    #[serde(default)]
    pub dependency: f32,
    #[serde(default)]
    pub curiosity: f32,
    #[serde(default)]
    pub comfort: f32,
    #[serde(default)]
    pub boundary_pressure: f32,
    pub love_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryStore {
    pub core: Vec<String>,
    pub recent: Vec<MemoryEntry>,
    pub schemas: Vec<SchemaEntry>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySourceType {
    CurrentSession,
    PreviousSession,
    ImportedLog,
    CrossSessionBleed,
    UserClaimed,
    NarratorInferred,
    SystemGenerated,
    PersistentCore,
    #[default]
    Unknown,
}

impl MemorySourceType {
    pub fn as_label(self) -> &'static str {
        match self {
            MemorySourceType::CurrentSession => "current_session",
            MemorySourceType::PreviousSession => "previous_session",
            MemorySourceType::ImportedLog => "imported_log",
            MemorySourceType::CrossSessionBleed => "cross_session_bleed",
            MemorySourceType::UserClaimed => "user_claimed",
            MemorySourceType::NarratorInferred => "narrator_inferred",
            MemorySourceType::SystemGenerated => "system_generated",
            MemorySourceType::PersistentCore => "persistent_core",
            MemorySourceType::Unknown => "unknown",
        }
    }

    pub fn imported_or_cross_session(self) -> bool {
        matches!(
            self,
            MemorySourceType::ImportedLog
                | MemorySourceType::PreviousSession
                | MemorySourceType::CrossSessionBleed
        )
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TruthStatus {
    Fiction,
    SceneEvent,
    CharacterBelief,
    NarratorClaim,
    UserClaimed,
    VerifiedEngine,
    ActualSystemEvent,
    #[default]
    Unknown,
}

impl TruthStatus {
    pub fn as_label(self) -> &'static str {
        match self {
            TruthStatus::Fiction => "fiction",
            TruthStatus::SceneEvent => "scene_event",
            TruthStatus::CharacterBelief => "character_belief",
            TruthStatus::NarratorClaim => "narrator_claim",
            TruthStatus::UserClaimed => "user_claimed",
            TruthStatus::VerifiedEngine => "verified_engine",
            TruthStatus::ActualSystemEvent => "actual_system_event",
            TruthStatus::Unknown => "unknown",
        }
    }

    pub fn is_engine_verified(self) -> bool {
        matches!(self, TruthStatus::VerifiedEngine | TruthStatus::ActualSystemEvent)
    }
}

const fn default_lived_experience() -> bool {
    true
}

const fn default_memory_active() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryEntry {
    pub id: String,
    pub timestamp: u64,
    pub content: String,
    pub salience: f32,
    pub tag: String,
    pub retrieval_strength: f32,
    #[serde(default)]
    pub source_type: MemorySourceType,
    #[serde(default)]
    pub source_session_id: Option<String>,
    #[serde(default)]
    pub source_conversation_id: Option<String>,
    #[serde(default)]
    pub source_message_id: Option<i64>,
    #[serde(default)]
    pub source_entity_id: Option<String>,
    #[serde(default = "default_lived_experience")]
    pub is_lived_experience: bool,
    #[serde(default)]
    pub is_imported_context: bool,
    #[serde(default)]
    pub perceived_by_entity_id: Option<String>,
    #[serde(default)]
    pub target_entity_ids: Vec<String>,
    #[serde(default)]
    pub interpretation: Option<String>,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub objective_event_id: Option<String>,
    #[serde(default)]
    pub truth_status: TruthStatus,
    #[serde(default)]
    pub architecture_verified: bool,
    #[serde(default)]
    pub memory_slot: Option<String>,
    #[serde(default)]
    pub owner_soul_id: Option<String>,
    #[serde(default)]
    pub relevance_tags: HashMap<String, u8>,
    #[serde(default)]
    pub knowledge_scope: Option<String>,
    #[serde(default = "default_memory_active")]
    pub is_active: bool,
    #[serde(default)]
    pub invalidated_by_patch_id: Option<String>,
    #[serde(default)]
    pub superseded_by_memory_id: Option<String>,
    #[serde(default)]
    pub is_retconned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryLayerReply {
    pub nonce: String,
    pub content: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub architecture_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaEntry {
    pub schema_type: String,
    pub summary: String,
    pub count: u64,
    #[serde(default)]
    pub schema_id: String,
    #[serde(default)]
    pub owner_soul_id: Option<String>,
    #[serde(default)]
    pub target_entity_ids: Vec<String>,
    #[serde(default)]
    pub trigger_tags: Vec<String>,
    #[serde(default)]
    pub salience: f32,
    #[serde(default)]
    pub reinforcement_count: u64,
    #[serde(default)]
    pub decay: f32,
    #[serde(default)]
    pub last_reinforced_turn: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlotStatus {
    Dominant,
    Background,
    Resolved,
    Stale,
    #[default]
    Unknown,
}

impl PlotStatus {
    pub fn as_label(self) -> &'static str {
        match self {
            PlotStatus::Dominant => "dominant",
            PlotStatus::Background => "background",
            PlotStatus::Resolved => "resolved",
            PlotStatus::Stale => "stale",
            PlotStatus::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlotEntry {
    pub plot_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub status: PlotStatus,
    #[serde(default)]
    pub salience: f32,
    #[serde(default)]
    pub started_turn: u64,
    #[serde(default)]
    pub last_touched_turn: u64,
    #[serde(default)]
    pub related_entities: Vec<String>,
    #[serde(default)]
    pub related_world_id: Option<String>,
    #[serde(default)]
    pub unresolved_questions: Vec<String>,
    #[serde(default)]
    pub resolution_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldLog {
    pub location: String,
    pub active_plots: Vec<String>,
    pub recent_events: Vec<String>,
    #[serde(default)]
    pub recent_event_records: Vec<WorldEventRecord>,
    pub key_objects: Vec<String>,
    #[serde(default)]
    pub object_states: Vec<ObjectState>,
    pub time_elapsed: String,
    #[serde(default)]
    pub scene_state: SceneState,
    #[serde(default)]
    pub dominant_current_plot: Option<PlotEntry>,
    #[serde(default)]
    pub background_plots: Vec<PlotEntry>,
    #[serde(default)]
    pub resolved_plots: Vec<PlotEntry>,
    #[serde(default)]
    pub stale_plot_decay: f32,
}

impl Default for WorldLog {
    fn default() -> Self {
        Self {
            location: "Unspecified starting scene.".into(),
            active_plots: vec!["Establish the first scene".into()],
            recent_events: Vec::new(),
            recent_event_records: Vec::new(),
            key_objects: Vec::new(),
            object_states: Vec::new(),
            time_elapsed: "Session start".into(),
            scene_state: SceneState::default(),
            dominant_current_plot: None,
            background_plots: Vec::new(),
            resolved_plots: Vec::new(),
            stale_plot_decay: 0.12,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneState {
    #[serde(default)]
    pub scene_state_id: String,
    #[serde(default)]
    pub current_scene: String,
    #[serde(default)]
    pub resolved_active_plot: String,
    #[serde(default)]
    pub scene_branch: String,
    #[serde(default)]
    pub focus: String,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub last_user_action: String,
    #[serde(default)]
    pub pressure_point: String,
    #[serde(default)]
    pub continuity_note: String,
}

impl Default for SceneState {
    fn default() -> Self {
        Self {
            scene_state_id: String::new(),
            current_scene: String::new(),
            resolved_active_plot: String::new(),
            scene_branch: String::new(),
            focus: String::new(),
            participants: Vec::new(),
            last_user_action: String::new(),
            pressure_point: String::new(),
            continuity_note: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldEventRecord {
    pub recent_event_id: String,
    pub content: String,
    #[serde(default = "default_world_event_active")]
    pub is_active: bool,
    #[serde(default)]
    pub invalidated_by_patch_id: Option<String>,
    #[serde(default)]
    pub superseded_by_event_id: Option<String>,
    #[serde(default)]
    pub created_at: u64,
}

const fn default_world_event_active() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjectState {
    #[serde(default)]
    pub object_observation_id: Option<String>,
    pub object_id: String,
    #[serde(default = "unknown_object_field")]
    pub object_kind: String,
    #[serde(default)]
    pub owner_entity_id: Option<String>,
    #[serde(default)]
    pub location: String,
    #[serde(default = "unknown_object_field")]
    pub status: String,
    #[serde(default)]
    pub open_state: Option<String>,
    #[serde(default)]
    pub lock_state: Option<String>,
    #[serde(default)]
    pub sealed: Option<bool>,
    #[serde(default)]
    pub contents_known: Option<bool>,
    #[serde(default)]
    pub contents_summary: Option<String>,
    #[serde(default)]
    pub properties: HashMap<String, String>,
    #[serde(default = "unknown_object_field")]
    pub power_state: String,
    #[serde(default = "unknown_object_field")]
    pub notification_mode: String,
    #[serde(default)]
    pub vibrate_enabled: Option<bool>,
    #[serde(default)]
    pub screen_wake_enabled: Option<bool>,
    #[serde(default)]
    pub can_receive_calls: Option<bool>,
    #[serde(default)]
    pub can_receive_texts: Option<bool>,
    #[serde(default)]
    pub last_observed_state: String,
    #[serde(default = "default_object_confidence")]
    pub confidence: f32,
}

impl Default for ObjectState {
    fn default() -> Self {
        Self {
            object_observation_id: None,
            object_id: String::new(),
            object_kind: unknown_object_field(),
            owner_entity_id: None,
            location: String::new(),
            status: unknown_object_field(),
            open_state: None,
            lock_state: None,
            sealed: None,
            contents_known: None,
            contents_summary: None,
            properties: HashMap::new(),
            power_state: unknown_object_field(),
            notification_mode: unknown_object_field(),
            vibrate_enabled: None,
            screen_wake_enabled: None,
            can_receive_calls: None,
            can_receive_texts: None,
            last_observed_state: String::new(),
            confidence: default_object_confidence(),
        }
    }
}

fn unknown_object_field() -> String {
    "unknown".into()
}

fn default_object_confidence() -> f32 {
    0.5
}

impl Soul {
    pub fn default_for_character(character_name: &str) -> Self {
        let now = current_timestamp();
        let mut relationships = HashMap::new();
        relationships.insert(
            "user".into(),
            Relationship {
                trust: 10.0,
                affection: 20.0,
                intimacy: 10.0,
                passion: 10.0,
                commitment: 10.0,
                fear: 10.0,
                desire: 20.0,
                respect: 10.0,
                conflict: 0.0,
                dependency: 0.0,
                curiosity: 10.0,
                comfort: 10.0,
                boundary_pressure: 0.0,
                love_type: String::new(),
            },
        );

        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            character_id: Uuid::new_v4().to_string(),
            character_name: character_name.trim().to_string(),
            soul_kind: default_soul_kind(),
            source_soul_id: None,
            source_savepoint_id: None,
            created_from_name: None,
            profile: CharacterProfile::default(),
            last_updated: now as i64,
            turn_counter: 0,
            turns_since_consolidation: 0,
            global: GlobalState {
                dev_stage: 6,
                attach_style: 2,
                fear_baseline: 15.0,
                resolve: 40.0,
                shame: 45.0,
                openness: 45.0,
                maslow: [60.0, 50.0, 40.0, 30.0, 20.0],
                sdt: [70.0, 40.0, 10.0],
            },
            trauma: TraumaState {
                phase: 2,
                symptoms: TraumaSymptoms {
                    hypervigilance: 10.0,
                    flashbacks: 10.0,
                    numbing: 10.0,
                    avoidance: 10.0,
                },
            },
            relationships,
            arousal: ArousalState::default(),
            memory: MemoryStore {
                core: vec![
                    "The Soul file has just been initialized; enduring identity is still forming."
                        .into(),
                ],
                recent: Vec::new(),
                schemas: Vec::new(),
            },
            debug_memory_layer_replies: Vec::new(),
            world: WorldLog::default(),
        }
    }
}

impl Default for Soul {
    fn default() -> Self {
        Self::default_for_character("Unnamed Character")
    }
}

pub fn new_default_soul(character_name: &str) -> Soul {
    Soul::default_for_character(character_name)
}

pub fn session_soul_from_savepoint(base: &Soul) -> Soul {
    let now = current_timestamp();
    let mut session = base.clone();
    session.character_id = Uuid::new_v4().to_string();
    session.soul_kind = "session_clone".into();
    session.source_soul_id = Some(base.character_id.clone());
    session.source_savepoint_id = Some(
        base.source_savepoint_id
            .clone()
            .unwrap_or_else(|| base.character_id.clone()),
    );
    session.created_from_name = Some(base.character_name.clone());
    session.debug_memory_layer_replies.clear();
    session.last_updated = now as i64;
    session
}

pub fn soul_savepoint_from_session(session: &Soul, name: &str, soul_kind: &str) -> Soul {
    let now = current_timestamp();
    let mut savepoint = session.clone();
    savepoint.character_id = Uuid::new_v4().to_string();
    savepoint.character_name = name.trim().to_string();
    savepoint.soul_kind = match soul_kind.trim() {
        "checkpoint" => "checkpoint".into(),
        "imported_package" => "imported_package".into(),
        _ => "savepoint".into(),
    };
    savepoint.source_soul_id = Some(session.character_id.clone());
    savepoint.source_savepoint_id = session.source_savepoint_id.clone();
    savepoint.created_from_name = Some(session.character_name.clone());
    savepoint.last_updated = now as i64;
    savepoint
}

pub fn neutral_user_relationship() -> Relationship {
    Relationship {
        trust: 10.0,
        affection: 20.0,
        intimacy: 10.0,
        passion: 10.0,
        commitment: 10.0,
        fear: 10.0,
        desire: 20.0,
        respect: 10.0,
        conflict: 0.0,
        dependency: 0.0,
        curiosity: 10.0,
        comfort: 10.0,
        boundary_pressure: 0.0,
        love_type: String::new(),
    }
}

pub fn current_timestamp() -> u64 {
    chrono::Utc::now().timestamp().max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soul_json_roundtrip_preserves_schema() {
        let soul = new_default_soul("Aurora Schwarz");
        let json = serde_json::to_string_pretty(&soul).expect("serialize");
        let decoded: Soul = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.schema_version, 1);
        assert_eq!(decoded.character_name, "Aurora Schwarz");
        assert_eq!(decoded.turn_counter, 0);
        assert!(decoded.relationships.contains_key("user"));
    }

    #[test]
    fn legacy_soul_json_defaults_profile() {
        let soul = new_default_soul("Aurora Schwarz");
        let mut value = serde_json::to_value(&soul).expect("value");
        value.as_object_mut().expect("object").remove("profile");
        let decoded: Soul = serde_json::from_value(value).expect("deserialize");

        assert_eq!(decoded.character_name, "Aurora Schwarz");
        assert_eq!(decoded.profile, CharacterProfile::default());
    }

    #[test]
    fn default_soul_kind_and_affection_are_savepoint_defaults() {
        let soul = new_default_soul("Aurora Schwarz");

        assert_eq!(soul.soul_kind, "savepoint");
        assert_eq!(soul.relationships["user"].affection, 20.0);
        assert!(soul.source_soul_id.is_none());
        assert!(soul.source_savepoint_id.is_none());
        assert!(soul.profile.opening_narrator_message.is_empty());
    }

    #[test]
    fn character_only_soul_json_defaults_world_log() {
        let soul = new_default_soul("Aurora Schwarz");
        let mut value = serde_json::to_value(&soul).expect("value");
        value.as_object_mut().expect("object").remove("world");
        let decoded: Soul = serde_json::from_value(value).expect("deserialize");

        assert_eq!(decoded.character_name, "Aurora Schwarz");
        assert_eq!(decoded.world, WorldLog::default());
    }

    #[test]
    fn legacy_soul_json_defaults_arousal() {
        let soul = new_default_soul("Aurora Schwarz");
        let mut value = serde_json::to_value(&soul).expect("value");
        value.as_object_mut().expect("object").remove("arousal");
        let decoded: Soul = serde_json::from_value(value).expect("deserialize");

        assert_eq!(decoded.character_name, "Aurora Schwarz");
        assert_eq!(decoded.arousal, ArousalState::default());
    }

    #[test]
    fn legacy_memory_json_defaults_source_metadata() {
        let raw = r#"{
            "id":"old",
            "timestamp":1,
            "content":"Aurora kept an old remembered fact.",
            "salience":75.0,
            "tag":"observation",
            "retrieval_strength":75.0
        }"#;

        let decoded: MemoryEntry = serde_json::from_str(raw).expect("legacy memory");

        assert_eq!(decoded.source_type, MemorySourceType::Unknown);
        assert!(decoded.is_lived_experience);
        assert!(!decoded.is_imported_context);
    }

    #[test]
    fn world_log_json_roundtrip_is_independent() {
        let mut world = WorldLog::default();
        world.location = "Carver City service tunnel".into();
        world.active_plots.push("Find the sealed stairwell".into());
        let json = serde_json::to_string_pretty(&world).expect("serialize");
        let decoded: WorldLog = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, world);
    }

    #[test]
    fn new_session_clones_selected_savepoint_fictional_state() {
        let mut soul = new_default_soul("Aurora Schwarz");
        soul.world.location = "Aurora's kitchen counter.".into();
        soul.world.recent_events = vec!["Romantic kitchen scene resolved.".into()];
        soul.world.active_plots = vec!["Interact with Rhy".into()];
        soul.world.key_objects = vec!["old phone on couch".into()];
        soul.world.time_elapsed = "One year after the first meeting.".into();
        soul.memory.recent.push(MemoryEntry {
            id: "recent".into(),
            timestamp: 1,
            content: "Aurora warmed to the user.".into(),
            salience: 90.0,
            tag: "bonding".into(),
            retrieval_strength: 90.0,
            source_type: MemorySourceType::CurrentSession,
            source_session_id: None,
            source_conversation_id: None,
            source_message_id: None,
            source_entity_id: None,
            is_lived_experience: true,
            is_imported_context: false,
            perceived_by_entity_id: None,
            target_entity_ids: Vec::new(),
            interpretation: None,
            confidence: None,
            objective_event_id: None,
            truth_status: TruthStatus::Unknown,
            architecture_verified: false,
            memory_slot: None,
            owner_soul_id: None,
            relevance_tags: Default::default(),
            knowledge_scope: None,
            is_active: true,
            invalidated_by_patch_id: None,
            superseded_by_memory_id: None,
            is_retconned: false,
        });
        soul.memory.schemas.push(SchemaEntry {
            schema_type: "attachment_pattern".into(),
            summary: "Aurora remembers promises that felt stabilizing.".into(),
            count: 2,
            schema_id: "attachment-pattern".into(),
            owner_soul_id: Some(soul.character_id.clone()),
            target_entity_ids: vec!["user".into()],
            trigger_tags: vec!["promise".into()],
            salience: 70.0,
            reinforcement_count: 2,
            decay: 0.0,
            last_reinforced_turn: soul.turn_counter,
        });
        soul.relationships.get_mut("user").unwrap().trust = 130.0;
        soul.relationships.get_mut("user").unwrap().affection = 126.0;

        let session = session_soul_from_savepoint(&soul);

        assert_ne!(session.character_id, soul.character_id);
        assert_eq!(session.soul_kind, "session_clone");
        assert_eq!(session.source_soul_id.as_deref(), Some(soul.character_id.as_str()));
        assert_eq!(
            session.source_savepoint_id.as_deref(),
            Some(soul.character_id.as_str())
        );
        assert_eq!(
            session.created_from_name.as_deref(),
            Some(soul.character_name.as_str())
        );
        assert_eq!(session.character_name, soul.character_name);
        assert_eq!(session.profile, soul.profile);
        assert_eq!(session.turn_counter, soul.turn_counter);
        assert_eq!(session.turns_since_consolidation, soul.turns_since_consolidation);
        assert_eq!(session.global, soul.global);
        assert_eq!(session.trauma, soul.trauma);
        assert_eq!(session.relationships, soul.relationships);
        assert_eq!(session.arousal, soul.arousal);
        assert_eq!(session.memory, soul.memory);
        assert_eq!(session.world, soul.world);

        let mut changed_session = session.clone();
        changed_session.world.location = "Different session room.".into();
        changed_session.relationships.get_mut("user").unwrap().trust = 1.0;
        changed_session.memory.recent[0].content = "Session-only change.".into();
        assert_eq!(soul.world.location, "Aurora's kitchen counter.");
        assert_eq!(soul.relationships["user"].trust, 130.0);
        assert_eq!(
            soul.memory.recent[0].source_type,
            MemorySourceType::CurrentSession
        );
        assert_eq!(soul.memory.recent[0].content, "Aurora warmed to the user.");
    }
}
