use serde::{Deserialize, Serialize};
use state_engine::{setting::SessionWorld, soul::Soul};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulSummary {
    pub character_id: String,
    pub character_name: String,
    pub soul_kind: String,
    pub source_soul_id: Option<String>,
    pub source_savepoint_id: Option<String>,
    pub avatar_image_id: Option<String>,
    pub last_updated: i64,
    pub recent_count: usize,
    pub core_count: usize,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingSummary {
    pub setting_id: String,
    pub setting_name: String,
    pub last_updated: i64,
    pub turn_counter: u64,
    pub location: String,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    #[serde(default = "default_message_channel")]
    pub channel: String,
    #[serde(default = "default_message_status")]
    pub status: String,
    #[serde(default = "default_message_origin")]
    pub origin: String,
    #[serde(default)]
    pub attachments: Vec<MessageAttachment>,
    pub hidden_at: Option<i64>,
}

fn default_message_status() -> String {
    "active".into()
}

fn default_message_channel() -> String {
    MESSAGE_CHANNEL_RP_SCENE.into()
}

fn default_message_origin() -> String {
    "active".into()
}

pub const MESSAGE_CHANNEL_RP_SCENE: &str = "rp_scene";
pub const MESSAGE_CHANNEL_COMMAND_OOC: &str = "command_ooc";
pub const MESSAGE_CHANNEL_COMMAND_SETUP: &str = "command_setup";
pub const MESSAGE_CHANNEL_COMMAND_STATE: &str = "command_state";
pub const MESSAGE_CHANNEL_COMMAND_PERSONA: &str = "command_persona";
pub const MESSAGE_CHANNEL_COMMAND_ASK: &str = "command_ask";
pub const MESSAGE_CHANNEL_COMMAND_HELP: &str = "command_help";
pub const MESSAGE_CHANNEL_SYSTEM_DEBUG: &str = "system_debug";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerPersona {
    pub persona_id: String,
    pub display_name: String,
    pub description: String,
    pub gender_code: String,
    pub pronouns: String,
    pub is_builtin: bool,
    pub is_archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub appearance: Option<String>,
    pub voice_style: Option<String>,
    pub boundaries: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageAsset {
    pub id: String,
    pub file_path: String,
    pub thumbnail_path: Option<String>,
    pub source: String,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub prompt: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub linked_soul_id: Option<String>,
    pub linked_conversation_id: Option<String>,
    pub linked_message_id: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageAttachment {
    pub id: i64,
    pub message_id: i64,
    pub image_asset_id: String,
    pub created_at: i64,
    pub image: ImageAsset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub title: String,
    pub soul_id: String,
    pub source_savepoint_id: Option<String>,
    pub world_id: Option<String>,
    pub source_setting_id: Option<String>,
    pub active_player_persona_id: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_message_preview: Option<String>,
    pub message_count: i64,
    pub archived_at: Option<i64>,
    pub active_evaluator_profile_id: Option<String>,
    pub is_benchmark: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageVariant {
    pub id: Option<i64>,
    pub message_id: i64,
    pub conversation_id: String,
    pub content: String,
    pub created_at: i64,
    pub label: Option<String>,
    pub source: Option<String>,
    pub is_selected: bool,
    pub soul_snapshot_json: Option<String>,
    pub debug_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPayloadLog {
    pub id: i64,
    pub conversation_id: String,
    pub message_id: Option<i64>,
    pub provider: String,
    pub mode: String,
    pub context_mode: String,
    pub model: String,
    pub base_url: String,
    pub system_message: String,
    pub user_message: String,
    pub context_text: String,
    pub estimated_system_tokens: usize,
    pub estimated_user_tokens: usize,
    pub estimated_total_tokens: usize,
    pub truncated: bool,
    pub created_at: i64,
    #[serde(default)]
    pub branch_id: Option<String>,
    #[serde(default)]
    pub active_turn_id: Option<String>,
    #[serde(default)]
    pub parent_turn_id: Option<String>,
    #[serde(default)]
    pub state_patch_ids_applied: Vec<String>,
    #[serde(default)]
    pub discarded_patch_ids_skipped: Vec<String>,
    #[serde(default)]
    pub state_rebuild_generation: Option<i64>,
    #[serde(default)]
    pub latest_assistant_variant_id: Option<i64>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub raw_provider_response: Option<String>,
    #[serde(default)]
    pub normalized_response: Option<String>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub provider_error: Option<String>,
    #[serde(default)]
    pub fallback_used: bool,
    #[serde(default)]
    pub fallback_reason: Option<String>,
    #[serde(default)]
    pub provider_request_id: Option<String>,
    #[serde(default)]
    pub provider_response_id: Option<String>,
    #[serde(default)]
    pub pipeline_trace_json: Option<String>,
}

impl Default for LlmPayloadLog {
    fn default() -> Self {
        Self {
            id: 0,
            conversation_id: String::new(),
            message_id: None,
            provider: String::new(),
            mode: String::new(),
            context_mode: String::new(),
            model: String::new(),
            base_url: String::new(),
            system_message: String::new(),
            user_message: String::new(),
            context_text: String::new(),
            estimated_system_tokens: 0,
            estimated_user_tokens: 0,
            estimated_total_tokens: 0,
            truncated: false,
            created_at: 0,
            branch_id: None,
            active_turn_id: None,
            parent_turn_id: None,
            state_patch_ids_applied: Vec::new(),
            discarded_patch_ids_skipped: Vec::new(),
            state_rebuild_generation: None,
            latest_assistant_variant_id: None,
            request_id: None,
            turn_id: None,
            raw_provider_response: None,
            normalized_response: None,
            finish_reason: None,
            provider_error: None,
            fallback_used: false,
            fallback_reason: None,
            provider_request_id: None,
            provider_response_id: None,
            pipeline_trace_json: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LlmPayloadResponseUpdate {
    pub request_id: Option<String>,
    pub turn_id: Option<String>,
    pub raw_provider_response: Option<String>,
    pub normalized_response: Option<String>,
    pub finish_reason: Option<String>,
    pub provider_error: Option<String>,
    pub fallback_used: Option<bool>,
    pub fallback_reason: Option<String>,
    pub provider_request_id: Option<String>,
    pub provider_response_id: Option<String>,
    pub pipeline_trace_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityRecord {
    pub entity_id: String,
    pub conversation_id: String,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub kind: String,
    pub controlled_by: String,
    pub linked_soul_id: Option<String>,
    pub active_in_scene: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub narrator_timeout_ms: Option<u64>,
    pub evaluator_timeout_ms: Option<u64>,
    pub evaluator_timeout_mode: Option<String>,
    pub evaluator_mode: Option<String>,
    #[serde(default)]
    pub structured_evaluator_policy: Option<String>,
    pub wait_for_evaluator_before_next_turn: Option<bool>,
    pub allow_send_with_stale_state: Option<bool>,
    pub evaluator_background_enabled: Option<bool>,
    pub anti_replay_forced_retry_enabled: Option<bool>,
    pub archived_at: Option<i64>,
    pub narrator_compatibility_status: i32,
    pub evaluator_compatibility_status: i32,
    pub command_compatibility_status: i32,
    pub evaluator_contract_version: i32,
    pub evaluator_prompt_version: i32,
    pub evaluator_last_tested_at: Option<i64>,
    pub evaluator_last_failure_reason: Option<String>,
    /// Structured-output level the provider achieved during the last contract
    /// test probe: 0 untested/failed, 1 prompt-only, 2 json_object, 3 json_schema.
    #[serde(default)]
    pub structured_output_support: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorJob {
    pub evaluator_job_id: String,
    pub conversation_id: String,
    pub turn_id: String,
    pub assistant_message_id: i64,
    pub status: String, // pending, running, completed, failed, canceled, timed_out
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub elapsed_ms: Option<i64>,
    pub timeout_ms: Option<u64>,
    pub timeout_mode: String, // finite, no_app_timeout
    pub model: Option<String>,
    pub provider: Option<String>,
    pub error_message: Option<String>,
    pub patch_applied: bool,
}

#[derive(Debug, Clone)]
pub struct TurnSnapshot {
    pub conversation_id: String,
    pub assistant_message_id: i64,
    pub user_text: String,
    pub soul_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBranch {
    pub branch_id: String,
    pub conversation_id: String,
    pub base_soul_json: String,
    pub base_session_world_json: String,
    pub active_turn_id: Option<String>,
    pub rebuild_generation: i64,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnCommit {
    pub turn_id: String,
    pub conversation_id: String,
    pub branch_id: String,
    pub parent_turn_id: Option<String>,
    pub user_message_id: Option<i64>,
    pub assistant_message_id: Option<i64>,
    pub state_patch_id: Option<String>,
    pub selected_variant_id: Option<i64>,
    pub created_at: i64,
    pub active_variant: bool,
    pub is_active: bool,
    pub is_discarded: bool,
    pub is_regenerated_variant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePatchRecord {
    pub patch_id: String,
    pub turn_id: String,
    pub parent_state_hash: Option<String>,
    pub patch_json: String,
    pub inverse_patch_json: Option<String>,
    pub applied_at: i64,
    pub applies_to: String,
    pub is_active: bool,
    pub invalidated_by_patch_id: Option<String>,
    pub supersedes_patch_id: Option<String>,
    pub patch_kind: String,
    pub parent_baseline_patch_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub source_assistant_message_id: Option<i64>,
    pub source_assistant_variant_id: Option<i64>,
    pub created_by_job_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompilerRunRecord {
    pub run_id: String,
    pub conversation_id: String,
    pub branch_id: String,
    pub turn_id: String,
    pub source_hash: String,
    pub mode: String,
    pub schema_version: u32,
    pub compiler_version: u32,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub status: String,
    pub enforcement_level: String,
    pub raw_response_json: Option<String>,
    pub artifact_json: Option<String>,
    pub error_message: Option<String>,
    pub commit_allowed: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompilerCandidateRecord {
    pub run_id: String,
    pub candidate_id: String,
    pub candidate_index: usize,
    pub kind: String,
    pub disposition: String,
    pub candidate_json: String,
    pub diagnostics_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryV2ProjectionRecord {
    pub conversation_id: String,
    pub branch_id: String,
    pub memory_id: String,
    pub layer: String,
    pub memory_kind: String,
    pub owner_entity_id: Option<String>,
    pub content: String,
    pub source_patch_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub source_message_id: Option<i64>,
    pub source_entity_id: Option<String>,
    pub source_quote: Option<String>,
    pub source_memory_ids_json: String,
    pub supporting_evidence_json: String,
    pub contradicting_evidence_json: String,
    pub confidence: f32,
    pub truth_status: String,
    pub validity: String,
    pub schema_version: u32,
    pub compiler_version: u32,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryV2ProjectionGeneration {
    pub conversation_id: String,
    pub branch_id: String,
    pub generation: i64,
    pub entry_count: usize,
    pub rebuilt_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryV2ConsolidationRun {
    pub proposed: usize,
    pub stored: usize,
    pub rejected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryV2RecallHit {
    pub memory: MemoryV2ProjectionRecord,
    pub lexical_score: f32,
    pub semantic_score: f32,
    pub temporal_score: f32,
    pub graph_score: f32,
    pub final_score: f32,
    pub selection_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MemoryV2RecallFilter {
    pub truth_statuses: Vec<String>,
    pub memory_kinds: Vec<String>,
    pub owner_entity_id: Option<String>,
    pub created_after_ms: Option<i64>,
    pub created_before_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StartupRecoveryReport {
    pub branches_rebuilt: usize,
    pub materialized_conversation_ids: Vec<String>,
    pub running_jobs_marked_retryable: usize,
    pub pending_job_ids: Vec<String>,
    pub failed_job_ids: Vec<String>,
    pub canceled_or_timed_out_job_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BranchPatchDebug {
    pub branch_id: String,
    pub active_turn_id: Option<String>,
    pub rebuild_generation: i64,
    pub applied_patches: Vec<String>,
    pub skipped_discarded_patches: Vec<String>,
    pub invalidated_patches: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LedgerRebuild {
    pub soul: Soul,
    pub session_world: SessionWorld,
    pub debug: BranchPatchDebug,
}
