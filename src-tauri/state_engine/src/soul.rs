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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryEntry {
    pub id: String,
    pub timestamp: u64,
    pub content: String,
    pub salience: f32,
    pub tag: String,
    pub retrieval_strength: f32,
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

pub fn fresh_scenario_soul(base: &Soul, scenario_world: Option<WorldLog>) -> Soul {
    let now = current_timestamp();
    let mut fresh = base.clone();
    fresh.character_id = Uuid::new_v4().to_string();
    fresh.last_updated = now as i64;
    fresh.turn_counter = 0;
    fresh.turns_since_consolidation = 0;
    fresh.world = scenario_world.unwrap_or_default();
    fresh.memory.recent.clear();
    fresh.memory.schemas.clear();
    fresh.relationships.insert("user".into(), neutral_user_relationship());
    fresh.arousal = ArousalState::default();
    fresh
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
    fn world_log_json_roundtrip_is_independent() {
        let mut world = WorldLog::default();
        world.location = "Carver City service tunnel".into();
        world.active_plots.push("Find the sealed stairwell".into());
        let json = serde_json::to_string_pretty(&world).expect("serialize");
        let decoded: WorldLog = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, world);
    }

    #[test]
    fn fresh_scenario_resets_volatile_state() {
        let mut soul = new_default_soul("Aurora Schwarz");
        soul.world.location = "Aurora's kitchen counter.".into();
        soul.world.recent_events = vec!["Romantic kitchen scene resolved.".into()];
        soul.memory.recent.push(MemoryEntry {
            id: "recent".into(),
            timestamp: 1,
            content: "Aurora warmed to the user.".into(),
            salience: 90.0,
            tag: "bonding".into(),
            retrieval_strength: 90.0,
            perceived_by_entity_id: None,
            target_entity_ids: Vec::new(),
            interpretation: None,
            confidence: None,
            objective_event_id: None,
        });
        soul.relationships.get_mut("user").unwrap().trust = 130.0;
        soul.relationships.get_mut("user").unwrap().affection = 126.0;
        let scenario_world = WorldLog {
            location: "Debt collector office threshold.".into(),
            active_plots: vec!["Collect the overdue payment".into()],
            recent_events: Vec::new(),
            key_objects: Vec::new(),
            time_elapsed: "Session start".into(),
        };

        let fresh = fresh_scenario_soul(&soul, Some(scenario_world.clone()));

        assert_ne!(fresh.character_id, soul.character_id);
        assert_eq!(fresh.world, scenario_world);
        assert!(fresh.memory.recent.is_empty());
        assert!(fresh.memory.schemas.is_empty());
        assert_eq!(fresh.relationships["user"], neutral_user_relationship());
        assert_eq!(soul.world.location, "Aurora's kitchen counter.");
        assert_eq!(soul.relationships["user"].trust, 130.0);
    }
}
