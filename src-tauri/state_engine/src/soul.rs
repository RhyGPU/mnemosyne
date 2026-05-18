use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{arousal::ArousalState, schema::CURRENT_SCHEMA_VERSION};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Soul {
    pub schema_version: u32,
    pub character_id: String,
    pub character_name: String,
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
    pub world: WorldLog,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CharacterProfile {
    pub description: String,
    pub appearance: String,
    pub personality: String,
    pub scenario: String,
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

const fn default_lived_experience() -> bool {
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaEntry {
    pub schema_type: String,
    pub summary: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldLog {
    pub location: String,
    pub active_plots: Vec<String>,
    pub recent_events: Vec<String>,
    pub key_objects: Vec<String>,
    pub time_elapsed: String,
}

impl Default for WorldLog {
    fn default() -> Self {
        Self {
            location: "Unspecified starting scene.".into(),
            active_plots: vec!["Establish the first scene".into()],
            recent_events: Vec::new(),
            key_objects: Vec::new(),
            time_elapsed: "Session start".into(),
        }
    }
}

impl Soul {
    pub fn default_for_character(character_name: &str) -> Self {
        let now = current_timestamp();
        let mut relationships = HashMap::new();
        relationships.insert(
            "user".into(),
            Relationship {
                trust: 10.0,
                affection: 200.0,
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
    session.last_updated = now as i64;
    session
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
        });
        soul.memory.schemas.push(SchemaEntry {
            schema_type: "attachment_pattern".into(),
            summary: "Aurora remembers promises that felt stabilizing.".into(),
            count: 2,
        });
        soul.relationships.get_mut("user").unwrap().trust = 130.0;
        soul.relationships.get_mut("user").unwrap().affection = 126.0;

        let session = session_soul_from_savepoint(&soul);

        assert_ne!(session.character_id, soul.character_id);
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
