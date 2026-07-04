import type { ReactNode } from "react";

export function LibraryView({
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
    <main className={`app-shell launcher-shell${railShellClass}`}>
      {appDialogNode}
      {railNav}
      {children}
    </main>
  );
}
