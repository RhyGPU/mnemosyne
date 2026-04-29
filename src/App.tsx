import {
  Brain,
  Database,
  FileDown,
  FileUp,
  MessageSquareText,
  MessageSquareX,
  Play,
  RefreshCcw,
  Save,
  Sparkles,
  Trash2,
} from "lucide-react";
import { ChangeEvent, FormEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  ApiProviderSettings,
  ChatMessage,
  ContextPreview,
  Soul,
  SoulSummary,
  compileContext,
  createDefaultSoul,
  deleteConversation,
  deleteSoul,
  getSoul,
  listConversationMessages,
  listSouls,
  runConsolidation,
  saveSoulFile,
  sendApiTurn,
  sendMockTurn,
  upsertSoul,
} from "./tauri";

const DEFAULT_CONVERSATION_ID = "local-mock";
const CONSOLIDATION_INTERVAL_TURNS = 10;
type ProviderKind = "Mock" | "API";
type NarrativeMode = "Realistic" | "Reader" | "God" | "Custom";
type PsychePresetName =
  | "Stranger"
  | "Traumatized Survivor"
  | "Trusting Friend"
  | "Devoted Partner"
  | "Hostile Rival"
  | "Custom";

type PsycheDraft = {
  global: {
    fear_baseline: number;
    resolve: number;
    shame: number;
    openness: number;
  };
  maslow: [number, number, number, number, number];
  sdt: [number, number, number];
  trauma: {
    phase: number;
    hypervigilance: number;
    flashbacks: number;
    numbing: number;
    avoidance: number;
  };
  relationship: {
    trust: number;
    affection: number;
    intimacy: number;
    passion: number;
    commitment: number;
    fear: number;
    desire: number;
  };
};

const PSYCHE_PRESETS: Record<PsychePresetName, PsycheDraft> = {
  Stranger: {
    global: { fear_baseline: 35, resolve: 40, shame: 35, openness: 35 },
    maslow: [70, 55, 35, 35, 20],
    sdt: [55, 45, 25],
    trauma: { phase: 1, hypervigilance: 30, flashbacks: 15, numbing: 20, avoidance: 35 },
    relationship: { trust: 0, affection: 0, intimacy: 0, passion: 0, commitment: 0, fear: 20, desire: 0 },
  },
  "Traumatized Survivor": {
    global: { fear_baseline: 75, resolve: 55, shame: 60, openness: 25 },
    maslow: [45, 20, 25, 20, 10],
    sdt: [25, 30, 15],
    trauma: { phase: 2, hypervigilance: 80, flashbacks: 65, numbing: 55, avoidance: 70 },
    relationship: { trust: -35, affection: -5, intimacy: 0, passion: 0, commitment: 0, fear: 70, desire: -10 },
  },
  "Trusting Friend": {
    global: { fear_baseline: 20, resolve: 55, shame: 25, openness: 70 },
    maslow: [75, 70, 80, 60, 35],
    sdt: [70, 60, 75],
    trauma: { phase: 3, hypervigilance: 20, flashbacks: 10, numbing: 15, avoidance: 20 },
    relationship: { trust: 55, affection: 60, intimacy: 35, passion: 5, commitment: 30, fear: 5, desire: 10 },
  },
  "Devoted Partner": {
    global: { fear_baseline: 15, resolve: 65, shame: 20, openness: 80 },
    maslow: [80, 75, 90, 70, 45],
    sdt: [75, 65, 90],
    trauma: { phase: 4, hypervigilance: 15, flashbacks: 5, numbing: 10, avoidance: 10 },
    relationship: { trust: 85, affection: 90, intimacy: 85, passion: 70, commitment: 90, fear: 0, desire: 75 },
  },
  "Hostile Rival": {
    global: { fear_baseline: 45, resolve: 80, shame: 20, openness: 10 },
    maslow: [70, 60, 15, 55, 25],
    sdt: [80, 70, 10],
    trauma: { phase: 1, hypervigilance: 55, flashbacks: 10, numbing: 35, avoidance: 60 },
    relationship: { trust: -80, affection: -65, intimacy: -50, passion: 0, commitment: -40, fear: 45, desire: -30 },
  },
  Custom: {
    global: { fear_baseline: 15, resolve: 40, shame: 45, openness: 45 },
    maslow: [60, 50, 40, 30, 20],
    sdt: [70, 40, 10],
    trauma: { phase: 2, hypervigilance: 10, flashbacks: 10, numbing: 10, avoidance: 10 },
    relationship: { trust: 10, affection: 20, intimacy: 10, passion: 10, commitment: 10, fear: 10, desire: 20 },
  },
};

