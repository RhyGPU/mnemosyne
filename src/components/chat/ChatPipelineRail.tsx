import type { TurnPipelineTrace } from "../../tauri";

type PipelineRailStage = {
  elapsed_ms: number;
  stage_name: string;
  status: string;
};

export function ChatPipelineRail({
  latestPipelineTrace,
  pipelineSteps,
  pipelineSummary,
}: {
  latestPipelineTrace: TurnPipelineTrace | null;
  pipelineSteps: PipelineRailStage[];
  pipelineSummary: string;
}) {
  return (
    <aside className="chat-pipeline-rail" aria-label="Turn pipeline">
      <div className="rail-progress-card">
        <div className="rail-progress-head">
          <span>Turn pipeline</span>
          <strong>{pipelineSummary}</strong>
        </div>
        <ol className="rail-progress-list">
          {pipelineSteps.map((stage, index) => {
            const isCompleted = ["completed", "ok", "success"].includes(stage.status);
            const isRunning = ["running", "pending", "in_progress", "ready"].includes(stage.status);
            const isFailed = stage.status === "failed" || stage.status === "error";
            const stateClass = isFailed ? "is-error" : isCompleted ? "is-done" : isRunning ? "is-active" : "is-waiting";

            return (
              <li key={`${stage.stage_name}-${index}`} className={`rail-progress-step ${stateClass}`}>
                <span className="rail-progress-node" aria-hidden="true" />
                <span className="rail-progress-copy">
                  <span className="rail-progress-name">{stage.stage_name}</span>
                  <span className="rail-progress-meta">
                    {latestPipelineTrace ? `${stage.status} / ${stage.elapsed_ms}ms` : index === 0 ? "Ready to send" : "Waiting"}
                  </span>
                </span>
              </li>
            );
          })}
        </ol>
      </div>
    </aside>
  );
}
