import type { RefObject } from "react";
import type { BackgroundJobProgress } from "../../tauri";
import { DevJobMonitor } from "./DevJobMonitor";

export type DevPipelineStage = {
  stage_name: string;
  status: string;
  elapsed_ms: number;
  error_message?: string | null;
};

export type DevStreamItem =
  | { kind: "chat"; key: string; t: number; role: string; content: string }
  | { kind: "log"; key: string; t: number; level: string; category: string; message: string };

function stageGlyph(status: string) {
  if (status === "failed") return { symbol: "[ERR]", className: "err" };
  if (status === "warning") return { symbol: "[WRN]", className: "warn" };
  if (status === "skipped") return { symbol: "[--]", className: "skip" };
  if (status === "running" || status === "pending" || status === "in_progress") {
    return { symbol: "[...]", className: "run" };
  }
  return { symbol: "[OK]", className: "ok" };
}

export function DevPipelinePanel({
  hasPipelineTrace,
  jobs,
  onCancelJob,
  onDismissJob,
  pipelineStages,
  pipelineSummary,
}: {
  hasPipelineTrace: boolean;
  jobs: BackgroundJobProgress[];
  onCancelJob: (job: BackgroundJobProgress) => void;
  onDismissJob: (jobId: string) => void;
  pipelineStages: DevPipelineStage[];
  pipelineSummary: string;
}) {
  return (
    <aside className="cli-rail" aria-label="Pipeline">
      <div className="cli-rail-title">// JOBS</div>
      <DevJobMonitor jobs={jobs} onCancel={onCancelJob} onDismiss={onDismissJob} />

      <div className="cli-rail-title">// LAST TURN PIPELINE</div>
      {hasPipelineTrace ? (
        <>
          <div className="cli-rail-status">{pipelineSummary}</div>
          {pipelineStages.map((stage) => {
            const glyph = stageGlyph(stage.status);
            return (
              <div
                className={`cli-stage ${glyph.className}`}
                key={stage.stage_name}
                title={stage.error_message ?? ""}
              >
                <span className="cli-stage-sym">{glyph.symbol}</span>
                <span className="cli-stage-name">{stage.stage_name}</span>
                <span className="cli-stage-ms">{stage.elapsed_ms}ms</span>
              </div>
            );
          })}
        </>
      ) : (
        <div className="cli-rail-empty">awaiting first turn_</div>
      )}
    </aside>
  );
}

export function DevStreamPanel({
  items,
  streamRef,
}: {
  items: DevStreamItem[];
  streamRef: RefObject<HTMLDivElement>;
}) {
  return (
    <section className="cli-stream" ref={streamRef} aria-label="Stream">
      {items.length === 0 ? (
        <div className="cli-line muted">// stream empty - type /chat &lt;message&gt; to begin</div>
      ) : (
        items.map((item) =>
          item.kind === "chat" ? (
            <div className="cli-line chatlog" key={item.key}>
              <span className="cli-tag">chatlog=</span>
              <span className="cli-role">{item.role}:</span>
              <span className="cli-text">{item.content}</span>
            </div>
          ) : (
            <div className={`cli-line log ${item.level}`} key={item.key}>
              <span className="cli-tag">log=</span>
              <span className="cli-cat">[{item.category}]</span>
              <span className="cli-text">{item.message}</span>
            </div>
          ),
        )
      )}
    </section>
  );
}