export function App() {
  const [souls, setSouls] = useState<SoulSummary[]>([]);
  const [soul, setSoul] = useState<Soul | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [context, setContext] = useState<ContextPreview | null>(null);
  const [draft, setDraft] = useState("");
  const [characterName, setCharacterName] = useState("Aurora Schwarz");
  const [characterDescription, setCharacterDescription] = useState("");
  const [characterAppearance, setCharacterAppearance] = useState("");
  const [characterPersonality, setCharacterPersonality] = useState("");
  const [characterSetting, setCharacterSetting] = useState("Unspecified starting scene.");
  const [psychePreset, setPsychePreset] = useState<PsychePresetName>("Custom");
  const [psyche, setPsyche] = useState<PsycheDraft>(PSYCHE_PRESETS.Custom);
  const [provider, setProvider] = useState<ProviderKind>("Mock");
  const [mode, setMode] = useState<NarrativeMode>("Reader");
  const [apiSettings, setApiSettings] = useState<ApiProviderSettings>({
    base_url: "https://api.openai.com/v1",
    api_key: "",
    model: "",
    system_prompt: "",
  });
  const [status, setStatus] = useState("Ready");
  const [busy, setBusy] = useState(false);
  const didBootstrap = useRef(false);
  const importInputRef = useRef<HTMLInputElement>(null);
  const currentConversationId = useMemo(
    () => (soul ? conversationIdForSoul(soul.character_id) : DEFAULT_CONVERSATION_ID),
    [soul?.character_id],
  );

  useEffect(() => {
    if (didBootstrap.current) return;
    didBootstrap.current = true;
    void bootstrap();
  }, []);

  useEffect(() => {
    if (!soul) return;
    void refreshContext(soul.character_id, currentConversationId);
  }, [soul?.character_id, currentConversationId, messages.length]);

  function setCreatorFieldsFromSoul(nextSoul: Soul) {
    setCharacterName(nextSoul.character_name);
    setCharacterDescription(nextSoul.profile.description);
    setCharacterAppearance(nextSoul.profile.appearance);
    setCharacterPersonality(nextSoul.profile.personality);
    setCharacterSetting(nextSoul.world.location);
    setPsyche(psycheFromSoul(nextSoul));
    setPsychePreset("Custom");
  }

  function updatePsyche(update: (current: PsycheDraft) => PsycheDraft) {
    setPsychePreset("Custom");
    setPsyche((current) => update(current));
  }

  function handlePresetChange(nextPreset: PsychePresetName) {
    setPsychePreset(nextPreset);
    setPsyche(PSYCHE_PRESETS[nextPreset]);
  }

  function applyCreatorFields(nextSoul: Soul) {
    const name = characterName.trim() || "Unnamed Character";
    const description = characterDescription.trim();
    const appearance = characterAppearance.trim();
    const personality = characterPersonality.trim();
    const setting = characterSetting.trim() || "Unspecified starting scene.";
    const core = [...nextSoul.memory.core];
    for (const memory of [
      description ? `Profile: ${description}` : "",
      appearance ? `Appearance: ${appearance}` : "",
      personality ? `Personality: ${personality}` : "",
    ].filter(Boolean)) {
      if (!core.includes(memory)) core.push(memory);
    }

    return {
      ...nextSoul,
      character_name: name,
      profile: {
        description,
        appearance,
        personality,
        scenario: setting,
      },
      global: {
        ...nextSoul.global,
        fear_baseline: psyche.global.fear_baseline,
        resolve: psyche.global.resolve,
        shame: psyche.global.shame,
        openness: psyche.global.openness,
        maslow: psyche.maslow,
        sdt: psyche.sdt,
      },
      trauma: {
        phase: psyche.trauma.phase,
        symptoms: {
          hypervigilance: psyche.trauma.hypervigilance,
          flashbacks: psyche.trauma.flashbacks,
          numbing: psyche.trauma.numbing,
          avoidance: psyche.trauma.avoidance,
        },
      },
      relationships: {
        ...nextSoul.relationships,
        user: {
          ...(nextSoul.relationships.user ?? {
            trust: 0,
            affection: 0,
            intimacy: 0,
            passion: 0,
            commitment: 0,
            fear: 0,
            desire: 0,
            love_type: "",
          }),
          trust: psyche.relationship.trust,
          affection: psyche.relationship.affection,
          intimacy: psyche.relationship.intimacy,
          passion: psyche.relationship.passion,
          commitment: psyche.relationship.commitment,
          fear: psyche.relationship.fear,
          desire: psyche.relationship.desire,
        },
      },
      memory: {
        ...nextSoul.memory,
        core,
      },
      world: {
        ...nextSoul.world,
        location: setting,
        active_plots: nextSoul.world.active_plots.length
          ? nextSoul.world.active_plots
          : ["Establish the first scene"],
      },
    };
  }

  async function bootstrap() {
    const existing = await listSouls();
    setSouls(existing);

    if (existing.length > 0) {
      const firstSoul = await getSoul(existing[0].character_id);
      setSoul(firstSoul);
      setCreatorFieldsFromSoul(firstSoul);
      setMessages(await listConversationMessages(conversationIdForSoul(firstSoul.character_id)));
      setStatus("Loaded local Soul index");
      return;
    }

    const nextSoul = await createDefaultSoul(characterName);
    await upsertSoul(nextSoul);
    setSoul(nextSoul);
    setSouls(await listSouls());
    setStatus("Created starter Soul");
  }

  async function refreshContext(soulId: string, conversationId: string) {
    const preview = await compileContext(soulId, conversationId);
    setContext(preview);
  }

  async function handleCreateSoul() {
    setBusy(true);
    try {
      const nextSoul = applyCreatorFields(
        await createDefaultSoul(characterName || "Unnamed Character"),
      );
      await upsertSoul(nextSoul);
      setSoul(nextSoul);
      setMessages([]);
      setSouls(await listSouls());
      setStatus("New Soul created");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSelectSoul(characterId: string) {
    const selected = souls.find((item) => item.character_id === characterId);
    if (!selected) return;

    setBusy(true);
    try {
      const nextSoul = await getSoul(selected.character_id);
      setStatus(`Selected ${nextSoul.character_name}`);
      setSoul(nextSoul);
      setCreatorFieldsFromSoul(nextSoul);
      setMessages(await listConversationMessages(conversationIdForSoul(nextSoul.character_id)));
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const text = draft.trim();
    if (!text || busy || !soul) return;

    setBusy(true);
    setDraft("");
    setStatus(provider === "API" ? "API provider thinking" : "Mock provider thinking");

    try {
      const result =
        provider === "API"
          ? await sendApiTurn(currentConversationId, soul.character_id, text, mode, apiSettings)
          : await sendMockTurn(currentConversationId, soul.character_id, text, mode);
      setSoul(result.soul);
      setMessages(result.messages);
      setContext(result.context_preview);
      setSouls(await listSouls());
      setStatus(result.consolidation_ran ? "Turn saved; consolidation ran" : "Turn saved");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleConsolidate() {
    if (!soul) return;
    setBusy(true);
    try {
      const nextSoul = await runConsolidation(soul.character_id);
      setSoul(nextSoul);
      setSouls(await listSouls());
      setContext(await compileContext(nextSoul.character_id, currentConversationId));
      setStatus("Memory consolidated");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteChat() {
    if (!soul) return;
    const confirmed = window.confirm(
      "Delete this local chat? Messages will be removed, but Soul memory and stats will remain.",
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await deleteConversation(currentConversationId);
      setMessages([]);
      setContext(await compileContext(soul.character_id, currentConversationId));
      setStatus("Chat deleted; Soul memory kept");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteSoul() {
    if (!soul) return;
    const confirmed = window.confirm(
      `Delete ${soul.character_name} and all local chats for this Soul? This cannot be undone.`,
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await deleteSoul(soul.character_id);
      const remaining = await listSouls();
      setSouls(remaining);

      if (remaining.length === 0) {
        setSoul(null);
        setMessages([]);
        setContext(null);
        setStatus("Soul deleted");
        return;
      }

      const nextSoul = await getSoul(remaining[0].character_id);
      const nextConversationId = conversationIdForSoul(nextSoul.character_id);
      setSoul(nextSoul);
      setCreatorFieldsFromSoul(nextSoul);
      setMessages(await listConversationMessages(nextConversationId));
      setContext(await compileContext(nextSoul.character_id, nextConversationId));
      setStatus("Soul deleted; selected next local Soul");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveSoul() {
    if (!soul) return;
    setBusy(true);
    try {
      const nextSoul = applyCreatorFields(soul);
      await upsertSoul(nextSoul);
      setSoul(nextSoul);
      await saveSoulFile(`${nextSoul.character_name.replace(/\s+/g, "_")}.soul.json`, nextSoul);
      setSouls(await listSouls());
      setStatus("Soul exported beside the app working directory");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleImportSoulFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;

    setBusy(true);
    try {
      const raw = JSON.parse(await file.text());
      const importedSoul = await soulFromImport(raw, file.name);
      await upsertSoul(importedSoul);
      setSoul(importedSoul);
      setCreatorFieldsFromSoul(importedSoul);
      setMessages(await listConversationMessages(conversationIdForSoul(importedSoul.character_id)));
      setSouls(await listSouls());
      setStatus(`Imported ${importedSoul.character_name}`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  const relationship = soul?.relationships.user;
  const turnsSinceConsolidation = soul?.turns_since_consolidation ?? 0;
  const turnsUntilConsolidation = soul
    ? Math.max(0, CONSOLIDATION_INTERVAL_TURNS - turnsSinceConsolidation)
    : CONSOLIDATION_INTERVAL_TURNS;
  const consolidationProgress = Math.min(
    100,
    Math.round((turnsSinceConsolidation / CONSOLIDATION_INTERVAL_TURNS) * 100),
  );
  const activeMessages = useMemo(
    () => messages.filter((message) => message.role !== "system"),
    [messages],
  );

  return (
    <main className="app-shell">
      <aside className="character-panel">
        <header className="panel-header">
          <div>
            <span className="eyebrow">Soul</span>
            <h1>{soul?.character_name ?? "Mnemosyne"}</h1>
          </div>
          <Brain aria-hidden="true" />
        </header>

        <div className="avatar" aria-hidden="true">
          {soul?.character_name.slice(0, 1) ?? "M"}
        </div>

        <section className="creator-section">
          <h2>Identity</h2>

        <label className="field">
          <span>Character</span>
          <input
            value={characterName}
            onChange={(event) => setCharacterName(event.target.value)}
            placeholder="Character name"
          />
        </label>

        <label className="field">
          <span>Appearance</span>
          <textarea
            value={characterAppearance}
            onChange={(event) => setCharacterAppearance(event.target.value)}
            placeholder="Visual details, outfit, body language"
          />
        </label>

        <label className="field">
          <span>Setting</span>
          <textarea
            value={characterSetting}
            onChange={(event) => setCharacterSetting(event.target.value)}
            placeholder="Starting location and scene"
          />
        </label>

        <label className="field">
          <span>Personality</span>
          <textarea
            value={characterPersonality}
            onChange={(event) => setCharacterPersonality(event.target.value)}
            placeholder="Voice, motives, boundaries"
          />
        </label>

        <label className="field">
          <span>Description</span>
          <textarea
            value={characterDescription}
            onChange={(event) => setCharacterDescription(event.target.value)}
            placeholder="Backstory or character card notes"
          />
        </label>
        </section>

        <section className="creator-section">
          <h2>Starting Psyche</h2>
          <label className="field">
            <span>Preset</span>
            <select
              value={psychePreset}
              onChange={(event) => handlePresetChange(event.target.value as PsychePresetName)}
            >
              {Object.keys(PSYCHE_PRESETS).map((presetName) => (
                <option key={presetName}>{presetName}</option>
              ))}
            </select>
          </label>

          <div className="slider-group">
            <h3>Global Traits</h3>
            <RangeField
              label="Fear Baseline"
              value={psyche.global.fear_baseline}
              onChange={(value) =>
                updatePsyche((current) => ({
                  ...current,
                  global: { ...current.global, fear_baseline: value },
                }))
              }
            />
            <RangeField
              label="Resolve"
              value={psyche.global.resolve}
              onChange={(value) =>
                updatePsyche((current) => ({
                  ...current,
                  global: { ...current.global, resolve: value },
                }))
              }
            />
            <RangeField
              label="Shame"
              value={psyche.global.shame}
              onChange={(value) =>
                updatePsyche((current) => ({
                  ...current,
                  global: { ...current.global, shame: value },
                }))
              }
            />
            <RangeField
              label="Openness"
              value={psyche.global.openness}
              onChange={(value) =>
                updatePsyche((current) => ({
                  ...current,
                  global: { ...current.global, openness: value },
                }))
              }
            />
          </div>

          <div className="slider-group">
            <h3>Needs</h3>
            {["Physiological", "Safety", "Belonging", "Esteem", "Actualization"].map(
              (label, index) => (
                <RangeField
                  key={label}
                  label={label}
                  value={psyche.maslow[index]}
                  onChange={(value) =>
                    updatePsyche((current) => {
                      const maslow = [...current.maslow] as PsycheDraft["maslow"];
                      maslow[index] = value;
                      return { ...current, maslow };
                    })
                  }
                />
              ),
            )}
          </div>

          <div className="slider-group">
            <h3>SDT</h3>
            {["Autonomy", "Competence", "Relatedness"].map((label, index) => (
              <RangeField
                key={label}
                label={label}
                value={psyche.sdt[index]}
                onChange={(value) =>
                  updatePsyche((current) => {
                    const sdt = [...current.sdt] as PsycheDraft["sdt"];
                    sdt[index] = value;
                    return { ...current, sdt };
                  })
                }
              />
            ))}
          </div>

          <div className="slider-group">
            <h3>Trauma</h3>
            <RangeField
              label="Phase"
              min={0}
              max={4}
              value={psyche.trauma.phase}
              onChange={(value) =>
                updatePsyche((current) => ({
                  ...current,
                  trauma: { ...current.trauma, phase: value },
                }))
              }
            />
            {[
              ["Hypervigilance", "hypervigilance"],
              ["Flashbacks", "flashbacks"],
              ["Numbing", "numbing"],
              ["Avoidance", "avoidance"],
            ].map(([label, key]) => (
              <RangeField
                key={key}
                label={label}
                value={psyche.trauma[key as keyof PsycheDraft["trauma"]]}
                onChange={(value) =>
                  updatePsyche((current) => ({
                    ...current,
                    trauma: { ...current.trauma, [key]: value },
                  }))
                }
              />
            ))}
          </div>

          <div className="slider-group">
            <h3>Relationship</h3>
            {[
              ["Trust", "trust", -100, 100],
              ["Affection", "affection", -100, 100],
              ["Intimacy", "intimacy", -100, 100],
              ["Passion", "passion", -100, 100],
              ["Commitment", "commitment", -100, 100],
              ["Fear", "fear", 0, 100],
              ["Desire", "desire", -100, 100],
            ].map(([label, key, min, max]) => (
              <RangeField
                key={key}
                label={String(label)}
                min={Number(min)}
                max={Number(max)}
                value={psyche.relationship[key as keyof PsycheDraft["relationship"]]}
                onChange={(value) =>
                  updatePsyche((current) => ({
                    ...current,
                    relationship: { ...current.relationship, [String(key)]: value },
                  }))
                }
              />
            ))}
          </div>
        </section>

        <button className="wide-button" onClick={handleCreateSoul} disabled={busy}>
          <Sparkles size={18} />
          New Soul
        </button>

        <section className="compact-list" aria-label="Saved souls">
          <h2>Local Souls</h2>
          {souls.length === 0 ? (
            <p className="muted">No saved Souls yet.</p>
          ) : (
            souls.map((item) => (
              <button
                key={item.character_id}
                className={`soul-row ${soul?.character_id === item.character_id ? "selected" : ""}`}
                onClick={() => handleSelectSoul(item.character_id)}
              >
                <span>{item.character_name}</span>
                <small>
                  {item.core_count} core / {item.recent_count} recent
                </small>
              </button>
            ))
          )}
        </section>

        <section className="stat-grid" aria-label="Relationship stats">
          <Stat label="Trust" value={relationship?.trust ?? 0} />
          <Stat label="Affection" value={relationship?.affection ?? 0} />
          <Stat label="Fear" value={relationship?.fear ?? 0} />
          <Stat label="Turns" value={soul?.turn_counter ?? 0} />
          <Stat label="Since Sleep" value={turnsSinceConsolidation} />
          <Stat label="Schemas" value={soul?.memory.schemas.length ?? 0} />
        </section>

        <section className="memory-section">
          <h2>Core Memories</h2>
          {(soul?.memory.core ?? []).slice(0, 4).map((memory) => (
            <p key={memory}>{memory}</p>
          ))}
        </section>

        <section className="memory-section">
          <h2>Schemas</h2>
          {(soul?.memory.schemas ?? []).map((schema) => (
            <p key={schema.schema_type}>
              <strong>{schema.schema_type}</strong>: {schema.summary}
            </p>
          ))}
        </section>
      </aside>

      <section className="chat-panel">
        <div className="chat-header">
          <div>
            <span className="eyebrow">Provider: {provider} / {mode}</span>
            <h2>Chat Window</h2>
          </div>
          <div className="token-pill">
            {context?.estimated_tokens ?? 0}
            <span>tok</span>
          </div>
        </div>

        <div className="message-list">
          {activeMessages.length === 0 ? (
            <div className="empty-state">
              <MessageSquareText size={34} />
              <p>Start a local mock turn to exercise memory, hidden state, and context compilation.</p>
            </div>
          ) : (
            activeMessages.map((message) => (
              <article className={`message ${message.role}`} key={message.id}>
                <span>{message.role === "user" ? "User" : "Narrator"}</span>
                <p>{message.content}</p>
              </article>
            ))
          )}
        </div>

        <form className="composer" onSubmit={handleSubmit}>
          <input
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Type message..."
            disabled={busy}
          />
          <button aria-label="Send message" disabled={busy || !draft.trim() || !soul}>
            <Play size={18} />
          </button>
        </form>
      </section>

      <aside className="controls-panel">
        <header className="panel-header">
          <div>
            <span className="eyebrow">Controls</span>
            <h2>Session</h2>
          </div>
          <Database aria-hidden="true" />
        </header>

        <label className="field">
          <span>Provider</span>
          <select
            value={provider}
            onChange={(event) => setProvider(event.target.value as ProviderKind)}
          >
            <option>Mock</option>
            <option>API</option>
          </select>
        </label>

        {provider === "API" ? (
          <section className="provider-settings">
            <label className="field">
              <span>Base URL</span>
              <input
                value={apiSettings.base_url}
                onChange={(event) =>
                  setApiSettings((current) => ({ ...current, base_url: event.target.value }))
                }
                placeholder="https://api.openai.com/v1"
              />
            </label>
            <label className="field">
              <span>Model</span>
              <input
                value={apiSettings.model}
                onChange={(event) =>
                  setApiSettings((current) => ({ ...current, model: event.target.value }))
                }
                placeholder="Model name"
              />
            </label>
            <label className="field">
              <span>API Key</span>
              <input
                type="password"
                value={apiSettings.api_key}
                onChange={(event) =>
                  setApiSettings((current) => ({ ...current, api_key: event.target.value }))
                }
                placeholder="Stored only in this session"
              />
            </label>
            {mode === "Custom" ? (
              <label className="field">
                <span>Custom Prompt</span>
                <textarea
                  value={apiSettings.system_prompt}
                  onChange={(event) =>
                    setApiSettings((current) => ({
                      ...current,
                      system_prompt: event.target.value,
                    }))
                  }
                />
              </label>
            ) : null}
          </section>
        ) : null}

        <label className="field">
          <span>Mode</span>
          <select
            value={mode}
            onChange={(event) => setMode(event.target.value as NarrativeMode)}
          >
            <option>Realistic</option>
            <option>Reader</option>
            <option>God</option>
            <option>Custom</option>
          </select>
        </label>

        <div className="button-grid">
          <input
            ref={importInputRef}
            className="hidden-file"
            type="file"
            accept="application/json,.json,.soul,.mne"
            onChange={handleImportSoulFile}
          />
          <button
            title="Import Soul"
            onClick={() => importInputRef.current?.click()}
            disabled={busy}
          >
            <FileUp size={18} />
          </button>
          <button title="Export Soul" onClick={handleSaveSoul} disabled={!soul || busy}>
            <FileDown size={18} />
          </button>
          <button
            title="Persist current Soul"
            onClick={async () => {
              if (!soul) return;
              const nextSoul = applyCreatorFields(soul);
              await upsertSoul(nextSoul);
              setSoul(nextSoul);
              setSouls(await listSouls());
              setStatus("Soul persisted");
            }}
            disabled={!soul || busy}
          >
            <Save size={18} />
          </button>
          <button title="Run consolidation" onClick={handleConsolidate} disabled={!soul || busy}>
            <RefreshCcw size={18} />
          </button>
          <button title="Delete current chat" onClick={handleDeleteChat} disabled={!soul || busy}>
            <MessageSquareX size={18} />
          </button>
          <button
            className="danger-button"
            title="Delete selected Soul"
            onClick={handleDeleteSoul}
            disabled={!soul || busy}
          >
            <Trash2 size={18} />
          </button>
        </div>

        <section className="diagnostics-section">
          <h2>Memory Cycle</h2>
          <div className="cycle-meter" aria-label="Consolidation progress">
            <div>
              <strong>{turnsSinceConsolidation}</strong>
              <span>/ {CONSOLIDATION_INTERVAL_TURNS} turns</span>
            </div>
            <div className="cycle-bar">
              <span style={{ width: `${consolidationProgress}%` }} />
            </div>
          </div>
          <dl className="diagnostic-grid">
            <div>
              <dt>Next sleep</dt>
              <dd>{turnsUntilConsolidation === 0 ? "Ready" : `${turnsUntilConsolidation} turns`}</dd>
            </div>
            <div>
              <dt>Core</dt>
              <dd>{soul?.memory.core.length ?? 0}</dd>
            </div>
            <div>
              <dt>Recent</dt>
              <dd>{soul?.memory.recent.length ?? 0}</dd>
            </div>
            <div>
              <dt>Context</dt>
              <dd>{context?.truncated ? "Trimmed" : "Within budget"}</dd>
            </div>
          </dl>
        </section>

        <section className="context-preview">
          <h2>Context</h2>
          <pre>{context?.text ?? "No context compiled yet."}</pre>
        </section>

        <section className="memory-section">
          <h2>Recent</h2>
          {(soul?.memory.recent ?? []).map((memory) => (
            <p key={memory.id}>
              <strong>{memory.tag}</strong> / {memory.salience}: {memory.content}
            </p>
          ))}
        </section>

        <footer className="status-line">{status}</footer>
      </aside>
    </main>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="stat">
      <span>{label}</span>
      <strong>{Math.round(value)}</strong>
    </div>
  );
}

function RangeField({
  label,
  value,
  min = 0,
  max = 100,
  onChange,
}: {
  label: string;
  value: number;
  min?: number;
  max?: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="range-field">
      <span>{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
      <strong>{value > 0 && min < 0 ? `+${value}` : value}</strong>
    </label>
  );
}

function conversationIdForSoul(soulId: string) {
  return `local-mock-${soulId}`;
}

function psycheFromSoul(soul: Soul): PsycheDraft {
  const relationship = soul.relationships.user ?? PSYCHE_PRESETS.Custom.relationship;
  return {
    global: {
      fear_baseline: soul.global.fear_baseline,
      resolve: soul.global.resolve,
      shame: soul.global.shame,
      openness: soul.global.openness,
    },
    maslow: [
      soul.global.maslow[0] ?? 60,
      soul.global.maslow[1] ?? 50,
      soul.global.maslow[2] ?? 40,
      soul.global.maslow[3] ?? 30,
      soul.global.maslow[4] ?? 20,
    ],
    sdt: [soul.global.sdt[0] ?? 70, soul.global.sdt[1] ?? 40, soul.global.sdt[2] ?? 10],
    trauma: {
      phase: soul.trauma.phase,
      hypervigilance: soul.trauma.symptoms.hypervigilance ?? 10,
      flashbacks: soul.trauma.symptoms.flashbacks ?? 10,
      numbing: soul.trauma.symptoms.numbing ?? 10,
      avoidance: soul.trauma.symptoms.avoidance ?? 10,
    },
    relationship: {
      trust: relationship.trust,
      affection: relationship.affection,
      intimacy: relationship.intimacy,
      passion: relationship.passion,
      commitment: relationship.commitment,
      fear: relationship.fear,
      desire: relationship.desire,
    },
  };
}

async function soulFromImport(raw: unknown, fallbackName: string) {
  const record = isRecord(raw) && isRecord(raw.soul) ? raw.soul : raw;
  if (!isRecord(record)) {
    throw new Error("Import file must be a Soul JSON object or package with a soul field");
  }

  const importedName = stringFrom(record.character_name) || stringFrom(record.name);
  const base = await createDefaultSoul(importedName || fallbackName.replace(/\.[^.]+$/, ""));
  const profile = isRecord(record.profile) ? record.profile : {};
  const world = isRecord(record.world) ? record.world : {};
  const memory = isRecord(record.memory) ? record.memory : {};
  const description =
    stringFrom(profile.description) || stringFrom(record.description) || stringFrom(record.persona);
  const appearance = stringFrom(profile.appearance) || stringFrom(record.appearance);
  const personality = stringFrom(profile.personality) || stringFrom(record.personality);
  const scenario =
    stringFrom(profile.scenario) || stringFrom(record.scenario) || stringFrom(record.setting);
  const location = stringFrom(world.location) || scenario || base.world.location;
  const core = stringArrayFrom(isRecord(memory) ? memory.core : undefined);

  return {
    ...base,
    ...record,
    schema_version: Number(record.schema_version) || base.schema_version,
    character_id: stringFrom(record.character_id) || base.character_id,
    character_name: importedName || base.character_name,
    profile: {
      description,
      appearance,
      personality,
      scenario,
    },
    memory: {
      ...base.memory,
      ...(isRecord(memory) ? memory : {}),
      core: core.length
        ? core
        : [
            ...base.memory.core,
            description ? `Profile: ${description}` : "",
            appearance ? `Appearance: ${appearance}` : "",
            personality ? `Personality: ${personality}` : "",
          ].filter(Boolean),
    },
    world: {
      ...base.world,
      ...(isRecord(world) ? world : {}),
      location,
      active_plots: stringArrayFrom(world.active_plots).length
        ? stringArrayFrom(world.active_plots)
        : base.world.active_plots,
    },
  } as Soul;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringFrom(value: unknown) {
  return typeof value === "string" ? value.trim() : "";
}

function stringArrayFrom(value: unknown) {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}
