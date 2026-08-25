import type { ChangeEvent, FormEvent, KeyboardEvent, ReactNode, RefObject } from "react";
import {
  ArrowLeft,
  ArrowRight,
  ChevronDown,
  Image as ImageIcon,
  Pencil,
  Play,
  RefreshCcw,
  Square,
  Terminal,
  Trash2,
  X,
} from "lucide-react";
import type {
  AssistantMessageVariant,
  ChatMessage,
  EvaluatorJob,
  ImageAsset,
} from "../../tauri";
import type { SlashCommandSuggestion } from "../../slashCommands";
import { splitAssistantDisplay } from "../../features/chat/model/assistantDisplay";
import { AssetImage, SoulAvatar } from "../primitives";

export function ChatWorkspaceHeader({
  avatar,
  busy,
  characterName,
  contextTokens,
  devLogCount,
  hasConversation,
  mode,
  moreMenu,
  onBackToLibrary,
  onOpenDevMode,
  onRename,
  provider,
  sessionContinuityLabel,
  sessionTitle,
  settingName,
  settingsToggle,
}: {
  avatar: ImageAsset | null;
  busy: boolean;
  characterName: string;
  contextTokens: number;
  devLogCount: number;
  hasConversation: boolean;
  mode: string;
  moreMenu: ReactNode;
  onBackToLibrary: () => void;
  onOpenDevMode: () => void;
  onRename: () => void;
  provider: string;
  sessionContinuityLabel: string;
  sessionTitle: string;
  settingName: string;
  settingsToggle: ReactNode;
}) {
  return (
    <header className="chat-only-header">
      <button className="ghost-action chat-back-mobile" onClick={onBackToLibrary}>
        <ArrowLeft size={18} aria-hidden="true" />
        <span>Library</span>
      </button>
      <SoulAvatar soulName={characterName} asset={avatar} />
      <div className="chat-header-info">
        <span className="eyebrow" title={`${settingName} / ${provider} / ${mode}`}>
          {settingName} / {provider} / {mode}
        </span>
        <h1>
          <span title={sessionTitle}>{sessionTitle}</span>
          <button
            className="inline-icon-button"
            type="button"
            title="Rename session"
            onClick={onRename}
            disabled={busy}
          >
            <Pencil size={14} aria-hidden="true" />
          </button>
        </h1>
        <p className="session-state-label" title={`${characterName} - ${sessionContinuityLabel}`}>
          {characterName} - {sessionContinuityLabel}
        </p>
      </div>
      <div className="chat-top-actions">
        {moreMenu}
        <div className="token-pill">
          {contextTokens}
          <span>tok</span>
        </div>
        {settingsToggle}
        <button
          type="button"
          className="dev-mode-toggle"
          onClick={onOpenDevMode}
          disabled={!hasConversation}
          title="Enter terminal Dev Mode"
        >
          <Terminal size={16} aria-hidden="true" />
          <span>Dev Mode</span>
          <strong>{devLogCount}</strong>
        </button>
      </div>
    </header>
  );
}

