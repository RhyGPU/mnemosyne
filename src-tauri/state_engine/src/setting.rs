use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    schema::CURRENT_SCHEMA_VERSION,
    soul::{current_timestamp, PlotEntry, WorldLog},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingSoul {
    pub schema_version: u32,
    pub setting_id: String,
    pub setting_name: String,
    #[serde(default)]
    pub scenario: String,
    pub last_updated: i64,
    pub turn_counter: u64,
    pub world: WorldLog,
}

impl SettingSoul {
    pub fn default_for_setting(setting_name: &str) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            setting_id: Uuid::new_v4().to_string(),
            setting_name: setting_name.trim().to_string(),
            scenario: String::new(),
            last_updated: current_timestamp() as i64,
            turn_counter: 0,
            world: WorldLog::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionWorld {
    pub schema_version: u32,
    pub world_id: String,
    #[serde(default)]
    pub source_setting_id: Option<String>,
    #[serde(default)]
    pub source_savepoint_id: Option<String>,
    #[serde(default = "default_session_world_kind")]
    pub world_kind: String,
    pub setting_name: String,
    pub location: String,
    pub active_plots: Vec<String>,
    pub recent_events: Vec<String>,
    pub key_objects: Vec<String>,
    pub time_elapsed: String,
    #[serde(default)]
    pub dominant_current_plot: Option<PlotEntry>,
    #[serde(default)]
    pub background_plots: Vec<PlotEntry>,
    #[serde(default)]
    pub resolved_plots: Vec<PlotEntry>,
    #[serde(default)]
    pub stale_plot_decay: f32,
    #[serde(default)]
    pub scenario: String,
    pub created_at: i64,
    pub last_updated: i64,
}

fn default_session_world_kind() -> String {
    "session_world".into()
}

impl SessionWorld {
    pub fn world_log(&self) -> WorldLog {
        WorldLog {
            location: self.location.clone(),
            active_plots: self.active_plots.clone(),
            recent_events: self.recent_events.clone(),
            key_objects: self.key_objects.clone(),
            time_elapsed: self.time_elapsed.clone(),
            dominant_current_plot: self.dominant_current_plot.clone(),
            background_plots: self.background_plots.clone(),
            resolved_plots: self.resolved_plots.clone(),
            stale_plot_decay: self.stale_plot_decay,
        }
    }

    pub fn set_world_log(&mut self, world: &WorldLog) {
        self.location = world.location.clone();
        self.active_plots = world.active_plots.clone();
        self.recent_events = world.recent_events.clone();
        self.key_objects = world.key_objects.clone();
        self.time_elapsed = world.time_elapsed.clone();
        self.dominant_current_plot = world.dominant_current_plot.clone();
        self.background_plots = world.background_plots.clone();
        self.resolved_plots = world.resolved_plots.clone();
        self.stale_plot_decay = world.stale_plot_decay;
    }
}

pub fn session_world_from_setting(setting: &SettingSoul) -> SessionWorld {
    let now = current_timestamp() as i64;
    SessionWorld {
        schema_version: CURRENT_SCHEMA_VERSION,
        world_id: Uuid::new_v4().to_string(),
        source_setting_id: Some(setting.setting_id.clone()),
        source_savepoint_id: Some(setting.setting_id.clone()),
        world_kind: "session_world".into(),
        setting_name: setting.setting_name.clone(),
        location: setting.world.location.clone(),
        active_plots: setting.world.active_plots.clone(),
        recent_events: setting.world.recent_events.clone(),
        key_objects: setting.world.key_objects.clone(),
        time_elapsed: setting.world.time_elapsed.clone(),
        dominant_current_plot: setting.world.dominant_current_plot.clone(),
        background_plots: setting.world.background_plots.clone(),
        resolved_plots: setting.world.resolved_plots.clone(),
        stale_plot_decay: setting.world.stale_plot_decay,
        scenario: setting.scenario.clone(),
        created_at: now,
        last_updated: now,
    }
}

pub fn session_world_from_legacy_world(
    setting_name: &str,
    source_savepoint_id: Option<String>,
    world: &WorldLog,
) -> SessionWorld {
    let now = current_timestamp() as i64;
    SessionWorld {
        schema_version: CURRENT_SCHEMA_VERSION,
        world_id: Uuid::new_v4().to_string(),
        source_setting_id: None,
        source_savepoint_id,
        world_kind: "session_world".into(),
        setting_name: setting_name.trim().to_string(),
        location: world.location.clone(),
        active_plots: world.active_plots.clone(),
        recent_events: world.recent_events.clone(),
        key_objects: world.key_objects.clone(),
        time_elapsed: world.time_elapsed.clone(),
        dominant_current_plot: world.dominant_current_plot.clone(),
        background_plots: world.background_plots.clone(),
        resolved_plots: world.resolved_plots.clone(),
        stale_plot_decay: world.stale_plot_decay,
        scenario: String::new(),
        created_at: now,
        last_updated: now,
    }
}

pub fn new_default_setting(setting_name: &str) -> SettingSoul {
    SettingSoul::default_for_setting(setting_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_soul_roundtrips_world_state() {
        let mut setting = new_default_setting("Carver City");
        setting.world.location = "Underground cell".into();
        setting.world.active_plots = vec!["Escape the facility".into()];

        let json = serde_json::to_string_pretty(&setting).expect("serialize");
        let decoded: SettingSoul = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.schema_version, 1);
        assert_eq!(decoded.setting_name, "Carver City");
        assert_eq!(decoded.world.location, "Underground cell");
    }
}
