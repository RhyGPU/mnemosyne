use serde::{Deserialize, Serialize};
use state_engine::evaluator_form::EvalRowTrace;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnPipelineTrace {
    pub request_id: String,
    pub turn_id: Option<String>,
    pub conversation_id: String,
    pub started_at: i64,
    pub total_elapsed_ms: u64,
    pub final_status: String, // success | partial_success | failed | canceled | running
    pub failing_stage: Option<String>,
    pub suggested_debug_action: Option<String>,
    pub stages: Vec<PipelineStageTrace>,
    #[serde(default)]
    pub evaluator_row_traces: Vec<EvalRowTrace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TurnTokenUsage>,
}

/// Per-turn token accounting for the trace panel. Counts come from the
/// provider's `usage` block when reported; otherwise they are character-based
/// estimates and the matching `*_estimated` flag is true.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TurnTokenUsage {
    pub narrator_prompt_tokens: Option<u64>,
    pub narrator_completion_tokens: Option<u64>,
    pub narrator_estimated: bool,
    pub evaluator_prompt_tokens: Option<u64>,
    pub evaluator_completion_tokens: Option<u64>,
    pub evaluator_estimated: bool,
}

impl TurnTokenUsage {
    pub fn total_tokens(&self) -> u64 {
        self.narrator_prompt_tokens.unwrap_or(0)
            + self.narrator_completion_tokens.unwrap_or(0)
            + self.evaluator_prompt_tokens.unwrap_or(0)
            + self.evaluator_completion_tokens.unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PipelineStageTrace {
    pub stage_id: String,
    pub stage_name: String,
    pub status: String, // success | warning | skipped | failed
    pub elapsed_ms: u64,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
    pub error_code: Option<PipelineErrorCode>,
    pub error_message: Option<String>,
    pub repair_action: Option<String>,
    pub artifact_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineErrorCode {
    DatabaseError,
    ContextError,
    NarratorCallError,
    NarratorParseError,
    NarratorGuardViolation,
    EvaluatorCallError,
    EvaluatorJsonParseError,
    EvaluatorValidationError,
    EvaluatorSpawnError,
    EvaluatorParseError,
    EvaluatorNormalizeError,
    PatchApplicationError,
    StaleBranchSkipped,
    Timeout,
}

impl TurnPipelineTrace {
    pub fn new(
        request_id: String,
        turn_id: Option<String>,
        conversation_id: String,
        started_at: i64,
    ) -> Self {
        Self {
            request_id,
            turn_id,
            conversation_id,
            started_at,
            total_elapsed_ms: 0,
            final_status: "running".into(),
            failing_stage: None,
            suggested_debug_action: None,
            stages: Vec::new(),
            evaluator_row_traces: Vec::new(),
            token_usage: None,
        }
    }

    pub fn record_stage(
        &mut self,
        stage_name: &str,
        status: &str,
        elapsed_ms: u64,
        input: Option<String>,
        output: Option<String>,
    ) {
        self.stages.retain(|s| s.stage_name != stage_name);
        self.stages.push(PipelineStageTrace {
            stage_id: format!("{}_{}", stage_name, self.request_id),
            stage_name: stage_name.to_string(),
            status: status.to_string(),
            elapsed_ms,
            input_summary: input,
            output_summary: output,
            error_code: None,
            error_message: None,
            repair_action: None,
            artifact_ref: None,
        });
    }

    pub fn record_stage_error(
        &mut self,
        stage_name: &str,
        elapsed_ms: u64,
        error_code: PipelineErrorCode,
        error_message: String,
        repair: Option<String>,
    ) {
        self.stages.retain(|s| s.stage_name != stage_name);
        self.stages.push(PipelineStageTrace {
            stage_id: format!("{}_{}", stage_name, self.request_id),
            stage_name: stage_name.to_string(),
            status: "failed".to_string(),
            elapsed_ms,
            input_summary: None,
            output_summary: None,
            error_code: Some(error_code),
            error_message: Some(error_message),
            repair_action: repair,
            artifact_ref: None,
        });
        self.failing_stage = Some(stage_name.to_string());
        self.final_status = "failed".to_string();
    }

    pub fn finalize_timing(&mut self, elapsed_from_start: u64) {
        let sum_stages: u64 = self.stages.iter().map(|s| s.elapsed_ms).sum();
        if elapsed_from_start > 0 {
            self.total_elapsed_ms = std::cmp::max(elapsed_from_start, sum_stages);
        } else {
            self.total_elapsed_ms = sum_stages;
        }
        if self.total_elapsed_ms == 0 && sum_stages > 0 {
            self.total_elapsed_ms = sum_stages;
        }
    }
}