export function ChatTranscript({
  avatar,
  bodyRef,
  bottomRef,
  busy,
  canGenerateFromUserMessage,
  characterName,
  messages,
  onDeleteMessage,
  onEditMessage,
  onFixMessage,
  onJumpToLatest,
  onPreviewImage,
  onRegenerateMessage,
  onScroll,
  onSelectVariant,
  showJumpToLatest,
  turnInProgress,
  variantsByMessage,
}: {
  avatar: ImageAsset | null;
  bodyRef: RefObject<HTMLElement>;
  bottomRef: RefObject<HTMLDivElement>;
  busy: boolean;
  canGenerateFromUserMessage: (message: ChatMessage) => boolean;
  characterName: string;
  messages: ChatMessage[];
  onDeleteMessage: (message: ChatMessage) => void;
  onEditMessage: (message: ChatMessage) => void;
  onFixMessage: (message: ChatMessage) => void;
  onJumpToLatest: () => void;
  onPreviewImage: (image: ImageAsset) => void;
  onRegenerateMessage: (message: ChatMessage) => void;
  onScroll: () => void;
  onSelectVariant: (message: ChatMessage, direction: -1 | 1) => void;
  showJumpToLatest: boolean;
  turnInProgress: boolean;
  variantsByMessage: Record<string, AssistantMessageVariant[]>;
}) {
  return (
    <section className="chat-only-scroll" ref={bodyRef} onScroll={onScroll}>
      <div className="chat-only-body" aria-live="polite" aria-atomic="false">
        {messages.length === 0 ? (
          <div className="empty-state chat-empty">
            <SoulAvatar soulName={characterName} asset={avatar} />
            <h2>Start the scene with {characterName || "your character"}</h2>
            <p>
              Type an action or a line of dialogue below to begin. Mnemosyne tracks memory,
              mood, and relationships as the story unfolds.
            </p>
            <ul className="chat-empty-hints">
              <li>
                <code>*you step inside, still damp from the rain*</code> - narrate an action
              </li>
              <li>
                <code>/ooc &lt;message&gt;</code> - talk out of character
              </li>
              <li>
                <code>/help</code> - list every command
              </li>
            </ul>
          </div>
        ) : (
          messages.map((message) => {
            const variants = variantsByMessage[String(message.id)] ?? [];
            const selectedIndex = getSelectedVariantIndex(variants);
            const canGenerateFromUser = canGenerateFromUserMessage(message);
            const assistantDisplay =
              message.role === "assistant" ? splitAssistantDisplay(message.content) : null;
            const olderGenerationTitle =
              "Regenerating older messages requires branch rewind and will be added later.";

            return (
              <article className={`message ${message.role}`} key={message.id}>
                <div className="message-heading">
                  <span>
                    {message.channel?.startsWith("command_")
                      ? "Command"
                      : message.role === "user"
                        ? "User"
                        : "Narrator"}
                  </span>
                  {message.role === "assistant" ? (
                    <div className="message-tools">
                      <div className="variant-switcher" aria-label="Response variants">
                        <button
                          title="Previous variant"
                          onClick={() => onSelectVariant(message, -1)}
                          disabled={
                            turnInProgress || variants.length <= 1 || selectedIndex <= 0
                          }
                        >
                          <ArrowLeft size={13} aria-hidden="true" />
                        </button>
                        <span>
                          {variants.length ? selectedIndex + 1 : 1} /{" "}
                          {Math.max(variants.length, 1)}
                        </span>
                        <button
                          title="Next variant"
                          onClick={() => onSelectVariant(message, 1)}
                          disabled={
                            turnInProgress ||
                            variants.length <= 1 ||
                            selectedIndex >= variants.length - 1
                          }
                        >
                          <ArrowRight size={13} aria-hidden="true" />
                        </button>
                      </div>
                      <button
                        title="Hide/Rewind response"
                        onClick={() => onDeleteMessage(message)}
                        disabled={turnInProgress}
                      >
                        <Trash2 size={14} aria-hidden="true" />
                      </button>
                    </div>
                  ) : (
                    <div className="message-tools">
                      <button
                        className="message-tool-action"
                        title={
                          canGenerateFromUser
                            ? "Regenerate response from this user message"
                            : olderGenerationTitle
                        }
                        onClick={() => onRegenerateMessage(message)}
                        disabled={turnInProgress || !canGenerateFromUser}
                      >
                        <RefreshCcw size={14} aria-hidden="true" />
                        <span>Regenerate</span>
                      </button>
                      <button
                        className="message-tool-action"
                        title={
                          canGenerateFromUser
                            ? "Fix response with instruction"
                            : olderGenerationTitle
                        }
                        onClick={() => onFixMessage(message)}
                        disabled={turnInProgress || !canGenerateFromUser}
                      >
                        <span>Fix</span>
                      </button>
                      <button
                        title="Edit this message"
                        onClick={() => onEditMessage(message)}
                        disabled={turnInProgress}
                      >
                        <Pencil size={14} aria-hidden="true" />
                      </button>
                      <button
                        title="Hide/Rewind message"
                        onClick={() => onDeleteMessage(message)}
                        disabled={turnInProgress}
                      >
                        <Trash2 size={14} aria-hidden="true" />
                      </button>
                    </div>
                  )}
                </div>
                {assistantDisplay ? (
                  <>
                    {assistantDisplay.prose ? (
                      <pre className="message-prose">{assistantDisplay.prose}</pre>
                    ) : null}
                    {assistantDisplay.status ? (
                      <details className="message-status">
                        <summary>Scene state</summary>
                        <p>{assistantDisplay.status}</p>
                      </details>
                    ) : null}
                  </>
                ) : (
                  <p>{message.content}</p>
                )}
                {message.attachments?.length ? (
                  <div className="message-attachments">
                    {message.attachments.map((attachment) => (
                      <button
                        className="image-attachment"
                        type="button"
                        key={attachment.id}
                        onClick={() => onPreviewImage(attachment.image)}
                        title="Open image preview"
                      >
                        <AssetImage asset={attachment.image} alt="Chat attachment" />
                        <span>
                          {attachment.image.source}
                          {attachment.image.width && attachment.image.height
                            ? ` / ${attachment.image.width}x${attachment.image.height}`
                            : ""}
                        </span>
                      </button>
                    ))}
                  </div>
                ) : null}
              </article>
            );
          })
        )}
        {busy && messages.length > 0 ? (
          <div className="typing-indicator" aria-label={`${characterName || "Narrator"} is writing`}>
            <span />
            <span />
            <span />
          </div>
        ) : null}
        <div ref={bottomRef} aria-hidden="true" />
      </div>
      {showJumpToLatest ? (
        <button type="button" className="jump-to-latest" onClick={onJumpToLatest}>
          <ChevronDown size={16} aria-hidden="true" />
          <span>Jump to latest</span>
        </button>
      ) : null}
    </section>
  );
}

