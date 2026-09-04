import {
  Archive,
  ArrowLeft,
  ArrowRight,
  Brain,
  ChevronDown,
  Clipboard,
  Database,
  FileDown,
  FolderOpen,
  FileUp,
  Home,
  Image as ImageIcon,
  MessageSquareText,
  PanelLeftClose,
  PanelLeftOpen,
  Pencil,
  Play,
  RefreshCcw,
  Save,
  Settings as SettingsIcon,
  Sparkles,
  Square,
  Terminal,
  Trash2,
  X,
} from "lucide-react";
import { ChangeEvent, FormEvent, KeyboardEvent, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  completeSlashCommandInput,
  canEnterSelectSlashSuggestion,
  filteredSlashCommands,
  nextSlashCommandIndex,
  shouldOpenSlashMenu,
} from "./slashCommands";
import {
  ApiProviderSettings,
  AssistantMessageVariant,
  BackgroundJobProgress,
  BenchmarkSettings,
  BenchmarkSummary,
  BenchmarkTarget,
  BenchmarkType,
  ChatMessage,
  ConversationSummary,
  ContextPreview,
  DevLogCategory,
  DevLogEntry,
  DevLogLevel,
  EvaluatorJob,
  LlmPayloadPreview,
  TurnPipelineTrace,
  PipelineStageTrace,
  ImageAsset,
  PlayerPersona,
  PlayerPersonaInput,
  ProviderProfile,
  SettingSoul,
  SettingSummary,
  SessionStateMap,
  Soul,
  SoulSummary,
  TurnDebug,
  ContextMode,
  clearSoulMemories,
  clearSoulProfileScenario,
  clearSoulRecentEvents,
  clearSoulWorldState,
  cancelEvaluatorJob,
  compileContext,
  curateMemory,
  createUserImageMessageFromFile,
  createDefaultSoul,
  createSessionSoulClone,
  createDefaultSetting,
  dedupeActiveAdjacentUserMessages,
  archiveConversation,
  restoreConversation,
  deleteMessage,
  deleteProviderProfile,
  archiveProviderProfile,
  restoreProviderProfile,
  listArchivedProviderProfiles,
  archiveSetting,
  restoreSetting,
  listArchivedSettings,
  deleteSoul,
  purgeSoul,
  exportLlmPayloadHistory,
  exportCharacterSoulMne,
  exportCurrentSessionCheckpointMne,
  exportScenarioBundleMne,
  exportWorldSettingMne,
  exportVisibleChatLog,
  getBranchPatchDebug,
  getEvaluatorJob,
  getLatestEvaluatorJob,
  getSetting,
  getImageAsset,
  getActivePlayerPersona,
  getSoul,
  inspectTurnBranchIntegrity,
  importImageAssetFromFile,
  importMneBundle,
  validateMneBundle,
  previewMneImport,
  importMneAsNew,
  listConversations,
  listSessionStateMap,
  listArchivedSessions,
  openSessionDataLocation,
  listProviderProfiles,
  archivePlayerPersona,
  listPlayerPersonas,
  listArchivedPlayerPersonas,
  listAssistantMessageVariants,
  listConversationMessages,
  listSettings,
  listSouls,
  listArchivedSouls,
  listenApiStream,
  listenBackgroundJobProgress,
  listenChatMessageSaved,
  listenDevLog,
  listenEvaluatorJobStatusChanged,
  listenPipelineTraceUpdated,
  previewApiPayload,
  rebuildSessionFromLedger,
  repairAccidentalNormalSendVariants,
  renameConversation,
  restoreInactiveMessages,
  restorePlayerPersona,
  restoreSoul,
  retryEvaluatorJob,
  runConsolidation,
  saveSessionAsNewSoul,
  saveSettingFile,
  saveSoulFile,
  selectAssistantMessageVariant,
  sendApiTurn,
  sendMockTurn,
  setActivePlayerPersona,
  touchConversationAccess,
  updateUserMessage,
  upsertPlayerPersona,
  upsertProviderProfile,
  upsertSetting,
  upsertSoul,
  runBenchmark,
  prepareBenchmarkSession,
  generateBenchmarkPlayerMessage,
  seedObservableKnowledge,
  generateTraditionalRpMessage,
  benchmarkTurnSummary,
  finalizeBenchmark,
  BenchmarkSessionInit,
  BenchmarkTurnSummary,
  hideLatestBenchmarkFailedUserMessage,
  TurnResult,
  runEvaluatorContractTest,
  runSessionFormEvalBenchmark,
  SessionFormEvalReport,
  runStructuredEvaluatorDiagnostic,
  StructuredEvaluatorDiagnosticSummary,
  setActiveEvaluatorProfile,
  listenEvaluatorAutoFallbackTriggered,
  listenEvaluatorOpsRejected,
  repairEvaluatorOps,
  embeddedRepairModelStatus,
} from "./tauri";
import {
  AssetImage,
  DisclaimerScreen,
  ImagePreviewModal,
  RangeField,
  SoulAvatar,
  Stat,
} from "./components/primitives";
import { AppDialog } from "./components/dialogs";
import { useAppDialogs } from "./app/useAppDialogs";
import { ChatMoreMenu } from "./components/chat/ChatMoreMenu";
import { ChatPipelineRail } from "./components/chat/ChatPipelineRail";
import { PersonaModal } from "./components/chat/PersonaModal";
import {
  ChatComposer,
  ChatTranscript,
  ChatWorkspaceHeader,
  EvaluatorStatusBanner,
} from "./components/chat/ChatWorkspace";
import {
  DevPipelinePanel,
  DevStreamPanel,
  type DevStreamItem,
} from "./components/dev/DevRuntimePanels";
import { useDevJobs } from "./features/dev/useDevJobs";
import {
  DEV_COMMAND_OPTIONS,
  devBooleanArg,
  devNumberArg,
  devStringArg,
  parseDevCommandArgs,
  type DevCommandName,
} from "./features/dev/commands";

import {
  formatLlmPayloadDebugBlock,
  makeDevLogEntry,
  sanitizeDevLogEntry,
} from "./features/dev/logging";
import {
  AboutSettingsPanel,
  DataSettingsPanel,
  GenerationSettingsPanel,
  QuickSettingsPanel,
  ReadingSettingsPanel,
} from "./components/settings/HumanSettingsPanels";
import {
  NarratorProviderPanel,
  ProviderProfilesPanel,
  RepairModelPanel,
  StateUpdaterProviderPanel,
} from "./components/settings/ProviderSettingsPanels";
import { ChatView } from "./components/views/ChatView";
import { DevModeShell } from "./components/views/DevModeShell";
import { HomeDashboard, HomeView } from "./components/views/HomeView";
import { LibraryView } from "./components/views/LibraryView";
import { SettingsPageView } from "./components/views/SettingsPageView";
import { KnowledgeGrid } from "./components/views/KnowledgeGrid";
import { StateMapDashboard, StateMapView } from "./components/views/StateMapView";
import { getFocusableElements, useModalBehavior } from "./components/a11y";
import {
  DEFAULT_GENERATION_PREFERENCES,
  DEFAULT_READING_PREFERENCES,
  GENERATION_PRESETS,
  type ChatStartMode,
  type GenerationPreferences,
  type GenerationPresetName,
  type NarrativeMode,
  type ReadingPreferences,
  type SettingsTab,
} from "./settings/preferences";
import {
  REPAIR_MODEL_AUTO,
  REPAIR_MODEL_EMBEDDED,
  REPAIR_MODEL_EVALUATOR,
} from "./settings/provider";
import {
  appendStreamingChunk,
  clearFailedStreamingTurn,
  hasSavedAssistantForGeneration,
  prepareMessagesForRender,
  reconcilePersistedMessages,
  seedStreamingTurn,
  upsertSavedChatMessage,
  type ActiveGeneration,
  type MessageRenderTrace,
} from "./features/chat/model/messageLifecycle";
import { useChatController } from "./features/chat/useChatController";
import {
  benchmarkEvaluatorJobCompletedOrSkipped,
  benchmarkLiveUpdaterOverride,
  fallbackBenchmarkTurnSummary,
  turnResultEvaluatorCompletedOrSkipped,
} from "./features/benchmark/model/benchmarkRuntime";
import { useEmbeddedRepairModel } from "./features/settings/useEmbeddedRepairModel";
import { useProviderSettings } from "./features/settings/useProviderSettings";
import {
  useBenchmarkController,
  type BenchmarkTurnPhase,
} from "./features/benchmark/useBenchmarkController";

import {
  CUSTOM_NARRATOR_PROMPT_STORAGE_KEY,
  CHAT_START_MODE_STORAGE_KEY,
  DEFAULT_EVALUATOR_MODE,
  DEFAULT_EVALUATOR_TIMEOUT_MS,
  DEV_LOG_LIMIT,
  MAX_CONSECUTIVE_EVALUATOR_FAILURES,
  DISCLAIMER_STORAGE_KEY,
  DISCLAIMER_VERSION,
  EMBEDDED_MODEL_PATH_STORAGE_KEY,
  EVALUATOR_EXECUTION_MODE_STORAGE_KEY,
  GENERATION_PREFERENCES_STORAGE_KEY,
  NARRATOR_PROVIDER_PROFILE_STORAGE_KEY,
  READING_PREFERENCES_STORAGE_KEY,
  REPAIR_PROVIDER_PROFILE_STORAGE_KEY,
  SESSIONS_PER_PAGE,
  SETTINGS_DRAWER_OPEN_STORAGE_KEY,
  SETTINGS_DRAWER_TAB_STORAGE_KEY,
  SETTINGS_FIRST_LAUNCH_SEEN_STORAGE_KEY,
  SHOW_ARCHIVED_SESSIONS_STORAGE_KEY,
  STRUCTURED_EVALUATOR_TRANSPORT_STORAGE_KEY,
  UPDATER_PROVIDER_PROFILE_STORAGE_KEY,
  USE_NARRATOR_FOR_UPDATER_STORAGE_KEY,
  hasAcceptedDisclaimerVersion,
  loadStoredBoolean,
  loadStoredChatStartMode,
  loadStoredCustomNarratorPrompt,
  loadStoredGenerationPreferences,
  loadStoredReadingPreferences,
  loadStoredSettingsTab,
} from "./app/preferencesStorage";
import {
  evaluatorJobBannerTitle,
  evaluatorJobRefreshesState,
  evaluatorJobStatusText,
} from "./features/chat/model/evaluatorJob";
import {
  conversationIdForSettingAndSoul,
  selectedVariantIndex,
} from "./features/chat/model/conversationIdentity";
import {
  PSYCHE_PRESETS,
  cloneForUi,
  formatSnapshotTimestamp,
  normalizeWorldDraft,
  parseMarkdownSoul,
  psycheFromSoul,
  settingFromImport,
  soulFromImport,
  worldDraftFromSetting,
  type PsycheDraft,
  type PsychePresetName,
  type WorldDraft,
} from "./features/library/model/editorModel";

