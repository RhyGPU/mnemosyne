import { Archive, RefreshCcw, Save } from "lucide-react";
import type {
  ApiProviderSettings,
  EmbeddedModelStatus,
  ProviderProfile,
} from "../../tauri";
import type { NarrativeMode } from "../../settings/preferences";
import { DEFAULT_EVALUATOR_TIMEOUT_MS } from "../../app/preferencesStorage";
import {
  REPAIR_MODEL_AUTO,
  REPAIR_MODEL_EMBEDDED,
  REPAIR_MODEL_EVALUATOR,
} from "../../settings/provider";

export function NarratorProviderPanel({
  apiSettings,
  busy,
  knownModels,
  mode,
  onArchive,
  onNameChange,
  onRememberModel,
  onSave,
  onSelectProfile,
  onSettingsChange,
  profileName,
  profiles,
  selectedProfileId,
}: {
  apiSettings: ApiProviderSettings;
  busy: boolean;
  knownModels: string[];
  mode: NarrativeMode;
  onArchive: () => void;
  onNameChange: (name: string) => void;
  onRememberModel: (model: string) => void;
  onSave: () => void;
  onSelectProfile: (profileId: string) => void;
  onSettingsChange: (update: Partial<ApiProviderSettings>) => void;
  profileName: string;
  profiles: ProviderProfile[];
  selectedProfileId: string;
}) {
  return (
    <section className="settings-section provider-pass-card">
      <div className="provider-pass-heading">
        <div>
          <h3>Narrator Provider</h3>
          <p>Narrator pass: writes visible RP response.</p>
        </div>
        <span className="provider-status-pill">{apiSettings.model || "No model"}</span>
      </div>
      <div className="provider-pass-grid">
        <label className="field">
          <span>Narrator Provider</span>
          <select
            value={selectedProfileId}
            onChange={(event) => onSelectProfile(event.target.value)}
            disabled={busy}
          >
            <option value="">Unsaved narrator profile</option>
            {profiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Profile Name</span>
          <input
            value={profileName}
            onChange={(event) => onNameChange(event.target.value)}
            placeholder="Narrator API"
            disabled={busy}
          />
        </label>
        <label className="field">
          <span>Base URL</span>
          <input
            value={apiSettings.base_url}
            onChange={(event) => onSettingsChange({ base_url: event.target.value })}
            placeholder="https://api.openai.com/v1"
            disabled={busy}
          />
        </label>
        <label className="field">
          <span>Model</span>
          <input
            value={apiSettings.model}
            list="mnemosyne-models"
            onChange={(event) => onSettingsChange({ model: event.target.value })}
            onBlur={(event) => onRememberModel(event.target.value)}
            placeholder="Type or pick a model"
            disabled={busy}
          />
          <datalist id="mnemosyne-models">
            {knownModels.map((model) => (
              <option key={model} value={model} />
            ))}
          </datalist>
          <small className="field-hint">
            {knownModels.length} saved - type a new id and it is remembered
          </small>
        </label>
        <label className="field">
          <span>Narrator Timeout (seconds)</span>
          <input
            type="number"
            min="0"
            value={
              apiSettings.narrator_timeout_ms
                ? Math.round(apiSettings.narrator_timeout_ms / 1000)
                : ""
            }
            onChange={(event) =>
              onSettingsChange({
                narrator_timeout_ms:
                  Number(event.target.value) > 0
                    ? Number(event.target.value) * 1000
                    : null,
              })
            }
            placeholder="None / provider default"
            disabled={busy}
          />
        </label>
        <label className="field">
          <span>API Key</span>
          <input
            type="password"
            value={apiSettings.api_key}
            onChange={(event) => onSettingsChange({ api_key: event.target.value })}
            placeholder="Stored locally with profile"
            disabled={busy}
          />
        </label>
        {mode === "Custom" ? (
          <label className="field custom-prompt-field">
            <span>Custom Narrator Prompt</span>
            <textarea
              value={apiSettings.system_prompt}
              onChange={(event) => onSettingsChange({ system_prompt: event.target.value })}
              placeholder="Replaces default narrator and mode prompts when filled. Leave empty for default Reader narration."
              disabled={busy}
            />
          </label>
        ) : null}
      </div>
      <div className="button-row">
        <button type="button" className="ghost-action" onClick={onSave} disabled={busy}>
          <Save size={16} aria-hidden="true" />
          <span>Save Narrator Profile</span>
        </button>
        <button
          type="button"
          className="ghost-action"
          onClick={onArchive}
          disabled={busy || !selectedProfileId}
        >
          <Archive size={16} aria-hidden="true" />
          <span>Archive Profile</span>
        </button>
      </div>
    </section>
  );
}

export function StateUpdaterProviderPanel({
  apiSettings,
  busy,
  effectiveSettings,
  evaluatorExecutionMode,
  onArchive,
  onExecutionModeChange,
  onNameChange,
  onRememberModel,
  onSave,
  onSelectProfile,
  onSettingsChange,
  onToggleUseNarrator,
  onUpdaterSettingsChange,
  profileName,
  profiles,
  selectedProfileId,
  stateUpdaterSettings,
  useNarratorProvider,
}: {
  apiSettings: ApiProviderSettings;
  busy: boolean;
  effectiveSettings: ApiProviderSettings;
  evaluatorExecutionMode: string;
  onArchive: () => void;
  onExecutionModeChange: (mode: string) => void;
  onNameChange: (name: string) => void;
  onRememberModel: (model: string) => void;
  onSave: () => void;
  onSelectProfile: (profileId: string) => void;
  onSettingsChange: (update: Partial<ApiProviderSettings>) => void;
  onToggleUseNarrator: (useNarrator: boolean) => void;
  onUpdaterSettingsChange: (update: Partial<ApiProviderSettings>) => void;
  profileName: string;
  profiles: ProviderProfile[];
  selectedProfileId: string;
  stateUpdaterSettings: ApiProviderSettings;
  useNarratorProvider: boolean;
}) {
  return (
    <section className="settings-section provider-pass-card">
      <div className="provider-pass-heading">
        <div>
          <h3>State Updater Provider</h3>
          <p>State updater pass: updates Soul, World, and Memory.</p>
        </div>
        <span className="provider-status-pill">
          {useNarratorProvider
            ? "Using narrator provider"
            : stateUpdaterSettings.model || "No model"}
        </span>
      </div>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={useNarratorProvider}
          onChange={(event) => onToggleUseNarrator(event.target.checked)}
          disabled={busy}
        />
        <span>Use narrator provider for state updater</span>
      </label>
      <div className="provider-pass-grid">
        <label className="field">
          <span>State Updater Timeout (seconds)</span>
          <input
            type="number"
            min="0"
            value={Math.round((effectiveSettings.evaluator_timeout_ms ?? DEFAULT_EVALUATOR_TIMEOUT_MS) / 1000)}
            onChange={(event) =>
              onSettingsChange({
                evaluator_timeout_ms:
                  Math.max(0, Number(event.target.value) || 0) * 1000,
              })
            }
            disabled={busy}
          />
        </label>
        <label className="field">
          <span>Timeout Mode</span>
          <select
            value={effectiveSettings.evaluator_timeout_mode ?? "finite"}
            onChange={(event) =>
              onSettingsChange({ evaluator_timeout_mode: event.target.value })
            }
            disabled={busy}
          >
            <option value="finite">Finite app timeout</option>
            <option value="no_app_timeout">No app timeout</option>
          </select>
        </label>
        <label className="field">
          <span>Execution Mode</span>
          <select
            value={evaluatorExecutionMode}
            onChange={(event) => onExecutionModeChange(event.target.value)}
            disabled={busy}
          >
            <option value="balanced">Balanced - evaluate every turn</option>
            <option value="fast">Fast - skip dialogue-only turns, catch up later</option>
            <option value="long_context">Long Context - evaluate every turn</option>
          </select>
        </label>
      </div>
      {useNarratorProvider ? (
        <p className="provider-note">
          Using narrator provider: {apiSettings.base_url || "No base URL"} /{" "}
          {apiSettings.model || "No model"}
        </p>
      ) : (
        <>
          <div className="provider-pass-grid">
            <label className="field">
              <span>State Updater Provider</span>
              <select
                value={selectedProfileId}
                onChange={(event) => onSelectProfile(event.target.value)}
                disabled={busy}
              >
                <option value="">Unsaved updater profile</option>
                {profiles.map((profile) => (
                  <option key={profile.id} value={profile.id}>
                    {profile.name}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>Profile Name</span>
              <input
                value={profileName}
                onChange={(event) => onNameChange(event.target.value)}
                placeholder="Updater API"
                disabled={busy}
              />
            </label>
            <label className="field">
              <span>Base URL</span>
              <input
                value={stateUpdaterSettings.base_url}
                onChange={(event) =>
                  onUpdaterSettingsChange({ base_url: event.target.value })
                }
                placeholder="http://localhost:11434/v1"
                disabled={busy}
              />
            </label>
            <label className="field">
              <span>Model</span>
              <input
                value={stateUpdaterSettings.model}
                list="mnemosyne-models"
                onChange={(event) =>
                  onUpdaterSettingsChange({ model: event.target.value })
                }
                onBlur={(event) => onRememberModel(event.target.value)}
                placeholder="Type or pick a model"
                disabled={busy}
              />
            </label>
            <label className="field">
              <span>API Key</span>
              <input
                type="password"
                value={stateUpdaterSettings.api_key}
                onChange={(event) =>
                  onUpdaterSettingsChange({ api_key: event.target.value })
                }
                placeholder="Stored locally with profile"
                disabled={busy}
              />
            </label>
          </div>
          <div className="button-row">
            <button type="button" className="ghost-action" onClick={onSave} disabled={busy}>
              <Save size={16} aria-hidden="true" />
              <span>Save Updater Profile</span>
            </button>
            <button
              type="button"
              className="ghost-action"
              onClick={onArchive}
              disabled={busy || !selectedProfileId}
            >
              <Archive size={16} aria-hidden="true" />
              <span>Archive Profile</span>
            </button>
          </div>
        </>
      )}
    </section>
  );
}

export function RepairModelPanel({
  busy,
  embeddedModel,
  embeddedModelError,
  embeddedModelPath,
  onEmbeddedModelPathChange,
  onSelectProfile,
  onStart,
  onStop,
  profiles,
  selectedProfileId,
}: {
  busy: boolean;
  embeddedModel: EmbeddedModelStatus;
  embeddedModelError: string | null;
  embeddedModelPath: string;
  onEmbeddedModelPathChange: (path: string) => void;
  onSelectProfile: (profileId: string) => void;
  onStart: () => void;
  onStop: () => void;
  profiles: ProviderProfile[];
  selectedProfileId: string;
}) {
  const status =
    selectedProfileId === REPAIR_MODEL_EMBEDDED
      ? embeddedModel.ready
        ? "Embedded local ready"
        : "Embedded local not ready"
      : selectedProfileId === REPAIR_MODEL_EVALUATOR
        ? "Same as evaluator"
        : selectedProfileId
          ? profiles.find((profile) => profile.id === selectedProfileId)?.name ??
            "Profile missing"
          : embeddedModel.ready
            ? "Auto: embedded local"
            : "Auto: evaluator";

  return (
    <section className="settings-section provider-pass-card">
      <div className="provider-pass-heading">
        <div>
          <h3>Repair Model</h3>
          <p>Focused background repair for evaluator operations rejected by validation.</p>
        </div>
        <span className="provider-status-pill">{status}</span>
      </div>
      <div className="provider-pass-grid">
        <label className="field">
          <span>Repair Model (light/local)</span>
          <select
            value={selectedProfileId}
            onChange={(event) => onSelectProfile(event.target.value)}
            disabled={busy}
          >
            <option value={REPAIR_MODEL_AUTO}>
              Automatic (local when ready, otherwise evaluator)
            </option>
            <option value={REPAIR_MODEL_EMBEDDED}>Embedded local model</option>
            <option value={REPAIR_MODEL_EVALUATOR}>Same as evaluator</option>
            {profiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Embedded model file (llamafile path)</span>
          <input
            value={embeddedModelPath}
            onChange={(event) => onEmbeddedModelPathChange(event.target.value)}
            placeholder="C:\path\to\your-model.llamafile.exe"
            disabled={busy}
          />
        </label>
      </div>
      <div className="button-row">
        <button
          type="button"
          className="ghost-action"
          onClick={onStart}
          disabled={busy || embeddedModel.running}
        >
          <span>
            {embeddedModel.running
              ? embeddedModel.ready
                ? "Embedded model running"
                : "Starting..."
              : "Start embedded model"}
          </span>
        </button>
        <button
          type="button"
          className="ghost-action"
          onClick={onStop}
          disabled={busy || !embeddedModel.running}
        >
          <span>Stop</span>
        </button>
      </div>
      <p className="provider-note">
        {embeddedModel.ready
          ? `Embedded model ready at ${embeddedModel.url} - repair uses it automatically when no profile is selected above.`
          : embeddedModel.running
            ? "Embedded model loading - large models can take a minute."
            : "Choose a local llamafile, enter its full path, and start it. Repair uses it automatically."}
        {embeddedModelError ? ` - ${embeddedModelError}` : ""}
      </p>
      <p className="provider-note">
        Pick a saved local or light profile above to point repair at your own endpoint instead.
      </p>
    </section>
  );
}

export function ProviderProfilesPanel({
  archivedProfiles,
  busy,
  onArchive,
  onRestore,
  profiles,
  selectedNarratorProfileId,
  selectedRepairProfileId,
  selectedUpdaterProfileId,
}: {
  archivedProfiles: ProviderProfile[];
  busy: boolean;
  onArchive: (profileId: string) => void;
  onRestore: (profileId: string) => void;
  profiles: ProviderProfile[];
  selectedNarratorProfileId: string;
  selectedRepairProfileId: string;
  selectedUpdaterProfileId: string;
}) {
  return (
    <section className="settings-section provider-pass-card">
      <div className="settings-section-heading">
        <div>
          <span className="eyebrow">Profiles</span>
          <h3>Saved Provider Profiles</h3>
        </div>
      </div>
      {profiles.length === 0 && archivedProfiles.length === 0 ? (
        <p className="settings-note">No profiles saved yet.</p>
      ) : (
        <div className="provider-profiles-list">
          {profiles.map((profile) => {
            const narratorActive = selectedNarratorProfileId === profile.id;
            const updaterActive = selectedUpdaterProfileId === profile.id;
            const repairActive = selectedRepairProfileId === profile.id;
            const active = narratorActive || updaterActive || repairActive;
            return (
              <div key={profile.id} className="profile-list-item">
                <div className="profile-list-identity">
                  <strong>{profile.name}</strong>
                  <span className="profile-list-model">({profile.model})</span>
                  {narratorActive ? (
                    <span className="provider-status-pill compact-status-pill">
                      Active Narrator
                    </span>
                  ) : null}
                  {updaterActive ? (
                    <span className="provider-status-pill compact-status-pill">
                      Active Updater
                    </span>
                  ) : null}
                  {repairActive ? (
                    <span className="provider-status-pill compact-status-pill">
                      Active Repair
                    </span>
                  ) : null}
                </div>
                <button
                  type="button"
                  className="ghost-action compact-ghost"
                  onClick={() => onArchive(profile.id)}
                  disabled={busy || active}
                  title={active ? "Cannot archive active profile" : "Archive profile"}
                >
                  <Archive size={12} aria-hidden="true" />
                  <span>Archive</span>
                </button>
              </div>
            );
          })}
          {archivedProfiles.length > 0 ? (
            <div className="archived-profile-group">
              <span className="eyebrow">Archived</span>
              <h4>Archived Profiles</h4>
              {archivedProfiles.map((profile) => (
                <div key={profile.id} className="profile-list-item archived">
                  <div className="profile-list-identity">
                    <span className="profile-list-name">{profile.name}</span>
                    <span className="profile-list-model">({profile.model})</span>
                  </div>
                  <button
                    type="button"
                    className="ghost-action compact-ghost"
                    onClick={() => onRestore(profile.id)}
                    disabled={busy}
                  >
                    <RefreshCcw size={12} aria-hidden="true" />
                    <span>Restore</span>
                  </button>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      )}
    </section>
  );
}