export function EvaluatorStatusBanner({
  allowProceedWithStaleState,
  isLive,
  job,
  onCancel,
  onClose,
  onProceed,
  onRetry,
  title,
}: {
  allowProceedWithStaleState: boolean;
  isLive: boolean;
  job: EvaluatorJob | null;
  onCancel: () => void;
  onClose: () => void;
  onProceed: () => void;
  onRetry: () => void;
  title: string;
}) {
  if (!job) return null;
  const canRetry =
    job.status === "failed" || job.status === "canceled" || job.status === "timed_out";

  return (
    <section className={`evaluator-job-banner ${job.status}`}>
      <button
        type="button"
        className="evaluator-job-banner-close"
        aria-label="Close state updater status"
        title="Close"
        onClick={onClose}
      >
        <X size={14} aria-hidden="true" />
      </button>
      <div>
        <strong>{title}</strong>
        <span>
          {job.model || "Evaluator"} / {job.status}
          {job.elapsed_ms ? ` / ${job.elapsed_ms}ms` : ""}
        </span>
        {job.error_message ? <small>{job.error_message}</small> : null}
      </div>
      <div className="evaluator-job-actions">
        {isLive ? (
          <button type="button" onClick={onCancel}>
            Cancel
          </button>
        ) : null}
        {canRetry ? (
          <button type="button" onClick={onRetry}>
            Retry
          </button>
        ) : null}
        {isLive && allowProceedWithStaleState ? (
          <button type="button" onClick={onProceed}>
            Proceed
          </button>
        ) : null}
      </div>
    </section>
  );
}

export function ChatComposer({
  busy,
  characterName,
  draft,
  imageInputRef,
  onDraftChange,
  onImageSelected,
  onInsertSlashCommand,
  onKeyDown,
  onStopGeneration,
  onSubmit,
  selectedSlashIndex,
  slashMenuOpen,
  slashSuggestions,
  soulAvailable,
  stateUpdating,
}: {
  busy: boolean;
  characterName: string;
  draft: string;
  imageInputRef: RefObject<HTMLInputElement>;
  onDraftChange: (value: string) => void;
  onImageSelected: (event: ChangeEvent<HTMLInputElement>) => void;
  onInsertSlashCommand: (command: string) => void;
  onKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => void;
  onStopGeneration: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  selectedSlashIndex: number;
  slashMenuOpen: boolean;
  slashSuggestions: SlashCommandSuggestion[];
  soulAvailable: boolean;
  stateUpdating: boolean;
}) {
  return (
    <form className="chat-only-composer" onSubmit={onSubmit}>
      <input
        ref={imageInputRef}
        className="hidden-file"
        type="file"
        accept="image/png,image/jpeg,image/webp,image/gif,.png,.jpg,.jpeg,.webp,.gif"
        onChange={onImageSelected}
      />
      <div className="composer-input-shell">
        <textarea
          value={draft}
          onChange={(event) => onDraftChange(event.target.value)}
          onKeyDown={onKeyDown}
          placeholder={`Message ${characterName} - narrate an action or speak. "/" for commands, Enter to send`}
          disabled={busy}
          rows={2}
          aria-autocomplete="list"
          aria-controls="slash-command-menu"
          aria-expanded={slashMenuOpen}
        />
        {slashMenuOpen ? (
          <div id="slash-command-menu" className="slash-command-menu" role="listbox">
            {slashSuggestions.map((item, index) => (
              <button
                key={item.command}
                type="button"
                className={index === selectedSlashIndex ? "selected" : ""}
                role="option"
                aria-selected={index === selectedSlashIndex}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => onInsertSlashCommand(item.command)}
              >
                <strong>{item.command}</strong>
                <span>{item.usage}</span>
                <small>{item.description}</small>
              </button>
            ))}
          </div>
        ) : null}
      </div>
      {busy ? (
        <button type="button" aria-label="Stop generation" onClick={onStopGeneration}>
          <Square size={16} aria-hidden="true" />
        </button>
      ) : (
        <>
          <button
            type="button"
            aria-label="Attach image"
            title="Attach image"
            onClick={() => imageInputRef.current?.click()}
            disabled={!soulAvailable || stateUpdating}
          >
            <ImageIcon size={18} aria-hidden="true" />
          </button>
          <button
            aria-label="Send message"
            disabled={!draft.trim() || !soulAvailable || stateUpdating}
          >
            <Play size={18} aria-hidden="true" />
          </button>
        </>
      )}
    </form>
  );
}

function getSelectedVariantIndex(variants: AssistantMessageVariant[]) {
  const index = variants.findIndex((variant) => variant.is_selected);
  return index >= 0 ? index : 0;
}
