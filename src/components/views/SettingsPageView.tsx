import type { ReactNode } from "react";
import { BookOpen, Clipboard, FolderOpen, SlidersHorizontal, Sparkles } from "lucide-react";
import type { SettingsTab } from "../../settings/preferences";

export function SettingsPageView({
  activePanel,
  appDialogNode,
  onTabChange,
  railNav,
  railShellClass,
  settingsTab,
}: {
  activePanel: ReactNode;
  appDialogNode: ReactNode;
  onTabChange: (tab: SettingsTab) => void;
  railNav: ReactNode;
  railShellClass: string;
  settingsTab: SettingsTab;
}) {
  const categories: Array<{ id: SettingsTab; label: string; icon: ReactNode }> = [
    { id: "ai", label: "AI", icon: <Sparkles size={18} aria-hidden="true" /> },
    { id: "generation", label: "Generation", icon: <SlidersHorizontal size={18} aria-hidden="true" /> },
    { id: "reading", label: "Reading", icon: <BookOpen size={18} aria-hidden="true" /> },
    { id: "data", label: "Data", icon: <FolderOpen size={18} aria-hidden="true" /> },
    { id: "about", label: "About", icon: <Clipboard size={18} aria-hidden="true" /> },
  ];

  return (
    <main className={`app-shell settings-page${railShellClass}`}>
      {appDialogNode}
      {railNav}
      <header className="launcher-header">
        <div>
          <span className="eyebrow">Preferences</span>
          <h1>Settings</h1>
        </div>
      </header>
      <div className="settings-page-body">
        <div className="settings-page-layout">
          <aside className="settings-page-nav" aria-label="Settings categories">
            {categories.map((tab) => (
              <button
                key={tab.id}
                type="button"
                className={settingsTab === tab.id ? "selected" : ""}
                onClick={() => onTabChange(tab.id)}
              >
                {tab.icon}
                <span>{tab.label}</span>
              </button>
            ))}
          </aside>
          <section className="settings-page-panel">{activePanel}</section>
        </div>
      </div>
    </main>
  );
}
