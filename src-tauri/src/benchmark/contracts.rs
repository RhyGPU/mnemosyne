use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkType {
    #[serde(alias = "self_play")]
    VisibleAiChat,
    #[serde(alias = "scripted_replay")]
    ScriptedVisibleReplay,
    HeadlessRegression,
    #[serde(alias = "multi_agent_self_play")]
    MultiAgentVisibleChat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkTarget {
    CurrentSession,
    NewBenchmarkSessionFromCurrentSoul,
    NewBenchmarkSessionFromSelectedSoulWorld,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSettings {
    #[serde(alias = "benchmark_type")]
    pub benchmark_type: BenchmarkType,
    #[serde(default = "default_benchmark_target")]
    pub target: BenchmarkTarget,
    #[serde(default)]
    pub current_conversation_id: Option<String>,
    pub turn_count: usize,
    pub narrator_style: String,
    pub evaluator_mode: Option<String>,
    pub structured_evaluator_transport: Option<String>,
    pub structured_evaluator_policy: Option<String>,
    pub structured_evaluator_max_retries: Option<u32>,
    pub player_simulator_profile_id: Option<String>,
    /// When set, the player simulator speaks as this Soul instead of the user
    /// persona, so a run exercises character-to-character interaction rather
    /// than character-to-user.
    #[serde(default)]
    pub player_character_soul_id: Option<String>,
    pub player_goal: String,
    #[serde(default = "default_true")]
    pub export_payload_history: bool,
    #[serde(default = "default_true")]
    pub export_mne: bool,
    #[serde(default = "default_true")]
    pub export_summary_json: bool,
    #[serde(default)]
    pub strict_tool_evaluator: bool,
    #[serde(default = "default_true")]
    pub wait_for_evaluator_each_turn: bool,
}

fn default_true() -> bool {
    true
}

fn default_benchmark_target() -> BenchmarkTarget {
    BenchmarkTarget::CurrentSession
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkObjectIdentityCheck {
    pub label: String,
    pub expected_object_id: String,
    pub found: bool,
}

/// Prompt/completion tokens per engine for one benchmark run.
///
/// The whole product claim is that a compact state brief costs less than
/// re-injecting the transcript every turn, so the two sides are measured with
/// the same method and reported side by side. `provider_reported` says whether
/// the numbers came from the provider's usage block or from character estimates;
/// a mixed run reports false so the figures are not over-trusted.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchmarkTokenComparison {
    pub narrator_prompt_tokens: u64,
    pub narrator_completion_tokens: u64,
    pub narrator_calls: usize,
    pub evaluator_prompt_tokens: u64,
    pub evaluator_completion_tokens: u64,
    pub evaluator_calls: usize,
    /// narrator + evaluator: what a Mnemosyne turn actually costs.
    pub mnemosyne_total_tokens: u64,
    pub mnemosyne_turns: usize,
    pub traditional_prompt_tokens: u64,
    pub traditional_completion_tokens: u64,
    pub traditional_total_tokens: u64,
    pub traditional_turns: usize,
    /// Harness cost, excluded from both sides: the simulated player's own calls.
    pub player_simulator_total_tokens: u64,
    pub player_simulator_calls: usize,
    pub provider_reported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkScorecard {
    pub visible_chat_messages_created: bool,
    pub normal_pipeline_used: bool,
    pub visible_turns_requested: usize,
    pub visible_turns_completed: usize,
    pub visible_user_messages_created: usize,
    pub visible_assistant_messages_created: usize,
    pub unique_user_message_ids: usize,
    pub unique_assistant_message_ids: usize,
    pub internal_evaluator_retry_count: usize,
    pub internal_evaluator_retry_payload_count: usize,
    pub duplicate_turn_rows_detected: bool,
    pub duplicate_turn_message_pairs: Vec<String>,
    pub player_simulator_payload_count: usize,
    pub turn_count_requested: usize,
    pub turn_count_completed: usize,
    pub player_simulator_calls: usize,
    pub narrator_calls: usize,
    pub evaluator_calls: usize,
    pub evaluator_waited_each_turn: bool,
    pub memory_updated: bool,
    pub object_state_updated: bool,
    pub relationship_updated: bool,
    pub relationship_target_checked: Option<String>,
    pub relationship_changed_from: Option<serde_json::Value>,
    pub relationship_changed_to: Option<serde_json::Value>,
    pub relationship_delta_patch_ids: Vec<String>,
    pub relationship_delta_sources: Vec<String>,
    pub evaluator_provider_failures: usize,
    pub structured_provider_429_count: usize,
    pub evaluator_response_failed_count: usize,
    pub evaluator_empty_patch_count: usize,
    pub form_rows_rejected_count: usize,
    pub local_repair_invoked_count: usize,
    pub local_reextract_invoked_count: usize,
    pub local_repair_payload_count: usize,
    pub local_repair_response_count: usize,
    pub local_repair_state_patch_count: usize,
    pub payload_history_export_succeeded: bool,
    pub narrator_visible_response_each_turn: bool,
    pub narrator_provider_error: Option<String>,
    pub stop_reason: Option<String>,
    pub failed_stage: Option<String>,
    pub evaluator_used_tool_call_where_required: bool,
    pub no_evaluator_form_v1_fallback_in_strict_mode: bool,
    pub syntactic_repair_unused_in_strict_mode: bool,
    /// Whether strict tool-call evaluation was actually requested for this run.
    /// The three checks above are only meaningful when this is true; the UI shows
    /// them as n/a otherwise instead of a misleading PASS.
    pub strict_tool_evaluator: bool,
    /// The evaluator transport actually used across the run (e.g.
    /// "evaluator_form_v1" / "evaluator_structured_v1"), derived from per-turn
    /// traces — so a form-mode run can't masquerade as a strict tool-call pass.
    pub evaluator_mode_actual: String,
    /// Side-by-side token cost. `None` when no payload rows were recorded.
    #[serde(default)]
    pub token_comparison: Option<BenchmarkTokenComparison>,
    /// When the primary evaluator failed/produced no state on a turn that
    /// warranted it, local repair must have been invoked AND recovered state.
    pub local_repair_recovered_state_when_warranted: bool,
    /// Repair was invoked but the local endpoint never answered (payloads sent,
    /// zero responses). The failure is "endpoint unreachable", NOT "repair tried
    /// and couldn't fix it" — reported as `local_repair_unavailable` so a dead
    /// server isn't blamed on the repair model.
    pub local_repair_unavailable: bool,
    pub memories_increased_over_time: bool,
    pub active_player_relationship_changed_when_warranted: bool,
    pub object_ids_stable: bool,
    pub default_player_not_normal_rp_relationship_target: bool,
    pub mne_export_succeeded: bool,
    pub pass: bool,
    pub failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkTurnSummary {
    pub turn_index: usize,
    pub stage: String,
    pub simulated_user_message: String,
    pub narrator_response_present: bool,
    pub narrator_error: Option<String>,
    pub evaluator_mode: String,
    pub structured_transport_actual: Option<String>,
    pub tool_calls_present: bool,
    pub tool_call_count: usize,
    pub structured_retry_count: usize,
    pub fallback_path: Vec<String>,
    pub syntactic_repair_used: bool,
    pub memory_count_after: usize,
    pub object_count_after: usize,
    pub relationship_summary_after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub benchmark_id: String,
    pub benchmark_type: String,
    pub conversation_id: String,
    pub started_at: i64,
    pub completed_at: i64,
    pub turn_count_requested: usize,
    pub turn_count_completed: usize,
    pub narrator_model: String,
    pub evaluator_model: String,
    pub player_simulator_model: Option<String>,
    pub narrator_failures: usize,
    pub evaluator_failures: usize,
    pub tool_call_success_count: usize,
    pub tool_call_failure_count: usize,
    pub retry_count: usize,
    pub retry_success_count: usize,
    pub fallback_count: usize,
    pub syntactic_repair_count: usize,
    pub default_player_leak_detected: bool,
    pub duplicate_relationship_context_detected: bool,
    pub final_memory_count: usize,
    pub final_object_state_count: usize,
    pub final_relationship_count: usize,
    pub visible_turns_requested: usize,
    pub visible_turns_completed: usize,
    pub visible_user_messages_created: usize,
    pub visible_assistant_messages_created: usize,
    pub unique_user_message_ids: usize,
    pub unique_assistant_message_ids: usize,
    pub internal_evaluator_retry_count: usize,
    pub internal_evaluator_retry_payload_count: usize,
    pub duplicate_turn_rows_detected: bool,
    pub duplicate_turn_message_pairs: Vec<String>,
    pub player_simulator_payload_count: usize,
    pub per_turn: Vec<BenchmarkTurnSummary>,
    pub object_identity_checks: Vec<BenchmarkObjectIdentityCheck>,
    pub mne_export_path: Option<String>,
    pub payload_history_path: Option<String>,
    pub summary_json_path: Option<String>,
    pub scorecard: BenchmarkScorecard,
}
