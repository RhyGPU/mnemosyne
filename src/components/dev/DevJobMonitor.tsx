import { useEffect, useMemo, useState } from "react";
import type { BackgroundJobProgress } from "../../tauri";

const LIVE_STATUSES = new Set(["queued", "pending", "running", "stopping"]);

function formatDuration(milliseconds: number | null | undefined) {
  if (!milliseconds || milliseconds < 1_000) {
    return `${Math.max(0, Math.round(milliseconds ?? 0))}ms`;
  }
  const totalSeconds = Math.round(milliseconds / 1_000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`;
}

function statusClass(status: string) {
  if (status === "succeeded" || status === "completed") return "ok";
  if (status === "failed" || status === "timed_out") return "err";
  if (status === "canceled" || status === "cancelled") return "warn";
  if (LIVE_STATUSES.has(status)) return "run";
  return "skip";
}

export function DevJobMonitor({
  jobs,
  onCancel,
  onDismiss,
}: {
  jobs: BackgroundJobProgress[];
  onCancel: (job: BackgroundJobProgress) => void;
  onDismiss: (jobId: string) => void;
}) {
  const [, setClock] = useState(0);
  const hasLiveJobs = jobs.some((job) => LIVE_STATUSES.has(job.status));

  useEffect(() => {
    if (!hasLiveJobs) return;
    const timer = window.setInterval(() => setClock((clock) => clock + 1), 1_000);
    return () => window.clearInterval(timer);
  }, [hasLiveJobs]);

  const orderedJobs = useMemo(
    () =>
      [...jobs].sort((left, right) => {
        const leftLive = LIVE_STATUSES.has(left.status) ? 1 : 0;
        const rightLive = LIVE_STATUSES.has(right.status) ? 1 : 0;
        return rightLive - leftLive || right.updated_at - left.updated_at;
      }),
    [jobs],
  );

  if (orderedJobs.length === 0) {
    return <div className="cli-rail-empty">no background jobs_</div>;
  }

  return (
    <div className="cli-job-monitor" aria-live="polite">
      {orderedJobs.map((job) => {
        const progress = job.total > 0 ? Math.min(100, Math.round((job.current / job.total) * 100)) : 0;
        const live = LIVE_STATUSES.has(job.status);
        const elapsed = live && job.started_at
          ? Math.max(job.elapsed_ms, Date.now() - job.started_at * 1_000)
          : job.elapsed_ms;
        return (
          <section className={`cli-job-row ${statusClass(job.status)}`} key={job.job_id}>
            <div className="cli-job-heading">
              <div>
                <strong>{job.label}</strong>
                <span>{job.phase || job.status}</span>
              </div>
              <span className="cli-job-status">{job.status}</span>
            </div>
            {job.total > 0 ? (
              <>
                <div className="cli-job-progress-line">
                  <span>{job.current}/{job.total}</span>
                  <span>{progress}%</span>
                </div>
                <div className="cli-meter-track" aria-label={`${job.label} progress`}>
                  <span style={{ width: `${progress}%` }} />
                </div>
              </>
            ) : null}
            <div className="cli-job-counts">
              <span>ok {job.succeeded}</span>
              <span>fail {job.failed}</span>
              <span>recover {job.recovered}</span>
            </div>
            <div className="cli-job-timing">
              <span>elapsed {formatDuration(elapsed)}</span>
              {job.estimated_remaining_ms != null ? (
                <span>eta {formatDuration(job.estimated_remaining_ms)}</span>
              ) : null}
            </div>
            {job.detail ? <p>{job.detail}</p> : null}
            {job.history.length > 0 ? (
              <details className="cli-job-history">
                <summary>history [{job.history.length}]</summary>
                <div>
                  {job.history.map((entry) => (
                    <div className={`cli-job-history-row ${statusClass(entry.status)}`} key={`${entry.index}-${entry.label}`}>
                      <span>{String(entry.index).padStart(2, "0")}</span>
                      <strong>{entry.label}</strong>
                      <span>{entry.status}</span>
                      <span>{formatDuration(entry.elapsed_ms)}</span>
                      {entry.detail ? <small>{entry.detail}</small> : null}
                    </div>
                  ))}
                </div>
              </details>
            ) : null}
            <div className="cli-job-actions">
              {live && job.cancellable ? (
                <button type="button" className="cli-mini-btn" onClick={() => onCancel(job)}>
                  [ CANCEL ]
                </button>
              ) : null}
              {!live ? (
                <button type="button" className="cli-mini-btn" onClick={() => onDismiss(job.job_id)}>
                  [ DISMISS ]
                </button>
              ) : null}
            </div>
          </section>
        );
      })}
    </div>
  );
}
