import type { ReactNode } from "react";

export function HomeView({
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
    <main className={`app-shell home-shell${railShellClass}`}>
      {appDialogNode}
      {railNav}
      {children}
    </main>
  );
}
