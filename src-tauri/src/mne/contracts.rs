use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MneBundleManifest {
    pub mne_version: u32,
    pub bundle_id: String,
    pub bundle_type: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    pub created_at: i64,
    pub app: String,
    pub schema_version: u32,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub soul_id: Option<String>,
    #[serde(default)]
    pub world_id: Option<String>,
    #[serde(default)]
    pub source_savepoint_id: Option<String>,
    #[serde(default)]
    pub source_setting_id: Option<String>,
    pub contents: MneBundleContents,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MneBundleContents {
    #[serde(default)]
    pub souls: Vec<String>,
    #[serde(default)]
    pub worlds: Vec<String>,
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub conversation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MneExportResult {
    pub path: String,
    pub manifest: MneBundleManifest,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MneImportResult {
    pub bundle_id: String,
    pub bundle_type: String,
    pub imported_soul_ids: Vec<String>,
    pub imported_setting_ids: Vec<String>,
    pub remapped_ids: HashMap<String, String>,
    pub summary: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MneValidationSummary {
    #[serde(default)]
    pub soul_name: Option<String>,
    #[serde(default)]
    pub soul_id: Option<String>,
    #[serde(default)]
    pub world_name: Option<String>,
    #[serde(default)]
    pub world_id: Option<String>,
    #[serde(default)]
    pub conversation_title: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub memory_count: usize,
    #[serde(default)]
    pub recent_event_count: usize,
    #[serde(default)]
    pub object_state_count: usize,
    #[serde(default)]
    pub relationship_count: usize,
    #[serde(default)]
    pub payload_log_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MneValidationReport {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub summary: MneValidationSummary,
}
