use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    schema::CURRENT_SCHEMA_VERSION,
    soul::{current_timestamp, WorldLog},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingSoul {
    pub schema_version: u32,
    pub setting_id: String,
    pub setting_name: String,
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
            last_updated: current_timestamp() as i64,
            turn_counter: 0,
            world: WorldLog::default(),
        }
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
