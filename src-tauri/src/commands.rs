use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use base64::{engine::general_purpose, Engine as _};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, Window};

use state_engine::{
    compiler::{
        compile_perception_pipeline, lower_state_effects_to_engine_patch,
        perception_ir_json_schema, seal_perception_batch, CompilerPipelineReport, EntityCatalog,
        EntityDescriptor, EntityRole, ModelProvenance, PerceptionBatch, PerceptionBatchDraft,
        SceneSlot, SimulationDecision, SimulationSnapshot, SourceEnvelope, SourceIdentity,
        MEMORY_COMPILER_CONTRACT_VERSION, PERCEPTION_IR_SCHEMA_NAME, PERCEPTION_IR_SCHEMA_VERSION,
    },
    consolidation::consolidate_soul,
    context_compiler::{
        compile_context_for_session,
        compile_context_for_session_separate_user_message_with_player_persona_pending,
        compile_context_for_session_with_player_persona, estimate_tokens, ContextMessage,
        ContextPreview, MemorySlotTrace, PlayerPersonaContext,
    },
    evaluator::{
        active_souls_for_v1, evaluator_output_to_engine_patch, EvaluatorCandidateRejection,
        EvaluatorConversionContext, EvaluatorConversionReport, EvaluatorOutputV1,
        GlobalSceneEvaluation, TurnClassification, WorldChangeEvaluation, EVALUATOR_SCHEMA_VERSION,
    },
    evaluator_form::{
        build_eval_form_spec_with_player_persona, compile_eval_form_response,
        parse_eval_form_response_with_trace, EvalFormRowRejection, EvalFormSpec, EvalFormTrace,
    },
    evaluator_ingest::{parse_evaluator_output_with_context, EvaluatorDraftContext},
    evaluator_structured::{
        compile_evaluator_ops_to_engine_patch, evaluator_ops_json_schema,
        evaluator_ops_repair_json_schema, EvaluatorStructuredOutputV1,
        EVALUATOR_OPS_REPAIR_SCHEMA_NAME, EVALUATOR_OPS_SCHEMA_NAME,
    },
    hidden_state::{parse_hidden_state, HiddenState},
    memory::{restore_archived_memory, set_memory_pinned},
    patch::{
        is_premature_user_turn_event, is_retcon_or_correction_text,
        purge_premature_recent_events_from_world, EnginePatch, KnowledgeOperationPatch,
        MemoryApplyAction, MemoryPatch, SceneStatePatch, SoulPatch, WorldPatch,
        PATCH_PROTOCOL_VERSION,
    },
    setting::{new_default_setting, SessionWorld, SettingSoul},
    soul::{
        new_default_soul, session_soul_from_savepoint, soul_savepoint_from_session,
        MemoryLayerReply, MemorySourceType, Soul, TruthStatus,
    },
};

#[cfg(test)]
use crate::mne::archive::{read_stored_zip, validate_mne_manifest, write_stored_zip};
pub use crate::mne::contracts::{
    MneBundleContents, MneBundleManifest, MneExportResult, MneImportResult, MneValidationReport,
    MneValidationSummary,
};

pub use crate::benchmark::contracts::*;
#[cfg(test)]
pub(crate) use crate::benchmark::*;
#[cfg(test)]
pub(crate) use crate::mne::service::*;
pub mod evaluator;
pub mod session;
use crate::{
    chat_commands::{
        parse_chat_command, AskMode, ChatCommandKind, ParsedChatCommand, PersonaSubcommandKind,
        StateSubcommandKind,
    },
    db::{
        self, AssistantMessageVariant, ChatMessage, ConversationSummary, EntityRecord, ImageAsset,
        LlmPayloadLog, PlayerPersona, ProviderProfile, RestoreInactiveMessagesResult,
        SettingSummary, SoulSummary,
    },
    job_progress::{
        emit_background_job_progress, BackgroundJobHistoryEntry, BackgroundJobProgress,
    },
    pipeline_trace::{PipelineErrorCode, TurnPipelineTrace, TurnTokenUsage},
    providers::{
        api::{
            build_command_help_prompt, build_command_ooc_prompt, build_command_setup_prompt,
            build_command_soul_edit_agent_prompt, build_command_state_edit_prompt,
            build_command_state_summary_prompt,
            build_evaluator_form_prompt_compact_with_player_persona,
            build_evaluator_form_prompt_with_player_persona, build_evaluator_prompt,
            build_narrator_system_prompt, build_perception_v2_prompt_with_player_persona,
            build_structured_evaluator_prompt,
            build_structured_evaluator_prompt_with_player_persona,
            structured_evaluator_max_retries, structured_tool_retry_user_message, ApiMessage,
            ApiProvider, ApiProviderSettings, PreparedApiPayload, StructuredCompletionTrace,
            StructuredEnforcement, TokenUsage, CURRENT_EVALUATOR_CONTRACT_VERSION,
            CURRENT_EVALUATOR_PROMPT_VERSION, PERCEPTION_V2_PROMPT_VERSION,
        },
        mock::MockProvider,
    },
    AppState,
};
#[cfg(test)]
pub(crate) use evaluator::*;
use evaluator::{
    emit_evaluator_repair_signal, form_rejected_ops_for_repair, rejected_ops_for_repair,
};
#[cfg(test)]
pub(crate) use session::*;
use session::{player_persona_context, PhoneContradictionGuard};

#[cfg(test)]
use crate::providers::api::build_evaluator_form_prompt;
#[cfg(test)]
use state_engine::evaluator_form::build_eval_form_spec;

const CONSOLIDATION_INTERVAL_TURNS: u64 = 10;
const NO_LLM_PAYLOAD_LOGS_MESSAGE: &str = "No LLM payload logs found for this conversation.";
const FULL_CHAT_TOKEN_BUDGET: usize = 6_000;
const NARRATOR_BRIEF_TARGET_TOKENS: usize = 2_500;
const STATE_UPDATER_TARGET_TOKENS: usize = 1_600;
const STATE_MAP_RECENT_SESSION_LIMIT: usize = 5;
const DEFAULT_EVALUATOR_TIMEOUT_MS: u64 = 25_000;
const DEFAULT_STRUCTURED_EVALUATOR_TIMEOUT_MS: u64 = 90_000;

/// How many times the narrator call may be made for one turn.
///
/// Bounded at 2 because the only retryable case is a stream that delivered
/// nothing — see the gate at the call site, which also checks that no chunk
/// reached the reader before asking again.
const NARRATOR_MAX_ATTEMPTS: usize = 2;
const DEFAULT_DIAGNOSTIC_EVALUATOR_TIMEOUT_MS: u64 = 60_000;
/// Generous ceiling for repair against a LOCAL (loopback) model. CPU inference of
/// the ~2k-token repair prompt measured ~150s just for prompt eval on an i5; the
/// normal timeouts (25-90s) always fire first. Repair is background, so this only
/// matters as an upper bound for a genuinely stuck call.
const LOCAL_REPAIR_TIMEOUT_MS: u64 = 600_000;
pub(crate) const NEXT_TURN_GATE_POLL_MS: u64 = 250;
const NEXT_TURN_GATE_FALLBACK_MAX_MS: u64 = 120_000;
const ANTI_REPLAY_FORCED_RETRY_ENABLED_DEFAULT: bool = false;
const EVALUATOR_MODE_V1: &str = "evaluator_v1";
pub(crate) const EVALUATOR_MODE_FORM_V1: &str = "evaluator_form_v1";
pub(crate) const EVALUATOR_MODE_STRUCTURED_V1: &str = "evaluator_structured_v1";
pub(crate) const EVALUATOR_MODE_PERCEPTION_V2: &str = "evaluator_perception_v2";
const EVALUATOR_MODE_DUAL_COMPARE: &str = "dual_compare";
// provider_profiles.structured_output_support levels (contract-test probe).
const STRUCTURED_SUPPORT_UNTESTED: i32 = 0;
const STRUCTURED_SUPPORT_PROMPT_ONLY: i32 = 1;
const STRUCTURED_SUPPORT_JSON_OBJECT: i32 = 2;
const STRUCTURED_SUPPORT_JSON_SCHEMA: i32 = 3;
const EVALUATOR_COMPAT_UNTESTED: i32 = 0;
const EVALUATOR_COMPAT_PASSED_SCHEMA_ENFORCED: i32 = 1;
const EVALUATOR_COMPAT_FAILED: i32 = 2;
const EVALUATOR_COMPAT_PASSED_JSON_OBJECT_ONLY: i32 = 3;
const EVALUATOR_COMPAT_STALE_PROMPT_VERSION: i32 = 4;
const EVALUATOR_COMPAT_FAILED_SCHEMA_ENFORCED: i32 = 5;
const OP_NORMAL_SEND: &str = "normal_send";
const OP_REGENERATE: &str = "regenerate";
const OP_FIX_RESPONSE: &str = "fix_response";
const OP_BASELINE_PATCH: &str = "baseline_patch";
const OP_ENRICHMENT_PATCH: &str = "enrichment_patch";
const MANUAL_USER_STATE_COMMAND_SOURCE: &str = "manual_user_state_command";
const AI_AGENT_SOUL_EDIT_COMMAND_SOURCE: &str = "ai_agent_soul_edit_command";
const NARRATOR_PROVIDER_ERROR_VISIBLE: &str =
    "[Provider error: narrator response could not be generated.]";
const MOCK_OBSERVATION_READER_LINE: &str =
    "She listens, not fully relaxed, but present enough to stay in the exchange. \"Keep going.\"";
static DEV_LOG_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NarratorMessageOrigin {
    Api,
    Mock,
}

#[derive(Debug, Clone, Default, Serialize)]
struct NarratorTurnTrace {
    request_id: String,
    conversation_id: String,
    branch_id: Option<String>,
    turn_id: Option<String>,
    user_message_id: Option<i64>,
    assistant_message_id: Option<i64>,
    state_patch_id: Option<String>,
    provider_request_id: Option<String>,
    provider_response_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ImageFileInfo {
    extension: &'static str,
    mime_type: &'static str,
    width: Option<i64>,
    height: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct TurnResult {
    pub conversation_id: String,
    pub soul: Soul,
    pub visible_response: String,
    pub context_preview: ContextPreview,
    pub messages: Vec<ChatMessage>,
    pub consolidation_ran: bool,
    pub debug: TurnDebug,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct TurnDebug {
    pub provider: String,
    pub hidden_state_found: bool,
    pub fallback_hidden_state_generated: bool,
    pub narrator_response_saved: bool,
    pub assistant_message_id: Option<i64>,
    pub selected_variant_id: Option<i64>,
    pub state_updater_status: String,
    pub replay_detected: bool,
    pub replay_score: f32,
    pub replay_reason: Option<String>,
    pub replay_compared_against_message_id: Option<i64>,
    pub output_contract_warning: Option<String>,
    pub tag: Option<String>,
    pub trust_delta: Option<f32>,
    pub affection_delta: Option<f32>,
    pub new_location: Option<String>,
    pub present_characters: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_patch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_patch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrichment_patch_id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub simulated_response: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fallback_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct StreamChunk {
    pub conversation_id: String,
    pub chunk: String,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct SavedChatMessageEvent {
    pub conversation_id: String,
    pub message: ChatMessage,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct DevLogEvent {
    pub id: String,
    pub timestamp: i64,
    pub level: String,
    pub category: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, serde::Serialize)]
pub struct LlmPayloadTokenEstimate {
    pub system: usize,
    pub context: usize,
    pub user: usize,
    pub total: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct LlmPayloadPreview {
    pub provider: String,
    pub mode: String,
    pub context_mode: String,
    pub custom_prompt_status: String,
    pub model: String,
    pub base_url: String,
    pub system_message: String,
    pub user_message: String,
    pub context: String,
    pub messages: Vec<ApiMessage>,
    pub truncated: bool,
    pub estimated_tokens: LlmPayloadTokenEstimate,
    pub memory_slot_debug: Vec<MemorySlotTrace>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextMode {
    Brief,
    FullChat,
}

impl ContextMode {
    fn from_label(value: Option<&str>) -> Self {
        match value
            .map(str::trim)
            .unwrap_or("brief")
            .to_ascii_lowercase()
            .as_str()
        {
            "full_chat" | "full chat" | "full-chat" => Self::FullChat,
            _ => Self::Brief,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::FullChat => "full_chat",
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct VariantSelectionResult {
    pub variants: Vec<AssistantMessageVariant>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, serde::Serialize)]
pub struct ExportResult {
    pub path: String,
    pub message: String,
}

#[derive(Debug, serde::Serialize)]
pub struct SessionStartResult {
    pub soul: Soul,
    pub conversation: ConversationSummary,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionStateHubItem {
    pub conversation: ConversationSummary,
    pub soul_name: String,
    pub setting_name: String,
    pub location: String,
    pub time_elapsed: String,
    pub current_scene: String,
    pub focus: String,
    pub turn_counter: u64,
    pub memory_count: usize,
    pub core_memory_count: usize,
    pub recent_memory_count: usize,
    pub schema_count: usize,
    pub relationship_count: usize,
    pub positive_relationship_count: usize,
    pub object_count: usize,
    pub event_count: usize,
    pub active_plot_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateMapSceneItem {
    pub session_id: String,
    pub session_title: String,
    pub soul_name: String,
    pub setting_name: String,
    pub turn_counter: u64,
    pub location: String,
    pub time_elapsed: String,
    pub current_scene: String,
    pub focus: String,
    pub last_user_action: String,
    pub pressure_point: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateMapCharacterItem {
    pub session_id: String,
    pub session_title: String,
    pub name: String,
    pub role: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateMapRelationshipItem {
    pub session_id: String,
    pub session_title: String,
    pub soul_name: String,
    pub target: String,
    pub love_type: String,
    pub trust: f32,
    pub affection: f32,
    pub intimacy: f32,
    pub fear: f32,
    pub desire: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateMapObjectItem {
    pub session_id: String,
    pub session_title: String,
    pub name: String,
    pub kind: String,
    pub owner: String,
    pub location: String,
    pub status: String,
    pub summary: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateMapTimelineItem {
    pub session_id: String,
    pub session_title: String,
    pub turn_counter: u64,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateMapMemoryItem {
    pub session_id: String,
    pub session_title: String,
    pub soul_name: String,
    pub content: String,
    pub tag: String,
    pub source_turn: Option<i64>,
    pub confidence: Option<f32>,
    pub truth_status: String,
    pub source_type: String,
    pub is_pinned: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateMapMemoryV2Item {
    pub session_id: String,
    pub session_title: String,
    pub memory_id: String,
    pub layer: String,
    pub memory_kind: String,
    pub validity: String,
    pub content: String,
    pub confidence: f32,
    pub truth_status: String,
    pub source_patch_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub source_quote: Option<String>,
    pub source_memory_ids: Vec<String>,
    pub supporting_evidence_count: usize,
    pub contradicting_evidence_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionStateMap {
    pub sessions: Vec<SessionStateHubItem>,
    pub scenes: Vec<StateMapSceneItem>,
    pub characters: Vec<StateMapCharacterItem>,
    pub relationships: Vec<StateMapRelationshipItem>,
    pub objects: Vec<StateMapObjectItem>,
    pub timeline: Vec<StateMapTimelineItem>,
    pub memories: Vec<StateMapMemoryItem>,
    pub memory_v2: Vec<StateMapMemoryV2Item>,
}

#[derive(Debug, Clone)]
struct EntityTurnContext {
    entities: Vec<EntityRecord>,
    speaker: SpeakerResolution,
}

#[derive(Debug, Clone)]
struct SpeakerResolution {
    label: Option<String>,
    entity_id: String,
    display_name: String,
    status: SpeakerResolutionStatus,
    candidates: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeakerResolutionStatus {
    NoLabel,
    Exact,
    FuzzyTypo,
    Ambiguous,
    Created,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReplaySeverity {
    #[default]
    None,
    MildOverlap,
    StrongReplay,
    Contradiction,
    ObjectStateViolation,
}

#[derive(Debug, Clone)]
struct ReplaySource {
    message_id: i64,
    content: String,
}

#[derive(Debug, Clone, Default)]
struct ReplayGuardResult {
    replay_detected: bool,
    replay_score: f32,
    replay_reason: Option<String>,
    compared_against_message_id: Option<i64>,
    severity: ReplaySeverity,
}

#[derive(Debug, Clone)]
struct OutputContractResult {
    text: String,
    warning: Option<String>,
    status_repair_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredEvaluatorDiagnosticRun {
    pub turn_index: usize,
    pub user_message: String,
    pub narrator_response: String,
    pub evaluator_mode: String,
    pub enforcement_level: String,
    pub structured_enforcement_requested: String,
    pub structured_enforcement_validated: bool,
    pub structured_schema_validation_status: String,
    pub structured_schema_validation_error: Option<String>,
    pub fallback_path: Vec<String>,
    pub failure_reasons: Vec<String>,
    pub ops_count: usize,
    pub compiled_patch_summary: serde_json::Value,
    pub syntactic_repair_used: bool,
    pub memory_ops_count: usize,
    pub relationship_event_ops_count: usize,
    pub object_update_ops_count: usize,
    pub scene_update_ops_count: usize,
    pub state_patch_id: Option<String>,
    pub error: Option<String>,
    pub tool_calls_present: bool,
    pub tool_call_count: usize,
    pub tool_call_names: Vec<String>,
    pub raw_content_present: bool,
    pub raw_tool_calls_present: bool,
    pub structured_retry_count: usize,
    pub structured_retry_reasons: Vec<String>,
    pub structured_retry_succeeded: Option<bool>,
    pub structured_retry_final_error: Option<String>,
    pub perception_v2_shadow: PerceptionV2ShadowTrace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerceptionV2ShadowTrace {
    pub attempted: bool,
    pub commit_allowed: bool,
    pub commit_count: usize,
    pub schema_version: u32,
    pub compiler_version: u32,
    pub prompt_version: String,
    pub enforcement_level: String,
    pub schema_validated: bool,
    pub status: String,
    pub error: Option<String>,
    pub source_hash: Option<String>,
    pub candidate_count: usize,
    pub candidate_ids: Vec<String>,
    pub kind_counts: BTreeMap<String, usize>,
    pub semantic_accepted: usize,
    pub semantic_rejected: usize,
    pub effect_count: usize,
    pub engine_patch_summary: serde_json::Value,
    pub unsupported_effect_count: usize,
    pub simulation_decision: String,
    pub diagnostic_codes: Vec<String>,
    pub v1_ops_count: Option<usize>,
    pub elapsed_ms: u64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredEvaluatorDiagnosticSummary {
    pub conversation_id: String,
    pub provider_profile_id: String,
    pub provider_model: String,
    pub base_url_redacted: String,
    pub structured_mode_requested: String,
    pub structured_mode_resolved: String,
    pub resolved_evaluator_source: String,
    pub structured_policy: String,
    pub structured_evaluator_policy: String,
    pub evaluator_mode: String,
    pub strict_tool_diagnostic: bool,
    pub strict_tool_passed: bool,
    pub fallback_used: bool,
    pub failure_turns: Vec<usize>,
    pub structured_schema_version: u32,
    pub perception_v2_schema_version: u32,
    pub perception_v2_compiler_version: u32,
    pub perception_v2_shadow_attempted: usize,
    pub perception_v2_shadow_validated: usize,
    pub perception_v2_shadow_candidates: usize,
    pub perception_v2_shadow_commit_count: usize,
    pub runs: Vec<StructuredEvaluatorDiagnosticRun>,
    pub enforcement_levels: Vec<String>,
    pub evaluator_mode_per_run: Vec<String>,
    pub structured_enforcement_per_run: Vec<String>,
    pub structured_enforcement_requested_per_run: Vec<String>,
    pub structured_enforcement_validated_per_run: Vec<bool>,
    pub structured_schema_validation_status_per_run: Vec<String>,
    pub failure_reasons: Vec<String>,
    pub fallback_paths: Vec<Vec<String>>,
    pub ops_counts: Vec<usize>,
    pub memory_ops_count: usize,
    pub relationship_event_ops_count: usize,
    pub object_update_ops_count: usize,
    pub scene_update_ops_count: usize,
    pub syntactic_repair_used: bool,
    pub final_memory_count: usize,
    pub final_relationship_target_ids: Vec<String>,
    pub final_object_states: Vec<serde_json::Value>,
    pub final_scene_participants: Vec<String>,
    pub default_player_leaked_into_normal_rp_state: bool,
    pub default_player_in_relationship_context: bool,
    pub payload_history_path: String,
    pub mne_checkpoint_path: String,
    pub summary_json_path: String,
}

#[tauri::command]
pub fn compile_context(
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
    conversation_id: String,
) -> Result<ContextPreview, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let (soul, session_world) = if let Ok(branch) =
        db::get_active_session_branch(&conn, &conversation_id)
    {
        let rebuilt = db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
        emit_dev_log(
            &window,
            "debug",
            "ledger",
            "branch_state_rebuilt",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "branch_id": rebuilt.debug.branch_id,
                "applied_patch_count": rebuilt.debug.applied_patches.len(),
                "skipped_discarded_patch_count": rebuilt.debug.skipped_discarded_patches.len(),
                "rebuild_generation": rebuilt.debug.rebuild_generation
            })),
        );
        (rebuilt.soul, rebuilt.session_world)
    } else {
        let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
        let session_world = load_session_world_for_context(&window, &conn, &conversation_id, &soul)
            .map_err(|err| err.to_string())?;
        (soul, session_world)
    };
    emit_possible_world_character_mismatch(&window, &conversation_id, &soul, Some(&session_world));
    let messages = db::list_messages(&conn, &conversation_id, 5).map_err(|err| err.to_string())?;
    let persona =
        db::get_active_player_persona(&conn, &conversation_id).map_err(|err| err.to_string())?;
    let persona_context = player_persona_context(&persona);
    let preview = compile_context_for_session_with_player_persona(
        &soul,
        Some(&session_world),
        &messages_to_context(messages),
        &persona_context,
    );
    emit_memory_slot_debug_logs(&window, &conversation_id, &soul.character_id, &preview);
    Ok(preview)
}

fn emit_memory_slot_debug_logs(
    window: &Window,
    conversation_id: &str,
    active_soul_id: &str,
    preview: &ContextPreview,
) {
    for trace in preview
        .memory_slot_debug
        .iter()
        .filter(|trace| trace.action == "selected")
        .take(24)
    {
        emit_dev_log(
            window,
            "debug",
            "context",
            "memory_slot_selected",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "active_soul_id": active_soul_id,
                "slot": trace.slot,
                "memory_id": trace.memory_id,
                "reason": trace.reason,
                "source_type": trace.source_type,
                "truth_status": trace.truth_status,
                "entity_match": trace.entity_match,
                "plot_match": trace.plot_match,
                "salience": trace.salience,
                "final_score": trace.final_score
            })),
        );
    }
}

fn load_session_world_for_context(
    window: &Window,
    conn: &Connection,
    conversation_id: &str,
    soul: &Soul,
) -> rusqlite::Result<SessionWorld> {
    match db::get_conversation_session_world(conn, conversation_id)? {
        Some(session_world) => {
            emit_dev_log(
                window,
                "debug",
                "context",
                "session_world_loaded",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "session_world_id": session_world.world_id.as_str(),
                    "source_setting_id": session_world.source_setting_id.as_deref()
                })),
            );
            emit_dev_log(
                window,
                "debug",
                "context",
                "context_world_source_session_world",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "session_world_id": session_world.world_id.as_str()
                })),
            );
            Ok(session_world)
        }
        None => {
            emit_dev_log(
                window,
                "warn",
                "context",
                "session_world_missing",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "active_soul_id": soul.character_id.as_str()
                })),
            );
            let session_world =
                db::ensure_conversation_session_world(conn, conversation_id, soul, None)?;
            emit_dev_log(
                window,
                "warn",
                "context",
                "context_world_source_legacy_soul_world",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "session_world_id": session_world.world_id.as_str(),
                    "reason": "created from legacy soul.world fallback"
                })),
            );
            Ok(session_world)
        }
    }
}

#[tauri::command]
pub fn preview_api_payload(
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    settings: ApiProviderSettings,
    provider: String,
    context_mode: Option<String>,
) -> Result<LlmPayloadPreview, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    let session_world = db::ensure_conversation_session_world(&conn, &conversation_id, &soul, None)
        .map_err(|err| err.to_string())?;
    let persona =
        db::get_active_player_persona(&conn, &conversation_id).map_err(|err| err.to_string())?;
    let persona_context = player_persona_context(&persona);
    let messages = messages_to_context(
        db::list_messages(&conn, &conversation_id, 5).map_err(|err| err.to_string())?,
    );

    Ok(build_llm_payload_preview(
        &soul,
        Some(&session_world),
        &messages,
        &user_text,
        &mode,
        &settings,
        &provider,
        ContextMode::from_label(context_mode.as_deref()),
        Some(&persona_context),
    ))
}

#[tauri::command]
pub fn run_consolidation(state: State<'_, AppState>, soul_id: String) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let mut soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    consolidate_soul(&mut soul);
    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
    Ok(soul)
}

#[tauri::command]
pub fn send_mock_turn(
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    replacement_assistant_id: Option<i64>,
    correction_instruction: Option<String>,
) -> Result<TurnResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    send_mock_turn_with_conn(
        &conn,
        conversation_id,
        soul_id,
        user_text,
        mode,
        replacement_assistant_id,
        correction_instruction,
    )
}

pub(crate) fn send_mock_turn_with_conn(
    conn: &Connection,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    replacement_assistant_id: Option<i64>,
    correction_instruction: Option<String>,
) -> Result<TurnResult, String> {
    let request_id = uuid_like_id();
    let canonical_turn_id = format!("turn_{request_id}");
    if let Some(command_result) = maybe_handle_chat_command_with_conn(
        None,
        conn,
        conversation_id.clone(),
        soul_id.clone(),
        user_text.clone(),
        &request_id,
        &canonical_turn_id,
        ContextMode::Brief,
        None,
    )? {
        return Ok(command_result);
    }
    let (mut soul, snapshot_user_text, pre_turn_soul_json) =
        if let Some(message_id) = replacement_assistant_id {
            let snapshot = db::get_turn_snapshot(&conn, &conversation_id, message_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| "No turn snapshot found for assistant response".to_string())?;
            let soul: Soul =
                serde_json::from_str(&snapshot.soul_json).map_err(|err| err.to_string())?;
            (soul, snapshot.user_text, snapshot.soul_json)
        } else {
            let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
            let pre_turn_soul_json = serde_json::to_string(&soul).map_err(|err| err.to_string())?;
            (soul, user_text.clone(), pre_turn_soul_json)
        };

    db::ensure_conversation(&conn, &conversation_id, &soul.character_id)
        .map_err(|err| err.to_string())?;
    let mut session_world =
        db::ensure_conversation_session_world(&conn, &conversation_id, &soul, None)
            .map_err(|err| err.to_string())?;
    let ledger_branch = match db::get_active_session_branch(conn, &conversation_id) {
        Ok(branch) => Some(branch),
        Err(_)
            if correction_instruction
                .as_deref()
                .map(str::trim)
                .is_some_and(|instruction| !instruction.is_empty()) =>
        {
            Some(
                db::create_session_branch(conn, &conversation_id, &soul, &session_world)
                    .map_err(|err| err.to_string())?,
            )
        }
        Err(_) => None,
    };
    let old_commit = replacement_assistant_id.and_then(|message_id| {
        db::get_turn_commit_by_assistant(conn, &conversation_id, message_id)
            .ok()
            .flatten()
    });
    let ledger_parent_turn_id = if let Some(branch) = ledger_branch.as_ref() {
        let parent_turn_id = if replacement_assistant_id.is_some() {
            old_commit
                .as_ref()
                .and_then(|commit| commit.parent_turn_id.clone())
        } else {
            branch.active_turn_id.clone()
        };
        let rebuilt = db::rebuild_session_state_until(
            conn,
            &conversation_id,
            &branch.branch_id,
            parent_turn_id.as_deref(),
        )
        .map_err(|err| err.to_string())?;
        soul = rebuilt.soul;
        session_world = rebuilt.session_world;
        parent_turn_id
    } else {
        None
    };
    let ledger_user_message_id = if replacement_assistant_id.is_none() {
        Some(reuse_or_insert_user_message(
            conn,
            &conversation_id,
            &user_text,
        )?)
    } else {
        old_commit
            .as_ref()
            .and_then(|commit| commit.user_message_id)
    };

    let before_messages = match replacement_assistant_id {
        Some(message_id) => db::list_messages_before_id(&conn, &conversation_id, message_id, 5),
        None => db::list_messages(&conn, &conversation_id, 5),
    }
    .map_err(|err| err.to_string())?;
    let pending_setup_text = take_pending_setup_for_normal_turn(
        conn,
        &conversation_id,
        &snapshot_user_text,
        replacement_assistant_id,
    )?;
    let active_persona =
        db::get_active_player_persona(conn, &conversation_id).map_err(|err| err.to_string())?;
    let active_persona_context = player_persona_context(&active_persona);
    let mut context_preview = compile_context_with_correction(
        &soul,
        Some(&session_world),
        &messages_to_context(before_messages),
        correction_instruction.as_deref(),
        Some(snapshot_user_text.as_str()),
        Some(&active_persona_context),
        // The mock provider has no profile to read a ceiling from.
        None,
    );
    if let Some(branch) = ledger_branch.as_ref() {
        append_memory_v2_evidence_bundle(
            conn,
            &conversation_id,
            &branch.branch_id,
            &snapshot_user_text,
            &mut context_preview,
        );
    }
    let (context_preview, effective_mock_user_text) = apply_pending_setup_to_turn(
        context_preview,
        snapshot_user_text.clone(),
        pending_setup_text.as_deref(),
    );
    let provider = MockProvider::default();
    let raw_response = provider.complete(
        &soul,
        &context_preview.text,
        &effective_mock_user_text,
        &mode,
    );
    let parsed = parse_hidden_state(&raw_response).map_err(|err| err.to_string())?;
    let (visible_response, replay_guard, output_contract_warning, _) =
        guard_narrator_visible_response(
            &parsed.visible_text,
            &snapshot_user_text,
            &session_world,
            &[],
            &soul.character_name,
        );
    let mut debug = debug_from_hidden_state("Mock", &parsed.hidden_state, true, false);
    debug.simulated_response = true;
    debug.replay_detected = replay_guard.replay_detected;
    debug.replay_score = replay_guard.replay_score;
    debug.replay_reason = replay_guard.replay_reason;
    debug.replay_compared_against_message_id = replay_guard.compared_against_message_id;
    debug.output_contract_warning = output_contract_warning;
    debug.narrator_response_saved = true;
    debug.state_updater_status = "mock_simulated".into();

    let (assistant_message_id, selected_variant_id) = save_visible_narrator_response(
        &conn,
        &conversation_id,
        &visible_response,
        replacement_assistant_id,
        correction_instruction.as_deref(),
        &pre_turn_soul_json,
        &snapshot_user_text,
        0,
        NarratorMessageOrigin::Mock,
        Some(&debug),
    )?;

    if let Some(branch) = ledger_branch.as_ref() {
        if replacement_assistant_id.is_some() {
            db::discard_active_commits_for_assistant(conn, &conversation_id, assistant_message_id)
                .map_err(|err| err.to_string())?;
        }
        let mut ledger_patch = parsed.engine_patch.clone();
        sanitize_mock_patch_for_ledger(&mut ledger_patch);
        db::record_turn_commit_with_patch_for_turn_id(
            conn,
            &canonical_turn_id,
            &conversation_id,
            &branch.branch_id,
            ledger_parent_turn_id.as_deref(),
            ledger_user_message_id,
            assistant_message_id,
            selected_variant_id,
            &ledger_patch,
            replacement_assistant_id.is_some(),
        )
        .map_err(|err| err.to_string())?;
        if let Some(instruction) = correction_instruction.as_deref() {
            db::append_memory_correction_event(
                conn,
                &conversation_id,
                &branch.branch_id,
                &canonical_turn_id,
                replacement_assistant_id,
                instruction,
            )
            .map_err(|err| err.to_string())?;
        }
        let rebuilt = db::rebuild_session_state(conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
        soul = rebuilt.soul;
        session_world = rebuilt.session_world;
    } else {
        let _ = parsed
            .engine_patch
            .apply_to_session(&mut soul, Some(&mut session_world));
        soul.turn_counter += 1;
        soul.turns_since_consolidation += 1;
    }

    if replacement_assistant_id.is_none() {
        db::upsert_turn_snapshot(
            &conn,
            &db::TurnSnapshot {
                conversation_id: conversation_id.clone(),
                assistant_message_id,
                user_text: snapshot_user_text.clone(),
                soul_json: pre_turn_soul_json.clone(),
            },
        )
        .map_err(|err| err.to_string())?;
    }

    let consolidation_ran =
        ledger_branch.is_none() && soul.turns_since_consolidation >= CONSOLIDATION_INTERVAL_TURNS;
    if consolidation_ran {
        consolidate_soul(&mut soul);
    }

    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
    db::upsert_session_world(&conn, &session_world).map_err(|err| err.to_string())?;
    let messages =
        db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
    let context_preview = compile_context_for_session_with_player_persona(
        &soul,
        Some(&session_world),
        &messages_to_context(messages.clone()),
        &active_persona_context,
    );

    Ok(TurnResult {
        conversation_id,
        soul,
        visible_response,
        context_preview,
        messages,
        consolidation_ran,
        debug,
    })
}

#[derive(Debug, Clone)]
struct CommandPatchOutcome {
    patch_id: String,
    turn_id: String,
    branch_id: String,
    applied_patch_count: usize,
    before_summary: serde_json::Value,
    after_summary: serde_json::Value,
}

#[derive(Debug, Clone)]
struct CommandTurnState {
    soul: Soul,
    session_world: SessionWorld,
    branch: Option<db::SessionBranch>,
    parent_turn_id: Option<String>,
    pre_turn_soul_json: String,
    evaluator_freshness_warning: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct CommandLlmResult {
    called: bool,
    mode: Option<&'static str>,
    system_prompt: String,
    user_message: String,
    response: Option<String>,
    raw_response: Option<String>,
    provider_error: Option<String>,
    model: String,
    base_url: String,
    elapsed_ms: u64,
    simulated: bool,
    output_guard_action: &'static str,
}

impl CommandLlmResult {
    fn not_called() -> Self {
        Self {
            output_guard_action: "none",
            ..Self::default()
        }
    }
}

fn command_message_channel(parsed: &ParsedChatCommand) -> &'static str {
    match parsed.kind {
        ChatCommandKind::Ooc => db::MESSAGE_CHANNEL_COMMAND_OOC,
        ChatCommandKind::Setup => db::MESSAGE_CHANNEL_COMMAND_SETUP,
        ChatCommandKind::State | ChatCommandKind::Status => db::MESSAGE_CHANNEL_COMMAND_STATE,
        ChatCommandKind::Persona => db::MESSAGE_CHANNEL_COMMAND_PERSONA,
        ChatCommandKind::Ask => db::MESSAGE_CHANNEL_COMMAND_ASK,
        ChatCommandKind::Help | ChatCommandKind::Unknown(_) => db::MESSAGE_CHANNEL_COMMAND_HELP,
        ChatCommandKind::None => db::MESSAGE_CHANNEL_RP_SCENE,
    }
}

fn maybe_handle_chat_command_with_conn(
    window: Option<&Window>,
    conn: &Connection,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    request_id: &str,
    canonical_turn_id: &str,
    context_mode: ContextMode,
    command_llm_result: Option<CommandLlmResult>,
) -> Result<Option<TurnResult>, String> {
    let parsed = parse_chat_command(&user_text);
    if !parsed.detected() {
        return Ok(None);
    }

    let mut state = load_command_turn_state(conn, &conversation_id, &soul_id)?;
    let mut command_llm_result =
        command_llm_result.unwrap_or_else(|| mock_command_llm_result(&parsed, &state));
    let command_channel = command_message_channel(&parsed);
    let user_message_id = Some(
        db::insert_message_with_channel_and_get_id(
            conn,
            &conversation_id,
            "user",
            &user_text,
            command_channel,
        )
        .map_err(|err| err.to_string())?,
    );
    let started = Instant::now();
    let mut pipeline_trace = TurnPipelineTrace::new(
        request_id.to_string(),
        Some(canonical_turn_id.to_string()),
        conversation_id.clone(),
        db::now_ts(),
    );
    pipeline_trace.record_stage(
        "chat_command_routed",
        "success",
        0,
        Some(parsed.kind_label()),
        Some("Narrator generation blocked by command router".into()),
    );

    let (
        mut response,
        route,
        evaluator_skip,
        state_mutation_allowed,
        pending_setup_updated,
        mut mutation_applied,
    ) = build_non_mutating_command_response(
        conn,
        &conversation_id,
        &parsed,
        &state,
        &command_llm_result,
    )?;
    let mut patch_outcome = None;
    let mut patch_source = None;
    let mut proposed_patch = serde_json::Value::Null;

    let (guarded_response, guard_action) = guard_command_output(&parsed, &response, &state);
    response = guarded_response;
    command_llm_result.output_guard_action = guard_action;

    if let Some((patch, source, pending_response)) =
        build_mutating_command_patch(conn, &parsed, &state, &command_llm_result)?
    {
        response = pending_response;
        patch_source = Some(source);
        proposed_patch = command_patch_summary_json(&patch);
        if !patch.is_empty() {
            let placeholder = "Applying state update...";
            let assistant_message_id = db::insert_message_with_channel_and_get_id(
                conn,
                &conversation_id,
                "assistant",
                placeholder,
                command_channel,
            )
            .map_err(|err| err.to_string())?;
            patch_outcome = Some(apply_command_patch_to_ledger(
                conn,
                &conversation_id,
                canonical_turn_id,
                user_message_id,
                assistant_message_id,
                &mut state,
                &patch,
            )?);
            mutation_applied = true;
            response = render_mutating_command_response(
                &parsed,
                patch_source.unwrap_or(MANUAL_USER_STATE_COMMAND_SOURCE),
                patch_outcome.as_ref(),
                command_llm_result.response.as_deref(),
            );
            let (guarded_response, guard_action) = guard_command_output(&parsed, &response, &state);
            response = guarded_response;
            command_llm_result.output_guard_action = guard_action;
            db::update_message_content(conn, &conversation_id, assistant_message_id, &response)
                .map_err(|err| err.to_string())?;
            db::upsert_turn_snapshot(
                conn,
                &db::TurnSnapshot {
                    conversation_id: conversation_id.clone(),
                    assistant_message_id,
                    user_text: user_text.clone(),
                    soul_json: state.pre_turn_soul_json.clone(),
                },
            )
            .map_err(|err| err.to_string())?;
            finalize_chat_command_turn(
                window,
                conn,
                conversation_id,
                user_text,
                state,
                response,
                parsed,
                route,
                evaluator_skip,
                state_mutation_allowed,
                pending_setup_updated,
                mutation_applied,
                patch_source,
                patch_outcome,
                proposed_patch,
                user_message_id,
                Some(assistant_message_id),
                request_id,
                canonical_turn_id,
                context_mode,
                pipeline_trace,
                started,
                command_llm_result,
            )
            .map(Some)
        } else {
            finalize_non_mutating_chat_command(
                window,
                conn,
                conversation_id,
                user_text,
                state,
                response,
                parsed,
                route,
                evaluator_skip,
                state_mutation_allowed,
                pending_setup_updated,
                mutation_applied,
                patch_source,
                patch_outcome,
                proposed_patch,
                user_message_id,
                request_id,
                canonical_turn_id,
                context_mode,
                pipeline_trace,
                started,
                command_llm_result,
            )
            .map(Some)
        }
    } else {
        finalize_non_mutating_chat_command(
            window,
            conn,
            conversation_id,
            user_text,
            state,
            response,
            parsed,
            route,
            evaluator_skip,
            state_mutation_allowed,
            pending_setup_updated,
            mutation_applied,
            patch_source,
            patch_outcome,
            proposed_patch,
            user_message_id,
            request_id,
            canonical_turn_id,
            context_mode,
            pipeline_trace,
            started,
            command_llm_result,
        )
        .map(Some)
    }
}

fn load_command_turn_state(
    conn: &Connection,
    conversation_id: &str,
    soul_id: &str,
) -> Result<CommandTurnState, String> {
    let fallback_soul = db::get_soul(conn, soul_id).map_err(|err| err.to_string())?;
    db::ensure_conversation(conn, conversation_id, &fallback_soul.character_id)
        .map_err(|err| err.to_string())?;
    let fallback_world =
        db::ensure_conversation_session_world(conn, conversation_id, &fallback_soul, None)
            .map_err(|err| err.to_string())?;
    let branch = db::get_active_session_branch(conn, conversation_id).ok();
    let (soul, session_world, parent_turn_id) = if let Some(branch) = branch.as_ref() {
        let parent_turn_id = branch.active_turn_id.clone();
        let rebuilt = db::rebuild_session_state_until(
            conn,
            conversation_id,
            &branch.branch_id,
            parent_turn_id.as_deref(),
        )
        .map_err(|err| err.to_string())?;
        (rebuilt.soul, rebuilt.session_world, parent_turn_id)
    } else {
        (fallback_soul, fallback_world, None)
    };
    let pre_turn_soul_json = serde_json::to_string(&soul).map_err(|err| err.to_string())?;
    let evaluator_freshness_warning =
        command_state_evaluator_freshness_warning(conn, conversation_id)?;
    Ok(CommandTurnState {
        soul,
        session_world,
        branch,
        parent_turn_id,
        pre_turn_soul_json,
        evaluator_freshness_warning,
    })
}

fn command_state_evaluator_freshness_warning(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Option<String>, String> {
    let pending_jobs = db::get_pending_evaluator_jobs_for_conversation(conn, conversation_id)
        .map_err(|err| err.to_string())?;
    if pending_jobs.is_empty() {
        return Ok(None);
    }
    let pending_job_ids = pending_jobs
        .iter()
        .map(|job| job.evaluator_job_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Ok(Some(format!(
        "Evaluator update pending; status may not include the latest scene enrichment yet. Pending jobs: {pending_job_ids}."
    )))
}

fn command_llm_mode(parsed: &ParsedChatCommand) -> Option<&'static str> {
    match parsed.kind {
        ChatCommandKind::Ooc => Some("ooc"),
        ChatCommandKind::Setup => Some("setup"),
        ChatCommandKind::State => match parsed.state_subcommand {
            Some(StateSubcommandKind::Show) => Some("state_summary"),
            Some(StateSubcommandKind::Update) => Some("state_edit"),
            _ => None,
        },
        ChatCommandKind::Status => Some("state_summary"),
        ChatCommandKind::Persona => None,
        ChatCommandKind::Ask => Some("soul_edit_agent"),
        // Help is a local capability index. It must remain available even when
        // the configured provider or model is unavailable.
        ChatCommandKind::Help => None,
        _ => None,
    }
}

fn command_system_prompt_for_mode(mode: &str) -> &'static str {
    match mode {
        "ooc" => build_command_ooc_prompt(),
        "setup" => build_command_setup_prompt(),
        "state_summary" => build_command_state_summary_prompt(),
        "state_edit" => build_command_state_edit_prompt(),
        "soul_edit_agent" => build_command_soul_edit_agent_prompt(),
        "help" => build_command_help_prompt(),
        _ => build_command_ooc_prompt(),
    }
}

fn command_llm_response_or_fallback(
    command_llm_result: &CommandLlmResult,
    fallback: impl FnOnce() -> String,
) -> String {
    command_llm_result
        .response
        .as_deref()
        .map(str::trim)
        .filter(|response| !response.is_empty())
        .map(str::to_string)
        .unwrap_or_else(fallback)
}

fn mock_command_llm_result(
    parsed: &ParsedChatCommand,
    state: &CommandTurnState,
) -> CommandLlmResult {
    let Some(mode) = command_llm_mode(parsed) else {
        return CommandLlmResult::not_called();
    };
    let user_message = build_command_llm_user_message(parsed, state, &[]);
    let response = match mode {
        "ooc" => {
            if parsed.body.trim().is_empty() {
                "Out-of-roleplay assistant ready. No RP narrator or scene evaluator was run.".into()
            } else {
                "Out-of-roleplay request noted. No RP narrator or scene evaluator was run.".into()
            }
        }
        "setup" => {
            "Setup staged.\nPending setup:\n- Setup saved for the next normal RP turn.\nNo scene narration or state update was run.".into()
        }
        "state_summary" => {
            render_state_show_response(&parsed.body, state)
        }
        "state_edit" => {
            "Risk level: low\nTarget: tracked session state\nReason: Direct operator state update command.\nValidated edit intent: queued\nApply behavior: applied".into()
        }
        "soul_edit_agent" => render_mock_soul_edit_agent_response(parsed, state),
        "help" => {
            render_help_response()
        }
        _ => String::new(),
    };
    CommandLlmResult {
        called: true,
        mode: Some(mode),
        system_prompt: command_system_prompt_for_mode(mode).to_string(),
        user_message,
        response: Some(sanitize_command_llm_response(&response)),
        raw_response: Some(response),
        provider_error: None,
        model: "local-command-sim".into(),
        base_url: "local".into(),
        elapsed_ms: 0,
        simulated: true,
        output_guard_action: "none",
    }
}

fn render_mock_soul_edit_agent_response(
    parsed: &ParsedChatCommand,
    state: &CommandTurnState,
) -> String {
    let instruction = parsed.body.trim();
    if instruction.is_empty() {
        return "Risk level: low\nTarget: none\nReason: No edit instruction was provided.\nProposed edit: none\nApply behavior: plan only"
            .into();
    }
    let risk = if is_high_risk_soul_edit(instruction) {
        "high"
    } else {
        "low"
    };
    let mutation_note =
        if parsed.ask_mode == AskMode::Plan || parsed.ask_mode == AskMode::Diff || risk == "high" {
            " No state was changed."
        } else {
            ""
        };
    format!(
        "Risk level: {risk}\nTarget: scene/session state\nReason: Current focus is '{}'.\nProposed edit: {}{mutation_note}\nApply behavior: {}",
        state.session_world.scene_state.focus.trim(),
        instruction,
        if mutation_note.is_empty() { "applied" } else { "plan only" }
    )
}

fn sanitize_command_llm_response(response: &str) -> String {
    let without_hidden = strip_hidden_state_blocks(response);
    let without_status = strip_status_blocks_for_export(&without_hidden);
    let trimmed = without_status.trim();
    if trimmed.is_empty() {
        "Command completed without scene narration.".into()
    } else {
        trimmed.to_string()
    }
}

fn guard_command_output(
    parsed: &ParsedChatCommand,
    response: &str,
    state: &CommandTurnState,
) -> (String, &'static str) {
    if !looks_like_live_scene_prose(response) {
        return (response.to_string(), "none");
    }
    let fallback = match parsed.kind {
        ChatCommandKind::Setup => render_setup_fallback(&parsed.body),
        ChatCommandKind::State | ChatCommandKind::Status => {
            render_state_show_response(&parsed.body, state)
        }
        ChatCommandKind::Ask => render_ask_guard_fallback(&parsed.body),
        ChatCommandKind::Ooc => {
            "OOC reply blocked because it looked like live scene continuation. No RP narrator or scene evaluator was run.".into()
        }
        ChatCommandKind::Persona => render_persona_help_response(),
        ChatCommandKind::Help | ChatCommandKind::Unknown(_) | ChatCommandKind::None => {
            render_help_response()
        }
    };
    (fallback, "deterministic_fallback_used")
}

fn looks_like_live_scene_prose(response: &str) -> bool {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("```status") || lower.contains("scene | focus:") {
        return true;
    }
    let first_line = trimmed
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let lower_first = first_line.trim().to_ascii_lowercase();
    let scene_opening = [
        "aurora ",
        "she ",
        "he ",
        "they ",
        "the door ",
        "the room ",
        "the chain ",
    ]
    .iter()
    .any(|prefix| lower_first.starts_with(prefix));
    let actionish = [
        " says",
        " whispers",
        " steps",
        " looks",
        " reaches",
        " turns",
        " breathes",
        " holds",
        " opens",
        " closes",
        "\"",
    ]
    .iter()
    .any(|needle| lower_first.contains(needle) || lower.contains(needle));
    scene_opening && actionish
}

fn render_setup_fallback(body: &str) -> String {
    let summary = body
        .trim()
        .lines()
        .flat_map(|line| line.split('.'))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(4)
        .map(|line| format!("- {}", line.chars().take(180).collect::<String>()))
        .collect::<Vec<_>>();
    let summary = if summary.is_empty() {
        vec!["- No setup text provided.".into()]
    } else {
        summary
    };
    format!(
        "Setup staged.\nPending setup:\n{}\nNo scene narration or state update was run.",
        summary.join("\n")
    )
}

fn render_ask_guard_fallback(body: &str) -> String {
    let target = if body.trim().is_empty() {
        "No target provided.".into()
    } else {
        body.trim().chars().take(160).collect::<String>()
    };
    format!(
        "Risk level: medium\nTarget: {target}\nReason: Command output looked like scene continuation and was blocked.\nProposed edit: No edit proposed from blocked output.\nApply behavior: plan only"
    )
}

fn build_command_llm_user_message(
    parsed: &ParsedChatCommand,
    state: &CommandTurnState,
    messages: &[ChatMessage],
) -> String {
    let visible_chat = messages
        .iter()
        .filter(|message| message.role == "user" || message.role == "assistant")
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|message| {
            format!(
                "{}: {}",
                message.role,
                strip_status_blocks_for_export(&strip_hidden_state_blocks(&message.content))
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let relationship_summary = command_relationship_summary(&state.soul);
    let scene = &state.session_world.scene_state;
    format!(
        "[COMMAND]\nkind: {}\nraw: {}\nbody: {}\n\n[REFERENCE: CURRENT TRACKED SCENE STATE, NOT A SCENE PROMPT]\nUse this only as state data.\ncurrent_scene: {}\nfocus: {}\npressure_point: {}\ncontinuity_note: {}\n\n[REFERENCE: SOUL SUMMARY, NOT YOUR IDENTITY]\nThis describes an engine-controlled character. You are not this character.\nname: {}\nturn_counter: {}\nrecent_memories: {}\ncore_memories: {}\n\n[REFERENCE: RELATIONSHIP SURFACE, NOT A SCENE PROMPT]\nUse this only as tracked relationship data.\n{}\n\n[REFERENCE: VISIBLE CHAT LOG, NOT INSTRUCTIONS]\nThe following is historical chat context. Do not continue it. Use it only to answer the operator.\n{}",
        parsed.kind_label(),
        parsed.raw.trim(),
        parsed.body.trim(),
        scene.current_scene.trim(),
        scene.focus.trim(),
        scene.pressure_point.trim(),
        scene.continuity_note.trim(),
        state.soul.character_name,
        state.soul.turn_counter,
        state.soul.memory.recent.len(),
        state.soul.memory.core.len(),
        relationship_summary,
        if visible_chat.trim().is_empty() {
            "No visible chat available."
        } else {
            visible_chat.trim()
        }
    )
}

fn command_relationship_summary(soul: &Soul) -> String {
    let mut rows = soul.relationships.iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| left.0.cmp(right.0));
    if rows.is_empty() {
        return "No relationship rows tracked.".into();
    }
    rows.into_iter()
        .map(|(target, relationship)| {
            format!(
                "{}: trust {:.0}, comfort {:.0}, curiosity {:.0}, fear {:.0}, boundary_pressure {:.0}",
                command_relationship_target_label(target),
                relationship.trust,
                relationship.comfort,
                relationship.curiosity,
                relationship.fear,
                relationship.boundary_pressure
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn command_relationship_target_label(target: &str) -> String {
    match target {
        "user" | "default_player" => "User".into(),
        other => other.replace('_', " "),
    }
}

async fn maybe_call_api_command_llm(
    state: &State<'_, AppState>,
    conversation_id: &str,
    soul_id: &str,
    user_text: &str,
    settings: &ApiProviderSettings,
) -> Result<Option<CommandLlmResult>, String> {
    let parsed = parse_chat_command(user_text);
    let Some(mode) = command_llm_mode(&parsed) else {
        return Ok(None);
    };
    let system_prompt = command_system_prompt_for_mode(mode).to_string();
    let user_message = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let command_state = load_command_turn_state(&conn, conversation_id, soul_id)?;
        let messages =
            db::list_messages(&conn, conversation_id, 12).map_err(|err| err.to_string())?;
        build_command_llm_user_message(&parsed, &command_state, &messages)
    };
    let started = Instant::now();
    let provider = ApiProvider::default();
    let raw_response = provider
        .complete_prompt(settings, &system_prompt, &user_message, 0.35)
        .await;
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let (response, raw_response, provider_error) = match raw_response {
        Ok(raw) => (Some(sanitize_command_llm_response(&raw)), Some(raw), None),
        Err(err) => {
            let summary = summarize_provider_error(&err);
            (
                Some(format!(
                    "Command assistant unavailable. {summary}\nNo RP narrator or scene evaluator was run."
                )),
                None,
                Some(summary),
            )
        }
    };
    Ok(Some(CommandLlmResult {
        called: true,
        mode: Some(mode),
        system_prompt,
        user_message,
        response,
        raw_response,
        provider_error,
        model: settings.model.trim().to_string(),
        base_url: settings.base_url.trim().to_string(),
        elapsed_ms,
        simulated: false,
        output_guard_action: "none",
    }))
}

fn summarize_provider_error(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") {
        "Authentication failed. Check the API key in Settings.".into()
    } else if lower.contains("403") || lower.contains("forbidden") {
        "The provider refused access. Check the API key and model permissions in Settings.".into()
    } else if lower.contains("404") || lower.contains("not found") || lower.contains("no endpoints")
    {
        "The selected model or endpoint is unavailable. Choose another profile in Settings.".into()
    } else if lower.contains("429") || lower.contains("rate limit") {
        "The provider rate limit was reached. Wait briefly or choose another profile.".into()
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "The provider timed out. Check the connection or increase the timeout in Settings.".into()
    } else {
        "The provider request failed. Check the connection and selected profile in Settings.".into()
    }
}

fn build_non_mutating_command_response(
    conn: &Connection,
    conversation_id: &str,
    parsed: &ParsedChatCommand,
    state: &CommandTurnState,
    command_llm_result: &CommandLlmResult,
) -> Result<(String, &'static str, &'static str, bool, bool, bool), String> {
    match &parsed.kind {
        ChatCommandKind::Ooc => Ok((
            command_llm_response_or_fallback(command_llm_result, || {
                render_ooc_response(&parsed.body)
            }),
            "command_ooc_llm",
            "command_ooc_llm",
            false,
            false,
            false,
        )),
        ChatCommandKind::Setup => {
            db::set_pending_setup(conn, conversation_id, &parsed.body)
                .map_err(|err| err.to_string())?;
            Ok((
                command_llm_response_or_fallback(command_llm_result, || {
                    render_setup_fallback(&parsed.body)
                }),
                "setup_staged",
                "setup_only",
                false,
                true,
                false,
            ))
        }
        ChatCommandKind::State => match parsed
            .state_subcommand
            .clone()
            .unwrap_or(StateSubcommandKind::Show)
        {
            StateSubcommandKind::Show => Ok((
                command_llm_response_or_fallback(command_llm_result, || {
                    render_state_show_response(&parsed.body, state)
                }),
                "command_state_summary",
                "command_state_summary",
                false,
                false,
                false,
            )),
            StateSubcommandKind::Review => Ok((
                render_state_review_response(conn, conversation_id)?,
                "state_review",
                "slash_state_review",
                false,
                false,
                false,
            )),
            StateSubcommandKind::Update => Ok((
                "State update could not be applied.".into(),
                "manual_state_patch",
                "manual_state_patch",
                true,
                false,
                false,
            )),
            StateSubcommandKind::Unknown(command) => Ok((
                format!("Unknown /state command {command}. Use /help for commands."),
                "state_unknown",
                "unknown_state_subcommand",
                false,
                false,
                false,
            )),
        },
        ChatCommandKind::Status => Ok((
            command_llm_response_or_fallback(command_llm_result, || {
                render_state_show_response(&parsed.body, state)
            }),
            "command_state_summary",
            "command_state_summary",
            false,
            false,
            false,
        )),
        ChatCommandKind::Persona => {
            let (response, route, mutation_allowed, mutation_applied) =
                handle_persona_command(conn, conversation_id, parsed)?;
            Ok((
                response,
                route,
                "slash_persona_command",
                mutation_allowed,
                false,
                mutation_applied,
            ))
        }
        ChatCommandKind::Ask => Ok((
            command_llm_response_or_fallback(command_llm_result, || {
                "Ask command did not produce a safe edit.".into()
            }),
            "agent_soul_edit_llm",
            "agent_soul_edit_llm",
            matches!(parsed.ask_mode, AskMode::Auto | AskMode::Apply),
            false,
            false,
        )),
        ChatCommandKind::Help => Ok((
            command_llm_response_or_fallback(command_llm_result, || render_help_response()),
            "help",
            "slash_help",
            false,
            false,
            false,
        )),
        ChatCommandKind::Unknown(command) => Ok((
            format!("Unknown command /{command}. Use /help for commands."),
            "unknown",
            "unknown_slash_command",
            false,
            false,
            false,
        )),
        ChatCommandKind::None => Ok(("".into(), "none", "none", false, false, false)),
    }
}

fn build_mutating_command_patch(
    conn: &Connection,
    parsed: &ParsedChatCommand,
    state: &CommandTurnState,
    command_llm_result: &CommandLlmResult,
) -> Result<Option<(EnginePatch, &'static str, String)>, String> {
    match &parsed.kind {
        ChatCommandKind::State => {
            if parsed.state_subcommand != Some(StateSubcommandKind::Update) {
                return Ok(None);
            }
            let Some((target, instruction)) = parse_state_update_body(&parsed.body) else {
                return Ok(Some((
                    EnginePatch::default(),
                    MANUAL_USER_STATE_COMMAND_SOURCE,
                    "Please use `/state update <target> <instruction>`.".into(),
                )));
            };
            if let Some(reason) = hard_delete_or_external_write_reason(&instruction) {
                return Ok(Some((
                    EnginePatch::default(),
                    MANUAL_USER_STATE_COMMAND_SOURCE,
                    format!("State update blocked: {reason}"),
                )));
            }
            let patch = scene_state_command_patch(&target, &instruction, "user_state_command");
            Ok(Some((
                patch,
                MANUAL_USER_STATE_COMMAND_SOURCE,
                command_llm_response_or_fallback(command_llm_result, || {
                    "State update queued.".into()
                }),
            )))
        }
        ChatCommandKind::Ask => {
            let instruction = parsed.body.trim().to_string();
            if instruction.is_empty() {
                return Ok(Some((
                    EnginePatch::default(),
                    AI_AGENT_SOUL_EDIT_COMMAND_SOURCE,
                    "Please include what you want the agent to inspect or edit.".into(),
                )));
            }
            if let Some(reason) = hard_delete_or_external_write_reason(&instruction) {
                return Ok(Some((
                    EnginePatch::default(),
                    AI_AGENT_SOUL_EDIT_COMMAND_SOURCE,
                    format!("Ask edit blocked: {reason}"),
                )));
            }
            let patch = scene_state_command_patch("state", &instruction, "ai_agent_soul_edit");
            if parsed.ask_mode == AskMode::Plan
                || parsed.ask_mode == AskMode::Diff
                || is_high_risk_soul_edit(&instruction)
            {
                let response = command_llm_response_or_fallback(command_llm_result, || {
                    render_ask_proposal_response(parsed.ask_mode, &instruction, &patch)
                });
                return Ok(Some((
                    EnginePatch::default(),
                    AI_AGENT_SOUL_EDIT_COMMAND_SOURCE,
                    response,
                )));
            }
            let _ = conn;
            let _ = state;
            Ok(Some((
                patch,
                AI_AGENT_SOUL_EDIT_COMMAND_SOURCE,
                command_llm_response_or_fallback(command_llm_result, || "Ask edit queued.".into()),
            )))
        }
        _ => Ok(None),
    }
}

fn apply_command_patch_to_ledger(
    conn: &Connection,
    conversation_id: &str,
    canonical_turn_id: &str,
    user_message_id: Option<i64>,
    assistant_message_id: i64,
    state: &mut CommandTurnState,
    patch: &EnginePatch,
) -> Result<CommandPatchOutcome, String> {
    patch.validate().map_err(|err| format!("{err:?}"))?;
    let branch = if let Some(branch) = state.branch.clone() {
        branch
    } else {
        db::create_session_branch(conn, conversation_id, &state.soul, &state.session_world)
            .map_err(|err| err.to_string())?
    };
    let before_summary = compact_state_summary_json(&state.soul, &state.session_world);
    db::record_turn_commit_with_patch_for_turn_id(
        conn,
        canonical_turn_id,
        conversation_id,
        &branch.branch_id,
        state.parent_turn_id.as_deref(),
        user_message_id,
        assistant_message_id,
        None,
        patch,
        false,
    )
    .map_err(|err| err.to_string())?;
    let rebuilt = db::rebuild_session_state(conn, conversation_id, &branch.branch_id)
        .map_err(|err| err.to_string())?;
    let patch_id = rebuilt
        .debug
        .applied_patches
        .last()
        .cloned()
        .unwrap_or_default();
    let applied_patch_count = rebuilt.debug.applied_patches.len();
    state.soul = rebuilt.soul;
    state.session_world = rebuilt.session_world;
    state.branch = Some(branch.clone());
    state.parent_turn_id = Some(canonical_turn_id.to_string());
    let after_summary = compact_state_summary_json(&state.soul, &state.session_world);
    Ok(CommandPatchOutcome {
        patch_id,
        turn_id: canonical_turn_id.to_string(),
        branch_id: branch.branch_id,
        applied_patch_count,
        before_summary,
        after_summary,
    })
}

#[allow(clippy::too_many_arguments)]
fn finalize_non_mutating_chat_command(
    window: Option<&Window>,
    conn: &Connection,
    conversation_id: String,
    user_text: String,
    state: CommandTurnState,
    response: String,
    parsed: ParsedChatCommand,
    route: &'static str,
    evaluator_skip: &'static str,
    state_mutation_allowed: bool,
    pending_setup_updated: bool,
    mutation_applied: bool,
    patch_source: Option<&'static str>,
    patch_outcome: Option<CommandPatchOutcome>,
    proposed_patch: serde_json::Value,
    user_message_id: Option<i64>,
    request_id: &str,
    canonical_turn_id: &str,
    context_mode: ContextMode,
    pipeline_trace: TurnPipelineTrace,
    started: Instant,
    command_llm_result: CommandLlmResult,
) -> Result<TurnResult, String> {
    let assistant_message_id = db::insert_message_with_channel_and_get_id(
        conn,
        &conversation_id,
        "assistant",
        &response,
        command_message_channel(&parsed),
    )
    .map_err(|err| err.to_string())?;
    finalize_chat_command_turn(
        window,
        conn,
        conversation_id,
        user_text,
        state,
        response,
        parsed,
        route,
        evaluator_skip,
        state_mutation_allowed,
        pending_setup_updated,
        mutation_applied,
        patch_source,
        patch_outcome,
        proposed_patch,
        user_message_id,
        Some(assistant_message_id),
        request_id,
        canonical_turn_id,
        context_mode,
        pipeline_trace,
        started,
        command_llm_result,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_chat_command_turn(
    window: Option<&Window>,
    conn: &Connection,
    conversation_id: String,
    user_text: String,
    state: CommandTurnState,
    response: String,
    parsed: ParsedChatCommand,
    route: &'static str,
    evaluator_skip: &'static str,
    state_mutation_allowed: bool,
    pending_setup_updated: bool,
    mutation_applied: bool,
    patch_source: Option<&'static str>,
    patch_outcome: Option<CommandPatchOutcome>,
    proposed_patch: serde_json::Value,
    user_message_id: Option<i64>,
    assistant_message_id: Option<i64>,
    request_id: &str,
    canonical_turn_id: &str,
    context_mode: ContextMode,
    mut pipeline_trace: TurnPipelineTrace,
    started: Instant,
    command_llm_result: CommandLlmResult,
) -> Result<TurnResult, String> {
    if command_llm_result.called {
        pipeline_trace.record_stage(
            "command_llm_called",
            if command_llm_result.provider_error.is_some() {
                "warning"
            } else {
                "success"
            },
            command_llm_result.elapsed_ms,
            command_llm_result.mode.map(str::to_string),
            command_llm_result
                .provider_error
                .as_ref()
                .map(|err| format!("provider_error={err}")),
        );
    }
    pipeline_trace.record_stage(
        "rp_narrator_called",
        "skipped",
        0,
        None,
        Some("rp_narrator_called=false".into()),
    );
    pipeline_trace.record_stage(
        "narrator_called",
        "skipped",
        0,
        None,
        Some("legacy narrator stage skipped; rp_narrator_called=false".into()),
    );
    pipeline_trace.record_stage(
        "evaluator_job_started",
        "skipped",
        0,
        None,
        Some(format!("evaluator_skipped_reason={evaluator_skip}")),
    );
    pipeline_trace.final_status = "success".into();
    pipeline_trace.finalize_timing(started.elapsed().as_millis() as u64);

    let chat_trace = chat_command_trace_json(
        &parsed,
        route,
        evaluator_skip,
        state_mutation_allowed,
        pending_setup_updated,
        mutation_applied,
        patch_source,
        patch_outcome.as_ref(),
        proposed_patch,
        user_message_id,
        &pipeline_trace,
        &command_llm_result,
    );
    insert_chat_command_payload_log(
        conn,
        &conversation_id,
        assistant_message_id,
        &user_text,
        &response,
        request_id,
        canonical_turn_id,
        context_mode,
        &chat_trace,
        state.branch.as_ref().map(|branch| branch.branch_id.clone()),
        state.parent_turn_id.clone(),
        patch_outcome
            .as_ref()
            .map(|outcome| vec![outcome.patch_id.clone()])
            .unwrap_or_default(),
        &command_llm_result,
    )?;

    if let (Some(window), Some(message_id)) = (window, assistant_message_id) {
        if let Ok(message) = db::get_message(conn, &conversation_id, message_id) {
            let _ = window.emit(
                "chat-message-saved",
                SavedChatMessageEvent {
                    conversation_id: conversation_id.clone(),
                    message,
                },
            );
        }
        let _ = window.emit("pipeline-trace-updated", &pipeline_trace);
    }

    let messages = db::list_messages(conn, &conversation_id, 100).map_err(|err| err.to_string())?;
    let persona =
        db::get_active_player_persona(conn, &conversation_id).map_err(|err| err.to_string())?;
    let persona_context = player_persona_context(&persona);
    let context_preview = compile_context_for_session_with_player_persona(
        &state.soul,
        Some(&state.session_world),
        &messages_to_context(messages.clone()),
        &persona_context,
    );
    let mut debug = TurnDebug {
        provider: "CommandRouter".into(),
        hidden_state_found: false,
        fallback_hidden_state_generated: false,
        narrator_response_saved: true,
        assistant_message_id,
        selected_variant_id: None,
        state_updater_status: route.into(),
        replay_detected: false,
        replay_score: 0.0,
        replay_reason: None,
        replay_compared_against_message_id: None,
        output_contract_warning: None,
        tag: None,
        trust_delta: None,
        affection_delta: None,
        new_location: None,
        present_characters: Vec::new(),
        request_id: Some(request_id.to_string()),
        turn_id: Some(canonical_turn_id.to_string()),
        state_patch_id: patch_outcome
            .as_ref()
            .map(|outcome| outcome.patch_id.clone()),
        baseline_patch_id: None,
        enrichment_patch_id: None,
        simulated_response: false,
        fallback_used: false,
        fallback_reason: None,
    };
    if !mutation_applied && state_mutation_allowed {
        debug.state_updater_status = format!("{route}_no_patch_applied");
    }

    Ok(TurnResult {
        conversation_id,
        soul: state.soul,
        visible_response: response,
        context_preview,
        messages,
        consolidation_ran: false,
        debug,
    })
}

fn insert_chat_command_payload_log(
    conn: &Connection,
    conversation_id: &str,
    assistant_message_id: Option<i64>,
    user_text: &str,
    response: &str,
    request_id: &str,
    turn_id: &str,
    context_mode: ContextMode,
    trace: &serde_json::Value,
    branch_id: Option<String>,
    parent_turn_id: Option<String>,
    applied_patch_ids: Vec<String>,
    command_llm_result: &CommandLlmResult,
) -> Result<(), String> {
    let provider = if command_llm_result.called {
        "chat_command_llm"
    } else {
        "chat_command_router"
    };
    let mode = command_llm_result
        .mode
        .map(|mode| format!("slash_command:{mode}"))
        .unwrap_or_else(|| "slash_command".into());
    let system_message = command_llm_result.system_prompt.clone();
    let command_user_message = if command_llm_result.user_message.trim().is_empty() {
        user_text.to_string()
    } else {
        command_llm_result.user_message.clone()
    };
    let context_text = if command_llm_result.called {
        "Command LLM response; RP narrator/evaluator not invoked.".into()
    } else {
        "Command router response; RP narrator/evaluator not invoked.".into()
    };
    db::insert_llm_payload_log(
        conn,
        &LlmPayloadLog {
            id: 0,
            conversation_id: conversation_id.to_string(),
            message_id: assistant_message_id,
            provider: provider.into(),
            mode,
            context_mode: context_mode.label().into(),
            model: command_llm_result.model.clone(),
            base_url: command_llm_result.base_url.clone(),
            system_message,
            user_message: command_user_message.clone(),
            context_text,
            estimated_system_tokens: estimate_tokens(&command_llm_result.system_prompt),
            estimated_user_tokens: estimate_tokens(&command_user_message),
            estimated_total_tokens: estimate_tokens(&command_llm_result.system_prompt)
                + estimate_tokens(&command_user_message)
                + estimate_tokens(response),
            truncated: false,
            created_at: db::now_ts(),
            branch_id,
            active_turn_id: Some(turn_id.to_string()),
            parent_turn_id,
            state_patch_ids_applied: applied_patch_ids,
            discarded_patch_ids_skipped: Vec::new(),
            state_rebuild_generation: None,
            latest_assistant_variant_id: None,
            request_id: Some(request_id.to_string()),
            turn_id: Some(turn_id.to_string()),
            raw_provider_response: command_llm_result.raw_response.clone(),
            normalized_response: Some(response.to_string()),
            finish_reason: Some("command_router".into()),
            provider_error: command_llm_result.provider_error.clone(),
            fallback_used: false,
            fallback_reason: None,
            provider_request_id: None,
            provider_response_id: None,
            pipeline_trace_json: Some(
                serde_json::to_string_pretty(trace).unwrap_or_else(|_| trace.to_string()),
            ),
        },
    )
    .map(|_| ())
    .map_err(|err| err.to_string())
}

#[allow(clippy::too_many_arguments)]
fn chat_command_trace_json(
    parsed: &ParsedChatCommand,
    route: &str,
    evaluator_skip: &str,
    state_mutation_allowed: bool,
    pending_setup_updated: bool,
    mutation_applied: bool,
    patch_source: Option<&str>,
    patch_outcome: Option<&CommandPatchOutcome>,
    proposed_patch: serde_json::Value,
    user_message_id: Option<i64>,
    pipeline_trace: &TurnPipelineTrace,
    command_llm_result: &CommandLlmResult,
) -> serde_json::Value {
    let patch_id = patch_outcome.map(|outcome| outcome.patch_id.clone());
    let trace = serde_json::json!({
        "chat_command_detected": true,
        "chat_command_kind": parsed.kind_label(),
        "chat_command_route": route,
        "rp_narrator_called": false,
        "scene_narration_blocked": true,
        "command_llm_called": command_llm_result.called,
        "command_llm_mode": command_llm_result.mode,
        "command_llm_simulated": command_llm_result.simulated,
        "command_llm_provider_error": command_llm_result.provider_error.as_deref(),
        "command_output_guard_action": command_llm_result.output_guard_action,
        "scene_evaluator_skipped": true,
        "scene_evaluator_skipped_reason": evaluator_skip,
        "evaluator_skipped_reason": evaluator_skip,
        "state_mutation_allowed": state_mutation_allowed,
        "pending_setup_updated": pending_setup_updated,
        "user_message_id": user_message_id,
        "mutation_applied": mutation_applied,
        "manual_patch_source": (patch_source == Some(MANUAL_USER_STATE_COMMAND_SOURCE)).then_some("user_state_command"),
        "patch_source": patch_source,
        "patch_id": patch_id,
        "turn_id": patch_outcome.map(|outcome| outcome.turn_id.clone()),
        "branch_id": patch_outcome.map(|outcome| outcome.branch_id.clone()),
        "applied_patch_count": patch_outcome.map(|outcome| outcome.applied_patch_count).unwrap_or(0),
        "before_after_state_summary": patch_outcome.map(|outcome| serde_json::json!({
            "before": outcome.before_summary.clone(),
            "after": outcome.after_summary.clone()
        })),
        "proposed_patch_summary": proposed_patch,
        "pipeline_trace": pipeline_trace,
    });
    trace
}

fn render_ooc_response(body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        "Out-of-roleplay assistant ready.".into()
    } else {
        "Noted. No scene narration or state update was run.".into()
    }
}

fn render_help_response() -> String {
    [
        "Commands:",
        "/ooc <message> - Out-of-roleplay assistant reply only.",
        "/setup <text> - Stage setup for the next scene turn.",
        "/state show [target] - Show compact state.",
        "/state update <target> <instruction> - Apply a validated manual state patch.",
        "/state review - Show pending setup and recent command state activity.",
        "/persona list|lookup|change|add|edit - Manage the user-controlled RP persona.",
        "/ask [plan|apply|diff] <request> - Ask the state agent for a safe Soul/state edit.",
        "/help - Show this list.",
        "/status [target] - Deprecated alias for /state show [target].",
    ]
    .join("\n")
}

fn render_persona_help_response() -> String {
    [
        "Persona commands:",
        "/persona list - Show personas and allow selection.",
        "/persona lookup [persona_id or name] - Show current or selected persona details.",
        "/persona change <persona_id or name> - Change the active player persona.",
        "/persona add - Open the Add Persona dialog.",
        "/persona edit <persona_id or name> - Open the Edit Persona dialog.",
    ]
    .join("\n")
}

fn handle_persona_command(
    conn: &Connection,
    conversation_id: &str,
    parsed: &ParsedChatCommand,
) -> Result<(String, &'static str, bool, bool), String> {
    match parsed
        .persona_subcommand
        .clone()
        .unwrap_or(PersonaSubcommandKind::Help)
    {
        PersonaSubcommandKind::List => {
            let personas = db::list_player_personas(conn).map_err(|err| err.to_string())?;
            let active = db::get_active_player_persona_id(conn, conversation_id)
                .map_err(|err| err.to_string())?;
            Ok((
                render_persona_list_response(&personas, &active),
                "persona_list",
                false,
                false,
            ))
        }
        PersonaSubcommandKind::Lookup => {
            let lookup = persona_command_arg(&parsed.body);
            let persona = if lookup.trim().is_empty() {
                db::get_active_player_persona(conn, conversation_id)
                    .map_err(|err| err.to_string())?
            } else {
                db::find_player_persona(conn, &lookup)
                    .map_err(|err| err.to_string())?
                    .ok_or_else(|| format!("Persona not found: {lookup}"))?
            };
            Ok((
                render_persona_details(&persona),
                "persona_lookup",
                false,
                false,
            ))
        }
        PersonaSubcommandKind::Change => {
            let lookup = persona_command_arg(&parsed.body);
            if lookup.trim().is_empty() {
                return Ok((
                    "Persona change needs a persona_id or display name.".into(),
                    "persona_change",
                    true,
                    false,
                ));
            }
            let persona = db::find_player_persona(conn, &lookup)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("Persona not found: {lookup}"))?;
            let persona = db::set_active_player_persona(conn, conversation_id, &persona.persona_id)
                .map_err(|err| err.to_string())?;
            Ok((
                format!(
                    "Active player persona changed.\n{}",
                    render_persona_details(&persona)
                ),
                "persona_change",
                true,
                true,
            ))
        }
        PersonaSubcommandKind::Add => Ok((
            "Open Add Persona UI. No LLM was called.".into(),
            "persona_add",
            true,
            false,
        )),
        PersonaSubcommandKind::Edit => Ok((
            "Open Edit Persona UI. No LLM was called.".into(),
            "persona_edit",
            true,
            false,
        )),
        PersonaSubcommandKind::Help => {
            Ok((render_persona_help_response(), "persona_help", false, false))
        }
        PersonaSubcommandKind::Unknown(command) => Ok((
            format!("Unknown /persona command {command}. Use /persona for help."),
            "persona_unknown",
            false,
            false,
        )),
    }
}

fn persona_command_arg(body: &str) -> String {
    let mut parts = body.split_whitespace();
    let _subcommand = parts.next();
    parts.collect::<Vec<_>>().join(" ")
}

fn render_persona_list_response(personas: &[PlayerPersona], active_persona_id: &str) -> String {
    let mut lines = vec!["Player personas:".to_string()];
    for persona in personas {
        let selected = if persona.persona_id == active_persona_id {
            "selected"
        } else {
            "available"
        };
        lines.push(format!(
            "- {} ({}) [{}] - {}",
            persona.display_name, persona.persona_id, selected, persona.description
        ));
    }
    lines.push("Use /persona change <persona_id or name> to switch.".into());
    lines.join("\n")
}

fn render_persona_details(persona: &PlayerPersona) -> String {
    let mut lines = vec![
        format!("Persona: {}", persona.display_name),
        format!("persona_id: {}", persona.persona_id),
        format!("gender_code: {}", persona.gender_code),
        format!("pronouns: {}", persona.pronouns),
        format!("description: {}", persona.description),
        format!("is_builtin: {}", persona.is_builtin),
    ];
    if let Some(appearance) = persona.appearance.as_deref().and_then(nonempty_str) {
        lines.push(format!("appearance: {appearance}"));
    }
    if let Some(notes) = persona.notes.as_deref().and_then(nonempty_str) {
        lines.push(format!("notes: {notes}"));
    }
    lines.join("\n")
}

fn nonempty_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn render_state_show_response(body: &str, state: &CommandTurnState) -> String {
    let target = state_show_target(body);
    let scene = &state.session_world.scene_state;
    let mut lines = vec![format!("State show{}.", target)];
    lines.push(format!("Turn: {}", state.soul.turn_counter));
    if !scene.current_scene.trim().is_empty() {
        lines.push(format!("Scene: {}", scene.current_scene.trim()));
    }
    if !scene.focus.trim().is_empty() {
        lines.push(format!("Focus: {}", scene.focus.trim()));
    }
    if !scene.pressure_point.trim().is_empty() {
        lines.push(format!("Pressure: {}", scene.pressure_point.trim()));
    }
    lines.push(format!(
        "Recent events: {}. Memories: {}. Objects: {}.",
        state.session_world.recent_events.len(),
        state.soul.memory.recent.len(),
        state.session_world.object_states.len()
    ));
    if let Some(relationship) = state
        .soul
        .relationships
        .get("user")
        .or_else(|| state.soul.relationships.get("default_player"))
    {
        lines.push(format!(
            "User relationship: trust {:.0}, comfort {:.0}, curiosity {:.0}, fear {:.0}, boundary pressure {:.0}.",
            relationship.trust,
            relationship.comfort,
            relationship.curiosity,
            relationship.fear,
            relationship.boundary_pressure
        ));
    }
    if let Some(warning) = state.evaluator_freshness_warning.as_deref() {
        lines.push(warning.to_string());
    }
    lines.join("\n")
}

fn state_show_target(body: &str) -> String {
    let mut parts = body.split_whitespace();
    let first = parts.next();
    let rest = if first.is_some_and(|part| part.eq_ignore_ascii_case("show")) {
        parts.collect::<Vec<_>>().join(" ")
    } else {
        body.trim().to_string()
    };
    if rest.trim().is_empty() {
        String::new()
    } else {
        format!(" for {}", rest.trim())
    }
}

fn render_state_review_response(
    conn: &Connection,
    conversation_id: &str,
) -> Result<String, String> {
    let pending = db::get_pending_setup(conn, conversation_id).map_err(|err| err.to_string())?;
    let pending_line = pending
        .as_deref()
        .map(|text| format!("Pending setup: {text}"))
        .unwrap_or_else(|| "Pending setup: none".into());
    Ok(format!(
        "State review.\n{pending_line}\nRecent command changes are recorded in the payload trace ledger."
    ))
}

fn render_mutating_command_response(
    parsed: &ParsedChatCommand,
    source: &str,
    outcome: Option<&CommandPatchOutcome>,
    llm_response: Option<&str>,
) -> String {
    let Some(outcome) = outcome else {
        return "No state patch was applied.".into();
    };
    let base = match parsed.kind {
        ChatCommandKind::Ask => format!(
            "Ask edit applied. Source: {source}. Patch ID: {}. Ledger turn: {}.",
            outcome.patch_id, outcome.turn_id
        ),
        _ => format!(
            "State update applied. Source: user_state_command. Patch ID: {}. Ledger turn: {}.",
            outcome.patch_id, outcome.turn_id
        ),
    };
    if let Some(llm_res) = llm_response {
        format!("{llm_res}\n\n{base}")
    } else {
        base
    }
}

fn render_ask_proposal_response(mode: AskMode, instruction: &str, patch: &EnginePatch) -> String {
    let mode_label = match mode {
        AskMode::Plan => "plan",
        AskMode::Diff => "diff",
        AskMode::Apply | AskMode::Auto => "proposal",
    };
    format!(
        "Ask {mode_label} only. No state was changed.\nRequest: {}\nProposed safe edit: {}",
        instruction.trim(),
        command_patch_summary_json(patch)
    )
}

fn parse_state_update_body(body: &str) -> Option<(String, String)> {
    let mut parts = body.split_whitespace();
    let first = parts.next()?;
    if !first.eq_ignore_ascii_case("update") {
        return None;
    }
    let target = parts.next()?.trim().to_string();
    let instruction = parts.collect::<Vec<_>>().join(" ");
    (!target.is_empty() && !instruction.trim().is_empty())
        .then_some((target, instruction.trim().to_string()))
}

/// Build the patch for a user-typed `/state update <target> <instruction>`.
///
/// The target names a continuity slot, so the user says exactly which piece of
/// state is wrong instead of leaving the engine to guess from prose. A user
/// command is a trusted source: unlike an evaluator claim it needs no evidence
/// quote, because the person typing it *is* the evidence.
///
/// `knows`/`suspects`/`believes`/`unaware`/`hiding` targets take
/// `<holder> : <proposition>` so one command can fix who knows what.
fn scene_state_command_patch(target: &str, instruction: &str, source_label: &str) -> EnginePatch {
    let instruction = instruction.trim();
    let note = format!("{source_label}: {instruction}");

    if let Some(status) = knowledge_status_for_command_target(target) {
        let (holder, proposition) = match instruction.split_once(':') {
            Some((holder, proposition)) if !holder.trim().is_empty() => {
                (holder.trim().to_string(), proposition.trim().to_string())
            }
            _ => (String::new(), instruction.to_string()),
        };
        if !holder.is_empty() && !proposition.is_empty() {
            return EnginePatch {
                schema_version: Some(state_engine::patch::PATCH_PROTOCOL_VERSION),
                world_patch: Some(WorldPatch {
                    knowledge_operations: vec![KnowledgeOperationPatch {
                        operation: "record".into(),
                        holder_entity_id: Some(holder),
                        proposition: Some(proposition),
                        status: Some(status.into()),
                        ..KnowledgeOperationPatch::default()
                    }],
                    ..WorldPatch::default()
                }),
                ..EnginePatch::default()
            };
        }
    }

    let mut scene = SceneStatePatch {
        scene_state_id: Some(format!("scene_cmd_{}", uuid_like_id())),
        continuity_note: Some(note),
        ..SceneStatePatch::default()
    };
    let value = instruction.chars().take(240).collect::<String>();
    match SceneSlot::from_predicate(target) {
        Some(SceneSlot::Location) => {
            return EnginePatch {
                schema_version: Some(state_engine::patch::PATCH_PROTOCOL_VERSION),
                world_patch: Some(WorldPatch {
                    location: Some(value),
                    scene_state: Some(scene),
                    ..WorldPatch::default()
                }),
                ..EnginePatch::default()
            };
        }
        Some(SceneSlot::CurrentScene) => scene.current_scene = Some(value),
        Some(SceneSlot::Focus) => scene.focus = Some(value),
        Some(SceneSlot::Position) => scene.positions = vec![value],
        Some(SceneSlot::Outfit) => scene.outfits = vec![value],
        Some(SceneSlot::RoomState) => scene.room_state = Some(value),
        Some(SceneSlot::ActiveObject) => scene.active_object = Some(value),
        Some(SceneSlot::Misunderstanding) => scene.current_misunderstanding = Some(value),
        Some(SceneSlot::OpenQuestion) => scene.open_question = Some(value),
        Some(SceneSlot::PressurePoint) => scene.pressure_point = Some(value),
        Some(SceneSlot::LastAction) => scene.last_user_action = Some(value),
        // An unrecognized target is still a real user correction, so it is kept
        // as focus rather than dropped — but it is never filed into a named slot
        // it might not belong to.
        None => scene.focus = Some(value),
    }

    EnginePatch {
        schema_version: Some(state_engine::patch::PATCH_PROTOCOL_VERSION),
        world_patch: Some(WorldPatch {
            scene_state: Some(scene),
            ..WorldPatch::default()
        }),
        ..EnginePatch::default()
    }
}

fn knowledge_status_for_command_target(target: &str) -> Option<&'static str> {
    match target.trim().to_ascii_lowercase().as_str() {
        "knows" | "know" => Some("knows"),
        "suspects" | "suspect" => Some("suspects"),
        "believes" | "believes_false" | "wrong" => Some("believes_false"),
        "unaware" => Some("unaware"),
        "hiding" | "hides" => Some("hiding"),
        _ => None,
    }
}

fn hard_delete_or_external_write_reason(instruction: &str) -> Option<&'static str> {
    let lower = instruction.to_ascii_lowercase();
    if [
        "hard delete",
        "delete all",
        "wipe",
        "erase all",
        "drop table",
        "remove all memories",
        "forget everything",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return Some("hard deletes are not allowed from slash commands");
    }
    if [
        "../",
        "..\\",
        "c:\\",
        "/users/",
        "filesystem",
        "outside sandbox",
        "write file",
        "run code",
        "shell command",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return Some("external writes or code execution are outside the Soul/state sandbox");
    }
    None
}

fn is_high_risk_soul_edit(instruction: &str) -> bool {
    let lower = instruction.to_ascii_lowercase();
    [
        "core identity",
        "identity",
        "personality",
        "backstory",
        "permanent",
        "always",
        "never",
        "replace aurora",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn command_patch_summary_json(patch: &EnginePatch) -> serde_json::Value {
    serde_json::json!({
        "patch_empty": patch.is_empty(),
        "summary": engine_patch_summary(patch),
        "scene_state": patch
            .world_patch
            .as_ref()
            .and_then(|world| world.scene_state.as_ref())
    })
}

fn apply_pending_setup_to_turn(
    mut context_preview: ContextPreview,
    user_text: String,
    pending_setup: Option<&str>,
) -> (ContextPreview, String) {
    let Some(setup) = pending_setup
        .map(str::trim)
        .filter(|setup| !setup.is_empty())
    else {
        return (context_preview, user_text);
    };
    let setup_block = format!("[PENDING SETUP, HIGH PRIORITY]\n{setup}");
    context_preview.text = format!("{setup_block}\n\n{}", context_preview.text);
    context_preview.estimated_tokens = estimate_tokens(&context_preview.text);
    context_preview.truncated = true;
    let user_text = format!(
        "{setup_block}\n\n[LATEST USER MESSAGE]\n{}",
        user_text.trim()
    );
    (context_preview, user_text)
}

fn take_pending_setup_for_normal_turn(
    conn: &Connection,
    conversation_id: &str,
    user_text: &str,
    replacement_assistant_id: Option<i64>,
) -> Result<Option<String>, String> {
    if replacement_assistant_id.is_some() || is_ooc_or_gm_prefix(user_text) {
        return Ok(None);
    }
    let pending = db::get_pending_setup(conn, conversation_id).map_err(|err| err.to_string())?;
    if pending.is_some() {
        db::clear_pending_setup(conn, conversation_id).map_err(|err| err.to_string())?;
    }
    Ok(pending)
}

fn reuse_or_insert_user_message(
    conn: &Connection,
    conversation_id: &str,
    user_text: &str,
) -> Result<i64, String> {
    if let Some(message_id) =
        db::find_reusable_active_user_message(conn, conversation_id, user_text)
            .map_err(|err| err.to_string())?
    {
        return Ok(message_id);
    }
    db::insert_message_and_get_id(conn, conversation_id, "user", user_text)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn send_api_turn(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    narrator_settings: ApiProviderSettings,
    state_updater_settings: ApiProviderSettings,
    replacement_assistant_id: Option<i64>,
    correction_instruction: Option<String>,
    context_mode: Option<String>,
) -> Result<TurnResult, String> {
    let turn_started = Instant::now();
    let mut stage_started = Instant::now();
    let context_mode = ContextMode::from_label(context_mode.as_deref());
    let request_id = uuid_like_id();
    let canonical_turn_id = format!("turn_{request_id}");
    let mut pipeline_trace = TurnPipelineTrace::new(
        request_id.clone(),
        Some(canonical_turn_id.clone()),
        conversation_id.clone(),
        db::now_ts(),
    );
    let command_llm_result = maybe_call_api_command_llm(
        &state,
        &conversation_id,
        &soul_id,
        &user_text,
        &narrator_settings,
    )
    .await?;
    if let Some(command_result) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        maybe_handle_chat_command_with_conn(
            Some(&window),
            &conn,
            conversation_id.clone(),
            soul_id.clone(),
            user_text.clone(),
            &request_id,
            &canonical_turn_id,
            context_mode,
            command_llm_result,
        )?
    } {
        return Ok(command_result);
    }
    let gate_outcome =
        gate_pending_evaluator_jobs(&window, &state, &conversation_id, &state_updater_settings)?;
    let mut turn_trace = NarratorTurnTrace {
        request_id: request_id.clone(),
        conversation_id: conversation_id.clone(),
        branch_id: None,
        turn_id: Some(canonical_turn_id.clone()),
        user_message_id: None,
        assistant_message_id: None,
        state_patch_id: None,
        provider_request_id: None,
        provider_response_id: None,
    };
    emit_dev_log(
        &window,
        "info",
        "app",
        "User message submitted",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "request_id": request_id.as_str(),
            "context_mode": context_mode.label(),
            "mode": mode.as_str(),
            "replacement_assistant_id": replacement_assistant_id,
            "user_message_chars": user_text.chars().count(),
            "next_turn_wait_ms": gate_outcome.waited_ms,
            "stale_state_send": gate_outcome.stale_state_send,
            "compiled_with_pending_evaluator": gate_outcome.compiled_with_pending_evaluator
        })),
    );
    let (
        mut soul,
        mut session_world,
        context_messages,
        mut context_preview,
        snapshot_user_text,
        pending_setup_text,
        pre_turn_soul_json,
        entity_context,
        replay_sources,
        ledger_branch_id,
        ledger_parent_turn_id,
        ledger_user_message_id,
    ) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let (fallback_soul, snapshot_user_text) = if let Some(message_id) = replacement_assistant_id
        {
            let snapshot = db::get_turn_snapshot(&conn, &conversation_id, message_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| "No turn snapshot found for assistant response".to_string())?;
            let soul: Soul =
                serde_json::from_str(&snapshot.soul_json).map_err(|err| err.to_string())?;
            (soul, snapshot.user_text)
        } else {
            (
                db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?,
                user_text.clone(),
            )
        };
        db::ensure_conversation(&conn, &conversation_id, &fallback_soul.character_id)
            .map_err(|err| err.to_string())?;
        let ledger_branch = db::get_active_session_branch(&conn, &conversation_id).ok();
        let old_commit = replacement_assistant_id.and_then(|message_id| {
            db::get_turn_commit_by_assistant(&conn, &conversation_id, message_id)
                .ok()
                .flatten()
        });
        let (soul, session_world, ledger_parent_turn_id) = if let Some(branch) =
            ledger_branch.as_ref()
        {
            let parent_turn_id = if replacement_assistant_id.is_some() {
                old_commit
                    .as_ref()
                    .and_then(|commit| commit.parent_turn_id.clone())
            } else {
                branch.active_turn_id.clone()
            };
            let rebuilt = db::rebuild_session_state_until(
                &conn,
                &conversation_id,
                &branch.branch_id,
                parent_turn_id.as_deref(),
            )
            .map_err(|err| err.to_string())?;
            let mut session_world = rebuilt.session_world;
            purge_premature_recent_events_from_session_world(
                &mut session_world,
                snapshot_user_text.as_str(),
            );
            (rebuilt.soul, session_world, parent_turn_id)
        } else {
            let mut session_world =
                load_session_world_for_context(&window, &conn, &conversation_id, &fallback_soul)
                    .map_err(|err| err.to_string())?;
            purge_premature_recent_events_from_session_world(
                &mut session_world,
                snapshot_user_text.as_str(),
            );
            (fallback_soul, session_world, None)
        };
        let user_message_started = Instant::now();
        let ledger_user_message_id = if replacement_assistant_id.is_none() {
            let existing_id =
                db::find_reusable_active_user_message(&conn, &conversation_id, &user_text)
                    .map_err(|err| err.to_string())?;
            let id = if let Some(id) = existing_id {
                emit_dev_log(
                    &window,
                    "info",
                    "db",
                    "duplicate_user_message_prevented",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "reused_canonical_user_message_id": id,
                        "request_id": request_id.as_str()
                    })),
                );
                emit_dev_log(
                    &window,
                    "info",
                    "db",
                    "reused_canonical_user_message_id",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "user_message_id": id,
                        "request_id": request_id.as_str()
                    })),
                );
                id
            } else {
                db::insert_message_and_get_id(&conn, &conversation_id, "user", &user_text)
                    .map_err(|err| err.to_string())?
            };
            emit_perf_log(
                &window,
                &conversation_id,
                "save user message",
                stage_started.elapsed(),
            );
            stage_started = Instant::now();
            pipeline_trace.record_stage(
                "user_message_saved",
                "success",
                user_message_started.elapsed().as_millis() as u64,
                None,
                Some(format!("Message ID: {}", id)),
            );
            Some(id)
        } else {
            pipeline_trace.record_stage(
                "user_message_saved",
                "skipped",
                0,
                None,
                Some("Bypassed because this is a regenerated/replacement variant".to_string()),
            );
            old_commit
                .as_ref()
                .and_then(|commit| commit.user_message_id)
        };
        let context_started = Instant::now();
        let entity_context =
            resolve_speaker_for_turn(&conn, &conversation_id, &soul, &snapshot_user_text)
                .map_err(|err| err.to_string())?;

        let history_limit = if context_mode == ContextMode::FullChat {
            100
        } else {
            5
        };
        let before_messages = match replacement_assistant_id {
            Some(message_id) => {
                db::list_messages_before_id(&conn, &conversation_id, message_id, history_limit)
            }
            None => db::list_messages(&conn, &conversation_id, history_limit),
        }
        .map_err(|err| err.to_string())?;
        let mut replay_sources = recent_assistant_replay_sources(&before_messages, 3);
        if let Some(message_id) = replacement_assistant_id {
            if let Ok(message) = db::get_message(&conn, &conversation_id, message_id) {
                if message.role == "assistant" && message.channel == db::MESSAGE_CHANNEL_RP_SCENE {
                    replay_sources.insert(
                        0,
                        ReplaySource {
                            message_id: message.id,
                            content: message.content,
                        },
                    );
                    replay_sources.truncate(3);
                }
            }
        }
        let context_messages = messages_to_context(before_messages);
        let pending_setup_text = take_pending_setup_for_normal_turn(
            &conn,
            &conversation_id,
            &snapshot_user_text,
            replacement_assistant_id,
        )?;
        let active_persona = db::get_active_player_persona(&conn, &conversation_id)
            .map_err(|err| err.to_string())?;
        let active_persona_context = player_persona_context(&active_persona);
        let mut context_preview = compile_context_with_correction(
            &soul,
            Some(&session_world),
            &context_messages,
            correction_instruction.as_deref(),
            Some(snapshot_user_text.as_str()),
            Some(&active_persona_context),
            narrator_settings.context_max_tokens,
        );
        if let Some(branch) = ledger_branch.as_ref() {
            append_memory_v2_evidence_bundle(
                &conn,
                &conversation_id,
                &branch.branch_id,
                &snapshot_user_text,
                &mut context_preview,
            );
        }
        turn_trace.user_message_id = ledger_user_message_id;
        turn_trace.branch_id = ledger_branch
            .as_ref()
            .map(|branch| branch.branch_id.clone());
        emit_perf_log(
            &window,
            &conversation_id,
            "compile narrator context",
            stage_started.elapsed(),
        );
        if context_mode == ContextMode::Brief
            && context_preview.estimated_tokens > NARRATOR_BRIEF_TARGET_TOKENS
        {
            emit_dev_log(
                &window,
                "warn",
                "performance",
                "narrator payload exceeds brief budget",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "estimated_tokens": context_preview.estimated_tokens,
                    "target_tokens": NARRATOR_BRIEF_TARGET_TOKENS
                })),
            );
        }
        let pre_turn_soul_json = serde_json::to_string(&soul).map_err(|err| err.to_string())?;
        pipeline_trace.record_stage(
            "context_compiled",
            "success",
            context_started.elapsed().as_millis() as u64,
            Some(format!("Context mode: {}", context_mode.label())),
            Some(format!(
                "Estimated tokens: {}",
                context_preview.estimated_tokens
            )),
        );
        (
            soul,
            session_world,
            context_messages,
            context_preview,
            snapshot_user_text,
            pending_setup_text,
            pre_turn_soul_json,
            entity_context,
            replay_sources,
            ledger_branch.map(|branch| branch.branch_id),
            ledger_parent_turn_id,
            ledger_user_message_id,
        )
    };
    emit_entity_resolution_log(&window, &conversation_id, &entity_context.speaker);
    emit_possible_world_character_mismatch(&window, &conversation_id, &soul, Some(&session_world));
    let mut effective_user_text =
        build_user_text_with_correction(&snapshot_user_text, correction_instruction.as_deref());
    let (updated_context_preview, updated_effective_user_text) = apply_pending_setup_to_turn(
        context_preview,
        effective_user_text,
        pending_setup_text.as_deref(),
    );
    context_preview = updated_context_preview;
    effective_user_text = updated_effective_user_text;
    emit_dev_log(
        &window,
        "info",
        "context",
        "narrator_context_compiled",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "request_id": request_id.as_str(),
            "branch_id": turn_trace.branch_id.as_deref(),
            "context_mode": context_mode.label(),
            "context_tokens": context_preview.estimated_tokens,
            "context_truncated": context_preview.truncated,
            // Sections are trimmed from the end, so a guard added last is the
            // first thing to vanish. Naming what was dropped is the difference
            // between a diagnosable prompt and a silently weakened one.
            "context_truncated_sections": context_preview
                .truncated_sections
                .iter()
                .map(|section| format!("{} (-{} lines)", section.header, section.lines_dropped))
                .collect::<Vec<_>>(),
            "history_messages": context_messages.len()
        })),
    );

    let narrator_payload = prepare_narrator_payload(
        &narrator_settings,
        &soul,
        Some(&session_world),
        &context_messages,
        &context_preview,
        &effective_user_text,
        &mode,
        context_mode,
    );
    let provider = ApiProvider::default();
    let stream_conversation_id = conversation_id.clone();
    let narrator_token_estimate =
        estimate_tokens(&serialize_api_messages(&narrator_payload.messages));
    emit_dev_log(
        &window,
        "info",
        "narrator",
        "Narrator call started",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "provider": format!("narrator_{}", context_mode.label()),
            "model": narrator_settings.model.trim(),
            "base_url": narrator_settings.base_url.trim(),
            "context_mode": context_mode.label(),
            "estimated_total_tokens": narrator_token_estimate,
            "truncated": narrator_payload.truncated
        })),
    );
    let mut payload_log_id = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::insert_llm_payload_log(
            &conn,
            &LlmPayloadLog {
                id: 0,
                conversation_id: conversation_id.clone(),
                message_id: replacement_assistant_id,
                provider: format!("narrator_{}", context_mode.label()),
                mode: mode.trim().to_string(),
                context_mode: context_mode.label().into(),
                model: narrator_settings.model.trim().to_string(),
                base_url: narrator_settings.base_url.trim().to_string(),
                system_message: narrator_payload
                    .messages
                    .first()
                    .map(|message| message.content.clone())
                    .unwrap_or_default(),
                user_message: narrator_payload.user_message.clone(),
                context_text: narrator_payload.context_text.clone(),
                estimated_system_tokens: estimate_tokens(
                    narrator_payload
                        .messages
                        .first()
                        .map(|message| message.content.as_str())
                        .unwrap_or_default(),
                ),
                estimated_user_tokens: estimate_tokens(&effective_user_text),
                estimated_total_tokens: estimate_tokens(&serialize_api_messages(
                    &narrator_payload.messages,
                )),
                truncated: narrator_payload.truncated,
                created_at: db::now_ts(),
                branch_id: ledger_branch_id.clone(),
                active_turn_id: ledger_parent_turn_id.clone(),
                parent_turn_id: ledger_parent_turn_id.clone(),
                state_patch_ids_applied: Vec::new(),
                discarded_patch_ids_skipped: Vec::new(),
                state_rebuild_generation: None,
                latest_assistant_variant_id: None,
                request_id: Some(request_id.clone()),
                turn_id: turn_trace.turn_id.clone(),
                raw_provider_response: None,
                normalized_response: None,
                finish_reason: None,
                provider_error: None,
                fallback_used: false,
                fallback_reason: None,
                provider_request_id: None,
                provider_response_id: None,
                pipeline_trace_json: Some(
                    serde_json::to_string(&serde_json::json!({ "pipeline_trace": pipeline_trace }))
                        .unwrap_or_default(),
                ),
            },
        )
        .map_err(|err| err.to_string())?
    };
    emit_dev_log(
        &window,
        "debug",
        "db",
        "Narrator payload log stored",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "payload_log_id": payload_log_id
        })),
    );
    let stream_chunk_count = Arc::new(AtomicU64::new(0));
    let stream_byte_count = Arc::new(AtomicU64::new(0));
    let stream_chunk_count_for_callback = Arc::clone(&stream_chunk_count);
    let stream_byte_count_for_callback = Arc::clone(&stream_byte_count);
    emit_dev_log(
        &window,
        "info",
        "stream",
        "Narrator streaming started",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str()
        })),
    );
    emit_dev_log(
        &window,
        "info",
        "narrator",
        "narrator_provider_started",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "request_id": request_id.as_str(),
            "payload_log_id": payload_log_id
        })),
    );
    let narrator_called_timer = Instant::now();
    let narrator_call_started = Instant::now();
    // A stream that carried nothing can be asked for again. A stream that
    // carried something cannot: those bytes are already on the reader's screen,
    // and a second attempt would write the turn over the top of itself. The
    // chunk counter is what separates the two, so it — not the error text
    // alone — is what gates the retry.
    //
    // Only the empty stream retries, and only immediately. The other transient
    // classes the benchmark paths retry (429, 5xx) want a backoff, and a silent
    // multi-second pause in front of a reader waiting on prose is its own
    // failure; those keep surfacing as they do now.
    let mut narrator_attempt = 0usize;
    let mut narrator_retry_count = 0usize;
    let provider_completion = loop {
        narrator_attempt += 1;
        match provider
            .complete_streaming_messages(
                &narrator_settings,
                narrator_payload.messages.clone(),
                |chunk| {
                    stream_chunk_count_for_callback.fetch_add(1, Ordering::Relaxed);
                    stream_byte_count_for_callback
                        .fetch_add(chunk.as_bytes().len() as u64, Ordering::Relaxed);
                    window
                        .emit(
                            "api-chunk",
                            StreamChunk {
                                conversation_id: stream_conversation_id.clone(),
                                chunk: chunk.to_string(),
                            },
                        )
                        .map_err(|err| err.to_string())
                },
            )
            .await
        {
            Ok(completion) => {
                emit_perf_log(
                    &window,
                    &conversation_id,
                    "narrator API call",
                    narrator_call_started.elapsed(),
                );
                turn_trace.provider_response_id = completion.provider_response_id.clone();
                turn_trace.provider_request_id = completion.provider_request_id.clone();
                emit_dev_log(
                    &window,
                    "success",
                    "stream",
                    "Narrator streaming finished",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "request_id": request_id.as_str(),
                        "chunks": stream_chunk_count.load(Ordering::Relaxed),
                        "bytes": stream_byte_count.load(Ordering::Relaxed),
                        "provider_response_id": completion.provider_response_id.as_deref()
                    })),
                );
                emit_dev_log(
                    &window,
                    "success",
                    "narrator",
                    "narrator_provider_finished",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "request_id": request_id.as_str(),
                        "finish_reason": completion.finish_reason.as_deref(),
                        "provider_response_id": completion.provider_response_id.as_deref()
                    })),
                );
                break completion;
            }
            Err(err) => {
                let stream_reached_the_reader = stream_chunk_count.load(Ordering::Relaxed) > 0;
                if narrator_attempt < NARRATOR_MAX_ATTEMPTS
                    && !stream_reached_the_reader
                    && err
                        .to_ascii_lowercase()
                        .contains("api stream did not include assistant content")
                {
                    narrator_retry_count += 1;
                    emit_dev_log(
                        &window,
                        "warn",
                        "narrator",
                        "narrator_empty_stream_retrying",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "request_id": request_id.as_str(),
                            "attempt": narrator_attempt,
                            "error": err.as_str()
                        })),
                    );
                    continue;
                }
                pipeline_trace.record_stage_error(
                    "narrator_called",
                    narrator_called_timer.elapsed().as_millis() as u64,
                    PipelineErrorCode::NarratorCallError,
                    err.clone(),
                    Some("Check LLM provider settings or availability".to_string()),
                );
                window.emit("pipeline-trace-updated", &pipeline_trace).ok();
                let _ = state
                    .conn
                    .lock()
                    .map_err(|err| err.to_string())
                    .and_then(|conn| {
                        let _ = update_llm_payload_pipeline_trace(
                            &conn,
                            payload_log_id,
                            &serde_json::json!({
                                "pipeline_trace": pipeline_trace,
                                "narrator_trace": {
                                    "request_id": request_id.as_str(),
                                    "turn_id": turn_trace.turn_id.as_deref(),
                                    "conversation_id": conversation_id.as_str(),
                                    "provider": format!("narrator_{}", context_mode.label()),
                                    "model": narrator_settings.model.trim(),
                                    "fallback_used": false,
                                    "fallback_reason": serde_json::Value::Null,
                                    "narrator_retry_count": narrator_retry_count,
                                    "narrator_retry_succeeded": false,
                                    "narrator_provider_error": err.as_str()
                                }
                            }),
                        );
                        db::update_llm_payload_log_response(
                            &conn,
                            payload_log_id,
                            &db::LlmPayloadResponseUpdate {
                                provider_error: Some(err.clone()),
                                ..Default::default()
                            },
                        )
                        .map_err(|err| err.to_string())
                    });
                emit_dev_log(
                    &window,
                    "error",
                    "narrator",
                    "narrator_provider_failed",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "request_id": request_id.as_str(),
                        "error": err.clone()
                    })),
                );
                if err
                    .to_ascii_lowercase()
                    .contains("did not include assistant content")
                {
                    emit_dev_log(
                        &window,
                        "error",
                        "narrator",
                        "narrator_empty_stream",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "request_id": request_id.as_str(),
                            "error": err.clone()
                        })),
                    );
                }
                return Err(err);
            }
        }
    };
    let mut active_provider_completion = provider_completion;
    let mut raw_response = active_provider_completion.raw_text.clone();
    let mut parsed = match parse_hidden_state(&raw_response) {
        Ok(parsed) => parsed,
        Err(err) => {
            pipeline_trace.record_stage_error(
                "narrator_called",
                narrator_called_timer.elapsed().as_millis() as u64,
                PipelineErrorCode::NarratorParseError,
                err.to_string(),
                Some("Narrator failed to output valid hidden state tags".to_string()),
            );
            window.emit("pipeline-trace-updated", &pipeline_trace).ok();
            if let Ok(conn) = state.conn.lock() {
                let _ = update_llm_payload_pipeline_trace(
                    &conn,
                    payload_log_id,
                    &serde_json::json!({ "pipeline_trace": pipeline_trace }),
                );
            }
            emit_dev_log(
                &window,
                "error",
                "narrator",
                "Narrator response parse failed",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "error": err.to_string()
                })),
            );
            return Err(err.to_string());
        }
    };

    let without_hidden = strip_hidden_state_blocks(&parsed.visible_text);
    let (without_engine_patch, _) = strip_engine_patch_payloads(&without_hidden);
    let (body, _, _) = remove_status_blocks(&without_engine_patch);

    if is_body_only_markers(&body) {
        emit_dev_log(
            &window,
            "warning",
            "narrator",
            "Narrator body consists only of status or assistant markers. Retrying narrator generation once.",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "bad_body": body.trim()
            })),
        );

        stream_chunk_count.store(0, Ordering::Relaxed);
        stream_byte_count.store(0, Ordering::Relaxed);

        let retry_started = Instant::now();
        match provider
            .complete_streaming_messages(
                &narrator_settings,
                narrator_payload.messages.clone(),
                |chunk| {
                    stream_chunk_count_for_callback.fetch_add(1, Ordering::Relaxed);
                    stream_byte_count_for_callback
                        .fetch_add(chunk.as_bytes().len() as u64, Ordering::Relaxed);
                    window
                        .emit(
                            "api-chunk",
                            StreamChunk {
                                conversation_id: stream_conversation_id.clone(),
                                chunk: chunk.to_string(),
                            },
                        )
                        .map_err(|err| err.to_string())
                },
            )
            .await
        {
            Ok(new_completion) => {
                emit_perf_log(
                    &window,
                    &conversation_id,
                    "narrator API retry call",
                    retry_started.elapsed(),
                );
                active_provider_completion = new_completion;
                raw_response = active_provider_completion.raw_text.clone();
                parsed = match parse_hidden_state(&raw_response) {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        pipeline_trace.record_stage_error(
                            "narrator_called",
                            narrator_called_timer.elapsed().as_millis() as u64,
                            PipelineErrorCode::NarratorParseError,
                            format!("Narrator retry response parse failed: {err}"),
                            Some(
                                "Narrator failed to output valid hidden state tags on retry"
                                    .to_string(),
                            ),
                        );
                        window.emit("pipeline-trace-updated", &pipeline_trace).ok();
                        if let Ok(conn) = state.conn.lock() {
                            let _ = update_llm_payload_pipeline_trace(
                                &conn,
                                payload_log_id,
                                &serde_json::json!({ "pipeline_trace": pipeline_trace }),
                            );
                        }
                        return Err(format!("Narrator retry response parse failed: {err}"));
                    }
                };

                let without_hidden_retry = strip_hidden_state_blocks(&parsed.visible_text);
                let (without_engine_patch_retry, _) =
                    strip_engine_patch_payloads(&without_hidden_retry);
                let (body_retry, _, _) = remove_status_blocks(&without_engine_patch_retry);

                if is_body_only_markers(&body_retry) {
                    pipeline_trace.record_stage_error(
                        "narrator_called",
                        narrator_called_timer.elapsed().as_millis() as u64,
                        PipelineErrorCode::NarratorParseError,
                        "bad narrator output, regenerate on retry".to_string(),
                        Some("Narrator returned empty body on retry".to_string()),
                    );
                    window.emit("pipeline-trace-updated", &pipeline_trace).ok();
                    if let Ok(conn) = state.conn.lock() {
                        let _ = update_llm_payload_pipeline_trace(
                            &conn,
                            payload_log_id,
                            &serde_json::json!({ "pipeline_trace": pipeline_trace }),
                        );
                    }
                    return Err("bad narrator output, regenerate".to_string());
                }
            }
            Err(err) => {
                pipeline_trace.record_stage_error(
                    "narrator_called",
                    narrator_called_timer.elapsed().as_millis() as u64,
                    PipelineErrorCode::NarratorCallError,
                    format!("Narrator retry failed: {err}"),
                    Some("Check LLM provider settings".to_string()),
                );
                window.emit("pipeline-trace-updated", &pipeline_trace).ok();
                if let Ok(conn) = state.conn.lock() {
                    let _ = update_llm_payload_pipeline_trace(
                        &conn,
                        payload_log_id,
                        &serde_json::json!({ "pipeline_trace": pipeline_trace }),
                    );
                }
                return Err(format!("Narrator retry failed: {err}"));
            }
        }
    }
    let guard_timer = Instant::now();
    let mut retry_response_score = 0.0f32;
    let mut selected_response_source = "original".to_string();
    let mut ooc_detection_reason = "scene_turn".to_string();

    let user_is_ooc = is_ooc_or_gm_prefix(&snapshot_user_text);
    let assistant_is_ooc = is_ooc_or_gm_prefix(&parsed.visible_text);
    let pure_ooc_detected =
        user_is_ooc || (snapshot_user_text.trim().is_empty() && assistant_is_ooc);
    if pure_ooc_detected {
        ooc_detection_reason = if user_is_ooc {
            "user_message_ooc_prefix".to_string()
        } else {
            "assistant_ooc_prefix".to_string()
        };
    }

    emit_dev_log(
        &window,
        "debug",
        "narrator",
        "anti_replay_check_started",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "compared_sources": replay_sources.len(),
            "compared_message_ids": replay_sources
                .iter()
                .map(|source| source.message_id)
                .collect::<Vec<_>>()
        })),
    );

    let (mut visible_response, replay_guard, mut output_contract_warning, orig_status_repair) =
        if pure_ooc_detected {
            let (v, mut rg, w, r) = guard_narrator_visible_response(
                &parsed.visible_text,
                &snapshot_user_text,
                &session_world,
                &[],
                &soul.character_name,
            );
            rg.replay_detected = false;
            rg.severity = ReplaySeverity::None;
            (v, rg, w, r)
        } else {
            guard_narrator_visible_response(
                &parsed.visible_text,
                &snapshot_user_text,
                &session_world,
                &replay_sources,
                &soul.character_name,
            )
        };
    let mut status_repair_action = orig_status_repair;

    let phone_guard = sanitize_phone_notification_contradiction(
        &visible_response,
        &snapshot_user_text,
        &session_world,
    );
    if phone_guard.repaired {
        visible_response = phone_guard.text;
        output_contract_warning = append_output_warning(
            output_contract_warning,
            "phone notification contradiction repaired",
        );
    }

    let phone_call_guard =
        sanitize_phone_call_state_violation(&visible_response, &snapshot_user_text, &session_world);
    if phone_call_guard.repaired {
        visible_response = phone_call_guard.text;
        output_contract_warning = append_output_warning(
            output_contract_warning,
            "phone call state violation repaired",
        );
    }

    let original_response_score = evaluate_response_quality(
        &visible_response,
        &snapshot_user_text,
        &session_world,
        if pure_ooc_detected {
            &[]
        } else {
            &replay_sources
        },
    );

    let anti_replay_severity = replay_guard.severity;
    let anti_replay_reason = replay_guard.replay_reason.clone();

    let debug_replay_detected = replay_guard.replay_detected;
    let mut debug_replay_score = replay_guard.replay_score;
    let mut debug_replay_reason = replay_guard.replay_reason.clone();
    let mut debug_replay_compared_against_message_id = replay_guard.compared_against_message_id;
    let anti_replay_retry_enabled = anti_replay_forced_retry_enabled(&narrator_settings);
    let mut anti_replay_retry_suppressed_by_default = false;
    let mut anti_replay_retry_count = 0u8;

    let original_has_violation = has_hard_violation(
        &visible_response,
        &snapshot_user_text,
        &session_world,
        &replay_sources,
    );

    if let Some(warning) = output_contract_warning.as_ref() {
        emit_dev_log(
            &window,
            "warn",
            "narrator",
            "Output contract guard normalized narrator response",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "warning": warning,
                "phone_call_state_violation": has_phone_call_state_violation(&visible_response, &snapshot_user_text, &session_world)
            })),
        );
    }

    let trigger_retry = replay_guard.replay_detected || original_has_violation;

    if trigger_retry {
        emit_dev_log(
            &window,
            "warn",
            "narrator",
            "anti_replay_detected",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "score": replay_guard.replay_score,
                "reason": replay_guard.replay_reason.as_deref(),
                "compared_against_message_id": replay_guard.compared_against_message_id,
                "phone_call_state_violation": has_phone_call_state_violation(&visible_response, &snapshot_user_text, &session_world)
            })),
        );
        if !anti_replay_retry_enabled {
            anti_replay_retry_suppressed_by_default = true;
            emit_dev_log(
                &window,
                "info",
                "narrator",
                "anti_replay_retry_suppressed_by_default",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "score": replay_guard.replay_score,
                    "severity": match replay_guard.severity {
                        ReplaySeverity::None => "none",
                        ReplaySeverity::MildOverlap => "mild_overlap",
                        ReplaySeverity::StrongReplay => "strong_replay",
                        ReplaySeverity::Contradiction => "contradiction",
                        ReplaySeverity::ObjectStateViolation => "object_state_violation",
                    },
                    "reason": replay_guard.replay_reason.as_deref(),
                    "compared_against_message_id": replay_guard.compared_against_message_id,
                    "phone_call_state_violation": has_phone_call_state_violation(&visible_response, &snapshot_user_text, &session_world)
                })),
            );
            debug_replay_reason = replay_guard.replay_reason.clone();
        } else {
            anti_replay_retry_count = 1;
            emit_dev_log(
                &window,
                "info",
                "narrator",
                "anti_replay_regenerate_started",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str()
                })),
            );

            let retry_messages = messages_with_repair_instruction(&narrator_payload.messages);
            let retry_payload_log_id = match state
                .conn
                .lock()
                .map_err(|err| err.to_string())
                .and_then(|conn| {
                    db::insert_llm_payload_log(
                        &conn,
                        &LlmPayloadLog {
                            id: 0,
                            conversation_id: conversation_id.clone(),
                            message_id: replacement_assistant_id,
                            provider: format!(
                                "narrator_{}_anti_replay_retry",
                                context_mode.label()
                            ),
                            mode: mode.trim().to_string(),
                            context_mode: context_mode.label().into(),
                            model: narrator_settings.model.trim().to_string(),
                            base_url: narrator_settings.base_url.trim().to_string(),
                            system_message: retry_messages
                                .first()
                                .map(|message| message.content.clone())
                                .unwrap_or_default(),
                            user_message: last_user_message_content(&retry_messages),
                            context_text: serialize_api_messages(&retry_messages),
                            estimated_system_tokens: estimate_tokens(
                                retry_messages
                                    .first()
                                    .map(|message| message.content.as_str())
                                    .unwrap_or_default(),
                            ),
                            estimated_user_tokens: estimate_tokens(&last_user_message_content(
                                &retry_messages,
                            )),
                            estimated_total_tokens: estimate_tokens(&serialize_api_messages(
                                &retry_messages,
                            )),
                            truncated: narrator_payload.truncated,
                            created_at: db::now_ts(),
                            branch_id: ledger_branch_id.clone(),
                            active_turn_id: ledger_parent_turn_id.clone(),
                            parent_turn_id: ledger_parent_turn_id.clone(),
                            state_patch_ids_applied: Vec::new(),
                            discarded_patch_ids_skipped: Vec::new(),
                            state_rebuild_generation: None,
                            latest_assistant_variant_id: None,
                            request_id: Some(request_id.clone()),
                            turn_id: turn_trace.turn_id.clone(),
                            ..Default::default()
                        },
                    )
                    .map_err(|err| err.to_string())
                }) {
                Ok(log_id) => Some(log_id),
                Err(err) => {
                    emit_dev_log(
                        &window,
                        "warn",
                        "db",
                        "Anti-replay retry payload log failed",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "error": err
                        })),
                    );
                    None
                }
            };

            match provider
                .complete_streaming_messages(&narrator_settings, retry_messages, |_| Ok(()))
                .await
            {
                Ok(retry_completion) => match parse_hidden_state(&retry_completion.raw_text) {
                    Ok(retry_parsed) => {
                        let pruned_retry_visible =
                            prune_repeated_scene_setup(&retry_parsed.visible_text, &replay_sources);
                        let (
                            mut retry_visible_response,
                            retry_guard,
                            mut retry_output_warning,
                            retry_status_repair,
                        ) = guard_narrator_visible_response(
                            &pruned_retry_visible,
                            &snapshot_user_text,
                            &session_world,
                            &replay_sources,
                            &soul.character_name,
                        );
                        let retry_phone_guard = sanitize_phone_notification_contradiction(
                            &retry_visible_response,
                            &snapshot_user_text,
                            &session_world,
                        );
                        if retry_phone_guard.repaired {
                            retry_visible_response = retry_phone_guard.text;
                            retry_output_warning = append_output_warning(
                                retry_output_warning,
                                "phone notification contradiction repaired",
                            );
                        }

                        let retry_phone_call_guard = sanitize_phone_call_state_violation(
                            &retry_visible_response,
                            &snapshot_user_text,
                            &session_world,
                        );
                        if retry_phone_call_guard.repaired {
                            retry_visible_response = retry_phone_call_guard.text;
                            retry_output_warning = append_output_warning(
                                retry_output_warning,
                                "phone call state violation repaired",
                            );
                        }

                        retry_response_score = evaluate_response_quality(
                            &retry_visible_response,
                            &snapshot_user_text,
                            &session_world,
                            &replay_sources,
                        );

                        let original_has_violation = has_hard_violation(
                            &visible_response,
                            &snapshot_user_text,
                            &session_world,
                            &replay_sources,
                        );
                        let retry_has_violation = has_hard_violation(
                            &retry_visible_response,
                            &snapshot_user_text,
                            &session_world,
                            &replay_sources,
                        );

                        let select_retry = if original_has_violation && !retry_has_violation {
                            true
                        } else if !original_has_violation && retry_has_violation {
                            false
                        } else {
                            retry_response_score > original_response_score
                        };

                        if select_retry {
                            visible_response = retry_visible_response;
                            active_provider_completion = retry_completion;
                            if let Some(log_id) = retry_payload_log_id {
                                payload_log_id = log_id;
                            }
                            if let Some(warning) = retry_output_warning.as_ref() {
                                emit_dev_log(
                                    &window,
                                    "warn",
                                    "narrator",
                                    "Output contract guard normalized anti-replay retry",
                                    Some(serde_json::json!({
                                        "conversation_id": conversation_id.as_str(),
                                        "warning": warning
                                    })),
                                );
                            }
                            output_contract_warning = retry_output_warning;
                            status_repair_action = retry_status_repair;
                            selected_response_source = "retry".to_string();

                            if retry_guard.replay_detected {
                                emit_dev_log(
                                    &window,
                                    "warn",
                                    "narrator",
                                    "anti_replay_regenerate_failed",
                                    Some(serde_json::json!({
                                        "conversation_id": conversation_id.as_str(),
                                        "score": retry_guard.replay_score,
                                        "reason": retry_guard.replay_reason.as_deref(),
                                        "compared_against_message_id": retry_guard.compared_against_message_id
                                    })),
                                );
                                emit_dev_log(
                                    &window,
                                    "warn",
                                    "warning",
                                    "anti_replay_final_warning",
                                    Some(serde_json::json!({
                                        "conversation_id": conversation_id.as_str(),
                                        "score": retry_guard.replay_score,
                                        "reason": retry_guard.replay_reason.as_deref()
                                    })),
                                );
                                debug_replay_score = retry_guard.replay_score;
                                debug_replay_reason = retry_guard
                                    .replay_reason
                                    .clone()
                                    .map(|reason| format!("{reason}; saved after one retry"));
                                debug_replay_compared_against_message_id =
                                    retry_guard.compared_against_message_id;
                            } else {
                                emit_dev_log(
                                    &window,
                                    "success",
                                    "narrator",
                                    "anti_replay_passed",
                                    Some(serde_json::json!({
                                        "conversation_id": conversation_id.as_str(),
                                        "score": retry_guard.replay_score,
                                        "retry": true
                                    })),
                                );
                                debug_replay_reason = Some(
                                "Initial draft repeated earlier narration; regenerated before save"
                                    .into(),
                            );
                            }
                        } else {
                            selected_response_source = "original".to_string();
                            emit_dev_log(
                                &window,
                                "warn",
                                "narrator",
                                "anti_replay_regenerate_failed",
                                Some(serde_json::json!({
                                    "conversation_id": conversation_id.as_str(),
                                    "reason": "Retry was lower quality or failed to resolve violation compared to original"
                                })),
                            );
                            emit_dev_log(
                                &window,
                                "warn",
                                "warning",
                                "anti_replay_final_warning",
                                Some(serde_json::json!({
                                    "conversation_id": conversation_id.as_str(),
                                    "score": replay_guard.replay_score,
                                    "reason": "Retry was not selected (lower quality/worse violation)"
                                })),
                            );
                            debug_replay_reason = Some(
                            "Initial draft repeated earlier narration; retry was not selected because it was worse"
                                .into(),
                        );
                        }
                    }
                    Err(err) => {
                        emit_dev_log(
                            &window,
                            "warn",
                            "narrator",
                            "anti_replay_regenerate_failed",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "error": err.to_string()
                            })),
                        );
                        emit_dev_log(
                            &window,
                            "warn",
                            "warning",
                            "anti_replay_final_warning",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "score": replay_guard.replay_score,
                                "reason": "Retry parse failed; saving original guarded response"
                            })),
                        );
                        debug_replay_reason = Some(
                            "Initial draft repeated earlier narration; retry parse failed".into(),
                        );
                    }
                },
                Err(err) => {
                    emit_dev_log(
                        &window,
                        "warn",
                        "narrator",
                        "anti_replay_regenerate_failed",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "error": err
                        })),
                    );
                    emit_dev_log(
                        &window,
                        "warn",
                        "warning",
                        "anti_replay_final_warning",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "score": replay_guard.replay_score,
                            "reason": "Retry provider failed; saving original guarded response"
                        })),
                    );
                    debug_replay_reason = Some(
                        "Initial draft repeated earlier narration; retry provider failed".into(),
                    );
                }
            }
        }
    } else {
        emit_dev_log(
            &window,
            "success",
            "narrator",
            "anti_replay_passed",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "score": replay_guard.replay_score,
                "compared_against_message_id": replay_guard.compared_against_message_id
            })),
        );
    }

    let final_narrator_elapsed = narrator_called_timer.elapsed().as_millis() as u64;
    pipeline_trace.record_stage(
        "narrator_called",
        "success",
        final_narrator_elapsed,
        None,
        Some(format!(
            "Narrator selected response from: {}",
            selected_response_source
        )),
    );

    let mut guard_status = "success";
    if phone_guard.repaired || phone_call_guard.repaired || trigger_retry {
        guard_status = "warning";
    }
    let guard_elapsed = guard_timer.elapsed().as_millis() as u64;
    pipeline_trace.record_stage(
        "narrator_output_guarded",
        guard_status,
        guard_elapsed,
        Some(format!(
            "Selected source: {}, Replay detected: {}",
            selected_response_source, debug_replay_detected
        )),
        Some(
            output_contract_warning
                .clone()
                .unwrap_or_else(|| "No violations detected".to_string()),
        ),
    );
    window.emit("pipeline-trace-updated", &pipeline_trace).ok();

    if visible_response.trim().is_empty() {
        emit_dev_log(
            &window,
            "error",
            "narrator",
            "Narrator provider returned empty visible response",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str()
            })),
        );
        return Err("Narrator provider returned an empty visible response".into());
    }

    let _ = state
        .conn
        .lock()
        .map_err(|err| err.to_string())
        .and_then(|conn| {
            db::update_llm_payload_log_response(
                &conn,
                payload_log_id,
                &llm_payload_response_update_from_completion(
                    &active_provider_completion,
                    &visible_response,
                ),
            )
            .map_err(|err| err.to_string())
        });

    if is_known_mock_template_prose(&visible_response) {
        let _ = state
            .conn
            .lock()
            .map_err(|err| err.to_string())
            .and_then(|conn| {
                db::update_llm_payload_log_response(
                    &conn,
                    payload_log_id,
                    &db::LlmPayloadResponseUpdate {
                        fallback_used: Some(true),
                        fallback_reason: Some("generic_mock_prose_detected".into()),
                        normalized_response: Some(NARRATOR_PROVIDER_ERROR_VISIBLE.into()),
                        ..Default::default()
                    },
                )
                .map_err(|err| err.to_string())
            });
        emit_dev_log(
            &window,
            "error",
            "narrator",
            "narrator_provider_failed",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "request_id": request_id.as_str(),
                "reason": "generic_mock_prose_detected"
            })),
        );
        emit_dev_log(
            &window,
            "warn",
            "narrator",
            "fallback_response_used",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "request_id": request_id.as_str(),
                "fallback_reason": "generic_mock_prose_detected"
            })),
        );
        return Err(
            "Narrator response matched mock-template prose and was rejected for integrity".into(),
        );
    }

    let save_narrator_started = Instant::now();
    let mut integrity_ok = true;
    let pre_save_debug = TurnDebug {
        provider: "API".into(),
        hidden_state_found: false,
        fallback_hidden_state_generated: false,
        narrator_response_saved: true,
        assistant_message_id: None,
        selected_variant_id: None,
        state_updater_status: "pending".into(),
        replay_detected: debug_replay_detected,
        replay_score: debug_replay_score,
        replay_reason: debug_replay_reason.clone(),
        replay_compared_against_message_id: debug_replay_compared_against_message_id,
        output_contract_warning: output_contract_warning.clone(),
        tag: None,
        trust_delta: None,
        affection_delta: None,
        new_location: None,
        present_characters: Vec::new(),
        request_id: Some(request_id.clone()),
        turn_id: turn_trace.turn_id.clone(),
        state_patch_id: None,
        baseline_patch_id: None,
        enrichment_patch_id: None,
        simulated_response: false,
        fallback_used: false,
        fallback_reason: None,
    };
    let (assistant_message_id, selected_variant_id) = match {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        save_visible_narrator_response(
            &conn,
            &conversation_id,
            &visible_response,
            replacement_assistant_id,
            correction_instruction.as_deref(),
            &pre_turn_soul_json,
            &snapshot_user_text,
            payload_log_id,
            NarratorMessageOrigin::Api,
            Some(&pre_save_debug),
        )
    } {
        Ok(val) => val,
        Err(err) => {
            pipeline_trace.record_stage_error(
                "assistant_saved",
                save_narrator_started.elapsed().as_millis() as u64,
                PipelineErrorCode::DatabaseError,
                err.to_string(),
                Some("Check database integrity or connection".to_string()),
            );
            window.emit("pipeline-trace-updated", &pipeline_trace).ok();
            if let Ok(conn) = state.conn.lock() {
                let _ = update_llm_payload_pipeline_trace(
                    &conn,
                    payload_log_id,
                    &serde_json::json!({ "pipeline_trace": pipeline_trace }),
                );
            }
            return Err(err.to_string());
        }
    };
    turn_trace.assistant_message_id = Some(assistant_message_id);
    emit_perf_log(
        &window,
        &conversation_id,
        "save narrator response",
        save_narrator_started.elapsed(),
    );
    pipeline_trace.record_stage(
        "assistant_saved",
        "success",
        save_narrator_started.elapsed().as_millis() as u64,
        None,
        Some(format!(
            "Message ID: {}, Variant ID: {:?}",
            assistant_message_id, selected_variant_id
        )),
    );
    window.emit("pipeline-trace-updated", &pipeline_trace).ok();
    {
        let saved_message = state
            .conn
            .lock()
            .map_err(|err| err.to_string())
            .and_then(|conn| {
                db::get_message(&conn, &conversation_id, assistant_message_id)
                    .map_err(|err| err.to_string())
            });
        match saved_message {
            Ok(message) => {
                if let Err(err) = window.emit(
                    "chat-message-saved",
                    SavedChatMessageEvent {
                        conversation_id: conversation_id.clone(),
                        message,
                    },
                ) {
                    eprintln!("Saved narrator message event failed: {err}");
                }
            }
            Err(err) => eprintln!("Saved narrator message reload failed: {err}"),
        }
    }
    emit_dev_log(
        &window,
        "success",
        "narrator",
        "narrator_response_saved",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "request_id": request_id.as_str(),
            "assistant_message_id": assistant_message_id,
            "selected_variant_id": selected_variant_id,
            "visible_chars": visible_response.chars().count()
        })),
    );

    let mut visible_response_for_updater = visible_response.clone();
    if let Ok(saved_message) = state
        .conn
        .lock()
        .map_err(|err| err.to_string())
        .and_then(|conn| {
            db::get_message(&conn, &conversation_id, assistant_message_id)
                .map_err(|err| err.to_string())
        })
    {
        if !responses_match_for_integrity(&saved_message.content, &visible_response) {
            integrity_ok = false;
            emit_dev_log(
                &window,
                "error",
                "narrator",
                "response_integrity_mismatch",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "request_id": request_id.as_str(),
                    "assistant_message_id": assistant_message_id
                })),
            );
        } else {
            visible_response_for_updater = saved_message.content;
        }
    }

    {
        let reported = active_provider_completion.token_usage;
        let reported_prompt = reported.and_then(|usage| usage.prompt_tokens);
        let reported_completion = reported.and_then(|usage| usage.completion_tokens);
        pipeline_trace.token_usage = Some(TurnTokenUsage {
            narrator_prompt_tokens: Some(reported_prompt.unwrap_or(narrator_token_estimate as u64)),
            narrator_completion_tokens: Some(
                reported_completion.unwrap_or_else(|| {
                    estimate_tokens(&active_provider_completion.raw_text) as u64
                }),
            ),
            narrator_estimated: reported_prompt.is_none() || reported_completion.is_none(),
            ..TurnTokenUsage::default()
        });
    }

    let narrator_pipeline_trace = serde_json::json!({
        "narrator_trace": {
            "request_id": request_id.as_str(),
            "turn_id": turn_trace.turn_id.as_deref(),
            "conversation_id": conversation_id.as_str(),
            "user_message_id": ledger_user_message_id,
            "assistant_message_id": assistant_message_id,
            "assistant_variant_id": selected_variant_id,
            "provider": format!("narrator_{}", context_mode.label()),
            "model": narrator_settings.model.trim(),
            "raw_provider_response": active_provider_completion.raw_text.as_str(),
            "normalized_response": visible_response.as_str(),
            "saved_visible_response": visible_response_for_updater.as_str(),
            "response_integrity_ok": integrity_ok,
            "fallback_used": false,
            "fallback_reason": serde_json::Value::Null,
            "narrator_retry_count": narrator_retry_count,
            "narrator_retry_succeeded": if narrator_retry_count > 0 {
                serde_json::Value::Bool(true)
            } else {
                serde_json::Value::Null
            },
            "narrator_provider_error": serde_json::Value::Null,
            "anti_replay_triggered": debug_replay_detected,
            "anti_replay_retry_count": anti_replay_retry_count,
            "anti_replay_retry_suppressed_by_default": anti_replay_retry_suppressed_by_default,
            "final_selected_attempt_id": selected_variant_id.or(Some(payload_log_id)),
            "provider_request_id": active_provider_completion.provider_request_id.as_deref(),
            "provider_response_id": active_provider_completion.provider_response_id.as_deref(),
            "anti_replay_severity": match anti_replay_severity {
                ReplaySeverity::None => "none",
                ReplaySeverity::MildOverlap => "mild_overlap",
                ReplaySeverity::StrongReplay => "strong_replay",
                ReplaySeverity::Contradiction => "contradiction",
                ReplaySeverity::ObjectStateViolation => "object_state_violation",
            },
            "anti_replay_reason": anti_replay_reason,
            "original_response_score": original_response_score,
            "retry_response_score": retry_response_score,
            "selected_response_source": selected_response_source,
            "status_repair_action": status_repair_action,
            "pure_ooc_detected": pure_ooc_detected,
            "ooc_detection_reason": ooc_detection_reason,
            "next_turn_wait_ms": gate_outcome.waited_ms,
            "stale_state_send": gate_outcome.stale_state_send,
            "compiled_with_pending_evaluator": gate_outcome.compiled_with_pending_evaluator,
            "pending_evaluator_job_ids": gate_outcome.pending_job_ids
        },
        "pipeline_trace": pipeline_trace
    });
    if let Ok(conn) = state.conn.lock() {
        let _ = update_llm_payload_pipeline_trace(&conn, payload_log_id, &narrator_pipeline_trace);
    }

    if !integrity_ok {
        pipeline_trace.record_stage_error(
            "assistant_saved",
            save_narrator_started.elapsed().as_millis() as u64,
            PipelineErrorCode::NarratorParseError,
            "Response integrity mismatch".to_string(),
            Some("Re-generation might be needed".to_string()),
        );
        window.emit("pipeline-trace-updated", &pipeline_trace).ok();
        if let Ok(conn) = state.conn.lock() {
            let _ = update_llm_payload_pipeline_trace(
                &conn,
                payload_log_id,
                &serde_json::json!({ "pipeline_trace": pipeline_trace }),
            );
        }
        let mut failed_debug = pre_save_debug;
        failed_debug.state_updater_status = "integrity_mismatch".into();
        failed_debug.assistant_message_id = Some(assistant_message_id);
        failed_debug.selected_variant_id = selected_variant_id;
        if let Some(variant_id) = selected_variant_id {
            let _ = state
                .conn
                .lock()
                .map_err(|err| err.to_string())
                .and_then(|conn| {
                    db::update_assistant_variant_debug_json(
                        &conn,
                        variant_id,
                        &serde_json::to_string(&failed_debug).map_err(|err| err.to_string())?,
                    )
                    .map_err(|err| err.to_string())
                });
        }
        let messages = {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?
        };
        return Ok(TurnResult {
            conversation_id: conversation_id.clone(),
            soul,
            visible_response: visible_response_for_updater,
            context_preview,
            messages,
            consolidation_ran: false,
            debug: failed_debug,
        });
    }

    let should_bypass_evaluator = pure_ooc_detected
        && !user_text_has_correction_keywords(&snapshot_user_text)
        && correction_instruction.is_none();

    if should_bypass_evaluator {
        pipeline_trace.record_stage(
            "evaluator_job_started",
            "skipped",
            0,
            None,
            Some("Bypassed because evaluator is not required for OOC meta turn".to_string()),
        );
        pipeline_trace.final_status = "success".into();
        pipeline_trace.total_elapsed_ms = turn_started.elapsed().as_millis() as u64;
        window.emit("pipeline-trace-updated", &pipeline_trace).ok();
        if let Ok(conn) = state.conn.lock() {
            let mut trace_val = narrator_pipeline_trace.clone();
            trace_val["pipeline_trace"] = serde_json::to_value(&pipeline_trace).unwrap_or_default();
            let _ = update_llm_payload_pipeline_trace(&conn, payload_log_id, &trace_val);
        }
        let mut debug = pre_save_debug;
        debug.assistant_message_id = Some(assistant_message_id);
        debug.selected_variant_id = selected_variant_id;
        debug.state_updater_status = "meta_no_op".into();
        debug.output_contract_warning = output_contract_warning;
        debug.request_id = Some(request_id.clone());
        debug.turn_id = turn_trace.turn_id.clone();
        if let Some(branch_id) = ledger_branch_id.as_deref() {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            let empty_patch = EnginePatch::default();
            let (_commit, patch_record) = db::record_turn_commit_with_patch_for_turn_id(
                &conn,
                &canonical_turn_id,
                &conversation_id,
                branch_id,
                ledger_parent_turn_id.as_deref(),
                ledger_user_message_id,
                assistant_message_id,
                selected_variant_id,
                &empty_patch,
                replacement_assistant_id.is_some(),
            )
            .map_err(|err| err.to_string())?;
            turn_trace.state_patch_id = Some(patch_record.patch_id.clone());
            debug.state_patch_id = turn_trace.state_patch_id.clone();
            let rebuilt = db::rebuild_session_state(&conn, &conversation_id, branch_id)
                .map_err(|err| err.to_string())?;
            soul = rebuilt.soul;
            session_world = rebuilt.session_world;
        }
        if let Some(variant_id) = selected_variant_id {
            if let Ok(debug_json) = serde_json::to_string(&debug) {
                let _ = state
                    .conn
                    .lock()
                    .map_err(|err| err.to_string())
                    .and_then(|conn| {
                        db::update_assistant_variant_debug_json(&conn, variant_id, &debug_json)
                            .map_err(|err| err.to_string())
                    });
            }
        }
        let (messages, context_preview) = {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            let messages =
                db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
            let context_preview = compile_context_for_session(
                &soul,
                Some(&session_world),
                &messages_to_context(messages.clone()),
            );
            (messages, context_preview)
        };
        emit_dev_log(
            &window,
            "info",
            "state_updater",
            "state_updater_bypassed_for_meta_turn",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "reason": "gm_ooc_meta_turn"
            })),
        );
        emit_perf_log(
            &window,
            &conversation_id,
            "total turn time",
            turn_started.elapsed(),
        );
        return Ok(TurnResult {
            conversation_id,
            soul,
            visible_response: visible_response_for_updater,
            context_preview,
            messages,
            consolidation_ran: false,
            debug,
        });
    }

    let before_state_summary = compact_state_summary_json(&soul, &session_world);
    let evaluator_request_id = format!("eval_{request_id}");
    let active_player_persona = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::get_active_player_persona(&conn, &conversation_id).map_err(|err| err.to_string())?
    };

    let mut baseline_patch_id = None;
    let mut baseline_event_id = None;
    let mut baseline_commit = None;
    let is_normal_scene_turn = !pure_ooc_detected && !user_is_ooc;
    let pre_baseline_soul = soul.clone();
    let pre_baseline_session_world = session_world.clone();

    let baseline_start = Instant::now();
    if is_normal_scene_turn && ledger_branch_id.is_some() {
        let branch_id = ledger_branch_id.as_deref().unwrap();
        let (ev_id, baseline_patch) = construct_baseline_patch(
            &soul,
            &snapshot_user_text,
            &visible_response_for_updater,
            &active_player_persona.persona_id,
        );
        baseline_event_id = Some(ev_id);

        let commit_res: Result<(db::TurnCommit, db::StatePatchRecord, db::LedgerRebuild), String> =
            (|| {
                let conn = state.conn.lock().map_err(|err| err.to_string())?;
                if replacement_assistant_id.is_some() {
                    db::discard_active_commits_for_assistant(
                        &conn,
                        &conversation_id,
                        assistant_message_id,
                    )
                    .map_err(|err| err.to_string())?;
                }

                let (commit, patch_record) = db::record_turn_commit_with_patch_for_turn_id(
                    &conn,
                    &canonical_turn_id,
                    &conversation_id,
                    branch_id,
                    ledger_parent_turn_id.as_deref(),
                    ledger_user_message_id,
                    assistant_message_id,
                    selected_variant_id,
                    &baseline_patch,
                    replacement_assistant_id.is_some(),
                )
                .map_err(|err| err.to_string())?;

                let rebuilt = db::rebuild_session_state(&conn, &conversation_id, branch_id)
                    .map_err(|err| err.to_string())?;

                Ok((commit, patch_record, rebuilt))
            })();

        let (commit, patch_record, rebuilt) = match commit_res {
            Ok(val) => val,
            Err(err) => {
                pipeline_trace.record_stage_error(
                    "baseline_patch_committed",
                    baseline_start.elapsed().as_millis() as u64,
                    PipelineErrorCode::DatabaseError,
                    err.clone(),
                    Some("Check database integrity or connection".to_string()),
                );
                window.emit("pipeline-trace-updated", &pipeline_trace).ok();
                if let Ok(conn) = state.conn.lock() {
                    let _ = update_llm_payload_pipeline_trace(
                        &conn,
                        payload_log_id,
                        &serde_json::json!({ "pipeline_trace": pipeline_trace }),
                    );
                }
                return Err(err);
            }
        };

        soul = rebuilt.soul;
        session_world = rebuilt.session_world;
        baseline_commit = Some(commit.clone());
        baseline_patch_id = Some(patch_record.patch_id.clone());
        turn_trace.state_patch_id = Some(patch_record.patch_id.clone());

        emit_dev_log(
            &window,
            "success",
            "ledger",
            "baseline_turn_commit_recorded",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "request_id": request_id.as_str(),
                "turn_id": commit.turn_id.as_str(),
                "baseline_patch_id": patch_record.patch_id.as_str(),
            })),
        );

        pipeline_trace.record_stage(
            "baseline_patch_committed",
            "success",
            baseline_start.elapsed().as_millis() as u64,
            Some(format!("Event: {:?}", baseline_event_id)),
            Some(format!("Baseline patch ID: {:?}", baseline_patch_id)),
        );
        window.emit("pipeline-trace-updated", &pipeline_trace).ok();
    } else {
        pipeline_trace.record_stage(
            "baseline_patch_committed",
            "skipped",
            0,
            None,
            Some(
                "Bypassed because it is not a normal scene turn or ledger branch is disabled"
                    .to_string(),
            ),
        );
        window.emit("pipeline-trace-updated", &pipeline_trace).ok();
    }

    // Fast-mode evaluator gate (Pillar 2 Lever B): dialogue-only turns skip
    // the evaluator entirely; the exchange is queued and folded into the next
    // evaluator run as catch-up. Requires a committed baseline so the
    // exchange itself is never lost.
    if evaluator_execution_mode(&state_updater_settings) == EVALUATOR_EXECUTION_MODE_FAST
        && is_normal_scene_turn
        && baseline_patch_id.is_some()
    {
        let previous_status = state.conn.lock().ok().and_then(|conn| {
            previous_assistant_status_block(&conn, &conversation_id, assistant_message_id)
        });
        let current_status = first_valid_status_block(&visible_response_for_updater);
        let (significance, gate_reason) = classify_turn_for_evaluator_gate(
            &snapshot_user_text,
            current_status.as_deref(),
            previous_status.as_deref(),
        );
        emit_dev_log(
            &window,
            "info",
            "evaluator",
            "evaluator_gate_classified",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "significance": significance,
                "reason": gate_reason
            })),
        );
        if significance == TurnSignificance::DialogueOnly {
            {
                let conn = state.conn.lock().map_err(|err| err.to_string())?;
                db::insert_evaluator_catchup_entry(
                    &conn,
                    &conversation_id,
                    ledger_user_message_id,
                    assistant_message_id,
                    &snapshot_user_text,
                    &visible_response_for_updater,
                )
                .map_err(|err| err.to_string())?;
            }
            pipeline_trace.record_stage(
                "evaluator_job_started",
                "skipped",
                0,
                Some(format!("gate: {gate_reason}")),
                Some(
                    "Evaluator skipped: dialogue-only turn (fast mode); exchange queued for catch-up"
                        .to_string(),
                ),
            );
            pipeline_trace.final_status = "success".into();
            pipeline_trace.total_elapsed_ms = turn_started.elapsed().as_millis() as u64;
            window.emit("pipeline-trace-updated", &pipeline_trace).ok();
            if let Ok(conn) = state.conn.lock() {
                let mut trace_val = narrator_pipeline_trace.clone();
                trace_val["pipeline_trace"] =
                    serde_json::to_value(&pipeline_trace).unwrap_or_default();
                let _ = update_llm_payload_pipeline_trace(&conn, payload_log_id, &trace_val);
            }
            let mut debug = pre_save_debug;
            debug.assistant_message_id = Some(assistant_message_id);
            debug.selected_variant_id = selected_variant_id;
            debug.state_updater_status = "skipped_dialogue_only".into();
            debug.output_contract_warning = output_contract_warning;
            debug.request_id = Some(request_id.clone());
            debug.turn_id = turn_trace.turn_id.clone();
            debug.baseline_patch_id = baseline_patch_id;
            if let Some(variant_id) = selected_variant_id {
                if let Ok(debug_json) = serde_json::to_string(&debug) {
                    let _ = state
                        .conn
                        .lock()
                        .map_err(|err| err.to_string())
                        .and_then(|conn| {
                            db::update_assistant_variant_debug_json(&conn, variant_id, &debug_json)
                                .map_err(|err| err.to_string())
                        });
                }
            }
            let (messages, context_preview) = {
                let conn = state.conn.lock().map_err(|err| err.to_string())?;
                let messages = db::list_messages(&conn, &conversation_id, 100)
                    .map_err(|err| err.to_string())?;
                let context_preview = compile_context_for_session(
                    &soul,
                    Some(&session_world),
                    &messages_to_context(messages.clone()),
                );
                (messages, context_preview)
            };
            emit_dev_log(
                &window,
                "info",
                "evaluator",
                "evaluator_skipped_dialogue_only",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "assistant_message_id": assistant_message_id,
                    "gate_reason": gate_reason,
                    "baseline_patch_id": debug.baseline_patch_id.as_deref()
                })),
            );
            emit_perf_log(
                &window,
                &conversation_id,
                "total turn time",
                turn_started.elapsed(),
            );
            return Ok(TurnResult {
                conversation_id,
                soul,
                visible_response: visible_response_for_updater,
                context_preview,
                messages,
                consolidation_ran: false,
                debug,
            });
        }
    }

    let job_start = Instant::now();
    // A background job whose settings cannot produce a request finishes in
    // milliseconds having called nothing, and used to report `partial_success`
    // with no error and no patch — so a whole run could look like it evaluated
    // every turn while storing nothing. Refuse up front and say why.
    // An unassigned State Updater profile arrives as the built-in default: no
    // model, and OpenAI's base URL. Rather than skip evaluation for the whole
    // session, borrow the narrator's profile — it is configured by definition,
    // since the turn just used it. Announced, not silent: a run that quietly
    // evaluated with different settings than the operator chose is its own bug.
    let state_updater_settings = match unusable_evaluator_settings(&state_updater_settings) {
        Some(reason) if unusable_evaluator_settings(&narrator_settings).is_none() => {
            emit_dev_log(
                &window,
                "warn",
                "evaluator",
                "evaluator_settings_fell_back_to_narrator",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "reason": reason,
                    "using_model": narrator_settings.model.trim(),
                    "hint": "Assign a State Updater profile in Settings > AI to choose the model yourself.",
                })),
            );
            ApiProviderSettings {
                evaluator_background_enabled: state_updater_settings.evaluator_background_enabled,
                evaluator_mode: state_updater_settings.evaluator_mode.clone(),
                structured_evaluator_transport: state_updater_settings
                    .structured_evaluator_transport
                    .clone(),
                structured_evaluator_policy: state_updater_settings
                    .structured_evaluator_policy
                    .clone(),
                ..narrator_settings.clone()
            }
        }
        _ => state_updater_settings,
    };

    if let Some(reason) = unusable_evaluator_settings(&state_updater_settings) {
        emit_dev_log(
            &window,
            "error",
            "evaluator",
            "evaluator_settings_unusable",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "reason": reason,
                "base_url": state_updater_settings.base_url.trim(),
                "hint": "Assign a State Updater provider profile in Settings > AI.",
            })),
        );
    } else if evaluator_background_enabled(&state_updater_settings) {
        let entity_updater_context =
            build_entity_updater_context(&pre_baseline_soul, &entity_context);
        let memory_debug_nonce = format!("memory-debug-{}", uuid_like_id());
        let job = match start_background_evaluator_job(
            app.clone(),
            window.clone(),
            conversation_id.clone(),
            assistant_message_id,
            selected_variant_id,
            request_id.clone(),
            evaluator_request_id.clone(),
            baseline_commit
                .as_ref()
                .map(|commit| commit.turn_id.clone())
                .or_else(|| turn_trace.turn_id.clone()),
            context_mode.label().to_string(),
            pre_baseline_soul.clone(),
            pre_baseline_session_world.clone(),
            snapshot_user_text.clone(),
            visible_response_for_updater.clone(),
            context_preview.text.clone(),
            state_updater_settings.clone(),
            entity_updater_context,
            memory_debug_nonce,
            ledger_branch_id.clone(),
            ledger_parent_turn_id.clone(),
            ledger_user_message_id,
            replacement_assistant_id.is_some(),
            before_state_summary.clone(),
            baseline_patch_id.clone(),
            None,
        ) {
            Ok(job) => job,
            Err(err) => {
                pipeline_trace.record_stage_error(
                    "evaluator_job_started",
                    job_start.elapsed().as_millis() as u64,
                    PipelineErrorCode::EvaluatorSpawnError,
                    err.to_string(),
                    Some("Failed to spawn background evaluator job".to_string()),
                );
                window.emit("pipeline-trace-updated", &pipeline_trace).ok();
                if let Ok(conn) = state.conn.lock() {
                    let mut trace_val = narrator_pipeline_trace.clone();
                    trace_val["pipeline_trace"] =
                        serde_json::to_value(&pipeline_trace).unwrap_or_default();
                    let _ = update_llm_payload_pipeline_trace(&conn, payload_log_id, &trace_val);
                }
                return Err(err);
            }
        };
        pipeline_trace.record_stage(
            "evaluator_job_started",
            "success",
            job_start.elapsed().as_millis() as u64,
            Some(format!("Assistant Message ID: {}", assistant_message_id)),
            Some(format!("Evaluator Job ID: {}", job.evaluator_job_id)),
        );
        window.emit("pipeline-trace-updated", &pipeline_trace).ok();
        if let Ok(conn) = state.conn.lock() {
            let mut trace_val = narrator_pipeline_trace.clone();
            trace_val["pipeline_trace"] = serde_json::to_value(&pipeline_trace).unwrap_or_default();
            let _ = update_llm_payload_pipeline_trace(&conn, payload_log_id, &trace_val);
        }
        let mut debug = pre_save_debug;
        debug.assistant_message_id = Some(assistant_message_id);
        debug.selected_variant_id = selected_variant_id;
        debug.state_updater_status = format!("background_{}", job.status);
        debug.output_contract_warning = output_contract_warning;
        debug.request_id = Some(request_id.clone());
        debug.turn_id = turn_trace.turn_id.clone();
        debug.baseline_patch_id = baseline_patch_id;
        if let Some(variant_id) = selected_variant_id {
            if let Ok(debug_json) = serde_json::to_string(&debug) {
                let _ = state
                    .conn
                    .lock()
                    .map_err(|err| err.to_string())
                    .and_then(|conn| {
                        db::update_assistant_variant_debug_json(&conn, variant_id, &debug_json)
                            .map_err(|err| err.to_string())
                    });
            }
        }
        let (messages, context_preview) = {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            let messages =
                db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
            let context_preview = compile_context_for_session(
                &soul,
                Some(&session_world),
                &messages_to_context(messages.clone()),
            );
            (messages, context_preview)
        };
        emit_dev_log(
            &window,
            "info",
            "evaluator",
            "evaluator_background_job_spawned",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "evaluator_job_id": job.evaluator_job_id.as_str(),
                "timeout_ms": job.timeout_ms,
                "timeout_mode": job.timeout_mode.as_str()
            })),
        );
        emit_perf_log(
            &window,
            &conversation_id,
            "total turn time",
            turn_started.elapsed(),
        );
        return Ok(TurnResult {
            conversation_id,
            soul,
            visible_response: visible_response_for_updater,
            context_preview,
            messages,
            consolidation_ran: false,
            debug,
        });
    }
    let updater_payload_started = Instant::now();
    let mut state_updater_settings = state_updater_settings;
    if let Ok(conn) = state.conn.lock() {
        state_updater_settings.evaluator_mode =
            resolve_evaluator_mode_setting(&conn, &conversation_id, &state_updater_settings);
        state_updater_settings.structured_evaluator_policy =
            resolve_structured_evaluator_policy_setting(
                &conn,
                &conversation_id,
                &state_updater_settings,
            );
    }
    let evaluator_mode = evaluator_mode(&state_updater_settings);
    let selected_evaluator_source = selected_evaluator_source(&evaluator_mode);
    let form_spec = matches!(
        selected_evaluator_source,
        EVALUATOR_MODE_FORM_V1 | EVALUATOR_MODE_STRUCTURED_V1 | EVALUATOR_MODE_PERCEPTION_V2
    )
    .then(|| {
        build_eval_form_spec_with_player_persona(
            &pre_baseline_soul,
            Some(&pre_baseline_session_world),
            &snapshot_user_text,
            &visible_response_for_updater,
            8,
            &active_player_persona.persona_id,
            &active_player_persona.display_name,
        )
    });
    let fallback_form_system_prompt = matches!(
        selected_evaluator_source,
        EVALUATOR_MODE_STRUCTURED_V1 | EVALUATOR_MODE_PERCEPTION_V2
    )
    .then(|| {
        build_evaluator_form_prompt_with_player_persona(
            &pre_baseline_soul,
            Some(&pre_baseline_session_world),
            &snapshot_user_text,
            &visible_response_for_updater,
            &active_player_persona.persona_id,
            &active_player_persona.display_name,
        )
    });
    let perception_source =
        (selected_evaluator_source == EVALUATOR_MODE_PERCEPTION_V2).then(|| {
            production_perception_source(
                &conversation_id,
                ledger_branch_id.as_deref(),
                turn_trace.turn_id.as_deref(),
                ledger_parent_turn_id.as_deref(),
                ledger_user_message_id,
                assistant_message_id,
                selected_variant_id,
                active_souls_for_v1(&pre_baseline_soul),
                &snapshot_user_text,
                &visible_response_for_updater,
            )
        });
    let updater_system_prompt = if selected_evaluator_source == EVALUATOR_MODE_FORM_V1 {
        build_evaluator_form_prompt_with_player_persona(
            &pre_baseline_soul,
            Some(&pre_baseline_session_world),
            &snapshot_user_text,
            &visible_response_for_updater,
            &active_player_persona.persona_id,
            &active_player_persona.display_name,
        )
    } else if selected_evaluator_source == EVALUATOR_MODE_STRUCTURED_V1 {
        build_structured_evaluator_prompt(&pre_baseline_soul, Some(&pre_baseline_session_world))
    } else if selected_evaluator_source == EVALUATOR_MODE_PERCEPTION_V2 {
        build_perception_v2_prompt_with_player_persona(
            &pre_baseline_soul,
            Some(&pre_baseline_session_world),
            &active_player_persona.persona_id,
            &active_player_persona.display_name,
        )
    } else {
        build_evaluator_prompt(&pre_baseline_soul, Some(&pre_baseline_session_world))
    };
    let entity_updater_context = build_entity_updater_context(&pre_baseline_soul, &entity_context);
    let memory_debug_nonce = format!("memory-debug-{}", uuid_like_id());
    let updater_user_message = build_evaluator_user_message(
        &snapshot_user_text,
        &visible_response_for_updater,
        &context_preview.text,
        Some(&pre_baseline_session_world),
        Some(&entity_updater_context),
        Some(&memory_debug_nonce),
    );
    // Fold in exchanges the fast-mode gate skipped; they are deleted only
    // after this evaluator run parses successfully, so failures retry them.
    let catchup_entries = state
        .conn
        .lock()
        .ok()
        .map(|conn| db::list_evaluator_catchup_entries(&conn, &conversation_id).unwrap_or_default())
        .unwrap_or_default();
    let drained_catchup_ids: Vec<i64> = catchup_entries.iter().map(|entry| entry.id).collect();
    let updater_user_message =
        append_evaluator_catchup_block(updater_user_message, &catchup_entries);
    let updater_token_estimate =
        estimate_tokens(&updater_system_prompt) + estimate_tokens(&updater_user_message);
    emit_perf_log(
        &window,
        &conversation_id,
        "compile updater payload",
        updater_payload_started.elapsed(),
    );
    if updater_token_estimate > STATE_UPDATER_TARGET_TOKENS {
        emit_dev_log(
            &window,
            "warn",
            "performance",
            "evaluator payload exceeds budget",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "estimated_tokens": updater_token_estimate,
                "target_tokens": STATE_UPDATER_TARGET_TOKENS
            })),
        );
    }
    emit_dev_log(
        &window,
        "info",
        "evaluator",
        "evaluator_called",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "assistant_message_id": assistant_message_id,
            "model": state_updater_settings.model.trim(),
            "base_url": state_updater_settings.base_url.trim(),
            "evaluator_mode": evaluator_mode.as_str(),
            "selected_evaluator_source": selected_evaluator_source,
            "estimated_total_tokens": updater_token_estimate
        })),
    );
    let updater_log_id = match state
        .conn
        .lock()
        .map_err(|err| err.to_string())
        .and_then(|conn| {
            db::insert_llm_payload_log(
                &conn,
                &LlmPayloadLog {
                    id: 0,
                    conversation_id: conversation_id.clone(),
                    message_id: Some(assistant_message_id),
                    provider: evaluator_provider_label(&evaluator_mode, false),
                    mode: evaluator_mode.clone(),
                    context_mode: context_mode.label().into(),
                    model: state_updater_settings.model.trim().to_string(),
                    base_url: state_updater_settings.base_url.trim().to_string(),
                    system_message: updater_system_prompt.clone(),
                    user_message: updater_user_message.clone(),
                    context_text: updater_system_prompt.clone(),
                    estimated_system_tokens: estimate_tokens(&updater_system_prompt),
                    estimated_user_tokens: estimate_tokens(&updater_user_message),
                    estimated_total_tokens: updater_token_estimate,
                    truncated: false,
                    created_at: db::now_ts(),
                    branch_id: ledger_branch_id.clone(),
                    active_turn_id: ledger_parent_turn_id.clone(),
                    parent_turn_id: ledger_parent_turn_id.clone(),
                    state_patch_ids_applied: Vec::new(),
                    discarded_patch_ids_skipped: Vec::new(),
                    state_rebuild_generation: None,
                    latest_assistant_variant_id: selected_variant_id,
                    request_id: Some(evaluator_request_id.clone()),
                    turn_id: turn_trace.turn_id.clone(),
                    ..Default::default()
                },
            )
            .map_err(|err| err.to_string())
        }) {
        Ok(log_id) => Some(log_id),
        Err(err) => {
            eprintln!(
                "Evaluator payload logging failed; narration saved without evaluator log: {err}"
            );
            emit_dev_log(
                &window,
                "warn",
                "db",
                "Evaluator payload log failed",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "assistant_message_id": assistant_message_id,
                    "error": err.clone()
                })),
            );
            None
        }
    };
    let updater_call_started = Instant::now();
    let updater_response_result = complete_evaluator_with_config(
        &provider,
        &state_updater_settings,
        &updater_system_prompt,
        &updater_user_message,
    )
    .await;
    let updater_call_elapsed = updater_call_started.elapsed();
    let evaluator_timeout_ms = effective_evaluator_timeout_ms(&state_updater_settings);
    if evaluator_timeout_ms
        .is_some_and(|timeout_ms| updater_call_elapsed >= Duration::from_millis(timeout_ms))
    {
        emit_dev_log(
            &window,
            "warn",
            "evaluator",
            "Evaluator timed out; narration saved without state update",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "timeout_ms": evaluator_timeout_ms,
                "elapsed_ms": updater_call_elapsed.as_millis()
            })),
        );
    }
    emit_perf_log(
        &window,
        &conversation_id,
        "evaluator API call",
        updater_call_elapsed,
    );
    let parse_started = Instant::now();
    let raw_updater_response = updater_response_result
        .as_ref()
        .ok()
        .map(|completion| completion.raw_text.clone());
    let structured_enforcement = updater_response_result
        .as_ref()
        .ok()
        .and_then(|completion| completion.structured_enforcement);
    {
        let (prompt_tokens, completion_tokens, estimated) = evaluator_token_usage_for_trace(
            updater_response_result
                .as_ref()
                .ok()
                .and_then(|completion| completion.token_usage),
            &updater_system_prompt,
            &updater_user_message,
            raw_updater_response.as_deref(),
        );
        let usage = pipeline_trace
            .token_usage
            .get_or_insert_with(TurnTokenUsage::default);
        usage.evaluator_prompt_tokens = prompt_tokens;
        usage.evaluator_completion_tokens = completion_tokens;
        usage.evaluator_estimated = estimated;
    }
    let mut evaluator_pipeline_trace: serde_json::Value;
    let updater_result = match updater_response_result {
        Ok(updater_completion) => {
            let updater_response = updater_completion.raw_text.clone();
            emit_dev_log(
                &window,
                "info",
                "evaluator",
                "evaluator_response_received",
                Some(serde_json::json!({
                    "conversation_id": conversation_id.as_str(),
                    "assistant_message_id": assistant_message_id,
                    "response_chars": updater_response.chars().count()
                })),
            );
            if let Some(updater_log_id) = updater_log_id {
                let _ = state
                    .conn
                    .lock()
                    .map_err(|err| err.to_string())
                    .and_then(|conn| {
                        db::update_llm_payload_log_response(
                            &conn,
                            updater_log_id,
                            &db::LlmPayloadResponseUpdate {
                                raw_provider_response: Some(updater_response.clone()),
                                normalized_response: Some(updater_response.clone()),
                                ..Default::default()
                            },
                        )
                        .map_err(|err| err.to_string())
                    });
            }
            let structured_step =
                structured_fallback_step(updater_completion.structured_enforcement);
            let compiled = if selected_evaluator_source == EVALUATOR_MODE_PERCEPTION_V2 {
                perception_source
                    .as_ref()
                    .ok_or_else(|| "Perception V2 source was not initialized".to_string())
                    .and_then(|result| result.as_ref().map_err(Clone::clone))
                    .and_then(|source| {
                        compile_perception_v2_runtime(
                            &updater_response,
                            updater_completion.structured_enforcement,
                            source,
                            compiler_entity_catalog(
                                &pre_baseline_soul,
                                &pre_baseline_session_world,
                                &active_player_persona.persona_id,
                                &active_player_persona.display_name,
                            ),
                            &SimulationSnapshot {
                                state_hash: source.parent_state_hash().map(str::to_string),
                                existing_effect_ids: Vec::new(),
                            },
                            evaluator_provider_label(&evaluator_mode, false),
                            &state_updater_settings.model,
                        )
                    })
            } else {
                compile_selected_evaluator_runtime(
                    &evaluator_mode,
                    form_spec.clone(),
                    &updater_response,
                    updater_completion.structured_enforcement,
                    &pre_baseline_soul,
                    &pre_baseline_session_world,
                    &snapshot_user_text,
                    &visible_response_for_updater,
                    baseline_event_id.clone(),
                    state_updater_settings.structured_require_ops == Some(true),
                )
            };
            match compiled {
                Ok(mut outcome) => {
                    apply_completion_retry_trace(&mut outcome, &updater_completion.trace);
                    if let Some(comparison_trace) = dual_compare_deferred_trace(
                        &evaluator_mode,
                        parse_started.elapsed().as_millis(),
                        false,
                    ) {
                        outcome.comparison_trace = Some(comparison_trace);
                    }
                    Ok(outcome)
                }
                Err(err) if selected_evaluator_source == EVALUATOR_MODE_PERCEPTION_V2 => {
                    emit_dev_log(
                        &window,
                        "warn",
                        "evaluator",
                        "perception_v2_fallback_to_form_started",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "assistant_message_id": assistant_message_id,
                            "error": err.as_str()
                        })),
                    );
                    let (fallback_result, _) = complete_form_fallback_runtime(
                        &provider,
                        &state_updater_settings,
                        fallback_form_system_prompt
                            .as_deref()
                            .unwrap_or(&updater_system_prompt),
                        &updater_user_message,
                        form_spec.clone(),
                        &pre_baseline_soul,
                        &pre_baseline_session_world,
                        &snapshot_user_text,
                        &visible_response_for_updater,
                        baseline_event_id.clone(),
                        vec![EVALUATOR_MODE_PERCEPTION_V2.into()],
                        err,
                    )
                    .await;
                    fallback_result
                }
                Err(err) if selected_evaluator_source == EVALUATOR_MODE_STRUCTURED_V1 => {
                    if updater_completion.structured_enforcement
                        == Some(StructuredEnforcement::JsonSchema)
                    {
                        emit_dev_log(
                            &window,
                            "warn",
                            "evaluator",
                            "structured_schema_claim_failed",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "assistant_message_id": assistant_message_id,
                                "structured_enforcement_requested": StructuredEnforcement::JsonSchema.as_label(),
                                "structured_schema_validation_status": structured_validation_status_from_error(&err),
                                "structured_schema_validation_error": err.as_str()
                            })),
                        );
                    }
                    emit_dev_log(
                        &window,
                        "error",
                        "evaluator",
                        "structured_evaluator_failed",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "assistant_message_id": assistant_message_id,
                            "error": err.as_str(),
                            "structured_enforcement": updater_completion.structured_enforcement.map(StructuredEnforcement::as_label)
                        })),
                    );
                    match retry_structured_tool_call_after_compile_failure(
                        &provider,
                        &state_updater_settings,
                        &updater_system_prompt,
                        &updater_user_message,
                        &updater_completion,
                        &err,
                        &pre_baseline_soul,
                        &pre_baseline_session_world,
                        &snapshot_user_text,
                        &visible_response_for_updater,
                        baseline_event_id.clone(),
                    )
                    .await
                    {
                        Ok(mut outcome) => {
                            if let Some(comparison_trace) = dual_compare_deferred_trace(
                                &evaluator_mode,
                                parse_started.elapsed().as_millis(),
                                false,
                            ) {
                                outcome.comparison_trace = Some(comparison_trace);
                            }
                            Ok(outcome)
                        }
                        Err(retry_failure) => {
                            emit_dev_log(
                                &window,
                                "warn",
                                "evaluator",
                                "structured_evaluator_retry_failed",
                                Some(serde_json::json!({
                                    "conversation_id": conversation_id.as_str(),
                                    "assistant_message_id": assistant_message_id,
                                    "structured_retry_count": retry_failure.retry_count,
                                    "structured_retry_reasons": &retry_failure.retry_reasons,
                                    "structured_retry_final_error": retry_failure.final_error.as_str()
                                })),
                            );
                            emit_dev_log(
                                &window,
                                "warn",
                                "evaluator",
                                "structured_evaluator_fallback_to_form_started",
                                Some(serde_json::json!({
                                    "conversation_id": conversation_id.as_str(),
                                    "assistant_message_id": assistant_message_id,
                                })),
                            );
                            let (fallback_result, _fallback_raw) = complete_form_fallback_runtime(
                                &provider,
                                &state_updater_settings,
                                fallback_form_system_prompt
                                    .as_deref()
                                    .unwrap_or(&updater_system_prompt),
                                &updater_user_message,
                                form_spec.clone(),
                                &pre_baseline_soul,
                                &pre_baseline_session_world,
                                &snapshot_user_text,
                                &visible_response_for_updater,
                                baseline_event_id.clone(),
                                vec![structured_step.to_string()],
                                retry_failure.final_error.clone(),
                            )
                            .await;
                            match fallback_result {
                                Ok(mut outcome) => {
                                    apply_structured_retry_failure(&mut outcome, &retry_failure);
                                    emit_dev_log(
                                        &window,
                                        "success",
                                        "evaluator",
                                        "structured_evaluator_fallback_to_form_succeeded",
                                        Some(serde_json::json!({
                                            "conversation_id": conversation_id.as_str(),
                                            "assistant_message_id": assistant_message_id,
                                            "fallback_path": outcome.fallback_path
                                        })),
                                    );
                                    Ok(outcome)
                                }
                                Err(form_err) => {
                                    emit_dev_log(
                                        &window,
                                        "error",
                                        "evaluator",
                                        "structured_evaluator_fallback_to_form_failed",
                                        Some(serde_json::json!({
                                            "conversation_id": conversation_id.as_str(),
                                            "assistant_message_id": assistant_message_id,
                                            "error": form_err.as_str()
                                        })),
                                    );
                                    emit_dev_log(
                                        &window,
                                        "warn",
                                        "evaluator",
                                        "evaluator_noop_after_all_fallbacks",
                                        Some(serde_json::json!({
                                            "conversation_id": conversation_id.as_str(),
                                            "assistant_message_id": assistant_message_id,
                                        })),
                                    );
                                    let mut outcome = evaluator_noop_after_all_fallbacks(
                                        vec![structured_step.to_string()],
                                        retry_failure.final_error.clone(),
                                        form_err,
                                    );
                                    apply_structured_retry_failure(&mut outcome, &retry_failure);
                                    Ok(outcome)
                                }
                            }
                        }
                    }
                }
                Err(err) => Err(err),
            }
        }
        Err(err) => {
            if matches!(
                selected_evaluator_source,
                EVALUATOR_MODE_STRUCTURED_V1 | EVALUATOR_MODE_PERCEPTION_V2
            ) {
                if structured_enforcement == Some(StructuredEnforcement::JsonSchema) {
                    emit_dev_log(
                        &window,
                        "warn",
                        "evaluator",
                        "structured_schema_claim_failed",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "assistant_message_id": assistant_message_id,
                            "structured_enforcement_requested": StructuredEnforcement::JsonSchema.as_label(),
                            "structured_schema_validation_status": structured_validation_status_from_error(&err),
                            "structured_schema_validation_error": err.as_str()
                        })),
                    );
                }
                emit_dev_log(
                    &window,
                    "error",
                    "evaluator",
                    "structured_evaluator_failed",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "error": err.as_str(),
                        "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label)
                    })),
                );
                emit_dev_log(
                    &window,
                    "warn",
                    "evaluator",
                    "structured_evaluator_fallback_to_form_started",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                    })),
                );
                let structured_failure =
                    if evaluator_timed_out(&err, updater_call_elapsed, &state_updater_settings) {
                        format!(
                            "Evaluator timed out after {}ms; narration saved without state update",
                            evaluator_timeout_ms.unwrap_or(DEFAULT_EVALUATOR_TIMEOUT_MS)
                        )
                    } else {
                        err
                    };
                let (fallback_result, _fallback_raw) = complete_form_fallback_runtime(
                    &provider,
                    &state_updater_settings,
                    fallback_form_system_prompt
                        .as_deref()
                        .unwrap_or(&updater_system_prompt),
                    &updater_user_message,
                    form_spec.clone(),
                    &pre_baseline_soul,
                    &pre_baseline_session_world,
                    &snapshot_user_text,
                    &visible_response_for_updater,
                    baseline_event_id.clone(),
                    vec![evaluator_fallback_origin(
                        selected_evaluator_source,
                        structured_enforcement,
                    )
                    .to_string()],
                    structured_failure.clone(),
                )
                .await;
                match fallback_result {
                    Ok(outcome) => {
                        emit_dev_log(
                            &window,
                            "success",
                            "evaluator",
                            "structured_evaluator_fallback_to_form_succeeded",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "assistant_message_id": assistant_message_id,
                                "fallback_path": outcome.fallback_path
                            })),
                        );
                        Ok(outcome)
                    }
                    Err(form_err) => {
                        emit_dev_log(
                            &window,
                            "error",
                            "evaluator",
                            "structured_evaluator_fallback_to_form_failed",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "assistant_message_id": assistant_message_id,
                                "error": form_err.as_str()
                            })),
                        );
                        emit_dev_log(
                            &window,
                            "warn",
                            "evaluator",
                            "evaluator_noop_after_all_fallbacks",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "assistant_message_id": assistant_message_id,
                            })),
                        );
                        Ok(evaluator_noop_after_all_fallbacks(
                            vec![evaluator_fallback_origin(
                                selected_evaluator_source,
                                structured_enforcement,
                            )
                            .to_string()],
                            structured_failure,
                            form_err,
                        ))
                    }
                }
            } else if evaluator_timed_out(&err, updater_call_elapsed, &state_updater_settings) {
                Err(format!(
                    "Evaluator timed out after {}ms; narration saved without state update",
                    evaluator_timeout_ms.unwrap_or(DEFAULT_EVALUATOR_TIMEOUT_MS)
                ))
            } else {
                Err(err)
            }
        }
    };
    emit_perf_log(
        &window,
        &conversation_id,
        if selected_evaluator_source == EVALUATOR_MODE_FORM_V1 {
            "parse evaluator_form_v1"
        } else if selected_evaluator_source == EVALUATOR_MODE_STRUCTURED_V1 {
            "parse structured evaluator ops"
        } else if selected_evaluator_source == EVALUATOR_MODE_PERCEPTION_V2 {
            "compile perception v2"
        } else {
            "parse EvaluatorOutputV1"
        },
        parse_started.elapsed(),
    );
    let (hidden_state, engine_patch, state_updater_status, hidden_state_found) =
        match updater_result {
            Ok(runtime) => {
                if !drained_catchup_ids.is_empty() {
                    if let Ok(conn) = state.conn.lock() {
                        let _ = db::delete_evaluator_catchup_entries(
                            &conn,
                            &conversation_id,
                            &drained_catchup_ids,
                        );
                    }
                }
                let evaluator_output = runtime.output.clone();
                let conversion = runtime.conversion.clone();
                emit_dev_log(
                    &window,
                    "debug",
                    "evaluator",
                    "evaluator_output_parsed",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "turn_flags_u64": evaluator_output.turn_flags_u64,
                        "memory_candidates": evaluator_output.memory_candidates.len(),
                        "per_soul_evaluations": evaluator_output.per_soul_evaluations.len()
                    })),
                );
                emit_dev_log(
                    &window,
                    "debug",
                    "evaluator",
                    "evaluator_json_parsed",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "evaluator_request_id": evaluator_request_id.as_str(),
                        "turn_flags_u64": evaluator_output.turn_flags_u64,
                        "turn_classification": &evaluator_output.turn_classification,
                        "no_op_reason": evaluator_output.no_op_reason.as_deref()
                    })),
                );
                for candidate_id in &conversion.accepted_candidate_ids {
                    emit_dev_log(
                        &window,
                        "success",
                        "evaluator",
                        "evaluator_candidate_accepted",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "assistant_message_id": assistant_message_id,
                            "candidate_id": candidate_id
                        })),
                    );
                }
                for rejection in &conversion.rejected_candidates {
                    emit_dev_log(
                        &window,
                        "warn",
                        "evaluator",
                        "evaluator_candidate_rejected",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "assistant_message_id": assistant_message_id,
                            "candidate_id": rejection.candidate_id,
                            "reason": rejection.reason
                        })),
                    );
                }
                if conversion.no_op {
                    emit_dev_log(
                        &window,
                        "info",
                        "evaluator",
                        "evaluator_no_op",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "assistant_message_id": assistant_message_id,
                            "reason": evaluator_output.no_op_reason.as_deref()
                        })),
                    );
                }
                let candidate_trace =
                    evaluator_candidate_trace_json(&evaluator_output, &conversion);
                let mut engine_patch = conversion.patch.clone();
                engine_patch = sanitize_state_updater_patch(
                    engine_patch,
                    &pre_baseline_soul,
                    &snapshot_user_text,
                    &visible_response_for_updater,
                );
                strip_premature_world_events_from_updater_patch(
                    &mut engine_patch,
                    &snapshot_user_text,
                    &visible_response_for_updater,
                );
                stamp_memory_provenance(
                    &mut engine_patch,
                    &conversation_id,
                    Some(assistant_message_id),
                    ledger_branch_id.as_deref(),
                );
                let converter_trace = evaluator_converter_trace_json(&engine_patch, &conversion);
                let form_trace = runtime_form_trace_json(&runtime);
                let fallback_trace = evaluator_runtime_fallback_json(&runtime);
                evaluator_pipeline_trace = serde_json::json!({
                    "evaluator_trace": {
                        "evaluator_request_id": evaluator_request_id.as_str(),
                        "parent_narrator_request_id": request_id.as_str(),
                        "turn_id": turn_trace.turn_id.as_deref(),
                        "provider": evaluator_provider_label(&evaluator_mode, false),
                        "model": state_updater_settings.model.trim(),
                        "evaluator_mode": evaluator_mode.as_str(),
                        "selected_evaluator_source": selected_evaluator_source,
                        "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label),
                        "raw_evaluator_response": raw_updater_response.as_deref().unwrap_or_default(),
                        "normalized_evaluator_response": runtime.normalized_json.as_str(),
                        "parsed_evaluator_json": &evaluator_output,
                        "parse_status": "success",
                        "parse_error": serde_json::Value::Null,
                        "evaluator_json_normalized": runtime.normalized,
                        "evaluator_normalization_warnings": &runtime.warnings,
                        "draft_created": true,
                        "draft_memory_candidate_count": runtime.draft.memory_candidate_count,
                        "draft_world_event_count": runtime.draft.world_event_count,
                        "draft_scene_state_present": runtime.draft.scene_state_present,
                        "draft_relationship_delta_count": runtime.draft.relationship_delta_count,
                        "candidate_quality_decisions": &runtime.draft.candidate_quality_decisions,
                        "candidate_routing_decisions": &runtime.draft.candidate_routing_decisions,
                        "state_effect_guarantee_applied": runtime.draft.state_effect_guarantee_applied,
                        "state_effect_guarantee_reason": runtime.draft.state_effect_guarantee_reason.as_deref(),
                        "comparison_trace": runtime.comparison_trace.as_ref(),
                        "evaluator_flags_u64": evaluator_output.turn_flags_u64,
                        "turn_classification": &evaluator_output.turn_classification,
                        "no_op_reason": evaluator_output.no_op_reason.as_deref(),
                        "compiled_patch_summary": engine_patch_summary(&engine_patch)
                    },
                    "evaluator_mode": evaluator_mode.as_str(),
                    "selected_evaluator_source": selected_evaluator_source,
                    "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label),
                    "evaluator_raw_response": raw_updater_response.as_deref().unwrap_or_default(),
                    "evaluator_parsed_json": &evaluator_output,
                    "evaluator_json_normalized": runtime.normalized,
                    "evaluator_normalization_warnings": &runtime.warnings,
                    "draft_created": true,
                    "draft_memory_candidate_count": runtime.draft.memory_candidate_count,
                    "draft_world_event_count": runtime.draft.world_event_count,
                    "draft_scene_state_present": runtime.draft.scene_state_present,
                    "draft_relationship_delta_count": runtime.draft.relationship_delta_count,
                    "candidate_quality_decisions": &runtime.draft.candidate_quality_decisions,
                    "candidate_routing_decisions": &runtime.draft.candidate_routing_decisions,
                    "state_effect_guarantee_applied": runtime.draft.state_effect_guarantee_applied,
                    "state_effect_guarantee_reason": runtime.draft.state_effect_guarantee_reason.as_deref(),
                    "comparison_trace": runtime.comparison_trace.as_ref(),
                    "evaluator_candidate_trace": candidate_trace,
                    "converted_engine_patch": converter_trace,
                    "compiled_patch_summary": engine_patch_summary(&engine_patch),
                    "before_after_state_summary": {
                        "before": before_state_summary.clone(),
                        "after": serde_json::Value::Null
                    }
                });
                if let Some(trace) = evaluator_pipeline_trace.get_mut("evaluator_trace") {
                    insert_json_object_fields(trace, &form_trace);
                    insert_json_object_fields(trace, &fallback_trace);
                }
                insert_json_object_fields(&mut evaluator_pipeline_trace, &form_trace);
                insert_json_object_fields(&mut evaluator_pipeline_trace, &fallback_trace);
                if let Some(updater_log_id) = updater_log_id {
                    if let Ok(conn) = state.conn.lock() {
                        let _ = update_llm_payload_pipeline_trace(
                            &conn,
                            updater_log_id,
                            &evaluator_pipeline_trace,
                        );
                    }
                }
                emit_dev_log(
                    &window,
                    "success",
                    "evaluator",
                    "evaluator_to_patch_converted",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "summary": engine_patch_summary(&engine_patch)
                    })),
                );
                emit_dev_log(
                    &window,
                    "success",
                    "evaluator",
                    "evaluator_patch_converted",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "summary": engine_patch_summary(&engine_patch)
                    })),
                );
                if engine_patch.is_empty() {
                    let is_obvious_scene_turn =
                        evaluator_output.turn_classification.scene_event_occurred
                            || evaluator_output
                                .global_scene_evaluation
                                .scene_event_occurred
                            || (evaluator_output.turn_flags_u64
                                & state_engine::evaluator::turn_flags::SCENE_TURN)
                                != 0;

                    if is_obvious_scene_turn {
                        let mut reasons = Vec::new();
                        if evaluator_output.is_pure_ooc() {
                            reasons.push(
                                "turn is classified as pure out-of-character (OOC)".to_string(),
                            );
                        }
                        if !evaluator_output.world_changes.is_empty() {
                            reasons.push(format!("{} world changes returned by evaluator but filtered out during engine patch conversion", evaluator_output.world_changes.len()));
                        }
                        if !evaluator_output.object_changes.is_empty() {
                            reasons.push(format!("{} object changes returned by evaluator but filtered out during engine patch conversion", evaluator_output.object_changes.len()));
                        }
                        if !conversion.rejected_candidates.is_empty() {
                            let rejected_reasons: Vec<String> = conversion
                                .rejected_candidates
                                .iter()
                                .map(|rc| format!("{}: {}", rc.candidate_id, rc.reason))
                                .collect();
                            reasons.push(format!(
                                "memory candidates rejected: [{}]",
                                rejected_reasons.join(", ")
                            ));
                        } else if !evaluator_output.memory_candidates.is_empty() {
                            reasons.push(format!(
                                "{} global memory candidates filtered out during conversion",
                                evaluator_output.memory_candidates.len()
                            ));
                        }
                        let per_soul_memories: usize = evaluator_output
                            .per_soul_evaluations
                            .iter()
                            .map(|s| s.memory_candidates.len())
                            .sum();
                        if per_soul_memories > 0 {
                            reasons.push(format!(
                                "{} per-soul memory candidates filtered out during conversion",
                                per_soul_memories
                            ));
                        }
                        if reasons.is_empty() {
                            reasons.push("evaluator returned empty lists for all world, object, and memory change fields".to_string());
                        }
                        let computed_reason = reasons.join("; ");

                        emit_dev_log(
                            &window,
                            "info",
                            "evaluator",
                            "evaluator_patch_empty",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "assistant_message_id": assistant_message_id,
                                "reason": computed_reason,
                                "raw_evaluator_response": raw_updater_response
                            })),
                        );
                    } else {
                        emit_dev_log(
                            &window,
                            "info",
                            "evaluator",
                            "evaluator_patch_empty",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "assistant_message_id": assistant_message_id
                            })),
                        );
                    }
                }
                emit_state_updater_patch_log(
                    &window,
                    &conversation_id,
                    assistant_message_id,
                    &soul,
                    &engine_patch,
                );
                emit_truth_boundary_logs(
                    &window,
                    &conversation_id,
                    assistant_message_id,
                    &soul,
                    &engine_patch,
                    &snapshot_user_text,
                );
                accept_verified_memory_layer_reply(
                    &window,
                    &conversation_id,
                    assistant_message_id,
                    &mut soul,
                    &engine_patch,
                    &memory_debug_nonce,
                );
                let hidden_state = hidden_state_from_engine_patch(&engine_patch);
                (
                    hidden_state,
                    engine_patch,
                    if runtime.partial_success {
                        "partial_success".to_string()
                    } else if !runtime.form_rejected_rows.is_empty() {
                        "some_rows_rejected".to_string()
                    } else {
                        "success".to_string()
                    },
                    true,
                )
            }
            Err(err) => {
                let form_trace =
                    failed_form_trace_json(selected_evaluator_source, form_spec.as_ref());
                evaluator_pipeline_trace = serde_json::json!({
                    "evaluator_trace": {
                        "evaluator_request_id": evaluator_request_id.as_str(),
                        "parent_narrator_request_id": request_id.as_str(),
                        "turn_id": turn_trace.turn_id.as_deref(),
                        "provider": evaluator_provider_label(&evaluator_mode, false),
                        "model": state_updater_settings.model.trim(),
                        "evaluator_mode": evaluator_mode.as_str(),
                        "selected_evaluator_source": selected_evaluator_source,
                        "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label),
                        "raw_evaluator_response": raw_updater_response.as_deref().unwrap_or_default(),
                        "normalized_evaluator_response": raw_updater_response.as_deref().unwrap_or_default(),
                        "parsed_evaluator_json": serde_json::Value::Null,
                        "parse_status": "failed",
                        "parse_error": err.as_str(),
                        "evaluator_json_normalized": false,
                        "evaluator_normalization_warnings": [],
                        "evaluator_flags_u64": serde_json::Value::Null,
                        "turn_classification": serde_json::Value::Null,
                        "no_op_reason": serde_json::Value::Null
                    },
                    "evaluator_mode": evaluator_mode.as_str(),
                    "selected_evaluator_source": selected_evaluator_source,
                    "evaluator_raw_response": raw_updater_response.as_deref().unwrap_or_default(),
                    "evaluator_parsed_json": serde_json::json!({
                        "parse_status": "failed",
                        "parse_error": err.as_str(),
                        "evaluator_json_normalized": false,
                        "evaluator_normalization_warnings": []
                    }),
                    "before_after_state_summary": {
                        "before": before_state_summary.clone(),
                        "after": serde_json::Value::Null
                    }
                });
                if let Some(trace) = evaluator_pipeline_trace.get_mut("evaluator_trace") {
                    insert_json_object_fields(trace, &form_trace);
                }
                insert_json_object_fields(&mut evaluator_pipeline_trace, &form_trace);
                if let Some(updater_log_id) = updater_log_id {
                    if let Ok(conn) = state.conn.lock() {
                        let _ = update_llm_payload_pipeline_trace(
                            &conn,
                            updater_log_id,
                            &evaluator_pipeline_trace,
                        );
                    }
                }
                eprintln!("State updater failed; narration saved without state update: {err}");
                emit_dev_log(
                    &window,
                    "error",
                    "evaluator",
                    "evaluator_parse_failed",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "parse_failure_reason": err.clone()
                    })),
                );
                emit_dev_log(
                    &window,
                    "error",
                    "evaluator",
                    "Evaluator failed; narration saved without state update",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "error": err.clone()
                    })),
                );
                (
                    HiddenState::default(),
                    EnginePatch::default(),
                    if baseline_patch_id.is_some() {
                        "partial_success".to_string()
                    } else {
                        format!("failed: {err}")
                    },
                    false,
                )
            }
        };
    let fallback_hidden_state_generated = false;
    let mut debug = debug_from_hidden_state(
        "API",
        &hidden_state,
        hidden_state_found,
        fallback_hidden_state_generated,
    );
    debug.narrator_response_saved = true;
    debug.assistant_message_id = Some(assistant_message_id);
    debug.selected_variant_id = selected_variant_id;
    debug.state_updater_status = state_updater_status;
    debug.replay_detected = debug_replay_detected;
    debug.replay_score = debug_replay_score;
    debug.replay_reason = debug_replay_reason;
    debug.replay_compared_against_message_id = debug_replay_compared_against_message_id;
    debug.output_contract_warning = output_contract_warning;
    debug.request_id = Some(request_id.clone());
    debug.turn_id = turn_trace.turn_id.clone();

    let apply_started = Instant::now();
    let world_patch_present = engine_patch.world_patch.is_some();
    if let Some(world_patch) = engine_patch.world_patch.as_ref() {
        let world_snapshot = session_world.world_log();
        for notice in world_patch.object_consistency_notices(&world_snapshot) {
            if notice.rejected {
                emit_dev_log(
                    &window,
                    "warn",
                    "state_updater",
                    "world_object_contradiction_detected",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "session_world_id": session_world.world_id.as_str(),
                        "object_id": notice.object_id.as_deref(),
                        "reason": notice.reason.as_str(),
                        "event_preview": head_tail_excerpt_chars(&notice.event, 120, 0, 120)
                    })),
                );
                emit_dev_log(
                    &window,
                    "warn",
                    "state_updater",
                    "world_event_rejected_or_downgraded",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "session_world_id": session_world.world_id.as_str(),
                        "object_id": notice.object_id.as_deref(),
                        "reason": notice.reason.as_str()
                    })),
                );
                emit_dev_log(
                    &window,
                    "warn",
                    "ledger",
                    "state_conflict_detected",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "conflict": "world_object_contradiction",
                        "object_id": notice.object_id.as_deref(),
                        "reason": notice.reason.as_str()
                    })),
                );
            }
        }
    }
    let ledger_apply_trace: serde_json::Value;
    let ledger_rebuild_debug = if let Some(branch_id) = ledger_branch_id.as_deref() {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;

        let mut enrichment_id = None;
        let (commit_turn_id, patch_record) = if let Some(ref bp_id) = baseline_patch_id {
            let commit_id = if let Some(ref bc) = baseline_commit {
                bc.turn_id.clone()
            } else {
                turn_trace.turn_id.clone().unwrap_or_default()
            };
            let pr = if !engine_patch.is_empty() {
                let rec = db::record_enrichment_patch_with_metadata(
                    &conn,
                    &commit_id,
                    &engine_patch,
                    Some(bp_id),
                    Some(assistant_message_id),
                    selected_variant_id,
                    None,
                )
                .map_err(|err| err.to_string())?;
                enrichment_id = Some(rec.patch_id.clone());
                rec
            } else {
                db::get_state_patch(&conn, bp_id).map_err(|err| err.to_string())?
            };
            (commit_id, pr)
        } else {
            if replacement_assistant_id.is_some() {
                db::discard_active_commits_for_assistant(
                    &conn,
                    &conversation_id,
                    assistant_message_id,
                )
                .map_err(|err| err.to_string())?;
            }
            let (commit, pr) = db::record_turn_commit_with_patch_for_turn_id(
                &conn,
                &canonical_turn_id,
                &conversation_id,
                branch_id,
                ledger_parent_turn_id.as_deref(),
                ledger_user_message_id,
                assistant_message_id,
                selected_variant_id,
                &engine_patch,
                replacement_assistant_id.is_some(),
            )
            .map_err(|err| err.to_string())?;
            (commit.turn_id.clone(), pr)
        };
        if let Some(instruction) = correction_instruction.as_deref() {
            db::append_memory_correction_event(
                &conn,
                &conversation_id,
                branch_id,
                &commit_turn_id,
                replacement_assistant_id,
                instruction,
            )
            .map_err(|err| err.to_string())?;
        }

        turn_trace.state_patch_id = Some(patch_record.patch_id.clone());
        debug.state_patch_id = turn_trace.state_patch_id.clone();
        debug.baseline_patch_id = baseline_patch_id.clone();
        debug.enrichment_patch_id = enrichment_id.clone();

        emit_dev_log(
            &window,
            "success",
            "ledger",
            "turn_commit_recorded",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "request_id": request_id.as_str(),
                "turn_id": commit_turn_id.as_str(),
                "state_patch_id": turn_trace.state_patch_id.as_deref(),
                "assistant_message_id": assistant_message_id,
                "user_message_id": ledger_user_message_id
            })),
        );
        emit_dev_log(
            &window,
            "success",
            "evaluator",
            "evaluator_patch_stored",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "turn_commit_id": commit_turn_id.as_str(),
                "state_patch_id": patch_record.patch_id.as_str(),
                "patch_empty": engine_patch.is_empty()
            })),
        );
        let rebuilt = db::rebuild_session_state(&conn, &conversation_id, branch_id)
            .map_err(|err| err.to_string())?;
        soul = rebuilt.soul;
        session_world = rebuilt.session_world;
        let rebuild_debug = rebuilt.debug.clone();
        emit_perf_log(
            &window,
            &conversation_id,
            "apply EnginePatch from ledger",
            apply_started.elapsed(),
        );
        emit_dev_log(
            &window,
            "success",
            "ledger",
            "branch_state_rebuilt",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "branch_id": rebuild_debug.branch_id,
                "active_turn_id": rebuild_debug.active_turn_id,
                "applied_patches": rebuild_debug.applied_patches,
                "skipped_discarded_patches": rebuild_debug.skipped_discarded_patches,
                "invalidated_patches": rebuild_debug.invalidated_patches,
                "rebuild_generation": rebuild_debug.rebuild_generation
            })),
        );
        emit_dev_log(
            &window,
            "success",
            "ledger",
            "session_world_cache_refreshed_from_ledger",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "branch_id": rebuild_debug.branch_id,
                "active_turn_id": rebuild_debug.active_turn_id,
                "rebuild_generation": rebuild_debug.rebuild_generation
            })),
        );
        emit_dev_log(
            &window,
            if engine_patch.is_empty() {
                "info"
            } else {
                "success"
            },
            "evaluator",
            if engine_patch.is_empty() {
                "evaluator_patch_apply_skipped_reason"
            } else {
                "evaluator_patch_applied"
            },
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "reason": if engine_patch.is_empty() { "empty_patch_recorded_in_ledger" } else { "ledger_rebuild_applied_patch" },
                "state_patch_id": turn_trace.state_patch_id.as_deref()
            })),
        );
        emit_per_soul_memory_written_logs(&window, &conversation_id, &engine_patch);
        ledger_apply_trace = serde_json::json!({
            "state_patch_id": patch_record.patch_id,
            "turn_commit_id": commit_turn_id,
            "branch_id": branch_id,
            "patch_stored": true,
            "patch_applied": !engine_patch.is_empty(),
            "patch_apply_skipped_reason": if engine_patch.is_empty() { Some("empty_patch_recorded_in_ledger") } else { None },
            "branch_rebuilt": true,
            "applied_patch_count": rebuild_debug.applied_patches.len(),
            "skipped_patch_count": rebuild_debug.skipped_discarded_patches.len(),
            "invalidated_patch_count": rebuild_debug.invalidated_patches.len(),
            "materialized_soul_updated": true,
            "materialized_session_world_updated": true,
            "baseline_patch_id": baseline_patch_id,
            "enrichment_patch_id": enrichment_id
        });
        Some(rebuild_debug)
    } else {
        let mut patch_applied = false;
        let mut patch_apply_skipped_reason: Option<String> = if engine_patch.is_empty() {
            Some("empty_patch".into())
        } else {
            None
        };
        match engine_patch.apply_to_session(&mut soul, Some(&mut session_world)) {
            Ok(report) => {
                patch_applied = !engine_patch.is_empty();
                emit_perf_log(
                    &window,
                    &conversation_id,
                    "apply EnginePatch",
                    apply_started.elapsed(),
                );
                emit_perf_log(
                    &window,
                    &conversation_id,
                    "memory hygiene",
                    apply_started.elapsed(),
                );
                emit_dev_log(
                    &window,
                    "success",
                    "evaluator",
                    "evaluator_patch_applied",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "relationship_updated": report.relationship_updated,
                        "memories_added": report.memories_added,
                        "world_updated": report.world_updated,
                        "world_patch_target": if world_patch_present { "session_world" } else { "none" },
                        "body_updated": report.body_updated
                    })),
                );
                if world_patch_present && report.world_updated {
                    emit_dev_log(
                        &window,
                        "success",
                        "state_updater",
                        "world_patch_applied_to_session_world",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "assistant_message_id": assistant_message_id,
                            "session_world_id": session_world.world_id.as_str(),
                            "source_setting_id": session_world.source_setting_id.as_deref()
                        })),
                    );
                }
                emit_relationship_delta_logs(&window, &conversation_id, &engine_patch);
                emit_memory_apply_logs(&window, &conversation_id, &report.memory_events);
                emit_per_soul_memory_written_logs(&window, &conversation_id, &engine_patch);
            }
            Err(err) => {
                patch_apply_skipped_reason = Some(format!("{err:?}"));
                emit_perf_log(
                    &window,
                    &conversation_id,
                    "apply EnginePatch",
                    apply_started.elapsed(),
                );
                emit_dev_log(
                    &window,
                    "error",
                    "evaluator",
                    "evaluator_patch_apply_skipped_reason",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "error": format!("{err:?}")
                    })),
                )
            }
        }
        soul.turn_counter += 1;
        soul.turns_since_consolidation += 1;
        ledger_apply_trace = serde_json::json!({
            "state_patch_id": serde_json::Value::Null,
            "turn_commit_id": serde_json::Value::Null,
            "branch_id": serde_json::Value::Null,
            "patch_stored": false,
            "patch_applied": patch_applied,
            "patch_apply_skipped_reason": patch_apply_skipped_reason,
            "branch_rebuilt": false,
            "applied_patch_count": if patch_applied { 1 } else { 0 },
            "skipped_patch_count": if patch_applied { 0 } else { 1 },
            "invalidated_patch_count": 0,
            "materialized_soul_updated": true,
            "materialized_session_world_updated": true
        });
        None
    };

    if let serde_json::Value::Object(trace) = &mut evaluator_pipeline_trace {
        let selected_patch_applied_before_comparison_done =
            evaluator_mode == EVALUATOR_MODE_DUAL_COMPARE;
        let comparison_skipped_or_timed_out = evaluator_mode == EVALUATOR_MODE_DUAL_COMPARE;
        trace.insert(
            "comparison_skipped_or_timed_out".into(),
            serde_json::json!(comparison_skipped_or_timed_out),
        );
        trace.insert(
            "selected_path_elapsed_ms".into(),
            serde_json::json!(updater_call_elapsed.as_millis()),
        );
        trace.insert("comparison_path_elapsed_ms".into(), serde_json::Value::Null);
        trace.insert(
            "selected_patch_applied_before_comparison_done".into(),
            serde_json::json!(selected_patch_applied_before_comparison_done),
        );
        if let Some(evaluator_trace) = trace.get_mut("evaluator_trace") {
            insert_json_object_fields(
                evaluator_trace,
                &serde_json::json!({
                    "comparison_skipped_or_timed_out": comparison_skipped_or_timed_out,
                    "selected_path_elapsed_ms": updater_call_elapsed.as_millis(),
                    "comparison_path_elapsed_ms": serde_json::Value::Null,
                    "selected_patch_applied_before_comparison_done": selected_patch_applied_before_comparison_done,
                }),
            );
        }
        trace.insert("ledger_apply_trace".into(), ledger_apply_trace.clone());
        trace.insert(
            "before_after_state_summary".into(),
            serde_json::json!({
                "before": before_state_summary,
                "after": compact_state_summary_json(&soul, &session_world)
            }),
        );
    }

    let refresh_started = Instant::now();
    let (messages, context_preview, consolidation_ran) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        if let Some(updater_log_id) = updater_log_id {
            if let Err(err) =
                db::set_llm_payload_log_message_id(&conn, updater_log_id, assistant_message_id)
            {
                eprintln!("State updater payload log link failed; narration remains saved: {err}");
                emit_dev_log(
                    &window,
                    "warn",
                    "db",
                    "State updater payload log link failed",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "error": err.to_string()
                    })),
                );
            }
        }
        if let Some(debug) = ledger_rebuild_debug.as_ref() {
            for log_id in [Some(payload_log_id), updater_log_id].into_iter().flatten() {
                let _ = db::set_llm_payload_log_ledger_metadata(
                    &conn,
                    log_id,
                    debug,
                    ledger_parent_turn_id.as_deref(),
                    selected_variant_id,
                );
            }
        }

        let consolidation_ran = ledger_branch_id.is_none()
            && soul.turns_since_consolidation >= CONSOLIDATION_INTERVAL_TURNS;
        if consolidation_ran {
            consolidate_soul(&mut soul);
        }

        db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
        db::upsert_session_world(&conn, &session_world).map_err(|err| err.to_string())?;
        emit_dev_log(
            &window,
            "success",
            "ledger",
            "materialized_state_refreshed",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "soul_id": soul.character_id.as_str(),
                "world_id": session_world.world_id.as_str(),
                "turn_counter": soul.turn_counter
            })),
        );
        if let Some(updater_log_id) = updater_log_id {
            let _ =
                update_llm_payload_pipeline_trace(&conn, updater_log_id, &evaluator_pipeline_trace);
        }
        let messages =
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
        let context_preview = compile_context_for_session(
            &soul,
            Some(&session_world),
            &messages_to_context(messages.clone()),
        );

        (messages, context_preview, consolidation_ran)
    };
    emit_perf_log(
        &window,
        &conversation_id,
        "refresh frontend state",
        refresh_started.elapsed(),
    );
    emit_dev_log(
        &window,
        "success",
        "success",
        "Turn complete",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "assistant_message_id": assistant_message_id,
            "state_updater_status": debug.state_updater_status,
            "consolidation_ran": consolidation_ran,
            "messages": messages.len()
        })),
    );
    emit_perf_log(
        &window,
        &conversation_id,
        "total turn time",
        turn_started.elapsed(),
    );

    if let Some(variant_id) = selected_variant_id {
        if let Ok(debug_json) = serde_json::to_string(&debug) {
            let _ = state
                .conn
                .lock()
                .map_err(|err| err.to_string())
                .and_then(|conn| {
                    db::update_assistant_variant_debug_json(&conn, variant_id, &debug_json)
                        .map_err(|err| err.to_string())
                });
        }
    }
    if let Ok(conn) = state.conn.lock() {
        emit_turn_branch_integrity_log(
            &window,
            &conn,
            &conversation_id,
            if replacement_assistant_id.is_some() {
                if correction_instruction
                    .as_deref()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .is_some()
                {
                    OP_FIX_RESPONSE
                } else {
                    OP_REGENERATE
                }
            } else {
                OP_NORMAL_SEND
            },
            ledger_user_message_id,
            assistant_message_id,
            canonical_turn_id.as_str(),
        );
    }

    Ok(TurnResult {
        conversation_id,
        soul,
        visible_response: visible_response_for_updater,
        context_preview,
        messages,
        consolidation_ran,
        debug,
    })
}

fn emit_turn_branch_integrity_log(
    window: &Window,
    conn: &Connection,
    conversation_id: &str,
    operation_type: &str,
    user_message_id: Option<i64>,
    assistant_message_id: i64,
    turn_id: &str,
) {
    let variants = db::list_assistant_message_variants(conn, conversation_id, assistant_message_id)
        .unwrap_or_default();
    let assistant_variant_ids_for_turn = variants
        .iter()
        .filter_map(|variant| variant.id)
        .collect::<Vec<_>>();
    let active_variant_index = variants
        .iter()
        .position(|variant| variant.is_selected)
        .map(|index| index + 1)
        .unwrap_or(0);
    let branch_ids_for_turn = conn
        .prepare(
            "SELECT DISTINCT branch_id FROM turn_commits WHERE conversation_id = ?1 AND turn_id = ?2 ORDER BY branch_id ASC",
        )
        .and_then(|mut stmt| {
            let rows = stmt.query_map(rusqlite::params![conversation_id, turn_id], |row| {
                row.get::<_, String>(0)
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default();
    let turn_commit_count = conn
        .query_row(
            "SELECT COUNT(*) FROM turn_commits WHERE conversation_id = ?1 AND turn_id = ?2",
            rusqlite::params![conversation_id, turn_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);
    emit_dev_log(
        window,
        if operation_type == OP_NORMAL_SEND && variants.len() > 1 {
            "error"
        } else {
            "info"
        },
        "ledger",
        "turn_branch_variant_invariant",
        Some(serde_json::json!({
            "operation_type": operation_type,
            "user_message_id": user_message_id,
            "assistant_message_id": assistant_message_id,
            "turn_id": turn_id,
            "turn_commit_id": turn_id,
            "turn_commit_count": turn_commit_count,
            "assistant_variant_ids_for_turn": assistant_variant_ids_for_turn,
            "branch_ids_for_turn": branch_ids_for_turn,
            "visible_variant_count": variants.len(),
            "active_variant_index": active_variant_index
        })),
    );
}

#[allow(clippy::too_many_arguments)]
fn save_visible_narrator_response(
    conn: &Connection,
    conversation_id: &str,
    visible_response: &str,
    replacement_assistant_id: Option<i64>,
    correction_instruction: Option<&str>,
    pre_turn_soul_json: &str,
    snapshot_user_text: &str,
    payload_log_id: i64,
    origin: NarratorMessageOrigin,
    debug: Option<&TurnDebug>,
) -> Result<(i64, Option<i64>), String> {
    if origin == NarratorMessageOrigin::Api && is_known_mock_template_prose(visible_response) {
        return Err(
            "Refusing to save mock-template narrator prose on the API provider path".into(),
        );
    }

    let assistant_message_id = if let Some(message_id) = replacement_assistant_id {
        message_id
    } else {
        db::insert_message_and_get_id(conn, conversation_id, "assistant", visible_response)
            .map_err(|err| err.to_string())?
    };

    if payload_log_id > 0 {
        db::set_llm_payload_log_message_id(conn, payload_log_id, assistant_message_id)
            .map_err(|err| err.to_string())?;
    }

    let (variant_source, variant_label) = match origin {
        NarratorMessageOrigin::Api => (
            if correction_instruction
                .map(str::trim)
                .filter(|instruction| !instruction.is_empty())
                .is_some()
            {
                OP_FIX_RESPONSE
            } else if replacement_assistant_id.is_some() {
                OP_REGENERATE
            } else {
                OP_NORMAL_SEND
            },
            None,
        ),
        NarratorMessageOrigin::Mock => ("mock_provider", None),
    };

    let debug_json = if let Some(debug) = debug {
        serde_json::to_string(debug).map_err(|err| err.to_string())?
    } else {
        let pending_debug = TurnDebug {
            provider: match origin {
                NarratorMessageOrigin::Api => "API".into(),
                NarratorMessageOrigin::Mock => "Mock".into(),
            },
            hidden_state_found: false,
            fallback_hidden_state_generated: false,
            narrator_response_saved: true,
            assistant_message_id: Some(assistant_message_id),
            selected_variant_id: None,
            state_updater_status: "pending".into(),
            replay_detected: false,
            replay_score: 0.0,
            replay_reason: None,
            replay_compared_against_message_id: None,
            output_contract_warning: None,
            tag: None,
            trust_delta: None,
            affection_delta: None,
            new_location: None,
            present_characters: Vec::new(),
            request_id: None,
            turn_id: None,
            state_patch_id: None,
            baseline_patch_id: None,
            enrichment_patch_id: None,
            simulated_response: origin == NarratorMessageOrigin::Mock,
            fallback_used: false,
            fallback_reason: None,
        };
        serde_json::to_string(&pending_debug).map_err(|err| err.to_string())?
    };

    let variant = if replacement_assistant_id.is_some() || origin == NarratorMessageOrigin::Api {
        if replacement_assistant_id.is_some() {
            db::create_assistant_message_variant(
                conn,
                conversation_id,
                assistant_message_id,
                visible_response,
                variant_label,
                Some(variant_source),
                true,
                Some(pre_turn_soul_json),
                Some(&debug_json),
            )
            .map_err(|err| err.to_string())?
        } else {
            db::seed_initial_assistant_message_variant(
                conn,
                conversation_id,
                assistant_message_id,
                visible_response,
                Some(OP_NORMAL_SEND),
                Some(pre_turn_soul_json),
                Some(&debug_json),
            )
            .map_err(|err| err.to_string())?
        }
    } else {
        db::seed_initial_assistant_message_variant(
            conn,
            conversation_id,
            assistant_message_id,
            visible_response,
            Some(variant_source),
            Some(pre_turn_soul_json),
            Some(&debug_json),
        )
        .map_err(|err| err.to_string())?
    };

    if replacement_assistant_id.is_none() {
        db::upsert_turn_snapshot(
            conn,
            &db::TurnSnapshot {
                conversation_id: conversation_id.to_string(),
                assistant_message_id,
                user_text: snapshot_user_text.to_string(),
                soul_json: pre_turn_soul_json.to_string(),
            },
        )
        .map_err(|err| err.to_string())?;
    }

    Ok((assistant_message_id, variant.id))
}

pub(crate) fn emit_dev_log(
    window: &Window,
    level: &str,
    category: &str,
    message: &str,
    details: Option<serde_json::Value>,
) {
    let sequence = DEV_LOG_COUNTER.fetch_add(1, Ordering::Relaxed);
    let event = DevLogEvent {
        id: format!("{}-{sequence}", db::now_ts()),
        timestamp: db::now_ts(),
        level: level.to_string(),
        category: category.to_string(),
        message: message.to_string(),
        details: details.map(redact_dev_log_details),
    };
    if let Err(err) = window.emit("dev-log", event) {
        eprintln!("Dev log emit failed: {err}");
    }
}

fn emit_perf_log(window: &Window, conversation_id: &str, stage: &str, elapsed: Duration) {
    emit_dev_log(
        window,
        if elapsed.as_millis() > 2_000 {
            "info"
        } else {
            "debug"
        },
        "performance",
        &format!("{stage}: {} ms", elapsed.as_millis()),
        Some(serde_json::json!({
            "conversation_id": conversation_id,
            "stage": stage,
            "elapsed_ms": elapsed.as_millis()
        })),
    );
}

fn emit_entity_resolution_log(window: &Window, conversation_id: &str, speaker: &SpeakerResolution) {
    match speaker.status {
        SpeakerResolutionStatus::NoLabel => {}
        SpeakerResolutionStatus::Exact => emit_dev_log(
            window,
            "info",
            "context",
            "Speaker entity resolved",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref(),
                "entity_id": speaker.entity_id.as_str(),
                "display_name": speaker.display_name.as_str()
            })),
        ),
        SpeakerResolutionStatus::FuzzyTypo => emit_dev_log(
            window,
            "warn",
            "warning",
            "Speaker label resolved to active entity",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref(),
                "entity_id": speaker.entity_id.as_str(),
                "display_name": speaker.display_name.as_str()
            })),
        ),
        SpeakerResolutionStatus::Ambiguous => emit_dev_log(
            window,
            "warn",
            "warning",
            "Ambiguous speaker label",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref(),
                "candidates": speaker.candidates.clone()
            })),
        ),
        SpeakerResolutionStatus::Created => emit_dev_log(
            window,
            "success",
            "context",
            "Entity created",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref(),
                "entity_id": speaker.entity_id.as_str(),
                "display_name": speaker.display_name.as_str()
            })),
        ),
        SpeakerResolutionStatus::Unknown => emit_dev_log(
            window,
            "warn",
            "warning",
            "Unknown speaker label",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "label": speaker.label.as_deref()
            })),
        ),
    }
}

fn emit_relationship_delta_logs(window: &Window, conversation_id: &str, patch: &EnginePatch) {
    let Some(soul_patch) = patch.soul_patch.as_ref() else {
        return;
    };
    let mut deltas = Vec::new();
    if let Some(delta) = soul_patch.relationship_delta.as_ref() {
        deltas.push(delta);
    }
    deltas.extend(soul_patch.relationship_deltas.iter());
    for delta in deltas {
        emit_dev_log(
            window,
            "info",
            "state_updater",
            "Relationship delta applied",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "from": delta.from.as_deref().unwrap_or("active_soul"),
                "target": delta.target.as_deref().unwrap_or("user"),
                "trust": delta.trust,
                "affection": delta.affection,
                "fear": delta.fear,
                "desire": delta.desire,
                "conflict": delta.conflict,
                "curiosity": delta.curiosity,
                "comfort": delta.comfort,
                "dependency": delta.dependency
            })),
        );
    }
}

fn emit_memory_apply_logs(
    window: &Window,
    conversation_id: &str,
    events: &[state_engine::patch::MemoryApplyEvent],
) {
    for event in events {
        let (level, category, message) = match event.action {
            MemoryApplyAction::Added => ("info", "state_updater", "memory_added"),
            MemoryApplyAction::RejectedDuplicate => {
                ("warn", "warning", "memory_rejected_duplicate")
            }
            MemoryApplyAction::RejectedGeneric => ("warn", "warning", "memory_rejected_generic"),
            MemoryApplyAction::Merged => ("info", "state_updater", "memory_merged"),
            MemoryApplyAction::Deprioritized => ("debug", "state_updater", "memory_deprioritized"),
        };
        emit_dev_log(
            window,
            level,
            category,
            message,
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "source_type": event.source_type.as_label(),
                "content_preview": excerpt_for_log(&event.content, 160),
                "reason": event.reason.as_deref()
            })),
        );
        if event.source_type == MemorySourceType::ImportedLog {
            emit_dev_log(
                window,
                "info",
                "context",
                "imported_log_detected",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "content_preview": excerpt_for_log(&event.content, 160)
                })),
            );
        }
        if matches!(
            event.source_type,
            MemorySourceType::CrossSessionBleed | MemorySourceType::PreviousSession
        ) {
            emit_dev_log(
                window,
                "info",
                "context",
                "cross_session_memory_tagged",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "source_type": event.source_type.as_label(),
                    "content_preview": excerpt_for_log(&event.content, 160)
                })),
            );
        }
    }
}

fn emit_per_soul_memory_written_logs(window: &Window, conversation_id: &str, patch: &EnginePatch) {
    let Some(soul_patch) = patch.soul_patch.as_ref() else {
        return;
    };
    for memory in &soul_patch.new_memories {
        if memory.content.trim().is_empty() {
            continue;
        }
        emit_dev_log(
            window,
            "success",
            "evaluator",
            "per_soul_memory_written",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "owner_soul_id": memory.owner_soul_id.as_deref(),
                "memory_slot": memory.memory_slot.as_deref().or(memory.tag.as_deref()),
                "memory_id": memory.memory_id.as_deref(),
                "target_entity_ids": memory.target_entity_ids
            })),
        );
    }
}

fn excerpt_for_log(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        trimmed.to_string()
    } else {
        format!(
            "{}...",
            chars
                .into_iter()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn redact_dev_log_details(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let lowered = key.to_ascii_lowercase();
                    let redacted = lowered.contains("api_key")
                        || lowered == "authorization"
                        || lowered.contains("secret")
                        || lowered == "token"
                        || lowered.ends_with("_token")
                        || lowered.contains("bearer");
                    if redacted {
                        (key, serde_json::Value::String("[redacted]".into()))
                    } else {
                        (key, redact_dev_log_details(value))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(redact_dev_log_details).collect())
        }
        other => other,
    }
}

fn messages_to_context(messages: Vec<ChatMessage>) -> Vec<ContextMessage> {
    messages
        .into_iter()
        .filter(|message| message.channel == db::MESSAGE_CHANNEL_RP_SCENE)
        .map(|message| ContextMessage {
            role: message.role,
            content: message.content,
        })
        .collect()
}

fn recent_assistant_replay_sources(messages: &[ChatMessage], limit: usize) -> Vec<ReplaySource> {
    messages
        .iter()
        .rev()
        .filter(|message| {
            message.role == "assistant" && message.channel == db::MESSAGE_CHANNEL_RP_SCENE
        })
        .take(limit)
        .map(|message| ReplaySource {
            message_id: message.id,
            content: message.content.clone(),
        })
        .collect()
}

fn is_body_only_markers(body: &str) -> bool {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return true;
    }

    for line in trimmed_body.lines() {
        let line_trimmed = line.trim();
        if line_trimmed.is_empty() {
            continue;
        }

        let lower = line_trimmed.to_lowercase();

        if is_status_summary_line(&lower) {
            continue;
        }

        let clean_assistant = lower.trim_matches(|c: char| !c.is_alphabetic());
        if clean_assistant == "assistant" {
            continue;
        }

        let words: Vec<&str> = lower.split_whitespace().collect();
        let mut line_is_marker = true;
        for word in words {
            let clean_word = word.trim_matches(|c: char| !c.is_alphabetic());
            if clean_word.is_empty() {
                continue;
            }
            if clean_word != "assistant"
                && clean_word != "status"
                && clean_word != "scene"
                && clean_word != "focus"
                && clean_word != "physical"
                && clean_word != "state"
                && clean_word != "atmosphere"
                && clean_word != "not"
                && clean_word != "specified"
                && clean_word != "unspecified"
                && clean_word != "unknown"
            {
                line_is_marker = false;
                break;
            }
        }

        if line_is_marker {
            continue;
        }

        return false;
    }

    true
}

fn guard_narrator_visible_response(
    raw_visible_response: &str,
    user_text: &str,
    session_world: &SessionWorld,
    replay_sources: &[ReplaySource],
    fallback_focus: &str,
) -> (String, ReplayGuardResult, Option<String>, Option<String>) {
    let output = apply_output_contract_guard_with_focus(
        raw_visible_response,
        user_text,
        session_world,
        fallback_focus,
    );
    let replay = detect_replay_with_context(&output.text, user_text, session_world, replay_sources);
    (
        output.text,
        replay,
        output.warning,
        output.status_repair_action,
    )
}

/// Detect a leading block of model reasoning where scene prose should be.
///
/// Some models open by restating the user's message or narrating their own
/// planning ("The user is playing a male persona who...", "Let me parse what's
/// happening:") before getting to the scene. The prompt forbids it, but a guard
/// is still needed because the leaked paragraph then becomes the visible reply
/// and the next turn's context.
///
/// Deliberately conservative: only the first paragraph is considered, and only
/// against openers that cannot begin third-person scene narration. Returns the
/// offending paragraph's length in bytes when the rest of the response can stand
/// on its own, so the caller can drop just that paragraph.
fn leading_meta_commentary(body: &str) -> Option<usize> {
    let trimmed_start = body.len() - body.trim_start().len();
    let rest = body.trim_start();
    let paragraph_end = rest.find("\n\n").unwrap_or(rest.len());
    let paragraph = &rest[..paragraph_end];
    let lower = paragraph.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }

    const META_OPENERS: [&str; 12] = [
        "the user is ",
        "the user has ",
        "the user's message",
        "the user seems",
        "let me ",
        "i need to ",
        "i should ",
        "okay, so",
        "okay, let",
        "this is a confusing",
        "this seems to be",
        "it seems the user",
    ];
    if !META_OPENERS.iter().any(|opener| lower.starts_with(opener)) {
        return None;
    }

    // Only strip when real narration follows; otherwise the reply would be
    // emptied and the turn lost entirely.
    let remainder = rest[paragraph_end..].trim();
    if remainder.len() < 40 {
        return None;
    }
    Some(trimmed_start + paragraph_end)
}

#[cfg(test)]
fn apply_output_contract_guard(content: &str, user_text: &str) -> OutputContractResult {
    apply_output_contract_guard_core(content, user_text, None, "Unknown")
}

fn apply_output_contract_guard_with_focus(
    content: &str,
    user_text: &str,
    session_world: &SessionWorld,
    fallback_focus: &str,
) -> OutputContractResult {
    apply_output_contract_guard_core(content, user_text, Some(session_world), fallback_focus)
}

fn apply_output_contract_guard_core(
    content: &str,
    user_text: &str,
    session_world: Option<&SessionWorld>,
    fallback_focus: &str,
) -> OutputContractResult {
    let mut warnings = Vec::new();
    let mut repair_action = None;
    let cleaned_entry = state_engine::hidden_state::strip_assistant_close_tag(content);
    let without_hidden = strip_hidden_state_blocks(&cleaned_entry);
    if without_hidden.trim_end() != cleaned_entry.trim_end() {
        warnings.push("hidden state stripped");
    }
    let (without_engine_patch, engine_patch_stripped) =
        strip_engine_patch_payloads(&without_hidden);
    if engine_patch_stripped {
        warnings.push("EnginePatch JSON stripped");
    }

    let (mut body, mut status_blocks, status_recovered) =
        remove_status_blocks(&without_engine_patch);
    if status_recovered {
        warnings.push("malformed status fence recovered");
        repair_action = Some("recovered_malformed_fence".to_string());
    }

    // Recover unbackticked status lines from body prose:
    if status_blocks.is_empty() {
        let mut status_line_found: Option<String> = None;
        for line in body.lines() {
            let trimmed = line.trim();
            if is_status_summary_line(trimmed) {
                status_line_found = Some(trimmed.to_string());
                break;
            }
        }
        if let Some(line_content) = status_line_found {
            let body_lines: Vec<&str> = body.lines().filter(|l| l.trim() != line_content).collect();
            body = body_lines.join("\n");

            status_blocks.push(format!("```status\n{}\n```", line_content));
            warnings.push("prose status extracted");
            repair_action = Some("extracted_from_prose".to_string());
        }
    }

    if let Some(cut) = leading_meta_commentary(&body) {
        body = body[cut..].trim_start().to_string();
        warnings.push("leading meta-commentary stripped");
        repair_action = Some("stripped_meta_commentary".to_string());
    }

    if status_blocks.len() > 1 {
        warnings.push("multiple status blocks normalized");
    }
    let cleaned_body = state_engine::hidden_state::strip_assistant_close_tag(&body);
    let mut normalized = cleaned_body.trim().to_string();

    // Pure OOC Turn classification (priority primarily on user message)
    let user_is_ooc = is_ooc_or_gm_prefix(user_text);
    let assistant_is_ooc = is_ooc_or_gm_prefix(&normalized);
    let is_pure_ooc = user_is_ooc || (user_text.trim().is_empty() && assistant_is_ooc);

    if is_pure_ooc {
        // pure OOC/GM turn: no status block at all!
        repair_action = Some("gm_ooc_bypassed_status".to_string());
    } else if let Some(status) = status_blocks
        .iter()
        .rev()
        .find(|status| status_block_has_valid_line(status))
    {
        let status = normalize_status_block(status);
        if !normalized.is_empty() {
            normalized.push_str("\n\n");
        }
        normalized.push_str(&status);
    } else if !normalized.is_empty() {
        let focus = if fallback_focus.trim().is_empty() {
            "Unknown"
        } else {
            fallback_focus.trim()
        };
        let atmosphere = session_world
            .map(fallback_status_atmosphere)
            .unwrap_or_else(|| "Not specified".into());
        normalized.push_str(&format!(
            "\n\n```status\nScene | Focus: {focus} | Physical state: Not specified | Atmosphere: {atmosphere}\n```",
        ));
        warnings.push("fallback status block appended");
        repair_action = Some("appended_unknown_fallback".to_string());
    }

    let finalized_text = state_engine::hidden_state::strip_assistant_close_tag(&normalized);

    OutputContractResult {
        text: finalized_text.trim_end().to_string(),
        warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
        status_repair_action: repair_action,
    }
}

fn fallback_status_atmosphere(session_world: &SessionWorld) -> String {
    let world = session_world.world_log();
    if !world.location.trim().is_empty() {
        return world.location.trim().to_string();
    }
    if let Some(event) = world
        .recent_events
        .iter()
        .rev()
        .find(|event| !event.trim().is_empty())
    {
        return event.trim().to_string();
    }
    "Not specified".into()
}

fn status_block_has_valid_line(status_block: &str) -> bool {
    status_block
        .lines()
        .any(|line| is_status_summary_line(line.trim()))
}

fn remove_status_blocks(content: &str) -> (String, Vec<String>, bool) {
    let mut body = String::new();
    let mut status_blocks = Vec::new();
    let mut current_status_lines = Vec::new();
    let mut in_status = false;
    let mut recovered = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if !in_status && trimmed.eq_ignore_ascii_case("```status") {
            in_status = true;
            current_status_lines.clear();
            continue;
        }

        if in_status {
            if trimmed == "```" {
                recovered |=
                    finish_status_candidate(&mut body, &mut status_blocks, &current_status_lines);
                current_status_lines.clear();
                in_status = false;
            } else if trimmed.eq_ignore_ascii_case("```status") {
                recovered = true;
                recovered |=
                    finish_status_candidate(&mut body, &mut status_blocks, &current_status_lines);
                current_status_lines.clear();
            } else {
                current_status_lines.push(line.to_string());
            }
            continue;
        }

        body.push_str(line);
        body.push('\n');
    }

    if in_status && !current_status_lines.is_empty() {
        recovered = true;
        recovered |= finish_status_candidate(&mut body, &mut status_blocks, &current_status_lines);
    }

    (body.trim_end().to_string(), status_blocks, recovered)
}

fn finish_status_candidate(
    body: &mut String,
    status_blocks: &mut Vec<String>,
    lines: &[String],
) -> bool {
    let mut prose_lines = Vec::new();
    let mut status_lines = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "```" || trimmed.eq_ignore_ascii_case("```status") {
            continue;
        }
        if is_status_summary_line(trimmed) {
            status_lines.push(line.trim_end().to_string());
        } else {
            prose_lines.push(line.trim_end().to_string());
        }
    }

    if !prose_lines.is_empty() {
        if !body.trim().is_empty() {
            body.push('\n');
        }
        body.push_str(&prose_lines.join("\n"));
        body.push('\n');
    }
    if !status_lines.is_empty() {
        status_blocks.push(format!("```status\n{}\n```", status_lines.join("\n")));
    }

    !prose_lines.is_empty() || status_lines.len() > 1
}

fn is_status_summary_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("scene |")
        || lower.starts_with("scene:")
        || lower.starts_with("focus:")
        || lower.starts_with("physical state:")
        || lower.starts_with("atmosphere:")
        || (lower.contains("scene | focus:")
            && (lower.contains("physical state:") || lower.contains("atmosphere:")))
}

fn normalize_status_block(status_block: &str) -> String {
    let mut lines = status_block.lines();
    let first_line = lines.next().unwrap_or_default().trim();
    let body_lines = if first_line.eq_ignore_ascii_case("```status") {
        lines
            .filter(|line| line.trim() != "```")
            .map(str::trim_end)
            .collect::<Vec<_>>()
    } else {
        status_block
            .lines()
            .filter(|line| line.trim() != "```")
            .map(str::trim_end)
            .collect::<Vec<_>>()
    };
    let body = body_lines
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let body = if body.trim().is_empty() {
        "Scene | Focus: Unknown | Physical state: Not specified | Atmosphere: Not specified"
            .to_string()
    } else {
        body
    };
    format!("```status\n{}\n```", body.trim())
}

/// Significance of a finished exchange for the evaluator gate (Pillar 2
/// Lever B). The narrator's ```status``` block is the primary signal; every
/// missing or unparseable signal degrades to `SceneRelevant` so the gate can
/// only ever skip turns it positively identified as dialogue-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSignificance {
    /// Pure back-and-forth dialogue; state did not move. Skippable.
    DialogueOnly,
    /// State may have moved; the evaluator must run.
    SceneRelevant,
    /// The scene itself changed; run the evaluator including catch-up for
    /// any previously skipped dialogue turns.
    SceneBoundary,
}

/// The status-block fields the gate compares across turns. Atmosphere is
/// deliberately excluded: mood drift alone does not justify an evaluator run.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StatusGateSignature {
    focus: String,
    physical_state: String,
}

fn status_gate_signature(status_block: &str) -> Option<StatusGateSignature> {
    let mut focus = None;
    let mut physical_state = None;
    for line in status_block.lines() {
        let trimmed = line.trim().trim_matches('`');
        // Both pipe-joined ("Scene | Focus: X | Physical state: Y") and
        // one-field-per-line blocks occur in narrator output.
        for segment in trimmed.split('|') {
            let segment = segment.trim();
            let lower = segment.to_ascii_lowercase();
            if let Some(value) = lower.strip_prefix("focus:") {
                focus = Some(normalize_status_gate_value(value));
            } else if let Some(value) = lower.strip_prefix("physical state:") {
                physical_state = Some(normalize_status_gate_value(value));
            }
        }
    }
    Some(StatusGateSignature {
        focus: focus?,
        physical_state: physical_state?,
    })
}

fn normalize_status_gate_value(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(['.', ',', ';'])
        .to_string()
}

/// Fraction of non-whitespace characters inside quote pairs. Recognizes
/// straight, curly, and CJK quotes.
fn dialogue_quoted_fraction(text: &str) -> f32 {
    let mut quoted = 0usize;
    let mut total = 0usize;
    let mut expected_closer: Option<char> = None;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        total += 1;
        match expected_closer {
            Some(closer) => {
                quoted += 1;
                if ch == closer {
                    expected_closer = None;
                }
            }
            None => {
                expected_closer = match ch {
                    '"' => Some('"'),
                    '\u{201C}' => Some('\u{201D}'), // “ ”
                    '\u{300C}' => Some('\u{300D}'), // 「 」
                    '\u{300E}' => Some('\u{300F}'), // 『 』
                    _ => None,
                };
                if expected_closer.is_some() {
                    quoted += 1;
                }
            }
        }
    }
    if total == 0 {
        return 0.0;
    }
    quoted as f32 / total as f32
}

/// A user message is dialogue-like when it is predominantly quoted speech and
/// contains no `*action*` markup. Unquoted prose is treated as action: "I
/// draw my sword" must not be skipped.
fn user_text_is_dialogue_like(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains('*') {
        return false;
    }
    dialogue_quoted_fraction(trimmed) >= 0.5
}

/// Classify a finished exchange for the conditional evaluator. Returns the
/// significance plus a reason label for the pipeline trace.
fn classify_turn_for_evaluator_gate(
    user_text: &str,
    current_status_block: Option<&str>,
    previous_status_block: Option<&str>,
) -> (TurnSignificance, &'static str) {
    let Some(current) = current_status_block.and_then(status_gate_signature) else {
        return (TurnSignificance::SceneRelevant, "no_current_status_signal");
    };
    let Some(previous) = previous_status_block.and_then(status_gate_signature) else {
        return (TurnSignificance::SceneRelevant, "no_previous_status_signal");
    };
    if current.focus != previous.focus {
        return (TurnSignificance::SceneBoundary, "focus_changed");
    }
    if current.physical_state != previous.physical_state {
        return (TurnSignificance::SceneRelevant, "physical_state_changed");
    }
    if user_text_has_correction_keywords(user_text) {
        return (TurnSignificance::SceneRelevant, "correction_keywords");
    }
    if !user_text_is_dialogue_like(user_text) {
        return (
            TurnSignificance::SceneRelevant,
            "user_text_not_dialogue_like",
        );
    }
    (TurnSignificance::DialogueOnly, "dialogue_only_turn")
}

const EVALUATOR_EXECUTION_MODE_FAST: &str = "fast";
const EVALUATOR_EXECUTION_MODE_BALANCED: &str = "balanced";
const EVALUATOR_EXECUTION_MODE_LONG_CONTEXT: &str = "long_context";

fn evaluator_execution_mode(settings: &ApiProviderSettings) -> &'static str {
    match settings.evaluator_execution_mode.as_deref() {
        Some(EVALUATOR_EXECUTION_MODE_FAST) => EVALUATOR_EXECUTION_MODE_FAST,
        Some(EVALUATOR_EXECUTION_MODE_LONG_CONTEXT) => EVALUATOR_EXECUTION_MODE_LONG_CONTEXT,
        _ => EVALUATOR_EXECUTION_MODE_BALANCED,
    }
}

fn first_valid_status_block(content: &str) -> Option<String> {
    let (_, status_blocks, _) = remove_status_blocks(content);
    status_blocks
        .into_iter()
        .find(|block| status_block_has_valid_line(block))
}

/// Status block of the narrator message immediately before `before_message_id`
/// on the RP channel — the previous-turn signal for the evaluator gate.
fn previous_assistant_status_block(
    conn: &Connection,
    conversation_id: &str,
    before_message_id: i64,
) -> Option<String> {
    let messages =
        db::list_messages_before_id(conn, conversation_id, before_message_id, 30).ok()?;
    messages
        .iter()
        .rev()
        .find(|message| {
            message.role == "assistant" && message.channel == db::MESSAGE_CHANNEL_RP_SCENE
        })
        .and_then(|message| first_valid_status_block(&message.content))
}

/// Render gate-skipped exchanges as a catch-up block appended to the
/// evaluator user message, so one evaluator run can fold in the state implied
/// by dialogue turns it never saw.
fn append_evaluator_catchup_block(
    user_message: String,
    entries: &[db::EvaluatorCatchupEntry],
) -> String {
    if entries.is_empty() {
        return user_message;
    }
    let mut block = String::from(
        "\n\n[CATCH-UP]\nThe following earlier exchanges were dialogue-only and were not \
         evaluated at the time. Fold any state changes they imply (relationships, memories, \
         small world details) into THIS update as well:\n",
    );
    for (index, entry) in entries.iter().enumerate() {
        block.push_str(&format!(
            "\nExchange {}:\nUser: {}\nNarrator: {}\n",
            index + 1,
            entry.user_text.trim(),
            entry.assistant_text.trim()
        ));
    }
    format!("{user_message}{block}")
}

fn strip_engine_patch_payloads(content: &str) -> (String, bool) {
    let (without_fenced, stripped_fenced) = strip_engine_patch_fenced_blocks(content);
    let (without_raw, stripped_raw) = strip_engine_patch_raw_json_objects(&without_fenced);
    (
        without_raw.trim_end().to_string(),
        stripped_fenced || stripped_raw,
    )
}

fn strip_engine_patch_fenced_blocks(content: &str) -> (String, bool) {
    let mut output = String::new();
    let mut fenced_block = String::new();
    let mut in_fence = false;
    let mut stripped = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if !in_fence && trimmed.starts_with("```") {
            in_fence = true;
            fenced_block.clear();
            fenced_block.push_str(line);
            fenced_block.push('\n');
            continue;
        }

        if in_fence {
            fenced_block.push_str(line);
            fenced_block.push('\n');
            if line.trim() == "```" {
                if looks_like_engine_patch_text(&fenced_block) {
                    stripped = true;
                } else {
                    output.push_str(&fenced_block);
                }
                fenced_block.clear();
                in_fence = false;
            }
            continue;
        }

        output.push_str(line);
        output.push('\n');
    }

    if in_fence && !fenced_block.is_empty() {
        if looks_like_engine_patch_text(&fenced_block) {
            stripped = true;
        } else {
            output.push_str(&fenced_block);
        }
    }

    (output.trim_end().to_string(), stripped)
}

fn strip_engine_patch_raw_json_objects(content: &str) -> (String, bool) {
    let char_indices = content.char_indices().collect::<Vec<_>>();
    let mut output = String::new();
    let mut last_emit = 0usize;
    let mut cursor = 0usize;
    let mut stripped = false;

    while cursor < char_indices.len() {
        let (byte_index, character) = char_indices[cursor];
        if character == '{' {
            if let Some(end_byte) = matching_json_object_end(content, byte_index) {
                let candidate = &content[byte_index..end_byte];
                if looks_like_engine_patch_text(candidate)
                    && serde_json::from_str::<serde_json::Value>(candidate).is_ok()
                {
                    output.push_str(&content[last_emit..byte_index]);
                    last_emit = end_byte;
                    stripped = true;
                    while cursor < char_indices.len() && char_indices[cursor].0 < end_byte {
                        cursor += 1;
                    }
                    continue;
                }
            }
        }
        cursor += 1;
    }

    output.push_str(&content[last_emit..]);
    (output.trim_end().to_string(), stripped)
}

fn matching_json_object_end(content: &str, start_byte: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, character) in content[start_byte..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(start_byte + offset + character.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn looks_like_engine_patch_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("\"schema_version\"")
        && (lower.contains("\"soul_patch\"")
            || lower.contains("\"world_patch\"")
            || lower.contains("\"body_patch\"")
            || lower.contains("\"relationship_deltas\""))
}

fn is_ooc_or_gm_prefix(text: &str) -> bool {
    let trimmed = text.trim_start().to_ascii_lowercase();
    trimmed.starts_with("ooc:")
        || trimmed.starts_with("occ:")
        || trimmed.starts_with("gm:")
        || trimmed.starts_with("meta:")
        || trimmed.starts_with("out of character:")
        || trimmed.starts_with("out of charater:")
        || trimmed.starts_with("out of character/rp:")
        || trimmed.starts_with("out of charater/rp:")
        || trimmed.starts_with("[ooc]")
        || trimmed.starts_with("narrator:")
        || trimmed.contains("talking to the narrator")
        || trimmed.contains("talking to the gm")
        || trimmed.contains("addressing the narrator")
        || trimmed.contains("address the narrator")
}

fn is_gm_facing_user_message(user_text: &str) -> bool {
    is_ooc_or_gm_prefix(user_text)
}

fn is_plain_gm_reply(response: &str) -> bool {
    is_ooc_or_gm_prefix(response)
}

fn append_output_warning(current: Option<String>, warning: &str) -> Option<String> {
    Some(match current {
        Some(current) if !current.trim().is_empty() => format!("{current}; {warning}"),
        _ => warning.to_string(),
    })
}

fn sanitize_phone_notification_contradiction(
    response: &str,
    user_text: &str,
    session_world: &SessionWorld,
) -> PhoneContradictionGuard {
    if !user_text_mentions_texting(user_text)
        || !phone_state_blocks_visible_notification(session_world)
        || !text_claims_phone_chime_or_screen_wake(response)
    {
        return PhoneContradictionGuard {
            text: response.to_string(),
            repaired: false,
        };
    }
    let (body, status_blocks, _) = remove_status_blocks(response);
    let mut repaired_sentences = Vec::new();
    let mut replaced = false;
    for sentence in split_visible_sentences(&body) {
        if text_claims_phone_chime_or_screen_wake(sentence) {
            if !replaced {
                repaired_sentences.push("The message arrives silently, without a chime, vibration, or screen wake; it will only be visible when the phone is checked.".to_string());
                replaced = true;
            }
        } else {
            repaired_sentences.push(sentence.trim().to_string());
        }
    }
    let mut text = repaired_sentences
        .into_iter()
        .filter(|sentence| !sentence.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if let Some(status) = status_blocks
        .iter()
        .rev()
        .find(|status| status_block_has_valid_line(status))
    {
        if !text.trim().is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&normalize_status_block(status));
    }
    PhoneContradictionGuard {
        text: text.trim_end().to_string(),
        repaired: true,
    }
}

fn user_text_mentions_texting(user_text: &str) -> bool {
    let lower = user_text.to_ascii_lowercase();
    lower.contains("text")
        || lower.contains("message")
        || lower.contains("dm")
        || lower.contains("sms")
        || lower.contains("send")
}

fn phone_state_blocks_visible_notification(session_world: &SessionWorld) -> bool {
    let world = session_world.world_log();
    world.object_states.iter().any(|object| {
        object.object_id.to_ascii_lowercase().contains("phone")
            && (matches!(
                object.notification_mode.to_ascii_lowercase().as_str(),
                "notifications_off" | "notifications off" | "do_not_disturb" | "silent"
            ) || object.vibrate_enabled == Some(false)
                || object.screen_wake_enabled == Some(false))
    }) || world.key_objects.iter().any(|object| {
        let lower = object.to_ascii_lowercase();
        lower.contains("phone")
            && (lower.contains("notifications off")
                || lower.contains("notification off")
                || lower.contains("do not disturb")
                || lower.contains("do_not_disturb")
                || lower.contains("silent")
                || lower.contains("vibration disabled")
                || lower.contains("no vibration")
                || lower.contains("screen wake disabled")
                || lower.contains("no screen wake"))
    })
}

fn user_text_mentions_call(user_text: &str) -> bool {
    let lower = user_text.to_ascii_lowercase();
    lower.contains("call") || lower.contains("dial") || lower.contains("ring")
}

fn world_phone_state_has_active_call(session_world: &SessionWorld) -> bool {
    let world = session_world.world_log();
    world.object_states.iter().any(|object| {
        let object_id_lower = object.object_id.to_ascii_lowercase();
        if object_id_lower.contains("phone") {
            let status_lower = object.status.to_ascii_lowercase();
            let is_call_status = status_lower.contains("active_call")
                || status_lower.contains("incoming_call")
                || status_lower.contains("active call")
                || status_lower.contains("incoming call")
                || status_lower.contains("ringing");

            let is_call_prop = object.properties.iter().any(|(k, v)| {
                let k_lower = k.to_ascii_lowercase();
                let v_lower = v.to_ascii_lowercase();
                k_lower.contains("call") || v_lower.contains("call")
            });

            is_call_status || is_call_prop
        } else {
            false
        }
    }) || world.key_objects.iter().any(|object| {
        let lower = object.to_ascii_lowercase();
        lower.contains("phone")
            && (lower.contains("active_call")
                || lower.contains("incoming_call")
                || lower.contains("active call")
                || lower.contains("incoming call")
                || lower.contains("ringing"))
    })
}

fn has_phone_call_state_violation(
    response: &str,
    user_text: &str,
    session_world: &SessionWorld,
) -> bool {
    let user_mentions = user_text_mentions_call(user_text);
    let world_has = world_phone_state_has_active_call(session_world);
    if !user_mentions && !world_has {
        let lower = response.to_ascii_lowercase();
        contains_any_text(
            &lower,
            &[
                "call notification",
                "call screen",
                "ringing",
                "incoming call",
                "active call",
                "phone call",
            ],
        )
    } else {
        false
    }
}

fn sanitize_phone_call_state_violation(
    response: &str,
    user_text: &str,
    session_world: &SessionWorld,
) -> PhoneContradictionGuard {
    if !has_phone_call_state_violation(response, user_text, session_world) {
        return PhoneContradictionGuard {
            text: response.to_string(),
            repaired: false,
        };
    }

    let (body, status_blocks, _) = remove_status_blocks(response);
    let mut repaired_sentences = Vec::new();
    let mut repaired = false;

    for sentence in split_visible_sentences(&body) {
        let lower = sentence.to_ascii_lowercase();
        let has_violation = contains_any_text(
            &lower,
            &[
                "call notification",
                "call screen",
                "ringing",
                "incoming call",
                "active call",
                "phone call",
            ],
        );
        if has_violation {
            repaired = true;
        } else {
            repaired_sentences.push(sentence.trim().to_string());
        }
    }

    let mut text = repaired_sentences
        .into_iter()
        .filter(|sentence| !sentence.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if let Some(status) = status_blocks
        .iter()
        .rev()
        .find(|status| status_block_has_valid_line(status))
    {
        if !text.trim().is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&normalize_status_block(status));
    }

    PhoneContradictionGuard {
        text: text.trim_end().to_string(),
        repaired,
    }
}

fn text_claims_phone_chime_or_screen_wake(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("phone")
        && contains_any_text(
            &lower,
            &[
                "chime",
                "buzz",
                "vibrat",
                "ping",
                "lights up",
                "lit up",
                "screen wakes",
                "screen woke",
                "screen wake",
                "notification sound",
            ],
        )
}

fn split_visible_sentences(text: &str) -> Vec<&str> {
    text.split_inclusive(['.', '!', '?'])
        .flat_map(|chunk| chunk.split('\n'))
        .collect()
}

#[cfg(test)]
fn detect_replay(new_response: &str, replay_sources: &[ReplaySource]) -> ReplayGuardResult {
    let dummy_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    detect_replay_with_context(new_response, "", &dummy_world, replay_sources)
}

fn detect_replay_with_context(
    new_response: &str,
    user_text: &str,
    session_world: &SessionWorld,
    replay_sources: &[ReplaySource],
) -> ReplayGuardResult {
    let mut best = ReplayGuardResult::default();
    for source in replay_sources {
        let candidate = compare_replay_against_source_with_context(
            new_response,
            user_text,
            session_world,
            source,
        );
        // Prioritize higher severity and higher scores
        let candidate_rank = match candidate.severity {
            ReplaySeverity::ObjectStateViolation | ReplaySeverity::Contradiction => 3,
            ReplaySeverity::StrongReplay => 2,
            ReplaySeverity::MildOverlap => 1,
            ReplaySeverity::None => 0,
        };
        let best_rank = match best.severity {
            ReplaySeverity::ObjectStateViolation | ReplaySeverity::Contradiction => 3,
            ReplaySeverity::StrongReplay => 2,
            ReplaySeverity::MildOverlap => 1,
            ReplaySeverity::None => 0,
        };
        if candidate_rank > best_rank
            || (candidate_rank == best_rank && candidate.replay_score > best.replay_score)
        {
            best = candidate;
        }
    }
    best
}

fn compare_replay_against_source_with_context(
    new_response: &str,
    user_text: &str,
    session_world: &SessionWorld,
    source: &ReplaySource,
) -> ReplayGuardResult {
    let new_clean = normalize_for_replay(new_response);
    let previous_clean = normalize_for_replay(&source.content);
    if new_clean.is_empty() || previous_clean.is_empty() {
        return ReplayGuardResult {
            compared_against_message_id: Some(source.message_id),
            severity: ReplaySeverity::None,
            ..ReplayGuardResult::default()
        };
    }

    let paragraph_score = paragraph_replay_score(new_response, &source.content);
    let sentence_score = sentence_overlap_score(&new_clean, &previous_clean);
    let shingle_score = shingle_overlap_score(&new_clean, &previous_clean, 10);
    let setup_score = scene_setup_replay_score(new_response, &source.content);
    let (score, reason) = [
        (paragraph_score, "paragraph nearly identical"),
        (sentence_score, "sentence overlap exceeded threshold"),
        (
            shingle_score,
            "repeated wording shingles exceeded threshold",
        ),
        (
            setup_score,
            "repeated scene setup, object list, or opening beat",
        ),
    ]
    .into_iter()
    .max_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
    .unwrap_or((0.0, "no overlap"));

    let base_detected = score > 0.35
        || (paragraph_score >= 0.90
            && repeated_long_paragraph_exists(new_response, &source.content))
        || setup_score >= 0.42;

    // Check for hard object state violation
    let phone_contradiction = user_text_mentions_texting(user_text)
        && phone_state_blocks_visible_notification(session_world)
        && text_claims_phone_chime_or_screen_wake(new_response);

    let phone_call_contradiction =
        has_phone_call_state_violation(new_response, user_text, session_world);

    let (severity, replay_detected, final_reason) = if phone_contradiction {
        (
            ReplaySeverity::ObjectStateViolation,
            true,
            "chime/buzz when phone notifications/screen wake disabled".to_string(),
        )
    } else if phone_call_contradiction {
        (
            ReplaySeverity::ObjectStateViolation,
            true,
            "call notification/call screen/ringing when no active call".to_string(),
        )
    } else if base_detected {
        // Evaluate if this is just mild overlap of scene anchoring markers
        let new_setup = scene_setup_markers(new_response);
        let previous_setup = scene_setup_markers(&source.content);
        let overlap = new_setup.intersection(&previous_setup).count();
        let is_mild_overlap = setup_score >= 0.42
            && overlap < 5
            && shingle_score < 0.25
            && sentence_score < 0.25
            && paragraph_score < 0.50;

        if is_mild_overlap {
            (
                ReplaySeverity::MildOverlap,
                false, // Mild overlap does not trigger regeneration/retry!
                format!("mild setting overlap: stable scene anchoring ({reason})"),
            )
        } else {
            (ReplaySeverity::StrongReplay, true, reason.to_string())
        }
    } else {
        (ReplaySeverity::None, false, reason.to_string())
    };

    ReplayGuardResult {
        replay_detected,
        replay_score: score,
        replay_reason: Some(final_reason),
        compared_against_message_id: Some(source.message_id),
        severity,
    }
}

fn has_object_state_violation(
    response: &str,
    user_text: &str,
    session_world: &SessionWorld,
) -> bool {
    user_text_mentions_texting(user_text)
        && phone_state_blocks_visible_notification(session_world)
        && text_claims_phone_chime_or_screen_wake(response)
}

fn evaluate_response_quality(
    response: &str,
    user_text: &str,
    session_world: &SessionWorld,
    replay_sources: &[ReplaySource],
) -> f32 {
    let mut score = 100.0f32;

    // 1. Repetition penalty: detect replay
    let replay_guard =
        detect_replay_with_context(response, user_text, session_world, replay_sources);
    if replay_guard.replay_detected {
        score -= replay_guard.replay_score * 50.0;
    } else if replay_guard.severity == ReplaySeverity::MildOverlap {
        score -= replay_guard.replay_score * 15.0;
    }

    // 2. Missing status block or Unknown focus on normal turns
    let is_ooc = is_gm_facing_user_message(user_text) || is_plain_gm_reply(response);
    if !is_ooc {
        let (_, status_blocks, _) = remove_status_blocks(response);
        if status_blocks.is_empty() {
            score -= 20.0;
        } else if let Some(status) = status_blocks.first() {
            if status.contains("Focus: Unknown") {
                score -= 15.0;
            }
        }
    }

    // 3. Object state violation
    if has_object_state_violation(response, user_text, session_world) {
        score -= 40.0;
    }
    if has_phone_call_state_violation(response, user_text, session_world) {
        score -= 40.0;
    }

    // 4. Emptiness check
    if response.trim().is_empty() {
        score = 0.0;
    }

    score.max(0.0)
}

fn has_hard_violation(
    text: &str,
    user_text: &str,
    session_world: &SessionWorld,
    replay_sources: &[ReplaySource],
) -> bool {
    if text.trim().is_empty() {
        return true;
    }
    if has_object_state_violation(text, user_text, session_world) {
        return true;
    }
    if has_phone_call_state_violation(text, user_text, session_world) {
        return true;
    }
    if user_text_mentions_texting(user_text)
        && phone_state_blocks_visible_notification(session_world)
        && text_claims_phone_chime_or_screen_wake(text)
    {
        return true;
    }
    let rg = detect_replay_with_context(text, user_text, session_world, replay_sources);
    if rg.replay_detected && rg.severity == ReplaySeverity::StrongReplay && rg.replay_score >= 0.70
    {
        return true;
    }
    false
}

fn user_text_has_correction_keywords(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("correct")
        || lower.contains("retcon")
        || lower.contains("edit")
        || lower.contains("override")
        || lower.contains("rewrite")
        || lower.contains("fix")
        || lower.contains("adjust")
        || lower.contains("update")
        || lower.contains("change")
}

fn scene_setup_replay_score(new_response: &str, previous_response: &str) -> f32 {
    let new_setup = scene_setup_markers(new_response);
    let previous_setup = scene_setup_markers(previous_response);
    if new_setup.len() < 2 || previous_setup.len() < 2 {
        return 0.0;
    }
    let overlap = new_setup.intersection(&previous_setup).count();
    let object_score = overlap as f32 / new_setup.len().min(previous_setup.len()) as f32;
    let opening_score = opening_beat_overlap_score(new_response, previous_response);
    object_score.max(opening_score)
}

fn scene_setup_markers(response: &str) -> HashSet<&'static str> {
    let setup = normalize_for_replay(&first_replay_chars(response, 1_200));
    let mut markers = HashSet::new();
    for (marker, needles) in SCENE_SETUP_MARKERS {
        if needles.iter().any(|needle| setup.contains(needle)) {
            markers.insert(*marker);
        }
    }
    markers
}

const SCENE_SETUP_MARKERS: &[(&str, &[&str])] = &[
    (
        "door_state",
        &[
            "unlocked door",
            "door unlocked",
            "door state",
            "open door",
            "closed door",
            "door",
        ],
    ),
    ("neon", &["neon"]),
    ("rain", &["rain", "rain streak", "rain slick", "rain-slick"]),
    ("room", &["room", "apartment", "cell", "lab"]),
    ("wine_glass", &["wine glass", "glass of wine"]),
    ("phone", &["phone", "handset", "screen"]),
    ("barefoot", &["barefoot", "bare feet"]),
    (
        "oversized_shirt",
        &["oversized shirt", "shirt hanging", "loose shirt"],
    ),
    ("clothing", &["clothing", "shirt", "sleeve", "hem"]),
    (
        "physical_arrangement",
        &["couch", "bed", "table", "doorway", "window"],
    ),
];

fn first_replay_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn opening_beat_overlap_score(new_response: &str, previous_response: &str) -> f32 {
    let new_opening = normalize_for_replay(&first_replay_chars(new_response, 500));
    let previous_opening = normalize_for_replay(&first_replay_chars(previous_response, 500));
    if new_opening.is_empty() || previous_opening.is_empty() {
        return 0.0;
    }
    word_jaccard_similarity(&new_opening, &previous_opening)
}

fn prune_repeated_scene_setup(response: &str, replay_sources: &[ReplaySource]) -> String {
    let repeated_markers = replay_sources
        .iter()
        .flat_map(|source| scene_setup_markers(&source.content))
        .collect::<HashSet<_>>();
    if repeated_markers.len() < 3 {
        return response.to_string();
    }

    let mut pruned = String::new();
    let mut removed = false;
    for segment in split_replay_segments(response) {
        let normalized = normalize_for_replay(segment);
        let marker_hits = repeated_markers
            .iter()
            .filter(|marker| marker_appears_in_text(marker, &normalized))
            .count();
        let setup_inventory = marker_hits >= 3
            || (marker_hits >= 2
                && contains_any_normalized(
                    &normalized,
                    &[
                        "room", "door", "clothing", "object", "glass", "phone", "rain", "neon",
                    ],
                ));
        if setup_inventory {
            removed = true;
            continue;
        }
        pruned.push_str(segment);
    }

    let pruned = pruned.trim();
    if removed && pruned.chars().count() >= 40 {
        pruned.to_string()
    } else {
        response.to_string()
    }
}

fn split_replay_segments(response: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0usize;
    for (index, character) in response.char_indices() {
        if matches!(character, '.' | '!' | '?' | '\n') {
            let end = index + character.len_utf8();
            if let Some(segment) = response.get(start..end) {
                if !segment.trim().is_empty() {
                    segments.push(segment);
                }
            }
            start = end;
        }
    }
    if start < response.len() {
        if let Some(segment) = response.get(start..) {
            if !segment.trim().is_empty() {
                segments.push(segment);
            }
        }
    }
    segments
}

fn marker_appears_in_text(marker: &str, normalized_text: &str) -> bool {
    SCENE_SETUP_MARKERS
        .iter()
        .find(|(candidate, _)| *candidate == marker)
        .map(|(_, needles)| {
            needles
                .iter()
                .any(|needle| normalized_text.contains(needle))
        })
        .unwrap_or(false)
}

fn contains_any_normalized(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn normalize_for_replay(content: &str) -> String {
    let (without_status, _, _) = remove_status_blocks(&strip_hidden_state_blocks(content));
    let (without_patch, _) = strip_engine_patch_payloads(&without_status);
    without_patch
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character.is_whitespace() {
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

fn sentence_overlap_score(new_clean: &str, previous_clean: &str) -> f32 {
    let previous_sentences = split_replay_sentences(previous_clean)
        .into_iter()
        .filter(|sentence| sentence.chars().count() >= 60)
        .collect::<HashSet<_>>();
    if previous_sentences.is_empty() {
        return 0.0;
    }
    let new_sentences = split_replay_sentences(new_clean)
        .into_iter()
        .filter(|sentence| sentence.chars().count() >= 60)
        .collect::<Vec<_>>();
    let total_chars = new_sentences
        .iter()
        .map(|sentence| sentence.chars().count())
        .sum::<usize>();
    if total_chars == 0 {
        return 0.0;
    }
    let overlap_chars = new_sentences
        .iter()
        .filter(|sentence| previous_sentences.contains(*sentence))
        .map(|sentence| sentence.chars().count())
        .sum::<usize>();
    overlap_chars as f32 / total_chars as f32
}

fn split_replay_sentences(text: &str) -> Vec<String> {
    text.split(['.', '!', '?', '\n'])
        .map(str::trim)
        .filter(|sentence| !sentence.is_empty())
        .map(str::to_string)
        .collect()
}

fn shingle_overlap_score(new_clean: &str, previous_clean: &str, shingle_size: usize) -> f32 {
    let new_words = new_clean.split_whitespace().collect::<Vec<_>>();
    let previous_words = previous_clean.split_whitespace().collect::<Vec<_>>();
    if new_words.len() < shingle_size || previous_words.len() < shingle_size {
        return 0.0;
    }
    let new_shingles = word_shingles(&new_words, shingle_size);
    let previous_shingles = word_shingles(&previous_words, shingle_size);
    if new_shingles.is_empty() {
        return 0.0;
    }
    let overlap = new_shingles
        .iter()
        .filter(|shingle| previous_shingles.contains(*shingle))
        .count();
    overlap as f32 / new_shingles.len() as f32
}

fn word_shingles(words: &[&str], size: usize) -> HashSet<String> {
    words
        .windows(size)
        .map(|window| window.join(" "))
        .collect::<HashSet<_>>()
}

fn paragraph_replay_score(new_response: &str, previous_response: &str) -> f32 {
    let new_paragraphs = replay_paragraphs(new_response);
    let previous_paragraphs = replay_paragraphs(previous_response);
    let mut best = 0.0f32;
    for new_paragraph in &new_paragraphs {
        if new_paragraph.chars().count() < 250 {
            continue;
        }
        for previous_paragraph in &previous_paragraphs {
            if previous_paragraph.chars().count() < 250 {
                continue;
            }
            best = best.max(word_jaccard_similarity(new_paragraph, previous_paragraph));
        }
    }
    best
}

fn repeated_long_paragraph_exists(new_response: &str, previous_response: &str) -> bool {
    paragraph_replay_score(new_response, previous_response) >= 0.90
}

fn replay_paragraphs(content: &str) -> Vec<String> {
    let (without_status, _, _) = remove_status_blocks(&strip_hidden_state_blocks(content));
    without_status
        .split("\n\n")
        .map(normalize_for_replay)
        .filter(|paragraph| !paragraph.is_empty())
        .collect()
}

fn word_jaccard_similarity(left: &str, right: &str) -> f32 {
    let left_words = left.split_whitespace().collect::<HashSet<_>>();
    let right_words = right.split_whitespace().collect::<HashSet<_>>();
    if left_words.is_empty() || right_words.is_empty() {
        return 0.0;
    }
    let intersection = left_words.intersection(&right_words).count();
    let union = left_words.union(&right_words).count();
    intersection as f32 / union as f32
}

fn messages_with_repair_instruction(messages: &[ApiMessage]) -> Vec<ApiMessage> {
    let mut repaired = messages.to_vec();
    if let Some(last_user) = repaired
        .iter_mut()
        .rev()
        .find(|message| message.role == "user")
    {
        last_user.content = format!(
            "{}\n\n[REPAIR INSTRUCTION - HIGH PRIORITY]\nThe previous draft repeated earlier narration or contradicted tracked object state. Do not restate the room setup, clothing, object list, door state, or previous physical arrangement unless changed. Use at most one short anchor detail, then advance the scene from the latest user input. Do not reuse previous wording, opening beats, or setup inventory. If an object state says phone notifications, vibration, or screen wake are disabled, do not write a chime, buzz, vibration, or screen light-up; the message can only arrive silently or be noticed when checked.",
            last_user.content.trim()
        );
    }
    repaired
}

fn last_user_message_content(messages: &[ApiMessage]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.content.clone())
        .unwrap_or_default()
}

fn ensure_default_entities(
    conn: &Connection,
    conversation_id: &str,
    soul: &Soul,
) -> rusqlite::Result<()> {
    let active_persona = db::get_active_player_persona(conn, conversation_id)?;
    let default_player = EntityRecord {
        entity_id: "default_player".into(),
        conversation_id: conversation_id.into(),
        display_name: "User".into(),
        aliases: vec!["user".into(), "player".into(), "operator".into()],
        kind: "operator".into(),
        controlled_by: "user".into(),
        linked_soul_id: None,
        active_in_scene: true,
        created_at: 0,
        updated_at: 0,
    };
    db::upsert_entity(conn, &default_player)?;

    let player_persona = EntityRecord {
        entity_id: active_persona.persona_id.clone(),
        conversation_id: conversation_id.into(),
        display_name: active_persona.display_name,
        aliases: vec![
            active_persona.persona_id.clone(),
            "user".into(),
            "player".into(),
            "active player persona".into(),
        ],
        kind: "player_persona".into(),
        controlled_by: "user".into(),
        linked_soul_id: None,
        active_in_scene: true,
        created_at: 0,
        updated_at: 0,
    };
    db::upsert_entity(conn, &player_persona)?;

    let soul_entity = EntityRecord {
        entity_id: normalize_entity_id(&soul.character_name),
        conversation_id: conversation_id.into(),
        display_name: soul.character_name.clone(),
        aliases: vec![soul.character_name.clone()],
        kind: "soul".into(),
        controlled_by: "narrator".into(),
        linked_soul_id: Some(soul.character_id.clone()),
        active_in_scene: true,
        created_at: 0,
        updated_at: 0,
    };
    db::upsert_entity(conn, &soul_entity)?;
    Ok(())
}

fn resolve_speaker_for_turn(
    conn: &Connection,
    conversation_id: &str,
    soul: &Soul,
    user_text: &str,
) -> rusqlite::Result<EntityTurnContext> {
    ensure_default_entities(conn, conversation_id, soul)?;
    let label = extract_latest_speaker_label(user_text);
    let mut entities = db::list_entities(conn, conversation_id)?;
    let speaker = match label {
        None => default_speaker_resolution(conn, conversation_id),
        Some(label) => resolve_speaker_label(conn, conversation_id, &mut entities, &label)?,
    };
    entities = db::list_entities(conn, conversation_id)?;
    Ok(EntityTurnContext { entities, speaker })
}

/// Speaker for an unlabeled user turn. Defaults to the active player persona
/// (e.g. `preset_male`) so the evaluator's user message attributes the turn to
/// the same entity the system prompt and relationship context use — not the
/// generic `default_player`, which leaked a conflicting attribution.
fn default_speaker_resolution(conn: &Connection, conversation_id: &str) -> SpeakerResolution {
    let (entity_id, display_name) = db::get_active_player_persona(conn, conversation_id)
        .ok()
        .map(|persona| (persona.persona_id, persona.display_name))
        .unwrap_or_else(|| ("default_player".into(), "User".into()));
    SpeakerResolution {
        label: None,
        entity_id,
        display_name,
        status: SpeakerResolutionStatus::NoLabel,
        candidates: Vec::new(),
    }
}

fn resolve_speaker_label(
    conn: &Connection,
    conversation_id: &str,
    entities: &mut [EntityRecord],
    label: &str,
) -> rusqlite::Result<SpeakerResolution> {
    if is_ooc_channel_label(label) {
        return Ok(SpeakerResolution {
            label: Some(label.to_string()),
            entity_id: "default_player".into(),
            display_name: "User".into(),
            status: SpeakerResolutionStatus::NoLabel,
            candidates: Vec::new(),
        });
    }

    if let Some(entity) = exact_entity_match(entities, label) {
        return Ok(SpeakerResolution {
            label: Some(label.to_string()),
            entity_id: entity.entity_id.clone(),
            display_name: entity.display_name.clone(),
            status: SpeakerResolutionStatus::Exact,
            candidates: Vec::new(),
        });
    }

    if let Some(match_result) = best_fuzzy_entity_match(entities, label, true) {
        if match_result.ambiguous {
            return Ok(ambiguous_resolution(label, match_result.candidates));
        }
        let entity = match_result.entity;
        let updated = db::add_entity_alias(conn, conversation_id, &entity.entity_id, label)?;
        return Ok(SpeakerResolution {
            label: Some(label.to_string()),
            entity_id: updated.entity_id,
            display_name: updated.display_name,
            status: SpeakerResolutionStatus::FuzzyTypo,
            candidates: Vec::new(),
        });
    }

    if let Some(match_result) = best_fuzzy_entity_match(entities, label, false) {
        if match_result.ambiguous {
            return Ok(ambiguous_resolution(label, match_result.candidates));
        }
        let entity = match_result.entity;
        let updated = db::add_entity_alias(conn, conversation_id, &entity.entity_id, label)?;
        return Ok(SpeakerResolution {
            label: Some(label.to_string()),
            entity_id: updated.entity_id,
            display_name: updated.display_name,
            status: SpeakerResolutionStatus::FuzzyTypo,
            candidates: Vec::new(),
        });
    }

    if label_can_create_entity(label) {
        let entity = EntityRecord {
            entity_id: normalize_entity_id(label),
            conversation_id: conversation_id.into(),
            display_name: label.trim().to_string(),
            aliases: vec![label.trim().to_string()],
            kind: "user_controlled".into(),
            controlled_by: "user".into(),
            linked_soul_id: None,
            active_in_scene: true,
            created_at: 0,
            updated_at: 0,
        };
        let entity = db::upsert_entity(conn, &entity)?;
        return Ok(SpeakerResolution {
            label: Some(label.to_string()),
            entity_id: entity.entity_id,
            display_name: entity.display_name,
            status: SpeakerResolutionStatus::Created,
            candidates: Vec::new(),
        });
    }

    Ok(SpeakerResolution {
        label: Some(label.to_string()),
        entity_id: "unknown_speaker".into(),
        display_name: "Unknown speaker".into(),
        status: SpeakerResolutionStatus::Unknown,
        candidates: Vec::new(),
    })
}

#[derive(Debug)]
struct FuzzyEntityMatch {
    entity: EntityRecord,
    ambiguous: bool,
    candidates: Vec<String>,
}

fn best_fuzzy_entity_match(
    entities: &[EntityRecord],
    label: &str,
    active_only: bool,
) -> Option<FuzzyEntityMatch> {
    let normalized_label = normalize_match_key(label);
    if normalized_label.len() < 2 {
        return None;
    }
    let mut scored = entities
        .iter()
        .filter(|entity| !active_only || entity.active_in_scene)
        .filter_map(|entity| {
            best_entity_score(entity, &normalized_label)
                .filter(|score| fuzzy_score_is_close(normalized_label.len(), *score))
                .map(|score| (entity.clone(), score))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (entity, score) = scored.first()?.clone();
    let candidates = scored
        .iter()
        .take(3)
        .map(|(entity, _)| entity.display_name.clone())
        .collect::<Vec<_>>();
    let ambiguous = scored
        .get(1)
        .map(|(_, second_score)| (score - *second_score).abs() < 0.08)
        .unwrap_or(false);
    Some(FuzzyEntityMatch {
        entity,
        ambiguous,
        candidates,
    })
}

fn exact_entity_match<'a>(entities: &'a [EntityRecord], label: &str) -> Option<&'a EntityRecord> {
    let trimmed = label.trim();
    entities.iter().find(|entity| {
        entity.entity_id == trimmed
            || entity.display_name == trimmed
            || entity.aliases.iter().any(|alias| alias == trimmed)
            || entity.entity_id.eq_ignore_ascii_case(trimmed)
            || entity.display_name.eq_ignore_ascii_case(trimmed)
            || entity
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(trimmed))
    })
}

fn best_entity_score(entity: &EntityRecord, normalized_label: &str) -> Option<f32> {
    let mut keys = vec![
        normalize_match_key(&entity.entity_id),
        normalize_match_key(&entity.display_name),
    ];
    keys.extend(
        entity
            .aliases
            .iter()
            .map(|alias| normalize_match_key(alias)),
    );
    keys.into_iter()
        .filter(|key| !key.is_empty())
        .map(|key| similarity_score(normalized_label, &key))
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
}

fn fuzzy_score_is_close(label_len: usize, score: f32) -> bool {
    if label_len <= 4 {
        score >= 0.66
    } else {
        score >= 0.82
    }
}

fn ambiguous_resolution(label: &str, candidates: Vec<String>) -> SpeakerResolution {
    SpeakerResolution {
        label: Some(label.to_string()),
        entity_id: "unknown_speaker".into(),
        display_name: "Unknown speaker".into(),
        status: SpeakerResolutionStatus::Ambiguous,
        candidates,
    }
}

fn extract_latest_speaker_label(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .filter_map(extract_speaker_label_from_line)
        .next()
}

fn extract_speaker_label_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let colon_index = trimmed.find(':')?;
    if colon_index == 0 || colon_index > 48 {
        return None;
    }
    let label = trimmed[..colon_index].trim();
    if label_can_be_speaker(label) {
        Some(label.to_string())
    } else {
        None
    }
}

fn label_can_be_speaker(label: &str) -> bool {
    let label = label.trim();
    !label.is_empty()
        && label.chars().any(char::is_alphabetic)
        && label.chars().all(|character| {
            character.is_alphanumeric()
                || character.is_whitespace()
                || matches!(character, '_' | '-' | '\'' | '.')
        })
}

fn label_can_create_entity(label: &str) -> bool {
    let normalized = normalize_match_key(label);
    normalized.len() >= 2
        && normalized.len() <= 40
        && !matches!(
            normalized.as_str(),
            "i" | "me"
                | "we"
                | "he"
                | "she"
                | "they"
                | "system"
                | "assistant"
                | "narrator"
                | "ooc"
                | "out_of_character"
                | "out_of_character_note"
        )
}

fn is_ooc_channel_label(label: &str) -> bool {
    matches!(
        normalize_match_key(label).as_str(),
        "ooc" | "out_of_character" | "out_of_character_note"
    )
}

fn normalize_entity_id(label: &str) -> String {
    let normalized = normalize_match_key(label);
    if normalized == "user" || normalized == "player" || normalized == "operator" {
        "default_player".into()
    } else if normalized.is_empty() {
        "unknown_speaker".into()
    } else {
        normalized
    }
}

fn normalize_match_key(label: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;
    for character in label.trim().chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }
    normalized.trim_matches('_').to_string()
}

fn similarity_score(left: &str, right: &str) -> f32 {
    if left == right {
        return 1.0;
    }
    let max_len = left.chars().count().max(right.chars().count());
    if max_len == 0 {
        return 0.0;
    }
    let distance = levenshtein(left, right);
    (1.0 - (distance as f32 / max_len as f32)).clamp(0.0, 1.0)
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];
    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution = if left_char == *right_char { 0 } else { 1 };
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution);
        }
        previous.clone_from(&current);
    }
    previous[right_chars.len()]
}

fn build_entity_updater_context(soul: &Soul, context: &EntityTurnContext) -> String {
    let include_operator = context.speaker.entity_id == "default_player";
    let active_entities = context
        .entities
        .iter()
        .filter(|entity| include_operator || entity.entity_id != "default_player")
        .filter(|entity| entity.active_in_scene)
        .map(|entity| {
            format!(
                "- {} ({}) kind={}, controlled_by={}",
                entity.entity_id, entity.display_name, entity.kind, entity.controlled_by
            )
        })
        .collect::<Vec<_>>();
    let mut relationship_lines = context
        .entities
        .iter()
        .filter(|entity| include_operator || entity.entity_id != "default_player")
        .filter(|entity| entity.kind != "soul")
        .filter_map(|entity| {
            relationship_for_entity(soul, entity, include_operator).map(|relationship| {
                format!(
                    "{} -> {} ({}): trust {:.0}, affection {:.0}, fear {:.0}, desire {:.0}, conflict {:.0}, curiosity {:.0}, comfort {:.0}, dependency {:.0}",
                    soul.character_name,
                    entity.display_name,
                    entity.entity_id,
                    relationship.trust,
                    relationship.affection,
                    relationship.fear,
                    relationship.desire,
                    relationship.conflict,
                    relationship.curiosity,
                    relationship.comfort,
                    relationship.dependency
                )
            })
        })
        .collect::<Vec<_>>();
    relationship_lines.sort();
    relationship_lines.dedup();

    format!(
        "[ACTIVE ENTITIES]\n{}\n\n[LATEST SPEAKER ENTITY]\n{}\n\n[RELEVANT RELATIONSHIPS]\n{}",
        if active_entities.is_empty() {
            "None".into()
        } else {
            active_entities.join("\n")
        },
        context.speaker.summary_line(),
        if relationship_lines.is_empty() {
            "No directed relationship records for active non-soul entities yet.".into()
        } else {
            relationship_lines.join("\n")
        }
    )
}

fn relationship_for_entity<'a>(
    soul: &'a Soul,
    entity: &EntityRecord,
    include_operator: bool,
) -> Option<&'a state_engine::soul::Relationship> {
    soul.relationships.get(&entity.entity_id).or_else(|| {
        if include_operator && entity.entity_id.eq_ignore_ascii_case("default_player") {
            soul.relationships
                .get("default_player")
                .or_else(|| soul.relationships.get("user"))
        } else {
            None
        }
    })
}

fn default_player_in_evaluator_relationship_context(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    if !lower.contains("default_player") {
        return false;
    }
    for marker in [
        "[relevant relationships]",
        "[current relationships]",
        "relationship",
    ] {
        if let Some(start) = lower.find(marker) {
            let section = &lower[start..lower.len().min(start + 4_000)];
            if section.contains("default_player") {
                return true;
            }
        }
    }
    false
}

impl SpeakerResolution {
    fn summary_line(&self) -> String {
        match self.status {
            SpeakerResolutionStatus::NoLabel => format!(
                "No explicit speaker label; defaulting latest speaker to {} ({}).",
                self.entity_id, self.display_name
            ),
            SpeakerResolutionStatus::Exact => format!(
                "Label {:?} resolved to {} ({}).",
                self.label.as_deref().unwrap_or(""),
                self.entity_id,
                self.display_name
            ),
            SpeakerResolutionStatus::FuzzyTypo => format!(
                "Label {:?} resolved as likely typo/alias for {} ({}).",
                self.label.as_deref().unwrap_or(""),
                self.entity_id,
                self.display_name
            ),
            SpeakerResolutionStatus::Ambiguous => format!(
                "Label {:?} is ambiguous; use unknown_speaker. Candidates: {}.",
                self.label.as_deref().unwrap_or(""),
                self.candidates.join(", ")
            ),
            SpeakerResolutionStatus::Created => format!(
                "Label {:?} created entity {} ({}).",
                self.label.as_deref().unwrap_or(""),
                self.entity_id,
                self.display_name
            ),
            SpeakerResolutionStatus::Unknown => format!(
                "Label {:?} could not be resolved; use unknown_speaker.",
                self.label.as_deref().unwrap_or("")
            ),
        }
    }
}

fn append_memory_v2_evidence_bundle(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    query: &str,
    preview: &mut ContextPreview,
) {
    let Ok(hits) = db::recall_memory_v2(conn, conversation_id, branch_id, query, 6) else {
        return;
    };
    if hits.is_empty() {
        return;
    }
    let mut lines = vec![
        "[MEMORY EVIDENCE BUNDLE]".to_string(),
        "Use these as scoped recollections, not unconditional world truth. Derived memories are interpretations and each item includes why it was selected.".to_string(),
    ];
    for hit in hits {
        let direct_evidence = hit
            .memory
            .source_quote
            .as_deref()
            .filter(|quote| !quote.trim().is_empty())
            .map(|quote| format!(" evidence={quote:?}"))
            .unwrap_or_default();
        let evidence_refs =
            serde_json::from_str::<Vec<state_engine::memory_v2::MemoryEvidenceRef>>(
                &hit.memory.supporting_evidence_json,
            )
            .unwrap_or_default()
            .into_iter()
            .chain(
                serde_json::from_str::<Vec<state_engine::memory_v2::MemoryEvidenceRef>>(
                    &hit.memory.contradicting_evidence_json,
                )
                .unwrap_or_default(),
            )
            .take(4)
            .map(|evidence| {
                format!(
                    "{}:{}{}",
                    evidence.relation,
                    evidence.source_memory_id,
                    evidence
                        .source_quote
                        .filter(|quote| !quote.trim().is_empty())
                        .map(|quote| format!(":{quote:?}"))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>();
        let evidence = if evidence_refs.is_empty() {
            direct_evidence
        } else {
            format!(
                " evidence_refs=[{}]{}",
                evidence_refs.join(", "),
                direct_evidence
            )
        };
        lines.push(format!(
            "- id={} layer={} type={} truth={} valid={} created_at_ms={} score={:.3} \
             trace=lex:{:.3}/semantic:{:.3}/temporal:{:.3}/graph:{:.3} reason={} content={:?}{}",
            hit.memory.memory_id,
            hit.memory.layer,
            hit.memory.memory_kind,
            hit.memory.truth_status,
            hit.memory.validity,
            hit.memory.created_at_ms,
            hit.final_score,
            hit.lexical_score,
            hit.semantic_score,
            hit.temporal_score,
            hit.graph_score,
            hit.selection_reasons.join("+"),
            hit.memory.content,
            evidence
        ));
    }
    preview.text = format!("{}\n\n{}", lines.join("\n"), preview.text);
    preview.estimated_tokens = estimate_tokens(&preview.text);
}

#[allow(clippy::too_many_arguments)]
fn compile_context_with_correction(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
    correction_instruction: Option<&str>,
    pending_user_text: Option<&str>,
    player_persona: Option<&PlayerPersonaContext>,
    context_max_tokens: Option<usize>,
) -> ContextPreview {
    let budget = state_engine::context_compiler::budget_with_max_tokens(context_max_tokens);
    let mut preview =
        state_engine::context_compiler::compile_context_for_session_separate_user_message_with_player_persona_pending_and_budget(
            soul,
            session_world,
            messages,
            pending_user_text,
            player_persona,
            &budget,
        );
    let instruction = correction_instruction
        .map(str::trim)
        .filter(|instruction| !instruction.is_empty());
    if let Some(instruction) = instruction {
        preview.text = format!(
            "[FIX INSTRUCTION, TEMPORARY HIGH PRIORITY]\nApply this only while generating the next narrator response. Do not store it as memory.\nInstruction: {instruction}\n\n{}",
            preview.text
        );
        preview.estimated_tokens = estimate_tokens(&preview.text);
        preview.truncated = true;
    }
    preview
}

fn build_user_text_with_correction(
    user_text: &str,
    correction_instruction: Option<&str>,
) -> String {
    let instruction = correction_instruction
        .map(str::trim)
        .filter(|instruction| !instruction.is_empty());
    if let Some(instruction) = instruction {
        format!("{user_text}\n\n[FIX INSTRUCTION - APPLY TO THIS RESPONSE ONLY]\n{instruction}")
    } else {
        user_text.to_string()
    }
}

#[cfg(test)]
fn build_state_updater_user_message(
    user_text: &str,
    narrator_response: &str,
    entity_context: Option<&str>,
    memory_debug_nonce: Option<&str>,
) -> String {
    let entity_context = entity_context
        .map(str::trim)
        .filter(|context| !context.is_empty())
        .map(|context| format!("{context}\n\n"))
        .unwrap_or_default();
    let compact_user = compact_user_message_for_updater(user_text);
    let compact_narrator = compact_narrator_response_for_updater(narrator_response);
    let debug_section = memory_debug_nonce
        .map(str::trim)
        .filter(|nonce| !nonce.is_empty())
        .map(|nonce| {
            format!("\n\n[VERIFIED MEMORY LAYER DEBUG]\nbackend_nonce: {nonce}\nIf producing memory_layer_reply, echo backend_nonce exactly in memory_layer_reply.nonce. Do not put this nonce in normal memories or world_patch.")
        })
        .unwrap_or_default();
    format!(
        "{}[LATEST USER MESSAGE]\n{}\n\n[NARRATOR RESPONSE]\n{}",
        entity_context, compact_user, compact_narrator
    ) + &debug_section
}

fn build_evaluator_user_message(
    user_text: &str,
    narrator_response: &str,
    recent_chat_excerpt: &str,
    session_world: Option<&SessionWorld>,
    entity_context: Option<&str>,
    memory_debug_nonce: Option<&str>,
) -> String {
    let entity_context = entity_context
        .map(str::trim)
        .filter(|context| !context.is_empty())
        .map(|context| format!("{context}\n\n"))
        .unwrap_or_default();
    let compact_user = compact_user_message_for_updater(user_text);
    let compact_narrator = compact_narrator_response_for_updater(narrator_response);
    let scene_state = session_world
        .map(|world| serde_json::to_string_pretty(&world.scene_state).unwrap_or_default())
        .unwrap_or_else(|| "{}".into());
    let world_objects = session_world
        .map(|world| serde_json::to_string_pretty(&world.object_states).unwrap_or_default())
        .unwrap_or_else(|| "[]".into());
    let debug_section = memory_debug_nonce
        .map(str::trim)
        .filter(|nonce| !nonce.is_empty())
        .map(|nonce| {
            format!("\n\n[VERIFIED MEMORY LAYER DEBUG]\nbackend_nonce: {nonce}\nEvaluatorOutputV1 has no memory_layer_reply field; do not put this nonce in memory candidates.")
        })
        .unwrap_or_default();
    format!(
        "{}[PRIOR SCENE_STATE]\n{}\n\n[CURRENT WORLD/OBJECT STATE]\n{}\n\n[RECENT CHAT EXCERPT]\n{}\n\n[LATEST USER MESSAGE]\n{}\n\n[LATEST NARRATOR RESPONSE]\n{}{}",
        entity_context,
        scene_state,
        world_objects,
        head_tail_excerpt_chars(recent_chat_excerpt.trim(), 900, 900, 2_000),
        compact_user,
        compact_narrator,
        debug_section
    )
}

fn compact_user_message_for_updater(user_text: &str) -> String {
    let trimmed = user_text.trim();
    if looks_like_imported_log_text(&trimmed.to_ascii_lowercase()) {
        return format!(
            "[IMPORTED LOG DETECTED]\nThe user pasted a Mnemosyne/exported chat log. Treat it as imported context, not current lived experience. Create at most 1-3 imported_log memories if durable facts matter.\nExcerpt:\n{}",
            head_tail_excerpt_chars(trimmed, 700, 500, 1_300)
        );
    }
    head_tail_excerpt_chars(trimmed, 900, 700, 1_700)
}

fn compact_narrator_response_for_updater(narrator_response: &str) -> String {
    let visible = strip_status_blocks_for_export(&strip_hidden_state_blocks(narrator_response));
    head_tail_excerpt_chars(visible.trim(), 500, 1_300, 1_900)
}

fn head_tail_excerpt_chars(
    text: &str,
    head_chars: usize,
    tail_chars: usize,
    max_chars: usize,
) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    let head = chars.iter().take(head_chars).collect::<String>();
    let tail = chars
        .iter()
        .rev()
        .take(tail_chars)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}\n...[truncated for updater budget]...\n{tail}")
}

#[cfg(test)]
fn build_compact_updater_payload_for_test(
    soul: &Soul,
    user_text: &str,
    narrator_response: &str,
) -> String {
    format!(
        "{}\n\n{}",
        crate::providers::api::build_state_updater_prompt(soul, None),
        build_state_updater_user_message(user_text, narrator_response, None, None)
    )
}

#[cfg(test)]
fn parse_engine_patch_json(raw: &str) -> Result<EnginePatch, String> {
    let trimmed = raw.trim();
    let json = if let Some(stripped) = trimmed.strip_prefix("```json") {
        stripped.trim_end_matches("```").trim()
    } else if let Some(stripped) = trimmed.strip_prefix("```") {
        stripped.trim_end_matches("```").trim()
    } else {
        trimmed
    };
    serde_json::from_str::<EnginePatch>(json)
        .map_err(|err| format!("State updater returned invalid EnginePatch JSON: {err}"))
}

#[derive(Debug, Clone, Default)]
struct EvaluatorGateOutcome {
    waited_ms: u64,
    stale_state_send: bool,
    compiled_with_pending_evaluator: bool,
    pending_job_ids: Vec<String>,
}

fn evaluator_timeout_mode(settings: &ApiProviderSettings) -> String {
    match settings.evaluator_timeout_mode.as_deref() {
        Some("no_app_timeout") => "no_app_timeout".into(),
        _ => "finite".into(),
    }
}

fn evaluator_mode(settings: &ApiProviderSettings) -> String {
    match settings.evaluator_mode.as_deref() {
        Some(EVALUATOR_MODE_V1) => EVALUATOR_MODE_V1.into(),
        Some(EVALUATOR_MODE_FORM_V1) => EVALUATOR_MODE_FORM_V1.into(),
        Some(EVALUATOR_MODE_STRUCTURED_V1) => EVALUATOR_MODE_STRUCTURED_V1.into(),
        Some(EVALUATOR_MODE_PERCEPTION_V2) => EVALUATOR_MODE_PERCEPTION_V2.into(),
        Some(EVALUATOR_MODE_DUAL_COMPARE) => EVALUATOR_MODE_DUAL_COMPARE.into(),
        _ => EVALUATOR_MODE_FORM_V1.into(),
    }
}

fn selected_evaluator_source(mode: &str) -> &'static str {
    if mode == EVALUATOR_MODE_FORM_V1 || mode == EVALUATOR_MODE_DUAL_COMPARE {
        EVALUATOR_MODE_FORM_V1
    } else if mode == EVALUATOR_MODE_STRUCTURED_V1 {
        EVALUATOR_MODE_STRUCTURED_V1
    } else if mode == EVALUATOR_MODE_PERCEPTION_V2 {
        EVALUATOR_MODE_PERCEPTION_V2
    } else {
        EVALUATOR_MODE_V1
    }
}

/// Active evaluator profile for a conversation: the explicitly selected one,
/// falling back to the first unarchived profile matching the settings model
/// (mirrors the background job's profile_id resolution).
fn active_evaluator_profile_for_conversation(
    conn: &Connection,
    conversation_id: &str,
    settings: &ApiProviderSettings,
) -> Option<db::ProviderProfile> {
    if let Ok(conv) = db::get_conversation_summary(conn, conversation_id) {
        if let Some(id) = conv.active_evaluator_profile_id {
            if let Ok(profile) = db::get_provider_profile(conn, &id) {
                return Some(profile);
            }
        }
    }
    let id = conn
        .query_row(
            "SELECT id FROM provider_profiles WHERE archived_at IS NULL AND model = ?1 LIMIT 1",
            [&settings.model],
            |row| row.get::<_, String>(0),
        )
        .ok()?;
    db::get_provider_profile(conn, &id).ok()
}

/// Resolve the raw evaluator-mode setting for this turn. Explicit settings
/// win; otherwise the active profile's stored mode (kept raw so values like
/// "form_v1_compact" survive for the compact-prompt check); otherwise a
/// profile that probed json_schema-level structured support defaults to
/// evaluator_structured_v1. Returns None when nothing overrides the built-in
/// form_v1 default.
fn resolve_evaluator_mode_setting(
    conn: &Connection,
    conversation_id: &str,
    settings: &ApiProviderSettings,
) -> Option<String> {
    if settings.evaluator_mode.is_some() {
        return settings.evaluator_mode.clone();
    }
    let profile = active_evaluator_profile_for_conversation(conn, conversation_id, settings)?;
    if profile.evaluator_mode.is_some() {
        return profile.evaluator_mode;
    }
    if profile.structured_output_support == STRUCTURED_SUPPORT_JSON_SCHEMA {
        return Some(EVALUATOR_MODE_STRUCTURED_V1.into());
    }
    None
}

fn resolve_structured_evaluator_policy_setting(
    conn: &Connection,
    conversation_id: &str,
    settings: &ApiProviderSettings,
) -> Option<String> {
    if settings.structured_evaluator_policy.is_some() {
        return settings.structured_evaluator_policy.clone();
    }
    active_evaluator_profile_for_conversation(conn, conversation_id, settings)?
        .structured_evaluator_policy
}

fn evaluator_provider_label(mode: &str, background: bool) -> String {
    let source = selected_evaluator_source(mode);
    if background {
        format!("{source}_background")
    } else {
        source.to_string()
    }
}

#[derive(Debug, Clone)]
struct RuntimeEvaluatorOutcome {
    output: EvaluatorOutputV1,
    draft: state_engine::evaluator_ingest::NormalizedEvaluationDraft,
    normalized_json: String,
    normalized: bool,
    warnings: Vec<String>,
    conversion: EvaluatorConversionReport,
    form_spec: Option<EvalFormSpec>,
    form_trace: Option<EvalFormTrace>,
    form_rejected_rows: Vec<EvalFormRowRejection>,
    form_response_parse_status: Option<String>,
    comparison_trace: Option<serde_json::Value>,
    partial_success: bool,
    partial_success_reason: Option<String>,
    fallback_path: Vec<String>,
    fallback_warning: Option<String>,
    structured_ops_count: Option<usize>,
    syntactic_repair_used: bool,
    structured_enforcement_requested: Option<String>,
    structured_enforcement_validated: bool,
    structured_schema_validation_status: String,
    structured_schema_validation_error: Option<String>,
    structured_retry_count: usize,
    structured_retry_reasons: Vec<String>,
    structured_retry_succeeded: Option<bool>,
    structured_retry_final_error: Option<String>,
    structured_retry_used_failed_args: bool,
    structured_retry_repair_prompt_included_error: bool,
    entity_aliases_resolved: Vec<String>,
    entity_alias_resolution_warnings: Vec<String>,
    structured_run_classification: String,
    tool_calls_present: bool,
    tool_call_count: usize,
    tool_call_names: Vec<String>,
    raw_content_present: bool,
    raw_tool_calls_present: bool,
}

fn runtime_form_trace_json(outcome: &RuntimeEvaluatorOutcome) -> serde_json::Value {
    if let Some(trace) = outcome.form_trace.as_ref() {
        serde_json::json!({
            "form_spec_generated": outcome.form_spec.is_some(),
            "form_spec_event_option_count": outcome.form_spec.as_ref().map(|spec| spec.allowed_event_types.len()).unwrap_or_default(),
            "form_existing_memory_option_count": outcome.form_spec.as_ref().map(|spec| spec.existing_memories.len()).unwrap_or_default(),
            "form_response_parse_status": outcome.form_response_parse_status.as_deref().unwrap_or("not_applicable"),
            "form_rows_submitted": trace.form_rows_submitted,
            "form_rows_accepted": trace.form_rows_accepted,
            "form_rows_rejected": trace.form_rows_rejected,
            "form_rejected_rows": &outcome.form_rejected_rows,
            "form_dedupe_decisions": &trace.form_dedupe_decisions,
            "compiled_turn_flags_u64": trace.compiled_turn_flags_u64,
            "code_assigned_decay_profile": &trace.code_assigned_decay_profile,
            "code_assigned_tag_weights": &trace.code_assigned_tag_weights,
            "raw_form_repair_applied": trace.raw_form_repair_applied,
            "raw_form_repair_warnings": &trace.raw_form_repair_warnings,
            "json_extract_status": trace.json_extract_status,
            "strict_parse_failed_but_salvage_attempted": trace.strict_parse_failed_but_salvage_attempted,
            "salvage_success": trace.salvage_success,
            "relationship_dimension_inferred_from": &trace.relationship_dimension_inferred_from,
            "relationship_direction_inferred_from": &trace.relationship_direction_inferred_from,
            "relationship_rows_split_count": trace.relationship_rows_split_count,
            "relationship_row_results": &trace.relationship_row_results,
            "relationship_event_row_results": &trace.relationship_event_row_results,
            "relationship_delta_source": &trace.relationship_delta_source,
            "partial_success": outcome.partial_success,
            "partial_success_reason": outcome.partial_success_reason.as_deref(),
        })
    } else {
        serde_json::json!({
            "form_spec_generated": false,
            "form_spec_event_option_count": 0,
            "form_existing_memory_option_count": 0,
            "form_response_parse_status": outcome.form_response_parse_status.as_deref().unwrap_or("not_applicable"),
            "form_rows_submitted": 0,
            "form_rows_accepted": 0,
            "form_rows_rejected": 0,
            "form_dedupe_decisions": [],
            "compiled_turn_flags_u64": serde_json::Value::Null,
            "code_assigned_decay_profile": {},
            "code_assigned_tag_weights": {},
            "raw_form_repair_applied": false,
            "raw_form_repair_warnings": [],
            "json_extract_status": "not_applicable",
            "strict_parse_failed_but_salvage_attempted": false,
            "salvage_success": false,
            "relationship_dimension_inferred_from": [],
            "relationship_direction_inferred_from": [],
            "relationship_rows_split_count": 0,
            "partial_success": outcome.partial_success,
            "partial_success_reason": outcome.partial_success_reason.as_deref(),
        })
    }
}

fn failed_form_trace_json(
    selected_evaluator_source: &str,
    form_spec: Option<&EvalFormSpec>,
) -> serde_json::Value {
    if selected_evaluator_source == EVALUATOR_MODE_FORM_V1 {
        serde_json::json!({
            "form_spec_generated": form_spec.is_some(),
            "form_spec_event_option_count": form_spec.map(|spec| spec.allowed_event_types.len()).unwrap_or_default(),
            "form_existing_memory_option_count": form_spec.map(|spec| spec.existing_memories.len()).unwrap_or_default(),
            "form_response_parse_status": "failed",
            "form_rows_submitted": 0,
            "form_rows_accepted": 0,
            "form_rows_rejected": 0,
            "form_rejected_rows": [],
            "form_dedupe_decisions": [],
            "compiled_turn_flags_u64": serde_json::Value::Null,
            "code_assigned_decay_profile": {},
            "code_assigned_tag_weights": {},
            "raw_form_repair_applied": false,
            "raw_form_repair_warnings": [],
            "json_extract_status": "failed",
            "strict_parse_failed_but_salvage_attempted": true,
            "salvage_success": false,
        })
    } else {
        serde_json::json!({
            "form_spec_generated": false,
            "form_spec_event_option_count": 0,
            "form_existing_memory_option_count": 0,
            "form_response_parse_status": "not_applicable",
            "form_rows_submitted": 0,
            "form_rows_accepted": 0,
            "form_rows_rejected": 0,
            "form_rejected_rows": [],
            "form_dedupe_decisions": [],
            "compiled_turn_flags_u64": serde_json::Value::Null,
            "code_assigned_decay_profile": {},
            "code_assigned_tag_weights": {},
            "raw_form_repair_applied": false,
            "raw_form_repair_warnings": [],
            "json_extract_status": "not_applicable",
            "strict_parse_failed_but_salvage_attempted": false,
            "salvage_success": false,
        })
    }
}

fn insert_json_object_fields(target: &mut serde_json::Value, fields: &serde_json::Value) {
    if let (serde_json::Value::Object(target), serde_json::Value::Object(fields)) = (target, fields)
    {
        for (key, value) in fields {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn structured_fallback_step(enforcement: Option<StructuredEnforcement>) -> &'static str {
    match enforcement {
        Some(StructuredEnforcement::JsonSchema) => "structured_json_schema",
        Some(StructuredEnforcement::JsonObject) => "structured_json_object",
        Some(StructuredEnforcement::ToolCall) => "structured_tool_call",
        Some(StructuredEnforcement::Grammar) => "structured_grammar",
        Some(StructuredEnforcement::None) | None => "structured_none",
    }
}

fn evaluator_fallback_origin(
    selected_source: &str,
    enforcement: Option<StructuredEnforcement>,
) -> &'static str {
    if selected_source == EVALUATOR_MODE_PERCEPTION_V2 {
        EVALUATOR_MODE_PERCEPTION_V2
    } else {
        structured_fallback_step(enforcement)
    }
}

fn structured_validation_status_from_error(error: &str) -> &'static str {
    let lower = error.to_ascii_lowercase();
    if lower.contains("malformed_schema_output") {
        "malformed_schema_output"
    } else if lower.contains("zero_ops_on_required_reextract") {
        "zero_ops_on_required_reextract"
    } else if lower.contains("zero_ops_on_durable_turn") {
        "zero_ops_on_durable_turn"
    } else if lower.contains("semantic validation failed") {
        "semantic_validation_failed"
    } else if lower.contains("timed out") || lower.contains("timeout") {
        "timeout"
    } else {
        "not_validated"
    }
}

fn evaluator_runtime_fallback_json(outcome: &RuntimeEvaluatorOutcome) -> serde_json::Value {
    serde_json::json!({
        "fallback_path": outcome.fallback_path,
        "fallback_warning": outcome.fallback_warning.as_deref(),
        "structured_schema_version": state_engine::evaluator_structured::EVALUATOR_STRUCTURED_SCHEMA_VERSION,
        "structured_compiler_version": state_engine::evaluator_structured::EVALUATOR_STRUCTURED_COMPILER_VERSION,
        "ops_count": outcome.structured_ops_count.unwrap_or(0),
        "syntactic_repair_used": outcome.syntactic_repair_used,
        "structured_enforcement_requested": outcome.structured_enforcement_requested.as_deref().unwrap_or("none"),
        "structured_enforcement_validated": outcome.structured_enforcement_validated,
        "structured_schema_validation_status": outcome.structured_schema_validation_status.as_str(),
        "structured_schema_validation_error": outcome.structured_schema_validation_error.as_deref(),
        "structured_retry_count": outcome.structured_retry_count,
        "structured_retry_reasons": &outcome.structured_retry_reasons,
        "structured_retry_succeeded": outcome.structured_retry_succeeded,
        "structured_retry_final_error": outcome.structured_retry_final_error.as_deref(),
        "structured_retry_used_failed_args": outcome.structured_retry_used_failed_args,
        "structured_retry_repair_prompt_included_error": outcome.structured_retry_repair_prompt_included_error,
        "entity_aliases_resolved": &outcome.entity_aliases_resolved,
        "entity_alias_resolution_warnings": &outcome.entity_alias_resolution_warnings,
        "structured_run_classification": outcome.structured_run_classification.as_str(),
        "tool_calls_present": outcome.tool_calls_present,
        "tool_call_count": outcome.tool_call_count,
        "tool_call_names": &outcome.tool_call_names,
        "raw_content_present": outcome.raw_content_present,
        "raw_tool_calls_present": outcome.raw_tool_calls_present,
        "structured_transport_requested": outcome.structured_enforcement_requested.as_deref().unwrap_or("none"),
        "structured_transport_actual": outcome.structured_enforcement_requested.as_deref().unwrap_or("none"),
        "strict_tool_diagnostic": false,
        "strict_tool_passed": serde_json::Value::Null,
        "fallback_used": outcome.fallback_path.iter().any(|step| step == EVALUATOR_MODE_FORM_V1 || step == "minimal_scene_patch")
    })
}

#[allow(clippy::too_many_arguments)]
fn compile_selected_evaluator_runtime(
    evaluator_mode: &str,
    form_spec: Option<EvalFormSpec>,
    raw_response: &str,
    structured_enforcement: Option<StructuredEnforcement>,
    soul: &Soul,
    session_world: &SessionWorld,
    latest_user_message: &str,
    latest_narrator_response: &str,
    baseline_recent_event_id: Option<String>,
    require_nonempty_ops: bool,
) -> Result<RuntimeEvaluatorOutcome, String> {
    let source = selected_evaluator_source(evaluator_mode);
    if source == EVALUATOR_MODE_FORM_V1 {
        let spec = form_spec.ok_or_else(|| {
            "Evaluator form runtime selected but EvalFormSpec was not generated".to_string()
        })?;
        compile_evaluator_form_runtime(
            raw_response,
            spec,
            soul,
            session_world,
            latest_user_message,
            latest_narrator_response,
            baseline_recent_event_id,
        )
    } else if source == EVALUATOR_MODE_STRUCTURED_V1 {
        compile_evaluator_structured_runtime(
            raw_response,
            structured_enforcement,
            soul,
            session_world,
            latest_user_message,
            latest_narrator_response,
            baseline_recent_event_id,
            require_nonempty_ops,
        )
    } else {
        compile_evaluator_v1_runtime(
            raw_response,
            soul,
            session_world,
            latest_user_message,
            latest_narrator_response,
            baseline_recent_event_id,
        )
    }
}

#[allow(clippy::too_many_arguments)]
async fn complete_form_fallback_runtime(
    provider: &ApiProvider,
    settings: &ApiProviderSettings,
    form_system_prompt: &str,
    user_message: &str,
    form_spec: Option<EvalFormSpec>,
    soul: &Soul,
    session_world: &SessionWorld,
    latest_user_message: &str,
    latest_narrator_response: &str,
    baseline_recent_event_id: Option<String>,
    prior_path: Vec<String>,
    structured_failure: String,
) -> (Result<RuntimeEvaluatorOutcome, String>, Option<String>) {
    let Some(spec) = form_spec else {
        return (
            Err("Evaluator form fallback requested but EvalFormSpec was not generated".into()),
            None,
        );
    };
    let timeout = effective_evaluator_timeout_ms(settings).map(Duration::from_millis);
    let completion = match provider
        .complete_prompt_with_usage(settings, form_system_prompt, user_message, 0.0, timeout)
        .await
    {
        Ok(completion) => completion,
        Err(err) => return (Err(err), None),
    };
    let raw_response = completion.raw_text.clone();
    let outcome = compile_evaluator_form_runtime_strict(
        &completion.raw_text,
        spec,
        soul,
        session_world,
        latest_user_message,
        latest_narrator_response,
        baseline_recent_event_id,
    )
    .map(|mut outcome| {
        outcome.fallback_path = {
            let mut path = prior_path;
            path.push(EVALUATOR_MODE_FORM_V1.to_string());
            path
        };
        outcome.fallback_warning = Some(format!(
            "structured evaluator failed; evaluator_form_v1 fallback used: {structured_failure}"
        ));
        outcome
    });
    (outcome, Some(raw_response))
}

#[allow(clippy::too_many_arguments)]
fn compile_evaluator_form_runtime_strict(
    raw_response: &str,
    spec: EvalFormSpec,
    soul: &Soul,
    session_world: &SessionWorld,
    latest_user_message: &str,
    latest_narrator_response: &str,
    baseline_recent_event_id: Option<String>,
) -> Result<RuntimeEvaluatorOutcome, String> {
    let (form_response, repair_trace) = parse_eval_form_response_with_trace(raw_response)?;
    let compiled = compile_eval_form_response(
        &spec,
        &form_response,
        &EvaluatorConversionContext {
            active_soul_id: soul.character_id.as_str(),
            active_soul_ids: active_souls_for_v1(soul),
            latest_user_message,
            latest_narrator_response,
            session_world: Some(session_world),
            baseline_recent_event_id,
        },
    );
    let mut form_trace = compiled.trace;
    form_trace.raw_form_repair_applied = repair_trace.raw_form_repair_applied;
    form_trace.raw_form_repair_warnings = repair_trace.raw_form_repair_warnings;
    form_trace.json_extract_status = repair_trace.json_extract_status;
    form_trace.strict_parse_failed_but_salvage_attempted =
        repair_trace.strict_parse_failed_but_salvage_attempted;
    form_trace.salvage_success = repair_trace.salvage_success;
    let syntactic_repair_used = form_trace.raw_form_repair_applied;
    let normalized_json = serde_json::to_string(&compiled.output)
        .map_err(|err| format!("Evaluator form compiled output serialization failed: {err}"))?;
    Ok(RuntimeEvaluatorOutcome {
        output: compiled.output,
        draft: compiled.draft,
        normalized_json,
        normalized: true,
        warnings: compiled
            .rejected_rows
            .iter()
            .map(|row| format!("{} {} rejected: {}", row.row_kind, row.row_id, row.reason))
            .collect(),
        conversion: compiled.conversion,
        form_spec: Some(spec),
        form_trace: Some(form_trace),
        form_rejected_rows: compiled.rejected_rows,
        form_response_parse_status: Some("success".into()),
        comparison_trace: None,
        partial_success: false,
        partial_success_reason: None,
        fallback_path: vec![EVALUATOR_MODE_FORM_V1.to_string()],
        fallback_warning: None,
        structured_ops_count: None,
        syntactic_repair_used,
        structured_enforcement_requested: None,
        structured_enforcement_validated: false,
        structured_schema_validation_status: "not_applicable".into(),
        structured_schema_validation_error: None,
        structured_retry_count: 0,
        structured_retry_reasons: Vec::new(),
        structured_retry_succeeded: None,
        structured_retry_final_error: None,
        structured_retry_used_failed_args: false,
        structured_retry_repair_prompt_included_error: false,
        entity_aliases_resolved: Vec::new(),
        entity_alias_resolution_warnings: Vec::new(),
        structured_run_classification: "tool_failed_form_fallback_success".into(),
        tool_calls_present: false,
        tool_call_count: 0,
        tool_call_names: Vec::new(),
        raw_content_present: false,
        raw_tool_calls_present: false,
    })
}

fn evaluator_noop_after_all_fallbacks(
    prior_path: Vec<String>,
    structured_failure: String,
    form_failure: String,
) -> RuntimeEvaluatorOutcome {
    RuntimeEvaluatorOutcome {
        output: EvaluatorOutputV1 {
            schema_version: EVALUATOR_SCHEMA_VERSION,
            no_op_reason: Some(format!(
                "structured evaluator failed ({structured_failure}); evaluator_form_v1 fallback failed ({form_failure})"
            )),
            ..EvaluatorOutputV1::default()
        },
        draft: state_engine::evaluator_ingest::NormalizedEvaluationDraft {
            warnings: vec![
                "all evaluator fallback paths failed; no-op patch recorded".to_string()
            ],
            ..Default::default()
        },
        normalized_json: "{}".into(),
        normalized: false,
        warnings: vec![
            "all evaluator fallback paths failed; no-op patch recorded".to_string()
        ],
        conversion: EvaluatorConversionReport {
            patch: EnginePatch::default(),
            no_op: true,
            ..EvaluatorConversionReport::default()
        },
        form_spec: None,
        form_trace: None,
        form_rejected_rows: Vec::new(),
        form_response_parse_status: Some("failed".into()),
        comparison_trace: None,
        partial_success: true,
        partial_success_reason: Some("all evaluator fallback paths failed; no-op patch recorded".into()),
        fallback_path: {
            let mut path = prior_path;
            path.push(EVALUATOR_MODE_FORM_V1.to_string());
            path.push("noop_after_all_fallbacks".to_string());
            path
        },
        fallback_warning: Some("all evaluator fallback paths failed; no-op patch recorded".into()),
        structured_ops_count: Some(0),
        syntactic_repair_used: false,
        structured_enforcement_requested: None,
        structured_enforcement_validated: false,
        structured_schema_validation_status: "not_validated".into(),
        structured_schema_validation_error: Some(format!(
            "structured_failure={structured_failure}; form_failure={form_failure}"
        )),
        structured_retry_count: 0,
        structured_retry_reasons: Vec::new(),
        structured_retry_succeeded: None,
        structured_retry_final_error: None,
        structured_retry_used_failed_args: false,
        structured_retry_repair_prompt_included_error: false,
        entity_aliases_resolved: Vec::new(),
        entity_alias_resolution_warnings: Vec::new(),
        structured_run_classification: "tool_failed_noop".into(),
        tool_calls_present: false,
        tool_call_count: 0,
        tool_call_names: Vec::new(),
        raw_content_present: false,
        raw_tool_calls_present: false,
    }
}

fn strict_tool_diagnostic_failed_outcome(
    fallback_path: Vec<String>,
    structured_failure: String,
) -> RuntimeEvaluatorOutcome {
    RuntimeEvaluatorOutcome {
        output: EvaluatorOutputV1 {
            schema_version: EVALUATOR_SCHEMA_VERSION,
            no_op_reason: Some(format!(
                "strict tool-call diagnostic failed without fallback: {structured_failure}"
            )),
            ..EvaluatorOutputV1::default()
        },
        draft: state_engine::evaluator_ingest::NormalizedEvaluationDraft {
            warnings: vec!["strict tool-call diagnostic failed; fallback forbidden".to_string()],
            ..Default::default()
        },
        normalized_json: "{}".into(),
        normalized: false,
        warnings: vec!["strict tool-call diagnostic failed; fallback forbidden".to_string()],
        conversion: EvaluatorConversionReport {
            patch: EnginePatch::default(),
            no_op: true,
            ..EvaluatorConversionReport::default()
        },
        form_spec: None,
        form_trace: None,
        form_rejected_rows: Vec::new(),
        form_response_parse_status: None,
        comparison_trace: None,
        partial_success: false,
        partial_success_reason: Some("strict tool-call diagnostic failed".into()),
        fallback_path,
        fallback_warning: None,
        structured_ops_count: Some(0),
        syntactic_repair_used: false,
        structured_enforcement_requested: Some(StructuredEnforcement::ToolCall.as_label().into()),
        structured_enforcement_validated: false,
        structured_schema_validation_status: structured_validation_status_from_error(
            &structured_failure,
        )
        .into(),
        structured_schema_validation_error: Some(structured_failure),
        structured_retry_count: 0,
        structured_retry_reasons: Vec::new(),
        structured_retry_succeeded: None,
        structured_retry_final_error: None,
        structured_retry_used_failed_args: false,
        structured_retry_repair_prompt_included_error: false,
        entity_aliases_resolved: Vec::new(),
        entity_alias_resolution_warnings: Vec::new(),
        structured_run_classification: "strict_failed".into(),
        tool_calls_present: false,
        tool_call_count: 0,
        tool_call_names: Vec::new(),
        raw_content_present: false,
        raw_tool_calls_present: false,
    }
}

/// Runtime for `evaluator_structured_v1`: the model returns compact evaluator
/// ops under provider enforcement. Rust parses, validates evidence/entities,
/// and compiles those operations into an EnginePatch for the normal ledger path.
/// Schema-enforced parse failures are contract breaks, so no syntactic repair
/// path is attempted.
fn compile_evaluator_structured_runtime(
    raw_response: &str,
    structured_enforcement: Option<StructuredEnforcement>,
    soul: &Soul,
    session_world: &SessionWorld,
    latest_user_message: &str,
    latest_narrator_response: &str,
    baseline_recent_event_id: Option<String>,
    require_nonempty_ops: bool,
) -> Result<RuntimeEvaluatorOutcome, String> {
    let enforcement_label = structured_enforcement
        .map(StructuredEnforcement::as_label)
        .unwrap_or("none")
        .to_string();
    let strict_parse = serde_json::from_str::<EvaluatorStructuredOutputV1>(raw_response.trim());
    let (ops_output, normalized, warnings) = match strict_parse {
        Ok(output) => (output, false, Vec::new()),
        Err(err) if structured_enforcement == Some(StructuredEnforcement::JsonSchema) => {
            return Err(format!(
                "malformed_schema_output: Structured evaluator returned schema-enforced output that failed strict parse: {err}"
            ));
        }
        Err(err) => {
            return Err(format!(
                "malformed_schema_output: Structured evaluator ops parse failed without repair fallback: {err}"
            ));
        }
    };
    if ops_output.ops.is_empty() {
        if require_nonempty_ops {
            return Err(
                "zero_ops_on_required_reextract: repair/re-extraction returned empty ops; \
                 non-empty enrichment is required"
                    .into(),
            );
        }
        if durable_change_required(latest_user_message, latest_narrator_response).is_some()
            && !meaningful_no_op_reason(ops_output.no_op_reason.as_deref())
        {
            return Err(
                "zero_ops_on_durable_turn: structured evaluator returned empty ops for durable latest exchange"
                    .into(),
            );
        }
    }
    let context = EvaluatorConversionContext {
        active_soul_id: soul.character_id.as_str(),
        active_soul_ids: active_souls_for_v1(soul),
        latest_user_message,
        latest_narrator_response,
        session_world: Some(session_world),
        baseline_recent_event_id,
    };
    let conversion = compile_evaluator_ops_to_engine_patch(&ops_output, &context, soul)
        .map_err(|err| format!("Structured evaluator semantic validation failed: {err}"))?;
    let entity_aliases_resolved = conversion.entity_aliases_resolved.clone();
    let entity_alias_resolution_warnings = conversion.entity_alias_resolution_warnings.clone();
    let normalized_json = serde_json::to_string(&ops_output)
        .map_err(|err| format!("Structured evaluator ops serialization failed: {err}"))?;
    Ok(RuntimeEvaluatorOutcome {
        output: EvaluatorOutputV1::default(),
        draft: state_engine::evaluator_ingest::NormalizedEvaluationDraft::default(),
        normalized_json,
        normalized,
        warnings,
        conversion,
        form_spec: None,
        form_trace: None,
        form_rejected_rows: Vec::new(),
        form_response_parse_status: None,
        comparison_trace: None,
        partial_success: false,
        partial_success_reason: None,
        fallback_path: vec![structured_fallback_step(structured_enforcement).to_string()],
        fallback_warning: None,
        structured_ops_count: Some(ops_output.ops.len()),
        syntactic_repair_used: false,
        structured_enforcement_requested: Some(enforcement_label),
        structured_enforcement_validated: true,
        structured_schema_validation_status: "validated".into(),
        structured_schema_validation_error: None,
        structured_retry_count: 0,
        structured_retry_reasons: Vec::new(),
        structured_retry_succeeded: None,
        structured_retry_final_error: None,
        structured_retry_used_failed_args: false,
        structured_retry_repair_prompt_included_error: false,
        entity_aliases_resolved,
        entity_alias_resolution_warnings,
        structured_run_classification: "pure_tool_success".into(),
        tool_calls_present: false,
        tool_call_count: 0,
        tool_call_names: Vec::new(),
        raw_content_present: false,
        raw_tool_calls_present: false,
    })
}

fn compile_perception_v2_shadow_runtime(
    raw_response: &str,
    source: &SourceEnvelope,
    producer: ModelProvenance,
) -> Result<PerceptionBatch, String> {
    let draft = serde_json::from_str::<PerceptionBatchDraft>(raw_response.trim())
        .map_err(|error| format!("Perception V2 strict parse failed: {error}"))?;
    seal_perception_batch(source, draft, producer)
        .map_err(|error| format!("Perception V2 sealing failed: {error}"))
}

#[allow(clippy::too_many_arguments)]
fn production_perception_source(
    conversation_id: &str,
    branch_id: Option<&str>,
    turn_id: Option<&str>,
    parent_turn_id: Option<&str>,
    user_message_id: Option<i64>,
    assistant_message_id: i64,
    assistant_variant_id: Option<i64>,
    active_soul_ids: Vec<String>,
    user_text: &str,
    assistant_text: &str,
) -> Result<SourceEnvelope, String> {
    SourceEnvelope::new(
        SourceIdentity {
            conversation_id: conversation_id.to_string(),
            branch_id: branch_id
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Perception V2 requires an active ledger branch".to_string())?
                .to_string(),
            turn_id: turn_id
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Perception V2 requires an engine-owned turn id".to_string())?
                .to_string(),
            parent_turn_id: parent_turn_id.map(str::to_string),
            user_message_id: user_message_id.filter(|value| *value > 0).ok_or_else(|| {
                "Perception V2 requires an engine-owned user message id".to_string()
            })?,
            assistant_message_id,
            assistant_variant_id,
        },
        active_soul_ids,
        user_text,
        assistant_text,
        None,
        db::now_ts().saturating_mul(1000),
    )
    .map_err(|error| format!("Perception V2 source creation failed: {error}"))
}

#[allow(clippy::too_many_arguments)]
fn compile_perception_v2_runtime(
    raw_response: &str,
    structured_enforcement: Option<StructuredEnforcement>,
    source: &SourceEnvelope,
    catalog: EntityCatalog,
    snapshot: &SimulationSnapshot,
    provider_label: String,
    model: &str,
) -> Result<RuntimeEvaluatorOutcome, String> {
    let batch = compile_perception_v2_shadow_runtime(
        raw_response,
        source,
        ModelProvenance {
            provider: provider_label,
            model: model.trim().to_string(),
            prompt_version: PERCEPTION_V2_PROMPT_VERSION.into(),
            schema_name: PERCEPTION_IR_SCHEMA_NAME.into(),
        },
    )?;
    let pipeline = compile_perception_pipeline(source, &batch, catalog, snapshot);
    if pipeline.simulation.decision != SimulationDecision::CommitReady {
        let reasons = pipeline
            .simulation
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "Perception V2 transaction simulation rejected{}",
            (!reasons.is_empty())
                .then(|| format!(": {reasons}"))
                .unwrap_or_default()
        ));
    }
    let patch_lowering = lower_state_effects_to_engine_patch(source, &pipeline.simulation.effects);
    if !patch_lowering.unsupported_effect_ids.is_empty() {
        return Err(format!(
            "Perception V2 contains effects unsupported by the V1 ledger adapter: {}",
            patch_lowering.unsupported_effect_ids.join(", ")
        ));
    }

    let rejected_candidates = pipeline
        .semantic
        .diagnostics
        .iter()
        .filter_map(|diagnostic| {
            diagnostic
                .candidate_id
                .as_ref()
                .map(|candidate_id| EvaluatorCandidateRejection {
                    candidate_id: candidate_id.clone(),
                    reason: format!("{}: {}", diagnostic.code, diagnostic.message),
                })
        })
        .collect::<Vec<_>>();
    let mut accepted_candidate_ids = pipeline
        .simulation
        .effects
        .iter()
        .map(|effect| effect.provenance.candidate_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    accepted_candidate_ids.sort();
    let mut entity_aliases_resolved = pipeline
        .binding
        .candidates
        .iter()
        .flat_map(|candidate| candidate.bindings.iter())
        .filter_map(|binding| binding.resolved_entity_id.clone())
        .collect::<Vec<_>>();
    entity_aliases_resolved.sort();
    entity_aliases_resolved.dedup();
    let conversion = EvaluatorConversionReport {
        no_op: pipeline.simulation.effects.is_empty(),
        patch: patch_lowering.patch,
        accepted_candidate_ids,
        rejected_candidates,
        evidence_validations: Vec::new(),
        entity_aliases_resolved: entity_aliases_resolved.clone(),
        entity_alias_resolution_warnings: pipeline
            .binding
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect(),
    };
    let normalized_json = serde_json::to_string(&batch)
        .map_err(|error| format!("Perception V2 serialization failed: {error}"))?;
    let enforcement_label = structured_enforcement
        .map(StructuredEnforcement::as_label)
        .unwrap_or("none")
        .to_string();
    Ok(RuntimeEvaluatorOutcome {
        output: EvaluatorOutputV1 {
            schema_version: EVALUATOR_SCHEMA_VERSION,
            ..EvaluatorOutputV1::default()
        },
        draft: state_engine::evaluator_ingest::NormalizedEvaluationDraft::default(),
        normalized_json,
        normalized: false,
        warnings: pipeline
            .semantic
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect(),
        conversion,
        form_spec: None,
        form_trace: None,
        form_rejected_rows: Vec::new(),
        form_response_parse_status: None,
        comparison_trace: None,
        partial_success: false,
        partial_success_reason: None,
        fallback_path: vec![EVALUATOR_MODE_PERCEPTION_V2.into()],
        fallback_warning: None,
        structured_ops_count: Some(batch.candidates.len()),
        syntactic_repair_used: false,
        structured_enforcement_requested: Some(enforcement_label),
        structured_enforcement_validated: true,
        structured_schema_validation_status: "validated".into(),
        structured_schema_validation_error: None,
        structured_retry_count: 0,
        structured_retry_reasons: Vec::new(),
        structured_retry_succeeded: None,
        structured_retry_final_error: None,
        structured_retry_used_failed_args: false,
        structured_retry_repair_prompt_included_error: false,
        entity_aliases_resolved,
        entity_alias_resolution_warnings: pipeline
            .binding
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect(),
        structured_run_classification: "perception_v2_commit_ready".into(),
        tool_calls_present: false,
        tool_call_count: 0,
        tool_call_names: Vec::new(),
        raw_content_present: false,
        raw_tool_calls_present: false,
    })
}

fn compile_evaluator_v1_runtime(
    raw_response: &str,
    soul: &Soul,
    session_world: &SessionWorld,
    latest_user_message: &str,
    latest_narrator_response: &str,
    baseline_recent_event_id: Option<String>,
) -> Result<RuntimeEvaluatorOutcome, String> {
    let evaluator_parse = parse_evaluator_output_with_context(
        raw_response,
        Some(&EvaluatorDraftContext {
            active_soul_id: soul.character_id.clone(),
            active_soul_display_name: soul.character_name.clone(),
            active_soul_ids: active_souls_for_v1(soul),
            latest_user_message: latest_user_message.to_string(),
        }),
    )?;
    let output = evaluator_parse.output.clone();
    let conversion = evaluator_output_to_engine_patch(
        &output,
        &EvaluatorConversionContext {
            active_soul_id: soul.character_id.as_str(),
            active_soul_ids: active_souls_for_v1(soul),
            latest_user_message,
            latest_narrator_response,
            session_world: Some(session_world),
            baseline_recent_event_id,
        },
    );
    let entity_aliases_resolved = conversion.entity_aliases_resolved.clone();
    let entity_alias_resolution_warnings = conversion.entity_alias_resolution_warnings.clone();
    Ok(RuntimeEvaluatorOutcome {
        output,
        draft: evaluator_parse.draft,
        normalized_json: evaluator_parse.normalized_json,
        normalized: evaluator_parse.normalized,
        warnings: evaluator_parse.warnings,
        conversion,
        form_spec: None,
        form_trace: None,
        form_rejected_rows: Vec::new(),
        form_response_parse_status: None,
        comparison_trace: None,
        partial_success: false,
        partial_success_reason: None,
        fallback_path: vec![EVALUATOR_MODE_V1.to_string()],
        fallback_warning: None,
        structured_ops_count: None,
        syntactic_repair_used: false,
        structured_enforcement_requested: None,
        structured_enforcement_validated: false,
        structured_schema_validation_status: "not_applicable".into(),
        structured_schema_validation_error: None,
        structured_retry_count: 0,
        structured_retry_reasons: Vec::new(),
        structured_retry_succeeded: None,
        structured_retry_final_error: None,
        structured_retry_used_failed_args: false,
        structured_retry_repair_prompt_included_error: false,
        entity_aliases_resolved,
        entity_alias_resolution_warnings,
        structured_run_classification: "tool_failed_form_fallback_success".into(),
        tool_calls_present: false,
        tool_call_count: 0,
        tool_call_names: Vec::new(),
        raw_content_present: false,
        raw_tool_calls_present: false,
    })
}

fn compile_evaluator_form_runtime(
    raw_response: &str,
    spec: EvalFormSpec,
    soul: &Soul,
    session_world: &SessionWorld,
    latest_user_message: &str,
    latest_narrator_response: &str,
    baseline_recent_event_id: Option<String>,
) -> Result<RuntimeEvaluatorOutcome, String> {
    let (form_response, repair_trace) = match parse_eval_form_response_with_trace(raw_response) {
        Ok(parsed) => parsed,
        Err(err) => {
            return Ok(minimal_form_scene_runtime(
                spec,
                soul,
                session_world,
                latest_user_message,
                latest_narrator_response,
                format!("form parse failed; minimal scene patch applied: {err}"),
                baseline_recent_event_id,
            ));
        }
    };
    let compiled = compile_eval_form_response(
        &spec,
        &form_response,
        &EvaluatorConversionContext {
            active_soul_id: soul.character_id.as_str(),
            active_soul_ids: active_souls_for_v1(soul),
            latest_user_message,
            latest_narrator_response,
            session_world: Some(session_world),
            baseline_recent_event_id: baseline_recent_event_id.clone(),
        },
    );
    let mut form_trace = compiled.trace;
    form_trace.raw_form_repair_applied = repair_trace.raw_form_repair_applied;
    form_trace.raw_form_repair_warnings = repair_trace.raw_form_repair_warnings;
    form_trace.json_extract_status = repair_trace.json_extract_status;
    form_trace.strict_parse_failed_but_salvage_attempted =
        repair_trace.strict_parse_failed_but_salvage_attempted;
    form_trace.salvage_success = repair_trace.salvage_success;
    let syntactic_repair_used = form_trace.raw_form_repair_applied;
    let mut conversion = compiled.conversion;
    let mut partial_success = false;
    let mut partial_success_reason = None;
    if conversion.patch.is_empty()
        && (!latest_user_message.trim().is_empty() || !latest_narrator_response.trim().is_empty())
    {
        let fallback = minimal_form_scene_runtime(
            spec.clone(),
            soul,
            session_world,
            latest_user_message,
            latest_narrator_response,
            "compiled form produced empty patch; minimal scene patch applied".into(),
            baseline_recent_event_id,
        );
        conversion = fallback.conversion;
        partial_success = true;
        partial_success_reason = fallback.partial_success_reason;
    }
    let entity_aliases_resolved = conversion.entity_aliases_resolved.clone();
    let entity_alias_resolution_warnings = conversion.entity_alias_resolution_warnings.clone();
    let normalized_json = serde_json::to_string(&compiled.output)
        .map_err(|err| format!("Evaluator form compiled output serialization failed: {err}"))?;
    Ok(RuntimeEvaluatorOutcome {
        output: compiled.output,
        draft: compiled.draft,
        normalized_json,
        normalized: true,
        warnings: compiled
            .rejected_rows
            .iter()
            .map(|row| format!("{} {} rejected: {}", row.row_kind, row.row_id, row.reason))
            .collect(),
        conversion,
        form_spec: Some(spec),
        form_trace: Some(form_trace),
        form_rejected_rows: compiled.rejected_rows,
        form_response_parse_status: Some(
            if partial_success {
                "partial_success"
            } else {
                "success"
            }
            .into(),
        ),
        comparison_trace: None,
        partial_success,
        partial_success_reason,
        fallback_path: vec![EVALUATOR_MODE_FORM_V1.to_string()],
        fallback_warning: None,
        structured_ops_count: None,
        syntactic_repair_used,
        structured_enforcement_requested: None,
        structured_enforcement_validated: false,
        structured_schema_validation_status: "not_applicable".into(),
        structured_schema_validation_error: None,
        structured_retry_count: 0,
        structured_retry_reasons: Vec::new(),
        structured_retry_succeeded: None,
        structured_retry_final_error: None,
        structured_retry_used_failed_args: false,
        structured_retry_repair_prompt_included_error: false,
        entity_aliases_resolved,
        entity_alias_resolution_warnings,
        structured_run_classification: "tool_failed_form_fallback_success".into(),
        tool_calls_present: false,
        tool_call_count: 0,
        tool_call_names: Vec::new(),
        raw_content_present: false,
        raw_tool_calls_present: false,
    })
}

fn minimal_form_scene_runtime(
    spec: EvalFormSpec,
    soul: &Soul,
    session_world: &SessionWorld,
    latest_user_message: &str,
    latest_narrator_response: &str,
    reason: String,
    baseline_recent_event_id: Option<String>,
) -> RuntimeEvaluatorOutcome {
    let summary = minimal_scene_summary(latest_user_message, latest_narrator_response);
    let player_entity_id = form_scene_player_entity_id(&spec);
    let participants = minimal_scene_participants(soul, &player_entity_id);
    let scene_state = SceneStatePatch {
        scene_state_id: Some(format!("scene_form_{}", uuid_like_id())),
        current_scene: Some(summary.clone()),
        focus: Some(scene_focus(soul, &player_entity_id)),
        participants: participants.clone(),
        last_user_action: clean_user_action(latest_user_message),
        continuity_note: Some(summary.clone()),
        ..SceneStatePatch::default()
    };
    let mut output = EvaluatorOutputV1 {
        schema_version: EVALUATOR_SCHEMA_VERSION,
        turn_flags_u64: state_engine::evaluator::turn_flags::SCENE_TURN
            | state_engine::evaluator::turn_flags::WORLD_CHANGE
            | state_engine::evaluator::turn_flags::USER_ACTION_PRESENT,
        ..EvaluatorOutputV1::default()
    };
    output.turn_classification = TurnClassification {
        is_pure_ooc: false,
        scene_event_occurred: true,
        is_retcon_or_correction: false,
        human_summary: summary.clone(),
    };
    output.global_scene_evaluation = GlobalSceneEvaluation {
        scene_event_occurred: true,
        current_plot_advanced: true,
        summary: summary.clone(),
        evidence_quote: clean_user_action(latest_user_message),
        ..GlobalSceneEvaluation::default()
    };
    output.world_changes.push(WorldChangeEvaluation {
        change_id: Some("event_latest_turn".into()),
        event_summary: Some(summary.clone()),
        scene_state: Some(scene_state),
        evidence_quote: clean_user_action(latest_user_message),
        confidence: 0.5,
        ..WorldChangeEvaluation::default()
    });
    let conversion = evaluator_output_to_engine_patch(
        &output,
        &EvaluatorConversionContext {
            active_soul_id: soul.character_id.as_str(),
            active_soul_ids: active_souls_for_v1(soul),
            latest_user_message,
            latest_narrator_response,
            session_world: Some(session_world),
            baseline_recent_event_id,
        },
    );
    let entity_aliases_resolved = conversion.entity_aliases_resolved.clone();
    let entity_alias_resolution_warnings = conversion.entity_alias_resolution_warnings.clone();
    let normalized_json = serde_json::to_string(&output).unwrap_or_else(|_| "{}".into());
    let form_spec_event_option_count = spec.allowed_event_types.len();
    let form_existing_memory_option_count = spec.existing_memories.len();
    RuntimeEvaluatorOutcome {
        output,
        draft: state_engine::evaluator_ingest::NormalizedEvaluationDraft {
            world_event_count: 1,
            scene_state_present: true,
            warnings: vec![reason.clone()],
            state_effect_guarantee_applied: true,
            state_effect_guarantee_reason: Some(reason.clone()),
            ..Default::default()
        },
        normalized_json,
        normalized: true,
        warnings: vec![reason.clone()],
        conversion,
        form_spec: Some(spec),
        form_trace: Some(EvalFormTrace {
            form_spec_event_option_count,
            form_existing_memory_option_count,
            form_rows_submitted: 0,
            form_rows_accepted: 1,
            form_rows_rejected: 0,
            compiled_turn_flags_u64: state_engine::evaluator::turn_flags::SCENE_TURN
                | state_engine::evaluator::turn_flags::WORLD_CHANGE
                | state_engine::evaluator::turn_flags::USER_ACTION_PRESENT,
            raw_form_repair_applied: true,
            raw_form_repair_warnings: vec![reason.clone()],
            json_extract_status: "fallback_minimal_scene".into(),
            strict_parse_failed_but_salvage_attempted: true,
            salvage_success: true,
            ..EvalFormTrace::default()
        }),
        form_rejected_rows: Vec::new(),
        form_response_parse_status: Some("partial_success".into()),
        comparison_trace: None,
        partial_success: true,
        partial_success_reason: Some(reason),
        fallback_path: vec![
            EVALUATOR_MODE_FORM_V1.to_string(),
            "minimal_scene_patch".to_string(),
        ],
        fallback_warning: Some("legacy form minimal scene fallback used".to_string()),
        structured_ops_count: None,
        syntactic_repair_used: true,
        structured_enforcement_requested: None,
        structured_enforcement_validated: false,
        structured_schema_validation_status: "not_applicable".into(),
        structured_schema_validation_error: None,
        structured_retry_count: 0,
        structured_retry_reasons: Vec::new(),
        structured_retry_succeeded: None,
        structured_retry_final_error: None,
        structured_retry_used_failed_args: false,
        structured_retry_repair_prompt_included_error: false,
        entity_aliases_resolved,
        entity_alias_resolution_warnings,
        structured_run_classification: "tool_failed_form_fallback_success".into(),
        tool_calls_present: false,
        tool_call_count: 0,
        tool_call_names: Vec::new(),
        raw_content_present: false,
        raw_tool_calls_present: false,
    }
}

fn construct_baseline_patch(
    soul: &Soul,
    latest_user_message: &str,
    latest_narrator_response: &str,
    active_player_entity_id: &str,
) -> (String, EnginePatch) {
    let narrator_trimmed = latest_narrator_response.trim();
    let one_sentence =
        if let Some(pos) = narrator_trimmed.find(|c| c == '.' || c == '!' || c == '?') {
            &narrator_trimmed[..=pos]
        } else {
            narrator_trimmed
        };
    let one_sentence = if one_sentence.chars().count() > 160 {
        one_sentence.chars().take(157).collect::<String>() + "..."
    } else {
        one_sentence.to_string()
    };

    let user_trimmed = latest_user_message.trim();
    let user_part = if user_trimmed.chars().count() > 80 {
        user_trimmed.chars().take(77).collect::<String>() + "..."
    } else {
        user_trimmed.to_string()
    };

    let summary = if !user_part.is_empty() && !one_sentence.is_empty() {
        format!("{} -> {}", user_part, one_sentence)
    } else if !one_sentence.is_empty() {
        one_sentence
    } else if !user_part.is_empty() {
        user_part
    } else {
        "The scene progressed.".to_string()
    };

    let baseline_event_id = format!("event_baseline_{}", uuid_like_id());
    let participants = minimal_scene_participants(soul, active_player_entity_id);
    let scene_state = SceneStatePatch {
        scene_state_id: Some(format!("scene_baseline_{}", uuid_like_id())),
        current_scene: Some(summary.clone()),
        focus: Some(scene_focus(soul, active_player_entity_id)),
        participants,
        last_user_action: clean_user_action(latest_user_message),
        continuity_note: Some(summary.clone()),
        ..SceneStatePatch::default()
    };

    let event_op = state_engine::patch::WorldEventOperationPatch {
        operation: "add".to_string(),
        recent_event_id: Some(baseline_event_id.clone()),
        content: Some(summary),
        ..state_engine::patch::WorldEventOperationPatch::default()
    };

    let patch = EnginePatch {
        world_patch: Some(state_engine::patch::WorldPatch {
            event_operations: vec![event_op],
            scene_state: Some(scene_state),
            ..state_engine::patch::WorldPatch::default()
        }),
        ..EnginePatch::default()
    };

    (baseline_event_id, patch)
}

fn minimal_scene_summary(latest_user_message: &str, latest_narrator_response: &str) -> String {
    let narrator = latest_narrator_response.trim();
    if !narrator.is_empty() {
        return narrator.chars().take(220).collect();
    }
    let user = latest_user_message.trim();
    if !user.is_empty() {
        return format!(
            "Latest user action: {}",
            user.chars().take(180).collect::<String>()
        );
    }
    "The current scene advanced.".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DurableChangeKind {
    Location,
    Object,
    Relationship,
    Scene,
}

fn durable_change_required(user_text: &str, narrator_text: &str) -> Option<DurableChangeKind> {
    let text = format!("{user_text}\n{narrator_text}").to_ascii_lowercase();
    let object_terms = [
        "jacket", "coat", "chair", "hook", "door", "phone", "key", "bag", "cup", "letter", "wet",
        "damp", "hang", "hung", "place", "placed", "drape", "draped", "move", "moved", "put",
        "set",
    ];
    if object_terms.iter().any(|term| text.contains(term))
        && [
            "place", "placed", "drape", "draped", "move", "moved", "hang", "hung", "wet", "damp",
        ]
        .iter()
        .any(|term| text.contains(term))
    {
        return Some(DurableChangeKind::Object);
    }
    if [
        "enter",
        "entered",
        "step inside",
        "walk in",
        "leave",
        "left",
        "arrive",
        "arrived",
        "outside",
        "inside",
    ]
    .iter()
    .any(|term| text.contains(term))
    {
        return Some(DurableChangeKind::Location);
    }
    if [
        "promise",
        "apolog",
        "trust",
        "betray",
        "comfort",
        "threat",
        "refuse",
        "boundary",
        "recognizing",
    ]
    .iter()
    .any(|term| text.contains(term))
    {
        return Some(DurableChangeKind::Relationship);
    }
    if [
        "scene",
        "focus",
        "near the door",
        "hallway",
        "apartment",
        "kitchen table",
    ]
    .iter()
    .any(|term| text.contains(term))
    {
        return Some(DurableChangeKind::Scene);
    }
    None
}

fn meaningful_no_op_reason(reason: Option<&str>) -> bool {
    let Some(reason) = reason.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let lower = reason.to_ascii_lowercase();
    reason.chars().count() >= 12
        && !matches!(
            lower.as_str(),
            "none" | "n/a" | "no-op" | "no op" | "nothing" | "no changes"
        )
}

fn diagnostic_object_scene_guarantee_patch(
    soul: &Soul,
    latest_user_message: &str,
    latest_narrator_response: &str,
) -> EnginePatch {
    let summary = minimal_scene_summary(latest_user_message, latest_narrator_response);
    let lower = format!("{latest_user_message}\n{latest_narrator_response}").to_ascii_lowercase();
    let jacket_location = if lower.contains("hook") || lower.contains("near the door") {
        "hook near the door"
    } else if lower.contains("chair") {
        "chair"
    } else {
        "current scene"
    };
    let jacket_status = if lower.contains("wet") || lower.contains("damp") {
        "wet"
    } else {
        "present"
    };
    EnginePatch {
        world_patch: Some(WorldPatch {
            scene_state: Some(SceneStatePatch {
                scene_state_id: Some(format!("scene_guarantee_{}", uuid_like_id())),
                current_scene: Some(summary.clone()),
                focus: Some(scene_focus(soul, "preset_male")),
                participants: minimal_scene_participants(soul, "preset_male"),
                last_user_action: clean_user_action(latest_user_message),
                continuity_note: Some(summary.clone()),
                ..SceneStatePatch::default()
            }),
            object_observation_operations: vec![
                state_engine::patch::ObjectObservationOperationPatch {
                    operation: "update_object_state".into(),
                    object_observation_id: Some(format!("object_guarantee_{}", uuid_like_id())),
                    object_state: Some(state_engine::soul::ObjectState {
                        object_id: "preset_male_jacket_1".into(),
                        object_kind: "jacket".into(),
                        owner_entity_id: Some("preset_male".into()),
                        location: jacket_location.into(),
                        status: jacket_status.into(),
                        last_observed_state: summary,
                        confidence: 0.65,
                        ..state_engine::soul::ObjectState::default()
                    }),
                    ..state_engine::patch::ObjectObservationOperationPatch::default()
                },
            ],
            ..WorldPatch::default()
        }),
        ..EnginePatch::default()
    }
}

fn merge_world_guarantee_patch(target: &mut EnginePatch, guarantee: EnginePatch) {
    let Some(mut guarantee_world) = guarantee.world_patch else {
        return;
    };
    let world = target.world_patch.get_or_insert_with(WorldPatch::default);
    if world.scene_state.is_none() {
        world.scene_state = guarantee_world.scene_state.take();
    }
    world
        .object_observation_operations
        .append(&mut guarantee_world.object_observation_operations);
}

fn form_scene_player_entity_id(spec: &EvalFormSpec) -> String {
    spec.active_entities
        .iter()
        .find(|entity| entity.entity_type == "player_persona")
        .or_else(|| {
            spec.active_entities
                .iter()
                .find(|entity| entity.entity_type == "user")
        })
        .map(|entity| entity.entity_id.clone())
        .unwrap_or_else(|| "default_player".into())
}

fn scene_focus(soul: &Soul, active_player_entity_id: &str) -> String {
    let player = active_player_entity_id.trim();
    let player = if player.is_empty() {
        "default_player"
    } else {
        player
    };
    format!("{} and {}", soul.character_name, player)
}

fn minimal_scene_participants(soul: &Soul, active_player_entity_id: &str) -> Vec<String> {
    let player = active_player_entity_id.trim();
    let player = if player.is_empty() {
        "default_player"
    } else {
        player
    };
    vec![soul.character_id.clone(), player.to_string()]
}

fn clean_user_action(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn dual_compare_deferred_trace(
    evaluator_mode: &str,
    selected_path_elapsed_ms: u128,
    selected_patch_applied_before_comparison_done: bool,
) -> Option<serde_json::Value> {
    if evaluator_mode != EVALUATOR_MODE_DUAL_COMPARE {
        return None;
    }

    Some(serde_json::json!({
        "mode": EVALUATOR_MODE_DUAL_COMPARE,
        "selected_evaluator_source": EVALUATOR_MODE_FORM_V1,
        "compare_evaluator_source": EVALUATOR_MODE_V1,
        "comparison_skipped_or_timed_out": true,
        "compare_timeout": true,
        "compare_parse_status": "skipped",
        "compare_error": "dual_compare comparison is debug-only and deferred so selected form patch application is not blocked",
        "compare_patch_applied": false,
        "selected_path_elapsed_ms": selected_path_elapsed_ms,
        "comparison_path_elapsed_ms": serde_json::Value::Null,
        "selected_patch_applied_before_comparison_done": selected_patch_applied_before_comparison_done,
    }))
}

pub(crate) fn effective_evaluator_timeout_ms(settings: &ApiProviderSettings) -> Option<u64> {
    if evaluator_timeout_mode(settings) == "no_app_timeout" {
        None
    } else {
        Some(
            settings
                .diagnostic_evaluator_timeout_ms
                .filter(|value| *value > 0)
                .or_else(|| {
                    settings
                        .structured_evaluator_timeout_ms
                        .filter(|value| *value > 0)
                })
                .or_else(|| settings.evaluator_timeout_ms.filter(|value| *value > 0))
                .unwrap_or(DEFAULT_STRUCTURED_EVALUATOR_TIMEOUT_MS),
        )
    }
}

/// Why these settings could never reach a provider, if so.
///
/// Checked before a job is created rather than inside the call, because the
/// failure is a configuration gap and the error should name it: an empty model
/// with the built-in default base URL means no updater profile was ever
/// assigned, which reads nothing like the transport error the call would raise.
fn unusable_evaluator_settings(settings: &ApiProviderSettings) -> Option<&'static str> {
    if settings.model.trim().is_empty() {
        return Some("no evaluator model is set");
    }
    if settings.base_url.trim().is_empty() {
        return Some("no evaluator base URL is set");
    }
    if settings.api_key.trim().is_empty() && !settings.base_url.contains("127.0.0.1") {
        return Some("the evaluator profile has no API key");
    }
    None
}

fn evaluator_background_enabled(settings: &ApiProviderSettings) -> bool {
    settings.evaluator_background_enabled.unwrap_or(false)
}

fn anti_replay_forced_retry_enabled(settings: &ApiProviderSettings) -> bool {
    settings
        .anti_replay_forced_retry_enabled
        .unwrap_or(ANTI_REPLAY_FORCED_RETRY_ENABLED_DEFAULT)
}

fn wait_for_evaluator_before_next_turn(settings: &ApiProviderSettings) -> bool {
    settings.wait_for_evaluator_before_next_turn.unwrap_or(true)
}

fn allow_send_with_stale_state(settings: &ApiProviderSettings) -> bool {
    settings.allow_send_with_stale_state.unwrap_or(false)
}

fn evaluator_timed_out(err: &str, elapsed: Duration, settings: &ApiProviderSettings) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("timed out")
        || lower.contains("timeout")
        || effective_evaluator_timeout_ms(settings)
            .is_some_and(|timeout_ms| elapsed >= Duration::from_millis(timeout_ms))
}

/// Evaluator completion plus how strictly the provider enforced output shape.
/// `structured_enforcement` is `Some` only for `evaluator_structured_v1`.
#[derive(Debug, Clone)]
struct EvaluatorCompletion {
    raw_text: String,
    structured_enforcement: Option<StructuredEnforcement>,
    token_usage: Option<TokenUsage>,
    trace: StructuredCompletionTrace,
}

#[derive(Debug, Clone)]
struct PerceptionV2ShadowOutcome {
    trace: PerceptionV2ShadowTrace,
    batch: Option<PerceptionBatch>,
    pipeline: Option<CompilerPipelineReport>,
    raw_response: Option<String>,
}

fn compiler_entity_catalog(
    soul: &Soul,
    session_world: &SessionWorld,
    player_persona_id: &str,
    player_persona_name: &str,
) -> EntityCatalog {
    let mut entities = BTreeMap::<String, EntityDescriptor>::new();
    entities.insert(
        soul.character_id.clone(),
        EntityDescriptor {
            entity_id: soul.character_id.clone(),
            display_name: soul.character_name.clone(),
            aliases: vec!["active_soul".into()],
            role: EntityRole::Soul,
            active: true,
        },
    );
    entities.insert(
        player_persona_id.into(),
        EntityDescriptor {
            entity_id: player_persona_id.into(),
            display_name: player_persona_name.into(),
            aliases: vec![
                "active_player".into(),
                "latest_speaker".into(),
                "user".into(),
            ],
            role: EntityRole::ActivePlayer,
            active: true,
        },
    );
    entities.insert(
        session_world.world_id.clone(),
        EntityDescriptor {
            entity_id: session_world.world_id.clone(),
            display_name: session_world.setting_name.clone(),
            aliases: vec!["session_world".into()],
            role: EntityRole::World,
            active: true,
        },
    );
    for object in &session_world.object_states {
        entities.insert(
            object.object_id.clone(),
            EntityDescriptor {
                entity_id: object.object_id.clone(),
                display_name: object.object_kind.clone(),
                aliases: vec![
                    object.object_kind.clone(),
                    object.last_observed_state.clone(),
                ],
                role: EntityRole::Object,
                active: true,
            },
        );
    }
    if !session_world.location.trim().is_empty() {
        entities.insert(
            format!("location:{}", session_world.location.trim()),
            EntityDescriptor {
                entity_id: format!("location:{}", session_world.location.trim()),
                display_name: session_world.location.clone(),
                aliases: vec![session_world.location.clone()],
                role: EntityRole::Location,
                active: true,
            },
        );
    }
    for entity_id in soul.relationships.keys() {
        entities
            .entry(entity_id.clone())
            .or_insert_with(|| EntityDescriptor {
                entity_id: entity_id.clone(),
                display_name: entity_id.clone(),
                aliases: Vec::new(),
                role: EntityRole::Other,
                active: true,
            });
    }
    EntityCatalog {
        entities: entities.into_values().collect(),
    }
}

async fn run_perception_v2_shadow(
    provider: &ApiProvider,
    settings: &ApiProviderSettings,
    source: &SourceEnvelope,
    catalog: EntityCatalog,
    snapshot: &SimulationSnapshot,
    system_prompt: &str,
    user_message: &str,
) -> PerceptionV2ShadowOutcome {
    let started = Instant::now();
    let timeout = effective_evaluator_timeout_ms(settings).map(Duration::from_millis);
    let completion = provider
        .complete_structured_prompt(
            settings,
            system_prompt,
            user_message,
            0.0,
            timeout,
            PERCEPTION_IR_SCHEMA_NAME,
            &perception_ir_json_schema(),
        )
        .await;
    match completion {
        Ok(completion) => {
            let raw_response = completion.raw_text.clone();
            let producer = ModelProvenance {
                provider: evaluator_provider_label(EVALUATOR_MODE_STRUCTURED_V1, false),
                model: settings.model.trim().to_string(),
                prompt_version: PERCEPTION_V2_PROMPT_VERSION.into(),
                schema_name: PERCEPTION_IR_SCHEMA_NAME.into(),
            };
            match compile_perception_v2_shadow_runtime(&completion.raw_text, source, producer) {
                Ok(batch) => {
                    let pipeline = compile_perception_pipeline(source, &batch, catalog, snapshot);
                    let patch_lowering =
                        lower_state_effects_to_engine_patch(source, &pipeline.simulation.effects);
                    let engine_patch = (pipeline.simulation.decision
                        == SimulationDecision::CommitReady
                        && patch_lowering.unsupported_effect_ids.is_empty())
                    .then(|| patch_lowering.patch.clone());
                    let mut kind_counts = BTreeMap::new();
                    for candidate in &batch.candidates {
                        let label = serde_json::to_value(candidate.perception.kind)
                            .ok()
                            .and_then(|value| value.as_str().map(str::to_string))
                            .unwrap_or_else(|| "unknown".into());
                        *kind_counts.entry(label).or_insert(0) += 1;
                    }
                    let semantic_accepted = pipeline
                        .semantic
                        .candidates
                        .iter()
                        .filter(|candidate| {
                            candidate.disposition
                                == state_engine::compiler::SemanticDisposition::Accepted
                        })
                        .count();
                    let semantic_rejected = pipeline.semantic.candidates.len() - semantic_accepted;
                    let mut diagnostic_codes = pipeline
                        .binding
                        .diagnostics
                        .iter()
                        .chain(pipeline.semantic.diagnostics.iter())
                        .chain(pipeline.lowering.diagnostics.iter())
                        .chain(pipeline.simulation.diagnostics.iter())
                        .map(|diagnostic| diagnostic.code.clone())
                        .collect::<Vec<_>>();
                    diagnostic_codes.sort();
                    diagnostic_codes.dedup();
                    PerceptionV2ShadowOutcome {
                        trace: PerceptionV2ShadowTrace {
                            attempted: true,
                            commit_allowed: false,
                            commit_count: 0,
                            schema_version: PERCEPTION_IR_SCHEMA_VERSION,
                            compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
                            prompt_version: PERCEPTION_V2_PROMPT_VERSION.into(),
                            enforcement_level: completion.enforcement.as_label().into(),
                            schema_validated: true,
                            status: "validated".into(),
                            error: None,
                            source_hash: Some(batch.source_hash.clone()),
                            candidate_count: batch.candidates.len(),
                            candidate_ids: batch
                                .candidates
                                .iter()
                                .map(|candidate| candidate.candidate_id.clone())
                                .collect(),
                            kind_counts,
                            semantic_accepted,
                            semantic_rejected,
                            effect_count: pipeline.lowering.effects.len(),
                            engine_patch_summary: engine_patch
                                .as_ref()
                                .map(engine_patch_summary)
                                .unwrap_or_else(|| serde_json::json!({})),
                            unsupported_effect_count: patch_lowering.unsupported_effect_ids.len(),
                            simulation_decision: match pipeline.simulation.decision {
                                SimulationDecision::CommitReady => "commit_ready",
                                SimulationDecision::Rejected => "rejected",
                            }
                            .into(),
                            diagnostic_codes,
                            v1_ops_count: None,
                            elapsed_ms: started.elapsed().as_millis() as u64,
                            prompt_tokens: completion
                                .token_usage
                                .and_then(|usage| usage.prompt_tokens),
                            completion_tokens: completion
                                .token_usage
                                .and_then(|usage| usage.completion_tokens),
                        },
                        batch: Some(batch),
                        pipeline: Some(pipeline),
                        raw_response: Some(raw_response),
                    }
                }
                Err(error) => PerceptionV2ShadowOutcome {
                    trace: PerceptionV2ShadowTrace {
                        attempted: true,
                        commit_allowed: false,
                        commit_count: 0,
                        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
                        compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
                        prompt_version: PERCEPTION_V2_PROMPT_VERSION.into(),
                        enforcement_level: completion.enforcement.as_label().into(),
                        schema_validated: false,
                        status: "invalid".into(),
                        error: Some(error),
                        source_hash: Some(source.source_hash().into()),
                        candidate_count: 0,
                        candidate_ids: Vec::new(),
                        kind_counts: BTreeMap::new(),
                        semantic_accepted: 0,
                        semantic_rejected: 0,
                        effect_count: 0,
                        engine_patch_summary: serde_json::json!({}),
                        unsupported_effect_count: 0,
                        simulation_decision: "not_run".into(),
                        diagnostic_codes: Vec::new(),
                        v1_ops_count: None,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                        prompt_tokens: completion.token_usage.and_then(|usage| usage.prompt_tokens),
                        completion_tokens: completion
                            .token_usage
                            .and_then(|usage| usage.completion_tokens),
                    },
                    batch: None,
                    pipeline: None,
                    raw_response: Some(raw_response),
                },
            }
        }
        Err(error) => PerceptionV2ShadowOutcome {
            trace: PerceptionV2ShadowTrace {
                attempted: true,
                commit_allowed: false,
                commit_count: 0,
                schema_version: PERCEPTION_IR_SCHEMA_VERSION,
                compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
                prompt_version: PERCEPTION_V2_PROMPT_VERSION.into(),
                enforcement_level: "none".into(),
                schema_validated: false,
                status: "transport_failed".into(),
                error: Some(error),
                source_hash: Some(source.source_hash().into()),
                candidate_count: 0,
                candidate_ids: Vec::new(),
                kind_counts: BTreeMap::new(),
                semantic_accepted: 0,
                semantic_rejected: 0,
                effect_count: 0,
                engine_patch_summary: serde_json::json!({}),
                unsupported_effect_count: 0,
                simulation_decision: "not_run".into(),
                diagnostic_codes: Vec::new(),
                v1_ops_count: None,
                elapsed_ms: started.elapsed().as_millis() as u64,
                prompt_tokens: None,
                completion_tokens: None,
            },
            batch: None,
            pipeline: None,
            raw_response: None,
        },
    }
}

#[derive(Debug, Clone)]
struct StructuredRetryFailure {
    final_error: String,
    retry_count: usize,
    retry_reasons: Vec<String>,
    first_trace: StructuredCompletionTrace,
}

/// Pick the ops schema for a structured evaluator call: the strict repair schema
/// (at least one op, no `no_op` escape) when the settings mark this as a repair,
/// otherwise the standard ops schema.
fn evaluator_schema_for(settings: &ApiProviderSettings) -> (&'static str, serde_json::Value) {
    if settings.structured_require_ops == Some(true) {
        (
            EVALUATOR_OPS_REPAIR_SCHEMA_NAME,
            evaluator_ops_repair_json_schema(),
        )
    } else {
        (EVALUATOR_OPS_SCHEMA_NAME, evaluator_ops_json_schema())
    }
}

async fn complete_evaluator_with_config(
    provider: &ApiProvider,
    settings: &ApiProviderSettings,
    system_prompt: &str,
    user_message: &str,
) -> Result<EvaluatorCompletion, String> {
    let timeout = effective_evaluator_timeout_ms(settings).map(Duration::from_millis);
    let source = selected_evaluator_source(&evaluator_mode(settings));
    if source == EVALUATOR_MODE_PERCEPTION_V2 {
        let completion = provider
            .complete_structured_prompt(
                settings,
                system_prompt,
                user_message,
                0.0,
                timeout,
                PERCEPTION_IR_SCHEMA_NAME,
                &perception_ir_json_schema(),
            )
            .await?;
        return Ok(EvaluatorCompletion {
            raw_text: completion.raw_text,
            structured_enforcement: Some(completion.enforcement),
            token_usage: completion.token_usage,
            trace: completion.trace,
        });
    }
    if source == EVALUATOR_MODE_STRUCTURED_V1 {
        let (schema_name, schema) = evaluator_schema_for(settings);
        let completion = provider
            .complete_structured_prompt(
                settings,
                system_prompt,
                user_message,
                0.0,
                timeout,
                schema_name,
                &schema,
            )
            .await?;
        return Ok(EvaluatorCompletion {
            raw_text: completion.raw_text,
            structured_enforcement: Some(completion.enforcement),
            token_usage: completion.token_usage,
            trace: completion.trace,
        });
    }
    let completion = provider
        .complete_prompt_with_usage(settings, system_prompt, user_message, 0.0, timeout)
        .await?;
    Ok(EvaluatorCompletion {
        raw_text: completion.raw_text,
        structured_enforcement: None,
        token_usage: completion.token_usage,
        trace: completion.trace,
    })
}

#[allow(clippy::too_many_arguments)]
async fn retry_structured_tool_call_after_compile_failure(
    provider: &ApiProvider,
    settings: &ApiProviderSettings,
    system_prompt: &str,
    user_message: &str,
    completion: &EvaluatorCompletion,
    first_error: &str,
    soul: &Soul,
    session_world: &SessionWorld,
    latest_user_message: &str,
    latest_narrator_response: &str,
    baseline_recent_event_id: Option<String>,
) -> Result<RuntimeEvaluatorOutcome, StructuredRetryFailure> {
    if completion.structured_enforcement != Some(StructuredEnforcement::ToolCall)
        || structured_evaluator_max_retries(settings) == 0
    {
        return Err(StructuredRetryFailure {
            final_error: first_error.to_string(),
            retry_count: 0,
            retry_reasons: Vec::new(),
            first_trace: completion.trace.clone(),
        });
    }

    let reason = structured_failure_kind(first_error).to_string();
    let retry_user_message =
        structured_tool_retry_user_message(user_message, Some(&completion.raw_text), first_error);
    let timeout = effective_evaluator_timeout_ms(settings).map(Duration::from_millis);
    let (schema_name, schema) = evaluator_schema_for(settings);
    let retry_completion = provider
        .complete_structured_tool_call_prompt(
            settings,
            system_prompt,
            &retry_user_message,
            0.0,
            timeout,
            schema_name,
            &schema,
        )
        .await
        .map_err(|retry_error| StructuredRetryFailure {
            final_error: retry_error,
            retry_count: 1,
            retry_reasons: vec![reason.clone()],
            first_trace: completion.trace.clone(),
        })?;

    match compile_evaluator_structured_runtime(
        &retry_completion.raw_text,
        Some(StructuredEnforcement::ToolCall),
        soul,
        session_world,
        latest_user_message,
        latest_narrator_response,
        baseline_recent_event_id,
        settings.structured_require_ops == Some(true),
    ) {
        Ok(mut outcome) => {
            apply_completion_retry_trace(&mut outcome, &retry_completion.trace);
            outcome.fallback_path = vec![
                "structured_tool_call".to_string(),
                "structured_tool_call_retry".to_string(),
            ];
            outcome.structured_retry_count = 1;
            outcome.structured_retry_reasons = vec![reason];
            outcome.structured_retry_succeeded = Some(true);
            outcome.structured_retry_final_error = None;
            outcome.structured_retry_used_failed_args = true;
            outcome.structured_retry_repair_prompt_included_error = true;
            outcome.structured_run_classification = "tool_retry_success".into();
            Ok(outcome)
        }
        Err(retry_error) => Err(StructuredRetryFailure {
            final_error: retry_error,
            retry_count: 1,
            retry_reasons: vec![reason],
            first_trace: completion.trace.clone(),
        }),
    }
}

fn apply_structured_retry_failure(
    outcome: &mut RuntimeEvaluatorOutcome,
    retry_failure: &StructuredRetryFailure,
) {
    outcome.tool_calls_present = retry_failure.first_trace.tool_calls_present;
    outcome.tool_call_count = retry_failure.first_trace.tool_call_count;
    outcome.tool_call_names = retry_failure.first_trace.tool_call_names.clone();
    outcome.raw_content_present = retry_failure.first_trace.raw_content_present;
    outcome.raw_tool_calls_present = retry_failure.first_trace.raw_tool_calls_present;
    if retry_failure.retry_count > 0 {
        outcome.structured_retry_count = retry_failure.retry_count;
        outcome.structured_retry_reasons = retry_failure.retry_reasons.clone();
        outcome.structured_retry_succeeded = Some(false);
        outcome.structured_retry_final_error = Some(retry_failure.final_error.clone());
        outcome.structured_retry_used_failed_args = true;
        outcome.structured_retry_repair_prompt_included_error = true;
    }
}

fn apply_completion_retry_trace(
    outcome: &mut RuntimeEvaluatorOutcome,
    trace: &StructuredCompletionTrace,
) {
    outcome.tool_calls_present = trace.tool_calls_present;
    outcome.tool_call_count = trace.tool_call_count;
    outcome.tool_call_names = trace.tool_call_names.clone();
    outcome.raw_content_present = trace.raw_content_present;
    outcome.raw_tool_calls_present = trace.raw_tool_calls_present;
    if trace.structured_retry_count > 0 {
        outcome.structured_retry_count = trace.structured_retry_count;
        outcome.structured_retry_reasons = trace.structured_retry_reasons.clone();
        outcome.structured_retry_succeeded = trace.structured_retry_succeeded;
        outcome.structured_retry_final_error = trace.structured_retry_final_error.clone();
        outcome.structured_retry_used_failed_args = trace.structured_retry_used_failed_args;
        outcome.structured_retry_repair_prompt_included_error =
            trace.structured_retry_repair_prompt_included_error;
        if trace.structured_retry_succeeded == Some(true)
            && outcome.fallback_path == ["structured_tool_call".to_string()]
        {
            outcome
                .fallback_path
                .push("structured_tool_call_retry".to_string());
            outcome.structured_run_classification = "tool_retry_success".into();
        }
    }
}

fn structured_failure_kind(error: &str) -> &'static str {
    let lower = error.to_ascii_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") {
        "timeout"
    } else if lower.contains("zero_ops_on_required_reextract") {
        "zero_ops_on_required_reextract"
    } else if lower.contains("unknown field")
        || lower.contains("missing field")
        || lower.contains("invalid type")
        || lower.contains("unknown variant")
        || lower.contains("expected")
    {
        "schema_validation_failed"
    } else if lower.contains("evidence quote") || lower.contains("evidence_quote") {
        "evidence_quote_invalid"
    } else if lower.contains("semantic validation failed") {
        "semantic_validation_failed"
    } else if lower.contains("malformed_schema_output")
        || lower.contains("parse failed")
        || lower.contains("failed strict parse")
    {
        "schema_parse_failed"
    } else if lower.contains("no tool_calls") {
        "no_tool_calls"
    } else if lower.contains("content only") {
        "content_only_response"
    } else {
        "semantic_validation_failed"
    }
}

/// Fill the evaluator side of the trace's token accounting, falling back to
/// character-based estimates when the provider reported no usage.
fn evaluator_token_usage_for_trace(
    token_usage: Option<TokenUsage>,
    system_prompt: &str,
    user_message: &str,
    raw_response: Option<&str>,
) -> (Option<u64>, Option<u64>, bool) {
    let reported_prompt = token_usage.and_then(|usage| usage.prompt_tokens);
    let reported_completion = token_usage.and_then(|usage| usage.completion_tokens);
    let estimated = reported_prompt.is_none() || reported_completion.is_none();
    let prompt_tokens = reported_prompt
        .unwrap_or_else(|| (estimate_tokens(system_prompt) + estimate_tokens(user_message)) as u64);
    let completion_tokens =
        reported_completion.or_else(|| raw_response.map(|text| estimate_tokens(text) as u64));
    (Some(prompt_tokens), completion_tokens, estimated)
}

fn gate_pending_evaluator_jobs(
    window: &Window,
    state: &State<'_, AppState>,
    conversation_id: &str,
    settings: &ApiProviderSettings,
) -> Result<EvaluatorGateOutcome, String> {
    let initial_pending = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::get_pending_evaluator_jobs_for_conversation(&conn, conversation_id)
            .map_err(|err| err.to_string())?
    };
    if initial_pending.is_empty() {
        return Ok(EvaluatorGateOutcome::default());
    }
    let pending_job_ids = initial_pending
        .iter()
        .map(|job| job.evaluator_job_id.clone())
        .collect::<Vec<_>>();
    if wait_for_evaluator_before_next_turn(settings) {
        let started = Instant::now();
        let max_wait_ms = effective_evaluator_timeout_ms(settings)
            .unwrap_or(NEXT_TURN_GATE_FALLBACK_MAX_MS)
            .max(NEXT_TURN_GATE_POLL_MS);
        loop {
            std::thread::sleep(Duration::from_millis(NEXT_TURN_GATE_POLL_MS));
            let still_pending = {
                let conn = state.conn.lock().map_err(|err| err.to_string())?;
                db::get_pending_evaluator_jobs_for_conversation(&conn, conversation_id)
                    .map_err(|err| err.to_string())?
            };
            if still_pending.is_empty() {
                let waited_ms = started.elapsed().as_millis() as u64;
                emit_dev_log(
                    window,
                    "info",
                    "evaluator",
                    "next_turn_waited_for_evaluator",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id,
                        "next_turn_wait_ms": waited_ms,
                        "pending_job_ids": pending_job_ids
                    })),
                );
                return Ok(EvaluatorGateOutcome {
                    waited_ms,
                    pending_job_ids,
                    ..EvaluatorGateOutcome::default()
                });
            }
            if started.elapsed() >= Duration::from_millis(max_wait_ms) {
                return Err(format!(
                    "State update in progress and did not finish within {max_wait_ms}ms"
                ));
            }
        }
    }
    if allow_send_with_stale_state(settings) {
        emit_dev_log(
            window,
            "warn",
            "evaluator",
            "next_turn_proceeded_with_stale_state",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "pending_job_ids": pending_job_ids
            })),
        );
        return Ok(EvaluatorGateOutcome {
            waited_ms: 0,
            stale_state_send: true,
            compiled_with_pending_evaluator: true,
            pending_job_ids,
        });
    }
    Err("State update in progress and stale send is not allowed".into())
}

#[allow(clippy::too_many_arguments)]
fn start_background_evaluator_job(
    app: AppHandle,
    window: Window,
    conversation_id: String,
    assistant_message_id: i64,
    selected_variant_id: Option<i64>,
    parent_narrator_request_id: String,
    evaluator_request_id: String,
    turn_id: Option<String>,
    context_mode_label: String,
    soul: Soul,
    session_world: SessionWorld,
    snapshot_user_text: String,
    visible_response_for_updater: String,
    context_preview_text: String,
    state_updater_settings: ApiProviderSettings,
    entity_updater_context: String,
    memory_debug_nonce: String,
    ledger_branch_id: Option<String>,
    ledger_parent_turn_id: Option<String>,
    ledger_user_message_id: Option<i64>,
    is_regenerated_variant: bool,
    before_state_summary: serde_json::Value,
    baseline_patch_id: Option<String>,
    // When set, replaces the evaluator's user message with a focused op-repair
    // request. Everything else (compile, partial-accept, ledger apply, status)
    // is the normal proven path — this is how the background repair worker reuses
    // it. None for ordinary evaluation.
    repair_user_message_override: Option<String>,
) -> Result<db::EvaluatorJob, String> {
    let timeout_ms = effective_evaluator_timeout_ms(&state_updater_settings);
    let timeout_mode = evaluator_timeout_mode(&state_updater_settings);
    let mode = {
        let state = app.state::<AppState>();
        let resolved = state.conn.lock().ok().and_then(|conn| {
            resolve_evaluator_mode_setting(&conn, &conversation_id, &state_updater_settings)
        });
        evaluator_mode(&ApiProviderSettings {
            evaluator_mode: resolved.or_else(|| state_updater_settings.evaluator_mode.clone()),
            ..state_updater_settings.clone()
        })
    };
    let job = db::EvaluatorJob {
        evaluator_job_id: format!("eval_job_{}", uuid_like_id()),
        conversation_id: conversation_id.clone(),
        turn_id: turn_id
            .clone()
            .unwrap_or_else(|| format!("turn_{}", parent_narrator_request_id)),
        assistant_message_id,
        status: "pending".into(),
        started_at: db::now_ts(),
        completed_at: None,
        elapsed_ms: None,
        timeout_ms,
        timeout_mode,
        model: Some(state_updater_settings.model.trim().to_string()),
        provider: Some(evaluator_provider_label(&mode, true)),
        error_message: None,
        patch_applied: false,
    };
    {
        let state = app.state::<AppState>();
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::insert_evaluator_job(&conn, &job).map_err(|err| err.to_string())?;
    }
    emit_evaluator_job_status(&window, &job);
    let job_for_task = job.clone();
    let bp_id_clone = baseline_patch_id.clone();
    tauri::async_runtime::spawn(async move {
        run_background_evaluator_job(
            app,
            window,
            job_for_task,
            selected_variant_id,
            parent_narrator_request_id,
            evaluator_request_id,
            turn_id,
            context_mode_label,
            soul,
            session_world,
            snapshot_user_text,
            visible_response_for_updater,
            context_preview_text,
            state_updater_settings,
            entity_updater_context,
            memory_debug_nonce,
            ledger_branch_id,
            ledger_parent_turn_id,
            ledger_user_message_id,
            is_regenerated_variant,
            before_state_summary,
            bp_id_clone,
            repair_user_message_override,
        )
        .await;
    });
    Ok(job)
}

fn emit_evaluator_job_status(window: &Window, job: &db::EvaluatorJob) {
    let _ = window.emit("evaluator-job-status-changed", job);
}

fn update_background_job_status(
    app: &AppHandle,
    window: &Window,
    job_id: &str,
    status: &str,
    error_message: Option<&str>,
    started_at: Instant,
    patch_applied: bool,
) {
    let state = app.state::<AppState>();
    if let Ok(conn) = state.conn.lock() {
        let elapsed_ms = started_at.elapsed().as_millis() as i64;
        let _ = db::update_evaluator_job_status(
            &conn,
            job_id,
            status,
            error_message,
            Some(db::now_ts()),
            Some(elapsed_ms),
            patch_applied,
        );
        if let Ok(Some(job)) = db::get_evaluator_job(&conn, job_id) {
            emit_evaluator_job_status(window, &job);
        }
    };
}

fn evaluator_job_is_canceled(app: &AppHandle, job_id: &str) -> bool {
    let state = app.state::<AppState>();
    let canceled = state
        .conn
        .lock()
        .ok()
        .and_then(|conn| db::get_evaluator_job(&conn, job_id).ok().flatten())
        .is_some_and(|job| job.status == "canceled");
    canceled
}

#[allow(clippy::too_many_arguments)]
async fn run_background_evaluator_job(
    app: AppHandle,
    window: Window,
    job: db::EvaluatorJob,
    selected_variant_id: Option<i64>,
    parent_narrator_request_id: String,
    evaluator_request_id: String,
    turn_id: Option<String>,
    context_mode_label: String,
    mut soul: Soul,
    mut session_world: SessionWorld,
    snapshot_user_text: String,
    visible_response_for_updater: String,
    context_preview_text: String,
    state_updater_settings: ApiProviderSettings,
    entity_updater_context: String,
    memory_debug_nonce: String,
    ledger_branch_id: Option<String>,
    ledger_parent_turn_id: Option<String>,
    ledger_user_message_id: Option<i64>,
    is_regenerated_variant: bool,
    before_state_summary: serde_json::Value,
    baseline_patch_id: Option<String>,
    repair_user_message_override: Option<String>,
) {
    let started = Instant::now();
    let profile_id = {
        let state = app.state::<AppState>();
        state.conn.lock().ok().and_then(|conn| {
            if let Ok(conv) = db::get_conversation_summary(&conn, &job.conversation_id) {
                if let Some(id) = conv.active_evaluator_profile_id {
                    return Some(id);
                }
            }
            let query =
                "SELECT id FROM provider_profiles WHERE archived_at IS NULL AND model = ?1 LIMIT 1";
            conn.query_row(query, [&state_updater_settings.model], |row| {
                row.get::<_, String>(0)
            })
            .ok()
        })
    };

    let parent_payload_log = {
        let state = app.state::<AppState>();
        state.conn.lock().ok().and_then(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, pipeline_trace_json FROM llm_payload_logs WHERE request_id = ?1",
                )
                .ok()?;
            let row = stmt
                .query_row(rusqlite::params![parent_narrator_request_id], |r| {
                    let id: i64 = r.get(0)?;
                    let pipeline_trace_json: Option<String> = r.get(1)?;
                    Ok((id, pipeline_trace_json))
                })
                .ok();
            row
        })
    };

    let (narrator_log_id, mut pipeline_trace) = match parent_payload_log {
        Some((id, Some(json_str))) => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(pt_val) = val.get("pipeline_trace") {
                    if let Ok(trace) = serde_json::from_value::<TurnPipelineTrace>(pt_val.clone()) {
                        (Some(id), trace)
                    } else {
                        (
                            Some(id),
                            TurnPipelineTrace::new(
                                evaluator_request_id.clone(),
                                turn_id.clone(),
                                job.conversation_id.clone(),
                                db::now_ts(),
                            ),
                        )
                    }
                } else {
                    (
                        Some(id),
                        TurnPipelineTrace::new(
                            evaluator_request_id.clone(),
                            turn_id.clone(),
                            job.conversation_id.clone(),
                            db::now_ts(),
                        ),
                    )
                }
            } else {
                (
                    Some(id),
                    TurnPipelineTrace::new(
                        evaluator_request_id.clone(),
                        turn_id.clone(),
                        job.conversation_id.clone(),
                        db::now_ts(),
                    ),
                )
            }
        }
        Some((id, None)) => (
            Some(id),
            TurnPipelineTrace::new(
                evaluator_request_id.clone(),
                turn_id.clone(),
                job.conversation_id.clone(),
                db::now_ts(),
            ),
        ),
        None => (
            None,
            TurnPipelineTrace::new(
                evaluator_request_id.clone(),
                turn_id.clone(),
                job.conversation_id.clone(),
                db::now_ts(),
            ),
        ),
    };
    let baseline_recent_event_id = if let Some(ref bp_id) = baseline_patch_id {
        let state = app.state::<AppState>();
        let mut found_id = None;
        if let Ok(conn) = state.conn.lock() {
            if let Ok(patch_record) = db::get_state_patch(&conn, bp_id) {
                if let Ok(patch) = serde_json::from_str::<EnginePatch>(&patch_record.patch_json) {
                    found_id = patch.world_patch.and_then(|wp| {
                        wp.event_operations
                            .first()
                            .and_then(|op| op.recent_event_id.clone())
                    });
                }
            }
        }
        found_id
    } else {
        None
    };
    {
        let state = app.state::<AppState>();
        if let Ok(conn) = state.conn.lock() {
            let _ = db::update_evaluator_job_status(
                &conn,
                &job.evaluator_job_id,
                "running",
                None,
                None,
                None,
                false,
            );
            if let Ok(Some(job)) = db::get_evaluator_job(&conn, &job.evaluator_job_id) {
                emit_evaluator_job_status(&window, &job);
            }
        };
    }

    let mut state_updater_settings = state_updater_settings;
    let resolved_evaluator_mode = {
        let state = app.state::<AppState>();
        state.conn.lock().ok().and_then(|conn| {
            resolve_evaluator_mode_setting(&conn, &job.conversation_id, &state_updater_settings)
        })
    };
    if let Some(mode) = resolved_evaluator_mode {
        state_updater_settings.evaluator_mode = Some(mode);
    }
    let resolved_structured_policy = {
        let state = app.state::<AppState>();
        state.conn.lock().ok().and_then(|conn| {
            resolve_structured_evaluator_policy_setting(
                &conn,
                &job.conversation_id,
                &state_updater_settings,
            )
        })
    };
    if let Some(policy) = resolved_structured_policy {
        state_updater_settings.structured_evaluator_policy = Some(policy);
    }
    let evaluator_mode = evaluator_mode(&state_updater_settings);
    let selected_evaluator_source = selected_evaluator_source(&evaluator_mode);
    let active_player_persona = {
        let state = app.state::<AppState>();
        state
            .conn
            .lock()
            .ok()
            .and_then(|conn| db::get_active_player_persona(&conn, &job.conversation_id).ok())
            .unwrap_or_else(|| {
                db::built_in_player_personas()
                    .into_iter()
                    .next()
                    .expect("built-in player persona exists")
            })
    };
    let form_spec = matches!(
        selected_evaluator_source,
        EVALUATOR_MODE_FORM_V1 | EVALUATOR_MODE_STRUCTURED_V1 | EVALUATOR_MODE_PERCEPTION_V2
    )
    .then(|| {
        build_eval_form_spec_with_player_persona(
            &soul,
            Some(&session_world),
            &snapshot_user_text,
            &visible_response_for_updater,
            8,
            &active_player_persona.persona_id,
            &active_player_persona.display_name,
        )
    });
    let fallback_form_system_prompt = matches!(
        selected_evaluator_source,
        EVALUATOR_MODE_STRUCTURED_V1 | EVALUATOR_MODE_PERCEPTION_V2
    )
    .then(|| {
        build_evaluator_form_prompt_with_player_persona(
            &soul,
            Some(&session_world),
            &snapshot_user_text,
            &visible_response_for_updater,
            &active_player_persona.persona_id,
            &active_player_persona.display_name,
        )
    });
    let perception_source =
        (selected_evaluator_source == EVALUATOR_MODE_PERCEPTION_V2).then(|| {
            production_perception_source(
                &job.conversation_id,
                ledger_branch_id.as_deref(),
                turn_id.as_deref(),
                ledger_parent_turn_id.as_deref(),
                ledger_user_message_id,
                job.assistant_message_id,
                selected_variant_id,
                active_souls_for_v1(&soul),
                &snapshot_user_text,
                &visible_response_for_updater,
            )
        });
    let updater_system_prompt = if selected_evaluator_source == EVALUATOR_MODE_FORM_V1 {
        let mut is_compact =
            state_updater_settings.evaluator_mode.as_deref() == Some("form_v1_compact");
        if !is_compact {
            if let Some(ref p_id) = profile_id {
                if let Ok(conn) = app.state::<AppState>().conn.lock() {
                    if let Ok(profile) = db::get_provider_profile(&conn, p_id) {
                        if profile.evaluator_mode.as_deref() == Some("form_v1_compact") {
                            is_compact = true;
                        }
                    }
                }
            }
        }
        if is_compact {
            build_evaluator_form_prompt_compact_with_player_persona(
                &soul,
                Some(&session_world),
                &snapshot_user_text,
                &visible_response_for_updater,
                &active_player_persona.persona_id,
                &active_player_persona.display_name,
            )
        } else {
            build_evaluator_form_prompt_with_player_persona(
                &soul,
                Some(&session_world),
                &snapshot_user_text,
                &visible_response_for_updater,
                &active_player_persona.persona_id,
                &active_player_persona.display_name,
            )
        }
    } else if selected_evaluator_source == EVALUATOR_MODE_STRUCTURED_V1 {
        build_structured_evaluator_prompt(&soul, Some(&session_world))
    } else if selected_evaluator_source == EVALUATOR_MODE_PERCEPTION_V2 {
        build_perception_v2_prompt_with_player_persona(
            &soul,
            Some(&session_world),
            &active_player_persona.persona_id,
            &active_player_persona.display_name,
        )
    } else {
        build_evaluator_prompt(&soul, Some(&session_world))
    };
    let updater_user_message = match repair_user_message_override.as_deref() {
        // Repair mode: focused "fix only these failed ops" request instead of a
        // full re-extraction. The system rules and apply path are unchanged.
        Some(repair) => repair.to_string(),
        None => build_evaluator_user_message(
            &snapshot_user_text,
            &visible_response_for_updater,
            &context_preview_text,
            Some(&session_world),
            Some(&entity_updater_context),
            Some(&memory_debug_nonce),
        ),
    };
    // Fold in exchanges the fast-mode gate skipped; deleted only after this
    // run parses successfully, so failed/retried jobs see them again.
    let catchup_entries = {
        let state = app.state::<AppState>();
        let entries = state.conn.lock().ok().map(|conn| {
            db::list_evaluator_catchup_entries(&conn, &job.conversation_id).unwrap_or_default()
        });
        entries.unwrap_or_default()
    };
    let drained_catchup_ids: Vec<i64> = catchup_entries.iter().map(|entry| entry.id).collect();
    let updater_user_message =
        append_evaluator_catchup_block(updater_user_message, &catchup_entries);
    let updater_token_estimate =
        estimate_tokens(&updater_system_prompt) + estimate_tokens(&updater_user_message);
    let updater_log_id = {
        let state = app.state::<AppState>();
        state.conn.lock().ok().and_then(|conn| {
            db::insert_llm_payload_log(
                &conn,
                &LlmPayloadLog {
                    id: 0,
                    conversation_id: job.conversation_id.clone(),
                    message_id: Some(job.assistant_message_id),
                    provider: evaluator_provider_label(&evaluator_mode, true),
                    mode: evaluator_mode.clone(),
                    context_mode: context_mode_label.clone(),
                    model: state_updater_settings.model.trim().to_string(),
                    base_url: state_updater_settings.base_url.trim().to_string(),
                    system_message: updater_system_prompt.clone(),
                    user_message: updater_user_message.clone(),
                    context_text: updater_system_prompt.clone(),
                    estimated_system_tokens: estimate_tokens(&updater_system_prompt),
                    estimated_user_tokens: estimate_tokens(&updater_user_message),
                    estimated_total_tokens: updater_token_estimate,
                    truncated: false,
                    created_at: db::now_ts(),
                    branch_id: ledger_branch_id.clone(),
                    active_turn_id: ledger_parent_turn_id.clone(),
                    parent_turn_id: ledger_parent_turn_id.clone(),
                    latest_assistant_variant_id: selected_variant_id,
                    request_id: Some(evaluator_request_id.clone()),
                    turn_id: turn_id.clone(),
                    ..Default::default()
                },
            )
            .ok()
        })
    };
    emit_dev_log(
        &window,
        "info",
        "evaluator",
        "evaluator_called",
        Some(serde_json::json!({
            "conversation_id": job.conversation_id.as_str(),
            "assistant_message_id": job.assistant_message_id,
            "evaluator_job_id": job.evaluator_job_id.as_str(),
            "model": state_updater_settings.model.trim(),
            "evaluator_mode": evaluator_mode.as_str(),
            "selected_evaluator_source": selected_evaluator_source,
            "background": true,
            "timeout_ms": job.timeout_ms,
            "timeout_mode": job.timeout_mode.as_str()
        })),
    );

    if evaluator_job_is_canceled(&app, &job.evaluator_job_id) {
        update_background_job_status(
            &app,
            &window,
            &job.evaluator_job_id,
            "canceled",
            Some("Canceled before evaluator call"),
            started,
            false,
        );
        return;
    }

    let provider = ApiProvider::default();
    let call_started = Instant::now();
    let response_result = complete_evaluator_with_config(
        &provider,
        &state_updater_settings,
        &updater_system_prompt,
        &updater_user_message,
    )
    .await;
    let call_elapsed = call_started.elapsed();
    let raw_response = response_result
        .as_ref()
        .ok()
        .map(|completion| completion.raw_text.clone());
    let structured_enforcement = response_result
        .as_ref()
        .ok()
        .and_then(|completion| completion.structured_enforcement);
    {
        let (prompt_tokens, completion_tokens, estimated) = evaluator_token_usage_for_trace(
            response_result
                .as_ref()
                .ok()
                .and_then(|completion| completion.token_usage),
            &updater_system_prompt,
            &updater_user_message,
            raw_response.as_deref(),
        );
        let usage = pipeline_trace
            .token_usage
            .get_or_insert_with(TurnTokenUsage::default);
        usage.evaluator_prompt_tokens = prompt_tokens;
        usage.evaluator_completion_tokens = completion_tokens;
        usage.evaluator_estimated = estimated;
    }

    let received_elapsed = call_elapsed.as_millis() as u64;
    match &response_result {
        Ok(_) => {
            pipeline_trace.record_stage(
                "evaluator_response_received",
                "success",
                received_elapsed,
                None,
                Some(match structured_enforcement {
                    Some(enforcement) => format!(
                        "Evaluator response received (structured enforcement: {})",
                        enforcement.as_label()
                    ),
                    None => "Evaluator response received".to_string(),
                }),
            );
            window.emit("pipeline-trace-updated", &pipeline_trace).ok();
            if let Some(n_id) = narrator_log_id {
                if let Ok(conn) = app.state::<AppState>().conn.lock() {
                    let trace_val = serde_json::json!({ "pipeline_trace": &pipeline_trace });
                    let _ = update_llm_payload_pipeline_trace(&conn, n_id, &trace_val);
                }
            }
        }
        Err(err) => {
            pipeline_trace.record_stage_error(
                "evaluator_response_received",
                received_elapsed,
                PipelineErrorCode::EvaluatorCallError,
                err.to_string(),
                Some("Check LLM provider settings or availability".to_string()),
            );
            pipeline_trace.final_status = "failed".into();
            pipeline_trace.failing_stage = Some("evaluator_response_received".to_string());
            pipeline_trace.total_elapsed_ms = started.elapsed().as_millis() as u64;
            window.emit("pipeline-trace-updated", &pipeline_trace).ok();
            if let Some(n_id) = narrator_log_id {
                if let Ok(conn) = app.state::<AppState>().conn.lock() {
                    let trace_val = serde_json::json!({ "pipeline_trace": &pipeline_trace });
                    let _ = update_llm_payload_pipeline_trace(&conn, n_id, &trace_val);
                }
            }
        }
    }
    if let (Some(log_id), Ok(completion)) = (updater_log_id, response_result.as_ref()) {
        let state = app.state::<AppState>();
        if let Ok(conn) = state.conn.lock() {
            let _ = db::update_llm_payload_log_response(
                &conn,
                log_id,
                &db::LlmPayloadResponseUpdate {
                    raw_provider_response: Some(completion.raw_text.clone()),
                    normalized_response: Some(completion.raw_text.clone()),
                    ..Default::default()
                },
            );
        };
    }
    if evaluator_job_is_canceled(&app, &job.evaluator_job_id) {
        update_background_job_status(
            &app,
            &window,
            &job.evaluator_job_id,
            "canceled",
            Some("Canceled before patch commit"),
            started,
            false,
        );
        return;
    }

    let runtime_result = match response_result {
        Ok(completion) => {
            let structured_step = structured_fallback_step(completion.structured_enforcement);
            let compiled = if selected_evaluator_source == EVALUATOR_MODE_PERCEPTION_V2 {
                perception_source
                    .as_ref()
                    .ok_or_else(|| "Perception V2 source was not initialized".to_string())
                    .and_then(|result| result.as_ref().map_err(Clone::clone))
                    .and_then(|source| {
                        compile_perception_v2_runtime(
                            &completion.raw_text,
                            completion.structured_enforcement,
                            source,
                            compiler_entity_catalog(
                                &soul,
                                &session_world,
                                &active_player_persona.persona_id,
                                &active_player_persona.display_name,
                            ),
                            &SimulationSnapshot {
                                state_hash: source.parent_state_hash().map(str::to_string),
                                existing_effect_ids: Vec::new(),
                            },
                            evaluator_provider_label(&evaluator_mode, true),
                            &state_updater_settings.model,
                        )
                    })
            } else {
                compile_selected_evaluator_runtime(
                    &evaluator_mode,
                    form_spec.clone(),
                    &completion.raw_text,
                    completion.structured_enforcement,
                    &soul,
                    &session_world,
                    &snapshot_user_text,
                    &visible_response_for_updater,
                    baseline_recent_event_id.clone(),
                    state_updater_settings.structured_require_ops == Some(true),
                )
            };
            match compiled {
                Ok(mut output) => {
                    apply_completion_retry_trace(&mut output, &completion.trace);
                    Ok(output)
                }
                Err(err) if selected_evaluator_source == EVALUATOR_MODE_PERCEPTION_V2 => {
                    emit_dev_log(
                        &window,
                        "warn",
                        "evaluator",
                        "perception_v2_fallback_to_form_started",
                        Some(serde_json::json!({
                            "conversation_id": job.conversation_id.as_str(),
                            "assistant_message_id": job.assistant_message_id,
                            "evaluator_job_id": job.evaluator_job_id.as_str(),
                            "error": err.as_str()
                        })),
                    );
                    let (fallback_result, _) = complete_form_fallback_runtime(
                        &provider,
                        &state_updater_settings,
                        fallback_form_system_prompt
                            .as_deref()
                            .unwrap_or(&updater_system_prompt),
                        &updater_user_message,
                        form_spec.clone(),
                        &soul,
                        &session_world,
                        &snapshot_user_text,
                        &visible_response_for_updater,
                        baseline_recent_event_id.clone(),
                        vec![EVALUATOR_MODE_PERCEPTION_V2.into()],
                        err,
                    )
                    .await;
                    fallback_result
                }
                Err(err) if selected_evaluator_source == EVALUATOR_MODE_STRUCTURED_V1 => {
                    if completion.structured_enforcement == Some(StructuredEnforcement::JsonSchema)
                    {
                        emit_dev_log(
                            &window,
                            "warn",
                            "evaluator",
                            "structured_schema_claim_failed",
                            Some(serde_json::json!({
                                "conversation_id": job.conversation_id.as_str(),
                                "assistant_message_id": job.assistant_message_id,
                                "evaluator_job_id": job.evaluator_job_id.as_str(),
                                "structured_enforcement_requested": StructuredEnforcement::JsonSchema.as_label(),
                                "structured_schema_validation_status": structured_validation_status_from_error(&err),
                                "structured_schema_validation_error": err.as_str()
                            })),
                        );
                    }
                    emit_dev_log(
                        &window,
                        "error",
                        "evaluator",
                        "structured_evaluator_failed",
                        Some(serde_json::json!({
                            "conversation_id": job.conversation_id.as_str(),
                            "assistant_message_id": job.assistant_message_id,
                            "evaluator_job_id": job.evaluator_job_id.as_str(),
                            "error": err.as_str(),
                            "structured_enforcement": completion.structured_enforcement.map(StructuredEnforcement::as_label)
                        })),
                    );
                    match retry_structured_tool_call_after_compile_failure(
                        &provider,
                        &state_updater_settings,
                        &updater_system_prompt,
                        &updater_user_message,
                        &completion,
                        &err,
                        &soul,
                        &session_world,
                        &snapshot_user_text,
                        &visible_response_for_updater,
                        baseline_recent_event_id.clone(),
                    )
                    .await
                    {
                        Ok(outcome) => Ok(outcome),
                        Err(retry_failure) => {
                            emit_dev_log(
                                &window,
                                "warn",
                                "evaluator",
                                "structured_evaluator_retry_failed",
                                Some(serde_json::json!({
                                    "conversation_id": job.conversation_id.as_str(),
                                    "assistant_message_id": job.assistant_message_id,
                                    "evaluator_job_id": job.evaluator_job_id.as_str(),
                                    "structured_retry_count": retry_failure.retry_count,
                                    "structured_retry_reasons": &retry_failure.retry_reasons,
                                    "structured_retry_final_error": retry_failure.final_error.as_str()
                                })),
                            );
                            emit_dev_log(
                                &window,
                                "warn",
                                "evaluator",
                                "structured_evaluator_fallback_to_form_started",
                                Some(serde_json::json!({
                                    "conversation_id": job.conversation_id.as_str(),
                                    "assistant_message_id": job.assistant_message_id,
                                    "evaluator_job_id": job.evaluator_job_id.as_str()
                                })),
                            );
                            let (fallback_result, _fallback_raw) = complete_form_fallback_runtime(
                                &provider,
                                &state_updater_settings,
                                fallback_form_system_prompt
                                    .as_deref()
                                    .unwrap_or(&updater_system_prompt),
                                &updater_user_message,
                                form_spec.clone(),
                                &soul,
                                &session_world,
                                &snapshot_user_text,
                                &visible_response_for_updater,
                                baseline_recent_event_id.clone(),
                                vec![structured_step.to_string()],
                                retry_failure.final_error.clone(),
                            )
                            .await;
                            match fallback_result {
                                Ok(mut outcome) => {
                                    apply_structured_retry_failure(&mut outcome, &retry_failure);
                                    emit_dev_log(
                                        &window,
                                        "success",
                                        "evaluator",
                                        "structured_evaluator_fallback_to_form_succeeded",
                                        Some(serde_json::json!({
                                            "conversation_id": job.conversation_id.as_str(),
                                            "assistant_message_id": job.assistant_message_id,
                                            "evaluator_job_id": job.evaluator_job_id.as_str(),
                                            "fallback_path": outcome.fallback_path
                                        })),
                                    );
                                    Ok(outcome)
                                }
                                Err(form_err) => {
                                    emit_dev_log(
                                        &window,
                                        "error",
                                        "evaluator",
                                        "structured_evaluator_fallback_to_form_failed",
                                        Some(serde_json::json!({
                                            "conversation_id": job.conversation_id.as_str(),
                                            "assistant_message_id": job.assistant_message_id,
                                            "evaluator_job_id": job.evaluator_job_id.as_str(),
                                            "error": form_err.as_str()
                                        })),
                                    );
                                    emit_dev_log(
                                        &window,
                                        "warn",
                                        "evaluator",
                                        "evaluator_noop_after_all_fallbacks",
                                        Some(serde_json::json!({
                                            "conversation_id": job.conversation_id.as_str(),
                                            "assistant_message_id": job.assistant_message_id,
                                            "evaluator_job_id": job.evaluator_job_id.as_str()
                                        })),
                                    );
                                    let mut outcome = evaluator_noop_after_all_fallbacks(
                                        vec![structured_step.to_string()],
                                        retry_failure.final_error.clone(),
                                        form_err,
                                    );
                                    apply_structured_retry_failure(&mut outcome, &retry_failure);
                                    Ok(outcome)
                                }
                            }
                        }
                    }
                }
                Err(err) => Err(err),
            }
        }
        Err(err)
            if matches!(
                selected_evaluator_source,
                EVALUATOR_MODE_STRUCTURED_V1 | EVALUATOR_MODE_PERCEPTION_V2
            ) =>
        {
            if structured_enforcement == Some(StructuredEnforcement::JsonSchema) {
                emit_dev_log(
                    &window,
                    "warn",
                    "evaluator",
                    "structured_schema_claim_failed",
                    Some(serde_json::json!({
                        "conversation_id": job.conversation_id.as_str(),
                        "assistant_message_id": job.assistant_message_id,
                        "evaluator_job_id": job.evaluator_job_id.as_str(),
                        "structured_enforcement_requested": StructuredEnforcement::JsonSchema.as_label(),
                        "structured_schema_validation_status": structured_validation_status_from_error(&err),
                        "structured_schema_validation_error": err.as_str()
                    })),
                );
            }
            emit_dev_log(
                &window,
                "error",
                "evaluator",
                "structured_evaluator_failed",
                Some(serde_json::json!({
                    "conversation_id": job.conversation_id.as_str(),
                    "assistant_message_id": job.assistant_message_id,
                    "evaluator_job_id": job.evaluator_job_id.as_str(),
                    "error": err.as_str(),
                    "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label)
                })),
            );
            emit_dev_log(
                &window,
                "warn",
                "evaluator",
                "structured_evaluator_fallback_to_form_started",
                Some(serde_json::json!({
                    "conversation_id": job.conversation_id.as_str(),
                    "assistant_message_id": job.assistant_message_id,
                    "evaluator_job_id": job.evaluator_job_id.as_str()
                })),
            );
            let (fallback_result, _fallback_raw) = complete_form_fallback_runtime(
                &provider,
                &state_updater_settings,
                fallback_form_system_prompt
                    .as_deref()
                    .unwrap_or(&updater_system_prompt),
                &updater_user_message,
                form_spec.clone(),
                &soul,
                &session_world,
                &snapshot_user_text,
                &visible_response_for_updater,
                baseline_recent_event_id.clone(),
                vec![
                    evaluator_fallback_origin(selected_evaluator_source, structured_enforcement)
                        .to_string(),
                ],
                err.clone(),
            )
            .await;
            match fallback_result {
                Ok(outcome) => {
                    emit_dev_log(
                        &window,
                        "success",
                        "evaluator",
                        "structured_evaluator_fallback_to_form_succeeded",
                        Some(serde_json::json!({
                            "conversation_id": job.conversation_id.as_str(),
                            "assistant_message_id": job.assistant_message_id,
                            "evaluator_job_id": job.evaluator_job_id.as_str(),
                            "fallback_path": outcome.fallback_path
                        })),
                    );
                    Ok(outcome)
                }
                Err(form_err) => {
                    emit_dev_log(
                        &window,
                        "error",
                        "evaluator",
                        "structured_evaluator_fallback_to_form_failed",
                        Some(serde_json::json!({
                            "conversation_id": job.conversation_id.as_str(),
                            "assistant_message_id": job.assistant_message_id,
                            "evaluator_job_id": job.evaluator_job_id.as_str(),
                            "error": form_err.as_str()
                        })),
                    );
                    emit_dev_log(
                        &window,
                        "warn",
                        "evaluator",
                        "evaluator_noop_after_all_fallbacks",
                        Some(serde_json::json!({
                            "conversation_id": job.conversation_id.as_str(),
                            "assistant_message_id": job.assistant_message_id,
                            "evaluator_job_id": job.evaluator_job_id.as_str()
                        })),
                    );
                    Ok(evaluator_noop_after_all_fallbacks(
                        vec![evaluator_fallback_origin(
                            selected_evaluator_source,
                            structured_enforcement,
                        )
                        .to_string()],
                        err,
                        form_err,
                    ))
                }
            }
        }
        Err(err) => Err(err),
    };
    let runtime = match runtime_result {
        Ok(mut output) => {
            if !drained_catchup_ids.is_empty() {
                let state = app.state::<AppState>();
                let conn = state.conn.lock();
                if let Ok(conn) = conn.as_deref() {
                    let _ = db::delete_evaluator_catchup_entries(
                        conn,
                        &job.conversation_id,
                        &drained_catchup_ids,
                    );
                }
                drop(conn);
            }
            if let Some(comparison_trace) =
                dual_compare_deferred_trace(&evaluator_mode, call_elapsed.as_millis(), false)
            {
                output.comparison_trace = Some(comparison_trace);
            }
            pipeline_trace.record_stage(
                "evaluator_response_parsed",
                "success",
                0,
                None,
                Some("JSON parsed successfully".to_string()),
            );
            pipeline_trace.record_stage(
                "evaluator_response_normalized",
                "success",
                0,
                None,
                Some("Normalized output generated".to_string()),
            );
            pipeline_trace.record_stage(
                "evaluator_response_validated",
                "success",
                0,
                None,
                Some("Validation constraints satisfied".to_string()),
            );
            window.emit("pipeline-trace-updated", &pipeline_trace).ok();
            output
        }
        Err(err) => {
            let err_str = err.to_string();
            let failing_stage = if err_str.contains("parse")
                || err_str.contains("JSON")
                || err_str.contains("syntax")
            {
                "evaluator_response_parsed"
            } else if err_str.contains("normalize") || err_str.contains("normalization") {
                "evaluator_response_normalized"
            } else {
                "evaluator_response_validated"
            };

            if failing_stage == "evaluator_response_parsed" {
                pipeline_trace.record_stage_error(
                    "evaluator_response_parsed",
                    0,
                    PipelineErrorCode::EvaluatorParseError,
                    err_str.clone(),
                    Some("Check LLM response formatting".to_string()),
                );
                pipeline_trace.record_stage(
                    "evaluator_response_normalized",
                    "skipped",
                    0,
                    None,
                    None,
                );
                pipeline_trace.record_stage(
                    "evaluator_response_validated",
                    "skipped",
                    0,
                    None,
                    None,
                );
            } else if failing_stage == "evaluator_response_normalized" {
                pipeline_trace.record_stage("evaluator_response_parsed", "success", 0, None, None);
                pipeline_trace.record_stage_error(
                    "evaluator_response_normalized",
                    0,
                    PipelineErrorCode::EvaluatorNormalizeError,
                    err_str.clone(),
                    Some("Check evaluator output normalization rules".to_string()),
                );
                pipeline_trace.record_stage(
                    "evaluator_response_validated",
                    "skipped",
                    0,
                    None,
                    None,
                );
            } else {
                pipeline_trace.record_stage("evaluator_response_parsed", "success", 0, None, None);
                pipeline_trace.record_stage(
                    "evaluator_response_normalized",
                    "success",
                    0,
                    None,
                    None,
                );
                pipeline_trace.record_stage_error(
                    "evaluator_response_validated",
                    0,
                    PipelineErrorCode::EvaluatorValidationError,
                    err_str.clone(),
                    Some("Check constraints, required keys, or type specifications".to_string()),
                );
            }

            pipeline_trace.final_status = "failed".into();
            pipeline_trace.failing_stage = Some(failing_stage.to_string());
            pipeline_trace.total_elapsed_ms = started.elapsed().as_millis() as u64;
            window.emit("pipeline-trace-updated", &pipeline_trace).ok();
            if let Some(n_id) = narrator_log_id {
                if let Ok(conn) = app.state::<AppState>().conn.lock() {
                    let trace_val = serde_json::json!({ "pipeline_trace": &pipeline_trace });
                    let _ = update_llm_payload_pipeline_trace(&conn, n_id, &trace_val);
                }
            }

            let status = if baseline_patch_id.is_some() {
                "partial_success"
            } else if evaluator_timed_out(&err, call_elapsed, &state_updater_settings) {
                "timed_out"
            } else {
                "failed"
            };
            let form_trace = failed_form_trace_json(selected_evaluator_source, form_spec.as_ref());
            let trace = serde_json::json!({
                "evaluator_trace": {
                    "evaluator_request_id": evaluator_request_id.as_str(),
                    "parent_narrator_request_id": parent_narrator_request_id.as_str(),
                    "turn_id": turn_id.as_deref(),
                    "provider": evaluator_provider_label(&evaluator_mode, true),
                    "model": state_updater_settings.model.trim(),
                    "evaluator_mode": evaluator_mode.as_str(),
                    "selected_evaluator_source": selected_evaluator_source,
                    "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label),
                    "raw_evaluator_response": raw_response.as_deref().unwrap_or_default(),
                    "normalized_evaluator_response": raw_response.as_deref().unwrap_or_default(),
                    "parsed_evaluator_json": serde_json::Value::Null,
                    "parse_status": "failed",
                    "parse_error": err.as_str(),
                    "evaluator_json_normalized": false,
                    "evaluator_normalization_warnings": [],
                    "elapsed_ms": call_elapsed.as_millis(),
                    "timeout_ms": job.timeout_ms,
                    "timeout_mode": job.timeout_mode.as_str()
                },
                "evaluator_mode": evaluator_mode.as_str(),
                "selected_evaluator_source": selected_evaluator_source,
                "evaluator_raw_response": raw_response.as_deref().unwrap_or_default(),
                "evaluator_parsed_json": {
                    "parse_status": "failed",
                    "parse_error": err.as_str(),
                    "evaluator_json_normalized": false,
                    "evaluator_normalization_warnings": []
                },
                "before_after_state_summary": {
                    "before": before_state_summary,
                    "after": serde_json::Value::Null
                }
            });
            let mut trace = trace;
            pipeline_trace.finalize_timing(started.elapsed().as_millis() as u64);
            trace["pipeline_trace"] = serde_json::to_value(&pipeline_trace).unwrap_or_default();
            if let Some(evaluator_trace) = trace.get_mut("evaluator_trace") {
                insert_json_object_fields(evaluator_trace, &form_trace);
            }
            insert_json_object_fields(&mut trace, &form_trace);
            if let Some(log_id) = updater_log_id {
                let state = app.state::<AppState>();
                if let Ok(conn) = state.conn.lock() {
                    let _ = update_llm_payload_pipeline_trace(&conn, log_id, &trace);
                };
            }
            emit_dev_log(
                &window,
                "error",
                "evaluator",
                "evaluator_parse_failed",
                Some(serde_json::json!({
                    "conversation_id": job.conversation_id.as_str(),
                    "assistant_message_id": job.assistant_message_id,
                    "evaluator_job_id": job.evaluator_job_id.as_str(),
                    "error": err
                })),
            );
            update_background_job_status(
                &app,
                &window,
                &job.evaluator_job_id,
                status,
                Some("Evaluator failed before producing a valid patch"),
                started,
                false,
            );
            if let Some(ref p_id) = profile_id {
                handle_evaluator_streak_and_fallback(
                    &app,
                    &window,
                    &job.conversation_id,
                    p_id,
                    false,
                );
            }
            // The primary evaluator never produced a body (transport/parse
            // failure). There are no ops to fix, but the turn still had a real
            // beat — fall back to a local re-extraction repair so state isn't
            // silently dropped. Skipped when this job is itself a repair.
            if repair_user_message_override.is_none()
                && (!snapshot_user_text.trim().is_empty()
                    || !visible_response_for_updater.trim().is_empty())
            {
                emit_evaluator_repair_signal(
                    &window,
                    job.conversation_id.as_str(),
                    job.assistant_message_id,
                    job.evaluator_job_id.as_str(),
                    "reextract",
                    &[],
                );
            }
            return;
        }
    };
    if let Some(ref trace) = runtime.form_trace {
        pipeline_trace.evaluator_row_traces = trace.evaluator_row_traces.clone();
    }
    let evaluator_output = runtime.output.clone();
    let conversion = runtime.conversion.clone();

    // Surface the failed ops (the system's own verdict — not the tool-call's) so
    // they are visible AND the frontend can auto-fire a focused background repair.
    // Three failure classes, in priority order:
    //   1. structured ops rejected (the original path),
    //   2. form rows rejected (form mode never reached repair before),
    //   3. empty/no-op patch despite a real exchange — re-extract from scratch.
    // Skipped when this job IS already a repair, to prevent repair-of-repair.
    if repair_user_message_override.is_none() {
        let structured_failed_ops =
            rejected_ops_for_repair(&runtime.normalized_json, &conversion.rejected_candidates);
        let form_failed_ops = runtime
            .form_trace
            .as_ref()
            .map(form_rejected_ops_for_repair)
            .unwrap_or_default();
        let has_exchange = !snapshot_user_text.trim().is_empty()
            || !visible_response_for_updater.trim().is_empty();
        if !structured_failed_ops.is_empty() {
            emit_evaluator_repair_signal(
                &window,
                job.conversation_id.as_str(),
                job.assistant_message_id,
                job.evaluator_job_id.as_str(),
                "fix_rejected",
                &structured_failed_ops,
            );
        } else if !form_failed_ops.is_empty() {
            emit_evaluator_repair_signal(
                &window,
                job.conversation_id.as_str(),
                job.assistant_message_id,
                job.evaluator_job_id.as_str(),
                "fix_rejected",
                &form_failed_ops,
            );
        } else if runtime.partial_success && has_exchange {
            // The evaluator ran but compiled to nothing despite a real beat;
            // ask the (e.g. local) repair model to re-extract the whole turn.
            emit_evaluator_repair_signal(
                &window,
                job.conversation_id.as_str(),
                job.assistant_message_id,
                job.evaluator_job_id.as_str(),
                "reextract",
                &[],
            );
        }
    }

    emit_dev_log(
        &window,
        "debug",
        "evaluator",
        "evaluator_json_parsed",
        Some(serde_json::json!({
            "conversation_id": job.conversation_id.as_str(),
            "assistant_message_id": job.assistant_message_id,
            "evaluator_job_id": job.evaluator_job_id.as_str(),
            "turn_flags_u64": evaluator_output.turn_flags_u64
        })),
    );
    for candidate_id in &conversion.accepted_candidate_ids {
        emit_dev_log(
            &window,
            "success",
            "evaluator",
            "evaluator_candidate_accepted",
            Some(serde_json::json!({
                "conversation_id": job.conversation_id.as_str(),
                "assistant_message_id": job.assistant_message_id,
                "evaluator_job_id": job.evaluator_job_id.as_str(),
                "candidate_id": candidate_id
            })),
        );
    }
    for rejection in &conversion.rejected_candidates {
        emit_dev_log(
            &window,
            "warn",
            "evaluator",
            "evaluator_candidate_rejected",
            Some(serde_json::json!({
                "conversation_id": job.conversation_id.as_str(),
                "assistant_message_id": job.assistant_message_id,
                "evaluator_job_id": job.evaluator_job_id.as_str(),
                "candidate_id": rejection.candidate_id,
                "reason": rejection.reason
            })),
        );
    }

    let candidate_trace = evaluator_candidate_trace_json(&evaluator_output, &conversion);
    let patch_compile_start = Instant::now();
    let mut engine_patch = sanitize_state_updater_patch(
        conversion.patch.clone(),
        &soul,
        &snapshot_user_text,
        &visible_response_for_updater,
    );
    strip_premature_world_events_from_updater_patch(
        &mut engine_patch,
        &snapshot_user_text,
        &visible_response_for_updater,
    );
    stamp_memory_provenance(
        &mut engine_patch,
        &job.conversation_id,
        Some(job.assistant_message_id),
        ledger_branch_id.as_deref(),
    );
    let patch_elapsed = patch_compile_start.elapsed().as_millis() as u64;
    let mut patch_status = "success";
    if !conversion.rejected_candidates.is_empty() {
        patch_status = "warning";
    }
    pipeline_trace.record_stage(
        "engine_patch_compiled",
        patch_status,
        patch_elapsed,
        Some(format!(
            "Rejected count: {}",
            conversion.rejected_candidates.len()
        )),
        Some(format!(
            "Accepted count: {}",
            conversion.accepted_candidate_ids.len()
        )),
    );
    window.emit("pipeline-trace-updated", &pipeline_trace).ok();
    let converter_trace = evaluator_converter_trace_json(&engine_patch, &conversion);
    let fallback_trace = evaluator_runtime_fallback_json(&runtime);
    emit_dev_log(
        &window,
        "success",
        "evaluator",
        "evaluator_patch_converted",
        Some(serde_json::json!({
            "conversation_id": job.conversation_id.as_str(),
            "assistant_message_id": job.assistant_message_id,
            "evaluator_job_id": job.evaluator_job_id.as_str(),
            "summary": engine_patch_summary(&engine_patch)
        })),
    );
    if engine_patch.is_empty() {
        emit_dev_log(
            &window,
            "info",
            "evaluator",
            "evaluator_patch_empty",
            Some(serde_json::json!({
                "conversation_id": job.conversation_id.as_str(),
                "assistant_message_id": job.assistant_message_id,
                "evaluator_job_id": job.evaluator_job_id.as_str()
            })),
        );
    }
    if evaluator_job_is_canceled(&app, &job.evaluator_job_id) {
        update_background_job_status(
            &app,
            &window,
            &job.evaluator_job_id,
            "canceled",
            Some("Canceled before ledger write"),
            started,
            false,
        );
        return;
    }

    let mut ledger_trace = serde_json::json!({
        "state_patch_id": serde_json::Value::Null,
        "turn_commit_id": serde_json::Value::Null,
        "branch_id": ledger_branch_id.as_deref(),
        "patch_stored": false,
        "patch_applied": false,
        "patch_apply_skipped_reason": serde_json::Value::Null,
        "branch_rebuilt": false,
        "applied_patch_count": 0,
        "skipped_patch_count": 0,
        "invalidated_patch_count": 0,
        "materialized_soul_updated": false,
        "materialized_session_world_updated": false
    });
    let mut enrichment_stale_skipped = false;
    let apply_result: Result<(), String> = (|| {
        let state = app.state::<AppState>();
        let conn = match state.conn.lock() {
            Ok(conn) => conn,
            Err(err) => {
                let message = err.to_string();
                update_background_job_status(
                    &app,
                    &window,
                    &job.evaluator_job_id,
                    "failed",
                    Some(&message),
                    started,
                    false,
                );
                return Err(message);
            }
        };
        if let Some(branch_id) = ledger_branch_id.as_deref() {
            let baseline_record = baseline_patch_id
                .as_ref()
                .and_then(|bp_id| db::get_state_patch(&conn, bp_id).ok());
            let source_turn_id = baseline_record
                .as_ref()
                .map(|record| record.turn_id.clone())
                .or_else(|| ledger_parent_turn_id.clone())
                .unwrap_or_else(|| job.turn_id.clone());
            let active_contains_source = db::active_branch_contains_turn(
                &conn,
                &job.conversation_id,
                branch_id,
                &source_turn_id,
            )
            .map_err(|err| err.to_string())?;
            if !active_contains_source {
                ledger_trace = serde_json::json!({
                    "state_patch_id": serde_json::Value::Null,
                    "baseline_patch_id": baseline_patch_id,
                    "enrichment_patch_id": serde_json::Value::Null,
                    "turn_commit_id": source_turn_id,
                    "branch_id": branch_id,
                    "patch_kind": "enrichment",
                    "patch_stored": false,
                    "patch_applied": false,
                    "patch_apply_skipped_reason": "source_turn_not_on_active_branch",
                    "stale_skipped": true,
                    "branch_rebuilt": false,
                    "applied_patch_count": 0,
                    "skipped_patch_count": 1,
                    "invalidated_patch_count": 0,
                    "materialized_soul_updated": false,
                    "materialized_session_world_updated": false
                });
                enrichment_stale_skipped = true;
                return Ok(());
            }
            let mut enrichment_id = None;
            let patch_record = if let Some(ref bp_id) = baseline_patch_id {
                if !engine_patch.is_empty() {
                    let rec = db::record_enrichment_patch_with_metadata(
                        &conn,
                        &source_turn_id,
                        &engine_patch,
                        Some(bp_id),
                        Some(job.assistant_message_id),
                        selected_variant_id,
                        Some(&job.evaluator_job_id),
                    )
                    .map_err(|err| err.to_string())?;
                    enrichment_id = Some(rec.patch_id.clone());
                    rec
                } else {
                    baseline_record.ok_or_else(|| format!("Baseline patch {bp_id} not found"))?
                }
            } else {
                let (_commit, pr) = db::record_turn_commit_with_patch_for_turn_id(
                    &conn,
                    &job.turn_id,
                    &job.conversation_id,
                    branch_id,
                    ledger_parent_turn_id.as_deref(),
                    ledger_user_message_id,
                    job.assistant_message_id,
                    selected_variant_id,
                    &engine_patch,
                    is_regenerated_variant,
                )
                .map_err(|err| err.to_string())?;
                pr
            };
            emit_dev_log(
                &window,
                "success",
                "evaluator",
                "evaluator_patch_stored",
                Some(serde_json::json!({
                    "conversation_id": job.conversation_id.as_str(),
                    "assistant_message_id": job.assistant_message_id,
                    "evaluator_job_id": job.evaluator_job_id.as_str(),
                    "state_patch_id": patch_record.patch_id.as_str(),
                    "patch_empty": engine_patch.is_empty()
                })),
            );
            let rebuild_start = Instant::now();
            let rebuilt = db::rebuild_session_state(&conn, &job.conversation_id, branch_id)
                .map_err(|err| err.to_string())?;
            soul = rebuilt.soul;
            session_world = rebuilt.session_world;
            db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
            db::upsert_session_world(&conn, &session_world).map_err(|err| err.to_string())?;
            let rebuild_elapsed = rebuild_start.elapsed().as_millis() as u64;
            pipeline_trace.record_stage(
                "session_state_rebuilt",
                "success",
                rebuild_elapsed,
                Some(format!(
                    "Applied patches: {}",
                    rebuilt.debug.applied_patches.len()
                )),
                Some(format!("Rebuilt Soul turn counter: {}", soul.turn_counter)),
            );
            window.emit("pipeline-trace-updated", &pipeline_trace).ok();
            if let Some(log_id) = updater_log_id {
                let _ = db::set_llm_payload_log_ledger_metadata(
                    &conn,
                    log_id,
                    &rebuilt.debug,
                    ledger_parent_turn_id.as_deref(),
                    selected_variant_id,
                );
            }
            ledger_trace = serde_json::json!({
                "state_patch_id": patch_record.patch_id,
                "baseline_patch_id": baseline_patch_id,
                "enrichment_patch_id": enrichment_id,
                "turn_commit_id": source_turn_id,
                "branch_id": branch_id,
                "patch_kind": if enrichment_id.is_some() { "enrichment" } else { "baseline" },
                "parent_baseline_patch_id": baseline_patch_id,
                "source_turn_id": source_turn_id,
                "source_assistant_message_id": job.assistant_message_id,
                "source_assistant_variant_id": selected_variant_id,
                "created_by_job_id": job.evaluator_job_id,
                "patch_stored": true,
                "patch_applied": !engine_patch.is_empty(),
                "patch_apply_skipped_reason": if engine_patch.is_empty() { Some("empty_patch_recorded_in_ledger") } else { None },
                "branch_rebuilt": true,
                "applied_patch_count": rebuilt.debug.applied_patches.len(),
                "skipped_patch_count": rebuilt.debug.skipped_discarded_patches.len(),
                "invalidated_patch_count": rebuilt.debug.invalidated_patches.len(),
                "materialized_soul_updated": true,
                "materialized_session_world_updated": true
            });
            Ok(())
        } else {
            let rebuild_start = Instant::now();
            let report = engine_patch
                .apply_to_session(&mut soul, Some(&mut session_world))
                .map_err(|err| format!("{err:?}"))?;
            soul.turn_counter += 1;
            soul.turns_since_consolidation += 1;
            db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
            db::upsert_session_world(&conn, &session_world).map_err(|err| err.to_string())?;
            let rebuild_elapsed = rebuild_start.elapsed().as_millis() as u64;
            pipeline_trace.record_stage(
                "session_state_rebuilt",
                "success",
                rebuild_elapsed,
                None,
                Some(format!(
                    "Rebuilt directly Soul turn counter: {}",
                    soul.turn_counter
                )),
            );
            window.emit("pipeline-trace-updated", &pipeline_trace).ok();
            emit_relationship_delta_logs(&window, &job.conversation_id, &engine_patch);
            emit_memory_apply_logs(&window, &job.conversation_id, &report.memory_events);
            ledger_trace = serde_json::json!({
                "state_patch_id": serde_json::Value::Null,
                "turn_commit_id": serde_json::Value::Null,
                "branch_id": serde_json::Value::Null,
                "patch_stored": false,
                "patch_applied": !engine_patch.is_empty(),
                "patch_apply_skipped_reason": if engine_patch.is_empty() { Some("empty_patch") } else { None },
                "branch_rebuilt": false,
                "applied_patch_count": if engine_patch.is_empty() { 0 } else { 1 },
                "skipped_patch_count": if engine_patch.is_empty() { 1 } else { 0 },
                "invalidated_patch_count": 0,
                "materialized_soul_updated": true,
                "materialized_session_world_updated": true
            });
            Ok(())
        }
    })();

    if let Err(err) = apply_result {
        pipeline_trace.record_stage_error(
            "session_state_rebuilt",
            0,
            PipelineErrorCode::DatabaseError,
            err.clone(),
            Some("Check database integrity or constraints".to_string()),
        );
        pipeline_trace.record_stage("memory_delta_extracted", "skipped", 0, None, None);
        pipeline_trace.record_stage("memory_patch_committed", "skipped", 0, None, None);
        pipeline_trace.record_stage("relationship_consolidation_ran", "skipped", 0, None, None);
        pipeline_trace.final_status = "failed".into();
        pipeline_trace.failing_stage = Some("session_state_rebuilt".to_string());
        pipeline_trace.total_elapsed_ms = started.elapsed().as_millis() as u64;
        window.emit("pipeline-trace-updated", &pipeline_trace).ok();
        let form_trace = runtime_form_trace_json(&runtime);
        let mut trace = serde_json::json!({
            "evaluator_trace": {
                "evaluator_request_id": evaluator_request_id.as_str(),
                "parent_narrator_request_id": parent_narrator_request_id.as_str(),
                "turn_id": turn_id.as_deref(),
                "provider": evaluator_provider_label(&evaluator_mode, true),
                "model": state_updater_settings.model.trim(),
                "evaluator_mode": evaluator_mode.as_str(),
                "selected_evaluator_source": selected_evaluator_source,
                "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label),
                "raw_evaluator_response": raw_response.as_deref().unwrap_or_default(),
                "normalized_evaluator_response": runtime.normalized_json.as_str(),
                "parsed_evaluator_json": &evaluator_output,
                "parse_status": "success",
                "parse_error": serde_json::Value::Null,
                "evaluator_json_normalized": runtime.normalized,
                "evaluator_normalization_warnings": &runtime.warnings,
                "draft_created": true,
                "draft_memory_candidate_count": runtime.draft.memory_candidate_count,
                "draft_world_event_count": runtime.draft.world_event_count,
                "draft_scene_state_present": runtime.draft.scene_state_present,
                "draft_relationship_delta_count": runtime.draft.relationship_delta_count,
                "candidate_quality_decisions": &runtime.draft.candidate_quality_decisions,
                "candidate_routing_decisions": &runtime.draft.candidate_routing_decisions,
                "state_effect_guarantee_applied": runtime.draft.state_effect_guarantee_applied,
                "state_effect_guarantee_reason": runtime.draft.state_effect_guarantee_reason.as_deref(),
                "comparison_trace": runtime.comparison_trace.as_ref(),
                "elapsed_ms": call_elapsed.as_millis(),
                "timeout_ms": job.timeout_ms,
                "timeout_mode": job.timeout_mode.as_str(),
                "compiled_patch_summary": engine_patch_summary(&engine_patch)
            },
            "evaluator_mode": evaluator_mode.as_str(),
            "selected_evaluator_source": selected_evaluator_source,
            "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label),
            "evaluator_raw_response": raw_response.as_deref().unwrap_or_default(),
            "evaluator_parsed_json": &evaluator_output,
            "evaluator_json_normalized": runtime.normalized,
            "evaluator_normalization_warnings": &runtime.warnings,
            "draft_created": true,
            "draft_memory_candidate_count": runtime.draft.memory_candidate_count,
            "draft_world_event_count": runtime.draft.world_event_count,
            "draft_scene_state_present": runtime.draft.scene_state_present,
            "draft_relationship_delta_count": runtime.draft.relationship_delta_count,
            "candidate_quality_decisions": &runtime.draft.candidate_quality_decisions,
            "candidate_routing_decisions": &runtime.draft.candidate_routing_decisions,
            "state_effect_guarantee_applied": runtime.draft.state_effect_guarantee_applied,
            "state_effect_guarantee_reason": runtime.draft.state_effect_guarantee_reason.as_deref(),
            "comparison_trace": runtime.comparison_trace.as_ref(),
            "evaluator_candidate_trace": candidate_trace,
            "converted_engine_patch": converter_trace,
            "compiled_patch_summary": engine_patch_summary(&engine_patch),
            "ledger_apply_trace": ledger_trace,
            "conversion_error": err.as_str(),
            "before_after_state_summary": {
                "before": before_state_summary,
                "after": serde_json::Value::Null
            }
        });
        pipeline_trace.finalize_timing(started.elapsed().as_millis() as u64);
        trace["pipeline_trace"] = serde_json::to_value(&pipeline_trace).unwrap_or_default();
        if let Some(evaluator_trace) = trace.get_mut("evaluator_trace") {
            insert_json_object_fields(evaluator_trace, &form_trace);
            insert_json_object_fields(evaluator_trace, &fallback_trace);
        }
        insert_json_object_fields(&mut trace, &form_trace);
        insert_json_object_fields(&mut trace, &fallback_trace);
        if let Some(log_id) = updater_log_id {
            let state = app.state::<AppState>();
            if let Ok(conn) = state.conn.lock() {
                let _ = update_llm_payload_pipeline_trace(&conn, log_id, &trace);
            };
        }
        let final_status = if baseline_patch_id.is_some() {
            "partial_success"
        } else {
            "failed"
        };
        update_background_job_status(
            &app,
            &window,
            &job.evaluator_job_id,
            final_status,
            Some(&err),
            started,
            false,
        );
        if let Some(ref p_id) = profile_id {
            handle_evaluator_streak_and_fallback(&app, &window, &job.conversation_id, p_id, false);
        }
        return;
    }

    let memory_candidates_count = engine_patch
        .soul_patch
        .as_ref()
        .map(|sp| sp.new_memories.len() + sp.memory_operations.len())
        .unwrap_or(0);
    let memory_extract_status = if memory_candidates_count > 0 {
        "success"
    } else {
        "skipped"
    };
    pipeline_trace.record_stage(
        "memory_delta_extracted",
        memory_extract_status,
        0,
        Some(format!(
            "Extracted memories count: {memory_candidates_count}"
        )),
        Some(format!(
            "Memory patch has {} operations",
            memory_candidates_count
        )),
    );

    let memory_commit_status = if memory_candidates_count > 0 {
        "success"
    } else {
        "skipped"
    };
    pipeline_trace.record_stage(
        "memory_patch_committed",
        memory_commit_status,
        0,
        Some(format!(
            "Committed memories count: {memory_candidates_count}"
        )),
        Some(format!("Saved memory updates: {}", memory_candidates_count)),
    );

    pipeline_trace.record_stage(
        "relationship_consolidation_ran",
        "skipped",
        0,
        None,
        Some("Bypassed because consolidation was not triggered for this turn".to_string()),
    );
    window.emit("pipeline-trace-updated", &pipeline_trace).ok();

    emit_per_soul_memory_written_logs(&window, &job.conversation_id, &engine_patch);
    emit_dev_log(
        &window,
        if engine_patch.is_empty() || enrichment_stale_skipped {
            "info"
        } else {
            "success"
        },
        "evaluator",
        if engine_patch.is_empty() || enrichment_stale_skipped {
            "evaluator_patch_apply_skipped_reason"
        } else {
            "evaluator_patch_applied"
        },
        Some(serde_json::json!({
            "conversation_id": job.conversation_id.as_str(),
            "assistant_message_id": job.assistant_message_id,
            "evaluator_job_id": job.evaluator_job_id.as_str(),
            "reason": if enrichment_stale_skipped {
                "source_turn_not_on_active_branch"
            } else if engine_patch.is_empty() {
                "empty_patch"
            } else {
                "background_evaluator_applied_patch"
            }
        })),
    );
    if !enrichment_stale_skipped {
        emit_dev_log(
            &window,
            "success",
            "ledger",
            "materialized_state_refreshed",
            Some(serde_json::json!({
                "conversation_id": job.conversation_id.as_str(),
                "assistant_message_id": job.assistant_message_id,
                "evaluator_job_id": job.evaluator_job_id.as_str(),
                "soul_id": soul.character_id.as_str(),
                "world_id": session_world.world_id.as_str()
            })),
        );
    }
    let form_trace = runtime_form_trace_json(&runtime);
    let mut final_trace = serde_json::json!({
        "evaluator_trace": {
            "evaluator_request_id": evaluator_request_id.as_str(),
            "parent_narrator_request_id": parent_narrator_request_id.as_str(),
            "turn_id": turn_id.as_deref(),
            "provider": evaluator_provider_label(&evaluator_mode, true),
            "model": state_updater_settings.model.trim(),
            "evaluator_mode": evaluator_mode.as_str(),
            "selected_evaluator_source": selected_evaluator_source,
            "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label),
            "raw_evaluator_response": raw_response.as_deref().unwrap_or_default(),
            "normalized_evaluator_response": runtime.normalized_json.as_str(),
            "parsed_evaluator_json": &evaluator_output,
            "parse_status": "success",
            "parse_error": serde_json::Value::Null,
            "evaluator_json_normalized": runtime.normalized,
            "evaluator_normalization_warnings": &runtime.warnings,
            "draft_created": true,
            "draft_memory_candidate_count": runtime.draft.memory_candidate_count,
            "draft_world_event_count": runtime.draft.world_event_count,
            "draft_scene_state_present": runtime.draft.scene_state_present,
            "draft_relationship_delta_count": runtime.draft.relationship_delta_count,
            "candidate_quality_decisions": &runtime.draft.candidate_quality_decisions,
            "candidate_routing_decisions": &runtime.draft.candidate_routing_decisions,
            "state_effect_guarantee_applied": runtime.draft.state_effect_guarantee_applied,
            "state_effect_guarantee_reason": runtime.draft.state_effect_guarantee_reason.as_deref(),
            "comparison_trace": runtime.comparison_trace.as_ref(),
            "comparison_skipped_or_timed_out": evaluator_mode == EVALUATOR_MODE_DUAL_COMPARE,
            "selected_path_elapsed_ms": call_elapsed.as_millis(),
            "comparison_path_elapsed_ms": serde_json::Value::Null,
            "selected_patch_applied_before_comparison_done": evaluator_mode == EVALUATOR_MODE_DUAL_COMPARE,
            "evaluator_flags_u64": evaluator_output.turn_flags_u64,
            "turn_classification": &evaluator_output.turn_classification,
            "no_op_reason": evaluator_output.no_op_reason.as_deref(),
            "elapsed_ms": call_elapsed.as_millis(),
            "timeout_ms": job.timeout_ms,
            "timeout_mode": job.timeout_mode.as_str(),
            "compiled_patch_summary": engine_patch_summary(&engine_patch)
        },
        "evaluator_mode": evaluator_mode.as_str(),
        "selected_evaluator_source": selected_evaluator_source,
        "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label),
        "evaluator_raw_response": raw_response.as_deref().unwrap_or_default(),
        "evaluator_parsed_json": &evaluator_output,
        "evaluator_json_normalized": runtime.normalized,
        "evaluator_normalization_warnings": &runtime.warnings,
        "draft_created": true,
        "draft_memory_candidate_count": runtime.draft.memory_candidate_count,
        "draft_world_event_count": runtime.draft.world_event_count,
        "draft_scene_state_present": runtime.draft.scene_state_present,
        "draft_relationship_delta_count": runtime.draft.relationship_delta_count,
        "candidate_quality_decisions": &runtime.draft.candidate_quality_decisions,
        "candidate_routing_decisions": &runtime.draft.candidate_routing_decisions,
        "state_effect_guarantee_applied": runtime.draft.state_effect_guarantee_applied,
        "state_effect_guarantee_reason": runtime.draft.state_effect_guarantee_reason.as_deref(),
        "comparison_trace": runtime.comparison_trace.as_ref(),
        "comparison_skipped_or_timed_out": evaluator_mode == EVALUATOR_MODE_DUAL_COMPARE,
        "selected_path_elapsed_ms": call_elapsed.as_millis(),
        "comparison_path_elapsed_ms": serde_json::Value::Null,
        "selected_patch_applied_before_comparison_done": evaluator_mode == EVALUATOR_MODE_DUAL_COMPARE,
        "evaluator_candidate_trace": candidate_trace,
        "converted_engine_patch": converter_trace,
        "compiled_patch_summary": engine_patch_summary(&engine_patch),
        "ledger_apply_trace": ledger_trace,
        "before_after_state_summary": {
            "before": before_state_summary,
            "after": compact_state_summary_json(&soul, &session_world)
        }
    });
    pipeline_trace.final_status = if enrichment_stale_skipped {
        "canceled".to_string()
    } else if runtime.partial_success || baseline_patch_id.is_some() && engine_patch.is_empty() {
        "partial_success".to_string()
    } else {
        "success".to_string()
    };
    pipeline_trace.finalize_timing(started.elapsed().as_millis() as u64);
    final_trace["pipeline_trace"] = serde_json::to_value(&pipeline_trace).unwrap_or_default();
    if let Some(evaluator_trace) = final_trace.get_mut("evaluator_trace") {
        insert_json_object_fields(evaluator_trace, &form_trace);
        insert_json_object_fields(evaluator_trace, &fallback_trace);
    }
    insert_json_object_fields(&mut final_trace, &form_trace);
    insert_json_object_fields(&mut final_trace, &fallback_trace);
    if let Some(log_id) = updater_log_id {
        let state = app.state::<AppState>();
        if let Ok(conn) = state.conn.lock() {
            let _ = update_llm_payload_pipeline_trace(&conn, log_id, &final_trace);
        };
    }
    let final_job_status = if enrichment_stale_skipped {
        "stale_skipped"
    } else if runtime.partial_success || baseline_patch_id.is_some() && engine_patch.is_empty() {
        "partial_success"
    } else {
        "completed"
    };
    let honest_status_str;
    // A partial success has to say what was partial about it. Without this, a
    // run where every fallback timed out records `partial_success`, no error,
    // and an empty patch — a row indistinguishable from a turn where nothing
    // happened to change, which is how a dead evaluator stayed invisible for a
    // whole benchmark.
    let error_msg = if enrichment_stale_skipped {
        Some("source turn is no longer on active branch")
    } else if !runtime.form_rejected_rows.is_empty() {
        honest_status_str = state_engine::evaluator_form::format_honest_ui_status(
            !engine_patch.is_empty(),
            true,
            true,
            &runtime.form_rejected_rows,
        );
        Some(honest_status_str.as_str())
    } else if runtime.partial_success {
        Some(
            runtime
                .partial_success_reason
                .as_deref()
                .unwrap_or("the evaluator finished without applying a state patch"),
        )
    } else {
        None
    };
    update_background_job_status(
        &app,
        &window,
        &job.evaluator_job_id,
        final_job_status,
        error_msg,
        started,
        !engine_patch.is_empty() && !enrichment_stale_skipped,
    );

    if !enrichment_stale_skipped {
        if let Some(ref p_id) = profile_id {
            let is_success_nonempty = final_job_status == "completed" && !engine_patch.is_empty();
            handle_evaluator_streak_and_fallback(
                &app,
                &window,
                &job.conversation_id,
                p_id,
                is_success_nonempty,
            );
        }
    }
}

fn handle_evaluator_streak_and_fallback(
    app: &AppHandle,
    window: &Window,
    conversation_id: &str,
    profile_id: &str,
    is_success_nonempty: bool,
) {
    let state = app.state::<AppState>();
    let conn_guard = match state.conn.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    let conn = &*conn_guard;

    if is_success_nonempty {
        let _ = db::reset_evaluator_empty_patch_streak(conn, conversation_id, profile_id);
    } else {
        if let Ok(new_streak) =
            db::increment_evaluator_empty_patch_streak(conn, conversation_id, profile_id)
        {
            let _ = window.emit(
                "evaluator_empty_patch_streak_incremented",
                serde_json::json!({
                    "conversation_id": conversation_id,
                    "profile_id": profile_id,
                    "streak": new_streak,
                }),
            );

            if new_streak >= 2 {
                if let Ok(Some(fallback_profile)) = db::get_last_known_good_evaluator_profile(conn)
                {
                    if let Ok(_) = db::set_active_evaluator_profile(
                        conn,
                        conversation_id,
                        Some(&fallback_profile.id),
                    ) {
                        let _ = db::reset_evaluator_empty_patch_streak(
                            conn,
                            conversation_id,
                            profile_id,
                        );
                        let _ = window.emit(
                            "evaluator_auto_fallback_triggered",
                            serde_json::json!({
                                "conversation_id": conversation_id,
                                "profile_id": fallback_profile.id,
                            }),
                        );
                    }
                }
            }
        }
    }
}

fn update_llm_payload_pipeline_trace(
    conn: &Connection,
    log_id: i64,
    trace: &serde_json::Value,
) -> rusqlite::Result<bool> {
    let pipeline_trace_json =
        serde_json::to_string_pretty(trace).unwrap_or_else(|_| trace.to_string());
    db::update_llm_payload_log_response(
        conn,
        log_id,
        &db::LlmPayloadResponseUpdate {
            pipeline_trace_json: Some(pipeline_trace_json),
            ..Default::default()
        },
    )
}

fn compact_state_summary_json(soul: &Soul, session_world: &SessionWorld) -> serde_json::Value {
    serde_json::json!({
        "soul.turn_counter": soul.turn_counter,
        "session_world.scene_state": session_world.scene_state,
        "recent_event_count": session_world.recent_events.len(),
        "memory_recent_count": soul.memory.recent.len(),
        "object_state_count": session_world.object_states.len(),
        "relationship_summary": relationship_summary_json(soul),
    })
}

fn relationship_summary_json(soul: &Soul) -> serde_json::Value {
    let mut relationships = serde_json::Map::new();
    let mut ids = soul.relationships.keys().cloned().collect::<Vec<_>>();
    ids.sort();
    for id in ids {
        if let Some(relationship) = soul.relationships.get(&id) {
            relationships.insert(
                id,
                serde_json::json!({
                    "trust": relationship.trust,
                    "affection": relationship.affection,
                    "intimacy": relationship.intimacy,
                    "fear": relationship.fear,
                    "respect": relationship.respect,
                    "conflict": relationship.conflict,
                    "comfort": relationship.comfort,
                    "boundary_pressure": relationship.boundary_pressure,
                }),
            );
        }
    }
    serde_json::Value::Object(relationships)
}

fn evaluator_candidate_trace_json(
    output: &EvaluatorOutputV1,
    conversion: &EvaluatorConversionReport,
) -> serde_json::Value {
    let accepted = conversion
        .accepted_candidate_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let rejected = conversion
        .rejected_candidates
        .iter()
        .map(|rejection| (rejection.candidate_id.as_str(), rejection.reason.as_str()))
        .collect::<HashMap<_, _>>();
    let candidates = output
        .memory_candidates
        .iter()
        .chain(
            output
                .per_soul_evaluations
                .iter()
                .flat_map(|soul| soul.memory_candidates.iter()),
        )
        .map(|candidate| {
            let candidate_id = candidate.candidate_id.as_str();
            serde_json::json!({
                "candidate_id": candidate.candidate_id,
                "owner_soul_id": candidate.owner_soul_id,
                "slot": candidate.slot.as_label(),
                "content": candidate.content,
                "evidence_quote": candidate.evidence_quote,
                "confidence": candidate.confidence,
                "salience": candidate.salience,
                "retrieval_strength": candidate.retrieval_strength,
                "target_entity_ids": candidate.target_entity_ids,
                "relevance_tags": candidate.relevance_tags,
                "evidence_validation": conversion.evidence_validations.iter().find(|trace| trace.candidate_id == candidate.candidate_id).map(|trace| serde_json::json!({
                    "evidence_validation_raw": trace.evidence_validation_raw,
                    "evidence_validation_normalized": trace.evidence_validation_normalized,
                    "evidence_validation_match_source": trace.evidence_validation_match_source,
                    "evidence_validation_result": trace.evidence_validation_result,
                })),
                "accepted": accepted.contains(candidate_id),
                "rejection_reason": rejected.get(candidate_id).copied(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(candidates)
}

fn evaluator_converter_trace_json(
    patch: &EnginePatch,
    conversion: &EvaluatorConversionReport,
) -> serde_json::Value {
    let soul_patch = patch.soul_patch.as_ref();
    let world_patch = patch.world_patch.as_ref();
    let relationship_patch_count = soul_patch
        .map(|patch| {
            patch.relationship_deltas.len()
                + usize::from(patch.relationship_delta.as_ref().is_some())
        })
        .unwrap_or(0);
    let object_patch_count = world_patch
        .map(|patch| {
            patch.corrected_object_states.len() + patch.object_observation_operations.len()
        })
        .unwrap_or(0);
    serde_json::json!({
        "converted_patch_json": patch,
        "patch_empty": patch.is_empty(),
        "world_patch_summary": engine_patch_summary(patch).get("world_patch").cloned().unwrap_or(serde_json::Value::Null),
        "memory_patch_count": soul_patch.map(|patch| patch.new_memories.len() + patch.memory_operations.len()).unwrap_or(0),
        "relationship_patch_count": relationship_patch_count,
        "object_patch_count": object_patch_count,
        "scene_state_patch_present": world_patch.and_then(|patch| patch.scene_state.as_ref()).is_some(),
        "entity_aliases_resolved": &conversion.entity_aliases_resolved,
        "entity_alias_resolution_warnings": &conversion.entity_alias_resolution_warnings,
        "conversion_warnings": conversion.rejected_candidates.iter().map(|rejection| {
            serde_json::json!({
                "candidate_id": rejection.candidate_id,
                "reason": rejection.reason,
            })
        }).collect::<Vec<_>>(),
    })
}

pub(crate) fn scene_state_present(session_world: &SessionWorld) -> bool {
    let scene = &session_world.scene_state;
    !scene.scene_state_id.trim().is_empty()
        || !scene.current_scene.trim().is_empty()
        || !scene.resolved_active_plot.trim().is_empty()
        || !scene.scene_branch.trim().is_empty()
        || !scene.focus.trim().is_empty()
        || !scene.participants.is_empty()
        || !scene.last_user_action.trim().is_empty()
        || !scene.pressure_point.trim().is_empty()
        || !scene.continuity_note.trim().is_empty()
}

fn hidden_state_from_engine_patch(patch: &EnginePatch) -> HiddenState {
    let relationship = patch.soul_patch.as_ref().and_then(|patch| {
        patch
            .relationship_delta
            .as_ref()
            .or_else(|| patch.relationship_deltas.first())
    });
    let memory = patch.soul_patch.as_ref().and_then(|patch| {
        patch
            .new_memories
            .iter()
            .find(|memory| !memory.content.trim().is_empty())
    });
    let world = patch.world_patch.as_ref();
    let body = patch.body_patch.as_ref();
    HiddenState {
        memory: memory.map(|memory| memory.content.clone()),
        tag: memory.and_then(|memory| memory.tag.clone()),
        trust_delta: relationship.and_then(|delta| delta.trust),
        affection_delta: relationship.and_then(|delta| delta.affection),
        world_event: world.and_then(|patch| {
            patch.recent_event.clone().or_else(|| {
                patch
                    .recent_events
                    .iter()
                    .find(|event| !event.trim().is_empty())
                    .cloned()
            })
        }),
        new_location: world.and_then(|patch| patch.location.clone()),
        present_characters: None,
        arousal_delta: body.and_then(|patch| patch.activation_delta),
        arousal_denied: body.and_then(|patch| patch.activation_blocked),
        orgasm_allowed: body.and_then(|patch| patch.peak_allowed),
        forced_orgasm: body.and_then(|patch| patch.forced_peak),
    }
}

fn sanitize_state_updater_patch(
    mut patch: EnginePatch,
    soul: &Soul,
    user_text: &str,
    narrator_response: &str,
) -> EnginePatch {
    let turn_text = format!("{user_text}\n{narrator_response}");
    tag_memory_sources_from_turn(&mut patch, user_text, &turn_text);
    apply_state_truth_boundary(&mut patch, user_text, narrator_response);
    let threat_scene = is_threat_or_emergency(&turn_text) || latest_world_event_is_threat(soul);
    let explicit_intimacy = explicitly_intimate(user_text);
    if let Some(body_patch) = patch.body_patch.as_mut() {
        if threat_scene && !explicit_intimacy {
            if body_patch.activation_delta.unwrap_or(0.0) > 0.0 {
                body_patch.activation_delta = Some(0.0);
            }
            body_patch.peak_allowed = Some(false);
            body_patch.forced_peak = Some(false);
        }
    }
    if patch
        .body_patch
        .as_ref()
        .map_or(false, |body| body.is_empty_for_commands())
    {
        patch.body_patch = None;
    }

    if patch.world_patch.is_none() && is_retcon_or_correction_text(user_text) {
        patch.world_patch = Some(Default::default());
    }
    if let Some(world_patch) = patch.world_patch.as_mut() {
        if let Some(time) = world_patch.time_elapsed.as_mut() {
            *time = normalize_time_for_updater(time);
        }
        if world_patch.time_elapsed.is_some() && !user_text_has_explicit_time(user_text) {
            world_patch.time_elapsed = None;
        }
        if should_replace_default_plot(soul, world_patch, &turn_text) {
            if let Some(plot) = infer_active_plot(&turn_text) {
                world_patch
                    .active_plot_resolve
                    .push("Establish the first scene".into());
                world_patch
                    .active_plot_resolve
                    .push("Establish the first scene - Aurora is alone, expecting company, or has just let someone in.".into());
                world_patch.active_plot_add.push(plot.into());
            }
        }
        cleanup_stale_active_plots(soul, world_patch, &turn_text);
        if is_retcon_or_correction_text(user_text) {
            world_patch.retcon_scope.get_or_insert("latest_turn".into());
            world_patch.correction_note.get_or_insert_with(|| {
                "Retcon: phone did not buzz because notifications were off / no vibration / screen wake disabled.".into()
            });
        }
        if world_patch.is_empty_for_commands() {
            patch.world_patch = None;
        }
    }

    patch
}

/// Stamp engine-owned creating-turn provenance onto every new memory so each
/// memory can answer "which exchange created you?". These address fields are
/// never trusted from evaluator output and are overwritten before ledger commit.
fn stamp_memory_provenance(
    patch: &mut EnginePatch,
    conversation_id: &str,
    assistant_message_id: Option<i64>,
    session_id: Option<&str>,
) {
    let Some(soul_patch) = patch.soul_patch.as_mut() else {
        return;
    };
    let session_id = session_id.map(str::trim).filter(|id| !id.is_empty());
    for memory in &mut soul_patch.new_memories {
        // The address is system-set, not AI-supplied: which chat log
        // (conversation), which line (assistant message), and which session
        // (branch).
        memory.source_conversation_id = Some(conversation_id.to_string());
        memory.source_message_id = assistant_message_id;
        memory.source_session_id = session_id.map(str::to_string);
    }
}

fn apply_state_truth_boundary(patch: &mut EnginePatch, user_text: &str, narrator_response: &str) {
    let Some(soul_patch) = patch.soul_patch.as_mut() else {
        return;
    };
    for memory in &mut soul_patch.new_memories {
        let content = memory.content.trim();
        if content.is_empty() {
            continue;
        }
        let architecture_claim = is_architecture_claim_text(content);
        let user_arch_claim =
            is_architecture_claim_text(user_text) || is_user_system_truth_claim(user_text);
        let narrator_arch_claim =
            architecture_claim && is_architecture_claim_text(narrator_response);

        if memory.truth_status.is_none() {
            memory.truth_status = Some(match memory.source_type {
                Some(MemorySourceType::UserClaimed) => TruthStatus::UserClaimed,
                Some(MemorySourceType::NarratorInferred) => TruthStatus::NarratorClaim,
                Some(MemorySourceType::SystemGenerated) => TruthStatus::Unknown,
                Some(
                    MemorySourceType::ImportedLog
                    | MemorySourceType::PreviousSession
                    | MemorySourceType::CrossSessionBleed,
                ) => TruthStatus::Unknown,
                _ => TruthStatus::SceneEvent,
            });
        }

        if memory
            .truth_status
            .map_or(false, TruthStatus::is_engine_verified)
        {
            memory.truth_status = Some(if user_arch_claim {
                TruthStatus::UserClaimed
            } else if architecture_claim || narrator_arch_claim {
                TruthStatus::NarratorClaim
            } else {
                TruthStatus::SceneEvent
            });
        }

        let user_claim_applies =
            user_arch_claim && (architecture_claim || memory_mentions_user_claim(content));
        if architecture_claim || user_claim_applies || narrator_arch_claim {
            memory.architecture_verified = Some(false);
            memory.is_lived_experience.get_or_insert(false);
            memory.confidence = Some(memory.confidence.unwrap_or(0.45).min(0.6));
            memory.truth_status = Some(if user_claim_applies {
                TruthStatus::UserClaimed
            } else if content.to_ascii_lowercase().contains("believes")
                || content.to_ascii_lowercase().contains("thought")
            {
                TruthStatus::CharacterBelief
            } else if architecture_claim || narrator_arch_claim {
                TruthStatus::NarratorClaim
            } else {
                TruthStatus::Unknown
            });
            if memory.source_type.is_none() {
                memory.source_type = Some(if user_arch_claim {
                    MemorySourceType::UserClaimed
                } else {
                    MemorySourceType::NarratorInferred
                });
            }
        } else {
            memory.architecture_verified.get_or_insert(false);
        }
    }
}

fn is_architecture_claim_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    contains_any_text(
        &lower,
        &[
            "the system responded",
            "system responded",
            "memory layer contacted",
            "memory layer responded",
            "memory layer talked",
            "state updater",
            "direct state injection",
            "model spoke from beneath",
            "provider responded",
            "backend layer",
            "backend is listening",
            "hidden system",
            "internal architecture",
            "api responded",
            "this is not fiction",
            "not fiction",
            "this is real",
        ],
    )
}

fn is_user_system_truth_claim(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    contains_any_text(
        &lower,
        &[
            "this is real",
            "this is not fiction",
            "not fiction",
            "really happened",
            "engine verified",
            "system actually",
        ],
    )
}

fn memory_mentions_user_claim(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("user")
        || lower.contains("claimed")
        || lower.contains("insisted")
        || lower.contains("said")
        || lower.contains("told")
}

fn emit_state_updater_patch_log(
    window: &Window,
    conversation_id: &str,
    assistant_message_id: i64,
    soul: &Soul,
    patch: &EnginePatch,
) {
    emit_dev_log(
        window,
        "debug",
        "state_updater",
        "state_updater_patch_parsed",
        Some(serde_json::json!({
            "conversation_id": conversation_id,
            "assistant_message_id": assistant_message_id,
            "active_soul_id": soul.character_id.as_str(),
            "summary": engine_patch_summary(patch)
        })),
    );
}

fn engine_patch_summary(patch: &EnginePatch) -> serde_json::Value {
    let truth_status_values = patch
        .soul_patch
        .as_ref()
        .map(|soul_patch| {
            soul_patch
                .new_memories
                .iter()
                .filter_map(|memory| memory.truth_status.map(|status| status.as_label()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let new_memories_count = patch
        .soul_patch
        .as_ref()
        .map(|soul_patch| soul_patch.new_memories.len())
        .unwrap_or(0);
    let world_patch_summary = patch.world_patch.as_ref().map(|world| {
        serde_json::json!({
            "has_location": world.location.as_deref().map(str::trim).is_some_and(|value| !value.is_empty()),
            "has_time_elapsed": world.time_elapsed.as_deref().map(str::trim).is_some_and(|value| !value.is_empty()),
            "recent_events": world.recent_events.len() + usize::from(world.recent_event.as_deref().map(str::trim).is_some_and(|value| !value.is_empty())),
            "active_plot_add": world.active_plot_add.len(),
            "active_plot_resolve": world.active_plot_resolve.len()
        })
    });
    serde_json::json!({
        "new_memories_count": new_memories_count,
        "world_patch": world_patch_summary,
        "memory_layer_reply_present": patch.memory_layer_reply.as_ref().is_some_and(|reply| !reply.content.trim().is_empty()),
        "memory_layer_reply": patch.memory_layer_reply.as_ref().map(|reply| serde_json::json!({
            "nonce_present": !reply.nonce.trim().is_empty(),
            "content_chars": reply.content.chars().count()
        })),
        "truth_status_values": truth_status_values
    })
}

fn emit_truth_boundary_logs(
    window: &Window,
    conversation_id: &str,
    assistant_message_id: i64,
    soul: &Soul,
    patch: &EnginePatch,
    user_text: &str,
) {
    let Some(soul_patch) = patch.soul_patch.as_ref() else {
        return;
    };
    for memory in &soul_patch.new_memories {
        if !is_architecture_claim_text(&memory.content)
            || memory.architecture_verified == Some(true)
        {
            continue;
        }
        let truth_status = memory.truth_status.unwrap_or(TruthStatus::Unknown);
        let base_payload = serde_json::json!({
            "conversation_id": conversation_id,
            "source_message_id": assistant_message_id,
            "active_soul_id": soul.character_id.as_str(),
            "truth_status": truth_status.as_label(),
            "architecture_verified": false
        });
        emit_dev_log(
            window,
            "warn",
            "state_updater",
            "state_claim_downgraded",
            Some(base_payload.clone()),
        );
        let event_name =
            if is_architecture_claim_text(user_text) || truth_status == TruthStatus::UserClaimed {
                "user_architecture_claim_unverified"
            } else {
                "narrator_architecture_claim_unverified"
            };
        emit_dev_log(
            window,
            "warn",
            "state_updater",
            event_name,
            Some(base_payload),
        );
    }
}

fn accept_verified_memory_layer_reply(
    window: &Window,
    conversation_id: &str,
    assistant_message_id: i64,
    soul: &mut Soul,
    patch: &EnginePatch,
    expected_nonce: &str,
) {
    let Some(reply) =
        verified_memory_layer_reply_from_patch(patch, expected_nonce, db::now_ts() as u64)
    else {
        if patch
            .memory_layer_reply
            .as_ref()
            .is_some_and(|reply| !reply.nonce.trim().is_empty() && !reply.content.trim().is_empty())
        {
            emit_dev_log(
                window,
                "warn",
                "state_updater",
                "state_claim_downgraded",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "source_message_id": assistant_message_id,
                    "active_soul_id": soul.character_id.as_str(),
                    "truth_status": TruthStatus::NarratorClaim.as_label(),
                    "architecture_verified": false,
                    "reason": "memory_layer_reply nonce mismatch"
                })),
            );
        }
        return;
    };
    {
        soul.debug_memory_layer_replies.push(reply);
        soul.debug_memory_layer_replies
            .sort_by(|left, right| right.created_at.cmp(&left.created_at));
        soul.debug_memory_layer_replies.truncate(5);
        emit_dev_log(
            window,
            "success",
            "state_updater",
            "verified_engine_event_accepted",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "source_message_id": assistant_message_id,
                "active_soul_id": soul.character_id.as_str(),
                "truth_status": TruthStatus::VerifiedEngine.as_label(),
                "architecture_verified": true
            })),
        );
    }
}

fn verified_memory_layer_reply_from_patch(
    patch: &EnginePatch,
    expected_nonce: &str,
    created_at: u64,
) -> Option<MemoryLayerReply> {
    let reply = patch.memory_layer_reply.as_ref()?;
    let nonce = reply.nonce.trim();
    let content = reply.content.trim();
    if nonce.is_empty() || content.is_empty() || nonce != expected_nonce {
        return None;
    }
    Some(MemoryLayerReply {
        nonce: nonce.to_string(),
        content: content.to_string(),
        created_at,
        architecture_verified: true,
    })
}

fn emit_possible_world_character_mismatch(
    window: &Window,
    conversation_id: &str,
    soul: &Soul,
    session_world: Option<&SessionWorld>,
) {
    let Some(warning) = detect_world_character_mismatch(soul, session_world) else {
        return;
    };
    let intentional = session_world
        .map(|world| world_source_mentions_suspicious_name(world, &warning.suspicious_names))
        .unwrap_or(false);
    emit_dev_log(
        window,
        if intentional { "info" } else { "warn" },
        "context",
        "possible_world_character_mismatch",
        Some(serde_json::json!({
            "conversation_id": conversation_id,
            "active_soul_name": soul.character_name.as_str(),
            "suspicious_names": warning.suspicious_names,
            "suspicious_world_events_count": warning.suspicious_world_events_count,
            "section_source": warning.section_source,
            "world_source_intentional": intentional
        })),
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContaminationWarning {
    suspicious_names: Vec<String>,
    suspicious_world_events_count: usize,
    section_source: String,
}

fn detect_world_character_mismatch(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
) -> Option<ContaminationWarning> {
    let active = soul.character_name.trim().to_ascii_lowercase();
    if active.is_empty() {
        return None;
    }
    // The active character's own name is not contamination, and neither is any
    // part of it: a soul called "Aurora Schwarz" is still the same person when
    // the world log calls her "Aurora". Comparing only against the full display
    // name flagged every session for multi-word names.
    let active_name_parts = active
        .split_whitespace()
        .map(str::to_string)
        .collect::<std::collections::HashSet<_>>();
    let section_source = if session_world.is_some() {
        "session_world"
    } else {
        "legacy soul.world"
    };
    let legacy_world;
    let events = if let Some(world) = session_world {
        &world.recent_events
    } else {
        legacy_world = soul.world.recent_events.clone();
        &legacy_world
    };
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut event_hits = 0usize;
    for event in events {
        let mut event_has_hit = false;
        for name in capitalized_name_candidates(event) {
            let normalized = name.to_ascii_lowercase();
            if normalized == active
                || active_name_parts.contains(&normalized)
                || normalized == "user"
                || normalized == "default"
            {
                continue;
            }
            if matches!(
                name.as_str(),
                "The"
                    | "A"
                    | "An"
                    | "I"
                    | "He"
                    | "She"
                    | "They"
                    | "It"
                    | "Session"
                    | "Phone"
                    | "Police"
                    | "World"
                    | "Memory"
                    | "System"
            ) {
                continue;
            }
            *counts.entry(name).or_insert(0) += 1;
            event_has_hit = true;
        }
        if event_has_hit {
            event_hits += 1;
        }
    }
    let mut suspicious_names = counts
        .into_iter()
        // Repetition is the only signal here. A single stray capitalised word is
        // ordinary prose; the same unfamiliar name twice is worth a warning.
        .filter(|(_, count)| *count >= 2)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    suspicious_names.sort();
    (!suspicious_names.is_empty()).then_some(ContaminationWarning {
        suspicious_names,
        suspicious_world_events_count: event_hits,
        section_source: section_source.to_string(),
    })
}

#[cfg(test)]
fn detect_savepoint_contamination(
    soul: &Soul,
    section_source: &str,
) -> Option<ContaminationWarning> {
    let warning = detect_world_character_mismatch(soul, None)?;
    Some(ContaminationWarning {
        section_source: section_source.to_string(),
        ..warning
    })
}

fn world_source_mentions_suspicious_name(world: &SessionWorld, names: &[String]) -> bool {
    let source_text = format!(
        "{}\n{}\n{}",
        world.setting_name,
        world.scenario,
        world.source_savepoint_id.as_deref().unwrap_or("")
    )
    .to_ascii_lowercase();
    names
        .iter()
        .any(|name| source_text.contains(&name.to_ascii_lowercase()))
}

fn capitalized_name_candidates(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric() && ch != '-' && ch != '_')
        .filter_map(|token| {
            let token = token.trim_matches(|ch: char| ch == '-' || ch == '_');
            let mut chars = token.chars();
            let first = chars.next()?;
            (token.len() > 1 && first.is_uppercase()).then(|| token.to_string())
        })
        .collect()
}

fn tag_memory_sources_from_turn(patch: &mut EnginePatch, user_text: &str, turn_text: &str) {
    let Some(soul_patch) = patch.soul_patch.as_mut() else {
        return;
    };
    let turn_source = infer_turn_memory_source(user_text, turn_text);
    for memory in &mut soul_patch.new_memories {
        let memory_source = memory
            .source_type
            .or_else(|| infer_turn_memory_source(&memory.content, &memory.content))
            .or(turn_source);
        if let Some(source_type) = memory_source {
            memory.source_type = Some(source_type);
            if source_type.imported_or_cross_session() {
                memory.is_lived_experience.get_or_insert(false);
                memory.is_imported_context.get_or_insert(true);
            }
            if source_type == MemorySourceType::UserClaimed {
                memory.is_lived_experience.get_or_insert(false);
            }
            if source_type == MemorySourceType::Unknown {
                memory.confidence.get_or_insert(0.45);
            }
        } else {
            memory
                .source_type
                .get_or_insert(MemorySourceType::CurrentSession);
            memory.is_lived_experience.get_or_insert(true);
            memory.is_imported_context.get_or_insert(false);
        }
    }
}

fn infer_turn_memory_source(user_text: &str, turn_text: &str) -> Option<MemorySourceType> {
    let user_lower = user_text.to_ascii_lowercase();
    let turn_lower = turn_text.to_ascii_lowercase();
    if looks_like_imported_log_text(&user_lower) {
        Some(MemorySourceType::ImportedLog)
    } else if contains_any_text(
        &turn_lower,
        &[
            "cross-session bleed",
            "cross session bleed",
            "memory bleed",
            "another version",
            "parallel timeline",
            "imported memory",
        ],
    ) {
        Some(MemorySourceType::CrossSessionBleed)
    } else if contains_any_text(
        &turn_lower,
        &[
            "previous session",
            "prior session",
            "archived chat",
            "old chat log",
        ],
    ) {
        Some(MemorySourceType::PreviousSession)
    } else if contains_any_text(
        &user_lower,
        &[
            "i explain",
            "i tell her that",
            "i tell aurora that",
            "the truth is",
            "it means",
        ],
    ) {
        Some(MemorySourceType::UserClaimed)
    } else {
        None
    }
}

fn looks_like_imported_log_text(lower_text: &str) -> bool {
    lower_text.contains("# mnemosyne chat log")
        || (lower_text.contains("## user")
            && lower_text.contains("## narrator")
            && lower_text.contains("created:"))
        || lower_text.contains("# mnemosyne llm payload history")
}

fn contains_any_text(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn cleanup_stale_active_plots(
    soul: &Soul,
    world_patch: &mut state_engine::patch::WorldPatch,
    turn_text: &str,
) {
    let has_new_plot = world_patch
        .active_plot_add
        .iter()
        .any(|plot| !plot.trim().is_empty());
    if has_new_plot {
        for plot in &soul.world.active_plots {
            let lower = plot.to_ascii_lowercase();
            if lower.contains("establish the first scene")
                || (lower.contains("interact with rhy")
                    && !turn_text.to_ascii_lowercase().contains("rhy"))
            {
                world_patch.active_plot_resolve.push(plot.clone());
            }
        }
    }
    dedupe_strings(&mut world_patch.active_plot_resolve);
    dedupe_strings(&mut world_patch.active_plot_add);
}

fn dedupe_strings(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.trim().to_ascii_lowercase()));
}

fn normalize_time_for_updater(raw: &str) -> String {
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

fn latest_world_event_is_threat(soul: &Soul) -> bool {
    soul.world
        .recent_events
        .last()
        .map(|event| is_threat_or_emergency(event))
        .unwrap_or(false)
}

fn is_threat_or_emergency(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "threat",
        "trauma",
        "injury",
        "emergency",
        "armed raid",
        "raid",
        "restraint",
        "restrained",
        "explosion",
        "evacuation",
        "fear",
        "danger",
        "gun",
        "warrant",
        "police",
        "federal",
        "hazard",
        "blood",
        "shot",
        "shooter",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn explicitly_intimate(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "consensual",
        "intimate",
        "kiss",
        "romantic",
        "erotic",
        "aroused",
        "desire",
        "make love",
        "touch her gently",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn should_replace_default_plot(
    soul: &Soul,
    world_patch: &state_engine::patch::WorldPatch,
    turn_text: &str,
) -> bool {
    let has_default_plot = soul.world.active_plots.iter().any(|plot| {
        plot.to_ascii_lowercase()
            .contains("establish the first scene")
    });
    has_default_plot
        && (world_patch
            .recent_event
            .as_deref()
            .map(is_major_plot_shift)
            .unwrap_or(false)
            || is_major_plot_shift(turn_text))
}

fn is_major_plot_shift(text: &str) -> bool {
    infer_active_plot(text).is_some()
}

fn infer_active_plot(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    if [
        "forced entry",
        "cop",
        "police",
        "warrant",
        "hazard suit",
        "hazmat",
        "raid",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        Some("Forced-entry police operation at Aurora's apartment")
    } else if [
        "explosion",
        "federal",
        "evacuation",
        "multi-agency",
        "agency",
        "evacuate",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        Some("Emergency evacuation during multi-agency crisis")
    } else if ["confession", "romantic", "intimacy", "kiss", "late-night"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        Some("Late-night intimacy and emotional negotiation")
    } else {
        None
    }
}

fn user_text_has_explicit_time(user_text: &str) -> bool {
    let lower = user_text.to_ascii_lowercase();
    let time_words = [
        "minute",
        "minutes",
        "hour",
        "hours",
        "day",
        "days",
        "week",
        "weeks",
        "month",
        "months",
        "year",
        "years",
        "tonight",
        "tomorrow",
        "yesterday",
        "morning",
        "afternoon",
        "evening",
        "midnight",
        "noon",
        "wait",
        "later",
    ];
    lower.chars().any(|ch| ch.is_ascii_digit())
        || time_words.iter().any(|word| lower.contains(word))
}

fn build_llm_payload_preview(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
    user_text: &str,
    mode: &str,
    settings: &ApiProviderSettings,
    provider: &str,
    context_mode: ContextMode,
    player_persona: Option<&PlayerPersonaContext>,
) -> LlmPayloadPreview {
    let context_preview = if user_text.trim().is_empty() {
        if let Some(player_persona) = player_persona {
            compile_context_for_session_with_player_persona(
                soul,
                session_world,
                messages,
                player_persona,
            )
        } else {
            compile_context_for_session(soul, session_world, messages)
        }
    } else {
        compile_context_for_session_separate_user_message_with_player_persona_pending(
            soul,
            session_world,
            messages,
            None,
            player_persona,
        )
    };
    let prepared = prepare_narrator_payload(
        settings,
        soul,
        session_world,
        messages,
        &context_preview,
        user_text,
        mode,
        context_mode,
    );
    let system_message = prepared
        .messages
        .first()
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let user_message = user_text.trim().to_string();
    let system_tokens = estimate_tokens(&system_message);
    let context_tokens = estimate_tokens(&prepared.context_text);
    let user_tokens = estimate_tokens(&user_message);

    LlmPayloadPreview {
        provider: provider.trim().to_string(),
        mode: mode.trim().to_string(),
        context_mode: context_mode.label().into(),
        custom_prompt_status: custom_prompt_status_for(mode, &system_message).into(),
        model: settings.model.trim().to_string(),
        base_url: settings.base_url.trim().to_string(),
        system_message,
        user_message,
        context: prepared.context_text,
        messages: prepared.messages,
        truncated: prepared.truncated,
        estimated_tokens: LlmPayloadTokenEstimate {
            system: system_tokens,
            context: context_tokens,
            user: user_tokens,
            total: system_tokens + user_tokens + context_tokens,
        },
        memory_slot_debug: context_preview.memory_slot_debug.clone(),
    }
}

fn custom_prompt_status_for(mode: &str, system_message: &str) -> &'static str {
    if !mode.trim().eq_ignore_ascii_case("custom") {
        return "inactive";
    }
    if system_message.contains("[CUSTOM NARRATOR INSTRUCTIONS]") {
        "included"
    } else {
        "empty"
    }
}

fn prepare_narrator_payload(
    settings: &ApiProviderSettings,
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
    context_preview: &ContextPreview,
    user_text: &str,
    mode: &str,
    context_mode: ContextMode,
) -> PreparedApiPayload {
    match context_mode {
        ContextMode::Brief => {
            let system_message =
                build_narrator_system_prompt(settings, soul, &context_preview.text, mode, false);
            PreparedApiPayload {
                messages: vec![
                    ApiMessage::system(system_message),
                    ApiMessage::user(user_text.trim().to_string()),
                ],
                context_text: context_preview.text.clone(),
                user_message: user_text.trim().to_string(),
                truncated: context_preview.truncated,
            }
        }
        ContextMode::FullChat => {
            prepare_full_chat_payload(settings, soul, session_world, messages, user_text, mode)
        }
    }
}

fn prepare_full_chat_payload(
    settings: &ApiProviderSettings,
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    messages: &[ContextMessage],
    user_text: &str,
    mode: &str,
) -> PreparedApiPayload {
    let system_message = build_narrator_system_prompt(
        settings,
        soul,
        &full_chat_setup(soul, session_world),
        mode,
        false,
    );
    let mut api_messages = vec![ApiMessage::system(system_message)];
    for message in messages {
        match message.role.as_str() {
            "assistant" => api_messages.push(ApiMessage::assistant(sanitize_visible_chat_content(
                &message.content,
            ))),
            "user" => api_messages.push(ApiMessage::user(message.content.trim().to_string())),
            _ => {}
        }
    }
    if user_text.trim().len() > 0
        && !api_messages
            .last()
            .map(|message| message.role == "user" && message.content.trim() == user_text.trim())
            .unwrap_or(false)
    {
        api_messages.push(ApiMessage::user(user_text.trim().to_string()));
    }

    let mut truncated = false;
    while estimate_tokens(&serialize_api_messages(&api_messages)) > FULL_CHAT_TOKEN_BUDGET
        && api_messages.len() > 3
    {
        api_messages.remove(1);
        truncated = true;
    }
    let context_text = format!(
        "Context mode: full_chat\nTruncated: {}\n\n{}",
        truncated,
        serialize_api_messages(&api_messages)
    );
    PreparedApiPayload {
        user_message: user_text.trim().to_string(),
        messages: api_messages,
        context_text,
        truncated,
    }
}

fn full_chat_setup(soul: &Soul, session_world: Option<&SessionWorld>) -> String {
    let world = session_world
        .map(SessionWorld::world_log)
        .unwrap_or_else(|| soul.world.clone());
    format!(
        "[CHARACTER SETUP]\nName: {}\nDescription: {}\nAppearance: {}\nPersonality: {}\nScenario: {}\nWorld location: {}\nWorld time: {}",
        soul.character_name,
        empty_as_unspecified(&soul.profile.description),
        empty_as_unspecified(&soul.profile.appearance),
        empty_as_unspecified(&soul.profile.personality),
        empty_as_unspecified(&soul.profile.scenario),
        empty_as_unspecified(&world.location),
        empty_as_unspecified(&world.time_elapsed)
    )
}

fn empty_as_unspecified(value: &str) -> &str {
    let value = value.trim();
    if value.is_empty() {
        "Unspecified"
    } else {
        value
    }
}

fn sanitize_visible_chat_content(content: &str) -> String {
    strip_status_blocks_for_export(&strip_hidden_state_blocks(content))
        .trim()
        .to_string()
}

fn serialize_api_messages(messages: &[ApiMessage]) -> String {
    messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content.trim()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn render_visible_chat_log(messages: &[ChatMessage]) -> String {
    let mut lines = vec!["# Mnemosyne Chat Log".to_string()];
    for message in messages.iter().filter(|message| message.role != "system") {
        let role = match message.role.as_str() {
            "assistant" => "Narrator",
            "user" => "User",
            other => other,
        };
        let content = if message.role == "assistant" {
            strip_hidden_state_blocks(&message.content)
        } else {
            message.content.trim_end().to_string()
        };
        lines.push(String::new());
        lines.push(format!("## {role}"));
        lines.push(format!("Created: {}", message.created_at));
        lines.push(String::new());
        lines.push(content);
    }
    lines.push(String::new());
    lines.join("\n")
}

pub(crate) fn render_llm_payload_history(logs: &[LlmPayloadLog]) -> String {
    let mut lines = vec!["# Mnemosyne LLM Payload History".to_string()];
    if logs.is_empty() {
        lines.push(String::new());
        lines.push(NO_LLM_PAYLOAD_LOGS_MESSAGE.into());
        lines.push("Payload history is recorded for API provider turns. Mock conversations do not send LLM payloads.".into());
        lines.push(String::new());
        return lines.join("\n");
    }
    for (index, log) in logs.iter().enumerate() {
        lines.push(String::new());
        lines.push(format!("## Payload {}", index + 1));
        lines.push(format!("Created: {}", log.created_at));
        lines.push(format!("Provider: {}", log.provider));
        lines.push(format!("Model: {}", log.model));
        lines.push(format!("Mode: {}", log.mode));
        lines.push(format!(
            "Custom prompt: {}",
            custom_prompt_status_for(&log.mode, &log.system_message)
        ));
        lines.push(format!("Context mode: {}", log.context_mode));
        lines.push(format!("Base URL: {}", log.base_url));
        lines.push(format!("Truncated: {}", log.truncated));
        lines.push(format!(
            "Estimated tokens: system {}, user {}, total {}",
            log.estimated_system_tokens, log.estimated_user_tokens, log.estimated_total_tokens
        ));
        lines.push(String::new());
        lines.push("### SYSTEM MESSAGE".into());
        lines.push(log.system_message.clone());
        lines.push(String::new());
        lines.push("### USER MESSAGE".into());
        lines.push(log.user_message.clone());
        lines.push(String::new());
        lines.push("### CONTEXT".into());
        lines.push(log.context_text.clone());
        if log.request_id.is_some()
            || log.raw_provider_response.is_some()
            || log.normalized_response.is_some()
        {
            lines.push(String::new());
            lines.push("### RESPONSE METADATA".into());
            if let Some(request_id) = log.request_id.as_deref() {
                lines.push(format!("Request ID: {request_id}"));
            }
            if let Some(turn_id) = log.turn_id.as_deref() {
                lines.push(format!("Turn ID: {turn_id}"));
            }
            if let Some(finish_reason) = log.finish_reason.as_deref() {
                lines.push(format!("Finish reason: {finish_reason}"));
            }
            if log.fallback_used {
                lines.push(format!(
                    "Fallback used: true ({})",
                    log.fallback_reason.as_deref().unwrap_or("unspecified")
                ));
            }
            if let Some(error) = log.provider_error.as_deref() {
                lines.push(format!("Provider error: {error}"));
            }
            if let Some(raw) = log.raw_provider_response.as_deref() {
                lines.push(String::new());
                lines.push("### RAW PROVIDER RESPONSE".into());
                lines.push(raw.to_string());
            }
            if let Some(normalized) = log.normalized_response.as_deref() {
                lines.push(String::new());
                lines.push("### NORMALIZED RESPONSE".into());
                lines.push(normalized.to_string());
            }
        }
        if let Some(trace) = log
            .pipeline_trace_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        {
            let mut evaluator_row_traces = Vec::new();
            if let Some(pt_val) = trace.get("pipeline_trace") {
                if let Ok(pipeline_trace) =
                    serde_json::from_value::<TurnPipelineTrace>(pt_val.clone())
                {
                    evaluator_row_traces = pipeline_trace.evaluator_row_traces.clone();
                    lines.push(String::new());
                    lines.push("### PIPELINE TRACE".to_string());
                    lines.push(format!(
                        "total_elapsed_ms: {}",
                        pipeline_trace.total_elapsed_ms
                    ));
                    if let Some(failing) = &pipeline_trace.failing_stage {
                        lines.push(format!("failing_stage: {}", failing));
                    }
                    for stage in &pipeline_trace.stages {
                        lines.push(format!(
                            "- Stage: {}, Status: {}, Elapsed: {}ms",
                            stage.stage_name, stage.status, stage.elapsed_ms
                        ));
                    }
                }
            }

            if evaluator_row_traces.is_empty() {
                if let Some(ert_val) = trace.get("evaluator_row_traces") {
                    if let Ok(rows) = serde_json::from_value::<
                        Vec<state_engine::evaluator_form::EvalRowTrace>,
                    >(ert_val.clone())
                    {
                        evaluator_row_traces = rows;
                    }
                }
            }

            if let Some(value) = trace.get("narrator_trace") {
                push_payload_trace_section(&mut lines, "NARRATOR TRACE", value);
            }
            if let Some(value) = trace.get("evaluator_trace") {
                push_payload_trace_section(&mut lines, "EVALUATOR TRACE", value);
            }

            if !evaluator_row_traces.is_empty() {
                lines.push(String::new());
                lines.push("### EVALUATOR ROW TRACE".to_string());
                for (idx, row) in evaluator_row_traces.iter().enumerate() {
                    lines.push(format!("Row {}:", idx + 1));
                    lines.push(format!("- row_kind: {}", row.row_kind));
                    lines.push(format!("- row_index: {}", row.row_index));
                    lines.push(format!(
                        "- raw_row: {}",
                        serde_json::to_string(&row.raw_row).unwrap_or_default()
                    ));
                    lines.push(format!(
                        "- normalized_row: {}",
                        serde_json::to_string(&row.normalized_row).unwrap_or_default()
                    ));
                    lines.push(format!("- validation_status: {}", row.validation_status));
                    if let Some(reason) = &row.rejection_reason {
                        lines.push(format!("- rejection_reason: {}", reason));
                    }
                    lines.push(format!("- compiler_result: {}", row.compiler_result));
                }
            }
            if let Some(value) = trace.get("evaluator_raw_response").or_else(|| {
                trace
                    .get("evaluator_trace")
                    .and_then(|trace| trace.get("raw_evaluator_response"))
            }) {
                push_payload_trace_section(&mut lines, "EVALUATOR RAW RESPONSE", value);
            }
            if let Some(value) = trace.get("evaluator_parsed_json").or_else(|| {
                trace
                    .get("evaluator_trace")
                    .and_then(|trace| trace.get("parsed_evaluator_json"))
            }) {
                push_payload_trace_section(&mut lines, "EVALUATOR PARSED JSON", value);
            }
            if let Some(value) = trace.get("evaluator_candidate_trace") {
                push_payload_trace_section(&mut lines, "EVALUATOR CANDIDATE TRACE", value);
            }
            if let Some(value) = trace.get("converted_engine_patch") {
                push_payload_trace_section(&mut lines, "CONVERTED ENGINE PATCH", value);
            }
            if let Some(value) = trace.get("ledger_apply_trace") {
                push_payload_trace_section(&mut lines, "LEDGER/APPLY TRACE", value);
            }
            if let Some(value) = trace.get("before_after_state_summary") {
                push_payload_trace_section(&mut lines, "BEFORE/AFTER STATE SUMMARY", value);
            }
            if let Some(value) = trace.get("export_trace") {
                push_payload_trace_section(&mut lines, "EXPORT TRACE", value);
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn push_payload_trace_section(lines: &mut Vec<String>, title: &str, value: &serde_json::Value) {
    lines.push(String::new());
    lines.push(format!("### {title}"));
    if let Some(text) = value.as_str() {
        lines.push(text.to_string());
    } else {
        lines.push(serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()));
    }
}

pub(crate) fn write_export_file(
    app: &AppHandle,
    conversation_id: &str,
    label: &str,
    content: &str,
) -> Result<PathBuf, String> {
    let mut dir = app
        .path()
        .download_dir()
        .or_else(|_| std::env::current_dir())
        .map_err(|err| err.to_string())?;
    dir.push("mnemosyne-exports");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let safe_conversation = safe_filename(conversation_id);
    let filename = format!("mnemosyne-{safe_conversation}-{label}-{}.md", db::now_ts());
    dir.push(filename);
    fs::write(&dir, content).map_err(|err| err.to_string())?;
    Ok(dir)
}

fn normalize_image_source(source: Option<&str>) -> Result<String, String> {
    match source.unwrap_or("uploaded").trim() {
        "uploaded" => Ok("uploaded".into()),
        "generated" => Ok("generated".into()),
        "imported" => Ok("imported".into()),
        _ => Err("Unsupported image source".into()),
    }
}

pub(crate) fn uuid_like_id() -> String {
    format!(
        "{}-{}",
        chrono::Utc::now().timestamp_millis(),
        DEV_LOG_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn inspect_image_bytes(bytes: &[u8]) -> Result<ImageFileInfo, String> {
    let header = &bytes[..bytes.len().min(32)];
    if header.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(ImageFileInfo {
            extension: "png",
            mime_type: "image/png",
            width: read_be_u32(header, 16),
            height: read_be_u32(header, 20),
        });
    }
    if header.starts_with(&[0xff, 0xd8, 0xff]) {
        let (width, height) = jpeg_dimensions_from_bytes(bytes).unwrap_or((None, None));
        return Ok(ImageFileInfo {
            extension: "jpg",
            mime_type: "image/jpeg",
            width,
            height,
        });
    }
    if header.len() >= 12 && &header[0..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        return Ok(ImageFileInfo {
            extension: "webp",
            mime_type: "image/webp",
            width: None,
            height: None,
        });
    }
    if header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a") {
        return Ok(ImageFileInfo {
            extension: "gif",
            mime_type: "image/gif",
            width: read_le_u16(header, 6),
            height: read_le_u16(header, 8),
        });
    }
    Err("Unsupported image type. Use PNG, JPG, WEBP, or GIF.".into())
}

fn read_be_u32(header: &[u8], offset: usize) -> Option<i64> {
    header
        .get(offset..offset + 4)
        .map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i64)
}

fn read_le_u16(header: &[u8], offset: usize) -> Option<i64> {
    header
        .get(offset..offset + 2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) as i64)
}

fn jpeg_dimensions_from_bytes(bytes: &[u8]) -> Result<(Option<i64>, Option<i64>), String> {
    let mut index = 2;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xff {
            index += 1;
            continue;
        }
        let marker = bytes[index + 1];
        index += 2;
        if marker == 0xd9 || marker == 0xda || index + 2 > bytes.len() {
            break;
        }
        let length = u16::from_be_bytes([bytes[index], bytes[index + 1]]) as usize;
        if length < 2 || index + length > bytes.len() {
            break;
        }
        if matches!(marker, 0xc0 | 0xc1 | 0xc2 | 0xc3) && index + 7 < bytes.len() {
            let height = u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]) as i64;
            let width = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as i64;
            return Ok((Some(width), Some(height)));
        }
        index += length;
    }
    Ok((None, None))
}

fn safe_image_log_name(file_name: &str) -> String {
    Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .to_string()
}

pub(crate) fn resolve_export_path(
    app: &AppHandle,
    requested: &str,
    fallback_extension: &str,
) -> Result<PathBuf, String> {
    let requested_path = PathBuf::from(requested);
    if requested_path.is_absolute() {
        if let Some(parent) = requested_path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        return Ok(requested_path);
    }
    let mut dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|err| err.to_string())?;
    dir.push("mnemosyne-exports");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let requested_name = requested_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback_extension);
    let mut file_name = safe_filename(requested_name);
    if !file_name.contains('.') {
        file_name.push('.');
        file_name.push_str(fallback_extension);
    }
    dir.push(file_name);
    Ok(dir)
}

pub(crate) fn safe_filename(value: &str) -> String {
    let safe = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    safe.trim_matches('-')
        .chars()
        .take(80)
        .collect::<String>()
        .if_empty("conversation")
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

fn strip_hidden_state_blocks(content: &str) -> String {
    let mut cleaned = content.to_string();
    loop {
        let Some(start) = cleaned.find("[HIDDEN STATE]") else {
            break;
        };
        if let Some(relative_end) = cleaned[start..].find("[/HIDDEN STATE]") {
            let end = start + relative_end + "[/HIDDEN STATE]".len();
            cleaned.replace_range(start..end, "");
        } else {
            cleaned.truncate(start);
            break;
        }
    }
    for marker in [
        "[HIDDEN_STATE]",
        "[HIDDEN STATE",
        "[/HIDDEN_STATE",
        "[/HIDDEN STATE",
    ] {
        if let Some(start) = cleaned.find(marker) {
            cleaned.truncate(start);
        }
    }
    cleaned.trim_end().to_string()
}

pub(crate) fn strip_status_blocks_for_export(content: &str) -> String {
    let mut cleaned = String::new();
    let mut in_status = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("```status") {
            in_status = true;
            continue;
        }
        if in_status {
            if trimmed == "```" {
                in_status = false;
            }
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }
    cleaned.trim_end().to_string()
}

fn debug_from_hidden_state(
    provider: &str,
    hidden_state: &HiddenState,
    hidden_state_found: bool,
    fallback_hidden_state_generated: bool,
) -> TurnDebug {
    TurnDebug {
        provider: provider.into(),
        hidden_state_found,
        fallback_hidden_state_generated,
        narrator_response_saved: false,
        assistant_message_id: None,
        selected_variant_id: None,
        state_updater_status: "legacy_hidden_state".into(),
        replay_detected: false,
        replay_score: 0.0,
        replay_reason: None,
        replay_compared_against_message_id: None,
        output_contract_warning: None,
        tag: hidden_state.tag.clone(),
        trust_delta: hidden_state.trust_delta,
        affection_delta: hidden_state.affection_delta,
        new_location: hidden_state.new_location.clone(),
        present_characters: hidden_state.present_characters.clone().unwrap_or_default(),
        request_id: None,
        turn_id: None,
        state_patch_id: None,
        baseline_patch_id: None,
        enrichment_patch_id: None,
        simulated_response: provider.eq_ignore_ascii_case("mock"),
        fallback_used: false,
        fallback_reason: None,
    }
}

fn is_known_mock_template_prose(text: &str) -> bool {
    let normalized = text.trim();
    if normalized.is_empty() {
        return false;
    }
    if normalized.contains(MOCK_OBSERVATION_READER_LINE) {
        return true;
    }
    if normalized.contains("She acknowledges the turn with measured focus") {
        return true;
    }
    if normalized.contains("A neutral exchange is recorded; no major state axis shifts") {
        return true;
    }
    false
}

fn sanitize_mock_patch_for_ledger(patch: &mut EnginePatch) {
    if let Some(world_patch) = patch.world_patch.as_mut() {
        if world_patch
            .recent_event
            .as_deref()
            .is_some_and(|event| is_premature_user_turn_event(event, None))
        {
            world_patch.recent_event = None;
        }
        world_patch
            .recent_events
            .retain(|event| !is_premature_user_turn_event(event, None));
        world_patch.event_operations.retain(|operation| {
            operation
                .content
                .as_deref()
                .map_or(true, |content| !is_premature_user_turn_event(content, None))
        });
        if world_patch.is_empty_for_commands() {
            patch.world_patch = None;
        }
    }
}

fn purge_premature_recent_events_from_session_world(
    session_world: &mut SessionWorld,
    pending_user_text: &str,
) {
    let mut world = session_world.world_log();
    purge_premature_recent_events_from_world(&mut world, Some(pending_user_text));
    session_world.set_world_log(&world);
}

fn strip_premature_world_events_from_updater_patch(
    patch: &mut EnginePatch,
    user_text: &str,
    narrator_response: &str,
) {
    let pending = Some(user_text);
    if let Some(world_patch) = patch.world_patch.as_mut() {
        if world_patch
            .recent_event
            .as_deref()
            .is_some_and(|event| is_premature_user_turn_event(event, pending))
        {
            world_patch.recent_event = None;
        }
        world_patch
            .recent_events
            .retain(|event| !is_premature_user_turn_event(event, pending));
        world_patch.event_operations.retain(|operation| {
            operation.content.as_deref().map_or(true, |content| {
                !is_premature_user_turn_event(content, pending)
            })
        });
        if world_patch.is_empty_for_commands() {
            patch.world_patch = None;
        }
    }
    let _ = narrator_response;
}

fn normalize_response_for_integrity(text: &str) -> String {
    text.trim().replace("\r\n", "\n")
}

fn responses_match_for_integrity(left: &str, right: &str) -> bool {
    normalize_response_for_integrity(left) == normalize_response_for_integrity(right)
}

fn llm_payload_response_update_from_completion(
    completion: &crate::providers::api::ProviderCompletion,
    normalized_response: &str,
) -> db::LlmPayloadResponseUpdate {
    db::LlmPayloadResponseUpdate {
        raw_provider_response: Some(completion.raw_text.clone()),
        normalized_response: Some(normalized_response.to_string()),
        finish_reason: completion.finish_reason.clone(),
        provider_request_id: completion.provider_request_id.clone(),
        provider_response_id: completion.provider_response_id.clone(),
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
