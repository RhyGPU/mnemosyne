import type {
  ApiProviderSettings,
  BenchmarkSettings,
  BenchmarkTurnSummary,
  EvaluatorJob,
  TurnResult,
} from "../../../tauri";

const COMPLETED_EVALUATOR_STATUSES = new Set([
  "completed",
  "partial_success",
  "some_rows_rejected",
  "stale_skipped",
]);

/**
 * A job that reached a terminal status but committed nothing.
 *
 * `partial_success` covers two different endings: the evaluator read the turn
 * and some rows were rejected, or every fallback path failed and a no-op was
 * recorded to keep the pipeline moving. Only the second leaves the state
 * untouched, and counting it as a completed turn is how ten turns of a dead
 * evaluator once reported as ten successes. `stale_skipped` is excluded because
 * skipping a superseded turn is the correct outcome, not a lost one.
 */
export function benchmarkEvaluatorJobCommittedNothing(
  job: Pick<EvaluatorJob, "status" | "patch_applied"> | null | undefined,
): boolean {
  return Boolean(job && job.status === "partial_success" && !job.patch_applied);
}

export function benchmarkLiveUpdaterOverride(
  settings: BenchmarkSettings,
): Partial<ApiProviderSettings> {
  const waitOverride: Partial<ApiProviderSettings> = settings.wait_for_evaluator_each_turn
    ? {
        evaluator_background_enabled: true,
        wait_for_evaluator_before_next_turn: false,
        allow_send_with_stale_state: true,
      }
    : {};

  if (!settings.strict_tool_evaluator) {
    return waitOverride;
  }

  return {
    ...waitOverride,
    evaluator_mode: settings.evaluator_mode ?? undefined,
    structured_evaluator_transport: settings.structured_evaluator_transport ?? undefined,
    structured_evaluator_policy: settings.structured_evaluator_policy ?? undefined,
    structured_evaluator_max_retries: settings.structured_evaluator_max_retries ?? undefined,
  };
}

export function turnResultEvaluatorCompletedOrSkipped(
  result: Pick<TurnResult, "debug"> | null | undefined,
): boolean {
  const status = result?.debug?.state_updater_status;
  return Boolean(
    status &&
      (status.startsWith("background_") || COMPLETED_EVALUATOR_STATUSES.has(status)),
  );
}

export function benchmarkEvaluatorJobCompletedOrSkipped(
  job: Pick<EvaluatorJob, "status" | "patch_applied"> | null | undefined,
): boolean {
  if (benchmarkEvaluatorJobCommittedNothing(job)) return false;
  return Boolean(job && COMPLETED_EVALUATOR_STATUSES.has(job.status));
}

export function fallbackBenchmarkTurnSummary(
  turnIndex: number,
  stage: string,
  userText: string,
  error: string,
  stateUpdaterSettings: Pick<ApiProviderSettings, "evaluator_mode">,
): BenchmarkTurnSummary {
  return {
    turn_index: turnIndex,
    stage,
    simulated_user_message: userText,
    narrator_response_present: false,
    narrator_error: error,
    evaluator_mode: stateUpdaterSettings.evaluator_mode || "evaluator_form_v1",
    tool_calls_present: false,
    tool_call_count: 0,
    structured_retry_count: 0,
    fallback_path: [],
    syntactic_repair_used: false,
    memory_count_after: 0,
    object_count_after: 0,
    relationship_summary_after: "",
  };
}
