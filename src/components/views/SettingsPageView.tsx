import type { ReactNode } from "react";

export function SettingsPageView({
  appDialogNode,
  railNav,
  railShellClass,
  settingsPageContent,
}: {
  appDialogNode: ReactNode;
  railNav: ReactNode;
  railShellClass: string;
  settingsPageContent: ReactNode;
}) {
  return (
    <main className={`app-shell settings-page${railShellClass}`}>
      {appDialogNode}
      {railNav}
      <header className="launcher-header">
        <div>
          <span className="eyebrow">Settings</span>
          <h1>Engine &amp; providers</h1>
        </div>
      </header>
      <div className="settings-page-body">{settingsPageContent}</div>
    </main>
  );
}
