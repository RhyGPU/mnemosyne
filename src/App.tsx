import {
  ArrowLeft,
  Brain,
  ChevronDown,
  Clipboard,
  Database,
  FileDown,
  FileUp,
  MessageSquareText,
  Pencil,
  Play,
  RefreshCcw,
  Save,
  Sparkles,
  Square,
  Terminal,
  Trash2,
} from "lucide-react";
import { ChangeEvent, FormEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  ApiProviderSettings,
  AssistantMessageVariant,
  ChatMessage,
  ContextPreview,
  DevLogCategory,
  DevLogEntry,
  DevLogLevel,
  LlmPayloadPreview,
  ProviderProfile,
  SettingSoul,
  SettingSummary,
  Soul,
  SoulSummary,
  TurnDebug,
  ContextMode,
  compileContext,
  createDefaultSoul,
  createFreshScenarioSoul,
  createDefaultSetting,
  deleteConversation,
  deleteMessage,
  deleteProviderProfile,
  deleteSetting,
  deleteSoul,
  exportLlmPayloadHistory,
  exportVisibleChatLog,
  getSetting,
  getSoul,
  listProviderProfiles,
  listAssistantMessageVariants,
  listConversationMessages,
  listSettings,
  listSouls,
  listenApiStream,
  listenChatMessageSaved,
  listenDevLog,
  previewApiPayload,
  runConsolidation,
  saveSettingFile,
  saveSoulFile,
  selectAssistantMessageVariant,
  sendApiTurn,
  sendMockTurn,
  updateUserMessage,
  upsertProviderProfile,
  upsertSetting,
  upsertSoul,
} from "./tauri";

