import type { ReactNode } from "react";
import { ChevronDown, Clipboard, FolderOpen, RefreshCcw, Settings as SettingsIcon } from "lucide-react";
import type {
  ChatStartMode,
  GenerationPreferences,
  GenerationPresetName,
  ReadingPreferences,
} from "../../settings/preferences";

type GenerationSamplerKey = "temperature" | "topP" | "frequencyPenalty" | "presencePenalty";

function clampNumber(value: number, min: number, max: number, fallback = min) {
  if (!Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, value));
}

export function GenerationSettingsPanel({
  busy,
  generationPreferences,
  onApplyPreset,
  onReset,
  onSetMaxTokens,
  onSetContextMaxTokens,
  onUpdateSampler,
  providerModeControls,
}: {
  busy: boolean;
  generationPreferences: GenerationPreferences;
  onApplyPreset: (preset: GenerationPresetName) => void;
  onReset: () => void;
  onSetMaxTokens: (value: number | null) => void;
  onSetContextMaxTokens: (value: number | null) => void;
  onUpdateSampler: (key: GenerationSamplerKey, value: number) => void;
  providerModeControls: ReactNode;
}) {
  return (
    <div className="settings-tab-panel">
      <section className="settings-section">
        <div className="settings-section-heading">
          <div>
            <span className="eyebrow">Narration</span>
            <h3>Response Direction</h3>
          </div>
        </div>
        <p className="settings-note">
          Presets change generation behavior without changing provider credentials or model selection.
        </p>
        <div className="settings-grid">
          <label className="field">
            <span>Generation Preset</span>
            <select
              value={generationPreferences.preset}
              onChange={(event) => onApplyPreset(event.target.value as GenerationPresetName)}
              disabled={busy}
            >
              <option value="balanced">Balanced</option>
              <option value="creative">Creative</option>
              <option value="focused">Focused</option>
              <option value="custom">Custom</option>
            </select>
          </label>
        </div>
        {providerModeControls}
      </section>

      <section className="settings-section">
        <div className="settings-section-heading">
          <div>
            <span className="eyebrow">Output</span>
            <h3>Creativity &amp; Length</h3>
          </div>
          <button type="button" className="ghost-action compact-ghost" onClick={onReset} disabled={busy}>
            <RefreshCcw size={14} aria-hidden="true" />
            <span>Reset</span>
          </button>
        </div>
        <div className="generation-control-grid">
          <label className="generation-control">
            <span>
              <strong>Creativity</strong>
              <small>Higher values make the narrator less predictable.</small>
            </span>
            <span className="generation-control-inputs">
              <input
                type="range"
                min="0"
                max="2"
                step="0.05"
                value={generationPreferences.temperature}
                onChange={(event) => onUpdateSampler("temperature", Number(event.target.value))}
                disabled={busy}
              />
              <input
                type="number"
                min="0"
                max="2"
                step="0.05"
                aria-label="Creativity value"
                value={generationPreferences.temperature}
                onChange={(event) =>
                  onUpdateSampler("temperature", clampNumber(Number(event.target.value), 0, 2))
                }
                disabled={busy}
              />
            </span>
          </label>
          <div className="generation-control">
            <span>
              <strong>Response Length</strong>
              <small>Leave disabled to use the provider or model default.</small>
            </span>
            <span className="generation-limit-row">
              <label className="toggle-row compact-toggle">
                <input
                  type="checkbox"
                  checked={generationPreferences.maxTokens !== null}
                  onChange={(event) =>
                    onSetMaxTokens(event.target.checked ? generationPreferences.maxTokens ?? 700 : null)
                  }
                  disabled={busy}
                />
                <span>Limit tokens</span>
              </label>
              <input
                className="generation-number-input"
                type="number"
                min="64"
                max="32768"
                step="64"
                aria-label="Maximum response tokens"
                value={generationPreferences.maxTokens ?? ""}
                placeholder="Provider default"
                onChange={(event) =>
                  onSetMaxTokens(
                    event.target.value
                      ? Math.round(clampNumber(Number(event.target.value), 64, 32768))
                      : null,
                  )
                }
                disabled={busy || generationPreferences.maxTokens === null}
              />
            </span>
          </div>
          <div className="field-row">
            <span>
              <strong>State Brief Size</strong>
              <small>
                How much compiled state the engine sends each turn. This is not the
                chat history — it is the memory, scene, and relationship summary.
                Raise it for large-context models; leave disabled for the default.
              </small>
            </span>
            <span className="generation-limit-row">
              <label className="toggle-row compact-toggle">
                <input
                  type="checkbox"
                  checked={generationPreferences.contextMaxTokens !== null}
                  onChange={(event) =>
                    onSetContextMaxTokens(
                      event.target.checked ? generationPreferences.contextMaxTokens ?? 6000 : null,
                    )
                  }
                  disabled={busy}
                />
                <span>Set size</span>
              </label>
              <input
                className="generation-number-input"
                type="number"
                min="1200"
                max="128000"
                step="500"
                aria-label="Maximum compiled state brief tokens"
                value={generationPreferences.contextMaxTokens ?? ""}
                placeholder="Engine default"
                onChange={(event) =>
                  onSetContextMaxTokens(
                    event.target.value
                      ? Math.round(clampNumber(Number(event.target.value), 1200, 128000))
                      : null,
                  )
                }
                disabled={busy || generationPreferences.contextMaxTokens === null}
              />
            </span>
          </div>
        </div>
      </section>

      <details className="settings-disclosure">
        <summary>
          <span>
            <strong>Advanced Sampling</strong>
            <small>OpenAI-compatible controls. Unsupported providers may ignore them.</small>
          </span>
          <ChevronDown size={16} aria-hidden="true" />
        </summary>
        <div className="settings-disclosure-body generation-control-grid">
          {([
            ["topP", "Nucleus sampling", "Restricts choices to the most likely token mass.", 0.01, 1, 0.01],
            ["frequencyPenalty", "Frequency penalty", "Reduces repeated wording based on frequency.", -2, 2, 0.05],
            ["presencePenalty", "Presence penalty", "Encourages introducing concepts not used yet.", -2, 2, 0.05],
          ] as const).map(([key, label, hint, min, max, step]) => (
            <label className="generation-control" key={key}>
              <span>
                <strong>{label}</strong>
                <small>{hint}</small>
              </span>
              <span className="generation-control-inputs">
                <input
                  type="range"
                  min={min}
                  max={max}
                  step={step}
                  value={generationPreferences[key]}
                  onChange={(event) => onUpdateSampler(key, Number(event.target.value))}
                  disabled={busy}
                />
                <input
                  type="number"
                  min={min}
                  max={max}
                  step={step}
                  aria-label={`${label} value`}
                  value={generationPreferences[key]}
                  onChange={(event) =>
                    onUpdateSampler(key, clampNumber(Number(event.target.value), min, max))
                  }
                  disabled={busy}
                />
              </span>
            </label>
          ))}
        </div>
      </details>
    </div>
  );
}

