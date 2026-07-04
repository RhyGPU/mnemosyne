import type { ReactNode } from "react";

export function DevModeShell({
  appDialogNode,
  diagnostics,
  inputForm,
  onExitDev,
  onOpenLibrary,
  pipelineRail,
  sessionTitle,
  stream,
}: {
  appDialogNode: ReactNode;
  diagnostics: ReactNode;
  inputForm: ReactNode;
  onExitDev: () => void;
  onOpenLibrary: () => void;
  pipelineRail: ReactNode;
  sessionTitle: string;
  stream: ReactNode;
}) {
  return (
    <main className="cli-shell">
      {appDialogNode}
      <div className="cli-scanlines" aria-hidden="true" />
      <header className="cli-header">
        <span className="cli-brand">root@mnemosyne</span>
        <span className="cli-path">:~/{sessionTitle.replace(/\s+/g, "_").toLowerCase()}$</span>
        <span className="cli-spacer" />
        <button type="button" className="cli-btn" onClick={onExitDev}>[ EXIT DEV ]</button>
        <button type="button" className="cli-btn" onClick={onOpenLibrary}>[ LIBRARY ]</button>
      </header>
      <div className="cli-body">
        {pipelineRail}
        {stream}
        {diagnostics}
      </div>
      {inputForm}
    </main>
  );
}
