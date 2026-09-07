import type { EvaluatorJob } from "../../../tauri";

export function evaluatorJobStatusText(job: EvaluatorJob) {
  if (job.status === "pending" || job.status === "running") return "Updating memory/state...";
  if (job.status === "completed" || job.status === "partial_success") {
    if (job.error_message && (job.error_message.startsWith("State updated") || job.error_message.includes("skipped"))) {
      return job.error_message;
    }
  }
  if (job.patch_applied) {
    if (job.error_message && (job.error_message.startsWith("State updated") || job.error_message.includes("skipped"))) {
      return job.error_message;
    }
  } else if (job.status === "failed") {
    return "State update failed";
  }
  if (job.status === "completed") {
    return job.patch_applied ? "Memory/state update completed" : "Memory/state update completed with no patch";
  }
  if (job.status === "partial_success") {
    // A partial success that committed nothing is not a partial anything. This
    // read "State updated partially" while the turn's state was lost outright,
    // which is only discovered much later, as the character not remembering.
    if (!job.patch_applied) {
      return "This turn's state was not saved";
    }
    if (job.error_message?.includes("some enrichment rows rejected")) {
      return "State updated; some enrichment rows rejected";
    }
    if (job.error_message?.includes("branch_advanced_before_background_evaluator_completed")) {
      return "State updated; enrichment finished after branch advanced";
    }
    return "State updated partially";
  }
  if (job.status === "some_rows_rejected") return "State updated; some enrichment rows rejected";
  if (job.status === "stale_skipped") return "State updated; enrichment skipped";
  if (job.status === "canceled") return "State update canceled";
  if (job.status === "timed_out") return "State update timed out";
  if (job.status === "failed") return "State update failed";
  return job.status;
}

export function evaluatorJobBannerTitle(job: EvaluatorJob) {
  if (job.status === "pending" || job.status === "running") return "Updating memory/state...";
  if (job.patch_applied) {
    if (job.error_message && (job.error_message.startsWith("State updated") || job.error_message.includes("skipped"))) {
      return job.error_message;
    }
  }
  if (job.status === "completed") return "Memory/state updated";
  if (job.status === "partial_success") return evaluatorJobStatusText(job);
  if (job.status === "some_rows_rejected") return "State updated; some enrichment rows rejected";
  if (job.status === "stale_skipped") return "State updated; enrichment skipped";
  if (job.status === "canceled") return "State update canceled";
  if (job.status === "timed_out") return "State update timed out";
  if (job.status === "failed") return "State update failed";
  return job.status;
}

/**
 * Did this job leave the turn without state?
 *
 * `partial_success` covers two endings: rows were read and some rejected, or
 * every path failed and a no-op was recorded to keep the pipeline moving. Only
 * the second loses the turn, and only `patch_applied` tells them apart.
 */
export function evaluatorJobLostTheTurn(job: Pick<EvaluatorJob, "status" | "patch_applied">) {
  return job.status === "partial_success" && !job.patch_applied;
}

export function evaluatorJobRefreshesState(job: EvaluatorJob) {
  return ["completed", "partial_success", "some_rows_rejected", "stale_skipped"].includes(job.status);
}
