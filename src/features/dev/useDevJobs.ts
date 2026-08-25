import { useRef, useState } from "react";
import type { BackgroundJobProgress } from "../../tauri";

export type StartDevJobOptions = {
  total?: number;
  phase?: string;
  detail?: string;
  cancellable?: boolean;
  jobId?: string;
};

export function useDevJobs() {
  const [jobsById, setJobsById] = useState<Record<string, BackgroundJobProgress>>({});
  const [dismissedJobIds, setDismissedJobIds] = useState<Set<string>>(() => new Set());
  const localSequenceRef = useRef(0);

  function startJob(
    kind: string,
    label: string,
    options: StartDevJobOptions = {},
  ) {
    localSequenceRef.current += 1;
    const nowSeconds = Math.floor(Date.now() / 1_000);
    const jobId = options.jobId ?? `${kind}:${nowSeconds}:${localSequenceRef.current}`;
    const job: BackgroundJobProgress = {
      job_id: jobId,
      kind,
      label,
      status: "running",
      phase: options.phase ?? "starting",
      current: 0,
      total: options.total ?? 0,
      succeeded: 0,
      failed: 0,
      recovered: 0,
      started_at: nowSeconds,
      updated_at: nowSeconds,
      elapsed_ms: 0,
      estimated_remaining_ms: null,
      detail: options.detail ?? null,
      cancellable: options.cancellable ?? false,
      history: [],
    };
    setDismissedJobIds((current) => {
      if (!current.has(jobId)) return current;
      const next = new Set(current);
      next.delete(jobId);
      return next;
    });
    setJobsById((current) => ({ ...current, [jobId]: job }));
    return jobId;
  }

  function updateJob(
    jobId: string | null | undefined,
    update: Partial<BackgroundJobProgress>,
  ) {
    if (!jobId) return;
    setJobsById((current) => {
      const existing = current[jobId];
      if (!existing) return current;
      const nowSeconds = Math.floor(Date.now() / 1_000);
      return {
        ...current,
        [jobId]: {
          ...existing,
          ...update,
          updated_at: nowSeconds,
          elapsed_ms: Math.max(
            update.elapsed_ms ?? 0,
            Date.now() - existing.started_at * 1_000,
          ),
        },
      };
    });
  }

  function finishJob(
    jobId: string | null | undefined,
    status: "succeeded" | "failed" | "canceled",
    update: Partial<BackgroundJobProgress> = {},
  ) {
    updateJob(jobId, {
      ...update,
      status,
      phase: update.phase ?? "complete",
      estimated_remaining_ms: 0,
      cancellable: false,
    });
  }

  function appendHistory(
    jobId: string | null | undefined,
    entry: BackgroundJobProgress["history"][number],
    update: Partial<BackgroundJobProgress> = {},
  ) {
    if (!jobId) return;
    setJobsById((current) => {
      const existing = current[jobId];
      if (!existing) return current;
      const nowSeconds = Math.floor(Date.now() / 1_000);
      return {
        ...current,
        [jobId]: {
          ...existing,
          ...update,
          updated_at: nowSeconds,
          elapsed_ms: Date.now() - existing.started_at * 1_000,
          history: [...existing.history, entry],
        },
      };
    });
  }

  function dismissJob(jobId: string) {
    setDismissedJobIds((current) => new Set(current).add(jobId));
  }

  function ingestJob(progress: BackgroundJobProgress) {
    if (progress.status === "running") {
      setDismissedJobIds((current) => {
        if (!current.has(progress.job_id)) return current;
        const next = new Set(current);
        next.delete(progress.job_id);
        return next;
      });
    }
    setJobsById((current) => ({
      ...current,
      [progress.job_id]: progress,
    }));
  }

  async function runOperation<T>(
    kind: string,
    label: string,
    operation: () => Promise<T>,
    options: {
      detail?: string;
      successDetail?: (result: T) => string;
    } = {},
  ): Promise<T> {
    const jobId = startJob(kind, label, {
      total: 1,
      phase: "running",
      detail: options.detail,
    });
    try {
      const result = await operation();
      const successDetail = options.successDetail?.(result);
      finishJob(jobId, "succeeded", {
        current: 1,
        total: 1,
        succeeded: 1,
        detail: successDetail ?? `${label} complete`,
        history: [
          {
            index: 1,
            label,
            status: "succeeded",
            detail: successDetail ?? null,
            elapsed_ms: null,
          },
        ],
      });
      return result;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      finishJob(jobId, "failed", {
        current: 1,
        total: 1,
        failed: 1,
        detail: message,
        history: [
          {
            index: 1,
            label,
            status: "failed",
            detail: message,
            elapsed_ms: null,
          },
        ],
      });
      throw error;
    }
  }

  return {
    jobsById,
    dismissedJobIds,
    startJob,
    updateJob,
    finishJob,
    appendHistory,
    dismissJob,
    ingestJob,
    runOperation,
  };
}