const DEFAULT_CONVERSATION_ID = "local-mock";
const CONSOLIDATION_INTERVAL_TURNS = 10;
const DEV_LOG_CATEGORIES: DevLogCategory[] = [
  "app",
  "db",
  "api",
  "narrator",
  "state_updater",
  "context",
  "stream",
  "performance",
  "error",
  "warning",
  "success",
];
type ProviderKind = "Mock" | "API";
type AppView = "home" | "library" | "editor" | "chat" | "statemap" | "settings";
type DisclaimerMode = "launch" | "manual" | null;
export function App() {
  const [souls, setSouls] = useState<SoulSummary[]>([]);
  const [benchmarkPlayerCharacterSoulId, setBenchmarkPlayerCharacterSoulId] = useState("");
  const [knowledgeEditorOpen, setKnowledgeEditorOpen] = useState(false);
  const [archivedSouls, setArchivedSouls] = useState<SoulSummary[]>([]);
  const [settings, setSettings] = useState<SettingSummary[]>([]);
  const [archivedSettings, setArchivedSettings] = useState<SettingSummary[]>([]);
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [sessionStateMap, setSessionStateMap] = useState<SessionStateMap | null>(null);
  async function refreshSessionStateHub() {
    try {
      setSessionStateMap(await listSessionStateMap());
    } catch (error) {
      console.error(error);
      logDev("warn", "db", "Failed to load State Map hub", { error: String(error) });
    }
  }
  async function refreshConversations() {
    try {
      const [active, archived] = await Promise.all([
        listConversations(),
        listArchivedSessions(),
      ]);
      setConversations([...active, ...archived]);
    } catch (err) {
      console.error(err);
      try {
        setConversations(await listConversations());
      } catch (_) {}
    }
    await refreshSessionStateHub();
  }
  async function refreshArchivedSettings() {
    try {
      setArchivedSettings(await listArchivedSettings());
    } catch (error) {
      logDev("warn", "db", "Failed to list archived worlds", { error: String(error) });
    }
  }
  async function refreshArchivedSouls() {
    try {
      setArchivedSouls(await listArchivedSouls());
    } catch (error) {
      logDev("warn", "db", "Failed to list archived characters", { error: String(error) });
    }
  }
  const [soul, setSoul] = useState<Soul | null>(null);
  const [setting, setSetting] = useState<SettingSoul | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [variantsByMessage, setVariantsByMessage] = useState<Record<number, AssistantMessageVariant[]>>({});
  const [context, setContext] = useState<ContextPreview | null>(null);
  const [llmPayload, setLlmPayload] = useState<LlmPayloadPreview | null>(null);
  const [draft, setDraft] = useState("");
  const [slashMenuDismissed, setSlashMenuDismissed] = useState(false);
  const [slashSelectedIndex, setSlashSelectedIndex] = useState(0);
  const [playerPersonas, setPlayerPersonas] = useState<PlayerPersona[]>([]);
  const [archivedPlayerPersonas, setArchivedPlayerPersonas] = useState<PlayerPersona[]>([]);
  const [activePlayerPersona, setActivePlayerPersonaState] = useState<PlayerPersona | null>(null);
  const [personaModalMode, setPersonaModalMode] = useState<"list" | "add" | "edit" | null>(null);
  const [personaListConfirmRequired, setPersonaListConfirmRequired] = useState(false);
  const [personaModalConversationId, setPersonaModalConversationId] = useState<string | null>(null);
  const [personaEditingId, setPersonaEditingId] = useState<string | null>(null);
  const [personaForm, setPersonaForm] = useState<PlayerPersonaInput>({
    display_name: "",
    gender_code: "custom",
    pronouns: "",
    description: "",
    appearance: "",
    notes: "",
  });
  const [characterName, setCharacterName] = useState("Aurora Schwarz");
  const [characterDescription, setCharacterDescription] = useState("");
  const [characterAppearance, setCharacterAppearance] = useState("");
  const [characterPersonality, setCharacterPersonality] = useState("");
  const [characterScenario, setCharacterScenario] = useState("");
  const [openingNarratorMessage, setOpeningNarratorMessage] = useState("");
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
  const [provider, setProvider] = useState<ProviderKind>("API");
  const [mode, setMode] = useState<NarrativeMode>("Reader");
  const [contextMode, setContextMode] = useState<ContextMode>("brief");
  const [providerProfiles, setProviderProfiles] = useState<ProviderProfile[]>([]);
  const [archivedProviderProfiles, setArchivedProviderProfiles] = useState<ProviderProfile[]>([]);
  // User-curated list of model ids, surfaced as autocomplete on every model
  // field so models can be chosen in-app without being configured elsewhere.
  const [knownModels, setKnownModels] = useState<string[]>(() => {
    try {
      const raw = localStorage.getItem("mnemosyne:known_models");
      const parsed = raw ? (JSON.parse(raw) as unknown) : null;
      return Array.isArray(parsed) ? parsed.filter((m): m is string => typeof m === "string") : [];
    } catch {
      return [];
    }
  });
  function rememberModel(id: string) {
    const trimmed = id.trim();
    if (!trimmed) return;
    setKnownModels((current) => {
      if (current.includes(trimmed)) return current;
      const next = [...current, trimmed].sort((a, b) => a.localeCompare(b));
      try {
        localStorage.setItem("mnemosyne:known_models", JSON.stringify(next));
      } catch {
        /* ignore persistence errors */
      }
      return next;
    });
  }
  useEffect(() => {
    providerProfiles.forEach((profile) => {
      if (profile.model) rememberModel(profile.model);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providerProfiles]);
  const {
    narratorProviderProfileName,
    setNarratorProviderProfileName,
    updaterProviderProfileName,
    setUpdaterProviderProfileName,
    selectedProviderProfileId,
    setSelectedProviderProfileId,
    selectedStateUpdaterProfileId,
    setSelectedStateUpdaterProfileId,
    selectedRepairProfileId,
    setSelectedRepairProfileId,
    useNarratorProviderForUpdater,
    setUseNarratorProviderForUpdater,
    devOverrideActive,
    setDevOverrideActive,
    apiSettings,
    setApiSettings,
    stateUpdaterSettings,
    setStateUpdaterSettings,
    generationPreferences,
    setGenerationPreferences,
    readingPreferences,
    setReadingPreferences,
    effectiveNarratorSettings,
    evaluatorExecutionMode,
    updateEvaluatorExecutionMode,
    structuredEvaluatorTransport,
    updateStructuredEvaluatorTransport,
  } = useProviderSettings();
  const [retryRepairBusy, setRetryRepairBusy] = useState(false);
  const [formEvalBusy, setFormEvalBusy] = useState(false);
  const [formEvalReport, setFormEvalReport] = useState<SessionFormEvalReport | null>(null);
  // The permanent dev-shell side panel toggles between dev features (default) and
  // settings, so both are reachable without leaving the session.
  const [devPanelTab, setDevPanelTab] = useState<"dev" | "settings" | "benchmarks">("dev");
  const {
    dialog: appDialog,
    resolve: resolveAppDialog,
    alert: alertDialog,
    confirm: confirmDialog,
    prompt: promptDialog,
  } = useAppDialogs();
  const [lastTurnDebug, setLastTurnDebug] = useState<TurnDebug | null>(null);
  const [view, setView] = useState<AppView>("home"); // v2 overhaul: rail-driven views
  const [railCollapsed, setRailCollapsed] = useState(false);
  const [chatStartMode, setChatStartMode] = useState<ChatStartMode>(loadStoredChatStartMode);
  const [sessionContinuityLabel, setSessionContinuityLabel] = useState("New Session starts from the selected Soul snapshot");
  const [currentSessionTitle, setCurrentSessionTitle] = useState("New Session");
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);
  const [selectedCharacterIds, setSelectedCharacterIds] = useState<string[]>([]);
  const [selectedAvatarAsset, setSelectedAvatarAsset] = useState<ImageAsset | null>(null);
  const [draftAvatarAsset, setDraftAvatarAsset] = useState<ImageAsset | null>(null);
  const [draftAvatarImageId, setDraftAvatarImageId] = useState<string | null>(null);
  const [previewImageAsset, setPreviewImageAsset] = useState<ImageAsset | null>(null);
  const [status, setStatus] = useState("Ready");
  // A decently-important failure (a provider/model call, the local model, an
  // export, etc.). Surfaced as a prominent dismissible banner with a fix hint,
  // rather than silently retrying or burying it in the status line.
  const [providerAlert, setProviderAlert] = useState<
    { title: string; detail?: string; hint?: string } | null
  >(null);
  const {
    path: embeddedModelPath,
    setPath: setEmbeddedModelPath,
    model: embeddedModel,
    busy: embeddedModelBusy,
    error: embeddedModelError,
    setError: setEmbeddedModelError,
    start: handleStartEmbeddedModel,
    stop: handleStopEmbeddedModel,
    ensureReady: ensureLocalRepairModelReady,
  } = useEmbeddedRepairModel({
    onStatus: setStatus,
    onAlert: setProviderAlert,
  });
  const [payloadCopied, setPayloadCopied] = useState(false);
  const [exportFeedback, setExportFeedback] = useState("");
  const [settingsDrawerOpen, setSettingsDrawerOpen] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>(loadStoredSettingsTab);
  const [latestPipelineTrace, setLatestPipelineTrace] = useState<TurnPipelineTrace | null>(null);
  const [devLogs, setDevLogs] = useState<DevLogEntry[]>([]);
  const [devCommandArgs, setDevCommandArgs] = useState("{}");
  const [devCommandRunning, setDevCommandRunning] = useState(false);
  const [devCommandResult, setDevCommandResult] = useState<string | null>(null);
  const [devCommandError, setDevCommandError] = useState<string | null>(null);
  const {
    jobsById: devJobsById,
    dismissedJobIds: dismissedDevJobIds,
    startJob: startTrackedDevJob,
    updateJob: updateTrackedDevJob,
    finishJob: finishTrackedDevJob,
    appendHistory: appendTrackedDevJobHistory,
    dismissJob: dismissTrackedDevJob,
    ingestJob: ingestTrackedDevJob,
    runOperation: runTrackedBackgroundOperation,
  } = useDevJobs();
  const {
    liveBenchmarkJobIdRef,
    structuredDiagnosticRunning,
    setStructuredDiagnosticRunning,
    structuredDiagnosticResult,
    setStructuredDiagnosticResult,
    structuredDiagnosticError,
    setStructuredDiagnosticError,
    benchmarkType,
    setBenchmarkType,
    benchmarkTarget,
    setBenchmarkTarget,
    benchmarkTurnCount,
    setBenchmarkTurnCount,
    benchmarkPlayerProfileId,
    setBenchmarkPlayerProfileId,
    benchmarkPlayerGoal,
    setBenchmarkPlayerGoal,
    benchmarkStrictToolEvaluator,
    setBenchmarkStrictToolEvaluator,
    benchmarkTransport,
    setBenchmarkTransport,
    benchmarkWaitForEvaluator,
    setBenchmarkWaitForEvaluator,
    benchmarkTraditionalOpponent,
    setBenchmarkTraditionalOpponent,
    benchmarkRunning,
    setBenchmarkRunning,
    benchmarkResult,
    setBenchmarkResult,
    benchmarkError,
    setBenchmarkError,
    benchmarkLiveActive,
    setBenchmarkLiveActive,
    benchmarkTurnsRemaining,
    setBenchmarkTurnsRemaining,
    benchmarkLivePhase,
    setBenchmarkLivePhase,
    benchmarkCtxRef,
    benchmarkTurnInFlightRef,
    benchmarkStopRef,
  } = useBenchmarkController();
  // Latest evaluator/repair endpoint settings, kept fresh so the background
  // op-repair listener (registered once) always uses current config.
  const repairSettingsRef = useRef<ApiProviderSettings | null>(null);
  const [disclaimerMode, setDisclaimerMode] = useState<DisclaimerMode>(() =>
    hasAcceptedDisclaimerVersion() ? null : "launch",
  );
  const [disclaimerUnderstood, setDisclaimerUnderstood] = useState(false);
  const [disclaimerRemember, setDisclaimerRemember] = useState(false);
  const [busy, setBusy] = useState(false);
  const [stateUpdating, setStateUpdating] = useState(false);
  const [activeEvaluatorJob, setActiveEvaluatorJob] = useState<EvaluatorJob | null>(null);
  const [evaluatorJobBannerDismissed, setEvaluatorJobBannerDismissed] = useState(false);
  const [showArchivedSessions, setShowArchivedSessions] = useState(() =>
    loadStoredBoolean(SHOW_ARCHIVED_SESSIONS_STORAGE_KEY, false),
  );
  const [sessionListPage, setSessionListPage] = useState(1);
  const didBootstrap = useRef(false);
  const importInputRef = useRef<HTMLInputElement>(null);
  const settingImportInputRef = useRef<HTMLInputElement>(null);
  const avatarInputRef = useRef<HTMLInputElement>(null);
  const chatImageInputRef = useRef<HTMLInputElement>(null);
  // In-session Dev Mode: a terminal-CLI re-skin of the session (matrix/phosphor).
  const [devModeActive, setDevModeActive] = useState(false);
  const [devTerminalInput, setDevTerminalInput] = useState("");
  const devStreamRef = useRef<HTMLDivElement>(null);
  const defaultConversationId = useMemo(
    () =>
      soul && setting
        ? conversationIdForSettingAndSoul(setting.setting_id, soul.character_id)
        : DEFAULT_CONVERSATION_ID,
    [setting?.setting_id, soul?.character_id],
  );
  const currentConversationId = activeConversationId ?? defaultConversationId;
  const {
    generationAbortRef,
    generationIdRef,
    activeGenerationRef,
    chatOnlyBodyRef,
    chatBottomRef,
    isPinnedToBottomRef,
    showJumpToLatest,
    setShowJumpToLatest,
    chatMoreMenuOpen,
    setChatMoreMenuOpen,
    chatMoreMenuRef,
    chatMoreButtonRef,
    personaModalRef,
    scrollChatToBottom,
    handleChatScroll,
    jumpToLatest,
  } = useChatController({
    active: view === "chat",
    conversationId: currentConversationId,
    messages,
  });
  const currentConversationIdRef = useRef(currentConversationId);
  const visibleConversations = useMemo(
    () =>
      conversations.filter((conversation) => {
        if (!soul) return false;
        const archived = Boolean(conversation.archived_at) || conversation.title.startsWith("[Archived] ");
        if (archived !== showArchivedSessions) return false;
        return (
          conversation.soul_id === soul.character_id ||
          conversation.source_savepoint_id === soul.character_id ||
          (Boolean(soul.source_savepoint_id) &&
            conversation.source_savepoint_id === soul.source_savepoint_id)
        );
      }),
    [conversations, showArchivedSessions, soul?.character_id, soul?.source_savepoint_id],
  );
  const sessionListTotalPages = Math.max(1, Math.ceil(visibleConversations.length / SESSIONS_PER_PAGE));
  const paginatedConversations = useMemo(() => {
    const start = (sessionListPage - 1) * SESSIONS_PER_PAGE;
    return visibleConversations.slice(start, start + SESSIONS_PER_PAGE);
  }, [visibleConversations, sessionListPage]);
  const selectedCharacterCount = selectedCharacterIds.filter((id) =>
    souls.some((item) => item.character_id === id),
  ).length;
  const primaryCharacterDescription =
    soul?.profile.description.trim() ||
    soul?.profile.scenario.trim() ||
    soul?.profile.personality.trim() ||
    "No description yet. Open the Character Editor to add one.";
  const selectedWorldSummary =
    setting?.world.location ||
    setting?.scenario ||
    "No world details yet. Open the World Editor to add a location and active plots.";

  useEffect(() => {
    currentConversationIdRef.current = currentConversationId;
  }, [currentConversationId]);

  // Keep the repair endpoint settings current. Repair is its own role (a light,
  // local model): if a Repair Model profile is selected it wins; otherwise fall
  // back to the evaluator/updater settings. The listener below reads this ref so
  // it never goes stale. (ProviderProfile is a superset of ApiProviderSettings;
  // the backend ignores the extra metadata fields.)
  useEffect(() => {
    const evalSettings = useNarratorProviderForUpdater ? apiSettings : stateUpdaterSettings;
    const repairProfile = selectedRepairProfileId &&
      selectedRepairProfileId !== REPAIR_MODEL_EVALUATOR &&
      selectedRepairProfileId !== REPAIR_MODEL_EMBEDDED
      ? providerProfiles.find((profile) => profile.id === selectedRepairProfileId)
      : undefined;
    const mayUseEmbeddedModel = selectedRepairProfileId !== REPAIR_MODEL_EVALUATOR;
    // Precedence: a chosen Repair profile wins; explicit/automatic embedded mode
    // uses the local model when ready; evaluator mode (and unavailable local
    // model) safely falls back to the evaluator settings.
    if (repairProfile) {
      repairSettingsRef.current = repairProfile;
    } else if (mayUseEmbeddedModel && embeddedModel.ready && embeddedModel.url) {
      repairSettingsRef.current = {
        ...evalSettings,
        base_url: embeddedModel.url,
        api_key: "local",
        model: embeddedModel.model ?? "local-model",
      };
    } else {
      repairSettingsRef.current = evalSettings;
    }
  });

  async function handleRetrySessionRepair() {
    const settings = repairSettingsRef.current;
    const conversationId = currentConversationId;
    if (!settings) {
      setStatus("No repair endpoint configured (pick a Repair Model or start the embedded model).");
      return;
    }
    if (!conversationId) {
      setStatus("Open a session first.");
      return;
    }
    setRetryRepairBusy(true);
    const repairJobId = startTrackedDevJob("repair", "Session Repair", {
      phase: "loading_turns",
      detail: "Preparing active assistant turns for re-extraction",
    });
    try {
      const targetsLocal = /\/\/(127\.0\.0\.1|localhost|0\.0\.0\.0)/i.test(settings.base_url ?? "");
      if (targetsLocal) {
        const ready = await ensureLocalRepairModelReady();
        if (!ready) {
          setProviderAlert({
            title: "Local repair model unavailable",
            detail: "Couldn't reach the local model to retry repair.",
            hint: "Start the embedded model (or run the .exe yourself) and confirm it's up, then retry.",
          });
          return;
        }
      }
      const messages = await listConversationMessages(conversationId);
      const assistantTurns = messages.filter(
        (message) => message.role === "assistant" && message.status === "active",
      );
      let fired = 0;
      updateTrackedDevJob(repairJobId, {
        total: assistantTurns.length,
        phase: "reextracting",
        detail: `${assistantTurns.length} turn(s) eligible for repair`,
      });
      for (const [index, message] of assistantTurns.entries()) {
        try {
          // reextract: re-derive durable state for the turn; turns without a
          // baseline patch (e.g. the opening message) error out and are skipped.
          await repairEvaluatorOps(conversationId, message.id, [], settings, "reextract");
          fired += 1;
          appendTrackedDevJobHistory(
            repairJobId,
            {
              index: index + 1,
              label: `Turn ${message.turn_id ?? message.id}`,
              status: "succeeded",
              detail: "Repair queued",
              elapsed_ms: null,
            },
            {
              current: index + 1,
              succeeded: fired,
              recovered: fired,
              phase: "reextracting",
            },
          );
        } catch (error) {
          // skip turns that can't be repaired
          appendTrackedDevJobHistory(
            repairJobId,
            {
              index: index + 1,
              label: `Turn ${message.turn_id ?? message.id}`,
              status: "skipped",
              detail: error instanceof Error ? error.message : String(error),
              elapsed_ms: null,
            },
            {
              current: index + 1,
              succeeded: fired,
              recovered: fired,
            },
          );
        }
      }
      setStatus(`Retry repair fired for ${fired} turn(s); state updates in the background.`);
      finishTrackedDevJob(repairJobId, "succeeded", {
        current: assistantTurns.length,
        total: assistantTurns.length,
        succeeded: fired,
        recovered: fired,
        detail: `Repair queued for ${fired}/${assistantTurns.length} turn(s)`,
      });
    } catch (error) {
      reportError(error, "Retry session repair failed", "state_updater");
      finishTrackedDevJob(repairJobId, "failed", {
        failed: 1,
        detail: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setRetryRepairBusy(false);
    }
  }

  // Auto-fire background op-repair: when the evaluator drops ops (its own
  // verdict, surfaced by the backend), retry ONLY those ops on the configured
  // endpoint, in the background. Fires after the original eval job, so there is
  // no overlap with the main turn's write. Best-effort: failures stay dropped.
  useEffect(() => {
    let active = true;
    let cleanup: (() => void) | undefined;
    void listenEvaluatorOpsRejected(async (payload) => {
      if (!active) return;
      // "reextract" carries no ops (the evaluator produced nothing usable); every
      // other kind needs at least one failed op to act on.
      const isReextract = payload.repair_kind === "reextract";
      if (!isReextract && !payload.failed_ops?.length) return;
      const settings = repairSettingsRef.current;
      if (!settings) return;
      // If repair targets the local model, verify it's actually reachable first ??
      // otherwise the call silently connection-refuses and state is dropped. Tell
      // the user to fix it in Settings instead of failing invisibly.
      const targetsLocalModel = /\/\/(127\.0\.0\.1|localhost|0\.0\.0\.0)/i.test(settings.base_url ?? "");
      if (targetsLocalModel) {
        // Auto-start the local model on demand rather than relying on the user
        // having it up; ensure it's actually answering before sending repair.
        const ready = await ensureLocalRepairModelReady();
        if (!active) return;
        if (!ready) {
          setProviderAlert({
            title: "Local repair model unavailable",
            detail: `Repair needs the local model at ${settings.base_url}, but it isn't responding and auto-start failed.`,
            hint: "Set a valid, runnable llamafile path in Settings.",
          });
          return;
        }
      }
      void repairEvaluatorOps(
        payload.conversation_id,
        payload.assistant_message_id,
        payload.failed_ops ?? [],
        settings,
        payload.repair_kind,
      ).catch(() => undefined);
    }).then((unlisten) => {
      cleanup = unlisten;
      if (!active) cleanup();
    });
    return () => {
      active = false;
      cleanup?.();
    };
  }, []);

  // Drives the live AI-vs-AI self-play loop. Each time the chat settles (not
  // busy/updating) and the benchmark conversation is active, fire the next turn
  // through the normal visible chat path so it streams in real time. Runs one
  // turn at a time, guarded by benchmarkTurnInFlightRef against re-entry.
  useEffect(() => {
    if (!benchmarkLiveActive) return;
    if (benchmarkTurnInFlightRef.current) return;
    const ctx = benchmarkCtxRef.current;
    if (!ctx) return;
    if (busy || stateUpdating) return;
    if (currentConversationId !== ctx.conversationId) return;
    if (!soul || soul.character_id !== ctx.soulId) return;
    if (benchmarkStopRef.current || benchmarkTurnsRemaining <= 0) {
      void finishBenchmarkLive();
      return;
    }
    void runOneBenchmarkTurn();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    benchmarkLiveActive,
    benchmarkTurnsRemaining,
    busy,
    stateUpdating,
    currentConversationId,
    soul,
    messages,
  ]);

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

  // Raise the prominent failure banner. `role` is a human label (e.g. "Narrator",
  // "Evaluator", "Player simulator"); the default hint points the user at Settings.
  function alertProviderFailure(role: string, detail?: string, hint?: string) {
    setProviderAlert({
      title: `${role} API failed`,
      detail,
      hint: hint ?? `Open Settings and check the ${role.toLowerCase()} model and API key.`,
    });
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
    let unlisten: (() => void) | undefined;
    void listenEvaluatorAutoFallbackTriggered((payload) => {
      void refreshConversations().then(() => {
        if (payload.conversation_id === currentConversationIdRef.current) {
          const profile = providerProfiles.find((p) => p.id === payload.profile_id);
          if (profile) {
            setSelectedStateUpdaterProfileId(profile.id);
            applyStateUpdaterProviderProfile(profile);
            setStatus(`Evaluator auto fallback triggered! Selected model "${profile.name}".`);
            logDev("warn", "state_updater", `Auto fallback triggered for conversation ${payload.conversation_id}: switched evaluator to profile "${profile.name}"`);
          }
        }
      });
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, [providerProfiles]);

  useEffect(() => {
    if (!devModeActive) return;
    const el = devStreamRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [devModeActive, devLogs, messages]);

  useEffect(() => {
    localStorage.setItem(NARRATOR_PROVIDER_PROFILE_STORAGE_KEY, selectedProviderProfileId);
  }, [selectedProviderProfileId]);

  useEffect(() => {
    localStorage.setItem(UPDATER_PROVIDER_PROFILE_STORAGE_KEY, selectedStateUpdaterProfileId);
  }, [selectedStateUpdaterProfileId]);

  useEffect(() => {
    localStorage.setItem(REPAIR_PROVIDER_PROFILE_STORAGE_KEY, selectedRepairProfileId);
  }, [selectedRepairProfileId]);

  useEffect(() => {
    localStorage.setItem(EMBEDDED_MODEL_PATH_STORAGE_KEY, embeddedModelPath);
  }, [embeddedModelPath]);

  useEffect(() => {
    localStorage.setItem(
      USE_NARRATOR_FOR_UPDATER_STORAGE_KEY,
      useNarratorProviderForUpdater ? "true" : "false",
    );
  }, [useNarratorProviderForUpdater]);

  useEffect(() => {
    localStorage.setItem(CUSTOM_NARRATOR_PROMPT_STORAGE_KEY, apiSettings.system_prompt);
  }, [apiSettings.system_prompt]);

  useEffect(() => {
    localStorage.setItem(GENERATION_PREFERENCES_STORAGE_KEY, JSON.stringify(generationPreferences));
  }, [generationPreferences]);

  useEffect(() => {
    localStorage.setItem(READING_PREFERENCES_STORAGE_KEY, JSON.stringify(readingPreferences));
    const root = document.documentElement;
    root.style.setProperty("--chat-reading-size", `${readingPreferences.proseSize}px`);
    root.style.setProperty("--chat-reading-line-height", String(readingPreferences.lineHeight));
    root.style.setProperty("--chat-reading-width", `${readingPreferences.columnWidth}px`);
    root.style.setProperty("--chat-message-gap", readingPreferences.compactSpacing ? "10px" : "16px");
    root.style.setProperty("--chat-message-padding-y", readingPreferences.compactSpacing ? "10px" : "14px");
  }, [readingPreferences]);

  useEffect(() => {
    localStorage.setItem(SETTINGS_DRAWER_TAB_STORAGE_KEY, settingsTab);
  }, [settingsTab]);

  useEffect(() => {
    localStorage.setItem(CHAT_START_MODE_STORAGE_KEY, chatStartMode);
  }, [chatStartMode]);

  useEffect(() => {
    localStorage.setItem(SHOW_ARCHIVED_SESSIONS_STORAGE_KEY, showArchivedSessions ? "true" : "false");
  }, [showArchivedSessions]);

  useEffect(() => {
    setSessionListPage(1);
  }, [showArchivedSessions, soul?.character_id]);

  useEffect(() => {
    setSessionListPage((page) => Math.min(page, sessionListTotalPages));
  }, [sessionListTotalPages]);

  useModalBehavior({
    active: Boolean(personaModalMode),
    onClose: closePersonaModal,
    panelRef: personaModalRef,
  });

  useEffect(() => {
    if (!chatMoreMenuOpen) return;
    const menu = chatMoreMenuRef.current;
    const focusItems = () => getFocusableElements(menu).filter((element) => element.getAttribute("role") === "menuitem");
    window.requestAnimationFrame(() => focusItems()[0]?.focus());
    const handleOutsideEvent = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (chatMoreMenuRef.current?.contains(target) || chatMoreButtonRef.current?.contains(target)) return;
      setChatMoreMenuOpen(false);
    };
    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setChatMoreMenuOpen(false);
        chatMoreButtonRef.current?.focus();
        return;
      }
      const items = focusItems();
      if (!items.length) return;
      const currentIndex = Math.max(0, items.findIndex((item) => item === document.activeElement));
      if (event.key === "ArrowDown") {
        event.preventDefault();
        items[(currentIndex + 1) % items.length]?.focus();
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        items[(currentIndex - 1 + items.length) % items.length]?.focus();
      } else if (event.key === "Home") {
        event.preventDefault();
        items[0]?.focus();
      } else if (event.key === "End") {
        event.preventDefault();
        items[items.length - 1]?.focus();
      }
    };
    document.addEventListener("pointerdown", handleOutsideEvent, true);
    document.addEventListener("keydown", handleKeyDown, true);
    return () => {
      document.removeEventListener("pointerdown", handleOutsideEvent, true);
      document.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [chatMoreMenuOpen]);

  useEffect(() => {
    if (!soul || view !== "chat") return;
    void refreshContext(soul.character_id, currentConversationId);
  }, [soul?.character_id, currentConversationId, messages.length, view]);

  useEffect(() => {
    void refreshAssistantVariants(currentConversationId, messages);
  }, [currentConversationId, messages]);

  useEffect(() => {
    let cancelled = false;
    const avatarId = soul?.profile.avatar_image_id;
    if (!avatarId) {
      setSelectedAvatarAsset(null);
      setDraftAvatarAsset(null);
      setDraftAvatarImageId(null);
      return;
    }
    getImageAsset(avatarId)
      .then((asset) => {
        if (!cancelled) {
          setSelectedAvatarAsset(asset);
          setDraftAvatarAsset(asset);
          setDraftAvatarImageId(asset.id);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setSelectedAvatarAsset(null);
          setDraftAvatarAsset(null);
          setDraftAvatarImageId(avatarId);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [soul?.character_id, soul?.profile.avatar_image_id]);

  useEffect(() => {
    if (!soul || view !== "chat") {
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
    generationPreferences.temperature,
    generationPreferences.topP,
    generationPreferences.frequencyPenalty,
    generationPreferences.presencePenalty,
    generationPreferences.maxTokens,
    stateUpdaterSettings.base_url,
    stateUpdaterSettings.model,
    contextMode,
    view,
  ]);

  const slashSuggestions = useMemo(() => filteredSlashCommands(draft), [draft]);
  const slashMenuOpen =
    !busy &&
    !slashMenuDismissed &&
    shouldOpenSlashMenu(draft) &&
    slashSuggestions.length > 0;
  const selectedSlashCommand =
    slashSuggestions[Math.min(slashSelectedIndex, Math.max(0, slashSuggestions.length - 1))];

  useEffect(() => {
    if (slashSelectedIndex >= slashSuggestions.length) {
      setSlashSelectedIndex(Math.max(0, slashSuggestions.length - 1));
    }
  }, [slashSelectedIndex, slashSuggestions.length]);

  const soulRef = useRef(soul);
  useEffect(() => {
    soulRef.current = soul;
  }, [soul]);

  const listenerRegistrationRef = useRef({
    apiStream: false,
    chatMessageSaved: false,
    evaluatorJobStatusChanged: false,
  });

  useEffect(() => {
    if (listenerRegistrationRef.current.apiStream) {
      logDev("debug", "stream", "Listener registration suppressed", { listener: "api_stream" });
      return;
    }
    listenerRegistrationRef.current.apiStream = true;
    let active = true;
    let cleanupFn: (() => void) | undefined;

    void listenApiStream((payload) => {
      if (!active) return;
      if (payload.conversation_id !== currentConversationIdRef.current) return;
      const activeGeneration = activeGenerationRef.current;
      if (!activeGeneration || activeGeneration.conversationId !== payload.conversation_id) return;
      if (activeGeneration.narratorSaved) return;
      setMessages((current) => appendStreamingChunk(current, payload.conversation_id, payload.chunk));
    }).then((cleanup) => {
      cleanupFn = cleanup;
      if (!active) {
        cleanup();
      }
    });

    return () => {
      active = false;
      listenerRegistrationRef.current.apiStream = false;
      if (cleanupFn) {
        cleanupFn();
      }
    };
  }, []);

  useEffect(() => {
    if (listenerRegistrationRef.current.chatMessageSaved) {
      logDev("debug", "stream", "Listener registration suppressed", { listener: "chat_message_saved" });
      return;
    }
    listenerRegistrationRef.current.chatMessageSaved = true;
    let active = true;
    let cleanupFn: (() => void) | undefined;

    void listenChatMessageSaved((payload) => {
      if (!active) return;
      if (payload.conversation_id !== currentConversationIdRef.current) return;
      setMessages((current) => {
        const result = upsertSavedChatMessage(current, payload.message);
        return result.messages;
      });
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
      cleanupFn = cleanup;
      if (!active) {
        cleanup();
      }
    });

    return () => {
      active = false;
      listenerRegistrationRef.current.chatMessageSaved = false;
      if (cleanupFn) {
        cleanupFn();
      }
    };
  }, []);

  useEffect(() => {
    if (listenerRegistrationRef.current.evaluatorJobStatusChanged) {
      logDev("debug", "stream", "Listener registration suppressed", { listener: "evaluator_job_status_changed" });
      return;
    }
    listenerRegistrationRef.current.evaluatorJobStatusChanged = true;
    let active = true;
    let cleanupFn: (() => void) | undefined;

    void listenEvaluatorJobStatusChanged((job) => {
      if (!active) return;
      if (job.conversation_id !== currentConversationIdRef.current) return;
      setActiveEvaluatorJob(job);
      const evaluatorStatus =
        job.status === "pending" || job.status === "running"
          ? "running"
          : job.status === "canceled"
            ? "canceled"
            : evaluatorJobRefreshesState(job)
              ? "succeeded"
              : "failed";
      const evaluatorTerminal = evaluatorStatus !== "running";
      const evaluatorProgress: BackgroundJobProgress = {
        job_id: `evaluator:${job.evaluator_job_id}`,
        kind: "evaluator",
        label: "State Evaluator",
        status: evaluatorStatus,
        phase: job.status,
        current: evaluatorTerminal ? 1 : 0,
        total: 1,
        succeeded: evaluatorStatus === "succeeded" ? 1 : 0,
        failed: evaluatorStatus === "failed" ? 1 : 0,
        recovered: job.status === "partial_success" ? 1 : 0,
        started_at: job.started_at,
        updated_at: job.completed_at ?? Math.floor(Date.now() / 1_000),
        elapsed_ms: job.elapsed_ms ?? 0,
        estimated_remaining_ms:
          evaluatorTerminal || !job.timeout_ms
            ? evaluatorTerminal
              ? 0
              : null
            : Math.max(0, job.timeout_ms - (job.elapsed_ms ?? 0)),
        detail: job.error_message ?? `${job.provider ?? "provider"} / ${job.model ?? "model"}`,
        cancellable: !evaluatorTerminal,
        history: evaluatorTerminal
          ? [
              {
                index: 1,
                label: "Evaluator",
                status: evaluatorStatus,
                detail: job.error_message ?? null,
                elapsed_ms: job.elapsed_ms ?? null,
              },
            ]
          : [],
      };
      ingestTrackedDevJob(evaluatorProgress);
      if (job.status === "running" || job.status === "pending") {
        setStatus("Updating memory/state...");
      } else if (evaluatorJobRefreshesState(job)) {
        setStatus(evaluatorJobStatusText(job));
        // State advanced, so any prior evaluator failure is resolved.
        setProviderAlert((current) => (current?.title.startsWith("Evaluator") ? null : current));
        if (job.status !== "stale_skipped" && soulRef.current) {
          void refreshContext(soulRef.current.character_id, job.conversation_id);
          void getSoul(soulRef.current.character_id).then(setSoul).catch(() => undefined);
        }
      } else if (job.status === "failed" || job.status === "timed_out") {
        setStatus(evaluatorJobStatusText(job));
        alertProviderFailure("Evaluator", job.error_message ?? evaluatorJobStatusText(job));
      } else if (job.status === "canceled") {
        setStatus("State update canceled");
      }
    }).then((cleanup) => {
      cleanupFn = cleanup;
      if (!active) {
        cleanup();
      }
    });

    return () => {
      active = false;
      listenerRegistrationRef.current.evaluatorJobStatusChanged = false;
      if (cleanupFn) {
        cleanupFn();
      }
    };
  }, []);

  useEffect(() => {
    let active = true;
    let cleanupFn: (() => void) | undefined;

    void listenPipelineTraceUpdated((trace) => {
      if (!active) return;
      if (trace.conversation_id !== currentConversationIdRef.current) return;
      setLatestPipelineTrace(trace);
    }).then((cleanup) => {
      cleanupFn = cleanup;
      if (!active) {
        cleanup();
      }
    });

    return () => {
      active = false;
      if (cleanupFn) {
        cleanupFn();
      }
    };
  }, []);

  useEffect(() => {
    let active = true;
    let cleanupFn: (() => void) | undefined;

    void listenBackgroundJobProgress((progress) => {
      if (!active) return;
      ingestTrackedDevJob(progress);
    }).then((cleanup) => {
      cleanupFn = cleanup;
      if (!active) cleanup();
    });

    return () => {
      active = false;
      cleanupFn?.();
    };
  }, []);

  useEffect(() => {
    if (!activeConversationId) {
      setActiveEvaluatorJob(null);
      return;
    }
    void getLatestEvaluatorJob(activeConversationId)
      .then(setActiveEvaluatorJob)
      .catch(() => undefined);
  }, [activeConversationId]);

  useEffect(() => {
    setEvaluatorJobBannerDismissed(false);
  }, [activeConversationId, activeEvaluatorJob?.evaluator_job_id]);

  function setCreatorFieldsFromSoul(nextSoul: Soul) {
    setCharacterName(nextSoul.character_name);
    setCharacterDescription(nextSoul.profile.description);
    setCharacterAppearance(nextSoul.profile.appearance);
    setCharacterPersonality(nextSoul.profile.personality);
    setCharacterScenario(nextSoul.profile.scenario);
    setOpeningNarratorMessage(nextSoul.profile.opening_narrator_message ?? "");
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
      narrator_timeout_ms: profile.narrator_timeout_ms ?? null,
      evaluator_timeout_ms: profile.evaluator_timeout_ms ?? DEFAULT_EVALUATOR_TIMEOUT_MS,
      structured_evaluator_timeout_ms: profile.structured_evaluator_timeout_ms ?? null,
      diagnostic_evaluator_timeout_ms: profile.diagnostic_evaluator_timeout_ms ?? null,
      evaluator_timeout_mode: profile.evaluator_timeout_mode ?? "finite",
      evaluator_mode: profile.evaluator_mode ?? DEFAULT_EVALUATOR_MODE,
      structured_evaluator_policy: profile.structured_evaluator_policy ?? "prefer",
      structured_evaluator_max_retries: profile.structured_evaluator_max_retries ?? 1,
      wait_for_evaluator_before_next_turn: profile.wait_for_evaluator_before_next_turn ?? true,
      allow_send_with_stale_state: profile.allow_send_with_stale_state ?? false,
      evaluator_background_enabled: profile.evaluator_background_enabled ?? false,
      anti_replay_forced_retry_enabled: profile.anti_replay_forced_retry_enabled ?? false,
    });
  }

  function applyStateUpdaterProviderProfile(profile: ProviderProfile) {
    setUpdaterProviderProfileName(profile.name);
    setStateUpdaterSettings({
      base_url: profile.base_url,
      api_key: profile.api_key,
      model: profile.model,
      system_prompt: profile.system_prompt,
      narrator_timeout_ms: profile.narrator_timeout_ms ?? null,
      evaluator_timeout_ms: profile.evaluator_timeout_ms ?? DEFAULT_EVALUATOR_TIMEOUT_MS,
      structured_evaluator_timeout_ms: profile.structured_evaluator_timeout_ms ?? null,
      diagnostic_evaluator_timeout_ms: profile.diagnostic_evaluator_timeout_ms ?? null,
      evaluator_timeout_mode: profile.evaluator_timeout_mode ?? "finite",
      evaluator_mode: profile.evaluator_mode ?? DEFAULT_EVALUATOR_MODE,
      structured_evaluator_policy: profile.structured_evaluator_policy ?? "prefer",
      structured_evaluator_max_retries: profile.structured_evaluator_max_retries ?? 1,
      wait_for_evaluator_before_next_turn: profile.wait_for_evaluator_before_next_turn ?? true,
      allow_send_with_stale_state: profile.allow_send_with_stale_state ?? false,
      evaluator_background_enabled: profile.evaluator_background_enabled ?? false,
      anti_replay_forced_retry_enabled: profile.anti_replay_forced_retry_enabled ?? false,
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
    const opening = openingNarratorMessage.trim();
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
        opening_narrator_message: opening,
        avatar_image_id: draftAvatarImageId,
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

  async function persistCurrentSetting() {
    if (!setting) return null;
    const nextSetting = applySettingFields(setting);
    await upsertSetting(nextSetting);
    setSetting(nextSetting);
    setSettings(await listSettings());
    return nextSetting;
  }

  async function bootstrap() {
    const [
      existingSouls,
      existingArchivedSouls,
      existingSettings,
      existingArchivedSettings,
      existingConversations,
      existingArchived,
      existingPlayerPersonas,
      existingArchivedPlayerPersonas,
    ] = await Promise.all([
      listSouls(),
      listArchivedSouls(),
      listSettings(),
      listArchivedSettings(),
      listConversations(),
      listArchivedSessions(),
      listPlayerPersonas(),
      listArchivedPlayerPersonas(),
    ]);
    setSouls(existingSouls);
    setArchivedSouls(existingArchivedSouls);
    setSettings(existingSettings);
    setArchivedSettings(existingArchivedSettings);
    setConversations([...existingConversations, ...existingArchived]);
    setPlayerPersonas(existingPlayerPersonas);
    setArchivedPlayerPersonas(existingArchivedPlayerPersonas);
    void loadProviderProfiles();

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
      setSelectedCharacterIds([firstSoul.character_id]);
      setCreatorFieldsFromSoul(firstSoul);
      setCurrentSessionTitle("New Session");
      setStatus("Loaded local Soul and Setting indexes");
      return;
    }

    const nextSoul = await createDefaultSoul(characterName);
    await upsertSoul(nextSoul);
    setSoul(nextSoul);
    setSelectedCharacterIds([nextSoul.character_id]);
    setSouls(await listSouls());
    setStatus("Created starter Soul and Setting");
  }

  async function loadProviderProfiles() {
    try {
      const existingProviderProfiles = await listProviderProfiles();
      setProviderProfiles(existingProviderProfiles);
      setArchivedProviderProfiles(await listArchivedProviderProfiles());
      if (existingProviderProfiles.length === 0) {
        const firstLaunchSeen = localStorage.getItem(SETTINGS_FIRST_LAUNCH_SEEN_STORAGE_KEY) === "true";
        if (!firstLaunchSeen) {
          setSettingsTab("ai");
          localStorage.setItem(SETTINGS_FIRST_LAUNCH_SEEN_STORAGE_KEY, "true");
        }
        return;
      }

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
    } catch (error) {
      logDev("warn", "app", "Provider profile load deferred failed", {
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function refreshContext(soulId: string, conversationId: string) {
    try {
      const preview = await compileContext(soulId, conversationId);
      setContext(preview);
      return preview;
    } catch (error) {
      setContext(null);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setStatus(`Context unavailable: ${errorMessage}`);
      logDev("warn", "context", "Context refresh failed", { soulId, conversationId, error: errorMessage });
      return null;
    }
  }

  async function handleSoulRepair(kind: "world" | "scenario" | "events" | "memories") {
    if (!soul) return;
    const labels = {
      world: "clear world state",
      scenario: "clear profile scenario",
      events: "clear recent events",
      memories: "clear memories",
    } as const;
    const confirmed = await confirmDialog(
      "Debug repair",
      `${labels[kind]} for ${soul.character_name}?`,
      true,
      devModeActive,
    );
    if (!confirmed) return;
    setBusy(true);
    try {
      const nextSoul =
        kind === "world"
          ? await clearSoulWorldState(soul.character_id)
          : kind === "scenario"
            ? await clearSoulProfileScenario(soul.character_id)
            : kind === "events"
              ? await clearSoulRecentEvents(soul.character_id)
              : await clearSoulMemories(soul.character_id);
      setSoul(nextSoul);
      setSouls((current: SoulSummary[]) =>
        current.map((item: SoulSummary) =>
          item.character_id === nextSoul.character_id
            ? {
                ...item,
                recent_count: nextSoul.memory.recent.length,
                core_count: nextSoul.memory.core.length,
              }
            : item,
        ),
      );
      await refreshContext(nextSoul.character_id, currentConversationId);
      logDev("warn", "app", "Soul debug repair applied", { kind, soul_id: nextSoul.character_id });
      setStatus(`Debug repair applied: ${labels[kind]}`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
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
        effectiveNarratorSettings,
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
      setSelectedCharacterIds((current) =>
        current.includes(nextSoul.character_id)
          ? [nextSoul.character_id, ...current.filter((id) => id !== nextSoul.character_id)]
          : [nextSoul.character_id, ...current],
      );
      setSelectedAvatarAsset(draftAvatarAsset);
      setActiveConversationId(null);
      setCurrentSessionTitle("New Session");
      setMessages([]);
      setLastTurnDebug(null);
      setSouls(await listSouls());
      setStatus("New Soul created");
      if (draftAvatarImageId) {
        logDev("success", "db", "avatar_updated", {
          soul_id: nextSoul.character_id,
          image_asset_id: draftAvatarImageId,
        });
      }
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
      setSelectedCharacterIds((current) =>
        current.includes(nextSoul.character_id)
          ? [nextSoul.character_id, ...current.filter((id) => id !== nextSoul.character_id)]
          : [nextSoul.character_id, ...current],
      );
      setActiveConversationId(null);
      setCurrentSessionTitle("New Session");
      setCreatorFieldsFromSoul(nextSoul);
      setLastTurnDebug(null);
      setMessages([]);
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
      setActiveConversationId(null);
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
      setActiveConversationId(null);
      setMessages([]);
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
    updaterOverride?: Partial<ApiProviderSettings>,
    options?: { rethrowErrors?: boolean },
  ): Promise<TurnResult | undefined> {
    if (!text || busy || stateUpdating || !soul) return undefined;
    // Clear any stale provider-failure banner at the start of a fresh turn.
    setProviderAlert(null);
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
      await persistCurrentSetting();
      await upsertSoul(soul);
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
              soul.character_id,
              text,
              mode,
              effectiveNarratorSettings,
              {
                ...(useNarratorProviderForUpdater ? apiSettings : stateUpdaterSettings),
                evaluator_execution_mode: evaluatorExecutionMode,
                structured_evaluator_transport: structuredEvaluatorTransport,
                ...(updaterOverride ?? {}),
              },
              contextMode,
              abortController.signal,
              replacementAssistantId,
              correctionInstruction,
            )
          : await sendMockTurn(
              turnConversationId,
              soul.character_id,
              text,
              mode,
              replacementAssistantId,
              correctionInstruction,
            );
      if (generationIdRef.current !== generationId || abortController.signal.aborted) {
        return undefined;
      }
      if (result.conversation_id !== currentConversationIdRef.current) {
        return undefined;
      }
      setSoul(result.soul);
      setMessages(result.messages);
      setContext(result.context_preview);
      setLastTurnDebug(result.debug);
      setSouls(await listSouls());
      await refreshConversations();
      setStateUpdating(false);
      const replayStatus = result.debug.replay_detected
        ? result.debug.replay_reason?.includes("regenerated before save")
          ? "Turn saved; anti-replay regenerated"
          : "Turn saved; anti-replay warning"
        : null;
      setStatus(
        replayStatus ??
          (result.debug.state_updater_status.startsWith("background")
            ? "Turn saved; memory/state updating in background"
            : result.debug.state_updater_status.startsWith("failed")
              ? "Turn saved; state updater failed"
              : result.consolidation_ran
                ? "Turn saved; consolidation ran"
                : "Turn saved"),
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
      return result;
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
        const detail = error instanceof Error ? error.message : String(error);
        setStatus(`State update failed; narration saved: ${detail}`);
        logDev("error", "state_updater", "State update failed after narration save", {
          conversation_id: turnConversationId,
          error: detail,
        });
        if (provider === "API") {
          alertProviderFailure("Evaluator", detail);
        }
      } else {
        reportError(error, "Generation failed", provider === "API" ? "api" : "app");
        if (provider === "API") {
          alertProviderFailure("Narrator", error instanceof Error ? error.message : String(error));
        }
      }
      if (options?.rethrowErrors) {
        throw error;
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
    return undefined;
  }

  async function submitDraft() {
    const text = draft.trim();
    if (!text || busy || stateUpdating || !soul) return;

    const personaUiAction = personaUiActionForText(text);
    setDraft("");
    setSlashMenuDismissed(false);
    setSlashSelectedIndex(0);
    await executeTurn(text);
    if (personaUiAction === "list") {
      await openPersonaList();
    } else if (personaUiAction === "add") {
      openPersonaAdd();
    } else if (personaUiAction === "edit") {
      await openPersonaEdit();
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    await submitDraft();
  }

  // Dev Mode terminal: `/chat <msg>` speaks in-character (reuses the real turn
  // pipeline); other input is treated as a dev command.
  async function handleDevTerminalSubmit(event: FormEvent) {
    event.preventDefault();
    const raw = devTerminalInput.trim();
    if (!raw) return;
    setDevTerminalInput("");
    if (raw === "/chat" || raw.startsWith("/chat ")) {
      const message = raw.slice("/chat".length).trim();
      if (!message) {
        logDev("warn", "app", "/chat needs a message, e.g. /chat I step inside.");
        return;
      }
      if (!soul || busy || stateUpdating) {
        logDev("warn", "app", "Cannot send right now (busy, updating state, or no character).");
        return;
      }
      logDev("info", "app", "dev /chat dispatched", { message });
      await executeTurn(message);
      return;
    }
    if (raw === "/clear") {
      setDevLogs([]);
      return;
    }
    if (raw === "/help") {
      logDev("info", "app", "commands: /chat <message> - /clear - /help");
      return;
    }
    logDev("warn", "app", `unknown command: ${raw} - type /help`);
  }

  function handleDraftChange(value: string) {
    setDraft(value);
    setSlashMenuDismissed(false);
    setSlashSelectedIndex(0);
  }

  function insertSlashCommand(command = selectedSlashCommand?.command) {
    if (!command) return;
    setDraft((current) => completeSlashCommandInput(current, command));
    setSlashMenuDismissed(true);
    setSlashSelectedIndex(0);
  }

  async function refreshPlayerPersonas(conversationId = currentConversationId) {
    const [personas, archived] = await Promise.all([
      listPlayerPersonas(),
      listArchivedPlayerPersonas(),
    ]);
    setPlayerPersonas(personas);
    setArchivedPlayerPersonas(archived);
    if (conversationId) {
      const active = await getActivePlayerPersona(conversationId);
      setActivePlayerPersonaState(active);
    }
    return personas;
  }

  async function openPersonaList(conversationId = currentConversationId, requireConfirm = false) {
    await refreshPlayerPersonas(conversationId);
    setPersonaModalConversationId(conversationId ?? null);
    setPersonaListConfirmRequired(requireConfirm);
    setPersonaModalMode("list");
  }

  function openPersonaAdd() {
    setPersonaEditingId(null);
    setPersonaForm({
      display_name: "",
      gender_code: "custom",
      pronouns: "",
      description: "",
      appearance: "",
      notes: "",
    });
    setPersonaModalMode("add");
  }

  async function openPersonaEdit(personaId?: string | null) {
    const personas = await refreshPlayerPersonas(personaModalConversationId ?? currentConversationId);
    const target =
      personas.find((persona) => persona.persona_id === personaId) ??
      activePlayerPersona ??
      personas.find((persona) => !persona.is_builtin);
    if (!target || target.is_builtin) {
      setStatus("Built-in personas cannot be edited; create a custom persona first.");
      setPersonaModalMode("list");
      return;
    }
    setPersonaEditingId(target.persona_id);
    setPersonaForm({
      persona_id: target.persona_id,
      display_name: target.display_name,
      gender_code: target.gender_code,
      pronouns: target.pronouns,
      description: target.description,
      appearance: target.appearance ?? "",
      notes: target.notes ?? "",
    });
    setPersonaModalMode("edit");
  }

  async function handleSelectPersona(personaId: string) {
    const conversationId = personaModalConversationId ?? currentConversationId;
    if (!conversationId) return;
    const active = await setActivePlayerPersona(conversationId, personaId);
    setActivePlayerPersonaState(active);
    await refreshPlayerPersonas(conversationId);
    setStatus(`Active persona: ${active.display_name}`);
  }

  function handleConfirmPersonaList() {
    if (!activePlayerPersona) {
      setStatus("Choose a player persona before continuing.");
      return;
    }
    setPersonaListConfirmRequired(false);
    setPersonaModalConversationId(null);
    setPersonaModalMode(null);
    setStatus(`Chat ready with player persona: ${activePlayerPersona.display_name}`);
  }

  function closePersonaModal() {
    setPersonaListConfirmRequired(false);
    setPersonaModalConversationId(null);
    setPersonaModalMode(null);
  }

  async function handleSavePersona() {
    const saved = await upsertPlayerPersona({
      ...personaForm,
      persona_id: personaEditingId ?? personaForm.persona_id ?? null,
    });
    const conversationId = personaModalConversationId ?? currentConversationId;
    await refreshPlayerPersonas(conversationId);
    if (conversationId) {
      await handleSelectPersona(saved.persona_id);
    }
    setPersonaModalMode("list");
  }

  async function handleArchivePersona(persona: PlayerPersona) {
    if (persona.is_builtin) {
      setStatus("Built-in personas stay available.");
      return;
    }
    if (activePlayerPersona?.persona_id === persona.persona_id) {
      setStatus("Select another persona before archiving the active one.");
      return;
    }
    const confirmed = await confirmDialog(
      "Archive player persona",
      `Archive ${persona.display_name}? It can be restored later.`,
      true,
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await archivePlayerPersona(persona.persona_id);
      await refreshPlayerPersonas(personaModalConversationId ?? currentConversationId);
      setStatus(`Archived persona: ${persona.display_name}`);
      logDev("warn", "db", "Player persona archived", { personaId: persona.persona_id });
    } catch (error) {
      reportError(error, "Persona archive failed", "db");
    } finally {
      setBusy(false);
    }
  }

  async function handleRestoreArchivedPersona(persona: PlayerPersona) {
    setBusy(true);
    try {
      await restorePlayerPersona(persona.persona_id);
      await refreshPlayerPersonas(personaModalConversationId ?? currentConversationId);
      setStatus(`Restored persona: ${persona.display_name}`);
      logDev("success", "db", "Player persona restored", { personaId: persona.persona_id });
    } catch (error) {
      reportError(error, "Persona restore failed", "db");
    } finally {
      setBusy(false);
    }
  }

  function personaUiActionForText(text: string) {
    const trimmed = text.trim().toLowerCase();
    if (trimmed === "/persona list") return "list";
    if (trimmed === "/persona add") return "add";
    if (trimmed.startsWith("/persona edit")) return "edit";
    return null;
  }

  function handleComposerKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (slashMenuOpen && slashSuggestions.length > 0) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSlashSelectedIndex((current) =>
          nextSlashCommandIndex(current, slashSuggestions.length, 1),
        );
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSlashSelectedIndex((current) =>
          nextSlashCommandIndex(current, slashSuggestions.length, -1),
        );
        return;
      }
      if (event.key === "Tab") {
        event.preventDefault();
        insertSlashCommand();
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setSlashMenuDismissed(true);
        return;
      }
      if (
        event.key === "Enter" &&
        !event.shiftKey &&
        !event.nativeEvent.isComposing
      ) {
        event.preventDefault();
        insertSlashCommand();
        return;
      }
    }
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) {
      return;
    }
    event.preventDefault();
    void submitDraft();
  }

  function handleViewDisclaimer() {
    setDisclaimerUnderstood(false);
    setDisclaimerRemember(false);
    setDisclaimerMode("manual");
    logDev("info", "app", "Disclaimer opened from settings");
  }

  function handleAcceptDisclaimer() {
    if (!disclaimerUnderstood) return;
    if (disclaimerRemember) {
      localStorage.setItem(
        DISCLAIMER_STORAGE_KEY,
        JSON.stringify({
          accepted: true,
          accepted_at: Date.now(),
          disclaimer_version: DISCLAIMER_VERSION,
        }),
      );
    }
    setDisclaimerMode(null);
    setDisclaimerUnderstood(false);
    setDisclaimerRemember(false);
    setStatus("Disclaimer accepted");
    logDev("info", "app", "Disclaimer accepted", {
      persisted: disclaimerRemember,
      disclaimer_version: DISCLAIMER_VERSION,
    });
  }

  function handleCloseDisclaimer() {
    setDisclaimerMode(null);
    setDisclaimerUnderstood(false);
    setDisclaimerRemember(false);
  }

  async function handleCopyLlmPayload() {
    if (!llmPayload) return;
    await navigator.clipboard.writeText(formatLlmPayloadDebugBlock(llmPayload));
    setPayloadCopied(true);
    setStatus("LLM payload copied");
    logDev("info", "app", "LLM payload copied");
    window.setTimeout(() => setPayloadCopied(false), 1800);
  }

  async function handleValidateMne() {
    const path = await promptDialog("Validate .mne bundle", "", {
      message: "Enter absolute path to the .mne file to validate.",
      placeholder: "C:\\path\\to\\bundle.mne",
    });
    if (!path) return;
    try {
      setStatus("Validating bundle...");
      const report = await runTrackedBackgroundOperation(
        "validation",
        "Validate .mne bundle",
        () => validateMneBundle(path),
        {
          detail: path,
          successDetail: (result) => result.valid ? "Bundle is valid" : "Bundle validation failed",
        },
      );
      logDev("info", "app", "MNE Validation Report", report);
      if (report.valid) {
        await alertDialog("Bundle is valid", `Soul: ${report.summary.soul_name || "N/A"}\nWorld: ${report.summary.world_name || "N/A"}\nMessages: ${report.summary.message_count}\nMemories: ${report.summary.memory_count}\nRelationships: ${report.summary.relationship_count}`);
      } else {
        await alertDialog("Bundle is invalid", `Errors:\n${report.errors.join("\n")}`);
      }
    } catch (err: any) {
      await alertDialog("Validation error", String(err));
      logDev("error", "app", "MNE Validation Failed", { error: err.toString() });
    } finally {
      setStatus("");
    }
  }

  async function handlePreviewMne() {
    const path = await promptDialog("Preview .mne bundle", "", {
      message: "Enter absolute path to the .mne file to preview.",
      placeholder: "C:\\path\\to\\bundle.mne",
    });
    if (!path) return;
    try {
      setStatus("Previewing bundle...");
      const report = await runTrackedBackgroundOperation(
        "import",
        "Preview .mne import",
        () => previewMneImport(path),
        {
          detail: path,
          successDetail: (result) =>
            `${result.summary.message_count} messages; ${result.summary.memory_count} memories`,
        },
      );
      logDev("info", "app", "MNE Preview Report", report);
      await alertDialog("MNE preview", `Soul: ${report.summary.soul_name || "N/A"} (${report.summary.soul_id || "N/A"})\nWorld: ${report.summary.world_name || "N/A"} (${report.summary.world_id || "N/A"})\nMessages: ${report.summary.message_count}\nMemories: ${report.summary.memory_count}\nObject States: ${report.summary.object_state_count}\nRelationships: ${report.summary.relationship_count}\nPayload Logs: ${report.summary.payload_log_count}`);
    } catch (err: any) {
      await alertDialog("Preview error", String(err));
      logDev("error", "app", "MNE Preview Failed", { error: err.toString() });
    } finally {
      setStatus("");
    }
  }

  async function handleImportMneAsNew() {
    const path = await promptDialog("Import .mne as new copy", "", {
      message: "Enter absolute path to the .mne file to import as a new copy.",
      placeholder: "C:\\path\\to\\bundle.mne",
    });
    if (!path) return;
    try {
      setStatus("Importing bundle...");
      const result = await runTrackedBackgroundOperation(
        "import",
        "Import .mne as new copy",
        () => importMneAsNew(path),
        {
          detail: path,
          successDetail: (importResult) => importResult.summary,
        },
      );
      logDev("info", "app", "MNE Import Result", result);
      await alertDialog("Import successful", `Summary: ${result.summary}\nRemapped IDs count: ${Object.keys(result.remapped_ids).length}`);
      setSouls(await listSouls());
      await refreshConversations();
    } catch (err: any) {
      await alertDialog("Import error", String(err));
      logDev("error", "app", "MNE Import Failed", { error: err.toString() });
    } finally {
      setStatus("");
    }
  }

  async function handleExportVisibleChatLog() {
    if (!currentConversationId) return;
    setBusy(true);
    try {
      logDev("info", "app", "Visible chat export started", {
        conversation_id: currentConversationId,
      });
      const result = await runTrackedBackgroundOperation(
        "export",
        "Export visible chat log",
        () => exportVisibleChatLog(currentConversationId),
        {
          detail: `Session ${currentConversationId}`,
          successDetail: (exportResult) => exportResult.path,
        },
      );
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
      const result = await runTrackedBackgroundOperation(
        "export",
        "Export LLM payload history",
        () => exportLlmPayloadHistory(currentConversationId),
        {
          detail: `Session ${currentConversationId}`,
          successDetail: (exportResult) => exportResult.path,
        },
      );
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

  function handleToggleCharacterSelection(characterId: string) {
    setSelectedCharacterIds((current) => {
      if (current.includes(characterId)) {
        if (soul?.character_id === characterId) return current;
        return current.filter((id) => id !== characterId);
      }
      return [...current, characterId];
    });
  }

  async function refreshActiveSessionAfterDevCommand(conversationId: string) {
    const nextMessages = await listConversationMessages(conversationId);
    setMessages(nextMessages);
    if (soul) {
      const nextSoul = await getSoul(soul.character_id);
      setSoul(nextSoul);
      setCreatorFieldsFromSoul(nextSoul);
      setContext(await compileContext(nextSoul.character_id, conversationId));
    }
    setSouls(await listSouls());
    await refreshConversations();
    await refreshAssistantVariants(conversationId, nextMessages);
  }

  async function runWhitelistedDevCommand(commandName: DevCommandName, argsOverride?: Record<string, unknown>) {
    if (!import.meta.env.DEV) return;
    const args = argsOverride ?? parseDevCommandArgs(devCommandArgs);
    const conversationId =
      typeof args.conversationId === "string" && args.conversationId.trim()
        ? args.conversationId.trim()
        : currentConversationId;
    if (!conversationId) {
      throw new Error("No active conversation_id.");
    }

    switch (commandName) {
      case "dedupe_active_adjacent_user_messages":
        return dedupeActiveAdjacentUserMessages(conversationId);
      case "restore_inactive_messages":
        return restoreInactiveMessages(conversationId);
      case "get_branch_patch_debug":
        return getBranchPatchDebug(conversationId);
      case "rebuild_session_from_ledger":
        return rebuildSessionFromLedger(conversationId);
      case "inspect_turn_branch_integrity":
        return inspectTurnBranchIntegrity(conversationId);
      case "repair_accidental_normal_send_variants":
        return repairAccidentalNormalSendVariants(conversationId);
      case "export_visible_chat_log":
        return exportVisibleChatLog(conversationId);
      case "export_llm_payload_history":
        return exportLlmPayloadHistory(conversationId);
      case "run_benchmark": {
        if (!soul) throw new Error("No selected Soul for benchmark.");
        const rawBenchmarkType = devStringArg(args, "benchmark_type", "benchmarkType") ?? benchmarkType;
        if (
          rawBenchmarkType !== "visible_ai_chat" &&
          rawBenchmarkType !== "scripted_visible_replay" &&
          rawBenchmarkType !== "headless_regression" &&
          rawBenchmarkType !== "multi_agent_visible_chat"
        ) {
          throw new Error(`Unsupported benchmark_type: ${rawBenchmarkType}`);
        }
        const nextBenchmarkType = rawBenchmarkType as BenchmarkType;
        const rawBenchmarkTarget = devStringArg(args, "target", "benchmarkTarget") ?? benchmarkTarget;
        if (
          rawBenchmarkTarget !== "current_session" &&
          rawBenchmarkTarget !== "new_benchmark_session_from_current_soul" &&
          rawBenchmarkTarget !== "new_benchmark_session_from_selected_soul_world"
        ) {
          throw new Error(`Unsupported benchmark target: ${rawBenchmarkTarget}`);
        }
        const requiresPlayerProfile =
          nextBenchmarkType === "visible_ai_chat" || nextBenchmarkType === "multi_agent_visible_chat";
        const playerProfileId =
          devStringArg(args, "player_simulator_profile_id", "playerSimulatorProfileId") ??
          selectedBenchmarkPlayerProfileId;
        if (requiresPlayerProfile && !playerProfileId) {
          throw new Error("Self-play benchmark requires player_simulator_profile_id.");
        }
        const updaterSettings = useNarratorProviderForUpdater ? apiSettings : stateUpdaterSettings;
        const strictToolEvaluator = devBooleanArg(
          args,
          "strict_tool_evaluator",
          benchmarkStrictToolEvaluator,
        );
        const benchmarkSettings: BenchmarkSettings = {
          benchmark_type: nextBenchmarkType,
          target: rawBenchmarkTarget as BenchmarkTarget,
          current_conversation_id:
            (rawBenchmarkTarget as BenchmarkTarget) === "current_session" ? conversationId : null,
          turn_count: Math.max(1, Math.floor(devNumberArg(args, "turn_count", benchmarkTurnCount))),
          narrator_style: devStringArg(args, "narrator_style", "narratorStyle") ?? mode,
          evaluator_mode: strictToolEvaluator
            ? "evaluator_structured_v1"
            : devStringArg(args, "evaluator_mode", "evaluatorMode") ??
              updaterSettings.evaluator_mode ??
              "evaluator_form_v1",
          structured_evaluator_transport: strictToolEvaluator
            ? "tool_call"
            : devStringArg(args, "structured_evaluator_transport", "structuredEvaluatorTransport") ??
              benchmarkTransport ??
              updaterSettings.structured_evaluator_transport ??
              structuredEvaluatorTransport,
          structured_evaluator_policy: strictToolEvaluator
            ? "required"
            : devStringArg(args, "structured_evaluator_policy", "structuredEvaluatorPolicy") ??
              updaterSettings.structured_evaluator_policy ??
              "prefer",
          structured_evaluator_max_retries: Math.max(
            0,
            Math.floor(
              devNumberArg(
                args,
                "structured_evaluator_max_retries",
                updaterSettings.structured_evaluator_max_retries ?? 1,
              ),
            ),
          ),
          player_simulator_profile_id: requiresPlayerProfile ? playerProfileId : null,
          player_goal:
            devStringArg(args, "player_goal", "playerGoal") ??
            benchmarkPlayerGoal,
          export_payload_history: devBooleanArg(args, "export_payload_history", true),
          export_mne: devBooleanArg(args, "export_mne", true),
          export_summary_json: devBooleanArg(args, "export_summary_json", true),
          strict_tool_evaluator: strictToolEvaluator,
          wait_for_evaluator_each_turn: devBooleanArg(
            args,
            "wait_for_evaluator_each_turn",
            benchmarkWaitForEvaluator,
          ),
        };
        const summary = await runBenchmark(
          devStringArg(args, "soul_id", "soulId") ?? soul.character_id,
          devStringArg(args, "setting_id", "settingId") ?? setting?.setting_id ?? null,
          devStringArg(args, "provider") ?? provider,
          effectiveNarratorSettings,
          updaterSettings,
          benchmarkSettings,
        );
        setBenchmarkResult(summary);
        setBenchmarkError(null);
        return summary;
      }
      case "seed_observable_knowledge": {
        if (!currentConversationId) {
          throw new Error("Open a session first.");
        }
        const seeded = await seedObservableKnowledge(
          currentConversationId,
          effectiveStateUpdaterSettings,
        );
        return {
          seeded_observations: seeded,
          note: "Everything the pass did not return stays unknown until the story discloses it.",
        };
      }
      default: {
        const exhaustive: never = commandName;
        throw new Error(`Command is not whitelisted: ${exhaustive}`);
      }
    }
  }

  async function handleRunDevCommand(commandName: DevCommandName, argsOverride?: Record<string, unknown>) {
    if (devCommandRunning) return;
    const conversationId =
      typeof argsOverride?.conversationId === "string"
        ? argsOverride.conversationId
        : currentConversationId;
    setDevCommandRunning(true);
    setDevCommandResult(null);
    setDevCommandError(null);
    const devJobId =
      commandName === "run_benchmark"
        ? null
        : startTrackedDevJob("dev_command", commandName.replace(/_/g, " "), {
            phase: "running",
            detail: conversationId ? `Session ${conversationId}` : "Global command",
          });
    try {
      const result = await runWhitelistedDevCommand(commandName, argsOverride);
      const formatted = JSON.stringify(result, null, 2);
      setDevCommandResult(formatted);
      const resultConversationId =
        result &&
        typeof result === "object" &&
        "conversation_id" in result &&
        typeof result.conversation_id === "string"
          ? result.conversation_id
          : conversationId;
      const benchmarkPassed =
        result &&
        typeof result === "object" &&
        "scorecard" in result &&
        result.scorecard &&
        typeof result.scorecard === "object" &&
        "pass" in result.scorecard
          ? Boolean(result.scorecard.pass)
          : null;
      setStatus(
        commandName === "run_benchmark" && benchmarkPassed !== null
          ? `${benchmarkPassed ? "PASS" : "FAIL"} benchmark from Dev Mode`
          : `Dev command complete: ${commandName}`,
      );
      logDev("success", "app", "Dev command complete", {
        command: commandName,
        conversation_id: resultConversationId,
        result,
      });
      if (commandName === "run_benchmark") {
        await refreshConversations();
      } else if (conversationId) {
        await refreshActiveSessionAfterDevCommand(conversationId);
      }
      finishTrackedDevJob(devJobId, "succeeded", {
        current: 1,
        total: 1,
        succeeded: 1,
        detail: `Completed ${commandName}`,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setDevCommandError(message);
      setStatus(`Dev command failed: ${message}`);
      logDev("error", "error", "Dev command failed", {
        command: commandName,
        conversation_id: conversationId,
        error: message,
      });
      finishTrackedDevJob(devJobId, "failed", {
        current: 1,
        total: 1,
        failed: 1,
        detail: message,
      });
    } finally {
      setDevCommandRunning(false);
    }
  }

  async function handleStartChat() {
    if (!soul || busy) return;
    if (chatStartMode === "continue") {
      const existingConversation = conversations.find(
        (conversation) =>
          conversation.conversation_id === defaultConversationId ||
          conversation.source_savepoint_id === soul.character_id ||
          (Boolean(soul.source_savepoint_id) &&
            conversation.source_savepoint_id === soul.source_savepoint_id),
      );
      if (existingConversation) {
        await handleSelectConversation(existingConversation);
        setSessionContinuityLabel("Using existing isolated Session");
        setView("chat");
        return;
      }
    }

    setBusy(true);
    try {
      const sourceId = soul.character_id;
      const session = await createSessionSoulClone(sourceId, setting?.setting_id);
      const sessionSoul = session.soul;
      const nextConversationId = session.conversation.conversation_id;
      setActiveConversationId(nextConversationId);
      setSoul(sessionSoul);
      setSelectedCharacterIds((current) =>
        current.includes(sourceId) ? current : [sourceId, ...current],
      );
      setCreatorFieldsFromSoul(sessionSoul);
      setMessages(session.messages);
      setContext(await compileContext(sessionSoul.character_id, nextConversationId));
      await refreshPlayerPersonas(nextConversationId);
      if (selectedStateUpdaterProfileId) {
        try {
          await setActiveEvaluatorProfile(nextConversationId, selectedStateUpdaterProfileId);
        } catch (e) {
          console.error("Failed to set active evaluator profile on new chat", e);
        }
      }
      setSouls(await listSouls());
      await refreshConversations();
      setLastTurnDebug(null);
      setSessionContinuityLabel("Isolated Session; source savepoints remain unchanged");
      setCurrentSessionTitle(session.conversation.title);
      setStatus("Started isolated Session from the selected Soul.");
      logDev("info", "context", "new session cloned selected Soul", {
        source_soul_id: sourceId,
        session_soul_id: sessionSoul.character_id,
        conversation_id: nextConversationId,
        active_plots: sessionSoul.world.active_plots,
        recent_events: sessionSoul.world.recent_events.length,
        memories: sessionSoul.memory.recent.length,
        schemas: sessionSoul.memory.schemas.length,
        relationship_targets: Object.keys(sessionSoul.relationships),
      });
      setView("chat");
      await openPersonaList(nextConversationId, true);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleRenameCurrentSession() {
    if (busy || !currentConversationId) return;
    const nextTitle = await promptDialog("Rename session", currentSessionTitle, {
      placeholder: "Session title",
      confirmLabel: "Rename",
    });
    if (nextTitle === null) return;
    const trimmed = nextTitle.trim();
    if (!trimmed) {
      setStatus("Session title cannot be empty");
      return;
    }
    setBusy(true);
    try {
      const renamed = await renameConversation(currentConversationId, trimmed, soul?.character_id);
      setCurrentSessionTitle(renamed.title);
      setConversations((current) =>
        current.some((conversation) => conversation.conversation_id === renamed.conversation_id)
          ? current.map((conversation) =>
              conversation.conversation_id === renamed.conversation_id ? renamed : conversation,
            )
          : [renamed, ...current],
      );
      setStatus(`Renamed session: ${renamed.title}`);
      logDev("success", "app", "Session renamed", {
        conversation_id: renamed.conversation_id,
        title: renamed.title,
      });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSelectConversation(conversation: ConversationSummary) {
    if (busy) return;
    setBusy(true);
    try {
      const accessedConversation = await touchConversationAccess(conversation.conversation_id);
      const conversationSoul = await getSoul(accessedConversation.soul_id);
      setSoul(conversationSoul);
      setSelectedCharacterIds((current) =>
        current.includes(conversationSoul.character_id)
          ? [conversationSoul.character_id, ...current.filter((id) => id !== conversationSoul.character_id)]
          : [conversationSoul.character_id, ...current],
      );
      setCreatorFieldsFromSoul(conversationSoul);
      setActiveConversationId(accessedConversation.conversation_id);
      setCurrentSessionTitle(accessedConversation.title);
      setSessionContinuityLabel(
        accessedConversation.source_savepoint_id
          ? "Loaded named Session clone"
          : "Loaded persistent Soul continuity chat",
      );
      setMessages(await listConversationMessages(accessedConversation.conversation_id));
      setConversations((current) => [
        accessedConversation,
        ...current.filter((item) => item.conversation_id !== accessedConversation.conversation_id),
      ]);
      if (accessedConversation.active_evaluator_profile_id) {
        const updaterProfile = providerProfiles.find((p) => p.id === accessedConversation.active_evaluator_profile_id);
        if (updaterProfile) {
          setSelectedStateUpdaterProfileId(updaterProfile.id);
          applyStateUpdaterProviderProfile(updaterProfile);
        }
      } else {
        if (selectedStateUpdaterProfileId) {
          try {
            await setActiveEvaluatorProfile(accessedConversation.conversation_id, selectedStateUpdaterProfileId);
          } catch (e) {
            console.error("Failed to set active evaluator profile on conversation select", e);
          }
        }
      }
      await refreshPlayerPersonas(accessedConversation.conversation_id);
      setLastTurnDebug(null);
      setView("chat");
      setStatus(`Loaded chat: ${accessedConversation.title}`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleOpenMostRecentChat() {
    if (busy) return;
    try {
      const latestConversations = await listConversations();
      const latest = latestConversations[0];
      if (!latest) {
        setStatus("No chats yet. Start one from Library.");
        setView("library");
        return;
      }
      await handleSelectConversation(latest);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleAvatarImageSelected(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    setBusy(true);
    try {
      const asset = await importImageAssetFromFile({
        file,
        source: "uploaded",
      });
      setDraftAvatarAsset(asset);
      setDraftAvatarImageId(asset.id);
      setStatus("Draft profile picture updated");
      logDev("info", "db", "avatar_draft_updated", {
        image_asset_id: asset.id,
      });
    } catch (error) {
      reportError(error, "Avatar image import failed", "db");
    } finally {
      setBusy(false);
    }
  }

  function handleRemoveAvatarImage() {
    setDraftAvatarAsset(null);
    setDraftAvatarImageId(null);
    setStatus("Draft profile picture removed");
    logDev("info", "db", "avatar_draft_updated", {
      image_asset_id: null,
    });
  }

  async function handleChatImageSelected(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || !soul) return;
    setBusy(true);
    try {
      const nextMessages = await createUserImageMessageFromFile(currentConversationId, file, draft);
      setDraft("");
      setMessages(nextMessages);
      await refreshConversations();
      setContext(await compileContext(soul.character_id, currentConversationId));
      setStatus("Image attached to chat");
      logDev("success", "db", "chat_image_attached", {
        conversation_id: currentConversationId,
      });
    } catch (error) {
      reportError(error, "Chat image attach failed", "db");
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

  function assistantForUserMessage(message: ChatMessage) {
    const messageIndex = activeMessages.findIndex((item) => item.id === message.id);
    if (messageIndex < 0 || message.role !== "user") return undefined;
    const firstFollowingTurnMessage = activeMessages
      .slice(messageIndex + 1)
      .find((item) => item.role === "assistant" || item.role === "user");
    return firstFollowingTurnMessage?.role === "assistant" ? firstFollowingTurnMessage : undefined;
  }

  function canGenerateFromUserMessage(message: ChatMessage) {
    if (message.role !== "user") return false;
    const messageIndex = activeMessages.findIndex((item) => item.id === message.id);
    if (messageIndex < 0) return false;
    const laterUserExists = activeMessages
      .slice(messageIndex + 1)
      .some((item) => item.role === "user");
    if (laterUserExists) return false;
    const assistant = assistantForUserMessage(message);
    return !assistant || assistant.id === latestAssistantMessageId;
  }

  async function executeTurnFromUserMessage(
    message: ChatMessage,
    statusLabel: string,
    correctionInstruction?: string,
  ) {
    if (busy || stateUpdating || !soul || message.role !== "user") return;
    if (!canGenerateFromUserMessage(message)) {
      setStatus("Regenerating older turns requires branch rewind and will be added later.");
      return;
    }

    try {
      const assistant = assistantForUserMessage(message);
      await executeTurn(message.content, statusLabel, assistant?.id, correctionInstruction);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleRegenerateFromUserMessage(message: ChatMessage) {
    await executeTurnFromUserMessage(message, "Regenerating response");
  }

  async function handleFixFromUserMessage(message: ChatMessage) {
    if (busy || stateUpdating || !soul || message.role !== "user") return;
    if (!canGenerateFromUserMessage(message)) {
      setStatus("Regenerating older turns requires branch rewind and will be added later.");
      return;
    }
    const instruction = (
      await promptDialog("Fix next response", "Continue from the current user message. Do not replay the wrong branch.", {
        message: "Correction instruction for the regenerated response.",
        textarea: true,
        confirmLabel: "Apply",
      })
    )?.trim();
    if (!instruction) return;
    await executeTurnFromUserMessage(message, "Applying fix instruction", instruction);
  }

  async function handleDeleteChatMessage(message: ChatMessage) {
    if (busy || stateUpdating) return;
    const confirmed = await confirmDialog(
      "Hide turn",
      message.role === "assistant"
        ? "Hide this generated response and all later turns in this session? This is recoverable with Restore hidden turns."
        : "Hide this user message and all later turns in this session? This is recoverable with Restore hidden turns.",
      true,
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await deleteMessage(message.conversation_id, message.id);
      const nextMessages = await listConversationMessages(message.conversation_id);
      setMessages(nextMessages);
      await refreshConversations();
      if (soul) {
        const nextSoul = await getSoul(soul.character_id);
        setSoul(nextSoul);
        setCreatorFieldsFromSoul(nextSoul);
        setContext(await compileContext(soul.character_id, message.conversation_id));
      }
      setStatus(
        nextMessages.length
          ? "Turn hidden; later visible chat was rewound"
          : "Turn hidden; this session is now empty but can be restored",
      );
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleRestoreHiddenTurns() {
    if (busy || !currentConversationId) return;
    setBusy(true);
    try {
      const restore = await restoreInactiveMessages(currentConversationId);
      const nextMessages = restore.messages;
      setMessages(nextMessages);
      await refreshConversations();
      if (soul) {
        const nextSoul = await getSoul(soul.character_id);
        setSoul(nextSoul);
        setCreatorFieldsFromSoul(nextSoul);
        setContext(await compileContext(soul.character_id, currentConversationId));
      }
      const skipped =
        restore.preview.skipped_duplicate_ids.length +
        restore.preview.skipped_pending_ids.length +
        restore.preview.skipped_failed_ids.length +
        restore.preview.skipped_retry_attempt_ids.length +
        restore.preview.skipped_regenerated_discarded_ids.length;
      setStatus(
        restore.preview.restored_message_ids.length
          ? `Hidden turns restored (${restore.preview.restored_message_ids.length} restored, ${skipped} skipped)`
          : "No hidden turns found",
      );
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleEditUserMessage(message: ChatMessage) {
    if (busy || stateUpdating || message.role !== "user") return;
    const nextContent = await promptDialog("Edit user message", message.content, {
      message: "Soul memory and later responses are not rewound.",
      textarea: true,
      confirmLabel: "Save",
    });
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
        const nextSoul = await getSoul(soul.character_id);
        setSoul(nextSoul);
        setCreatorFieldsFromSoul(nextSoul);
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

  async function handleCancelEvaluatorJob() {
    if (!activeEvaluatorJob || !activeEvaluatorJobIsLive) return;
    try {
      await cancelEvaluatorJob(activeEvaluatorJob.evaluator_job_id);
      setActiveEvaluatorJob({ ...activeEvaluatorJob, status: "canceled", error_message: "Canceled by user" });
      setStatus("State update canceled");
    } catch (error) {
      reportError(error, "Cancel evaluator job failed", "state_updater");
    }
  }

  function handleDismissEvaluatorJobBanner() {
    setEvaluatorJobBannerDismissed(true);
  }

  async function handleRetryEvaluatorJob() {
    if (!activeEvaluatorJob) return;
    try {
      await retryEvaluatorJob(
        activeEvaluatorJob.conversation_id,
        activeEvaluatorJob.assistant_message_id,
        effectiveStateUpdaterSettings,
      );
      setStatus("Retrying memory/state update");
      const latest = await getLatestEvaluatorJob(activeEvaluatorJob.conversation_id);
      setActiveEvaluatorJob(latest);
    } catch (error) {
      reportError(error, "Retry evaluator job failed", "state_updater");
    }
  }

  function handleProceedWithStaleState() {
    setStatus("Proceeding with current visible chat; pending state may be stale");
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

  async function handleCurateRecentMemory(memoryId: string, operation: "pin" | "unpin" | "restore_archived") {
    if (!soul || !currentConversationId) return;
    setBusy(true);
    try {
      const result = await curateMemory(currentConversationId, soul.character_id, memoryId, operation);
      setSoul(result.soul);
      setSouls(await listSouls());
      await refreshContext(result.soul.character_id, currentConversationId);
      const label =
        operation === "pin"
          ? "Memory pinned"
          : operation === "unpin"
            ? "Memory unpinned"
            : "Memory restored";
      setStatus(label);
      logDev("success", "db", label, {
        memoryId,
        patchId: result.patch_id,
        operation: result.operation,
      });
    } catch (error) {
      reportError(error, "Memory curation failed", "db");
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteChat(conversationId = currentConversationId) {
    if (!soul) return;
    const deletingActiveConversation = conversationId === currentConversationId;
    const confirmed = await confirmDialog(
      "Archive session",
      "Archive this local session? It stays recoverable; messages, payload logs, and session clones are kept.",
      true,
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await archiveConversation(conversationId);
      await refreshConversations();
      if (deletingActiveConversation) {
        if (!currentSessionTitle.startsWith("[Archived] ")) {
          setCurrentSessionTitle(`[Archived] ${currentSessionTitle}`);
        }
      }
      setLastTurnDebug(null);
      setStatus("Session archived; data kept and recoverable");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleRestoreSession(conversationId = currentConversationId) {
    if (!soul || !conversationId) return;
    const confirmed = await confirmDialog(
      "Restore session",
      "Restore this archived session to your active chats?",
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await restoreConversation(conversationId);
      await refreshConversations();
      if (currentConversationId === conversationId) {
        if (currentSessionTitle.startsWith("[Archived] ")) {
          setCurrentSessionTitle(currentSessionTitle.replace("[Archived] ", ""));
        }
      }
      setStatus("Session restored successfully");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleOpenSessionDataLocation() {
    try {
      const path = await openSessionDataLocation();
      setStatus(`Opened session data folder: ${path}`);
      logDev("info", "app", "Opened session data folder", { path });
    } catch (error) {
      reportError(error, "Failed to open session data folder", "error");
    }
  }

  async function handleDeleteSoul() {
    if (!soul) return;
    const confirmed = await confirmDialog(
      "Archive Soul",
      `Archive ${soul.character_name}? Local chats and savepoint history remain safe and recoverable.`,
      true,
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await deleteSoul(soul.character_id);
      const [remaining, archived] = await Promise.all([listSouls(), listArchivedSouls()]);
      setSouls(remaining);
      setArchivedSouls(archived);
      await refreshConversations();
      setActiveConversationId(null);
      setSelectedCharacterIds((current) => current.filter((id) => id !== soul.character_id));

      if (remaining.length === 0) {
        setSoul(null);
        setMessages([]);
        setContext(null);
        setStatus("Soul archived");
        return;
      }

      const nextSoul = await getSoul(remaining[0].character_id);
      setSoul(nextSoul);
      setSelectedCharacterIds((current) =>
        current.includes(nextSoul.character_id)
          ? [nextSoul.character_id, ...current.filter((id) => id !== nextSoul.character_id)]
          : [nextSoul.character_id, ...current],
      );
      setCreatorFieldsFromSoul(nextSoul);
      setMessages([]);
      setContext(null);
      setStatus("Soul archived; selected next local Soul");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleRestoreArchivedSoul(soulId: string) {
    setBusy(true);
    try {
      await restoreSoul(soulId);
      const [active, archived] = await Promise.all([listSouls(), listArchivedSouls()]);
      setSouls(active);
      setArchivedSouls(archived);
      const restored = await getSoul(soulId);
      setSoul(restored);
      setSelectedCharacterIds((current) =>
        current.includes(restored.character_id)
          ? [restored.character_id, ...current.filter((id) => id !== restored.character_id)]
          : [restored.character_id, ...current],
      );
      setCreatorFieldsFromSoul(restored);
      setActiveConversationId(null);
      setMessages([]);
      setContext(null);
      setStatus(`Restored character: ${restored.character_name}`);
      logDev("success", "db", "Character restored", { soulId });
    } catch (error) {
      reportError(error, "Character restore failed", "db");
    } finally {
      setBusy(false);
    }
  }

  async function handlePurgeSoul() {
    if (!soul) return;
    const confirmed = await confirmDialog(
      "Permanently delete Soul",
      `Permanently delete ${soul.character_name}? This creates a safety backup but cannot be undone from the UI.`,
      true,
    );
    if (!confirmed) return;
    setBusy(true);
    try {
      await purgeSoul(soul.character_id);
      const remaining = await listSouls();
      setSouls(remaining);
      setArchivedSouls(await listArchivedSouls());
      await refreshConversations();
      setActiveConversationId(null);
      setMessages([]);
      setContext(null);
      if (remaining.length === 0) {
        setSoul(null);
        setStatus("Character permanently deleted");
        return;
      }
      const nextSoul = await getSoul(remaining[0].character_id);
      setSoul(nextSoul);
      setCreatorFieldsFromSoul(nextSoul);
      setSelectedCharacterIds([nextSoul.character_id]);
      setStatus("Character permanently deleted; selected next");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleArchiveSetting() {
    if (!setting) return;

    const activeSettingIds: string[] = [];
    if (settings.length <= 1) {
      activeSettingIds.push(setting.setting_id);
    }
    if (activeConversationId) {
      const activeConv = conversations.find((c) => c.conversation_id === activeConversationId);
      if (activeConv?.source_setting_id) {
        activeSettingIds.push(activeConv.source_setting_id);
      }
    }

    if (activeSettingIds.includes(setting.setting_id)) {
      await alertDialog("Cannot archive setting", "Cannot archive the active/default setting. Switch settings first.");
      return;
    }

    const confirmed = await confirmDialog(
      "Archive setting",
      `Archive ${setting.setting_name}? Local chats and world settings remain safe and recoverable.`,
      true,
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await archiveSetting(setting.setting_id, activeSettingIds);
      const remaining = await listSettings();
      setSettings(remaining);
      void refreshArchivedSettings();

      if (remaining.length === 0) {
        const nextSetting = await createDefaultSetting("Starter Setting");
        await upsertSetting(nextSetting);
        setSetting(nextSetting);
        setEditorFieldsFromSetting(nextSetting);
        setSettings(await listSettings());
        setMessages([]);
        setStatus("Setting archived; created starter Setting");
        return;
      }

      const nextSetting = await getSetting(remaining[0].setting_id);
      setSetting(nextSetting);
      setEditorFieldsFromSetting(nextSetting);
      setActiveConversationId(null);
      setMessages([]);
      setStatus("Setting archived; selected next local Setting");
    } catch (error) {
      reportError(error, "Setting archive failed", "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleRestoreArchivedSetting(settingId: string) {
    setBusy(true);
    try {
      await restoreSetting(settingId);
      const [active, archived] = await Promise.all([listSettings(), listArchivedSettings()]);
      setSettings(active);
      setArchivedSettings(archived);
      const restored = await getSetting(settingId);
      setSetting(restored);
      setEditorFieldsFromSetting(restored);
      setActiveConversationId(null);
      setMessages([]);
      setStatus(`Restored world: ${restored.setting_name}`);
      logDev("success", "db", "World restored", { settingId });
    } catch (error) {
      reportError(error, "World restore failed", "db");
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

  async function handleSessionFormEvalBenchmark() {
    const profileId = selectedStateUpdaterProfileId;
    if (!currentConversationId) {
      setStatus("Open a session first.");
      return;
    }
    if (!profileId) {
      setStatus("Select an Evaluator (state updater) provider profile first.");
      return;
    }
    setFormEvalBusy(true);
    setFormEvalReport(null);
    setStatus("Running form-eval benchmark on this session (dry-run, nothing applied)...");
    try {
      // Resolve the repair endpoint the same way live repair does. If the user's
      // repair selection is the embedded local model, make sure it's actually up
      // (auto-start + wait) and build the settings from its live URL, otherwise
      // the benchmark would silently test repair against the eval profile.
      let repairSettings = repairSettingsRef.current ?? undefined;
      const wantsEmbedded =
        selectedRepairProfileId !== REPAIR_MODEL_EVALUATOR &&
        !providerProfiles.some((profile) => profile.id === selectedRepairProfileId) &&
        Boolean(embeddedModelPath.trim());
      if (wantsEmbedded) {
        setStatus("Ensuring local repair model is up for the benchmark...");
        const ready = await ensureLocalRepairModelReady();
        const status = ready ? await embeddedRepairModelStatus() : null;
        if (status?.ready && status.url) {
          repairSettings = {
            ...(repairSettingsRef.current ?? apiSettings),
            base_url: status.url,
            api_key: "local",
            model: status.model ?? "local-model",
          };
        } else {
          setProviderAlert({
            title: "Local repair model unavailable",
            detail: "The embedded repair model couldn't be started for the benchmark.",
            hint: "Check the model path in Settings. The repair stage will use the eval profile instead.",
          });
        }
      }
      setStatus("Running form-eval benchmark on this session (dry-run, nothing applied)...");
      const report = await runSessionFormEvalBenchmark(
        currentConversationId,
        profileId,
        repairSettings,
      );
      setFormEvalReport(report);
      setStatus(
        `Form-eval benchmark: ${report.form_passed}/${report.turns_total} form-valid, ${report.repair_recovered} recovered by repair.`,
      );
      logDev("info", "state_updater", "Session form-eval benchmark complete", {
        turns: report.turns_total,
        form_passed: report.form_passed,
        form_failed: report.form_failed,
        repair_recovered: report.repair_recovered,
      });
    } catch (error) {
      reportError(error, "Session form-eval benchmark failed", "state_updater");
    } finally {
      setFormEvalBusy(false);
    }
  }

  async function handleRunContractTest(profileId: string) {
    const profile = providerProfiles.find((item) => item.id === profileId);
    if (!profile) return;
    setBusy(true);
    setStatus(`Running evaluator contract test for ${profile.name}...`);
    try {
      const report = await runEvaluatorContractTest(profileId);
      if (report.passed) {
        setStatus(`Evaluator contract test passed for ${profile.name}!`);
        logDev("success", "state_updater", `Evaluator contract test passed for ${profile.name}`);
      } else {
        const errorMsg = report.errors.join("; ");
        setStatus(`Evaluator contract test failed for ${profile.name}: ${errorMsg}`);
        logDev("error", "error", `Evaluator contract test failed for ${profile.name}: ${errorMsg}`, {
          errors: report.errors,
          raw_response: report.raw_response,
        });
        await alertDialog("Evaluator contract test failed", `${errorMsg}\n\nRaw response:\n${report.raw_response}`, devModeActive);
      }
      setProviderProfiles(await listProviderProfiles());
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setStatus(`Evaluator contract test error: ${msg}`);
      reportError(error, "Evaluator contract test failed", "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleRunStructuredDiagnostic() {
    if (structuredDiagnosticRunning) return;
    setStructuredDiagnosticRunning(true);
    setStructuredDiagnosticResult(null);
    setStructuredDiagnosticError(null);
    const profileId = selectedStateUpdaterProfileId || selectedProviderProfileId || null;
    setStatus("Running structured evaluator diagnostic...");
    try {
      const summary = await runStructuredEvaluatorDiagnostic(profileId);
      setStructuredDiagnosticResult(summary);
      const enforcement = summary.structured_enforcement_per_run?.join(", ") || summary.enforcement_levels.join(", ");
      const failed =
        summary.resolved_evaluator_source !== "evaluator_structured_v1" ||
        summary.evaluator_mode_per_run.some((mode) => mode !== "evaluator_structured_v1") ||
        summary.runs.some((run) => run.error);
      setStatus(
        `${failed ? "FAIL" : "PASS"} structured diagnostic: ${summary.provider_model}; enforcement ${enforcement || "none"}`,
      );
      logDev(failed ? "error" : "success", "state_updater", "Structured evaluator diagnostic completed", {
        pass: !failed,
        model: summary.provider_model,
        structured_mode_requested: summary.structured_mode_requested,
        structured_mode_resolved: summary.structured_mode_resolved,
        resolved_evaluator_source: summary.resolved_evaluator_source,
        enforcement: summary.structured_enforcement_per_run,
        fallback_paths: summary.fallback_paths,
        payload_history_path: summary.payload_history_path,
        mne_checkpoint_path: summary.mne_checkpoint_path,
        summary_json_path: summary.summary_json_path,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setStructuredDiagnosticError(message);
      setStatus(`Structured diagnostic failed: ${message}`);
      reportError(error, "Structured evaluator diagnostic failed", "state_updater");
    } finally {
      setStructuredDiagnosticRunning(false);
    }
  }

  async function handleRunBenchmark() {
    if (!soul || benchmarkRunning) return;
    const requiresPlayerProfile =
      benchmarkType === "visible_ai_chat" || benchmarkType === "multi_agent_visible_chat";
    if (requiresPlayerProfile && !selectedBenchmarkPlayerProfileId) {
      setBenchmarkError("Visible AI Chat benchmark requires a Player Simulator profile.");
      setStatus("Choose a Player Simulator profile before running Visible AI Chat.");
      return;
    }

    const updaterSettings = useNarratorProviderForUpdater ? apiSettings : stateUpdaterSettings;
    const settingsPayload: BenchmarkSettings = {
      benchmark_type: benchmarkType,
      target: benchmarkTarget,
      current_conversation_id: benchmarkTarget === "current_session" ? currentConversationId : null,
      turn_count: Math.max(1, Math.floor(benchmarkTurnCount) || 1),
      narrator_style: mode,
      evaluator_mode: benchmarkStrictToolEvaluator
        ? "evaluator_structured_v1"
        : updaterSettings.evaluator_mode ?? "evaluator_form_v1",
      structured_evaluator_transport: benchmarkStrictToolEvaluator
        ? "tool_call"
        : benchmarkTransport ?? updaterSettings.structured_evaluator_transport ?? structuredEvaluatorTransport,
      structured_evaluator_policy: benchmarkStrictToolEvaluator
        ? "required"
        : updaterSettings.structured_evaluator_policy ?? "prefer",
      structured_evaluator_max_retries: updaterSettings.structured_evaluator_max_retries ?? 1,
      player_simulator_profile_id: requiresPlayerProfile ? selectedBenchmarkPlayerProfileId : null,
      player_character_soul_id: benchmarkPlayerCharacterSoulId || null,
      player_goal: benchmarkPlayerGoal,
      export_payload_history: true,
      export_mne: true,
      export_summary_json: true,
      strict_tool_evaluator: benchmarkStrictToolEvaluator,
      wait_for_evaluator_each_turn: benchmarkWaitForEvaluator,
    };

    // Visible AI Chat drives turns live through the normal chat path so the
    // AI-vs-AI exchange streams into the chat window. Other modes (scripted /
    // headless / mock) keep the blocking backend orchestration.
    if (benchmarkType === "visible_ai_chat" && provider === "API") {
      await startLiveBenchmark(settingsPayload, updaterSettings);
      return;
    }

    setBenchmarkRunning(true);
    setBenchmarkResult(null);
    setBenchmarkError(null);
    setBenchmarkLivePhase("preparing");
    setStatus("Running benchmark...");
    try {
      const summary = await runBenchmark(
        soul.character_id,
        setting?.setting_id ?? null,
        provider,
        effectiveNarratorSettings,
        updaterSettings,
        settingsPayload,
      );
      setBenchmarkResult(summary);
      await refreshConversations();
      if (summary.conversation_id === currentConversationIdRef.current) {
        setMessages(await listConversationMessages(summary.conversation_id));
        await refreshContext(soul.character_id, summary.conversation_id);
      } else {
        setActiveConversationId(summary.conversation_id);
        setMessages(await listConversationMessages(summary.conversation_id));
      }
      setStatus(`${summary.scorecard.pass ? "PASS" : "FAIL"} benchmark: ${summary.benchmark_id}`);
      logDev(summary.scorecard.pass ? "success" : "warn", "app", "Benchmark completed", {
        benchmark_id: summary.benchmark_id,
        benchmark_type: summary.benchmark_type,
        pass: summary.scorecard.pass,
        failure_reasons: summary.scorecard.failure_reasons,
        summary_json_path: summary.summary_json_path,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setBenchmarkError(message);
      setStatus(`Benchmark failed: ${message}`);
      reportError(error, "Benchmark failed", "app");
    } finally {
      setBenchmarkRunning(false);
      setBenchmarkLivePhase("idle");
    }
  }

  // A live benchmark turn must hit the EXACT same pipeline as a turn you type,
  // so the exported payloads/session show the real behavior you're diagnosing.
  // The only optional deviations are:
  //   - "Wait for evaluator each turn": runs the evaluator as a tracked
  //     BACKGROUND job (so the evaluator banner stays live), and the loop waits
  //     on that job before the next turn, keeping per-turn commit ordering.
  //   - "Strict Tool Evaluator": opt-in probe that pins the evaluator to
  //     structured tool-calling. OFF (default for faithful repro) = your exact
  //     chat evaluator settings flow through untouched.
  // Wait between live benchmark turns until the background evaluator has
  // committed. EVENT-DRIVEN: resolves the instant the job's completion event
  // arrives (no poll lag), which is why this is much faster than the old 700ms
  // polling loop. A slow 3s poll is kept in case an event is missed. The job's
  // configured timeout is authoritative; in no-app-timeout mode Stop remains
  // available through the poll. Returns immediately if there's no live job.
  async function waitForBenchmarkEvaluatorJob(conversationId: string): Promise<any> {
    const isTerminal = (status: string) => status !== "pending" && status !== "running";
    // Fast path: nothing in flight.
    let pending: any;
    try {
      pending = await getLatestEvaluatorJob(conversationId);
      if (!pending || isTerminal(pending.status)) return pending;
    } catch {
      return undefined;
    }
    // Wait for THIS job, by id. Re-reading "the latest job" on the way out is
    // what produced `evaluator_failed: running`: by the time a slow job's
    // terminal event arrived, the next turn's job already existed, and the
    // benchmark read the newcomer's `running` as this turn's verdict.
    const pendingJobId: string = pending.evaluator_job_id;
    return new Promise<any>((resolve) => {
      let done = false;
      let unlisten: (() => void) | undefined;
      let safetyPoll: number | undefined;
      const finish = (job: any) => {
        if (done) return;
        done = true;
        if (safetyPoll !== undefined) window.clearInterval(safetyPoll);
        unlisten?.();
        resolve(job);
      };
      // Primary signal: this job's own status-changed event.
      void listenEvaluatorJobStatusChanged((job) => {
        if (job.evaluator_job_id === pendingJobId && isTerminal(job.status)) {
          finish(job);
        }
      }).then((stop) => {
        unlisten = stop;
        if (done) stop();
      });
      // Safety nets: honor Stop and slow-poll in case an event is dropped.
      safetyPoll = window.setInterval(() => {
        if (benchmarkStopRef.current) {
          finish(pending);
          return;
        }
        void getEvaluatorJob(pendingJobId)
          .then((job) => {
            if (!job) {
              finish(undefined);
            } else if (isTerminal(job.status)) {
              finish(job);
            }
          })
          .catch(() => undefined);
      }, 3000);
    });
  }

  async function startLiveBenchmark(
    settingsPayload: BenchmarkSettings,
    updaterSettings: ApiProviderSettings,
  ) {
    if (!soul) return;
    liveBenchmarkJobIdRef.current = startTrackedDevJob("benchmark", "Live Benchmark", {
      total: settingsPayload.turn_count,
      phase: "preparing",
      detail: `${benchmarkType} / ${benchmarkTarget}`,
      cancellable: true,
    });
    setBenchmarkRunning(true);
    setBenchmarkResult(null);
    setBenchmarkError(null);
    setBenchmarkLivePhase("preparing");
    setStatus("Preparing live benchmark session...");
    // Auto-start the local repair model if it's configured but not up, and wait
    // for it to be ready before running turns so repair has a live endpoint
    // instead of silently connection-refusing. Non-fatal: if it won't start, we
    // warn and run anyway (the scorecard reports local_repair_unavailable).
    if (embeddedModelPath.trim()) {
      updateTrackedDevJob(liveBenchmarkJobIdRef.current, {
        phase: "repair_model_startup",
        detail: "Ensuring the local repair model is ready",
      });
      setStatus("Ensuring local repair model is up...");
      // Single shared implementation (also used by the repair listener): starts
      // the model if needed and waits for it to actually answer before turn 1.
      const localReady = await ensureLocalRepairModelReady(120000);
      if (localReady) {
        setStatus("Local repair model ready");
      } else {
        setProviderAlert({
          title: "Local repair model didn't start",
          detail: "Auto-start of the local repair model failed or timed out.",
          hint: "Check the llamafile path in Settings (the file must be runnable). The run will continue, but repair can't recover state.",
        });
      }
    }
    try {
      const init: BenchmarkSessionInit = await prepareBenchmarkSession(
        soul.character_id,
        setting?.setting_id ?? null,
        settingsPayload,
      );
      const liveUpdaterSettings: ApiProviderSettings = {
        ...updaterSettings,
        evaluator_execution_mode: evaluatorExecutionMode,
        structured_evaluator_transport: structuredEvaluatorTransport,
        ...benchmarkLiveUpdaterOverride(settingsPayload),
      };
      benchmarkCtxRef.current = {
        benchmarkId: init.benchmark_id,
        conversationId: init.conversation_id,
        soulId: init.session_soul_id,
        startedAt: init.started_at,
        playerProfileId: settingsPayload.player_simulator_profile_id ?? "",
        playerGoal: settingsPayload.player_goal,
        playerCharacterSoulId: settingsPayload.player_character_soul_id ?? null,
        traditionalOpponent: benchmarkTraditionalOpponent,
        settings: settingsPayload,
        narratorSettings: effectiveNarratorSettings,
        updaterSettings: liveUpdaterSettings,
        initialMemoryCount: init.initial_memory_count,
        initialObjectCount: init.initial_object_count,
        initialRelationshipCount: init.initial_relationship_count,
        relationshipTargetChecked: init.relationship_target_checked,
        initialActivePlayerRelationship: init.initial_active_player_relationship ?? null,
        perTurn: [],
        narratorFailures: 0,
        completedTurns: 0,
        consecutiveEvaluatorFailures: 0,
        nextTurnIndex: 0,
        lastPlayerText: "",
      };
      benchmarkStopRef.current = false;
      benchmarkTurnInFlightRef.current = false;
      setActiveConversationId(init.conversation_id);
      setMessages(await listConversationMessages(init.conversation_id));
      await refreshContext(soul.character_id, init.conversation_id);
      setBenchmarkTurnsRemaining(settingsPayload.turn_count);
      setBenchmarkLiveActive(true);
      setBenchmarkLivePhase("player_generation");
      updateTrackedDevJob(liveBenchmarkJobIdRef.current, {
        phase: "player_generation",
        detail: `Starting cycle 1/${settingsPayload.turn_count}`,
      });
      setStatus(`Live benchmark running: 0/${settingsPayload.turn_count} turns`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      benchmarkCtxRef.current = null;
      setBenchmarkError(message);
      setStatus(`Benchmark failed: ${message}`);
      reportError(error, "Benchmark failed", "app");
      setBenchmarkRunning(false);
      setBenchmarkLivePhase("failed");
      finishTrackedDevJob(liveBenchmarkJobIdRef.current, "failed", {
        phase: "preparing",
        failed: 1,
        detail: message,
      });
    }
  }

  async function runOneBenchmarkTurn() {
    const ctx = benchmarkCtxRef.current;
    if (!ctx) return;
    benchmarkTurnInFlightRef.current = true;
    const turnIndex = ctx.nextTurnIndex;
    const turnLabel = `Benchmark turn ${turnIndex + 1}/${ctx.settings.turn_count}`;
    // Null until the player line is generated this turn, so a failure in
    // generation is recorded as an empty message (not the previous turn's text).
    let playerText: string | null = null;
    let phase: BenchmarkTurnPhase = "player_generation";
    try {
      const opponentLabel = ctx.traditionalOpponent ? "Traditional RP" : "AI player";
      setBenchmarkLivePhase("player_generation");
      updateTrackedDevJob(liveBenchmarkJobIdRef.current, {
        phase: "player_generation",
        current: turnIndex,
        detail: `${turnLabel}: ${opponentLabel} thinking`,
      });
      setStatus(`${turnLabel}: ${opponentLabel} thinking...`);
      const generate = ctx.traditionalOpponent
        ? generateTraditionalRpMessage
        : generateBenchmarkPlayerMessage;
      playerText = await generate(
        ctx.conversationId,
        ctx.soulId,
        ctx.playerProfileId,
        ctx.playerGoal,
        ctx.playerCharacterSoulId,
      );
      ctx.lastPlayerText = playerText;
      if (benchmarkStopRef.current) {
        benchmarkTurnInFlightRef.current = false;
        return;
      }
      // Send as a visible user turn; the narrator streams its reply live.
      phase = "execute_turn";
      setBenchmarkLivePhase("execute_turn");
      updateTrackedDevJob(liveBenchmarkJobIdRef.current, {
        phase: "turn_pipeline",
        detail: `${turnLabel}: narrator and evaluator pipeline`,
      });
      const result = await executeTurn(
        playerText,
        turnLabel,
        undefined,
        undefined,
        benchmarkLiveUpdaterOverride(ctx.settings),
        { rethrowErrors: true },
      );
      // Stop may have aborted the narrator mid-stream, so don't record a partial
      // turn; bail and let the effect finalize what actually completed.
      if (benchmarkStopRef.current) {
        benchmarkTurnInFlightRef.current = false;
        return;
      }
      if (!result) {
        throw new Error("executeTurn did not return a completed turn");
      }
      if (!turnResultEvaluatorCompletedOrSkipped(result)) {
        throw new Error(`evaluator_failed: ${result.debug.state_updater_status}`);
      }
      // If an evaluator job is running in the background, wait for a terminal
      // success/skip state before counting this benchmark turn complete.
      if (
        ctx.settings.wait_for_evaluator_each_turn ||
        result.debug.state_updater_status.startsWith("background_")
      ) {
        phase = "evaluator_wait";
        setBenchmarkLivePhase("evaluator_wait");
        updateTrackedDevJob(liveBenchmarkJobIdRef.current, {
          phase: "evaluator_wait",
          detail: `${turnLabel}: waiting for state commit`,
        });
        setStatus(`${turnLabel}: waiting for evaluator...`);
        const evaluatorJob = await waitForBenchmarkEvaluatorJob(ctx.conversationId);
        if (benchmarkStopRef.current) {
          benchmarkTurnInFlightRef.current = false;
          return;
        }
        if (result.debug.state_updater_status.startsWith("background_") && !evaluatorJob) {
          throw new Error("evaluator_failed: background evaluator job did not reach a terminal status");
        }
        if (!benchmarkEvaluatorJobCompletedOrSkipped(evaluatorJob)) {
          const detail = evaluatorJob?.error_message ? `: ${evaluatorJob.error_message}` : "";
          throw new Error(`evaluator_failed: ${evaluatorJob?.status ?? "unknown"}${detail}`);
        }
      }
      phase = "turn_summary";
      setBenchmarkLivePhase("turn_summary");
      const summary = await benchmarkTurnSummary(
        ctx.conversationId,
        turnIndex,
        playerText,
        "completed",
        null,
        ctx.updaterSettings,
      );
      ctx.perTurn.push(summary);
      ctx.completedTurns += 1;
      ctx.consecutiveEvaluatorFailures = 0;
      ctx.nextTurnIndex = turnIndex + 1;
      phase = "completed";
      setBenchmarkLivePhase("completed");
      setBenchmarkTurnsRemaining((remaining) => remaining - 1);
      const liveElapsedMs = Math.max(0, Date.now() - ctx.startedAt * 1_000);
      const averageCycleMs = liveElapsedMs / Math.max(1, ctx.completedTurns);
      appendTrackedDevJobHistory(
        liveBenchmarkJobIdRef.current,
        {
          index: turnIndex + 1,
          label: `Cycle ${turnIndex + 1}`,
          status: "succeeded",
          detail: "Narrator and evaluator pipeline completed",
          elapsed_ms: null,
        },
        {
          phase: "cycle_complete",
          current: ctx.completedTurns,
          succeeded: ctx.completedTurns,
          estimated_remaining_ms: Math.max(
            0,
            Math.round(averageCycleMs * (ctx.settings.turn_count - ctx.completedTurns)),
          ),
          detail: `Completed cycle ${ctx.completedTurns}/${ctx.settings.turn_count}`,
        },
      );
      setStatus(`Live benchmark running: ${ctx.completedTurns}/${ctx.settings.turn_count} turns`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      // Distinguish where it broke so the summary isn't misleading: a null
      // playerText means the AI player-line call failed (the narrator never ran).
      const stage =
        playerText === null
          ? "player_line_generation_failed"
          : phase === "turn_summary"
            ? "benchmark_summary_failed"
          : message.startsWith("evaluator_failed:") ||
              message.startsWith("State update in progress") ||
              message.startsWith("state_update_failed_after_narration_saved:") ||
              phase === "evaluator_wait"
            ? "evaluator_failed"
            : "narrator_failed";
      // Surface the failing role prominently so the user knows what to fix.
      if (stage === "player_line_generation_failed") {
        alertProviderFailure("Player simulator", message);
      } else if (stage === "narrator_failed") {
        alertProviderFailure("Narrator", message);
      } else if (stage === "evaluator_failed") {
        alertProviderFailure("Evaluator", message);
      } else if (stage === "benchmark_summary_failed") {
        setProviderAlert({
          title: "Benchmark summary failed",
          detail: message,
          hint: "The run finished but the summary/export step failed.",
        });
      }
      if (stage === "narrator_failed") {
        ctx.narratorFailures += 1;
        try {
          const hiddenMessageId = await hideLatestBenchmarkFailedUserMessage(
            ctx.conversationId,
            playerText ?? "",
          );
          if (hiddenMessageId !== null) {
            setMessages(await listConversationMessages(ctx.conversationId));
          }
        } catch (cleanupError) {
          reportError(
            cleanupError,
            "Failed to hide benchmark user message after narrator failure",
            "app",
          );
        }
      }
      try {
        const failureDetail =
          stage === "narrator_failed"
            ? `narrator_provider_error: ${message}`
            : stage === "evaluator_failed"
              ? `evaluator_error: ${message}`
              : stage === "benchmark_summary_failed"
                ? `benchmark_summary_error: ${message}`
                : `player_simulator_error: ${message}`;
        const failedSummary = await benchmarkTurnSummary(
          ctx.conversationId,
          turnIndex,
          playerText ?? "",
          stage,
          failureDetail,
          ctx.updaterSettings,
        );
        ctx.perTurn.push(failedSummary);
      } catch (summaryError) {
        const summaryMessage =
          summaryError instanceof Error ? summaryError.message : String(summaryError);
        const fallbackDetail =
          stage === "narrator_failed"
            ? `narrator_provider_error: ${message}; fallback_summary_error: ${summaryMessage}`
            : stage === "benchmark_summary_failed"
            ? `benchmark_summary_error: ${message}; fallback_summary_error: ${summaryMessage}`
            : `summary_capture_failed_after_${stage}: ${summaryMessage}; original_error: ${message}`;
        ctx.perTurn.push(
          fallbackBenchmarkTurnSummary(
            turnIndex,
            stage,
            playerText ?? "",
            fallbackDetail,
            ctx.updaterSettings,
          ),
        );
      }
      // The narrator already wrote this turn and it is saved; only the state
      // extraction fell over. Ending the whole run there throws away the nine
      // turns still to come over one slow provider call, so a lone evaluator
      // failure costs the turn and nothing more. A run whose evaluator is
      // genuinely dead still stops — a benchmark of an engine that never
      // commits state measures nothing worth paying for.
      const evaluatorFailureIsRecoverable = stage === "evaluator_failed";
      if (evaluatorFailureIsRecoverable) {
        ctx.consecutiveEvaluatorFailures += 1;
      }
      const abortRun =
        !evaluatorFailureIsRecoverable ||
        ctx.consecutiveEvaluatorFailures >= MAX_CONSECUTIVE_EVALUATOR_FAILURES ||
        turnIndex + 1 >= ctx.settings.turn_count;
      if (abortRun) {
        benchmarkStopRef.current = true;
        setBenchmarkTurnsRemaining(0);
        setBenchmarkLivePhase("failed");
      } else {
        ctx.nextTurnIndex = turnIndex + 1;
        setBenchmarkTurnsRemaining((remaining) => remaining - 1);
      }
      const detail = abortRun
        ? message
        : `${message} (continuing; ${ctx.consecutiveEvaluatorFailures}/${MAX_CONSECUTIVE_EVALUATOR_FAILURES} consecutive evaluator failures)`;
      appendTrackedDevJobHistory(
        liveBenchmarkJobIdRef.current,
        {
          index: turnIndex + 1,
          label: `Cycle ${turnIndex + 1}`,
          status: "failed",
          detail,
          elapsed_ms: null,
        },
        {
          status: abortRun ? "failed" : "running",
          phase: stage,
          current: ctx.completedTurns,
          succeeded: ctx.completedTurns,
          failed: ctx.perTurn.length - ctx.completedTurns,
          estimated_remaining_ms: abortRun ? 0 : null,
          cancellable: !abortRun,
          detail,
        },
      );
      logDev("warn", "app", "Live benchmark turn failed", {
        conversation_id: ctx.conversationId,
        turn_index: turnIndex,
        stage,
        error: message,
        run_aborted: abortRun,
        consecutive_evaluator_failures: ctx.consecutiveEvaluatorFailures,
      });
    } finally {
      benchmarkTurnInFlightRef.current = false;
    }
  }

  async function finishBenchmarkLive() {
    const ctx = benchmarkCtxRef.current;
    if (!ctx) return;
    benchmarkTurnInFlightRef.current = true;
    setBenchmarkLiveActive(false);
    setBenchmarkLivePhase("finalizing");
    updateTrackedDevJob(liveBenchmarkJobIdRef.current, {
      phase: "finalizing",
      detail: "Exporting artifacts and computing scorecard",
    });
    setStatus("Finalizing benchmark...");
    try {
      const summary = await finalizeBenchmark(
        ctx.benchmarkId,
        ctx.conversationId,
        ctx.startedAt,
        ctx.narratorSettings,
        ctx.updaterSettings,
        ctx.settings,
        ctx.initialMemoryCount,
        ctx.initialObjectCount,
        ctx.initialRelationshipCount,
        ctx.relationshipTargetChecked,
        ctx.initialActivePlayerRelationship,
        ctx.completedTurns,
        ctx.narratorFailures,
        ctx.perTurn,
      );
      setBenchmarkResult(summary);
      setBenchmarkError(null);
      await refreshConversations();
      if (ctx.conversationId === currentConversationIdRef.current) {
        setMessages(await listConversationMessages(ctx.conversationId));
        await refreshContext(ctx.soulId, ctx.conversationId);
      }
      setStatus(`${summary.scorecard.pass ? "PASS" : "FAIL"} benchmark: ${summary.benchmark_id}`);
      logDev(summary.scorecard.pass ? "success" : "warn", "app", "Benchmark completed", {
        benchmark_id: summary.benchmark_id,
        benchmark_type: summary.benchmark_type,
        pass: summary.scorecard.pass,
        failure_reasons: summary.scorecard.failure_reasons,
        summary_json_path: summary.summary_json_path,
      });
      finishTrackedDevJob(
        liveBenchmarkJobIdRef.current,
        summary.scorecard.pass ? "succeeded" : "failed",
        {
          current: summary.turn_count_completed,
          total: summary.turn_count_requested,
          succeeded: summary.turn_count_completed,
          failed: summary.scorecard.pass ? 0 : Math.max(1, summary.narrator_failures + summary.evaluator_failures),
          recovered: summary.retry_success_count,
          detail: summary.scorecard.pass
            ? "Benchmark passed"
            : summary.scorecard.failure_reasons.join("; "),
        },
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setBenchmarkError(message);
      setStatus(`Benchmark failed: ${message}`);
      reportError(error, "Benchmark failed", "app");
      finishTrackedDevJob(liveBenchmarkJobIdRef.current, "failed", {
        phase: "finalizing",
        failed: 1,
        detail: message,
      });
    } finally {
      benchmarkCtxRef.current = null;
      benchmarkStopRef.current = false;
      benchmarkTurnInFlightRef.current = false;
      setBenchmarkRunning(false);
      setBenchmarkLivePhase("idle");
      liveBenchmarkJobIdRef.current = null;
    }
  }

  function handleStopBenchmark() {
    if (!benchmarkLiveActive) return;
    benchmarkStopRef.current = true;
    setBenchmarkLivePhase("stopping");
    setBenchmarkTurnsRemaining(0);
    updateTrackedDevJob(liveBenchmarkJobIdRef.current, {
      status: "stopping",
      phase: "stopping",
      detail: "Cancel requested; waiting for the active call to stop",
    });
    // Abort any in-flight narrator stream so Stop takes effect immediately
    // rather than waiting out the current turn's network call. (A player-line
    // call already in flight is a backend command and can't be aborted, but its
    // timeout is capped at ~90s, so it returns and the loop bails shortly.)
    generationAbortRef.current?.abort();
    setStatus("Stopping benchmark...");
  }

  async function handleSelectStateUpdaterProfile(profileId: string) {
    if (!profileId) {
      setSelectedStateUpdaterProfileId("");
      if (currentConversationId) {
        await setActiveEvaluatorProfile(currentConversationId, null);
        await refreshConversations();
      }
      return;
    }
    const profile = providerProfiles.find((item) => item.id === profileId);
    if (!profile) return;

    if (profile.evaluator_compatibility_status === 2 && !devOverrideActive) {
      await alertDialog(
        "Cannot activate profile",
        `Compatibility status for "${profile.name}" is FAILED. Run contract test or enable Developer Override to proceed.`,
      );
      return;
    }

    const CURRENT_EVALUATOR_PROMPT_VERSION = 1;
    const isStalePrompt = profile.evaluator_prompt_version !== CURRENT_EVALUATOR_PROMPT_VERSION;
    if (profile.evaluator_compatibility_status === 0 || isStalePrompt) {
      const reason = profile.evaluator_compatibility_status === 0 ? "is untested" : "has a stale prompt version";
      const confirmRun = await confirmDialog(
        "Run compatibility test?",
        `Profile "${profile.name}" ${reason}.\n\nRun the compatibility contract test now? Cancel bypasses and loads with a developer warning.`
      );
      if (confirmRun) {
        await handleRunContractTest(profileId);
        const updatedProfiles = await listProviderProfiles();
        setProviderProfiles(updatedProfiles);
        const refreshedProfile = updatedProfiles.find(item => item.id === profileId);
        if (refreshedProfile && refreshedProfile.evaluator_compatibility_status !== 1 && !devOverrideActive) {
          await alertDialog("Profile activation cancelled", "Contract test did not pass.");
          return;
        }
      } else {
        logDev("warn", "warning", `Bypassed compatibility gate for untested/stale profile ${profile.name}`);
      }
    }

    setSelectedStateUpdaterProfileId(profileId);
    applyStateUpdaterProviderProfile(profile);

    if (currentConversationId) {
      try {
        await setActiveEvaluatorProfile(currentConversationId, profileId);
        await refreshConversations();
      } catch (error) {
        reportError(error, "Failed to persist active evaluator profile on conversation", "db");
      }
    }

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
    const existing = providerProfiles.find(p => p.id === selectedProviderProfileId);
    const profile: ProviderProfile = {
      id: selectedProviderProfileId || crypto.randomUUID(),
      name: trimmedName,
      base_url: apiSettings.base_url,
      api_key: apiSettings.api_key,
      model: apiSettings.model,
      system_prompt: apiSettings.system_prompt,
      narrator_timeout_ms: apiSettings.narrator_timeout_ms ?? null,
      evaluator_timeout_ms: apiSettings.evaluator_timeout_ms ?? DEFAULT_EVALUATOR_TIMEOUT_MS,
      structured_evaluator_timeout_ms: apiSettings.structured_evaluator_timeout_ms ?? null,
      diagnostic_evaluator_timeout_ms: apiSettings.diagnostic_evaluator_timeout_ms ?? null,
      evaluator_timeout_mode: apiSettings.evaluator_timeout_mode ?? "finite",
      evaluator_mode: apiSettings.evaluator_mode ?? DEFAULT_EVALUATOR_MODE,
      structured_evaluator_policy: apiSettings.structured_evaluator_policy ?? "prefer",
      structured_evaluator_max_retries: apiSettings.structured_evaluator_max_retries ?? 1,
      wait_for_evaluator_before_next_turn: apiSettings.wait_for_evaluator_before_next_turn ?? true,
      allow_send_with_stale_state: apiSettings.allow_send_with_stale_state ?? false,
      evaluator_background_enabled: apiSettings.evaluator_background_enabled ?? false,
      anti_replay_forced_retry_enabled: apiSettings.anti_replay_forced_retry_enabled ?? false,
      created_at: existing?.created_at ?? 0,
      updated_at: 0,
      narrator_compatibility_status: existing?.narrator_compatibility_status ?? 0,
      evaluator_compatibility_status: existing?.evaluator_compatibility_status ?? 0,
      command_compatibility_status: existing?.command_compatibility_status ?? 0,
      evaluator_contract_version: existing?.evaluator_contract_version ?? 0,
      evaluator_prompt_version: existing?.evaluator_prompt_version ?? 0,
      evaluator_last_tested_at: existing?.evaluator_last_tested_at,
      evaluator_last_failure_reason: existing?.evaluator_last_failure_reason,
      structured_output_support: existing?.structured_output_support ?? 0,
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
    const existing = providerProfiles.find(p => p.id === selectedStateUpdaterProfileId);
    const profile: ProviderProfile = {
      id: selectedStateUpdaterProfileId || crypto.randomUUID(),
      name: trimmedName,
      base_url: stateUpdaterSettings.base_url,
      api_key: stateUpdaterSettings.api_key,
      model: stateUpdaterSettings.model,
      system_prompt: stateUpdaterSettings.system_prompt,
      narrator_timeout_ms: stateUpdaterSettings.narrator_timeout_ms ?? null,
      evaluator_timeout_ms: stateUpdaterSettings.evaluator_timeout_ms ?? DEFAULT_EVALUATOR_TIMEOUT_MS,
      structured_evaluator_timeout_ms: stateUpdaterSettings.structured_evaluator_timeout_ms ?? null,
      diagnostic_evaluator_timeout_ms: stateUpdaterSettings.diagnostic_evaluator_timeout_ms ?? null,
      evaluator_timeout_mode: stateUpdaterSettings.evaluator_timeout_mode ?? "finite",
      evaluator_mode: stateUpdaterSettings.evaluator_mode ?? DEFAULT_EVALUATOR_MODE,
      structured_evaluator_policy: stateUpdaterSettings.structured_evaluator_policy ?? "prefer",
      structured_evaluator_max_retries: stateUpdaterSettings.structured_evaluator_max_retries ?? 1,
      wait_for_evaluator_before_next_turn: stateUpdaterSettings.wait_for_evaluator_before_next_turn ?? true,
      allow_send_with_stale_state: stateUpdaterSettings.allow_send_with_stale_state ?? false,
      evaluator_background_enabled: stateUpdaterSettings.evaluator_background_enabled ?? false,
      anti_replay_forced_retry_enabled: stateUpdaterSettings.anti_replay_forced_retry_enabled ?? false,
      created_at: existing?.created_at ?? 0,
      updated_at: 0,
      narrator_compatibility_status: existing?.narrator_compatibility_status ?? 0,
      evaluator_compatibility_status: existing?.evaluator_compatibility_status ?? 0,
      command_compatibility_status: existing?.command_compatibility_status ?? 0,
      evaluator_contract_version: existing?.evaluator_contract_version ?? 0,
      evaluator_prompt_version: existing?.evaluator_prompt_version ?? 0,
      evaluator_last_tested_at: existing?.evaluator_last_tested_at,
      evaluator_last_failure_reason: existing?.evaluator_last_failure_reason,
      structured_output_support: existing?.structured_output_support ?? 0,
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

  async function handleArchiveProviderProfile(profileId: string) {
    if (busy || !profileId) return;
    const activeIds = [];
    if (selectedProviderProfileId) activeIds.push(selectedProviderProfileId);
    if (selectedStateUpdaterProfileId) activeIds.push(selectedStateUpdaterProfileId);
    if (
      selectedRepairProfileId &&
      selectedRepairProfileId !== REPAIR_MODEL_EVALUATOR &&
      selectedRepairProfileId !== REPAIR_MODEL_EMBEDDED
    ) {
      activeIds.push(selectedRepairProfileId);
    }
    if (activeIds.includes(profileId)) {
      await alertDialog("Cannot archive profile", "Cannot archive the active provider profile. Switch profiles first.");
      return;
    }
    setBusy(true);
    try {
      await archiveProviderProfile(profileId, activeIds);
      setProviderProfiles(await listProviderProfiles());
      setArchivedProviderProfiles(await listArchivedProviderProfiles());
      setStatus("Provider profile archived");
      logDev("warn", "warning", `Provider profile ${profileId} archived`);
    } catch (error) {
      reportError(error, "Provider profile archive failed", "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleRestoreProviderProfile(profileId: string) {
    if (busy || !profileId) return;
    setBusy(true);
    try {
      await restoreProviderProfile(profileId);
      setProviderProfiles(await listProviderProfiles());
      setArchivedProviderProfiles(await listArchivedProviderProfiles());
      setStatus("Provider profile restored");
      logDev("success", "app", `Provider profile ${profileId} restored`);
    } catch (error) {
      reportError(error, "Provider profile restore failed", "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteNarratorProviderProfile() {
    if (!selectedProviderProfileId) return;
    await handleArchiveProviderProfile(selectedProviderProfileId);
  }

  async function handleDeleteStateUpdaterProviderProfile() {
    if (!selectedStateUpdaterProfileId) return;
    await handleArchiveProviderProfile(selectedStateUpdaterProfileId);
  }

  async function handleCreateSnapshot() {
    if (!soul) return;
    const defaultName = `${soul.character_name} Snapshot ${formatSnapshotTimestamp(new Date())}`;
    const name = await promptDialog("Name this Soul snapshot", defaultName, {
      placeholder: "Snapshot name",
      confirmLabel: "Save Snapshot",
    });
    if (name === null) return;
    setBusy(true);
    try {
      const snapshot = await saveSessionAsNewSoul(soul.character_id, name, "checkpoint");
      setSouls(await listSouls());
      setStatus(`Created Soul snapshot: ${snapshot.character_name}`);
      logDev("success", "db", "Snapshot created from current Soul", {
        current_soul_id: soul.character_id,
        snapshot_soul_id: snapshot.character_id,
      });
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
      await saveSoulFile(`${soul.character_name.replace(/\s+/g, "_")}.soul.json`, soul);
      setStatus("Current Soul exported; app library was not modified");
      logDev("success", "app", "Current Soul exported without mutation", {
        soul_id: soul.character_id,
      });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleExportSoulMne() {
    if (!soul) return;
    setBusy(true);
    try {
      const result = await runTrackedBackgroundOperation(
        "export",
        "Export Soul .mne",
        () => exportCharacterSoulMne(soul.character_id, ""),
        {
          detail: soul.character_name,
          successDetail: (exportResult) => exportResult.path,
        },
      );
      setStatus(`Soul .mne exported: ${result.path}`);
      logDev("success", "app", "Soul .mne exported", {
        path: result.path,
        bundle_id: result.manifest.bundle_id,
        bundle_type: result.manifest.bundle_type,
        soul_id: result.manifest.soul_id,
      });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleExportSettingMne() {
    if (!setting) return;
    setBusy(true);
    try {
      const nextSetting = applySettingFields(setting);
      await upsertSetting(nextSetting);
      setSetting(nextSetting);
      const result = await runTrackedBackgroundOperation(
        "export",
        "Export World .mne",
        () => exportWorldSettingMne(nextSetting.setting_id, ""),
        {
          detail: nextSetting.setting_name,
          successDetail: (exportResult) => exportResult.path,
        },
      );
      setSettings(await listSettings());
      setStatus(`World .mne exported: ${result.path}`);
      logDev("success", "app", "World .mne exported", {
        path: result.path,
        bundle_id: result.manifest.bundle_id,
        bundle_type: result.manifest.bundle_type,
        world_id: result.manifest.world_id,
      });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleExportScenarioMne() {
    if (!soul || !setting) return;
    setBusy(true);
    try {
      const result = await runTrackedBackgroundOperation(
        "export",
        "Export Scenario .mne",
        () => exportScenarioBundleMne(soul.character_id, setting.setting_id, ""),
        {
          detail: `${soul.character_name} in ${setting.setting_name}`,
          successDetail: (exportResult) => exportResult.path,
        },
      );
      setStatus(`Scenario .mne exported: ${result.path}`);
      logDev("success", "app", "Scenario .mne exported", {
        path: result.path,
        bundle_id: result.manifest.bundle_id,
        bundle_type: result.manifest.bundle_type,
        soul_id: result.manifest.soul_id,
        world_id: result.manifest.world_id,
      });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleExportCurrentSessionMne() {
    if (!currentConversationId) return;
    setBusy(true);
    try {
      const result = await runTrackedBackgroundOperation(
        "export",
        "Export session checkpoint",
        () => exportCurrentSessionCheckpointMne(currentConversationId, ""),
        {
          detail: `Session ${currentConversationId}`,
          successDetail: (exportResult) => exportResult.path,
        },
      );
      setStatus(`Session checkpoint .mne exported: ${result.path}`);
      logDev("success", "app", "Session checkpoint .mne exported", {
        path: result.path,
        bundle_id: result.manifest.bundle_id,
        bundle_type: result.manifest.bundle_type,
        conversation_id: result.manifest.conversation_id,
      });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleImportMne() {
    const manualPath = await promptDialog("Import .mne bundle", "", {
      message: "Enter the absolute path to the .mne bundle.",
      placeholder: "C:\\path\\to\\bundle.mne",
      confirmLabel: "Import",
    });
    const filePath = manualPath?.trim() || null;
    if (!filePath) return;
    setBusy(true);
    try {
      const result = await runTrackedBackgroundOperation(
        "import",
        "Import .mne bundle",
        () => importMneBundle(filePath),
        {
          detail: filePath,
          successDetail: (importResult) => importResult.summary,
        },
      );
      setSouls(await listSouls());
      setSettings(await listSettings());
      setStatus(result.summary);
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
      setStatus("Setting exported to mnemosyne-exports");
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
      const importedSoul = await runTrackedBackgroundOperation(
        "import",
        "Import Soul file",
        async () => {
          const text = await file.text();
          let raw: any;
          if (file.name.endsWith(".md") || !text.trim().startsWith("{")) {
            raw = parseMarkdownSoul(text, file.name);
          } else {
            raw = JSON.parse(text);
          }
          const nextSoul = await soulFromImport(raw, file.name);
          await upsertSoul(nextSoul);
          setSoul(nextSoul);
          setSelectedCharacterIds((current) =>
            current.includes(nextSoul.character_id)
              ? [nextSoul.character_id, ...current.filter((id) => id !== nextSoul.character_id)]
              : [nextSoul.character_id, ...current],
          );
          setActiveConversationId(null);
          setCurrentSessionTitle("New Session");
          setCreatorFieldsFromSoul(nextSoul);
          setMessages([]);
          setSouls(await listSouls());
          return nextSoul;
        },
        {
          detail: file.name,
          successDetail: (result) => `Imported ${result.character_name}`,
        },
      );
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
      const importedSetting = await runTrackedBackgroundOperation(
        "import",
        "Import World file",
        async () => {
          const raw = JSON.parse(await file.text());
          const nextSetting = settingFromImport(raw, file.name);
          await upsertSetting(nextSetting);
          setSetting(nextSetting);
          setEditorFieldsFromSetting(nextSetting);
          setSettings(await listSettings());
          setActiveConversationId(null);
          setMessages([]);
          return nextSetting;
        },
        {
          detail: file.name,
          successDetail: (result) => `Imported ${result.setting_name}`,
        },
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
  const activeListenersCount = Object.values(listenerRegistrationRef.current).filter(Boolean).length;
  const { messages: activeMessages, trace: renderTrace } = useMemo(
    () => prepareMessagesForRender(messages, activeListenersCount),
    [messages, activeListenersCount],
  );
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    logDev(
      renderTrace.duplicate_saved_db_assistant_detected ? "error" : "debug",
      "stream",
      renderTrace.duplicate_saved_db_assistant_detected
        ? "duplicate_saved_db_assistant_detected"
        : "visible_bubble_trace",
      {
        visible_bubble_trace: renderTrace.visible_bubble_trace,
        duplicate_visual_pair: renderTrace.duplicate_visual_pair,
        duplicate_render_suppressed_count: renderTrace.duplicate_render_suppressed_count,
        active_listener_count: renderTrace.active_listener_count,
      },
    );
  }, [renderTrace]);
  const latestAssistantMessageId = useMemo(
    () =>
      [...activeMessages].reverse().find((message) => message.role === "assistant")?.id ?? null,
    [activeMessages],
  );
  const turnInProgress = busy || stateUpdating;
  const effectiveStateUpdaterSettings = useNarratorProviderForUpdater
    ? apiSettings
    : stateUpdaterSettings;
  const selectedBenchmarkPlayerProfileId =
    benchmarkPlayerProfileId || selectedProviderProfileId || selectedStateUpdaterProfileId;
  const updateEffectiveStateUpdaterSettings = (update: Partial<ApiProviderSettings>) => {
    if (useNarratorProviderForUpdater) {
      setApiSettings((current) => ({ ...current, ...update }));
    } else {
      setStateUpdaterSettings((current) => ({ ...current, ...update }));
    }
  };
  const activeEvaluatorJobIsLive =
    activeEvaluatorJob?.status === "pending" || activeEvaluatorJob?.status === "running";
  const showEvaluatorJobBanner = Boolean(activeEvaluatorJob) && !evaluatorJobBannerDismissed;
  const disclaimerScreen = (
    <DisclaimerScreen
      mode={disclaimerMode}
      understood={disclaimerUnderstood}
      remember={disclaimerRemember}
      onUnderstoodChange={setDisclaimerUnderstood}
      onRememberChange={setDisclaimerRemember}
      onAccept={handleAcceptDisclaimer}
      onClose={handleCloseDisclaimer}
    />
  );
  const providerModeControls = (
    <div className="settings-grid">
      <label className="field">
        <span>Narrator Style</span>
        <select
          value={mode}
          onChange={(event) => setMode(event.target.value as NarrativeMode)}
          disabled={busy}
        >
          <option>Realistic</option>
          <option>Reader</option>
          <option>Active Director</option>
          <option>GM Simulation</option>
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
          disabled={busy}
        >
          <option value="brief">Mnemosyne Brief</option>
          <option value="full_chat">Full Chat</option>
        </select>
      </label>
    </div>
  );
  const applyGenerationPreset = (preset: GenerationPresetName) => {
    if (preset === "custom") {
      setGenerationPreferences((current) => ({ ...current, preset }));
      return;
    }
    setGenerationPreferences((current) => ({
      ...current,
      ...GENERATION_PRESETS[preset],
      preset,
    }));
  };
  const updateGenerationSampler = (
    key: "temperature" | "topP" | "frequencyPenalty" | "presencePenalty",
    value: number,
  ) => {
    setGenerationPreferences((current) => ({
      ...current,
      [key]: value,
      preset: "custom",
    }));
  };
  const benchmarkStopReason = benchmarkResult?.scorecard.stop_reason ?? null;
  const benchmarkFailStoppedBeforeCompletion = Boolean(
    benchmarkResult &&
      benchmarkStopReason &&
      benchmarkResult.scorecard.visible_turns_completed < benchmarkResult.scorecard.visible_turns_requested,
  );
  const benchmarkEvaluatorFailStopped = benchmarkFailStoppedBeforeCompletion && benchmarkStopReason === "evaluator_failed";
  const benchmarkNarratorFailStopped = benchmarkFailStoppedBeforeCompletion && benchmarkStopReason === "narrator_failed";
  const skipAfterBenchmarkFailStop = (passed: boolean): boolean | "n/a" =>
    benchmarkFailStoppedBeforeCompletion ? "n/a" : passed;
  const skipGrowthAfterEvaluatorFailStop = (passed: boolean): boolean | "n/a" =>
    benchmarkEvaluatorFailStopped ? "n/a" : passed;
  const benchmarkScoreRows = benchmarkResult
    ? [
        ["Visible turns", skipAfterBenchmarkFailStop(benchmarkResult.scorecard.visible_turns_completed === benchmarkResult.scorecard.visible_turns_requested)],
        ["Visible user messages", skipAfterBenchmarkFailStop(benchmarkResult.scorecard.visible_user_messages_created === benchmarkResult.scorecard.visible_turns_requested)],
        ["Visible assistant messages", skipAfterBenchmarkFailStop(benchmarkResult.scorecard.visible_assistant_messages_created === benchmarkResult.scorecard.visible_turns_requested)],
        ["Duplicate turn rows", !benchmarkResult.scorecard.duplicate_turn_rows_detected],
        ["Visible chat messages created", skipAfterBenchmarkFailStop(benchmarkResult.scorecard.visible_chat_messages_created)],
        ["Normal pipeline used", skipAfterBenchmarkFailStop(benchmarkResult.scorecard.normal_pipeline_used)],
        ["Player simulator calls", benchmarkResult.scorecard.player_simulator_payload_count > 0 || benchmarkResult.benchmark_type === "scripted_visible_replay"],
        ["Narrator calls", benchmarkResult.scorecard.narrator_calls >= benchmarkResult.turn_count_completed],
        ["Evaluator calls", benchmarkResult.scorecard.evaluator_calls >= benchmarkResult.turn_count_completed],
        ["Evaluator waited each turn", benchmarkResult.scorecard.evaluator_waited_each_turn],
        ...(benchmarkEvaluatorFailStopped ? ([["Evaluator completed each turn", false]] as Array<[string, boolean | "n/a"]>) : []),
        ["Memory updated", skipGrowthAfterEvaluatorFailStop(benchmarkResult.scorecard.memory_updated)],
        [
          "Object state updated (when required)",
          skipGrowthAfterEvaluatorFailStop(
            benchmarkResult.object_identity_checks.length === 0 || benchmarkResult.scorecard.object_state_updated,
          ),
        ],
        ["Relationship updated", skipGrowthAfterEvaluatorFailStop(benchmarkResult.scorecard.relationship_updated)],
        ["Payload history exported", benchmarkResult.scorecard.payload_history_export_succeeded],
        ["Narrator response each turn", benchmarkNarratorFailStopped ? false : benchmarkResult.scorecard.narrator_visible_response_each_turn],
        // Strict-only checks are meaningless unless strict tool mode was requested.
        // Show n/a (with the mode that actually ran) instead of a misleading PASS.
        ["Tool call required path", benchmarkResult.scorecard.strict_tool_evaluator ? benchmarkResult.scorecard.evaluator_used_tool_call_where_required : "n/a"],
        ["No form fallback in strict mode", benchmarkResult.scorecard.strict_tool_evaluator ? benchmarkResult.scorecard.no_evaluator_form_v1_fallback_in_strict_mode : "n/a"],
        ["No syntactic repair in strict mode", benchmarkResult.scorecard.strict_tool_evaluator ? benchmarkResult.scorecard.syntactic_repair_unused_in_strict_mode : "n/a"],
        [`Evaluator mode actual: ${benchmarkResult.scorecard.evaluator_mode_actual || "unknown"}`, "n/a"],
        ["Local repair recovered state when warranted", benchmarkResult.scorecard.local_repair_recovered_state_when_warranted],
        // Surfaces a dead local endpoint distinctly from "repair tried and failed".
        ["Local repair endpoint reachable (when needed)", benchmarkResult.scorecard.local_repair_unavailable ? false : "n/a"],
        ["Memories increased", skipGrowthAfterEvaluatorFailStop(benchmarkResult.scorecard.memories_increased_over_time)],
        ["Active player relationship present", skipGrowthAfterEvaluatorFailStop(benchmarkResult.scorecard.active_player_relationship_changed_when_warranted)],
        ["Object IDs stable", benchmarkResult.scorecard.object_ids_stable],
        ["No default_player RP relationship leak", benchmarkResult.scorecard.default_player_not_normal_rp_relationship_target],
        [".mne export succeeded", benchmarkResult.scorecard.mne_export_succeeded],
      ] as Array<[string, boolean | "n/a"]>
    : [];
  const benchmarkRunnerPanel = (
    <section className="settings-section provider-pass-card">
      <div className="provider-pass-heading">
        <div>
          <span className="eyebrow">Dev</span>
          <h3>Benchmark Runner</h3>
          <p>Drives real visible chat turns through the normal narrator and evaluator pipeline.</p>
        </div>
        <span className="provider-status-pill">
          {benchmarkResult ? (benchmarkResult.scorecard.pass ? "PASS" : "FAIL") : "Idle"}
        </span>
      </div>
      <div className="provider-pass-grid">
        <div className="field" style={{ gridColumn: "1 / -1" }}>
          <span>Form-Eval Dry-Run (open session)</span>
          <p className="provider-note">
            Replays this session's chat log through the non-tool-call FORM evaluator and the
            repair path, validating each result the way the live system does, but applies
            nothing to the session. Uses the selected Evaluator profile. Slow on CPU models.
          </p>
          <button
            type="button"
            className="ghost-action"
            onClick={() => void handleSessionFormEvalBenchmark()}
            disabled={formEvalBusy || !currentConversationId}
          >
            <span>{formEvalBusy ? "Running form-eval..." : "Run form-eval benchmark on this session"}</span>
          </button>
          {formEvalReport && (
            <div style={{ marginTop: "8px", fontSize: "0.8rem" }}>
              <div>
                <strong>{formEvalReport.model}</strong> - {formEvalReport.turns_total} turns -
                form-valid {formEvalReport.form_passed} - failed {formEvalReport.form_failed} -
                repair-recovered {formEvalReport.repair_recovered} - repair via{" "}
                <strong>{formEvalReport.repair_model}</strong>              </div>
              <ul style={{ margin: "4px 0 0", paddingLeft: "1.1rem" }}>
                {formEvalReport.per_turn.map((turn) => (
                  <li
                    key={turn.turn_index}
                    style={{
                      color: turn.form_passed
                        ? "#86efac"
                        : turn.repair_recovered
                          ? "#fbbf24"
                          : "#f87171",
                    }}
                  >
                    turn {turn.turn_index}:{" "}
                    {turn.form_passed
                      ? `form OK (${turn.form_rows_accepted} rows accepted)`
                      : `form FAIL${
                          turn.repair_attempted
                            ? turn.repair_recovered
                              ? ` ??repair recovered (${turn.repair_ops} ops)`
                              : turn.repair_error
                                ? ` ??repair ERROR: ${turn.repair_error.slice(0, 90)}`
                                : ` ??repair parsed ${turn.repair_ops} ops but committed no state (no-op/under-extraction)`
                            : ""
                        }`}
                    {turn.form_error && !turn.form_passed
                      ? ` ??form: ${turn.form_error.slice(0, 90)}`
                      : ""}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
        <label className="field">
          <span>Benchmark Mode</span>
          <select
            value={benchmarkType}
            onChange={(event) => setBenchmarkType(event.target.value as BenchmarkType)}
            disabled={benchmarkRunning}
          >
            <option value="visible_ai_chat">Visible AI Chat</option>
            <option value="scripted_visible_replay">Scripted Visible Replay</option>
            <option value="headless_regression">Headless Regression</option>
            <option value="multi_agent_visible_chat">Multi-Agent Visible Chat</option>
          </select>
        </label>
        <label className="field">
          <span>Target</span>
          <select
            value={benchmarkTarget}
            onChange={(event) => setBenchmarkTarget(event.target.value as BenchmarkTarget)}
            disabled={benchmarkRunning}
          >
            <option value="current_session">Current Session</option>
            <option value="new_benchmark_session_from_current_soul">New Benchmark Session From Current Soul</option>
            <option value="new_benchmark_session_from_selected_soul_world">New Benchmark Session From Selected Soul/World</option>
          </select>
        </label>
        <label className="field">
          <span>Player Simulator Profile</span>
          <select
            value={selectedBenchmarkPlayerProfileId}
            onChange={(event) => setBenchmarkPlayerProfileId(event.target.value)}
            disabled={benchmarkRunning}
          >
            <option value="">Select profile</option>
            {providerProfiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Player Character (optional)</span>
          <select
            value={benchmarkPlayerCharacterSoulId}
            onChange={(event) => setBenchmarkPlayerCharacterSoulId(event.target.value)}
            disabled={benchmarkRunning}
          >
            <option value="">User persona only</option>
            {souls
              .filter((item) => item.character_id !== soul?.character_id)
              .map((item) => (
                <option key={item.character_id} value={item.character_id}>
                  {item.character_name}
                </option>
              ))}
          </select>
          <span className="hint">
            Play a second character against the narrator instead of only the user
            persona. The persona stays in the scene either way.
          </span>
        </label>
        <label className="field">
          <span>Player Goal</span>
          <textarea
            value={benchmarkPlayerGoal}
            onChange={(event) => setBenchmarkPlayerGoal(event.target.value)}
            disabled={benchmarkRunning}
            rows={3}
          />
        </label>
        <label className="field">
          <span>Turn Count</span>
          <input
            type="number"
            min="1"
            max="50"
            value={benchmarkTurnCount}
            onChange={(event) => setBenchmarkTurnCount(Math.max(1, Number(event.target.value) || 1))}
            disabled={benchmarkRunning}
          />
        </label>
        <label className="field">
          <span>Structured Evaluator Transport</span>
          <select
            value={benchmarkStrictToolEvaluator ? "tool_call" : benchmarkTransport ?? "auto"}
            onChange={(event) => setBenchmarkTransport(event.target.value)}
            disabled={benchmarkRunning || benchmarkStrictToolEvaluator}
          >
            <option value="tool_call">tool_call</option>
            <option value="auto">auto</option>
            <option value="json_schema">json_schema</option>
            <option value="json_object">json_object</option>
            <option value="prompt_json">prompt_json</option>
          </select>
        </label>
      </div>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={benchmarkStrictToolEvaluator}
          onChange={(event) => setBenchmarkStrictToolEvaluator(event.target.checked)}
          disabled={benchmarkRunning}
        />
        <span>Strict Tool Evaluator (diagnostic probe - overrides your chat evaluator settings)</span>
      </label>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={benchmarkWaitForEvaluator}
          onChange={(event) => setBenchmarkWaitForEvaluator(event.target.checked)}
          disabled={benchmarkRunning}
        />
        <span>Wait For Evaluator Each Turn</span>
      </label>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={benchmarkTraditionalOpponent}
          onChange={(event) => setBenchmarkTraditionalOpponent(event.target.checked)}
          disabled={benchmarkRunning}
        />
        <span>Traditional RP opponent (full chat, no memory - comparison)</span>
      </label>
      <div className="button-row">
        <button
          type="button"
          className="ghost-action"
          onClick={() => void handleRunBenchmark()}
          disabled={busy || benchmarkRunning || !soul}
        >
          <Play size={16} />
          <span>{benchmarkRunning ? "Running Benchmark..." : "Run Benchmark"}</span>
        </button>
        <button
          type="button"
          className="ghost-action"
          onClick={() => handleStopBenchmark()}
          disabled={!benchmarkLiveActive}
        >
          <Square size={16} />
          <span>Stop Benchmark</span>
        </button>
      </div>
      {(benchmarkResult || benchmarkError) && (
        <div className="provider-note">
          {benchmarkResult ? (
            <>
              <strong>{benchmarkResult.scorecard.pass ? "PASS" : "FAIL"}</strong>{" "}
              {benchmarkResult.benchmark_type} visible turns: {benchmarkResult.scorecard.visible_turns_completed} /{" "}
              {benchmarkResult.scorecard.visible_turns_requested}.
              <br />
              Internal evaluator retries: {benchmarkResult.scorecard.internal_evaluator_retry_count} rows /{" "}
              {benchmarkResult.scorecard.internal_evaluator_retry_payload_count} payloads. Player simulator calls:{" "}
              {benchmarkResult.scorecard.player_simulator_payload_count}. Duplicate turn rows:{" "}
              {benchmarkResult.scorecard.duplicate_turn_rows_detected ? "FAIL" : "PASS"}.
              <br />
              Scorecard:{" "}
              {benchmarkScoreRows
                .map(([label, passed]) => `${passed === "n/a" ? "n/a" : passed ? "PASS" : "FAIL"} ${label}`)
                .join("; ")}
              {benchmarkResult.scorecard.failure_reasons.length ? (
                <>
                  <br />
                  Failure reasons: {benchmarkResult.scorecard.failure_reasons.join(", ")}
                </>
              ) : null}
              {benchmarkResult.scorecard.narrator_provider_error ? (
                <>
                  <br />
                  <strong>narrator_provider_error:</strong> {benchmarkResult.scorecard.narrator_provider_error}
                </>
              ) : null}
              {benchmarkResult.scorecard.stop_reason ? (
                <>
                  <br />
                  <strong>stop_reason:</strong> {benchmarkResult.scorecard.stop_reason}
                </>
              ) : null}
              {benchmarkResult.scorecard.failed_stage ? (
                <>
                  <br />
                  <strong>failed_stage:</strong> {benchmarkResult.scorecard.failed_stage}
                </>
              ) : null}
              <br />
              <strong>relationship_target_checked:</strong>{" "}{benchmarkResult.scorecard.relationship_target_checked ?? "none"}
              <br />
              <strong>relationship_changed_from:</strong>{" "}{JSON.stringify(benchmarkResult.scorecard.relationship_changed_from ?? null)}
              <br />
              <strong>relationship_changed_to:</strong>{" "}{JSON.stringify(benchmarkResult.scorecard.relationship_changed_to ?? null)}
              <br />
              <strong>relationship_delta_patch_ids:</strong>{" "}{benchmarkResult.scorecard.relationship_delta_patch_ids.join(", ") || "none"}
              <br />
              <strong>relationship_delta_sources:</strong>{" "}{benchmarkResult.scorecard.relationship_delta_sources.join(", ") || "none"}
              <br />
              <strong>evaluator_provider_failures:</strong>{" "}{benchmarkResult.scorecard.evaluator_provider_failures}
              <br />
              <strong>structured_provider_429_count:</strong>{" "}{benchmarkResult.scorecard.structured_provider_429_count}
              <br />
              <strong>evaluator_response_failed_count:</strong>{" "}{benchmarkResult.scorecard.evaluator_response_failed_count}
              <br />
              <strong>evaluator_empty_patch_count:</strong>{" "}{benchmarkResult.scorecard.evaluator_empty_patch_count}
              <br />
              <strong>form_rows_rejected_count:</strong>{" "}{benchmarkResult.scorecard.form_rows_rejected_count}
              <br />
              <strong>local_repair_invoked_count:</strong>{" "}{benchmarkResult.scorecard.local_repair_invoked_count}
              <br />
              <strong>local_reextract_invoked_count:</strong>{" "}{benchmarkResult.scorecard.local_reextract_invoked_count}
              <br />
              <strong>local_repair_payload_count:</strong>{" "}{benchmarkResult.scorecard.local_repair_payload_count}
              <br />
              <strong>local_repair_response_count:</strong>{" "}{benchmarkResult.scorecard.local_repair_response_count}
              <br />
              <strong>local_repair_state_patch_count:</strong>{" "}{benchmarkResult.scorecard.local_repair_state_patch_count}
              <br />
              <strong>completed_visible_turns:</strong> {benchmarkResult.scorecard.visible_turns_completed}
              <br />
              <strong>requested_turns:</strong> {benchmarkResult.scorecard.visible_turns_requested}
              <br />
              <strong>player_simulator_calls:</strong> {benchmarkResult.scorecard.player_simulator_calls}
              <br />
              <strong>narrator_calls:</strong> {benchmarkResult.scorecard.narrator_calls}
              <br />
              <strong>evaluator_calls:</strong> {benchmarkResult.scorecard.visible_turns_completed === 0 ? "skipped_due_to_no_completed_turn" : benchmarkResult.scorecard.evaluator_calls}
              <br />
              {benchmarkResult.scorecard.token_comparison ? (() => {
                const t = benchmarkResult.scorecard.token_comparison!;
                const perTurn = (total: number, turns: number) =>
                  turns > 0 ? Math.round(total / turns).toLocaleString() : "-";
                const ratio =
                  t.mnemosyne_total_tokens > 0 && t.traditional_total_tokens > 0
                    ? (t.traditional_total_tokens / t.mnemosyne_total_tokens).toFixed(2)
                    : null;
                return (
                  <>
                    <strong>token cost</strong>{" "}
                    <span className="muted">
                      ({t.provider_reported ? "provider-reported" : "estimated"})
                    </span>
                    <table className="benchmark-token-table">
                      <thead>
                        <tr>
                          <th>engine</th>
                          <th>prompt</th>
                          <th>completion</th>
                          <th>total</th>
                          <th>per turn</th>
                        </tr>
                      </thead>
                      <tbody>
                        <tr>
                          <td>Mnemosyne (narrator+evaluator)</td>
                          <td>{(t.narrator_prompt_tokens + t.evaluator_prompt_tokens).toLocaleString()}</td>
                          <td>{(t.narrator_completion_tokens + t.evaluator_completion_tokens).toLocaleString()}</td>
                          <td>{t.mnemosyne_total_tokens.toLocaleString()}</td>
                          <td>{perTurn(t.mnemosyne_total_tokens, t.mnemosyne_turns)}</td>
                        </tr>
                        <tr>
                          <td>Traditional (full transcript)</td>
                          <td>{t.traditional_prompt_tokens.toLocaleString()}</td>
                          <td>{t.traditional_completion_tokens.toLocaleString()}</td>
                          <td>{t.traditional_total_tokens.toLocaleString()}</td>
                          <td>{perTurn(t.traditional_total_tokens, t.traditional_turns)}</td>
                        </tr>
                      </tbody>
                    </table>
                    {ratio ? (
                      <span className="muted">
                        Traditional costs {ratio}x Mnemosyne over this run.
                      </span>
                    ) : (
                      <span className="muted">
                        Run the traditional opponent too for a side-by-side ratio.
                      </span>
                    )}
                    <br />
                    <span className="muted">
                      Player simulator (harness, excluded):{" "}
                      {t.player_simulator_total_tokens.toLocaleString()} over {t.player_simulator_calls} call(s)
                    </span>
                    <br />
                  </>
                );
              })() : null}
              Payload: {benchmarkResult.payload_history_path ?? "not exported"}
              <br />
              MNE: {benchmarkResult.mne_export_path ?? "not exported"}
              <br />
              Summary: {benchmarkResult.summary_json_path ?? "not exported"}
            </>
          ) : (
            <>
              <strong>FAIL</strong> {benchmarkError}
            </>
          )}
        </div>
      )}
    </section>
  );
  const providerSettingsPanel = (
    <div className="settings-tab-panel">
      <section className="settings-section">
        <div className="settings-section-heading">
          <div>
            <span className="eyebrow">Provider</span>
            <h3>AI Connection</h3>
          </div>
          <button type="button" className="ghost-action compact-ghost" onClick={handleViewDisclaimer}>
            View Disclaimer
          </button>
        </div>
        <p className="settings-note">API presets are saved in the existing local provider profile store.</p>
      </section>

      {provider === "API" ? (
        <>
          <NarratorProviderPanel
            apiSettings={apiSettings}
            busy={busy}
            knownModels={knownModels}
            mode={mode}
            onArchive={() => void handleDeleteNarratorProviderProfile()}
            onNameChange={setNarratorProviderProfileName}
            onRememberModel={rememberModel}
            onSave={() => void handleSaveNarratorProviderProfile()}
            onSelectProfile={(profileId) => void handleSelectProviderProfile(profileId)}
            onSettingsChange={(update) =>
              setApiSettings((current) => ({ ...current, ...update }))
            }
            profileName={narratorProviderProfileName}
            profiles={providerProfiles}
            selectedProfileId={selectedProviderProfileId}
          />

          <StateUpdaterProviderPanel
            apiSettings={apiSettings}
            busy={busy}
            effectiveSettings={effectiveStateUpdaterSettings}
            evaluatorExecutionMode={evaluatorExecutionMode}
            onArchive={() => void handleDeleteStateUpdaterProviderProfile()}
            onExecutionModeChange={updateEvaluatorExecutionMode}
            onNameChange={setUpdaterProviderProfileName}
            onRememberModel={rememberModel}
            onSave={() => void handleSaveStateUpdaterProviderProfile()}
            onSelectProfile={(profileId) => void handleSelectStateUpdaterProfile(profileId)}
            onSettingsChange={updateEffectiveStateUpdaterSettings}
            onToggleUseNarrator={setUseNarratorProviderForUpdater}
            onUpdaterSettingsChange={(update) =>
              setStateUpdaterSettings((current) => ({ ...current, ...update }))
            }
            profileName={updaterProviderProfileName}
            profiles={providerProfiles}
            selectedProfileId={selectedStateUpdaterProfileId}
            stateUpdaterSettings={stateUpdaterSettings}
            useNarratorProvider={useNarratorProviderForUpdater}
          />

          <RepairModelPanel
            busy={embeddedModelBusy}
            embeddedModel={embeddedModel}
            embeddedModelError={embeddedModelError}
            embeddedModelPath={embeddedModelPath}
            onEmbeddedModelPathChange={setEmbeddedModelPath}
            onSelectProfile={setSelectedRepairProfileId}
            onStart={() => void handleStartEmbeddedModel()}
            onStop={() => void handleStopEmbeddedModel()}
            profiles={providerProfiles}
            selectedProfileId={selectedRepairProfileId}
          />

          <ProviderProfilesPanel
            archivedProfiles={archivedProviderProfiles}
            busy={busy}
            onArchive={(profileId) => void handleArchiveProviderProfile(profileId)}
            onRestore={(profileId) => void handleRestoreProviderProfile(profileId)}
            profiles={providerProfiles}
            selectedNarratorProfileId={selectedProviderProfileId}
            selectedRepairProfileId={selectedRepairProfileId}
            selectedUpdaterProfileId={selectedStateUpdaterProfileId}
          />
        </>
      ) : null}
    </div>
  );
  const devEvaluatorSettingsPanel = (
    <section className="settings-section dev-only-settings">
      <div className="settings-section-heading">
        <div>
          <span className="eyebrow">Dev Only</span>
          <h3>Evaluator Contract</h3>
        </div>
      </div>
      <div className="settings-grid">
        <label className="field">
          <span>Evaluator Mode</span>
          <select
            value={effectiveStateUpdaterSettings.evaluator_mode ?? "evaluator_form_v1"}
            onChange={(event) =>
              updateEffectiveStateUpdaterSettings({
                evaluator_mode: event.target.value,
              })
            }
            disabled={busy}
          >
            <option value="evaluator_form_v1">Legacy Form Evaluator</option>
            <option value="evaluator_perception_v2">Perception V2 (Preview)</option>
            <option value="evaluator_structured_v1">Structured Ops Evaluator</option>
          </select>
        </label>
        <label className="field">
          <span>Structured Policy</span>
          <select
            value={effectiveStateUpdaterSettings.structured_evaluator_policy ?? "prefer"}
            onChange={(event) =>
              updateEffectiveStateUpdaterSettings({
                structured_evaluator_policy: event.target.value,
              })
            }
            disabled={busy}
          >
            <option value="required">required</option>
            <option value="prefer">prefer</option>
            <option value="allow_fallback">allow_fallback</option>
          </select>
        </label>
        <label className="field">
          <span>Structured Transport</span>
          <select
            value={structuredEvaluatorTransport}
            onChange={(event) => updateStructuredEvaluatorTransport(event.target.value)}
            disabled={busy}
          >
            <option value="auto">Auto - tool calls first, then JSON schema</option>
            <option value="tool_call">Tool calls (require real function calls)</option>
            <option value="json_schema">JSON schema (response_format)</option>
            <option value="json_object">JSON object</option>
            <option value="prompt_json">Prompt-only JSON</option>
          </select>
        </label>
      </div>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={effectiveStateUpdaterSettings.evaluator_background_enabled ?? false}
          onChange={(event) =>
            updateEffectiveStateUpdaterSettings({
              evaluator_background_enabled: event.target.checked,
            })
          }
          disabled={busy}
        />
        <span>Run evaluator in background</span>
      </label>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={effectiveStateUpdaterSettings.wait_for_evaluator_before_next_turn ?? true}
          onChange={(event) =>
            updateEffectiveStateUpdaterSettings({
              wait_for_evaluator_before_next_turn: event.target.checked,
            })
          }
          disabled={busy}
        />
        <span>Wait for state update before next turn</span>
      </label>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={effectiveStateUpdaterSettings.allow_send_with_stale_state ?? false}
          onChange={(event) =>
            updateEffectiveStateUpdaterSettings({
              allow_send_with_stale_state: event.target.checked,
            })
          }
          disabled={busy}
        />
        <span>Allow send with stale state</span>
      </label>
    </section>
  );
  const generationSettingsPanel = (
    <GenerationSettingsPanel
      busy={busy}
      generationPreferences={generationPreferences}
      onApplyPreset={applyGenerationPreset}
      onReset={() => setGenerationPreferences(DEFAULT_GENERATION_PREFERENCES)}
      onSetMaxTokens={(maxTokens) =>
        setGenerationPreferences((current) => ({ ...current, maxTokens }))
      }
      onSetContextMaxTokens={(contextMaxTokens) =>
        setGenerationPreferences((current) => ({ ...current, contextMaxTokens }))
      }
      onUpdateSampler={updateGenerationSampler}
      providerModeControls={providerModeControls}
    />
  );
  const readingSettingsPanel = (
    <ReadingSettingsPanel
      onChange={(update) =>
        setReadingPreferences((current) => ({ ...current, ...update }))
      }
      onReset={() => setReadingPreferences(DEFAULT_READING_PREFERENCES)}
      readingPreferences={readingPreferences}
    />
  );
  const settingsDrawerToggle = (
    <button
      type="button"
      className={`settings-drawer-toggle ${settingsDrawerOpen ? "open" : ""}`}
      onClick={() => {
        setSettingsDrawerOpen((open) => !open);
      }}
    >
      <SettingsIcon size={16} />
      <span>Settings</span>
    </button>
  );
  const dataSettingsPanel = (
    <DataSettingsPanel
      busy={busy}
      chatStartMode={chatStartMode}
      onChatStartModeChange={setChatStartMode}
      onOpenDataFolder={() => void openSessionDataLocation()}
      onShowArchivedSessionsChange={setShowArchivedSessions}
      showArchivedSessions={showArchivedSessions}
    />
  );
  const aboutSettingsPanel = <AboutSettingsPanel onViewDisclaimer={handleViewDisclaimer} />;
  const activeSettingsPanel =
    settingsTab === "ai"
      ? providerSettingsPanel
      : settingsTab === "generation"
        ? generationSettingsPanel
        : settingsTab === "reading"
          ? readingSettingsPanel
          : settingsTab === "data"
            ? dataSettingsPanel
            : aboutSettingsPanel;
  const settingsDrawerContent = (
    <QuickSettingsPanel
      busy={busy}
      generationPreferences={generationPreferences}
      onApplyPreset={applyGenerationPreset}
      onOpenFullSettings={() => {
        setSettingsDrawerOpen(false);
        setView("settings");
      }}
      onSetMaxTokens={(maxTokens) =>
        setGenerationPreferences((current) => ({ ...current, maxTokens }))
      }
      onUpdateSampler={updateGenerationSampler}
      providerModeControls={providerModeControls}
    />
  );
  const settingsDrawerPanel = settingsDrawerOpen ? (
    <aside className="settings-drawer-panel" aria-label="Settings">
      <header className="settings-drawer-header">
        <div>
          <span className="eyebrow">Play</span>
          <h2>Quick Settings</h2>
        </div>
        <button type="button" onClick={() => setSettingsDrawerOpen(false)}>
          Close
        </button>
      </header>
      {settingsDrawerContent}
    </aside>
  ) : null;
  const appDialogNode = <AppDialog dialog={appDialog} onResolve={resolveAppDialog} />;
  if (disclaimerMode) {
    return disclaimerScreen;
  }

  const railShellClass = railCollapsed ? " rail-collapsed" : "";
  const railNav = (
    <nav className={`app-rail${railShellClass}`} aria-label="Primary navigation">
      <div className="app-rail-brand" role="img" aria-label="Mnemosyne" title="Mnemosyne">
        <strong aria-hidden="true">Mnemosyne</strong>
        <span aria-hidden="true">the narrator writes / the state map remembers</span>
      </div>
      <button
        type="button"
        className="app-rail-collapse"
        onClick={() => setRailCollapsed((collapsed) => !collapsed)}
        title={railCollapsed ? "Expand navigation" : "Collapse navigation"}
        aria-label={railCollapsed ? "Expand navigation" : "Collapse navigation"}
      >
        {railCollapsed ? <PanelLeftOpen size={16} aria-hidden="true" /> : <PanelLeftClose size={16} aria-hidden="true" />}
      </button>
      <button type="button" className={`app-rail-item${view === "home" ? " is-active" : ""}`} aria-label="Home" title="Home" onClick={() => setView("home")}>
        <Home size={18} aria-hidden="true" />
        <span>Home</span>
      </button>
      <button type="button" className={`app-rail-item${view === "chat" ? " is-active" : ""}`} aria-label="Play" title="Play" onClick={() => void handleOpenMostRecentChat()}>
        <Play size={18} aria-hidden="true" />
        <span>Play</span>
      </button>
      <button type="button" className={`app-rail-item${view === "statemap" ? " is-active" : ""}`} aria-label="State Map" title="State Map" onClick={() => { setView("statemap"); void refreshSessionStateHub(); }}>
        <Brain size={18} aria-hidden="true" />
        <span>State Map</span>
      </button>
      <button type="button" className={`app-rail-item${view === "editor" || view === "library" ? " is-active" : ""}`} aria-label="Library" title="Library" onClick={() => setView("library")}>
        <FolderOpen size={18} aria-hidden="true" />
        <span>Library</span>
      </button>
      <button type="button" className={`app-rail-item${view === "settings" ? " is-active" : ""}`} aria-label="Settings" title="Settings" onClick={() => { setSettingsDrawerOpen(false); setView("settings"); }}>
        <SettingsIcon size={18} aria-hidden="true" />
        <span>Settings</span>
      </button>
      <div className="app-rail-spacer" />
      <div className="app-rail-engine" aria-label="Engine status">
        <span>Engine</span>
        <dl>
          <div><dt>Narrator</dt><dd>{apiSettings.model || "No model"}</dd></div>
          <div><dt>Evaluator</dt><dd>{effectiveStateUpdaterSettings.evaluator_mode ?? "form"}</dd></div>
          <div><dt>Mode</dt><dd>{mode}</dd></div>
        </dl>
      </div>
    </nav>
  );

  if (view === "chat") {
    const currentConversation = conversations.find((c) => c.conversation_id === currentConversationId);
    const isCurrentSessionArchived = currentConversation
      ? (Boolean(currentConversation.archived_at) || currentConversation.title.startsWith("[Archived] "))
      : currentSessionTitle.startsWith("[Archived] ");
    const defaultPipelineStages = [
      { stage_name: "Input queued", status: "ready", elapsed_ms: 0 },
      { stage_name: "Narrator", status: "waiting", elapsed_ms: 0 },
      { stage_name: "Evaluator", status: "waiting", elapsed_ms: 0 },
      { stage_name: "Repair pass", status: "waiting", elapsed_ms: 0 },
      { stage_name: "State map", status: "waiting", elapsed_ms: 0 },
    ];
    const pipelineSteps = latestPipelineTrace?.stages.length ? latestPipelineTrace.stages : defaultPipelineStages;
    const pipelineSummary = latestPipelineTrace
      ? `${latestPipelineTrace.final_status} / ${latestPipelineTrace.total_elapsed_ms}ms`
      : "Idle / awaiting input";

    if (devModeActive) {
      const devStream: DevStreamItem[] = [
        ...activeMessages.map((m) => ({
          kind: "chat" as const,
          key: `c${m.id}`,
          t: m.created_at,
          role: m.role,
          content: m.content,
        })),
        ...devLogs.map((l) => ({
          kind: "log" as const,
          key: `l${l.id}`,
          t: l.timestamp,
          level: l.level,
          category: l.category,
          message: l.message,
        })),
      ].sort((a, b) => a.t - b.t);

      const monitoredJobs = Object.values(devJobsById).filter(
        (job) => !dismissedDevJobIds.has(job.job_id),
      );
      const cancelMonitoredJob = (job: BackgroundJobProgress) => {
        if (job.kind === "benchmark" && benchmarkLiveActive) {
          handleStopBenchmark();
          return;
        }
        if (job.kind === "evaluator" && activeEvaluatorJobIsLive) {
          void handleCancelEvaluatorJob();
        }
      };

      const devPipelineRail = (
        <DevPipelinePanel
          hasPipelineTrace={Boolean(latestPipelineTrace)}
          jobs={monitoredJobs}
          onCancelJob={cancelMonitoredJob}
          onDismissJob={dismissTrackedDevJob}
          pipelineStages={pipelineSteps}
          pipelineSummary={pipelineSummary}
        />
      );
      const devStreamPanel = (
        <DevStreamPanel items={devStream} streamRef={devStreamRef} />
      );
      const devDiagnosticsPanel = (
            <aside className="cli-diagnostics" aria-label="Dev and settings panel">
              <div className="cli-diag-tabs">
                <button
                  type="button"
                  className={`cli-btn${devPanelTab === "dev" ? " selected" : ""}`}
                  onClick={() => setDevPanelTab("dev")}
                >
                  [ DEV ]
                </button>
                <button
                  type="button"
                  className={`cli-btn${devPanelTab === "settings" ? " selected" : ""}`}
                  onClick={() => setDevPanelTab("settings")}
                >
                  [ SETTINGS ]
                </button>
                <button
                  type="button"
                  className={`cli-btn${devPanelTab === "benchmarks" ? " selected" : ""}`}
                  onClick={() => setDevPanelTab("benchmarks")}
                >
                  [ BENCHMARKS ]
                </button>
              </div>
              {devPanelTab === "settings" ? (
                <div className="cli-embedded-settings">
                  {devEvaluatorSettingsPanel}
                  <section className="settings-section dev-only-settings">
                    <div className="settings-section-heading">
                      <div>
                        <span className="eyebrow">Dev Only</span>
                        <h3>Evaluator Gates</h3>
                      </div>
                    </div>
                    <label className="toggle-row">
                      <input
                        type="checkbox"
                        checked={devOverrideActive}
                        onChange={(event) => setDevOverrideActive(event.target.checked)}
                      />
                      <span>Allow untested or incompatible evaluator profiles</span>
                    </label>
                    <p className="settings-note">
                      This bypass only lasts for the current app run. Compatibility tests and raw evaluator
                      diagnostics remain in the DEV tab.
                    </p>
                  </section>
                  {providerSettingsPanel}
                </div>
              ) : devPanelTab === "benchmarks" ? (
                <div className="cli-embedded-settings">{benchmarkRunnerPanel}</div>
              ) : (
                <>
              <section className="cli-diag-card">
                <div className="cli-diag-title">// MEMORY CYCLE</div>
                <dl className="cli-diag-grid">
                  <div><dt>core</dt><dd>{soul?.memory.core.length ?? 0}</dd></div>
                  <div><dt>recent</dt><dd>{soul?.memory.recent.length ?? 0}</dd></div>
                  <div><dt>schemas</dt><dd>{soul?.memory.schemas.length ?? 0}</dd></div>
                  <div><dt>turns</dt><dd>{turnsSinceConsolidation}</dd></div>
                  <div><dt>context</dt><dd>{context?.truncated ? "truncated" : context ? "within budget" : "no payload"}</dd></div>
                  <div><dt>tokens</dt><dd>{context?.estimated_tokens ?? 0}</dd></div>
                </dl>
              </section>
              <section className="cli-diag-card">
                <div className="cli-diag-title">// API DEBUG</div>
                <dl className="cli-diag-grid">
                  <div><dt>provider</dt><dd>{provider}</dd></div>
                  <div><dt>mode</dt><dd>{mode}</dd></div>
                  <div><dt>model</dt><dd>{apiSettings.model || "none"}</dd></div>
                  <div><dt>evaluator</dt><dd>{effectiveStateUpdaterSettings.evaluator_mode ?? "form"}</dd></div>
                  <div><dt>transport</dt><dd>{structuredEvaluatorTransport}</dd></div>
                  <div><dt>fallback</dt><dd>{effectiveStateUpdaterSettings.structured_evaluator_policy ?? "prefer"}</dd></div>
                </dl>
              </section>
              <section className="cli-diag-card">
                <div className="cli-diag-title">// LLM PAYLOAD</div>
                <dl className="cli-diag-grid">
                  <div><dt>system</dt><dd>{llmPayload?.estimated_tokens.system ?? 0}</dd></div>
                  <div><dt>context</dt><dd>{llmPayload?.estimated_tokens.context ?? 0}</dd></div>
                  <div><dt>user</dt><dd>{llmPayload?.estimated_tokens.user ?? 0}</dd></div>
                  <div><dt>total</dt><dd>{llmPayload?.estimated_tokens.total ?? 0}</dd></div>
                </dl>
                <div className="cli-diag-actions">
                  <button type="button" className="cli-mini-btn" onClick={handleCopyLlmPayload} disabled={!llmPayload}>[ COPY PAYLOAD ]</button>
                  <button type="button" className="cli-mini-btn" onClick={handleExportLlmPayloadHistory} disabled={busy || !currentConversationId}>[ EXPORT HISTORY ]</button>
                </div>
              </section>
              <section className="cli-diag-card danger">
                <div className="cli-diag-title">// DESTRUCTIVE REPAIR</div>
                <div className="cli-diag-actions grid">
                  <button type="button" className="cli-mini-btn" onClick={() => void handleSoulRepair("world")} disabled={busy || !soul}>[ CLEAR WORLD ]</button>
                  <button type="button" className="cli-mini-btn" onClick={() => void handleSoulRepair("scenario")} disabled={busy || !soul}>[ CLEAR SCENARIO ]</button>
                  <button type="button" className="cli-mini-btn" onClick={() => void handleSoulRepair("events")} disabled={busy || !soul}>[ CLEAR EVENTS ]</button>
                  <button type="button" className="cli-mini-btn danger" onClick={() => void handleSoulRepair("memories")} disabled={busy || !soul}>[ CLEAR MEMORIES ]</button>
                </div>
              </section>
              <section className="cli-diag-card">
                <div className="cli-diag-title">// EVALUATOR CHECK</div>
                <div className="cli-diag-actions">
                  <button
                    type="button"
                    className="cli-mini-btn"
                    onClick={() => {
                      if (selectedStateUpdaterProfileId)
                        void handleRunContractTest(selectedStateUpdaterProfileId);
                      else setStatus("Select an Evaluator profile in the Settings tab first.");
                    }}
                    disabled={busy}
                  >
                    [ CONTRACT TEST ]
                  </button>
                  <button
                    type="button"
                    className="cli-mini-btn"
                    title="Run Structured Evaluator Diagnostic"
                    onClick={() => void handleRunStructuredDiagnostic()}
                    disabled={busy || structuredDiagnosticRunning || (!selectedStateUpdaterProfileId && !selectedProviderProfileId)}
                  >
                    [ STRUCTURED DIAG ]
                  </button>
                  <button
                    type="button"
                    className="cli-mini-btn"
                    onClick={() => void handleRetrySessionRepair()}
                    disabled={retryRepairBusy}
                  >
                    [ {retryRepairBusy ? "RETRYING" : "RETRY REPAIR"} ]
                  </button>
                </div>
                {(structuredDiagnosticResult || structuredDiagnosticError) && (
                  <div className="cli-diag-note">
                    {structuredDiagnosticResult ? (
                      <>
                        {structuredDiagnosticResult.provider_model} /{" "}
                        {structuredDiagnosticResult.structured_enforcement_per_run.join(", ") || "none"}
                      </>
                    ) : (
                      <>FAIL / {structuredDiagnosticError}</>
                    )}
                  </div>
                )}
              </section>
              <section className="cli-diag-card">
                <div className="cli-diag-title">// SESSION REPAIR</div>
                <div className="cli-diag-actions grid">
                  <button type="button" className="cli-mini-btn" onClick={() => void handleRunDevCommand("dedupe_active_adjacent_user_messages", {})} disabled={devCommandRunning || !currentConversationId}>[ DEDUPE TURNS ]</button>
                  <button type="button" className="cli-mini-btn" onClick={() => void handleRunDevCommand("restore_inactive_messages", {})} disabled={devCommandRunning || !currentConversationId}>[ RESTORE HIDDEN ]</button>
                  <button type="button" className="cli-mini-btn" onClick={() => void handleRunDevCommand("repair_accidental_normal_send_variants", {})} disabled={devCommandRunning || !currentConversationId}>[ REPAIR VARIANTS ]</button>
                  <button type="button" className="cli-mini-btn" onClick={() => void handleRunDevCommand("rebuild_session_from_ledger", {})} disabled={devCommandRunning || !currentConversationId}>[ REBUILD LEDGER ]</button>
                  <button type="button" className="cli-mini-btn" onClick={() => void handleRunDevCommand("inspect_turn_branch_integrity", {})} disabled={devCommandRunning || !currentConversationId}>[ INSPECT BRANCH ]</button>
                  <button type="button" className="cli-mini-btn" onClick={() => void handleRunDevCommand("get_branch_patch_debug", {})} disabled={devCommandRunning || !currentConversationId}>[ PATCH DEBUG ]</button>
                </div>
                {devCommandResult ? <pre className="cli-diag-pre">{devCommandResult}</pre> : null}
              </section>
                </>
              )}
            </aside>
      );
      const devInputForm = (
          <form className="cli-input" onSubmit={handleDevTerminalSubmit}>
            <span className="cli-prompt">root@mnemosyne:~$</span>
            <input
              value={devTerminalInput}
              onChange={(event) => setDevTerminalInput(event.target.value)}
              placeholder="/chat <message> to speak - /help"
              spellCheck={false}
              autoComplete="off"
              autoFocus
            />
            <span className="cli-cursor" aria-hidden="true">|</span>
          </form>
      );

      return (
        <DevModeShell
          appDialogNode={appDialogNode}
          diagnostics={devDiagnosticsPanel}
          inputForm={devInputForm}
          onExitDev={() => setDevModeActive(false)}
          onOpenLibrary={() => setView("library")}
          pipelineRail={devPipelineRail}
          sessionTitle={currentSessionTitle || "session"}
          stream={devStreamPanel}
        />
      );
    }

    const chatPipelineRail = (
      <ChatPipelineRail
        latestPipelineTrace={latestPipelineTrace}
        pipelineSteps={pipelineSteps}
        pipelineSummary={pipelineSummary}
      />
    );

    return (
      <ChatView
        appDialogNode={appDialogNode}
        pipelineRail={chatPipelineRail}
        railNav={railNav}
        railShellClass={railShellClass}
      >
        <ChatWorkspaceHeader
          avatar={selectedAvatarAsset}
          busy={busy}
          characterName={soul?.character_name ?? "Mnemosyne"}
          contextTokens={context?.estimated_tokens ?? 0}
          devLogCount={devLogs.length}
          hasConversation={Boolean(currentConversationId)}
          mode={mode}
          moreMenu={
            <ChatMoreMenu
              activeMessageCount={activeMessages.length}
              busy={busy}
              currentConversationId={currentConversationId}
              isArchived={isCurrentSessionArchived}
              menuOpen={chatMoreMenuOpen}
              menuRef={chatMoreMenuRef}
              onArchive={() => void handleDeleteChat()}
              onExportChat={() => void handleExportVisibleChatLog()}
              onExportSession={() => void handleExportCurrentSessionMne()}
              onOpenDevMode={() => setDevModeActive(true)}
              onRestoreHiddenTurns={handleRestoreHiddenTurns}
              onRestoreSession={() => void handleRestoreSession()}
              setMenuOpen={setChatMoreMenuOpen}
              triggerRef={chatMoreButtonRef}
            />
          }
          onBackToLibrary={() => setView("library")}
          onOpenDevMode={() => setDevModeActive(true)}
          onRename={() => void handleRenameCurrentSession()}
          provider={provider}
          sessionContinuityLabel={sessionContinuityLabel}
          sessionTitle={currentSessionTitle || "New Session"}
          settingName={setting?.setting_name ?? "Local Setting"}
          settingsToggle={settingsDrawerToggle}
        />

        <ChatTranscript
          avatar={selectedAvatarAsset}
          bodyRef={chatOnlyBodyRef}
          bottomRef={chatBottomRef}
          busy={busy}
          canGenerateFromUserMessage={canGenerateFromUserMessage}
          characterName={soul?.character_name ?? "Mnemosyne"}
          messages={activeMessages}
          onDeleteMessage={(message) => void handleDeleteChatMessage(message)}
          onEditMessage={(message) => void handleEditUserMessage(message)}
          onFixMessage={(message) => void handleFixFromUserMessage(message)}
          onJumpToLatest={jumpToLatest}
          onPreviewImage={setPreviewImageAsset}
          onRegenerateMessage={(message) => void handleRegenerateFromUserMessage(message)}
          onScroll={handleChatScroll}
          onSelectVariant={(message, direction) => void handleSelectVariant(message, direction)}
          showJumpToLatest={showJumpToLatest}
          turnInProgress={turnInProgress}
          variantsByMessage={variantsByMessage}
        />

        {showEvaluatorJobBanner ? (
          <EvaluatorStatusBanner
            allowProceedWithStaleState={
              effectiveStateUpdaterSettings.allow_send_with_stale_state ?? false
            }
            isLive={activeEvaluatorJobIsLive}
            job={activeEvaluatorJob}
            onCancel={() => void handleCancelEvaluatorJob()}
            onClose={handleDismissEvaluatorJobBanner}
            onProceed={handleProceedWithStaleState}
            onRetry={() => void handleRetryEvaluatorJob()}
            title={activeEvaluatorJob ? evaluatorJobBannerTitle(activeEvaluatorJob) : ""}
          />
        ) : null}

        {personaModalMode ? (
          <PersonaModal
            activePersona={activePlayerPersona}
            archivedPersonas={archivedPlayerPersonas}
            busy={busy}
            form={personaForm}
            listConfirmRequired={personaListConfirmRequired}
            mode={personaModalMode}
            modalRef={personaModalRef}
            onArchive={(persona) => void handleArchivePersona(persona)}
            onBackdropClose={closePersonaModal}
            onCancelForm={() => setPersonaModalMode("list")}
            onClose={closePersonaModal}
            onConfirmList={handleConfirmPersonaList}
            onEdit={(personaId) => void openPersonaEdit(personaId)}
            onOpenAdd={openPersonaAdd}
            onRestore={(persona) => void handleRestoreArchivedPersona(persona)}
            onSave={() => void handleSavePersona()}
            onSelect={(personaId) => void handleSelectPersona(personaId)}
            personas={playerPersonas}
            setForm={setPersonaForm}
          />
        ) : null}

        <ChatComposer
          busy={busy}
          characterName={soul?.character_name ?? "Mnemosyne"}
          draft={draft}
          imageInputRef={chatImageInputRef}
          onDraftChange={handleDraftChange}
          onImageSelected={handleChatImageSelected}
          onInsertSlashCommand={insertSlashCommand}
          onKeyDown={handleComposerKeyDown}
          onStopGeneration={handleStopGeneration}
          onSubmit={handleSubmit}
          selectedSlashIndex={slashSelectedIndex}
          slashMenuOpen={slashMenuOpen}
          slashSuggestions={slashSuggestions}
          soulAvailable={Boolean(soul)}
          stateUpdating={stateUpdating}
        />
        <ImagePreviewModal asset={previewImageAsset} onClose={() => setPreviewImageAsset(null)} />
        {settingsDrawerPanel}
      </ChatView>
    );
  }

  if (view === "home") {
    return (
      <HomeView appDialogNode={appDialogNode} railNav={railNav} railShellClass={railShellClass}>
        <HomeDashboard
          busy={busy}
          conversations={conversations}
          onOpenLibrary={() => setView("library")}
          onSelectConversation={(conversation) => void handleSelectConversation(conversation)}
          onSelectSoul={(soulId) => void handleSelectSoul(soulId)}
          souls={souls}
        />
      </HomeView>
    );
  }

  if (view === "statemap") {
    return (
      <StateMapView appDialogNode={appDialogNode} railNav={railNav} railShellClass={railShellClass}>
        <StateMapDashboard
          busy={busy}
          onBackToPlay={() => setView("chat")}
          onRefresh={() => void refreshSessionStateHub()}
          stateMap={sessionStateMap}
        />
      </StateMapView>
    );
  }

  if (view === "settings") {
    return (
      <SettingsPageView
        activePanel={activeSettingsPanel}
        appDialogNode={appDialogNode}
        onTabChange={setSettingsTab}
        railNav={railNav}
        railShellClass={railShellClass}
        settingsTab={settingsTab}
      />
    );
  }

  return (
    <LibraryView appDialogNode={appDialogNode} railNav={railNav} railShellClass={railShellClass}>
      <header className="launcher-header">
        <div>
          <span className="eyebrow">{view === "editor" ? "Workshop" : "Launcher"}</span>
          <h1>{view === "editor" ? "Edit Library" : "Choose Scene"}</h1>
          <p>
            {setting?.setting_name ?? "No world selected"} / {soul?.character_name ?? "No primary character"} /{" "}
            {selectedCharacterCount} selected
          </p>
        </div>
        <div className="launcher-actions">
          {view === "editor" ? (
            <button type="button" className="ghost-action" onClick={() => setView("library")}>
              <ArrowLeft size={16} />
              <span>Back to Home</span>
            </button>
          ) : (
            <button type="button" className="ghost-action primary-cta" onClick={() => setView("editor")}>
              <Pencil size={16} />
              <span>Create / Edit</span>
            </button>
          )}
        </div>
      </header>

      <input ref={settingImportInputRef} className="hidden-file" type="file" accept="application/json,.json,.setting" onChange={handleImportSettingFile} />
      <input ref={importInputRef} className="hidden-file" type="file" accept="application/json,.json,.soul,.md,.txt" onChange={handleImportSoulFile} />

      {view === "library" && (
      <section className="library-grid launcher-grid">
        <section className="workspace-card library-card">
          <header className="panel-header">
            <div>
              <span className="eyebrow">World</span>
              <h2>{setting?.setting_name ?? "Select a World"}</h2>
              <p>{selectedWorldSummary}</p>
            </div>
            <Database aria-hidden="true" />
          </header>

          <section className="compact-list library-list world-picker-list" aria-label="Saved worlds">
            {settings.length === 0 ? (
              <div className="grid-empty">
                <p className="muted">No worlds yet.</p>
                <button type="button" className="ghost-action primary-cta" onClick={() => { void handleCreateSetting(); setView("editor"); }} disabled={busy}>
                  <Sparkles size={16} />
                  <span>Create your first world</span>
                </button>
              </div>
            ) : (
              settings.map((item) => (
                <button
                  key={item.setting_id}
                  className={`soul-row world-row ${setting?.setting_id === item.setting_id ? "selected" : ""}`}
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

          {archivedSettings.length > 0 ? (
            <section className="compact-list library-list archived-resource-list" aria-label="Archived worlds">
              <div className="list-section-heading">
                <strong>Archived Worlds</strong>
                <span className="muted">{archivedSettings.length}</span>
              </div>
              {archivedSettings.map((item) => (
                <article key={item.setting_id} className="soul-row archived-resource-row">
                  <span>{item.setting_name}</span>
                  <small>
                    {item.turn_counter} turns / {item.location || "No location"}
                  </small>
                  <button
                    type="button"
                    className="ghost-action"
                    onClick={() => void handleRestoreArchivedSetting(item.setting_id)}
                    disabled={busy}
                    title="Restore archived world"
                  >
                    <RefreshCcw size={14} />
                    <span>Restore</span>
                  </button>
                </article>
              ))}
            </section>
          ) : null}
        </section>

        <section className="workspace-card library-card">
          <header className="panel-header character-heading">
            <div>
              <span className="eyebrow">Primary Character</span>
              <h2>{soul?.character_name ?? "Select a Character"}</h2>
              <p>{primaryCharacterDescription}</p>
            </div>
            <SoulAvatar soulName={soul?.character_name ?? "Mnemosyne"} asset={selectedAvatarAsset} />
          </header>

          <section className="character-grid" aria-label="Saved characters">
            {souls.length === 0 ? (
              <div className="grid-empty">
                <p className="muted">No characters yet - Mnemosyne ships with none; bring your own.</p>
                <button type="button" className="ghost-action primary-cta" onClick={() => { void handleCreateSoul(); setView("editor"); }} disabled={busy}>
                  <Sparkles size={16} />
                  <span>Create your first character</span>
                </button>
              </div>
            ) : (
              souls.map((item) => (
                <article
                  key={item.character_id}
                  className={`character-card ${soul?.character_id === item.character_id ? "primary" : ""} ${
                    selectedCharacterIds.includes(item.character_id) ? "selected" : ""
                  }`}
                >
                  <button
                    type="button"
                    className="character-card-main"
                    onClick={() => handleSelectSoul(item.character_id)}
                    disabled={busy}
                  >
                    <span className="soul-row-avatar">
                      {item.avatar_image_id ? <ImageIcon size={14} /> : item.character_name.slice(0, 1)}
                    </span>
                    <span className="character-card-copy">
                      <strong>{item.character_name}</strong>
                      <small>
                        {item.core_count} core / {item.recent_count} recent
                        {item.avatar_image_id ? " / avatar" : " / no avatar"}
                      </small>
                    </span>
                  </button>
                  <div className="character-card-footer">
                    <label className="mini-toggle">
                      <input
                        type="checkbox"
                        checked={selectedCharacterIds.includes(item.character_id)}
                        onChange={() => handleToggleCharacterSelection(item.character_id)}
                        disabled={busy}
                      />
                      <span>Select</span>
                    </label>
                    {soul?.character_id === item.character_id ? <span className="primary-pill">Primary</span> : null}
                  </div>
                </article>
              ))
            )}
          </section>

          {archivedSouls.length > 0 ? (
            <section className="compact-list library-list archived-resource-list" aria-label="Archived characters">
              <div className="list-section-heading">
                <strong>Archived Characters</strong>
                <span className="muted">{archivedSouls.length}</span>
              </div>
              {archivedSouls.map((item) => (
                <article key={item.character_id} className="soul-row archived-resource-row">
                  <span>{item.character_name}</span>
                  <small>
                    {item.core_count} core / {item.recent_count} recent
                    {item.avatar_image_id ? " / avatar" : " / no avatar"}
                  </small>
                  <button
                    type="button"
                    className="ghost-action"
                    onClick={() => void handleRestoreArchivedSoul(item.character_id)}
                    disabled={busy}
                    title="Restore archived character"
                  >
                    <RefreshCcw size={14} />
                    <span>Restore</span>
                  </button>
                </article>
              ))}
            </section>
          ) : null}
        </section>
      </section>
      )}

      {view === "library" && (
      <section className="workspace-card launch-card">
        <div>
          <span className="eyebrow">Ready</span>
          <h1>
            {soul?.character_name ?? "Choose a Soul"} in {setting?.setting_name ?? "a Setting"}
          </h1>
          <p className="launcher-primary-note">
            Multi-select is staged for later group chat support. Start Chat uses the Primary character only.
          </p>
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
              <span>New isolated Session</span>
            </label>
          </div>
          <p className="session-state-label">
            {chatStartMode === "fresh"
              ? "Starts an isolated chat from the selected Soul; the library Soul remains unchanged."
              : "Using persistent Soul continuity"}
          </p>
          <section className="session-list" aria-label="Saved chats">
            <div className="session-list-heading">
              <div className="session-list-title">
                <span className="eyebrow">{showArchivedSessions ? "Archived chats" : "Active chats"}</span>
                <strong>{visibleConversations.length}</strong>
              </div>
              <div className="session-list-actions">
                <button
                  type="button"
                  className="session-data-folder-button"
                  title="Open the local session database folder in Explorer"
                  onClick={() => void handleOpenSessionDataLocation()}
                  disabled={busy}
                >
                  <FolderOpen size={14} />
                  <span>Open in Explorer</span>
                </button>
                <label className="archive-toggle">
                  <input
                    type="checkbox"
                    checked={showArchivedSessions}
                    onChange={(event) => setShowArchivedSessions(event.target.checked)}
                  />
                  <span>Show archived</span>
                </label>
              </div>
            </div>
            {visibleConversations.length === 0 ? (
              <p className="muted">
                {showArchivedSessions
                  ? "No archived chats for this Soul."
                  : "No active named chats for this Soul yet."}
              </p>
            ) : (
              <>
                {paginatedConversations.map((conversation) => (
                  <div
                    key={conversation.conversation_id}
                    className="session-row"
                  >
                    <button type="button" onClick={() => handleSelectConversation(conversation)} disabled={busy}>
                      <span>{conversation.title}</span>
                      <small>
                        {conversation.message_count} messages
                        {conversation.last_message_preview ? ` / ${conversation.last_message_preview}` : ""}
                      </small>
                    </button>
                    {showArchivedSessions ? (
                      <button
                        type="button"
                        className="session-delete-button"
                        title="Restore session"
                        onClick={() => handleRestoreSession(conversation.conversation_id)}
                        disabled={busy}
                      >
                        <RefreshCcw size={14} />
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="session-delete-button"
                        title="Archive session"
                        onClick={() => handleDeleteChat(conversation.conversation_id)}
                        disabled={busy}
                      >
                        <Trash2 size={14} />
                      </button>
                    )}
                  </div>
                ))}
                {sessionListTotalPages > 1 ? (
                  <nav className="session-list-pagination" aria-label="Chat list pages">
                    <button
                      type="button"
                      onClick={() => setSessionListPage((page) => Math.max(1, page - 1))}
                      disabled={sessionListPage <= 1 || busy}
                    >
                      Previous
                    </button>
                    <span>
                      {sessionListPage} / {sessionListTotalPages}
                    </span>
                    <button
                      type="button"
                      onClick={() =>
                        setSessionListPage((page) => Math.min(sessionListTotalPages, page + 1))
                      }
                      disabled={sessionListPage >= sessionListTotalPages || busy}
                    >
                      Next
                    </button>
                  </nav>
                ) : null}
              </>
            )}
          </section>
        </div>
        <button className="start-chat-button" onClick={handleStartChat} disabled={!soul || busy}>
          <MessageSquareText size={20} />
          <span>Start Chat With Primary</span>
        </button>
      </section>
      )}

      {view === "editor" && (
      <section className="play-grid">
        <div className="editor-toolbar">
          <div className="editor-action-group">
            <span className="editor-action-label">World</span>
            <button type="button" title="New World" onClick={handleCreateSetting} disabled={busy}><Sparkles size={16} /><span>New</span></button>
            <button type="button" title="Import World JSON" onClick={() => settingImportInputRef.current?.click()} disabled={busy}><FileUp size={16} /><span>Import</span></button>
            <button type="button" title="Export World JSON" onClick={handleSaveSetting} disabled={!setting || busy}><FileDown size={16} /><span>Export</span></button>
            <button type="button" title="Export World as .mne" onClick={handleExportSettingMne} disabled={!setting || busy}><FileDown size={16} /><span>.mne</span></button>
            <button type="button" title="Persist current World" onClick={async () => { const next = await persistCurrentSetting(); if (next) setStatus("World saved"); }} disabled={!setting || busy}><Save size={16} /><span>Save</span></button>
            <button type="button" className="ghost-action danger" title="Archive World" onClick={handleArchiveSetting} disabled={!setting || busy}><Archive size={16} /><span>Archive</span></button>
          </div>
          <div className="editor-action-group">
            <span className="editor-action-label">Character</span>
            <button type="button" title="New Character" onClick={handleCreateSoul} disabled={busy}><Sparkles size={16} /><span>New</span></button>
            <button type="button" title="Import Character JSON" onClick={() => importInputRef.current?.click()} disabled={busy}><FileUp size={16} /><span>Import</span></button>
            <button type="button" title="Import .mne bundle" onClick={handleImportMne} disabled={busy}><FileUp size={16} /><span>.mne In</span></button>
            <button type="button" title="Snapshot Character to library" onClick={handleCreateSnapshot} disabled={!soul || busy}><Save size={16} /><span>Snapshot</span></button>
            <button type="button" title="Export Character JSON" onClick={handleSaveSoul} disabled={!soul || busy}><FileDown size={16} /><span>Export</span></button>
            <button type="button" title="Export Character as .mne" onClick={handleExportSoulMne} disabled={!soul || busy}><FileDown size={16} /><span>Char .mne</span></button>
            <button type="button" title="Export Character + World as Scenario .mne" onClick={handleExportScenarioMne} disabled={!soul || !setting || busy}><FileDown size={16} /><span>Scenario</span></button>
            <button type="button" title="Run consolidation (Sleep)" onClick={handleConsolidate} disabled={!soul || busy}><RefreshCcw size={16} /><span>Sleep</span></button>
            <button type="button" className="ghost-action danger" title="Archive Character" onClick={handleDeleteSoul} disabled={!soul || busy}><Archive size={16} /><span>Archive</span></button>
            <button type="button" className="ghost-action purge" title="Hard-delete Character - permanent" onClick={handlePurgeSoul} disabled={!soul || busy}><Trash2 size={16} /><span>Purge</span></button>
          </div>
        </div>
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
                    <span>Opening Narrator Message</span>
                    <textarea
                      value={openingNarratorMessage}
                      onChange={(event) => setOpeningNarratorMessage(event.target.value)}
                      placeholder="Optional first narrator message shown when starting a new session from this Soul."
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

          <section
            className={`creator-section workspace-card collapsible-section ${
              knowledgeEditorOpen ? "open" : ""
            }`}
          >
            <button
              className="section-toggle studio-toggle"
              type="button"
              onClick={() => setKnowledgeEditorOpen((open) => !open)}
              aria-expanded={knowledgeEditorOpen}
            >
              <span>
                <span className="eyebrow">Relationship</span>
                <strong>What they know about each other</strong>
              </span>
              <ChevronDown size={18} aria-hidden="true" />
            </button>

            {knowledgeEditorOpen ? (
              <div className="section-body">
                <p className="hint">
                  Where the story starts. Once it is running the engine works most of this
                  out from the transcript — a name counts as known once it has been said —
                  so this is for what it cannot see: a previous session, or something that
                  happened off-screen.
                </p>
                <KnowledgeGrid conversationId={currentConversationId} busy={busy} />
              </div>
            ) : null}
          </section>
        </aside>
      </section>
      )}

      <ImagePreviewModal asset={previewImageAsset} onClose={() => setPreviewImageAsset(null)} />
    </LibraryView>
  );
}
