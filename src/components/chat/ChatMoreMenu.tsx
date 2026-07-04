import { Archive, FileDown, RefreshCcw, Terminal } from "lucide-react";
import type { RefObject } from "react";

export function ChatMoreMenu({
  activeMessageCount,
  busy,
  currentConversationId,
  isArchived,
  menuOpen,
  menuRef,
  onArchive,
  onExportChat,
  onExportSession,
  onOpenDevMode,
  onRestoreHiddenTurns,
  onRestoreSession,
  setMenuOpen,
  triggerRef,
}: {
  activeMessageCount: number;
  busy: boolean;
  currentConversationId: string | null;
  isArchived: boolean;
  menuOpen: boolean;
  menuRef: RefObject<HTMLDivElement>;
  onArchive: () => void;
  onExportChat: () => void;
  onExportSession: () => void;
  onOpenDevMode: () => void;
  onRestoreHiddenTurns: () => void;
  onRestoreSession: () => void;
  setMenuOpen: (open: boolean | ((open: boolean) => boolean)) => void;
  triggerRef: RefObject<HTMLButtonElement>;
}) {
  const closeAfter = (action: () => void) => {
    action();
    setMenuOpen(false);
  };

  return (
    <div className="chat-more-menu-wrap">
      <button
        ref={triggerRef}
        type="button"
        className={`ghost-action chat-more-btn${menuOpen ? " open" : ""}`}
        title="More actions"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        onClick={() => setMenuOpen((open) => !open)}
      >
        ...
      </button>
      {menuOpen ? (
        <div className="chat-more-menu" role="menu" ref={menuRef}>
          <button type="button" role="menuitem" className="chat-more-menu-item" onClick={() => closeAfter(onOpenDevMode)}>
            <Terminal size={14} />
            <span>Dev Mode</span>
          </button>
          <div className="chat-more-menu-divider" role="separator" />
          {isArchived ? (
            <button type="button" role="menuitem" className="chat-more-menu-item" onClick={() => closeAfter(onRestoreSession)} disabled={busy}>
              <RefreshCcw size={14} />
              <span>Restore Session</span>
            </button>
          ) : (
            <button type="button" role="menuitem" className="chat-more-menu-item danger" onClick={() => closeAfter(onArchive)} disabled={busy}>
              <Archive size={14} />
              <span>Archive Session</span>
            </button>
          )}
          <div className="chat-more-menu-divider" role="separator" />
          <button
            type="button"
            role="menuitem"
            className="chat-more-menu-item"
            onClick={() => closeAfter(onRestoreHiddenTurns)}
            disabled={busy || !currentConversationId}
          >
            <RefreshCcw size={14} />
            <span>Restore Turns</span>
          </button>
          <button
            type="button"
            role="menuitem"
            className="chat-more-menu-item"
            onClick={() => closeAfter(onExportSession)}
            disabled={busy || !currentConversationId}
          >
            <FileDown size={14} />
            <span>Session .mne</span>
          </button>
          <button
            type="button"
            role="menuitem"
            className="chat-more-menu-item"
            onClick={() => closeAfter(onExportChat)}
            disabled={busy || activeMessageCount === 0}
          >
            <FileDown size={14} />
            <span>Export Chat</span>
          </button>
        </div>
      ) : null}
    </div>
  );
}