export function ReadingSettingsPanel({
  onChange,
  onReset,
  readingPreferences,
}: {
  onChange: (update: Partial<ReadingPreferences>) => void;
  onReset: () => void;
  readingPreferences: ReadingPreferences;
}) {
  return (
    <div className="settings-tab-panel">
      <section className="settings-section">
        <div className="settings-section-heading">
          <div>
            <span className="eyebrow">Play</span>
            <h3>Reading Surface</h3>
          </div>
          <button type="button" className="ghost-action compact-ghost" onClick={onReset}>
            <RefreshCcw size={14} aria-hidden="true" />
            <span>Reset</span>
          </button>
        </div>
        <p className="settings-note">
          These controls change the prose column in Play without changing generated content.
        </p>
        <div className="generation-control-grid">
          {([
            ["proseSize", "Prose size", "Text size for roleplay messages.", 15, 24, 1, "px"],
            ["lineHeight", "Line height", "Vertical breathing room for long sessions.", 1.4, 2.1, 0.05, ""],
            ["columnWidth", "Reading width", "Maximum width of the Play prose column.", 680, 1080, 20, "px"],
          ] as const).map(([key, label, hint, min, max, step, suffix]) => (
            <label className="generation-control" key={key}>
              <span>
                <strong>{label}</strong>
                <small>{hint}</small>
              </span>
              <span className="generation-control-inputs">
                <input
                  type="range"
                  min={min}
                  max={max}
                  step={step}
                  value={readingPreferences[key]}
                  onChange={(event) => onChange({ [key]: Number(event.target.value) })}
                />
                <output>{readingPreferences[key]}{suffix}</output>
              </span>
            </label>
          ))}
        </div>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={readingPreferences.compactSpacing}
            onChange={(event) => onChange({ compactSpacing: event.target.checked })}
          />
          <span>Use compact message spacing</span>
        </label>
      </section>
      <section className="reading-preview" aria-label="Reading preview">
        <span className="eyebrow">Preview</span>
        <p>
          Rain traces the glass while the room remembers what the characters would rather leave unsaid.
        </p>
      </section>
    </div>
  );
}

