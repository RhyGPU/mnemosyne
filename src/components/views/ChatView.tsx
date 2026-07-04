import type { ReactNode } from "react";

export function ChatView({
  appDialogNode,
  children,
  pipelineRail,
  railNav,
  railShellClass,
}: {
  appDialogNode: ReactNode;
  children: ReactNode;
  pipelineRail: ReactNode;
  railNav: ReactNode;
  railShellClass: string;
}) {
  return (
    <div className={`chat-with-sidebar${railShellClass}`}>
      {appDialogNode}
      {railNav}
      {pipelineRail}
      {children}
    </div>
  );
}
