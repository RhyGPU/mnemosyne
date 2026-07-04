import type { ReactNode } from "react";

export function StateMapView({
  appDialogNode,
  children,
  railNav,
  railShellClass,
}: {
  appDialogNode: ReactNode;
  children: ReactNode;
  railNav: ReactNode;
  railShellClass: string;
}) {
  return (
    <main className={`app-shell statemap-shell${railShellClass}`}>
      {appDialogNode}
      {railNav}
      {children}
    </main>
  );
}
