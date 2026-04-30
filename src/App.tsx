import {
  ArrowLeft,
  Brain,
  ChevronDown,
  Database,
  FileDown,
  FileUp,
  MessageSquareText,
  Play,
  RefreshCcw,
  Save,
  Sparkles,
  Square,
  Trash2,
} from "lucide-react";
import { ChangeEvent, FormEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  ApiProviderSettings,
  ChatMessage,
  ContextPreview,
  SettingSoul,
  SettingSummary,
  Soul,
  SoulSummary,
  TurnDebug,
  compileContext,
  createDefaultSoul,
  createDefaultSetting,
  deleteConversation,
  deleteMessage,
  deleteSetting,
  deleteSoul,
  getSetting,
  getSoul,
  listConversationMessages,
  listSettings,
  listSouls,
  runConsolidation,
  saveSettingFile,
  saveSoulFile,
  sendApiTurn,
  sendMockTurn,
  upsertSetting,
  upsertSoul,
} from "./tauri";

const DEFAULT_CONVERSATION_ID = "local-mock";
const CONSOLIDATION_INTERVAL_TURNS = 10;
type ProviderKind = "Mock" | "API";
type NarrativeMode = "Realistic" | "Reader" | "God" | "Custom";
type AppView = "library" | "chat";
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

type WorldDraft = {
  location: string;
  activePlots: string;
  keyObjects: string;
  timeElapsed: string;
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
  const [settings, setSettings] = useState<SettingSummary[]>([]);
  const [soul, setSoul] = useState<Soul | null>(null);
  const [setting, setSetting] = useState<SettingSoul | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [context, setContext] = useState<ContextPreview | null>(null);
  const [draft, setDraft] = useState("");
  const [characterName, setCharacterName] = useState("Aurora Schwarz");
  const [characterDescription, setCharacterDescription] = useState("");
  const [characterAppearance, setCharacterAppearance] = useState("");
  const [characterPersonality, setCharacterPersonality] = useState("");
  const [characterScenario, setCharacterScenario] = useState("");
  const [settingName, setSettingName] = useState("Starter Setting");
  const [worldDraft, setWorldDraft] = useState<WorldDraft>({
    location: "Unspecified starting scene.",
    activePlots: "Establish the first scene",
    keyObjects: "",
    timeElapsed: "Session start",
  });
  const [psychePreset, setPsychePreset] = useState<PsychePresetName>("Custom");
  const [psyche, setPsyche] = useState<PsycheDraft>(PSYCHE_PRESETS.Custom);
  const [settingEditorOpen, setSettingEditorOpen] = useState(false);
  const [soulEditorOpen, setSoulEditorOpen] = useState(false);
  const [psycheOpen, setPsycheOpen] = useState(false);
  const [provider, setProvider] = useState<ProviderKind>("Mock");
  const [mode, setMode] = useState<NarrativeMode>("Reader");
  const [apiSettings, setApiSettings] = useState<ApiProviderSettings>({
    base_url: "https://api.openai.com/v1",
    api_key: "",
    model: "",
    system_prompt: "",
  });
  const [lastTurnDebug, setLastTurnDebug] = useState<TurnDebug | null>(null);
  const [view, setView] = useState<AppView>("library");
  const [status, setStatus] = useState("Ready");
  const [busy, setBusy] = useState(false);
  const didBootstrap = useRef(false);
  const importInputRef = useRef<HTMLInputElement>(null);
  const settingImportInputRef = useRef<HTMLInputElement>(null);
  const generationAbortRef = useRef<AbortController | null>(null);
  const generationIdRef = useRef(0);
  const currentConversationId = useMemo(
    () =>
      soul && setting
        ? conversationIdForSettingAndSoul(setting.setting_id, soul.character_id)
        : DEFAULT_CONVERSATION_ID,
    [setting?.setting_id, soul?.character_id],
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
    setCharacterScenario(nextSoul.profile.scenario);
    setPsyche(psycheFromSoul(nextSoul));
    setPsychePreset("Custom");
  }

  function setEditorFieldsFromSetting(nextSetting: SettingSoul) {
    setSettingName(nextSetting.setting_name);
    setWorldDraft(worldDraftFromSetting(nextSetting));
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
    const scenario = characterScenario.trim();
    const world = normalizeWorldDraft(worldDraft);
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
        scenario,
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
        location: world.location,
        active_plots: world.activePlots,
        key_objects: world.keyObjects,
        time_elapsed: world.timeElapsed,
      },
    };
  }

  function applySettingFields(nextSetting: SettingSoul) {
    const world = normalizeWorldDraft(worldDraft);
    return {
      ...nextSetting,
      setting_name: settingName.trim() || "Untitled Setting",
      last_updated: Math.floor(Date.now() / 1000),
      world: {
        ...nextSetting.world,
        location: world.location,
        active_plots: world.activePlots,
        key_objects: world.keyObjects,
        time_elapsed: world.timeElapsed,
      },
    };
  }

  function mirrorSettingIntoSoul(nextSoul: Soul, nextSetting: SettingSoul) {
    return {
      ...nextSoul,
      world: nextSetting.world,
    };
  }

  async function persistCurrentSetting() {
    if (!setting) return null;
    const nextSetting = applySettingFields(setting);
    await upsertSetting(nextSetting);
    setSetting(nextSetting);
    setSettings(await listSettings());
    return nextSetting;
  }

  async function bootstrap() {
    const [existingSouls, existingSettings] = await Promise.all([listSouls(), listSettings()]);
    setSouls(existingSouls);
    setSettings(existingSettings);

    let activeSetting: SettingSoul;
    if (existingSettings.length > 0) {
      activeSetting = await getSetting(existingSettings[0].setting_id);
    } else {
      activeSetting = await createDefaultSetting(settingName);
      await upsertSetting(activeSetting);
      setSettings(await listSettings());
    }
    setSetting(activeSetting);
    setEditorFieldsFromSetting(activeSetting);

    if (existingSouls.length > 0) {
      const firstSoul = await getSoul(existingSouls[0].character_id);
      setSoul(firstSoul);
      setCreatorFieldsFromSoul(firstSoul);
      setMessages(
        await listConversationMessages(
          conversationIdForSettingAndSoul(activeSetting.setting_id, firstSoul.character_id),
        ),
      );
      setStatus("Loaded local Soul and Setting indexes");
      return;
    }

    const nextSoul = await createDefaultSoul(characterName);
    await upsertSoul(nextSoul);
    setSoul(nextSoul);
    setSouls(await listSouls());
    setStatus("Created starter Soul and Setting");
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
      setLastTurnDebug(null);
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
      setLastTurnDebug(null);
      setMessages(
        await listConversationMessages(
          setting
            ? conversationIdForSettingAndSoul(setting.setting_id, nextSoul.character_id)
            : conversationIdForSoul(nextSoul.character_id),
        ),
      );
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleCreateSetting() {
    setBusy(true);
    try {
      const nextSetting = applySettingFields(
        await createDefaultSetting(settingName || "Untitled Setting"),
      );
      await upsertSetting(nextSetting);
      setSetting(nextSetting);
      setEditorFieldsFromSetting(nextSetting);
      setSettings(await listSettings());
      setMessages([]);
      setLastTurnDebug(null);
      setStatus("New Setting created");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSelectSetting(settingId: string) {
    const selected = settings.find((item) => item.setting_id === settingId);
    if (!selected) return;

    setBusy(true);
    try {
      const nextSetting = await getSetting(selected.setting_id);
      setSetting(nextSetting);
      setEditorFieldsFromSetting(nextSetting);
      setLastTurnDebug(null);
      setMessages(
        soul
          ? await listConversationMessages(
              conversationIdForSettingAndSoul(nextSetting.setting_id, soul.character_id),
            )
          : [],
      );
      setStatus(`Selected ${nextSetting.setting_name}`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function executeTurn(text: string, statusLabel?: string) {
    if (!text || busy || !soul) return;
    const generationId = generationIdRef.current + 1;
    generationIdRef.current = generationId;
    const abortController = new AbortController();
    generationAbortRef.current = abortController;
    setBusy(true);
    setStatus(statusLabel ?? (provider === "API" ? "API provider thinking" : "Mock provider thinking"));

    try {
      const activeSetting = await persistCurrentSetting();
      const activeSoul = activeSetting ? mirrorSettingIntoSoul(soul, activeSetting) : soul;
      await upsertSoul(activeSoul);
      const result =
        provider === "API"
          ? await sendApiTurn(
              currentConversationId,
              activeSoul.character_id,
              text,
              mode,
              apiSettings,
              abortController.signal,
            )
          : await sendMockTurn(currentConversationId, activeSoul.character_id, text, mode);
      if (generationIdRef.current !== generationId || abortController.signal.aborted) {
        return;
      }
      if (activeSetting) {
        const updatedSetting = {
          ...activeSetting,
          turn_counter: activeSetting.turn_counter + 1,
          last_updated: Math.floor(Date.now() / 1000),
          world: result.soul.world,
        };
        await upsertSetting(updatedSetting);
        setSetting(updatedSetting);
        setEditorFieldsFromSetting(updatedSetting);
      }
      setSoul(result.soul);
      setMessages(result.messages);
      setContext(result.context_preview);
      setLastTurnDebug(result.debug);
      setSouls(await listSouls());
      setStatus(result.consolidation_ran ? "Turn saved; consolidation ran" : "Turn saved");
    } catch (error) {
      if (abortController.signal.aborted) {
        setStatus("Generation stopped");
      } else {
        setStatus(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (generationIdRef.current === generationId) {
        setBusy(false);
        generationAbortRef.current = null;
      }
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const text = draft.trim();
    if (!text || busy || !soul) return;

    setDraft("");
    await executeTurn(text);
  }

  async function handleRegenerate() {
    if (busy || !soul) return;
    const lastUserMessage = [...activeMessages].reverse().find((message) => message.role === "user");
    if (!lastUserMessage) {
      setStatus("No user message to regenerate");
      return;
    }
    await executeTurn(lastUserMessage.content, "Regenerating last turn");
  }

  async function handleRegenerateFromMessage(message: ChatMessage) {
    if (busy || !soul || message.role !== "assistant") return;
    const messageIndex = activeMessages.findIndex((item) => item.id === message.id);
    const previousUserMessage = activeMessages
      .slice(0, messageIndex)
      .reverse()
      .find((item) => item.role === "user");

    if (!previousUserMessage) {
      setStatus("No user prompt found for this response");
      return;
    }

    await executeTurn(previousUserMessage.content, "Regenerating response");
  }

  async function handleDeleteChatMessage(message: ChatMessage) {
    if (busy) return;
    const confirmed = window.confirm(
      message.role === "assistant"
        ? "Delete this generated response? Soul memory is not rewound."
        : "Delete this user message? Soul memory and later responses are not rewound.",
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await deleteMessage(message.conversation_id, message.id);
      const nextMessages = await listConversationMessages(message.conversation_id);
      setMessages(nextMessages);
      if (soul) {
        setContext(await compileContext(soul.character_id, message.conversation_id));
      }
      setStatus(message.role === "assistant" ? "Generated response deleted" : "User message deleted");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  function handleStopGeneration() {
    generationAbortRef.current?.abort();
    generationIdRef.current += 1;
    generationAbortRef.current = null;
    setBusy(false);
    setStatus("Generation stopped");
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
      setLastTurnDebug(null);
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
      const nextConversationId = setting
        ? conversationIdForSettingAndSoul(setting.setting_id, nextSoul.character_id)
        : conversationIdForSoul(nextSoul.character_id);
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

  async function handleDeleteSetting() {
    if (!setting) return;
    const confirmed = window.confirm(
      `Delete ${setting.setting_name}? Local chats for this Setting remain orphaned until chat cleanup is added.`,
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await deleteSetting(setting.setting_id);
      const remaining = await listSettings();
      setSettings(remaining);

      if (remaining.length === 0) {
        const nextSetting = await createDefaultSetting("Starter Setting");
        await upsertSetting(nextSetting);
        setSetting(nextSetting);
        setEditorFieldsFromSetting(nextSetting);
        setSettings(await listSettings());
        setMessages([]);
        setStatus("Setting deleted; created starter Setting");
        return;
      }

      const nextSetting = await getSetting(remaining[0].setting_id);
      setSetting(nextSetting);
      setEditorFieldsFromSetting(nextSetting);
      setMessages(
        soul
          ? await listConversationMessages(
              conversationIdForSettingAndSoul(nextSetting.setting_id, soul.character_id),
            )
          : [],
      );
      setStatus("Setting deleted; selected next local Setting");
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
      const activeSetting = await persistCurrentSetting();
      const nextSoul = activeSetting
        ? mirrorSettingIntoSoul(applyCreatorFields(soul), activeSetting)
        : applyCreatorFields(soul);
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

  async function handleSaveSetting() {
    if (!setting) return;
    setBusy(true);
    try {
      const nextSetting = applySettingFields(setting);
      await upsertSetting(nextSetting);
      setSetting(nextSetting);
      setEditorFieldsFromSetting(nextSetting);
      await saveSettingFile(
        `${nextSetting.setting_name.replace(/\s+/g, "_")}.setting.json`,
        nextSetting,
      );
      setSettings(await listSettings());
      setStatus("Setting exported beside the app working directory");
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
      setMessages(
        await listConversationMessages(
          setting
            ? conversationIdForSettingAndSoul(setting.setting_id, importedSoul.character_id)
            : conversationIdForSoul(importedSoul.character_id),
        ),
      );
      setSouls(await listSouls());
      setStatus(`Imported ${importedSoul.character_name}`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleImportSettingFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;

    setBusy(true);
    try {
      const raw = JSON.parse(await file.text());
      const importedSetting = settingFromImport(raw, file.name);
      await upsertSetting(importedSetting);
      setSetting(importedSetting);
      setEditorFieldsFromSetting(importedSetting);
      setSettings(await listSettings());
      setMessages(
        soul
          ? await listConversationMessages(
              conversationIdForSettingAndSoul(importedSetting.setting_id, soul.character_id),
            )
          : [],
      );
      setStatus(`Imported ${importedSetting.setting_name}`);
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

  if (view === "chat") {
    return (
      <main className="chat-only-shell">
        <header className="chat-only-header">
          <button className="ghost-action" onClick={() => setView("library")} disabled={busy}>
            <ArrowLeft size={18} />
            <span>Library</span>
          </button>
          <div>
            <span className="eyebrow">
              {setting?.setting_name ?? "Local Setting"} / {provider} / {mode}
            </span>
            <h1>{soul?.character_name ?? "Mnemosyne"}</h1>
          </div>
          <div className="token-pill">
            {context?.estimated_tokens ?? 0}
            <span>tok</span>
          </div>
        </header>

        <section className="chat-only-body">
          {activeMessages.length === 0 ? (
            <div className="empty-state">
              <MessageSquareText size={34} />
              <p>No messages yet.</p>
            </div>
          ) : (
            activeMessages.map((message) => (
              <article className={`message ${message.role}`} key={message.id}>
                <div className="message-heading">
                  <span>{message.role === "user" ? "User" : "Narrator"}</span>
                  {message.role === "assistant" ? (
                    <div className="message-tools">
                      <button
                        title="Regenerate this response"
                        onClick={() => handleRegenerateFromMessage(message)}
                        disabled={busy}
                      >
                        <RefreshCcw size={14} />
                      </button>
                      <button
                        title="Delete this response"
                        onClick={() => handleDeleteChatMessage(message)}
                        disabled={busy}
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  ) : (
                    <div className="message-tools">
                      <button
                        title="Delete this message"
                        onClick={() => handleDeleteChatMessage(message)}
                        disabled={busy}
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  )}
                </div>
                <p>{message.content}</p>
              </article>
            ))
          )}
        </section>

        <form className="chat-only-composer" onSubmit={handleSubmit}>
          <input
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Type message..."
            disabled={busy}
          />
          {busy ? (
            <button type="button" aria-label="Stop generation" onClick={handleStopGeneration}>
              <Square size={16} />
            </button>
          ) : (
            <button aria-label="Send message" disabled={!draft.trim() || !soul}>
              <Play size={18} />
            </button>
          )}
        </form>
      </main>
    );
  }

  return (
    <main className="app-shell">
      <section className="library-grid">
        <section className="workspace-card library-card">
          <header className="panel-header">
            <div>
              <span className="eyebrow">Scenes</span>
              <h2>Local Settings</h2>
            </div>
            <Database aria-hidden="true" />
          </header>

          <div className="action-grid compact-actions">
            <input
              ref={settingImportInputRef}
              className="hidden-file"
              type="file"
              accept="application/json,.json,.setting,.mne"
              onChange={handleImportSettingFile}
            />
            <button title="New Setting" onClick={handleCreateSetting} disabled={busy}>
              <Sparkles size={18} />
              <span>New</span>
            </button>
            <button
              title="Import Setting"
              onClick={() => settingImportInputRef.current?.click()}
              disabled={busy}
            >
              <FileUp size={18} />
              <span>Import</span>
            </button>
            <button title="Export Setting" onClick={handleSaveSetting} disabled={!setting || busy}>
              <FileDown size={18} />
              <span>Export</span>
            </button>
            <button
              title="Persist current Setting"
              onClick={async () => {
                const nextSetting = await persistCurrentSetting();
                if (nextSetting) setStatus("Setting persisted");
              }}
              disabled={!setting || busy}
            >
              <Save size={18} />
              <span>Save</span>
            </button>
            <button
              className="danger-button"
              title="Delete selected Setting"
              onClick={handleDeleteSetting}
              disabled={!setting || busy}
            >
              <Trash2 size={18} />
              <span>Delete</span>
            </button>
          </div>

          <section className="compact-list library-list" aria-label="Saved settings">
            {settings.length === 0 ? (
              <p className="muted">No saved Settings yet.</p>
            ) : (
              settings.map((item) => (
                <button
                  key={item.setting_id}
                  className={`soul-row ${setting?.setting_id === item.setting_id ? "selected" : ""}`}
                  onClick={() => handleSelectSetting(item.setting_id)}
                >
                  <span>{item.setting_name}</span>
                  <small>
                    {item.turn_counter} turns / {item.location}
                  </small>
                </button>
              ))
            )}
          </section>

          <section className={`collapsible-section ${settingEditorOpen ? "open" : ""}`}>
            <button
              className="section-toggle studio-toggle"
              type="button"
              onClick={() => setSettingEditorOpen((open) => !open)}
              aria-expanded={settingEditorOpen}
            >
              <span>
                <span className="eyebrow">World</span>
                <strong>{setting?.setting_name ?? "Environment Creator"}</strong>
              </span>
              <ChevronDown size={18} aria-hidden="true" />
            </button>

            {settingEditorOpen ? (
              <div className="collapsible-content">
                <div className="form-grid single-column-form">
                  <label className="field">
                    <span>Name</span>
                    <input
                      value={settingName}
                      onChange={(event) => setSettingName(event.target.value)}
                      placeholder="Setting name"
                    />
                  </label>
                  <label className="field">
                    <span>Location</span>
                    <textarea
                      value={worldDraft.location}
                      onChange={(event) =>
                        setWorldDraft((current) => ({ ...current, location: event.target.value }))
                      }
                      placeholder="Shared scene location"
                    />
                  </label>
                  <label className="field">
                    <span>Active Plots</span>
                    <textarea
                      value={worldDraft.activePlots}
                      onChange={(event) =>
                        setWorldDraft((current) => ({
                          ...current,
                          activePlots: event.target.value,
                        }))
                      }
                      placeholder="One plot per line"
                    />
                  </label>
                  <label className="field">
                    <span>Key Objects</span>
                    <textarea
                      value={worldDraft.keyObjects}
                      onChange={(event) =>
                        setWorldDraft((current) => ({
                          ...current,
                          keyObjects: event.target.value,
                        }))
                      }
                      placeholder="One object per line"
                    />
                  </label>
                  <label className="field">
                    <span>Time</span>
                    <input
                      value={worldDraft.timeElapsed}
                      onChange={(event) =>
                        setWorldDraft((current) => ({
                          ...current,
                          timeElapsed: event.target.value,
                        }))
                      }
                      placeholder="Session start"
                    />
                  </label>
                </div>
              </div>
            ) : null}
          </section>
        </section>

        <section className="workspace-card library-card">
          <header className="panel-header character-heading">
            <div>
              <span className="eyebrow">Characters</span>
              <h2>Local Souls</h2>
            </div>
            <div className="avatar" aria-hidden="true">
              {soul?.character_name.slice(0, 1) ?? "M"}
            </div>
          </header>

          <div className="action-grid compact-actions">
            <input
              ref={importInputRef}
              className="hidden-file"
              type="file"
              accept="application/json,.json,.soul,.mne"
              onChange={handleImportSoulFile}
            />
            <button title="New Soul" onClick={handleCreateSoul} disabled={busy}>
              <Sparkles size={18} />
              <span>New</span>
            </button>
            <button
              title="Import Soul"
              onClick={() => importInputRef.current?.click()}
              disabled={busy}
            >
              <FileUp size={18} />
              <span>Import</span>
            </button>
            <button title="Export Soul" onClick={handleSaveSoul} disabled={!soul || busy}>
              <FileDown size={18} />
              <span>Export</span>
            </button>
            <button
              title="Persist current Soul"
              onClick={async () => {
                if (!soul) return;
                const activeSetting = await persistCurrentSetting();
                const nextSoul = activeSetting
                  ? mirrorSettingIntoSoul(applyCreatorFields(soul), activeSetting)
                  : applyCreatorFields(soul);
                await upsertSoul(nextSoul);
                setSoul(nextSoul);
                setSouls(await listSouls());
                setStatus("Soul and active Setting persisted");
              }}
              disabled={!soul || busy}
            >
              <Save size={18} />
              <span>Save</span>
            </button>
            <button title="Run consolidation" onClick={handleConsolidate} disabled={!soul || busy}>
              <RefreshCcw size={18} />
              <span>Sleep</span>
            </button>
            <button
              className="danger-button"
              title="Delete selected Soul"
              onClick={handleDeleteSoul}
              disabled={!soul || busy}
            >
              <Trash2 size={18} />
              <span>Delete</span>
            </button>
          </div>

          <section className="compact-list library-list" aria-label="Saved souls">
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

          <section className={`collapsible-section ${soulEditorOpen ? "open" : ""}`}>
            <button
              className="section-toggle studio-toggle"
              type="button"
              onClick={() => setSoulEditorOpen((open) => !open)}
              aria-expanded={soulEditorOpen}
            >
              <span>
                <span className="eyebrow">Soul</span>
                <strong>{soul?.character_name ?? "Character Creator"}</strong>
              </span>
              <ChevronDown size={18} aria-hidden="true" />
            </button>

            {soulEditorOpen ? (
              <div className="collapsible-content">
                <div className="form-grid single-column-form">
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
                    <span>Scenario Notes</span>
                    <textarea
                      value={characterScenario}
                      onChange={(event) => setCharacterScenario(event.target.value)}
                      placeholder="Character-specific role, premise, or card notes"
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
                </div>

                <section className={`collapsible-section ${psycheOpen ? "open" : ""}`}>
                  <button
                    className="section-toggle"
                    type="button"
                    onClick={() => setPsycheOpen((open) => !open)}
                    aria-expanded={psycheOpen}
                  >
                    <span>
                      <span className="eyebrow">Preset: {psychePreset}</span>
                      <strong>Starting Psyche</strong>
                    </span>
                    <ChevronDown size={18} aria-hidden="true" />
                  </button>

                  {psycheOpen ? (
                    <div className="collapsible-content psyche-grid single-column-form">
                      <label className="field wide-field">
                        <span>Preset</span>
                        <select
                          value={psychePreset}
                          onChange={(event) =>
                            handlePresetChange(event.target.value as PsychePresetName)
                          }
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
                    </div>
                  ) : null}
                </section>
              </div>
            ) : null}
          </section>
        </section>
      </section>

      <section className="workspace-card provider-card">
        <header className="panel-header">
          <div>
            <span className="eyebrow">Connection</span>
            <h2>Provider Settings</h2>
          </div>
          <Sparkles aria-hidden="true" />
        </header>

        <div className="session-strip launcher-provider-strip">
          <label className="field">
            <span>Provider</span>
            <select
              value={provider}
              onChange={(event) => setProvider(event.target.value as ProviderKind)}
              disabled={busy}
            >
              <option>Mock</option>
              <option>API</option>
            </select>
          </label>
          <label className="field">
            <span>Mode</span>
            <select
              value={mode}
              onChange={(event) => setMode(event.target.value as NarrativeMode)}
              disabled={busy}
            >
              <option>Realistic</option>
              <option>Reader</option>
              <option>God</option>
              <option>Custom</option>
            </select>
          </label>
          {provider === "API" ? (
            <>
              <label className="field">
                <span>Base URL</span>
                <input
                  value={apiSettings.base_url}
                  onChange={(event) =>
                    setApiSettings((current) => ({ ...current, base_url: event.target.value }))
                  }
                  placeholder="https://api.openai.com/v1"
                  disabled={busy}
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
                  disabled={busy}
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
                  disabled={busy}
                />
              </label>
              {mode === "Custom" ? (
                <label className="field custom-prompt-field">
                  <span>Custom Prompt</span>
                  <textarea
                    value={apiSettings.system_prompt}
                    onChange={(event) =>
                      setApiSettings((current) => ({
                        ...current,
                        system_prompt: event.target.value,
                      }))
                    }
                    disabled={busy}
                  />
                </label>
              ) : null}
            </>
          ) : null}
        </div>
      </section>

      <section className="workspace-card launch-card">
        <div>
          <span className="eyebrow">Ready</span>
          <h1>
            {soul?.character_name ?? "Choose a Soul"} in {setting?.setting_name ?? "a Setting"}
          </h1>
        </div>
        <button className="start-chat-button" onClick={() => setView("chat")} disabled={!soul}>
          <MessageSquareText size={20} />
          <span>Start Chat</span>
        </button>
      </section>

      <section className="play-grid">
        <aside className="studio-panel">
          <section className="setting-section workspace-card">
            <header className="panel-header">
              <div>
                <span className="eyebrow">World</span>
                <h2>Environment</h2>
              </div>
              <Database aria-hidden="true" />
            </header>

            <div className="form-grid single-column-form">
              <label className="field">
                <span>Name</span>
                <input
                  value={settingName}
                  onChange={(event) => setSettingName(event.target.value)}
                  placeholder="Setting name"
                />
              </label>
              <label className="field">
                <span>Location</span>
                <textarea
                  value={worldDraft.location}
                  onChange={(event) =>
                    setWorldDraft((current) => ({ ...current, location: event.target.value }))
                  }
                  placeholder="Shared scene location"
                />
              </label>
              <label className="field">
                <span>Active Plots</span>
                <textarea
                  value={worldDraft.activePlots}
                  onChange={(event) =>
                    setWorldDraft((current) => ({ ...current, activePlots: event.target.value }))
                  }
                  placeholder="One plot per line"
                />
              </label>
              <label className="field">
                <span>Key Objects</span>
                <textarea
                  value={worldDraft.keyObjects}
                  onChange={(event) =>
                    setWorldDraft((current) => ({ ...current, keyObjects: event.target.value }))
                  }
                  placeholder="One object per line"
                />
              </label>
              <label className="field">
                <span>Time</span>
                <input
                  value={worldDraft.timeElapsed}
                  onChange={(event) =>
                    setWorldDraft((current) => ({ ...current, timeElapsed: event.target.value }))
                  }
                  placeholder="Session start"
                />
              </label>
            </div>
          </section>

          <section className={`creator-section workspace-card collapsible-section ${soulEditorOpen ? "open" : ""}`}>
            <button
              className="section-toggle studio-toggle"
              type="button"
              onClick={() => setSoulEditorOpen((open) => !open)}
              aria-expanded={soulEditorOpen}
            >
              <span>
                <span className="eyebrow">Soul</span>
                <strong>{soul?.character_name ?? "Character Studio"}</strong>
              </span>
              <ChevronDown size={18} aria-hidden="true" />
            </button>

            {soulEditorOpen ? (
              <div className="collapsible-content">
                <div className="form-grid single-column-form">
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
                    <span>Scenario Notes</span>
                    <textarea
                      value={characterScenario}
                      onChange={(event) => setCharacterScenario(event.target.value)}
                      placeholder="Character-specific role, premise, or card notes"
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
                </div>

                <section className={`collapsible-section ${psycheOpen ? "open" : ""}`}>
                  <button
                    className="section-toggle"
                    type="button"
                    onClick={() => setPsycheOpen((open) => !open)}
                    aria-expanded={psycheOpen}
                  >
                    <span>
                      <span className="eyebrow">Preset: {psychePreset}</span>
                      <strong>Starting Psyche</strong>
                    </span>
                    <ChevronDown size={18} aria-hidden="true" />
                  </button>

                  {psycheOpen ? (
                    <div className="collapsible-content psyche-grid single-column-form">
                      <label className="field wide-field">
                        <span>Preset</span>
                        <select
                          value={psychePreset}
                          onChange={(event) =>
                            handlePresetChange(event.target.value as PsychePresetName)
                          }
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
                    </div>
                  ) : null}
                </section>
              </div>
            ) : null}
          </section>
        </aside>
      </section>

      <section className="insight-grid">
        <section className="workspace-card">
          <header className="panel-header">
            <div>
              <span className="eyebrow">State</span>
              <h2>Memory</h2>
            </div>
            <Brain aria-hidden="true" />
          </header>

          <section className="stat-grid" aria-label="Relationship stats">
            <Stat label="Trust" value={relationship?.trust ?? 0} />
            <Stat label="Affection" value={relationship?.affection ?? 0} />
            <Stat label="Fear" value={relationship?.fear ?? 0} />
            <Stat label="Turns" value={soul?.turn_counter ?? 0} />
            <Stat label="Since Sleep" value={turnsSinceConsolidation} />
            <Stat label="Schemas" value={soul?.memory.schemas.length ?? 0} />
          </section>

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

          <section className="memory-section">
            <h2>Recent</h2>
            {(soul?.memory.recent ?? []).map((memory) => (
              <p key={memory.id}>
                <strong>{memory.tag}</strong> / {memory.salience}: {memory.content}
              </p>
            ))}
          </section>
        </section>

        <section className="workspace-card">
          <section className="diagnostics-section api-debug-section">
            <h2>API Debug</h2>
            <dl className="diagnostic-grid">
              <div>
                <dt>Provider</dt>
                <dd>{lastTurnDebug?.provider ?? provider}</dd>
              </div>
              <div>
                <dt>Hidden</dt>
                <dd>
                  {lastTurnDebug
                    ? lastTurnDebug.hidden_state_found
                      ? "Parsed"
                      : "Missing"
                    : "No turn"}
                </dd>
              </div>
              <div>
                <dt>Fallback</dt>
                <dd>{lastTurnDebug?.fallback_hidden_state_generated ? "Generated" : "No"}</dd>
              </div>
              <div>
                <dt>Tag</dt>
                <dd>{lastTurnDebug?.tag ?? "-"}</dd>
              </div>
              <div>
                <dt>Trust</dt>
                <dd>{formatDebugDelta(lastTurnDebug?.trust_delta)}</dd>
              </div>
              <div>
                <dt>Affection</dt>
                <dd>{formatDebugDelta(lastTurnDebug?.affection_delta)}</dd>
              </div>
              <div>
                <dt>Location</dt>
                <dd>{lastTurnDebug?.new_location ?? "-"}</dd>
              </div>
              <div>
                <dt>Present</dt>
                <dd>{lastTurnDebug?.present_characters.join(", ") || "-"}</dd>
              </div>
            </dl>
          </section>

          <section className="context-preview">
            <h2>Context</h2>
            <pre>{context?.text ?? "No context compiled yet."}</pre>
          </section>
        </section>

        <footer className="status-line">{status}</footer>
      </section>
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

function formatDebugDelta(value: number | null | undefined) {
  if (value === null || value === undefined) return "-";
  return value > 0 ? `+${value}` : String(value);
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

function conversationIdForSettingAndSoul(settingId: string, soulId: string) {
  return `local-mock-${settingId}-${soulId}`;
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

function worldDraftFromSoul(soul: Soul): WorldDraft {
  return {
    location: soul.world.location || "Unspecified starting scene.",
    activePlots: soul.world.active_plots.join("\n") || "Establish the first scene",
    keyObjects: soul.world.key_objects.join("\n"),
    timeElapsed: soul.world.time_elapsed || "Session start",
  };
}

function worldDraftFromSetting(setting: SettingSoul): WorldDraft {
  return {
    location: setting.world.location || "Unspecified starting scene.",
    activePlots: setting.world.active_plots.join("\n") || "Establish the first scene",
    keyObjects: setting.world.key_objects.join("\n"),
    timeElapsed: setting.world.time_elapsed || "Session start",
  };
}

function normalizeWorldDraft(world: WorldDraft) {
  return {
    location: world.location.trim() || "Unspecified starting scene.",
    activePlots: linesFromText(world.activePlots, ["Establish the first scene"]),
    keyObjects: linesFromText(world.keyObjects, []),
    timeElapsed: world.timeElapsed.trim() || "Session start",
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

function settingFromImport(raw: unknown, fallbackName: string): SettingSoul {
  const record = isRecord(raw) && isRecord(raw.setting) ? raw.setting : raw;
  if (!isRecord(record)) {
    throw new Error("Import file must be a Setting JSON object or package with a setting field");
  }

  const world = isRecord(record.world) ? record.world : record;
  const fallbackSettingName = fallbackName.replace(/\.[^.]+$/, "");
  return {
    schema_version: Number(record.schema_version) || 1,
    setting_id: stringFrom(record.setting_id) || crypto.randomUUID(),
    setting_name:
      stringFrom(record.setting_name) || stringFrom(record.name) || fallbackSettingName,
    last_updated: Math.floor(Date.now() / 1000),
    turn_counter: Number(record.turn_counter) || 0,
    world: {
      location:
        stringFrom(world.location) ||
        stringFrom(record.location) ||
        "Unspecified starting scene.",
      active_plots: stringArrayFrom(world.active_plots).length
        ? stringArrayFrom(world.active_plots)
        : stringArrayFrom(record.active_plots).length
          ? stringArrayFrom(record.active_plots)
          : ["Establish the first scene"],
      recent_events: stringArrayFrom(world.recent_events),
      key_objects: stringArrayFrom(world.key_objects),
      time_elapsed: stringFrom(world.time_elapsed) || "Session start",
    },
  };
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

function linesFromText(text: string, fallback: string[]) {
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  return lines.length ? lines : fallback;
}