export function DataSettingsPanel({
  busy,
  chatStartMode,
  onChatStartModeChange,
  onOpenDataFolder,
  onShowArchivedSessionsChange,
  showArchivedSessions,
}: {
  busy: boolean;
  chatStartMode: ChatStartMode;
  onChatStartModeChange: (mode: ChatStartMode) => void;
  onOpenDataFolder: () => void;
  onShowArchivedSessionsChange: (show: boolean) => void;
  showArchivedSessions: boolean;
}) {
  return (
    <div className="settings-tab-panel">
      <section className="settings-section">
        <div className="settings-section-heading">
          <div>
            <span className="eyebrow">Sessions</span>
            <h3>Session Defaults</h3>
          </div>
        </div>
        <div className="chat-start-options settings-radio-group" role="radiogroup" aria-label="Default session start mode">
          <label>
            <input
              type="radio"
              name="settings-chat-start-mode"
              value="continue"
              checked={chatStartMode === "continue"}
              onChange={() => onChatStartModeChange("continue")}
              disabled={busy}
            />
            <span>Continue Soul continuity</span>
          </label>
          <label>
            <input
              type="radio"
              name="settings-chat-start-mode"
              value="fresh"
              checked={chatStartMode === "fresh"}
              onChange={() => onChatStartModeChange("fresh")}
              disabled={busy}
            />
            <span>New isolated Session</span>
          </label>
        </div>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={showArchivedSessions}
            onChange={(event) => onShowArchivedSessionsChange(event.target.checked)}
          />
          <span>Show archived sessions by default</span>
        </label>
      </section>
      <section className="settings-section">
        <div className="settings-section-heading">
          <div>
            <span className="eyebrow">Data</span>
            <h3>Storage &amp; Imports</h3>
          </div>
        </div>
        <p className="settings-note">
          Session data lives locally. Import, export, and archive workflows stay in Library and Dev Mode so this page remains a clean preferences surface.
        </p>
        <div className="button-row">
          <button type="button" className="ghost-action" onClick={onOpenDataFolder} disabled={busy}>
            <FolderOpen size={16} aria-hidden="true" />
            <span>Open Data Folder</span>
          </button>
        </div>
      </section>
    </div>
  );
}

export function AboutSettingsPanel({ onViewDisclaimer }: { onViewDisclaimer: () => void }) {
  return (
    <div className="settings-tab-panel">
      <section className="settings-section">
        <div className="settings-section-heading">
          <div>
            <span className="eyebrow">About</span>
            <h3>Mnemosyne</h3>
          </div>
        </div>
        <p className="settings-note">
          Mnemosyne is an experimental AI roleplay state engine. Human-facing surfaces stay paper/editorial; raw machine inspection stays in terminal Dev Mode.
        </p>
        <div className="button-row">
          <button type="button" className="ghost-action" onClick={onViewDisclaimer}>
            <Clipboard size={16} aria-hidden="true" />
            <span>View Disclaimer</span>
          </button>
        </div>
      </section>
    </div>
  );
}

export function QuickSettingsPanel({
  busy,
  generationPreferences,
  onApplyPreset,
  onOpenFullSettings,
  onSetMaxTokens,
  onUpdateSampler,
  providerModeControls,
}: {
  busy: boolean;
  generationPreferences: GenerationPreferences;
  onApplyPreset: (preset: GenerationPresetName) => void;
  onOpenFullSettings: () => void;
  onSetMaxTokens: (value: number | null) => void;
  onUpdateSampler: (key: GenerationSamplerKey, value: number) => void;
  providerModeControls: ReactNode;
}) {
  return (
    <div className="settings-quick-panel">
      <section className="settings-quick-section">
        <div className="settings-section-heading">
          <div>
            <span className="eyebrow">This Session</span>
            <h3>Narration</h3>
          </div>
        </div>
        {providerModeControls}
      </section>
      <section className="settings-quick-section">
        <div className="settings-section-heading">
          <div>
            <span className="eyebrow">Generation</span>
            <h3>Response Shape</h3>
          </div>
        </div>
        <label className="field">
          <span>Preset</span>
          <select
            value={generationPreferences.preset}
            onChange={(event) => onApplyPreset(event.target.value as GenerationPresetName)}
            disabled={busy}
          >
            <option value="balanced">Balanced</option>
            <option value="creative">Creative</option>
            <option value="focused">Focused</option>
            <option value="custom">Custom</option>
          </select>
        </label>
        <label className="generation-control quick-generation-control">
          <span>
            <strong>Creativity</strong>
            <small>Predictable at the left, more varied at the right.</small>
          </span>
          <span className="generation-control-inputs">
            <input
              type="range"
              min="0"
              max="2"
              step="0.05"
              value={generationPreferences.temperature}
              onChange={(event) => onUpdateSampler("temperature", Number(event.target.value))}
              disabled={busy}
            />
            <output>{generationPreferences.temperature.toFixed(2)}</output>
          </span>
        </label>
        <div className="generation-limit-row">
          <label className="toggle-row compact-toggle">
            <input
              type="checkbox"
              checked={generationPreferences.maxTokens !== null}
              onChange={(event) =>
                onSetMaxTokens(event.target.checked ? generationPreferences.maxTokens ?? 700 : null)
              }
              disabled={busy}
            />
            <span>Limit response length</span>
          </label>
          <input
            className="generation-number-input"
            type="number"
            min="64"
            max="32768"
            step="64"
            aria-label="Maximum response tokens"
            value={generationPreferences.maxTokens ?? ""}
            placeholder="Provider default"
            onChange={(event) =>
              onSetMaxTokens(
                event.target.value
                  ? Math.round(clampNumber(Number(event.target.value), 64, 32768))
                  : null,
              )
            }
            disabled={busy || generationPreferences.maxTokens === null}
          />
        </div>
      </section>
      <button type="button" className="ghost-action settings-open-full" onClick={onOpenFullSettings}>
        <SettingsIcon size={16} aria-hidden="true" />
        <span>Open Full Settings</span>
      </button>
    </div>
  );
}