const DEFAULT_CONVERSATION_ID = "local-mock";
const CONSOLIDATION_INTERVAL_TURNS = 10;
const NARRATOR_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:narrator_provider_profile_id";
const UPDATER_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:state_updater_provider_profile_id";
const USE_NARRATOR_FOR_UPDATER_STORAGE_KEY = "mnemosyne:use_narrator_provider_for_updater";
const DEV_LOG_LIMIT = 1000;
const DEV_LOG_CATEGORIES: DevLogCategory[] = [
  "app",
  "db",
  "api",
  "narrator",
  "state_updater",
  "context",
  "stream",
  "error",
  "warning",
  "success",
];
const DEV_LOG_LEVELS: DevLogLevel[] = ["info", "warn", "error", "debug", "success"];
type ProviderKind = "Mock" | "API";
type NarrativeMode = "Realistic" | "Reader" | "God" | "Custom";
type AppView = "library" | "chat";
type ChatStartMode = "continue" | "fresh";
type ActiveGeneration = {
  id: number;
  conversationId: string;
  narratorSaved: boolean;
  knownAssistantIds: Set<number>;
  replacementAssistantId?: number;
  replacementOriginalContent?: string;
};
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
  const [variantsByMessage, setVariantsByMessage] = useState<Record<number, AssistantMessageVariant[]>>({});
  const [context, setContext] = useState<ContextPreview | null>(null);
  const [llmPayload, setLlmPayload] = useState<LlmPayloadPreview | null>(null);
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
  const [contextMode, setContextMode] = useState<ContextMode>("brief");
  const [providerProfiles, setProviderProfiles] = useState<ProviderProfile[]>([]);
  const [narratorProviderProfileName, setNarratorProviderProfileName] = useState("Narrator API");
  const [updaterProviderProfileName, setUpdaterProviderProfileName] = useState("Updater API");
  const [selectedProviderProfileId, setSelectedProviderProfileId] = useState(() =>
    localStorage.getItem(NARRATOR_PROVIDER_PROFILE_STORAGE_KEY) ?? "",
  );
  const [selectedStateUpdaterProfileId, setSelectedStateUpdaterProfileId] = useState(() =>
    localStorage.getItem(UPDATER_PROVIDER_PROFILE_STORAGE_KEY) ?? "",
  );
  const [useNarratorProviderForUpdater, setUseNarratorProviderForUpdater] = useState(
    () => localStorage.getItem(USE_NARRATOR_FOR_UPDATER_STORAGE_KEY) !== "false",
  );
  const [apiSettings, setApiSettings] = useState<ApiProviderSettings>({
    base_url: "https://api.openai.com/v1",
    api_key: "",
    model: "",
    system_prompt: "",
  });
  const [stateUpdaterSettings, setStateUpdaterSettings] = useState<ApiProviderSettings>({
    base_url: "https://api.openai.com/v1",
    api_key: "",
    model: "",
    system_prompt: "",
  });
  const [lastTurnDebug, setLastTurnDebug] = useState<TurnDebug | null>(null);
  const [view, setView] = useState<AppView>("library");
  const [chatStartMode, setChatStartMode] = useState<ChatStartMode>("continue");
  const [sessionContinuityLabel, setSessionContinuityLabel] = useState("Using persistent Soul continuity");
  const [status, setStatus] = useState("Ready");
  const [payloadCopied, setPayloadCopied] = useState(false);
  const [exportFeedback, setExportFeedback] = useState("");
  const [devConsoleOpen, setDevConsoleOpen] = useState(false);
  const [devLogs, setDevLogs] = useState<DevLogEntry[]>([]);
  const [devConsolePaused, setDevConsolePaused] = useState(false);
  const [devLogLevelFilter, setDevLogLevelFilter] = useState<DevLogLevel | "all">("all");
  const [devLogCategoryFilter, setDevLogCategoryFilter] = useState<DevLogCategory | "all">("all");
  const [busy, setBusy] = useState(false);
  const [stateUpdating, setStateUpdating] = useState(false);
  const didBootstrap = useRef(false);
  const importInputRef = useRef<HTMLInputElement>(null);
  const settingImportInputRef = useRef<HTMLInputElement>(null);
  const generationAbortRef = useRef<AbortController | null>(null);
  const generationIdRef = useRef(0);
  const activeGenerationRef = useRef<ActiveGeneration | null>(null);
  const devConsoleBodyRef = useRef<HTMLDivElement>(null);
  const currentConversationId = useMemo(
    () =>
      soul && setting
        ? conversationIdForSettingAndSoul(setting.setting_id, soul.character_id)
        : DEFAULT_CONVERSATION_ID,
    [setting?.setting_id, soul?.character_id],
  );
  const currentConversationIdRef = useRef(currentConversationId);

  useEffect(() => {
    currentConversationIdRef.current = currentConversationId;
  }, [currentConversationId]);

  function appendDevLog(entry: DevLogEntry) {
    setDevLogs((current) => [...current, sanitizeDevLogEntry(entry)].slice(-DEV_LOG_LIMIT));
  }

  function logDev(
    level: DevLogLevel,
    category: DevLogCategory,
    message: string,
    details?: Record<string, unknown>,
  ) {
    appendDevLog(makeDevLogEntry(level, category, message, details));
  }

  function reportError(error: unknown, message = "Operation failed", category: DevLogCategory = "error") {
    const errorMessage = error instanceof Error ? error.message : String(error);
    setStatus(errorMessage);
    logDev("error", category, message, { error: errorMessage });
  }

  useEffect(() => {
    if (didBootstrap.current) return;
    didBootstrap.current = true;
    void bootstrap();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenDevLog((payload) => {
      appendDevLog(payload);
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!devConsoleOpen || devConsolePaused) return;
    const body = devConsoleBodyRef.current;
    if (!body) return;
    body.scrollTop = body.scrollHeight;
  }, [devLogs, devConsoleOpen, devConsolePaused]);

  useEffect(() => {
    localStorage.setItem(NARRATOR_PROVIDER_PROFILE_STORAGE_KEY, selectedProviderProfileId);
  }, [selectedProviderProfileId]);

  useEffect(() => {
    localStorage.setItem(UPDATER_PROVIDER_PROFILE_STORAGE_KEY, selectedStateUpdaterProfileId);
  }, [selectedStateUpdaterProfileId]);

  useEffect(() => {
    localStorage.setItem(
      USE_NARRATOR_FOR_UPDATER_STORAGE_KEY,
      useNarratorProviderForUpdater ? "true" : "false",
    );
  }, [useNarratorProviderForUpdater]);

  useEffect(() => {
    if (!soul) return;
    void refreshContext(soul.character_id, currentConversationId);
  }, [soul?.character_id, currentConversationId, messages.length]);

  useEffect(() => {
    void refreshAssistantVariants(currentConversationId, messages);
  }, [currentConversationId, messages]);

  useEffect(() => {
    if (!soul) {
      setLlmPayload(null);
      return;
    }
    const payloadUserText =
      draft.trim() || [...messages].reverse().find((message) => message.role === "user")?.content || "";
    void refreshLlmPayload(soul.character_id, currentConversationId, payloadUserText);
  }, [
    soul?.character_id,
    currentConversationId,
    messages.length,
    draft,
    provider,
    mode,
    apiSettings.base_url,
    apiSettings.model,
    apiSettings.system_prompt,
    stateUpdaterSettings.base_url,
    stateUpdaterSettings.model,
    contextMode,
  ]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenApiStream((payload) => {
      if (payload.conversation_id !== currentConversationIdRef.current) return;
      const activeGeneration = activeGenerationRef.current;
      if (!activeGeneration || activeGeneration.conversationId !== payload.conversation_id) return;
      if (activeGeneration.narratorSaved) return;
      setMessages((current) => appendStreamingChunk(current, payload.conversation_id, payload.chunk));
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenChatMessageSaved((payload) => {
      if (payload.conversation_id !== currentConversationIdRef.current) return;
      setMessages((current) => upsertSavedChatMessage(current, payload.message));
      const activeGeneration = activeGenerationRef.current;
      if (activeGeneration?.conversationId === payload.conversation_id) {
        activeGeneration.narratorSaved = true;
        setBusy(false);
        setStateUpdating(true);
        setStatus("Updating state...");
        logDev("success", "narrator", "Saved narrator message displayed", {
          conversation_id: payload.conversation_id,
          assistant_message_id: payload.message.id,
        });
        void reloadSavedNarratorMessages(payload.conversation_id);
      }
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

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

  function applyNarratorProviderProfile(profile: ProviderProfile) {
    setNarratorProviderProfileName(profile.name);
    setApiSettings({
      base_url: profile.base_url,
      api_key: profile.api_key,
      model: profile.model,
      system_prompt: profile.system_prompt,
    });
  }

  function applyStateUpdaterProviderProfile(profile: ProviderProfile) {
    setUpdaterProviderProfileName(profile.name);
    setStateUpdaterSettings({
      base_url: profile.base_url,
      api_key: profile.api_key,
      model: profile.model,
      system_prompt: profile.system_prompt,
    });
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
    const [existingSouls, existingSettings, existingProviderProfiles] = await Promise.all([
      listSouls(),
      listSettings(),
      listProviderProfiles(),
    ]);
    setSouls(existingSouls);
    setSettings(existingSettings);
    setProviderProfiles(existingProviderProfiles);
    if (existingProviderProfiles.length > 0) {
      const savedNarratorId = localStorage.getItem(NARRATOR_PROVIDER_PROFILE_STORAGE_KEY) ?? "";
      const savedUpdaterId = localStorage.getItem(UPDATER_PROVIDER_PROFILE_STORAGE_KEY) ?? "";
      const narratorProfile =
        existingProviderProfiles.find((profile) => profile.id === savedNarratorId) ??
        existingProviderProfiles[0];
      const updaterProfile = existingProviderProfiles.find((profile) => profile.id === savedUpdaterId);
      setSelectedProviderProfileId(narratorProfile.id);
      applyNarratorProviderProfile(narratorProfile);
      if (updaterProfile) {
        setSelectedStateUpdaterProfileId(updaterProfile.id);
        applyStateUpdaterProviderProfile(updaterProfile);
      }
    }

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

  async function refreshAssistantVariants(conversationId: string, nextMessages = messages) {
    const assistantMessages = nextMessages.filter((message) => message.role === "assistant");
    if (!assistantMessages.length) {
      setVariantsByMessage({});
      return;
    }
    try {
      const entries = await Promise.all(
        assistantMessages.map(async (message) => [
          message.id,
          await listAssistantMessageVariants(conversationId, message.id),
        ] as const),
      );
      setVariantsByMessage(Object.fromEntries(entries));
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function refreshLlmPayload(soulId: string, conversationId: string, userText: string) {
    try {
      const preview = await previewApiPayload(
        conversationId,
        soulId,
        userText,
        mode,
        apiSettings,
        provider,
        contextMode,
      );
      setLlmPayload(preview);
    } catch {
      setLlmPayload(null);
    }
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

  async function executeTurn(
    text: string,
    statusLabel?: string,
    replacementAssistantId?: number,
    correctionInstruction?: string,
  ) {
    if (!text || busy || stateUpdating || !soul) return;
    const generationId = generationIdRef.current + 1;
    const turnConversationId = currentConversationId;
    const replacementOriginalContent = replacementAssistantId
      ? messages.find(
          (message) =>
            message.conversation_id === turnConversationId &&
            message.id === replacementAssistantId &&
            message.role === "assistant",
        )?.content
      : undefined;
    generationIdRef.current = generationId;
    activeGenerationRef.current = {
      id: generationId,
      conversationId: turnConversationId,
      narratorSaved: false,
      knownAssistantIds: new Set(
        messages
          .filter((message) => message.conversation_id === turnConversationId && message.role === "assistant" && message.id > 0)
          .map((message) => message.id),
      ),
      replacementAssistantId,
      replacementOriginalContent,
    };
    const abortController = new AbortController();
    generationAbortRef.current = abortController;
    setBusy(true);
    setStateUpdating(false);
    setStatus(statusLabel ?? (provider === "API" ? "API provider thinking" : "Mock provider thinking"));
    logDev("info", "app", "Generation started", {
      conversation_id: turnConversationId,
      provider,
      mode,
      context_mode: contextMode,
      replacement_assistant_id: replacementAssistantId ?? null,
    });
    let savedMessagePollId: number | undefined;

    const pollSavedNarrator = async () => {
      const activeGeneration = activeGenerationRef.current;
      if (!activeGeneration || activeGeneration.id !== generationId || activeGeneration.narratorSaved) return;
      try {
        const nextMessages = await listConversationMessages(turnConversationId);
        if (turnConversationId !== currentConversationIdRef.current) return;
        if (!hasSavedAssistantForGeneration(nextMessages, activeGeneration)) return;
        activeGeneration.narratorSaved = true;
        setMessages((current) => reconcilePersistedMessages(current, nextMessages));
        setBusy(false);
        setStateUpdating(true);
        setStatus("Updating state...");
        logDev("info", "db", "Saved narrator message reloaded by fallback poll", {
          conversation_id: turnConversationId,
        });
      } catch {
        // The final sendApiTurn result is still the authoritative fallback.
      }
    };

    try {
      const activeSetting = await persistCurrentSetting();
      const activeSoul = activeSetting ? mirrorSettingIntoSoul(soul, activeSetting) : soul;
      await upsertSoul(activeSoul);
      if (provider === "API") {
        setMessages((current) =>
          seedStreamingTurn(current, turnConversationId, text, replacementAssistantId),
        );
        savedMessagePollId = window.setInterval(() => {
          void pollSavedNarrator();
        }, 700);
      }
      const result =
        provider === "API"
          ? await sendApiTurn(
              turnConversationId,
              activeSoul.character_id,
              text,
              mode,
              apiSettings,
              useNarratorProviderForUpdater ? apiSettings : stateUpdaterSettings,
              contextMode,
              abortController.signal,
              replacementAssistantId,
              correctionInstruction,
            )
          : await sendMockTurn(
              turnConversationId,
              activeSoul.character_id,
              text,
              mode,
              replacementAssistantId,
              correctionInstruction,
            );
      if (generationIdRef.current !== generationId || abortController.signal.aborted) {
        return;
      }
      if (result.conversation_id !== currentConversationIdRef.current) {
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
      setStateUpdating(false);
      setStatus(
        result.debug.state_updater_status.startsWith("failed")
          ? "Turn saved; state updater failed"
          : result.consolidation_ran
            ? "Turn saved; consolidation ran"
            : "Turn saved",
      );
      logDev(
        result.debug.state_updater_status.startsWith("failed") ? "warn" : "success",
        result.debug.state_updater_status.startsWith("failed") ? "warning" : "success",
        "Generation turn complete",
        {
          conversation_id: result.conversation_id,
          assistant_message_id: result.debug.assistant_message_id,
          selected_variant_id: result.debug.selected_variant_id,
          state_updater_status: result.debug.state_updater_status,
        },
      );
    } catch (error) {
      const activeGeneration = activeGenerationRef.current;
      const narratorWasSaved =
        activeGeneration?.id === generationId && activeGeneration.narratorSaved;
      setStateUpdating(false);
      if (provider === "API" && !narratorWasSaved && turnConversationId === currentConversationIdRef.current) {
        try {
          setMessages(await listConversationMessages(turnConversationId));
        } catch {
          setMessages((current) =>
            clearFailedStreamingTurn(
              current,
              turnConversationId,
              replacementAssistantId,
              replacementOriginalContent,
            ),
          );
        }
      }
      if (abortController.signal.aborted) {
        setStatus("Generation stopped");
        logDev("warn", "warning", "Generation stopped", { conversation_id: turnConversationId });
      } else if (narratorWasSaved) {
        setStatus(error instanceof Error ? `State update failed; narration saved: ${error.message}` : String(error));
        logDev("error", "state_updater", "State update failed after narration save", {
          conversation_id: turnConversationId,
          error: error instanceof Error ? error.message : String(error),
        });
      } else {
        reportError(error, "Generation failed", provider === "API" ? "api" : "app");
      }
    } finally {
      if (savedMessagePollId !== undefined) {
        window.clearInterval(savedMessagePollId);
      }
      if (generationIdRef.current === generationId) {
        setBusy(false);
        setStateUpdating(false);
        generationAbortRef.current = null;
        activeGenerationRef.current = null;
      }
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const text = draft.trim();
    if (!text || busy || stateUpdating || !soul) return;

    setDraft("");
    await executeTurn(text);
  }

  async function handleCopyLlmPayload() {
    if (!llmPayload) return;
    await navigator.clipboard.writeText(formatLlmPayloadDebugBlock(llmPayload));
    setPayloadCopied(true);
    setStatus("LLM payload copied");
    logDev("info", "app", "LLM payload copied");
    window.setTimeout(() => setPayloadCopied(false), 1800);
  }

  async function handleCopyDevLogs() {
    await navigator.clipboard.writeText(formatDevLogs(devLogs));
    setStatus("Dev Console logs copied");
    logDev("info", "app", "Dev Console logs copied", { entries: devLogs.length });
  }

  function handleClearDevLogs() {
    setDevLogs([]);
    setStatus("Dev Console cleared");
  }

  function handleExportDevLogs() {
    const content = formatDevLogs(devLogs);
    const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `mnemosyne-dev-console-${Date.now()}.log`;
    link.click();
    URL.revokeObjectURL(url);
    setStatus("Dev Console logs exported");
    logDev("info", "app", "Dev Console logs exported", { entries: devLogs.length });
  }

  async function handleExportVisibleChatLog() {
    if (!currentConversationId) return;
    setBusy(true);
    try {
      logDev("info", "app", "Visible chat export started", {
        conversation_id: currentConversationId,
      });
      const result = await exportVisibleChatLog(currentConversationId);
      const message = `${result.message} ${result.path}`;
      setExportFeedback(message);
      setStatus(message);
      logDev("success", "app", "Visible chat export finished", {
        conversation_id: currentConversationId,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setExportFeedback(message);
      reportError(error, "Visible chat export failed", "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleExportLlmPayloadHistory() {
    if (!currentConversationId) return;
    setBusy(true);
    try {
      logDev("info", "app", "LLM payload history export started", {
        conversation_id: currentConversationId,
      });
      const result = await exportLlmPayloadHistory(currentConversationId);
      const message = `${result.message} ${result.path}`;
      setExportFeedback(message);
      setStatus(message);
      logDev("success", "app", "LLM payload history export finished", {
        conversation_id: currentConversationId,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setExportFeedback(message);
      reportError(error, "LLM payload history export failed", "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleStartChat() {
    if (!soul || busy) return;
    if (chatStartMode === "continue") {
      setSessionContinuityLabel("Using persistent Soul continuity");
      setView("chat");
      return;
    }

    setBusy(true);
    try {
      const freshSoul = await createFreshScenarioSoul(soul.character_id, setting?.setting_id);
      const nextConversationId = setting
        ? conversationIdForSettingAndSoul(setting.setting_id, freshSoul.character_id)
        : conversationIdForSoul(freshSoul.character_id);
      setSoul(freshSoul);
      setCreatorFieldsFromSoul(freshSoul);
      setMessages(await listConversationMessages(nextConversationId));
      setContext(await compileContext(freshSoul.character_id, nextConversationId));
      setSouls(await listSouls());
      setLastTurnDebug(null);
      setSessionContinuityLabel("Fresh scenario state");
      setStatus("Started fresh scenario state; original Soul continuity was preserved.");
      setView("chat");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleRegenerate() {
    if (busy || stateUpdating || !soul) return;
    const lastUserMessage = [...activeMessages].reverse().find((message) => message.role === "user");
    if (!lastUserMessage) {
      setStatus("No user message to regenerate");
      return;
    }
    const lastAssistantMessage = [...activeMessages]
      .reverse()
      .find((message) => message.role === "assistant" && message.id > lastUserMessage.id);
    await executeTurn(
      lastUserMessage.content,
      "Regenerating last turn",
      lastAssistantMessage?.id,
    );
  }

  async function handleRegenerateFromMessage(message: ChatMessage) {
    if (busy || stateUpdating || !soul || message.role !== "assistant") return;
    if (message.id !== latestAssistantMessageId) {
      setStatus("Regenerating older messages requires branch rewind and will be added later.");
      return;
    }
    const messageIndex = activeMessages.findIndex((item) => item.id === message.id);
    const previousUserMessage = activeMessages
      .slice(0, messageIndex)
      .reverse()
      .find((item) => item.role === "user");

    if (!previousUserMessage) {
      setStatus("No user prompt found for this response");
      return;
    }

    await executeTurn(previousUserMessage.content, "Regenerating response", message.id);
  }

  async function handleFixWithInstruction(message: ChatMessage) {
    if (busy || stateUpdating || !soul || message.role !== "assistant") return;
    if (message.id !== latestAssistantMessageId) {
      setStatus("Regenerating older messages requires branch rewind and will be added later.");
      return;
    }
    const messageIndex = activeMessages.findIndex((item) => item.id === message.id);
    const previousUserMessage = activeMessages
      .slice(0, messageIndex)
      .reverse()
      .find((item) => item.role === "user");
    if (!previousUserMessage) {
      setStatus("No user prompt found for this response");
      return;
    }
    const instruction = window
      .prompt(
        "Correction instruction for next response:",
        "Continue from the kitchen. Do not replay the phone reveal.",
      )
      ?.trim();
    if (!instruction) return;
    await executeTurn(previousUserMessage.content, "Applying fix instruction", message.id, instruction);
  }

  async function handleDeleteChatMessage(message: ChatMessage) {
    if (busy || stateUpdating) return;
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

  async function handleEditUserMessage(message: ChatMessage) {
    if (busy || stateUpdating || message.role !== "user") return;
    const nextContent = window.prompt(
      "Edit user message. Soul memory and later responses are not rewound.",
      message.content,
    );
    if (nextContent === null) return;
    const trimmed = nextContent.trim();
    if (!trimmed || trimmed === message.content) return;

    setBusy(true);
    try {
      const nextMessages = await updateUserMessage(message.conversation_id, message.id, trimmed);
      setMessages(nextMessages);
      if (soul) {
        setContext(await compileContext(soul.character_id, message.conversation_id));
      }
      setStatus("User message edited");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSelectVariant(message: ChatMessage, direction: -1 | 1) {
    if (busy || stateUpdating || message.role !== "assistant") return;
    const variants = variantsByMessage[message.id] ?? [];
    if (variants.length <= 1) return;
    const selectedIndex = selectedVariantIndex(variants);
    const nextIndex = selectedIndex + direction;
    const nextVariant = variants[nextIndex];
    if (!nextVariant?.id) return;

    setBusy(true);
    try {
      const result = await selectAssistantMessageVariant(
        message.conversation_id,
        message.id,
        nextVariant.id,
      );
      setMessages(result.messages);
      setVariantsByMessage((current) => ({ ...current, [message.id]: result.variants }));
      if (soul) {
        setContext(await compileContext(soul.character_id, message.conversation_id));
      }
      setStatus(`Selected response variant ${nextIndex + 1} / ${variants.length}`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  function handleStopGeneration() {
    generationAbortRef.current?.abort();
    generationIdRef.current += 1;
    activeGenerationRef.current = null;
    generationAbortRef.current = null;
    setBusy(false);
    setStateUpdating(false);
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

  async function handleSelectProviderProfile(profileId: string) {
    setSelectedProviderProfileId(profileId);
    const profile = providerProfiles.find((item) => item.id === profileId);
    if (!profile) return;
    applyNarratorProviderProfile(profile);
    setStatus(`Loaded narrator profile ${profile.name}`);
    logDev("info", "app", "Narrator provider profile selected", {
      profile: profile.name,
      model: profile.model,
      base_url: profile.base_url,
    });
  }

  async function handleSelectStateUpdaterProfile(profileId: string) {
    setSelectedStateUpdaterProfileId(profileId);
    const profile = providerProfiles.find((item) => item.id === profileId);
    if (!profile) return;
    applyStateUpdaterProviderProfile(profile);
    setStatus(`Loaded state updater profile ${profile.name}`);
    logDev("info", "app", "State updater provider profile selected", {
      profile: profile.name,
      model: profile.model,
      base_url: profile.base_url,
    });
  }

  async function handleSaveNarratorProviderProfile() {
    if (busy) return;
    const trimmedName = narratorProviderProfileName.trim() || "Narrator API";
    const profile: ProviderProfile = {
      id: selectedProviderProfileId || crypto.randomUUID(),
      name: trimmedName,
      base_url: apiSettings.base_url,
      api_key: apiSettings.api_key,
      model: apiSettings.model,
      system_prompt: apiSettings.system_prompt,
      created_at: 0,
      updated_at: 0,
    };
    try {
      const saved = await upsertProviderProfile(profile);
      setSelectedProviderProfileId(saved.id);
      setNarratorProviderProfileName(saved.name);
      setProviderProfiles(await listProviderProfiles());
      setStatus(`Saved narrator profile ${saved.name}`);
      logDev("success", "app", "Narrator provider profile saved", {
        profile: saved.name,
        model: saved.model,
        base_url: saved.base_url,
      });
    } catch (error) {
      reportError(error, "Narrator provider profile save failed", "error");
    }
  }

  async function reloadSavedNarratorMessages(conversationId: string) {
    try {
      const nextMessages = await listConversationMessages(conversationId);
      if (conversationId !== currentConversationIdRef.current) return;
      setMessages((current) => reconcilePersistedMessages(current, nextMessages));
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleSaveStateUpdaterProviderProfile() {
    if (busy) return;
    const trimmedName = updaterProviderProfileName.trim() || "Updater API";
    const profile: ProviderProfile = {
      id: selectedStateUpdaterProfileId || crypto.randomUUID(),
      name: trimmedName,
      base_url: stateUpdaterSettings.base_url,
      api_key: stateUpdaterSettings.api_key,
      model: stateUpdaterSettings.model,
      system_prompt: stateUpdaterSettings.system_prompt,
      created_at: 0,
      updated_at: 0,
    };
    try {
      const saved = await upsertProviderProfile(profile);
      setSelectedStateUpdaterProfileId(saved.id);
      setUpdaterProviderProfileName(saved.name);
      setProviderProfiles(await listProviderProfiles());
      setStatus(`Saved state updater profile ${saved.name}`);
      logDev("success", "app", "State updater provider profile saved", {
        profile: saved.name,
        model: saved.model,
        base_url: saved.base_url,
      });
    } catch (error) {
      reportError(error, "State updater provider profile save failed", "error");
    }
  }

  async function handleDeleteNarratorProviderProfile() {
    if (busy || !selectedProviderProfileId) return;
    try {
      await deleteProviderProfile(selectedProviderProfileId);
      setSelectedProviderProfileId("");
      setNarratorProviderProfileName("Narrator API");
      setProviderProfiles(await listProviderProfiles());
      setStatus("Narrator profile deleted");
      logDev("warn", "warning", "Narrator provider profile deleted");
    } catch (error) {
      reportError(error, "Narrator provider profile delete failed", "error");
    }
  }

  async function handleDeleteStateUpdaterProviderProfile() {
    if (busy || !selectedStateUpdaterProfileId) return;
    try {
      await deleteProviderProfile(selectedStateUpdaterProfileId);
      setSelectedStateUpdaterProfileId("");
      setUpdaterProviderProfileName("Updater API");
      setProviderProfiles(await listProviderProfiles());
      setStatus("State updater profile deleted");
      logDev("warn", "warning", "State updater provider profile deleted");
    } catch (error) {
      reportError(error, "State updater provider profile delete failed", "error");
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
  const latestAssistantMessageId = useMemo(
    () =>
      [...activeMessages].reverse().find((message) => message.role === "assistant")?.id ?? null,
    [activeMessages],
  );
  const turnInProgress = busy || stateUpdating;
  const filteredDevLogs = useMemo(
    () =>
      devLogs.filter(
        (entry) =>
          (devLogLevelFilter === "all" || entry.level === devLogLevelFilter) &&
          (devLogCategoryFilter === "all" || entry.category === devLogCategoryFilter),
      ),
    [devLogs, devLogLevelFilter, devLogCategoryFilter],
  );
  const devConsole = (
    <>
      <button
        type="button"
        className={`dev-console-toggle ${devConsoleOpen ? "open" : ""}`}
        onClick={() => {
          setDevConsoleOpen((open) => !open);
          logDev("info", "app", devConsoleOpen ? "Dev Console closed" : "Dev Console opened");
        }}
      >
        <Terminal size={16} />
        <span>Dev Console</span>
        <strong>{devLogs.length}</strong>
      </button>
      {devConsoleOpen ? (
        <aside className="dev-console-panel" aria-label="Dev Console">
          <header className="dev-console-header">
            <div>
              <span className="eyebrow">Live terminal</span>
              <h2>Dev Console</h2>
            </div>
            <button type="button" onClick={() => setDevConsoleOpen(false)}>
              Close
            </button>
          </header>
          <div className="dev-console-controls">
            <label>
              <span>Level</span>
              <select
                value={devLogLevelFilter}
                onChange={(event) => setDevLogLevelFilter(event.target.value as DevLogLevel | "all")}
              >
                <option value="all">All</option>
                {DEV_LOG_LEVELS.map((level) => (
                  <option key={level} value={level}>
                    {level}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Category</span>
              <select
                value={devLogCategoryFilter}
                onChange={(event) =>
                  setDevLogCategoryFilter(event.target.value as DevLogCategory | "all")
                }
              >
                <option value="all">All</option>
                {DEV_LOG_CATEGORIES.map((category) => (
                  <option key={category} value={category}>
                    {category}
                  </option>
                ))}
              </select>
            </label>
            <label className="dev-console-checkbox">
              <input
                type="checkbox"
                checked={devConsolePaused}
                onChange={(event) => setDevConsolePaused(event.target.checked)}
              />
              <span>Pause scroll</span>
            </label>
          </div>
          <div className="dev-console-actions">
            <button type="button" onClick={handleCopyDevLogs} disabled={devLogs.length === 0}>
              <Clipboard size={14} />
              <span>Copy</span>
            </button>
            <button type="button" onClick={handleExportDevLogs} disabled={devLogs.length === 0}>
              <FileDown size={14} />
              <span>Export</span>
            </button>
            <button type="button" onClick={handleClearDevLogs} disabled={devLogs.length === 0}>
              <Trash2 size={14} />
              <span>Clear</span>
            </button>
          </div>
          <div className="dev-console-body" ref={devConsoleBodyRef}>
            {filteredDevLogs.length === 0 ? (
              <p className="dev-console-empty">No logs.</p>
            ) : (
              filteredDevLogs.map((entry) => (
                <article className={`dev-log-entry ${entry.level}`} key={entry.id}>
                  <div className="dev-log-line">
                    <time>{formatDevLogTimestamp(entry.timestamp)}</time>
                    <span className={`dev-log-level ${entry.level}`}>{entry.level}</span>
                    <span className="dev-log-category">{entry.category}</span>
                    <span className="dev-log-message">{entry.message}</span>
                  </div>
                  {entry.details && Object.keys(entry.details).length > 0 ? (
                    <pre>{JSON.stringify(entry.details, null, 2)}</pre>
                  ) : null}
                </article>
              ))
            )}
          </div>
        </aside>
      ) : null}
    </>
  );

  if (view === "chat") {
    return (
      <main className="chat-only-shell">
        <header className="chat-only-header">
          <button className="ghost-action" onClick={() => setView("library")}>
            <ArrowLeft size={18} />
            <span>Library</span>
          </button>
          <div>
            <span className="eyebrow">
              {setting?.setting_name ?? "Local Setting"} / {provider} / {mode}
            </span>
            <h1>{soul?.character_name ?? "Mnemosyne"}</h1>
            <p className="session-state-label">{sessionContinuityLabel}</p>
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
            activeMessages.map((message) => {
              const variants = variantsByMessage[message.id] ?? [];
              const selectedIndex = selectedVariantIndex(variants);
              const canGenerate = message.id === latestAssistantMessageId;
              const olderGenerationTitle =
                "Regenerating older messages requires branch rewind and will be added later.";

              return (
                <article className={`message ${message.role}`} key={message.id}>
                  <div className="message-heading">
                    <span>{message.role === "user" ? "User" : "Narrator"}</span>
                    {message.role === "assistant" ? (
                      <div className="message-tools">
                        <button
                          className="message-tool-action"
                          title={canGenerate ? "Regenerate response" : olderGenerationTitle}
                          onClick={() => handleRegenerateFromMessage(message)}
                          disabled={turnInProgress || !canGenerate}
                        >
                          <RefreshCcw size={14} />
                          <span>Regenerate</span>
                        </button>
                        <button
                          className="message-tool-action"
                          title={canGenerate ? "Fix response with instruction" : olderGenerationTitle}
                          onClick={() => handleFixWithInstruction(message)}
                          disabled={turnInProgress || !canGenerate}
                        >
                          <span>Fix</span>
                        </button>
                        <div className="variant-switcher" aria-label="Response variants">
                          <button
                            title="Previous variant"
                            onClick={() => handleSelectVariant(message, -1)}
                            disabled={turnInProgress || variants.length <= 1 || selectedIndex <= 0}
                          >
                            <ArrowLeft size={13} />
                          </button>
                          <span>
                            {variants.length ? selectedIndex + 1 : 1} / {Math.max(variants.length, 1)}
                          </span>
                          <button
                            title="Next variant"
                            onClick={() => handleSelectVariant(message, 1)}
                            disabled={
                              turnInProgress || variants.length <= 1 || selectedIndex >= variants.length - 1
                            }
                          >
                            <ArrowLeft size={13} className="next-variant-icon" />
                          </button>
                        </div>
                        <button
                          title="Delete this response"
                          onClick={() => handleDeleteChatMessage(message)}
                          disabled={turnInProgress}
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    ) : (
                      <div className="message-tools">
                        <button
                          title="Edit this message"
                          onClick={() => handleEditUserMessage(message)}
                          disabled={turnInProgress}
                        >
                          <Pencil size={14} />
                        </button>
                        <button
                          title="Delete this message"
                          onClick={() => handleDeleteChatMessage(message)}
                          disabled={turnInProgress}
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    )}
                  </div>
                  {message.role === "assistant" ? (
                    <pre style={{ whiteSpace: "pre-wrap", margin: 0, fontFamily: "inherit" }}>
                      {normalizeAssistantDisplay(message.content)}
                    </pre>
                  ) : (
                    <p>{message.content}</p>
                  )}
                </article>
              );
            })
          )}
        </section>

        <form className="chat-only-composer" onSubmit={handleSubmit}>
          <input
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Type message..."
            disabled={turnInProgress}
          />
          {busy ? (
            <button type="button" aria-label="Stop generation" onClick={handleStopGeneration}>
              <Square size={16} />
            </button>
          ) : (
            <button aria-label="Send message" disabled={!draft.trim() || !soul || stateUpdating}>
              <Play size={18} />
            </button>
          )}
        </form>
        {devConsole}
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
              onChange={(event) => {
                const nextProvider = event.target.value as ProviderKind;
                setProvider(nextProvider);
                logDev("info", "app", "Provider mode changed", { provider: nextProvider });
              }}
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
          <label className="field">
            <span>Context Mode</span>
            <select
              value={contextMode}
              onChange={(event) => {
                const nextMode = event.target.value as ContextMode;
                setContextMode(nextMode);
                logDev("info", "context", "Context mode changed", { context_mode: nextMode });
              }}
              disabled={busy || provider !== "API"}
            >
              <option value="brief">Mnemosyne Brief</option>
              <option value="full_chat">Full Chat</option>
            </select>
          </label>
          {provider === "API" ? (
            <>
              <div className="provider-pass-card">
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
                      value={selectedProviderProfileId}
                      onChange={(event) => handleSelectProviderProfile(event.target.value)}
                      disabled={busy}
                    >
                      <option value="">Unsaved narrator profile</option>
                      {providerProfiles.map((profile) => (
                        <option key={profile.id} value={profile.id}>
                          {profile.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="field">
                    <span>Profile Name</span>
                    <input
                      value={narratorProviderProfileName}
                      onChange={(event) => setNarratorProviderProfileName(event.target.value)}
                      placeholder="Narrator API"
                      disabled={busy}
                    />
                  </label>
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
                      placeholder="Stored locally with profile"
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
                </div>
                <div className="button-row">
                  <button
                    type="button"
                    className="ghost-action"
                    onClick={handleSaveNarratorProviderProfile}
                    disabled={busy}
                  >
                    <Save size={16} />
                    <span>Save Narrator Profile</span>
                  </button>
                  <button
                    type="button"
                    className="ghost-action"
                    onClick={handleDeleteNarratorProviderProfile}
                    disabled={busy || !selectedProviderProfileId}
                  >
                    <Trash2 size={16} />
                    <span>Delete</span>
                  </button>
                </div>
              </div>

              <div className="provider-pass-card">
                <div className="provider-pass-heading">
                  <div>
                    <h3>State Updater Provider</h3>
                    <p>State updater pass: updates Soul/World/Memory.</p>
                  </div>
                  <span className="provider-status-pill">
                    {useNarratorProviderForUpdater
                      ? "Using narrator provider"
                      : stateUpdaterSettings.model || "No model"}
                  </span>
                </div>
                <label className="toggle-row">
                  <input
                    type="checkbox"
                    checked={useNarratorProviderForUpdater}
                    onChange={(event) => setUseNarratorProviderForUpdater(event.target.checked)}
                    disabled={busy}
                  />
                  <span>Use narrator provider for state updater</span>
                </label>
                {useNarratorProviderForUpdater ? (
                  <p className="provider-note">
                    Using narrator provider: {apiSettings.base_url || "No base URL"} / {apiSettings.model || "No model"}
                  </p>
                ) : (
                  <>
                    <div className="provider-pass-grid">
                      <label className="field">
                        <span>State Updater Provider</span>
                        <select
                          value={selectedStateUpdaterProfileId}
                          onChange={(event) => handleSelectStateUpdaterProfile(event.target.value)}
                          disabled={busy}
                        >
                          <option value="">Unsaved updater profile</option>
                          {providerProfiles.map((profile) => (
                            <option key={profile.id} value={profile.id}>
                              {profile.name}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label className="field">
                        <span>Profile Name</span>
                        <input
                          value={updaterProviderProfileName}
                          onChange={(event) => setUpdaterProviderProfileName(event.target.value)}
                          placeholder="Updater API"
                          disabled={busy}
                        />
                      </label>
                      <label className="field">
                        <span>Base URL</span>
                        <input
                          value={stateUpdaterSettings.base_url}
                          onChange={(event) =>
                            setStateUpdaterSettings((current) => ({
                              ...current,
                              base_url: event.target.value,
                            }))
                          }
                          placeholder="http://localhost:11434/v1"
                          disabled={busy}
                        />
                      </label>
                      <label className="field">
                        <span>Model</span>
                        <input
                          value={stateUpdaterSettings.model}
                          onChange={(event) =>
                            setStateUpdaterSettings((current) => ({
                              ...current,
                              model: event.target.value,
                            }))
                          }
                          placeholder="Cheaper/local model"
                          disabled={busy}
                        />
                      </label>
                      <label className="field">
                        <span>API Key</span>
                        <input
                          type="password"
                          value={stateUpdaterSettings.api_key}
                          onChange={(event) =>
                            setStateUpdaterSettings((current) => ({
                              ...current,
                              api_key: event.target.value,
                            }))
                          }
                          placeholder="Stored locally with profile"
                          disabled={busy}
                        />
                      </label>
                    </div>
                    <div className="button-row">
                      <button
                        type="button"
                        className="ghost-action"
                        onClick={handleSaveStateUpdaterProviderProfile}
                        disabled={busy}
                      >
                        <Save size={16} />
                        <span>Save Updater Profile</span>
                      </button>
                      <button
                        type="button"
                        className="ghost-action"
                        onClick={handleDeleteStateUpdaterProviderProfile}
                        disabled={busy || !selectedStateUpdaterProfileId}
                      >
                        <Trash2 size={16} />
                        <span>Delete</span>
                      </button>
                    </div>
                  </>
                )}
              </div>
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
          <div className="chat-start-options" role="radiogroup" aria-label="Chat start mode">
            <label>
              <input
                type="radio"
                name="chat-start-mode"
                value="continue"
                checked={chatStartMode === "continue"}
                onChange={() => setChatStartMode("continue")}
                disabled={busy}
              />
              <span>Continue Soul continuity</span>
            </label>
            <label>
              <input
                type="radio"
                name="chat-start-mode"
                value="fresh"
                checked={chatStartMode === "fresh"}
                onChange={() => setChatStartMode("fresh")}
                disabled={busy}
              />
              <span>Start fresh scenario state</span>
            </label>
          </div>
          <p className="session-state-label">
            {chatStartMode === "fresh"
              ? "Fresh scenario state resets world, recent memories, and relationship for this new session."
              : "Using persistent Soul continuity"}
          </p>
        </div>
        <button className="start-chat-button" onClick={handleStartChat} disabled={!soul || busy}>
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
                <dt>Narration</dt>
                <dd>{lastTurnDebug?.narrator_response_saved ? "Saved" : "No turn"}</dd>
              </div>
              <div>
                <dt>Updater</dt>
                <dd>{lastTurnDebug?.state_updater_status ?? "-"}</dd>
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

          <section className="context-preview payload-inspector">
            <div className="payload-header">
              <h2>LLM Payload Inspector</h2>
              <div className="payload-actions">
                {payloadCopied ? <span className="copy-feedback">Payload copied</span> : null}
                {exportFeedback ? <span className="export-feedback">{exportFeedback}</span> : null}
                <button
                  className="ghost-action"
                  title="Copy LLM Payload"
                  onClick={handleCopyLlmPayload}
                  disabled={!llmPayload}
                >
                  <Clipboard size={16} />
                  <span>{payloadCopied ? "Copied!" : "Copy LLM Payload"}</span>
                </button>
                <button
                  className="ghost-action"
                  title="Export Visible Chat Log"
                  onClick={handleExportVisibleChatLog}
                  disabled={busy || activeMessages.length === 0}
                >
                  <FileDown size={16} />
                  <span>Export Visible Chat Log</span>
                </button>
                <button
                  className="ghost-action"
                  title="Export LLM Payload History"
                  onClick={handleExportLlmPayloadHistory}
                  disabled={busy}
                >
                  <FileDown size={16} />
                  <span>Export LLM Payload History</span>
                </button>
              </div>
            </div>
            <dl className="diagnostic-grid payload-grid">
              <div>
                <dt>Provider</dt>
                <dd>{llmPayload?.provider ?? provider}</dd>
              </div>
              <div>
                <dt>Mode</dt>
                <dd>{llmPayload?.mode ?? mode}</dd>
              </div>
              <div>
                <dt>Context Mode</dt>
                <dd>{llmPayload?.context_mode === "full_chat" ? "Full Chat" : "Mnemosyne Brief"}</dd>
              </div>
              <div>
                <dt>Payload Trim</dt>
                <dd>{llmPayload?.truncated ? "Trimmed" : "Within budget"}</dd>
              </div>
              <div>
                <dt>Model</dt>
                <dd>{llmPayload?.model || "-"}</dd>
              </div>
              <div>
                <dt>Base URL</dt>
                <dd>{llmPayload?.base_url || "-"}</dd>
              </div>
              <div>
                <dt>System Tokens</dt>
                <dd>{llmPayload?.estimated_tokens.system ?? 0}</dd>
              </div>
              <div>
                <dt>Context Tokens</dt>
                <dd>{llmPayload?.estimated_tokens.context ?? 0}</dd>
              </div>
              <div>
                <dt>User Tokens</dt>
                <dd>{llmPayload?.estimated_tokens.user ?? 0}</dd>
              </div>
              <div>
                <dt>Total Tokens</dt>
                <dd>{llmPayload?.estimated_tokens.total ?? 0}</dd>
              </div>
            </dl>
            <h3>System Message</h3>
            <pre>{llmPayload?.system_message ?? "No LLM payload compiled yet."}</pre>
            <h3>Context, already included inside System Message</h3>
            <pre>{llmPayload?.context ?? "No context compiled yet."}</pre>
            {llmPayload?.context_mode === "full_chat" ? (
              <>
                <h3>Full Chat Messages Sent</h3>
                <pre>{llmPayload.messages.map((message) => `${message.role}: ${message.content}`).join("\n\n")}</pre>
              </>
            ) : null}
            <h3>Current User Message</h3>
            <pre>{llmPayload?.user_message || "No current user message."}</pre>
          </section>

          <section className="context-preview">
            <h2>Context</h2>
            <pre>{context?.text ?? "No context compiled yet."}</pre>
          </section>
        </section>

        <footer className="status-line">{status}</footer>
      </section>
      {devConsole}
    </main>
  );
}

function formatLlmPayloadDebugBlock(payload: LlmPayloadPreview) {
  const chatMessages =
    payload.context_mode === "full_chat"
      ? `\n\n=== FULL CHAT MESSAGES SENT ===\n${payload.messages
          .map((message) => `${message.role}: ${message.content}`)
          .join("\n\n")}`
      : "";
  return `=== SYSTEM MESSAGE ===
${payload.system_message}

=== CONTEXT, already included inside SYSTEM MESSAGE ===
${payload.context}
${chatMessages}

=== USER MESSAGE ===
${payload.user_message}

=== ESTIMATED TOKENS ===
System: ${payload.estimated_tokens.system}
Context: ${payload.estimated_tokens.context}
User: ${payload.estimated_tokens.user}
Total: ${payload.estimated_tokens.total}

=== PROVIDER ===
Provider: ${payload.provider}
Mode: ${payload.mode}
Context Mode: ${payload.context_mode}
Truncated: ${payload.truncated}
Model: ${payload.model || "-"}
Base URL: ${payload.base_url || "-"}`;
}

function makeDevLogEntry(
  level: DevLogLevel,
  category: DevLogCategory,
  message: string,
  details?: Record<string, unknown>,
): DevLogEntry {
  const id =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return sanitizeDevLogEntry({
    id,
    timestamp: Math.floor(Date.now() / 1000),
    level,
    category,
    message,
    details: details ?? null,
  });
}

function sanitizeDevLogEntry(entry: DevLogEntry): DevLogEntry {
  return {
    ...entry,
    details: sanitizeDevLogDetails(entry.details) as Record<string, unknown> | null | undefined,
  };
}

function sanitizeDevLogDetails(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sanitizeDevLogDetails);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>).map(([key, nested]) => {
      const lowered = key.toLowerCase();
      const shouldRedact =
        lowered.includes("api_key") ||
        lowered === "authorization" ||
        lowered.includes("secret") ||
        lowered === "token" ||
        lowered.endsWith("_token") ||
        lowered.includes("bearer");
      return [key, shouldRedact ? "[redacted]" : sanitizeDevLogDetails(nested)];
    }),
  );
}

function formatDevLogTimestamp(timestamp: number) {
  const millis = timestamp > 1_000_000_000_000 ? timestamp : timestamp * 1000;
  return new Date(millis).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatDevLogs(logs: DevLogEntry[]) {
  return logs
    .map((entry) => {
      const details =
        entry.details && Object.keys(entry.details).length
          ? `\n${JSON.stringify(entry.details, null, 2)}`
          : "";
      return `[${formatDevLogTimestamp(entry.timestamp)}] ${entry.level.toUpperCase()} ${entry.category}: ${entry.message}${details}`;
    })
    .join("\n\n");
}

function selectedVariantIndex(variants: AssistantMessageVariant[]) {
  const index = variants.findIndex((variant) => variant.is_selected);
  return index >= 0 ? index : 0;
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

function seedStreamingTurn(
  messages: ChatMessage[],
  conversationId: string,
  userText: string,
  replacementAssistantId?: number,
) {
  const now = Math.floor(Date.now() / 1000);
  const seeded = replacementAssistantId
    ? messages.map((message) =>
        message.id === replacementAssistantId && message.role === "assistant"
          ? { ...message, content: "" }
          : message,
      )
    : [
        ...messages,
        {
          id: -Date.now(),
          conversation_id: conversationId,
          role: "user" as const,
          content: userText,
          created_at: now,
        },
        {
          id: -Date.now() - 1,
          conversation_id: conversationId,
          role: "assistant" as const,
          content: "",
          created_at: now,
        },
      ];

  if (
    replacementAssistantId &&
    !seeded.some((message) => message.id === replacementAssistantId && message.role === "assistant")
  ) {
    seeded.push({
      id: -Date.now() - 1,
      conversation_id: conversationId,
      role: "assistant",
      content: "",
      created_at: now,
    });
  }

  return seeded;
}

function appendStreamingChunk(messages: ChatMessage[], conversationId: string, chunk: string) {
  const next = [...messages];
  for (let index = next.length - 1; index >= 0; index -= 1) {
    const message = next[index];
    if (message.conversation_id === conversationId && message.role === "assistant") {
      next[index] = { ...message, content: `${message.content}${chunk}` };
      return next;
    }
  }

  next.push({
    id: -Date.now(),
    conversation_id: conversationId,
    role: "assistant",
    content: chunk,
    created_at: Math.floor(Date.now() / 1000),
  });
  return next;
}

function upsertSavedChatMessage(messages: ChatMessage[], savedMessage: ChatMessage) {
  const existingIndex = messages.findIndex(
    (message) =>
      message.conversation_id === savedMessage.conversation_id && message.id === savedMessage.id,
  );
  if (existingIndex >= 0) {
    const next = [...messages];
    next[existingIndex] = savedMessage;
    return removeDuplicateStreamingAssistants(next, savedMessage.conversation_id, savedMessage.id);
  }

  if (savedMessage.role === "assistant") {
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      const message = messages[index];
      if (
        message.conversation_id === savedMessage.conversation_id &&
        message.role === "assistant" &&
        message.id < 0
      ) {
        const next = [...messages];
        next[index] = savedMessage;
        return removeDuplicateStreamingAssistants(next, savedMessage.conversation_id, savedMessage.id);
      }
    }
  }

  return removeDuplicateStreamingAssistants(
    [...messages, savedMessage].sort((left, right) => left.id - right.id),
    savedMessage.conversation_id,
    savedMessage.id,
  );
}

function removeDuplicateStreamingAssistants(
  messages: ChatMessage[],
  conversationId: string,
  savedMessageId: number,
) {
  return messages.filter(
    (message) =>
      !(
        message.conversation_id === conversationId &&
        message.role === "assistant" &&
        message.id < 0 &&
        savedMessageId > 0
      ),
  );
}

function reconcilePersistedMessages(current: ChatMessage[], persisted: ChatMessage[]) {
  if (!persisted.length) return current;
  const persistedConversationId = persisted[0].conversation_id;
  return [
    ...current.filter((message) => message.conversation_id !== persistedConversationId),
    ...persisted,
  ].sort((left, right) => left.id - right.id);
}

function hasSavedAssistantForGeneration(messages: ChatMessage[], activeGeneration: ActiveGeneration) {
  if (activeGeneration.replacementAssistantId) {
    const replacement = messages.find(
      (message) =>
        message.id === activeGeneration.replacementAssistantId && message.role === "assistant",
    );
    return Boolean(
      replacement &&
        replacement.content.trim() &&
        replacement.content !== activeGeneration.replacementOriginalContent,
    );
  }

  return messages.some(
    (message) =>
      message.role === "assistant" &&
      message.id > 0 &&
      !activeGeneration.knownAssistantIds.has(message.id),
  );
}

function clearFailedStreamingTurn(
  messages: ChatMessage[],
  conversationId: string,
  replacementAssistantId?: number,
  replacementOriginalContent?: string,
) {
  return messages.flatMap((message) => {
    if (message.conversation_id !== conversationId || message.role !== "assistant") {
      return [message];
    }
    if (replacementAssistantId && message.id === replacementAssistantId) {
      return [{ ...message, content: replacementOriginalContent ?? message.content }];
    }
    if (message.id < 0) {
      return [];
    }
    return [message];
  });
}

function normalizeAssistantDisplay(content: string) {
  const withoutHidden = stripHiddenStateBlocks(content);
  return normalizeTrailingStatusBlock(withoutHidden);
}

function stripHiddenStateBlocks(content: string) {
  let cleaned = content;
  cleaned = cleaned.replace(/\[HIDDEN STATE\][\s\S]*?(?:\[\/HIDDEN STATE\]|$)/g, "");
  cleaned = cleaned.replace(/\[HIDDEN STATE[\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[HIDDEN_STATE\][\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[HIDDEN_STATE[\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[\/HIDDEN STATE[\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[\/HIDDEN_STATE[\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[\s*$/g, "");
  return cleaned.trimEnd();
}

function normalizeTrailingStatusBlock(content: string) {
  const statusMatch = content.match(/```status[\s\S]*?```[\t ]*$/);
  if (!statusMatch || statusMatch.index === undefined) {
    return content;
  }
  const start = statusMatch.index;
  const body = content.slice(0, start).trimEnd();
  const status = statusMatch[0].trim();
  return body ? `${body}\n\n${status}` : status;
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
