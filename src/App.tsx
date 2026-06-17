import {
  Archive,
  ArrowLeft,
  Brain,
  ChevronDown,
  Clipboard,
  Database,
  FileDown,
  FolderOpen,
  FileUp,
  Image as ImageIcon,
  MessageSquareText,
  Pencil,
  Play,
  RefreshCcw,
  Save,
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
  exportLlmPayloadHistory,
  exportCharacterSoulMne,
  exportCurrentSessionCheckpointMne,
  exportScenarioBundleMne,
  exportWorldSettingMne,
  exportVisibleChatLog,
  getBranchPatchDebug,
  getLatestEvaluatorJob,
  getSetting,
  getImageAsset,
  getImageAssetDataUrl,
  getActivePlayerPersona,
  getSoul,
  inspectTurnBranchIntegrity,
  importImageAssetFromFile,
  importMneBundle,
  validateMneBundle,
  previewMneImport,
  importMneAsNew,
  listConversations,
  listArchivedSessions,
  openSessionDataLocation,
  listProviderProfiles,
  listPlayerPersonas,
  listAssistantMessageVariants,
  listConversationMessages,
  listSettings,
  listSouls,
  listenApiStream,
  listenChatMessageSaved,
  listenDevLog,
  listenEvaluatorJobStatusChanged,
  listenPipelineTraceUpdated,
  previewApiPayload,
  rebuildSessionFromLedger,
  repairAccidentalNormalSendVariants,
  renameConversation,
  restoreInactiveMessages,
  retryEvaluatorJob,
  runConsolidation,
  saveSessionAsNewSoul,
  saveSettingFile,
  saveSoulFile,
  selectAssistantMessageVariant,
  sendApiTurn,
  sendMockTurn,
  setActivePlayerPersona,
  updateUserMessage,
  upsertPlayerPersona,
  upsertProviderProfile,
  upsertSetting,
  upsertSoul,
  runBenchmark,
  prepareBenchmarkSession,
  generateBenchmarkPlayerMessage,
  benchmarkTurnSummary,
  finalizeBenchmark,
  BenchmarkSessionInit,
  BenchmarkTurnSummary,
  runEvaluatorContractTest,
  runStructuredEvaluatorDiagnostic,
  StructuredEvaluatorDiagnosticSummary,
  setActiveEvaluatorProfile,
  listenEvaluatorAutoFallbackTriggered,
} from "./tauri";

const DEFAULT_CONVERSATION_ID = "local-mock";
const CONSOLIDATION_INTERVAL_TURNS = 10;
const NARRATOR_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:narrator_provider_profile_id";
const UPDATER_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:state_updater_provider_profile_id";
const USE_NARRATOR_FOR_UPDATER_STORAGE_KEY = "mnemosyne:use_narrator_provider_for_updater";
const CUSTOM_NARRATOR_PROMPT_STORAGE_KEY = "mnemosyne:custom_narrator_prompt";
const SETTINGS_DRAWER_OPEN_STORAGE_KEY = "mnemosyne:settings_drawer_open";
const SETTINGS_DRAWER_TAB_STORAGE_KEY = "mnemosyne:settings_drawer_tab";
const SETTINGS_FIRST_LAUNCH_SEEN_STORAGE_KEY = "mnemosyne:settings_first_launch_seen_v1";
const CHAT_START_MODE_STORAGE_KEY = "mnemosyne:chat_start_mode";
const SHOW_ARCHIVED_SESSIONS_STORAGE_KEY = "mnemosyne:show_archived_sessions";
const EVALUATOR_EXECUTION_MODE_STORAGE_KEY = "mnemosyne:evaluator_execution_mode";
const STRUCTURED_EVALUATOR_TRANSPORT_STORAGE_KEY = "mnemosyne:structured_evaluator_transport";
const SESSIONS_PER_PAGE = 10;
const DEV_CONSOLE_PAUSED_STORAGE_KEY = "mnemosyne:dev_console_paused";
const DEV_LOG_LEVEL_FILTER_STORAGE_KEY = "mnemosyne:dev_log_level_filter";
const DEV_LOG_CATEGORY_FILTER_STORAGE_KEY = "mnemosyne:dev_log_category_filter";
const DISCLAIMER_STORAGE_KEY = "mnemosyne_disclaimer_accepted_v1";
const DISCLAIMER_VERSION = 1;
const DEV_LOG_LIMIT = 1000;
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
const DEV_LOG_LEVELS: DevLogLevel[] = ["info", "warn", "error", "debug", "success"];
type ProviderKind = "Mock" | "API";
type NarrativeMode = "Realistic" | "Reader" | "Active Director" | "GM Simulation" | "Custom";
type AppView = "library" | "chat";
type ChatStartMode = "continue" | "fresh";
type DisclaimerMode = "launch" | "manual" | null;
type SettingsTab = "ai" | "chat" | "library" | "dev";
type DevCommandName =
  | "dedupe_active_adjacent_user_messages"
  | "restore_inactive_messages"
  | "get_branch_patch_debug"
  | "rebuild_session_from_ledger"
  | "inspect_turn_branch_integrity"
  | "repair_accidental_normal_send_variants"
  | "export_visible_chat_log"
  | "export_llm_payload_history"
  | "run_benchmark";

const DEV_COMMAND_OPTIONS: Array<{ name: DevCommandName; label: string; defaultArgs: string }> = [
  {
    name: "dedupe_active_adjacent_user_messages",
    label: "Repair Duplicate Turns",
    defaultArgs: "{}",
  },
  {
    name: "restore_inactive_messages",
    label: "Restore Hidden Turns",
    defaultArgs: "{}",
  },
  {
    name: "get_branch_patch_debug",
    label: "Get Branch Patch Debug",
    defaultArgs: "{}",
  },
  {
    name: "rebuild_session_from_ledger",
    label: "Rebuild Session From Ledger",
    defaultArgs: "{}",
  },
  {
    name: "inspect_turn_branch_integrity",
    label: "Inspect Branch Integrity",
    defaultArgs: "{}",
  },
  {
    name: "repair_accidental_normal_send_variants",
    label: "Repair Accidental Variants",
    defaultArgs: "{}",
  },
  {
    name: "export_visible_chat_log",
    label: "Export Visible Chat",
    defaultArgs: "{}",
  },
  {
    name: "export_llm_payload_history",
    label: "Export Payload History",
    defaultArgs: "{}",
  },
  {
    name: "run_benchmark",
    label: "Run Benchmark",
    defaultArgs: JSON.stringify(
      {
        benchmark_type: "visible_ai_chat",
        target: "current_session",
        turn_count: 5,
        strict_tool_evaluator: true,
        player_goal: "Build cautious trust with the active Soul while respecting boundaries.",
      },
      null,
      2,
    ),
  },
];

type ActiveGeneration = {
  id: number;
  conversationId: string;
  narratorSaved: boolean;
  knownAssistantIds: Set<number>;
  replacementAssistantId?: number;
  replacementOriginalContent?: string;
};

// Mutable state for a live AI-vs-AI benchmark run. Held in a ref (not React
// state) so the per-turn effect reads/mutates it without re-render churn.
type BenchmarkLiveContext = {
  benchmarkId: string;
  conversationId: string;
  soulId: string;
  startedAt: number;
  playerProfileId: string;
  playerGoal: string;
  settings: BenchmarkSettings;
  narratorSettings: ApiProviderSettings;
  updaterSettings: ApiProviderSettings;
  initialMemoryCount: number;
  initialObjectCount: number;
  initialRelationshipCount: number;
  perTurn: BenchmarkTurnSummary[];
  narratorFailures: number;
  completedTurns: number;
  nextTurnIndex: number;
  lastPlayerText: string;
};

interface MessageRenderTrace {
  frontend_message_render_count: number;
  saved_message_count: number;
  pending_message_count: number;
  rendered_message_count: number;
  duplicate_saved_suppressed: number;
  duplicate_pending_suppressed: number;
  pending_replaced_by_saved: number;
  pending_assistant_replaced_by_saved: number;
  active_listener_count: number;
  pending_assistant_count: number;
  rendered_saved_message_count: number;
  rendered_pending_message_count: number;
  duplicate_render_suppressed_count: number;
  duplicate_visual_pair: boolean;
  duplicate_saved_db_assistant_detected: boolean;
  visible_bubble_trace: VisibleBubbleTraceRow[];
}

type VisibleBubbleRenderSource =
  | "saved_db"
  | "pending_overlay"
  | "streaming_overlay"
  | "local_optimistic"
  | "unknown";

interface VisibleBubbleTraceRow {
  render_index: number;
  role: ChatMessage["role"];
  render_source: VisibleBubbleRenderSource;
  message_id: number;
  request_id?: string;
  assistant_message_id?: number;
  turn_id?: string;
  content_hash: string;
  created_at: number;
  status?: string;
  origin?: string;
  duplicate_visual_pair?: boolean;
  duplicate_render_sources?: VisibleBubbleRenderSource[];
}

function evaluatorJobStatusText(job: EvaluatorJob) {
  if (job.status === "pending" || job.status === "running") return "Updating memory/state...";
  if (job.status === "completed" || job.status === "partial_success") {
    if (job.error_message && (job.error_message.startsWith("State updated") || job.error_message.includes("skipped"))) {
      return job.error_message;
    }
  }
  if (job.patch_applied) {
    if (job.error_message && (job.error_message.startsWith("State updated") || job.error_message.includes("skipped"))) {
      return job.error_message;
    }
  } else if (job.status === "failed") {
    return "State update failed";
  }
  if (job.status === "completed") {
    return job.patch_applied ? "Memory/state update completed" : "Memory/state update completed with no patch";
  }
  if (job.status === "partial_success") {
    if (job.error_message?.includes("some enrichment rows rejected")) {
      return "State updated; some enrichment rows rejected";
    }
    if (job.error_message?.includes("branch_advanced_before_background_evaluator_completed")) {
      return "State updated; enrichment finished after branch advanced";
    }
    return "State updated partially";
  }
  if (job.status === "some_rows_rejected") return "State updated; some enrichment rows rejected";
  if (job.status === "stale_skipped") return "State updated; enrichment skipped";
  if (job.status === "canceled") return "State update canceled";
  if (job.status === "timed_out") return "State update timed out";
  if (job.status === "failed") return "State update failed";
  return job.status;
}

function evaluatorJobBannerTitle(job: EvaluatorJob) {
  if (job.status === "pending" || job.status === "running") return "Updating memory/state...";
  if (job.patch_applied) {
    if (job.error_message && (job.error_message.startsWith("State updated") || job.error_message.includes("skipped"))) {
      return job.error_message;
    }
  }
  if (job.status === "completed") return "Memory/state updated";
  if (job.status === "partial_success") return evaluatorJobStatusText(job);
  if (job.status === "some_rows_rejected") return "State updated; some enrichment rows rejected";
  if (job.status === "stale_skipped") return "State updated; enrichment skipped";
  if (job.status === "canceled") return "State update canceled";
  if (job.status === "timed_out") return "State update timed out";
  if (job.status === "failed") return "State update failed";
  return job.status;
}

function evaluatorJobRefreshesState(job: EvaluatorJob) {
  return ["completed", "partial_success", "some_rows_rejected", "stale_skipped"].includes(job.status);
}
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
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
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
  const [devOverrideActive, setDevOverrideActive] = useState(false);
  const [apiSettings, setApiSettings] = useState<ApiProviderSettings>({
    base_url: "https://api.openai.com/v1",
    api_key: "",
    model: "",
    system_prompt: loadStoredCustomNarratorPrompt(),
    narrator_timeout_ms: null,
    evaluator_timeout_ms: 25_000,
    structured_evaluator_timeout_ms: 90_000,
    diagnostic_evaluator_timeout_ms: 60_000,
    evaluator_timeout_mode: "finite",
    evaluator_mode: "evaluator_form_v1",
    structured_evaluator_policy: "prefer",
    structured_evaluator_max_retries: 1,
    wait_for_evaluator_before_next_turn: true,
    allow_send_with_stale_state: false,
    evaluator_background_enabled: false,
    anti_replay_forced_retry_enabled: false,
  });
  const [stateUpdaterSettings, setStateUpdaterSettings] = useState<ApiProviderSettings>({
    base_url: "https://api.openai.com/v1",
    api_key: "",
    model: "",
    system_prompt: "",
    narrator_timeout_ms: null,
    evaluator_timeout_ms: 25_000,
    structured_evaluator_timeout_ms: 90_000,
    diagnostic_evaluator_timeout_ms: 60_000,
    evaluator_timeout_mode: "finite",
    evaluator_mode: "evaluator_form_v1",
    structured_evaluator_policy: "prefer",
    structured_evaluator_max_retries: 1,
    wait_for_evaluator_before_next_turn: true,
    allow_send_with_stale_state: false,
    evaluator_background_enabled: false,
    anti_replay_forced_retry_enabled: false,
  });
  const [evaluatorExecutionMode, setEvaluatorExecutionMode] = useState<string>(
    () => localStorage.getItem(EVALUATOR_EXECUTION_MODE_STORAGE_KEY) ?? "balanced",
  );
  const updateEvaluatorExecutionMode = (mode: string) => {
    setEvaluatorExecutionMode(mode);
    localStorage.setItem(EVALUATOR_EXECUTION_MODE_STORAGE_KEY, mode);
  };
  const [structuredEvaluatorTransport, setStructuredEvaluatorTransport] = useState<string>(
    () => localStorage.getItem(STRUCTURED_EVALUATOR_TRANSPORT_STORAGE_KEY) ?? "auto",
  );
  const updateStructuredEvaluatorTransport = (transport: string) => {
    setStructuredEvaluatorTransport(transport);
    localStorage.setItem(STRUCTURED_EVALUATOR_TRANSPORT_STORAGE_KEY, transport);
  };
  const [lastTurnDebug, setLastTurnDebug] = useState<TurnDebug | null>(null);
  const [view, setView] = useState<AppView>("library");
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
  const [payloadCopied, setPayloadCopied] = useState(false);
  const [exportFeedback, setExportFeedback] = useState("");
  const [settingsDrawerOpen, setSettingsDrawerOpen] = useState(() =>
    loadStoredBoolean(SETTINGS_DRAWER_OPEN_STORAGE_KEY, false),
  );
  const [settingsTab, setSettingsTab] = useState<SettingsTab>(loadStoredSettingsTab);
  const [devConsoleOpen, setDevConsoleOpen] = useState(false);
  const [latestPipelineTrace, setLatestPipelineTrace] = useState<TurnPipelineTrace | null>(null);
  const [devLogs, setDevLogs] = useState<DevLogEntry[]>([]);
  const [devConsolePaused, setDevConsolePaused] = useState(() =>
    loadStoredBoolean(DEV_CONSOLE_PAUSED_STORAGE_KEY, false),
  );
  const [devLogLevelFilter, setDevLogLevelFilter] = useState<DevLogLevel | "all">(
    loadStoredDevLogLevelFilter,
  );
  const [devLogCategoryFilter, setDevLogCategoryFilter] = useState<DevLogCategory | "all">(
    loadStoredDevLogCategoryFilter,
  );
  const [devCommandName, setDevCommandName] = useState<DevCommandName>(
    "dedupe_active_adjacent_user_messages",
  );
  const [devCommandArgs, setDevCommandArgs] = useState("{}");
  const [devCommandRunning, setDevCommandRunning] = useState(false);
  const [devCommandResult, setDevCommandResult] = useState<string | null>(null);
  const [devCommandError, setDevCommandError] = useState<string | null>(null);
  const [structuredDiagnosticRunning, setStructuredDiagnosticRunning] = useState(false);
  const [structuredDiagnosticResult, setStructuredDiagnosticResult] =
    useState<StructuredEvaluatorDiagnosticSummary | null>(null);
  const [structuredDiagnosticError, setStructuredDiagnosticError] = useState<string | null>(null);
  const [benchmarkType, setBenchmarkType] = useState<BenchmarkType>("visible_ai_chat");
  const [benchmarkTarget, setBenchmarkTarget] = useState<BenchmarkTarget>("current_session");
  const [benchmarkTurnCount, setBenchmarkTurnCount] = useState(5);
  const [benchmarkPlayerProfileId, setBenchmarkPlayerProfileId] = useState("");
  const [benchmarkPlayerGoal, setBenchmarkPlayerGoal] = useState(
    "Build cautious trust with the active Soul while respecting boundaries.",
  );
  // Default OFF: a live Visible AI Chat run should mirror a turn you typed so
  // exported payloads/session show the real pipeline. Strict mode is an opt-in
  // probe that pins the evaluator to structured tool-calling.
  const [benchmarkStrictToolEvaluator, setBenchmarkStrictToolEvaluator] = useState(false);
  const [benchmarkTransport, setBenchmarkTransport] = useState<ApiProviderSettings["structured_evaluator_transport"]>("tool_call");
  const [benchmarkWaitForEvaluator, setBenchmarkWaitForEvaluator] = useState(true);
  const [benchmarkRunning, setBenchmarkRunning] = useState(false);
  const [benchmarkResult, setBenchmarkResult] = useState<BenchmarkSummary | null>(null);
  const [benchmarkError, setBenchmarkError] = useState<string | null>(null);
  // Live self-play: drives one visible `executeTurn` per turn via an effect so
  // the AI-vs-AI exchange streams into the real chat instead of running headless.
  const [benchmarkLiveActive, setBenchmarkLiveActive] = useState(false);
  const [benchmarkTurnsRemaining, setBenchmarkTurnsRemaining] = useState(0);
  const benchmarkCtxRef = useRef<BenchmarkLiveContext | null>(null);
  const benchmarkTurnInFlightRef = useRef(false);
  const benchmarkStopRef = useRef(false);
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
  const generationAbortRef = useRef<AbortController | null>(null);
  const generationIdRef = useRef(0);
  const activeGenerationRef = useRef<ActiveGeneration | null>(null);
  const devConsoleBodyRef = useRef<HTMLDivElement>(null);
  const chatOnlyBodyRef = useRef<HTMLElement>(null);
  const chatBottomRef = useRef<HTMLDivElement>(null);
  const defaultConversationId = useMemo(
    () =>
      soul && setting
        ? conversationIdForSettingAndSoul(setting.setting_id, soul.character_id)
        : DEFAULT_CONVERSATION_ID,
    [setting?.setting_id, soul?.character_id],
  );
  const currentConversationId = activeConversationId ?? defaultConversationId;
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
    if (!devConsoleOpen || devConsolePaused) return;
    const body = devConsoleBodyRef.current;
    if (!body) return;
    body.scrollTop = body.scrollHeight;
  }, [devLogs, devConsoleOpen, devConsolePaused]);

  useLayoutEffect(() => {
    if (view !== "chat") return;
    const body = chatOnlyBodyRef.current;
    if (!body) return;
    let secondFrame = 0;
    const scrollToBottom = () => {
      body.scrollTop = body.scrollHeight;
      chatBottomRef.current?.scrollIntoView({ block: "end" });
    };
    scrollToBottom();
    const frame = window.requestAnimationFrame(() => {
      scrollToBottom();
      secondFrame = window.requestAnimationFrame(scrollToBottom);
    });
    return () => {
      window.cancelAnimationFrame(frame);
      window.cancelAnimationFrame(secondFrame);
    };
  }, [view, currentConversationId, messages.length]);

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
    localStorage.setItem(CUSTOM_NARRATOR_PROMPT_STORAGE_KEY, apiSettings.system_prompt);
  }, [apiSettings.system_prompt]);

  useEffect(() => {
    localStorage.setItem(SETTINGS_DRAWER_OPEN_STORAGE_KEY, settingsDrawerOpen ? "true" : "false");
  }, [settingsDrawerOpen]);

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

  useEffect(() => {
    localStorage.setItem(DEV_CONSOLE_PAUSED_STORAGE_KEY, devConsolePaused ? "true" : "false");
  }, [devConsolePaused]);

  useEffect(() => {
    localStorage.setItem(DEV_LOG_LEVEL_FILTER_STORAGE_KEY, devLogLevelFilter);
  }, [devLogLevelFilter]);

  useEffect(() => {
    localStorage.setItem(DEV_LOG_CATEGORY_FILTER_STORAGE_KEY, devLogCategoryFilter);
  }, [devLogCategoryFilter]);

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
      if (job.status === "running" || job.status === "pending") {
        setStatus("Updating memory/state...");
      } else if (evaluatorJobRefreshesState(job)) {
        setStatus(evaluatorJobStatusText(job));
        if (job.status !== "stale_skipped" && soulRef.current) {
          void refreshContext(soulRef.current.character_id, job.conversation_id);
          void getSoul(soulRef.current.character_id).then(setSoul).catch(() => undefined);
        }
      } else if (job.status === "failed" || job.status === "timed_out") {
        setStatus(evaluatorJobStatusText(job));
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
      evaluator_timeout_ms: profile.evaluator_timeout_ms ?? 25_000,
      structured_evaluator_timeout_ms: profile.structured_evaluator_timeout_ms ?? 90_000,
      diagnostic_evaluator_timeout_ms: profile.diagnostic_evaluator_timeout_ms ?? 60_000,
      evaluator_timeout_mode: profile.evaluator_timeout_mode ?? "finite",
      evaluator_mode: profile.evaluator_mode ?? "evaluator_form_v1",
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
      evaluator_timeout_ms: profile.evaluator_timeout_ms ?? 25_000,
      structured_evaluator_timeout_ms: profile.structured_evaluator_timeout_ms ?? 90_000,
      diagnostic_evaluator_timeout_ms: profile.diagnostic_evaluator_timeout_ms ?? 60_000,
      evaluator_timeout_mode: profile.evaluator_timeout_mode ?? "finite",
      evaluator_mode: profile.evaluator_mode ?? "evaluator_form_v1",
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
    const [existingSouls, existingSettings, existingConversations, existingArchived] = await Promise.all([
      listSouls(),
      listSettings(),
      listConversations(),
      listArchivedSessions(),
    ]);
    setSouls(existingSouls);
    setSettings(existingSettings);
    setConversations([...existingConversations, ...existingArchived]);
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
          setSettingsDrawerOpen(true);
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
    const preview = await compileContext(soulId, conversationId);
    setContext(preview);
  }

  async function handleSoulRepair(kind: "world" | "scenario" | "events" | "memories") {
    if (!soul) return;
    const labels = {
      world: "clear world state",
      scenario: "clear profile scenario",
      events: "clear recent events",
      memories: "clear memories",
    } as const;
    const confirmed = window.confirm(`Debug repair: ${labels[kind]} for ${soul.character_name}?`);
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
              apiSettings,
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
        return;
      }
      if (result.conversation_id !== currentConversationIdRef.current) {
        return;
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
    const personas = await listPlayerPersonas();
    setPlayerPersonas(personas);
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

  async function handleValidateMne() {
    const path = window.prompt("Enter absolute path to the .mne file to validate:");
    if (!path) return;
    try {
      setStatus("Validating bundle...");
      const report = await validateMneBundle(path);
      logDev("info", "app", "MNE Validation Report", report);
      if (report.valid) {
        window.alert(`Bundle is VALID!\n\nSoul: ${report.summary.soul_name || "N/A"}\nWorld: ${report.summary.world_name || "N/A"}\nMessages: ${report.summary.message_count}\nMemories: ${report.summary.memory_count}\nRelationships: ${report.summary.relationship_count}`);
      } else {
        window.alert(`Bundle is INVALID!\n\nErrors:\n${report.errors.join("\n")}`);
      }
    } catch (err: any) {
      window.alert(`Validation Error: ${err}`);
      logDev("error", "app", "MNE Validation Failed", { error: err.toString() });
    } finally {
      setStatus("");
    }
  }

  async function handlePreviewMne() {
    const path = window.prompt("Enter absolute path to the .mne file to preview:");
    if (!path) return;
    try {
      setStatus("Previewing bundle...");
      const report = await previewMneImport(path);
      logDev("info", "app", "MNE Preview Report", report);
      window.alert(`MNE Preview:\n\nSoul: ${report.summary.soul_name || "N/A"} (${report.summary.soul_id || "N/A"})\nWorld: ${report.summary.world_name || "N/A"} (${report.summary.world_id || "N/A"})\nMessages: ${report.summary.message_count}\nMemories: ${report.summary.memory_count}\nObject States: ${report.summary.object_state_count}\nRelationships: ${report.summary.relationship_count}\nPayload Logs: ${report.summary.payload_log_count}`);
    } catch (err: any) {
      window.alert(`Preview Error: ${err}`);
      logDev("error", "app", "MNE Preview Failed", { error: err.toString() });
    } finally {
      setStatus("");
    }
  }

  async function handleImportMneAsNew() {
    const path = window.prompt("Enter absolute path to the .mne file to import as new copy:");
    if (!path) return;
    try {
      setStatus("Importing bundle...");
      const result = await importMneAsNew(path);
      logDev("info", "app", "MNE Import Result", result);
      window.alert(`Import Successful!\n\nSummary: ${result.summary}\nRemapped IDs count: ${Object.keys(result.remapped_ids).length}`);
      setSouls(await listSouls());
      await refreshConversations();
    } catch (err: any) {
      window.alert(`Import Error: ${err}`);
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

  function parseDevCommandArgs(): Record<string, unknown> {
    const trimmed = devCommandArgs.trim();
    if (!trimmed) return {};
    const parsed = JSON.parse(trimmed) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("JSON args must be an object.");
    }
    return parsed as Record<string, unknown>;
  }

  function devStringArg(args: Record<string, unknown>, ...keys: string[]) {
    for (const key of keys) {
      const value = args[key];
      if (typeof value === "string" && value.trim()) return value.trim();
    }
    return null;
  }

  function devNumberArg(args: Record<string, unknown>, key: string, fallback: number) {
    const value = args[key];
    const parsed = typeof value === "number" ? value : typeof value === "string" ? Number(value) : NaN;
    return Number.isFinite(parsed) ? parsed : fallback;
  }

  function devBooleanArg(args: Record<string, unknown>, key: string, fallback: boolean) {
    const value = args[key];
    if (typeof value === "boolean") return value;
    if (typeof value === "string") {
      if (value.toLowerCase() === "true") return true;
      if (value.toLowerCase() === "false") return false;
    }
    return fallback;
  }

  async function runWhitelistedDevCommand(commandName: DevCommandName, argsOverride?: Record<string, unknown>) {
    if (!import.meta.env.DEV) return;
    const args = argsOverride ?? parseDevCommandArgs();
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
          apiSettings,
          updaterSettings,
          benchmarkSettings,
        );
        setBenchmarkResult(summary);
        setBenchmarkError(null);
        return summary;
      }
      default: {
        const exhaustive: never = commandName;
        throw new Error(`Command is not whitelisted: ${exhaustive}`);
      }
    }
  }

  async function handleRunDevCommand(commandName = devCommandName, argsOverride?: Record<string, unknown>) {
    if (!import.meta.env.DEV || devCommandRunning) return;
    const conversationId =
      typeof argsOverride?.conversationId === "string"
        ? argsOverride.conversationId
        : currentConversationId;
    setDevCommandRunning(true);
    setDevCommandResult(null);
    setDevCommandError(null);
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
          ? `${benchmarkPassed ? "PASS" : "FAIL"} benchmark from Dev Console`
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
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setDevCommandError(message);
      setStatus(`Dev command failed: ${message}`);
      logDev("error", "error", "Dev command failed", {
        command: commandName,
        conversation_id: conversationId,
        error: message,
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
    const nextTitle = window.prompt("Session title", currentSessionTitle);
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
      const conversationSoul = await getSoul(conversation.soul_id);
      setSoul(conversationSoul);
      setSelectedCharacterIds((current) =>
        current.includes(conversationSoul.character_id)
          ? [conversationSoul.character_id, ...current.filter((id) => id !== conversationSoul.character_id)]
          : [conversationSoul.character_id, ...current],
      );
      setCreatorFieldsFromSoul(conversationSoul);
      setActiveConversationId(conversation.conversation_id);
      setCurrentSessionTitle(conversation.title);
      setSessionContinuityLabel(
        conversation.source_savepoint_id
          ? "Loaded named Session clone"
          : "Loaded persistent Soul continuity chat",
      );
      setMessages(await listConversationMessages(conversation.conversation_id));
      if (conversation.active_evaluator_profile_id) {
        const updaterProfile = providerProfiles.find((p) => p.id === conversation.active_evaluator_profile_id);
        if (updaterProfile) {
          setSelectedStateUpdaterProfileId(updaterProfile.id);
          applyStateUpdaterProviderProfile(updaterProfile);
        }
      } else {
        if (selectedStateUpdaterProfileId) {
          try {
            await setActiveEvaluatorProfile(conversation.conversation_id, selectedStateUpdaterProfileId);
          } catch (e) {
            console.error("Failed to set active evaluator profile on conversation select", e);
          }
        }
      }
      await refreshPlayerPersonas(conversation.conversation_id);
      setLastTurnDebug(null);
      setView("chat");
      setStatus(`Loaded chat: ${conversation.title}`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
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
    const assistant = assistantForUserMessage(message);
    if (assistant) {
      await executeTurn(message.content, statusLabel, assistant.id, correctionInstruction);
      return;
    }

    try {
      await executeTurn(message.content, statusLabel, undefined, correctionInstruction);
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
    const instruction = window
      .prompt(
        "Correction instruction for next response:",
        "Continue from the current user message. Do not replay the wrong branch.",
      )
      ?.trim();
    if (!instruction) return;
    await executeTurnFromUserMessage(message, "Applying fix instruction", instruction);
  }

  async function handleDeleteChatMessage(message: ChatMessage) {
    if (busy || stateUpdating) return;
    const confirmed = window.confirm(
      message.role === "assistant"
        ? "Hide this generated response and all later turns in this session? This is recoverable with Restore hidden turns."
        : "Hide this user message and all later turns in this session? This is recoverable with Restore hidden turns.",
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await deleteMessage(message.conversation_id, message.id);
      const nextMessages = await listConversationMessages(message.conversation_id);
      setMessages(nextMessages);
      await refreshConversations();
      if (soul) {
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

  async function handleDeleteChat(conversationId = currentConversationId) {
    if (!soul) return;
    const deletingActiveConversation = conversationId === currentConversationId;
    const confirmed = window.confirm(
      "Archive this local session? It stays recoverable; messages, payload logs, and session clones are kept.",
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
    const confirmed = window.confirm(
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
    const confirmed = window.confirm(
      `Archive ${soul.character_name}? Local chats and savepoint history remain safe and recoverable.`,
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await deleteSoul(soul.character_id);
      const remaining = await listSouls();
      setSouls(remaining);
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
      window.alert("Cannot archive the active/default setting. Switch settings first.");
      return;
    }

    const confirmed = window.confirm(
      `Archive ${setting.setting_name}? Local chats and world settings remain safe and recoverable.`,
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      await archiveSetting(setting.setting_id, activeSettingIds);
      const remaining = await listSettings();
      setSettings(remaining);

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
        window.alert(`Evaluator contract test failed:\n\n${errorMsg}\n\nRaw response:\n${report.raw_response}`);
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
    setStatus("Running benchmark...");
    try {
      const summary = await runBenchmark(
        soul.character_id,
        setting?.setting_id ?? null,
        provider,
        apiSettings,
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
    }
  }

  // A live benchmark turn must hit the EXACT same pipeline as a turn you type,
  // so the exported payloads/session show the real behavior you're diagnosing.
  // The only optional deviations are:
  //   - "Wait for evaluator each turn": runs the evaluator as a tracked
  //     BACKGROUND job (so the evaluator banner stays live), and the loop waits
  //     on that job before the next turn — keeping per-turn commit ordering.
  //   - "Strict Tool Evaluator": opt-in probe that pins the evaluator to
  //     structured tool-calling. OFF (default for faithful repro) = your exact
  //     chat evaluator settings flow through untouched.
  function benchmarkLiveUpdaterOverride(settings: BenchmarkSettings): Partial<ApiProviderSettings> {
    const waitOverride: Partial<ApiProviderSettings> = settings.wait_for_evaluator_each_turn
      ? {
          // Background so a job row is created and `evaluator_job_status_changed`
          // events fire → the tracker UI updates. The loop (waitForBenchmark-
          // EvaluatorJob) blocks on the job before capturing the turn summary
          // and the next message, so state is committed in order.
          evaluator_background_enabled: true,
          allow_send_with_stale_state: true,
        }
      : {};
    if (!settings.strict_tool_evaluator) {
      // Faithful repro: no evaluator overrides, just the turn sequencing.
      return waitOverride;
    }
    return {
      ...waitOverride,
      evaluator_mode: settings.evaluator_mode ?? undefined,
      structured_evaluator_transport: settings.structured_evaluator_transport ?? undefined,
      structured_evaluator_policy: settings.structured_evaluator_policy ?? undefined,
      structured_evaluator_max_retries: settings.structured_evaluator_max_retries ?? undefined,
    };
  }

  // Poll the latest evaluator job for the conversation until it reaches a
  // terminal state (or Stop / a safety cap). Used between live benchmark turns
  // so the background evaluator has committed before the next turn — while the
  // evaluator banner tracks the same job live. Returns when there is no live
  // job (terminal, or none — e.g. a dialogue-only turn the gate skipped).
  async function waitForBenchmarkEvaluatorJob(conversationId: string) {
    const deadlineMs = Date.now() + 300_000; // safety cap above any single job's own timeout
    while (Date.now() < deadlineMs) {
      if (benchmarkStopRef.current) return;
      let job: EvaluatorJob | null = null;
      try {
        job = await getLatestEvaluatorJob(conversationId);
      } catch {
        return; // best-effort: don't wedge the run on a polling error
      }
      if (!job || (job.status !== "pending" && job.status !== "running")) {
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, 700));
    }
  }

  async function startLiveBenchmark(
    settingsPayload: BenchmarkSettings,
    updaterSettings: ApiProviderSettings,
  ) {
    if (!soul) return;
    setBenchmarkRunning(true);
    setBenchmarkResult(null);
    setBenchmarkError(null);
    setStatus("Preparing live benchmark session...");
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
        settings: settingsPayload,
        narratorSettings: apiSettings,
        updaterSettings: liveUpdaterSettings,
        initialMemoryCount: init.initial_memory_count,
        initialObjectCount: init.initial_object_count,
        initialRelationshipCount: init.initial_relationship_count,
        perTurn: [],
        narratorFailures: 0,
        completedTurns: 0,
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
      setStatus(`Live benchmark running: 0/${settingsPayload.turn_count} turns`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      benchmarkCtxRef.current = null;
      setBenchmarkError(message);
      setStatus(`Benchmark failed: ${message}`);
      reportError(error, "Benchmark failed", "app");
      setBenchmarkRunning(false);
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
    try {
      setStatus(`${turnLabel}: AI player thinking...`);
      playerText = await generateBenchmarkPlayerMessage(
        ctx.conversationId,
        ctx.soulId,
        ctx.playerProfileId,
        ctx.playerGoal,
      );
      ctx.lastPlayerText = playerText;
      if (benchmarkStopRef.current) {
        benchmarkTurnInFlightRef.current = false;
        return;
      }
      // Send as a visible user turn; the narrator streams its reply live.
      await executeTurn(
        playerText,
        turnLabel,
        undefined,
        undefined,
        benchmarkLiveUpdaterOverride(ctx.settings),
      );
      // Stop may have aborted the narrator mid-stream — don't record a partial
      // turn; bail and let the effect finalize what actually completed.
      if (benchmarkStopRef.current) {
        benchmarkTurnInFlightRef.current = false;
        return;
      }
      // Evaluator runs as a background job (tracked by the banner). Wait for it
      // to commit before reading the turn summary and starting the next turn,
      // so per-turn deltas reflect committed state.
      if (ctx.settings.wait_for_evaluator_each_turn) {
        setStatus(`${turnLabel}: waiting for evaluator...`);
        await waitForBenchmarkEvaluatorJob(ctx.conversationId);
        if (benchmarkStopRef.current) {
          benchmarkTurnInFlightRef.current = false;
          return;
        }
      }
      const summary = await benchmarkTurnSummary(
        ctx.conversationId,
        turnIndex,
        playerText,
        null,
        ctx.updaterSettings,
      );
      ctx.perTurn.push(summary);
      ctx.completedTurns += 1;
      ctx.nextTurnIndex = turnIndex + 1;
      setBenchmarkTurnsRemaining((remaining) => remaining - 1);
      setStatus(`Live benchmark running: ${ctx.completedTurns}/${ctx.settings.turn_count} turns`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      // Distinguish where it broke so the summary isn't misleading: a null
      // playerText means the AI player-line call failed (the narrator never ran).
      const stage = playerText === null ? "player line generation" : "narrator turn";
      ctx.narratorFailures += 1;
      try {
        const failedSummary = await benchmarkTurnSummary(
          ctx.conversationId,
          turnIndex,
          playerText ?? "",
          `${stage} failed: ${message}`,
          ctx.updaterSettings,
        );
        ctx.perTurn.push(failedSummary);
      } catch {
        // Summary capture is best-effort once a turn has already failed.
      }
      benchmarkStopRef.current = true;
      setBenchmarkTurnsRemaining(0);
      logDev("warn", "app", "Live benchmark turn failed", {
        conversation_id: ctx.conversationId,
        turn_index: turnIndex,
        stage,
        error: message,
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
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setBenchmarkError(message);
      setStatus(`Benchmark failed: ${message}`);
      reportError(error, "Benchmark failed", "app");
    } finally {
      benchmarkCtxRef.current = null;
      benchmarkStopRef.current = false;
      benchmarkTurnInFlightRef.current = false;
      setBenchmarkRunning(false);
    }
  }

  function handleStopBenchmark() {
    if (!benchmarkLiveActive) return;
    benchmarkStopRef.current = true;
    setBenchmarkTurnsRemaining(0);
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
      window.alert(`Cannot activate profile "${profile.name}": Compatibility status is FAILED. Run contract test or enable Developer Override to proceed.`);
      return;
    }

    const CURRENT_EVALUATOR_PROMPT_VERSION = 1;
    const isStalePrompt = profile.evaluator_prompt_version !== CURRENT_EVALUATOR_PROMPT_VERSION;
    if (profile.evaluator_compatibility_status === 0 || isStalePrompt) {
      const reason = profile.evaluator_compatibility_status === 0 ? "is untested" : "has a stale prompt version";
      const confirmRun = window.confirm(
        `Profile "${profile.name}" ${reason}.\n\nWould you like to run the compatibility contract test now?\n\nClick Cancel to bypass and load with a developer warning.`
      );
      if (confirmRun) {
        await handleRunContractTest(profileId);
        const updatedProfiles = await listProviderProfiles();
        setProviderProfiles(updatedProfiles);
        const refreshedProfile = updatedProfiles.find(item => item.id === profileId);
        if (refreshedProfile && refreshedProfile.evaluator_compatibility_status !== 1 && !devOverrideActive) {
          window.alert("Contract test did not pass. Profile activation cancelled.");
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
      evaluator_timeout_ms: apiSettings.evaluator_timeout_ms ?? 25_000,
      structured_evaluator_timeout_ms: apiSettings.structured_evaluator_timeout_ms ?? 90_000,
      diagnostic_evaluator_timeout_ms: apiSettings.diagnostic_evaluator_timeout_ms ?? 60_000,
      evaluator_timeout_mode: apiSettings.evaluator_timeout_mode ?? "finite",
      evaluator_mode: apiSettings.evaluator_mode ?? "evaluator_form_v1",
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
      evaluator_timeout_ms: stateUpdaterSettings.evaluator_timeout_ms ?? 25_000,
      structured_evaluator_timeout_ms: stateUpdaterSettings.structured_evaluator_timeout_ms ?? 90_000,
      diagnostic_evaluator_timeout_ms: stateUpdaterSettings.diagnostic_evaluator_timeout_ms ?? 60_000,
      evaluator_timeout_mode: stateUpdaterSettings.evaluator_timeout_mode ?? "finite",
      evaluator_mode: stateUpdaterSettings.evaluator_mode ?? "evaluator_form_v1",
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
    if (activeIds.includes(profileId)) {
      window.alert("Cannot archive the active provider profile. Switch profiles first.");
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
    const name = window.prompt("Name this Soul snapshot", defaultName);
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
      const result = await exportCharacterSoulMne(soul.character_id, "");
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
      const result = await exportWorldSettingMne(nextSetting.setting_id, "");
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
      const result = await exportScenarioBundleMne(soul.character_id, setting.setting_id, "");
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
      const result = await exportCurrentSessionCheckpointMne(currentConversationId, "");
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
    const manualPath = window.prompt("Path to .mne bundle");
    const filePath = manualPath?.trim() || null;
    if (!filePath) return;
    setBusy(true);
    try {
      const result = await importMneBundle(filePath);
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
      const raw = JSON.parse(await file.text());
      const importedSoul = await soulFromImport(raw, file.name);
      await upsertSoul(importedSoul);
      setSoul(importedSoul);
      setSelectedCharacterIds((current) =>
        current.includes(importedSoul.character_id)
          ? [importedSoul.character_id, ...current.filter((id) => id !== importedSoul.character_id)]
          : [importedSoul.character_id, ...current],
      );
      setActiveConversationId(null);
      setCurrentSessionTitle("New Session");
      setCreatorFieldsFromSoul(importedSoul);
      setMessages([]);
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
      setActiveConversationId(null);
      setMessages([]);
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
  const filteredDevLogs = useMemo(
    () =>
      devLogs.filter(
        (entry) =>
          (devLogLevelFilter === "all" || entry.level === devLogLevelFilter) &&
          (devLogCategoryFilter === "all" || entry.category === devLogCategoryFilter),
      ),
    [devLogs, devLogLevelFilter, devLogCategoryFilter],
  );
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
  const benchmarkScoreRows = benchmarkResult
    ? [
        ["Visible chat messages created", benchmarkResult.scorecard.visible_chat_messages_created],
        ["Normal pipeline used", benchmarkResult.scorecard.normal_pipeline_used],
        ["Player simulator calls", benchmarkResult.scorecard.player_simulator_calls > 0 || benchmarkResult.benchmark_type === "scripted_visible_replay"],
        ["Narrator calls", benchmarkResult.scorecard.narrator_calls >= benchmarkResult.turn_count_completed],
        ["Evaluator calls", benchmarkResult.scorecard.evaluator_calls >= benchmarkResult.turn_count_completed],
        ["Evaluator waited each turn", benchmarkResult.scorecard.evaluator_waited_each_turn],
        ["Memory updated", benchmarkResult.scorecard.memory_updated],
        ["Object state updated", benchmarkResult.scorecard.object_state_updated],
        ["Relationship updated", benchmarkResult.scorecard.relationship_updated],
        ["Payload history exported", benchmarkResult.scorecard.payload_history_export_succeeded],
        ["Narrator response each turn", benchmarkResult.scorecard.narrator_visible_response_each_turn],
        ["Tool call required path", benchmarkResult.scorecard.evaluator_used_tool_call_where_required],
        ["No form fallback in strict mode", benchmarkResult.scorecard.no_evaluator_form_v1_fallback_in_strict_mode],
        ["No syntactic repair in strict mode", benchmarkResult.scorecard.syntactic_repair_unused_in_strict_mode],
        ["Memories increased", benchmarkResult.scorecard.memories_increased_over_time],
        ["Active player relationship present", benchmarkResult.scorecard.active_player_relationship_changed_when_warranted],
        ["Object IDs stable", benchmarkResult.scorecard.object_ids_stable],
        ["No default_player RP relationship leak", benchmarkResult.scorecard.default_player_not_normal_rp_relationship_target],
        [".mne export succeeded", benchmarkResult.scorecard.mne_export_succeeded],
      ] as Array<[string, boolean]>
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
        <span>Strict Tool Evaluator (diagnostic probe — overrides your chat evaluator settings)</span>
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
              {benchmarkResult.benchmark_type} completed {benchmarkResult.turn_count_completed} /{" "}
              {benchmarkResult.turn_count_requested} turns.
              <br />
              Scorecard:{" "}
              {benchmarkScoreRows.map(([label, passed]) => `${passed ? "PASS" : "FAIL"} ${label}`).join("; ")}
              {benchmarkResult.scorecard.failure_reasons.length ? (
                <>
                  <br />
                  Failure reasons: {benchmarkResult.scorecard.failure_reasons.join(", ")}
                </>
              ) : null}
              <br />
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
        {providerModeControls}
      </section>

      {provider === "API" ? (
        <>
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
                  value={selectedProviderProfileId}
                  onChange={(event) => void handleSelectProviderProfile(event.target.value)}
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
                <span>Narrator Timeout (seconds)</span>
                <input
                  type="number"
                  min="0"
                  value={Math.round((apiSettings.narrator_timeout_ms ?? 0) / 1000)}
                  onChange={(event) =>
                    setApiSettings((current) => ({
                      ...current,
                      narrator_timeout_ms:
                        Number(event.target.value) > 0 ? Number(event.target.value) * 1000 : null,
                    }))
                  }
                  placeholder="0 = provider default"
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
                  <span>Custom Narrator Prompt</span>
                  <textarea
                    value={apiSettings.system_prompt}
                    onChange={(event) =>
                      setApiSettings((current) => ({
                        ...current,
                        system_prompt: event.target.value,
                      }))
                    }
                    placeholder="Replaces default narrator + mode prompts when filled. Use Custom mode. Leave empty for default Reader narration."
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
                <Archive size={16} />
                <span>Archive Profile</span>
              </button>
            </div>
          </section>

          <section className="settings-section provider-pass-card">
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
            <div className="provider-pass-grid">
              <label className="field">
                <span>State Updater Timeout (seconds)</span>
                <input
                  type="number"
                  min="0"
                  value={Math.round((effectiveStateUpdaterSettings.evaluator_timeout_ms ?? 25_000) / 1000)}
                  onChange={(event) =>
                    updateEffectiveStateUpdaterSettings({
                      evaluator_timeout_ms: Math.max(0, Number(event.target.value) || 0) * 1000,
                    })
                  }
                  disabled={busy}
                />
              </label>
              <label className="field">
                <span>Timeout Mode</span>
                <select
                  value={effectiveStateUpdaterSettings.evaluator_timeout_mode ?? "finite"}
                  onChange={(event) =>
                    updateEffectiveStateUpdaterSettings({
                      evaluator_timeout_mode: event.target.value,
                    })
                  }
                  disabled={busy}
                >
                  <option value="finite">Finite app timeout</option>
                  <option value="no_app_timeout">No app timeout</option>
                </select>
              </label>
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
                  <option value="evaluator_structured_v1">Structured Ops Evaluator</option>
                </select>
              </label>
              <label className="field">
                <span>Structured Evaluator Policy</span>
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
                <span>Structured Evaluator Transport</span>
                <select
                  value={structuredEvaluatorTransport}
                  onChange={(event) => updateStructuredEvaluatorTransport(event.target.value)}
                  disabled={busy}
                >
                  <option value="auto">Auto ??tool calls first, then JSON schema</option>
                  <option value="tool_call">Tool calls (require real function calls)</option>
                  <option value="json_schema">JSON schema (response_format)</option>
                  <option value="json_object">JSON object</option>
                  <option value="prompt_json">Prompt-only JSON</option>
                </select>
              </label>
              <label className="field">
                <span>Execution Mode</span>
                <select
                  value={evaluatorExecutionMode}
                  onChange={(event) => updateEvaluatorExecutionMode(event.target.value)}
                  disabled={busy}
                >
                  <option value="balanced">Balanced ??evaluate every turn</option>
                  <option value="fast">Fast ??skip dialogue-only turns, catch up later</option>
                  <option value="long_context">Long Context ??evaluate every turn</option>
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
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={devOverrideActive}
                onChange={(event) => setDevOverrideActive(event.target.checked)}
                disabled={busy}
              />
              <span>Developer override (skip evaluator gates)</span>
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
                      onChange={(event) => void handleSelectStateUpdaterProfile(event.target.value)}
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
                    <Archive size={16} />
                    <span>Archive Profile</span>
                  </button>
                  <button
                    type="button"
                    className="ghost-action"
                    onClick={() => void handleRunContractTest(selectedStateUpdaterProfileId)}
                    disabled={busy || !selectedStateUpdaterProfileId}
                  >
                    <Play size={16} />
                    <span>Run Contract Test</span>
                  </button>
                  <button
                    type="button"
                    className="ghost-action"
                    onClick={() => void handleRunStructuredDiagnostic()}
                    disabled={busy || structuredDiagnosticRunning || (!selectedStateUpdaterProfileId && !selectedProviderProfileId)}
                  >
                    <Play size={16} />
                    <span>Run Structured Evaluator Diagnostic</span>
                  </button>
                </div>
              </>
            )}
            {useNarratorProviderForUpdater && (
              <div className="button-row">
                <button
                  type="button"
                  className="ghost-action"
                  onClick={() => void handleRunStructuredDiagnostic()}
                  disabled={busy || structuredDiagnosticRunning || (!selectedStateUpdaterProfileId && !selectedProviderProfileId)}
                >
                  <Play size={16} />
                  <span>Run Structured Evaluator Diagnostic</span>
                </button>
              </div>
            )}
            {(structuredDiagnosticResult || structuredDiagnosticError) && (
              <div className="provider-note">
                {structuredDiagnosticResult ? (
                  <>
                    <strong>
                      {structuredDiagnosticResult.resolved_evaluator_source === "evaluator_structured_v1" &&
                      structuredDiagnosticResult.evaluator_mode_per_run.every((mode) => mode === "evaluator_structured_v1") &&
                      structuredDiagnosticResult.runs.every((run) => !run.error)
                        ? "PASS"
                        : "FAIL"}
                    </strong>{" "}
                    {structuredDiagnosticResult.provider_model} · enforcement{" "}
                    {structuredDiagnosticResult.structured_enforcement_per_run.join(", ") || "none"} · fallback{" "}
                    {structuredDiagnosticResult.fallback_paths.map((path) => path.join(" > ")).join(" | ") || "none"}
                    <br />
                    Payload: {structuredDiagnosticResult.payload_history_path}
                    <br />
                    MNE: {structuredDiagnosticResult.mne_checkpoint_path}
                    <br />
                    Summary: {structuredDiagnosticResult.summary_json_path}
                  </>
                ) : (
                  <><strong>FAIL</strong> {structuredDiagnosticError}</>
                )}
              </div>
            )}
          </section>

          <section className="settings-section provider-pass-card">
            <div className="settings-section-heading">
              <div>
                <span className="eyebrow">Profiles</span>
                <h3>Saved Provider Profiles</h3>
              </div>
            </div>
            
            {providerProfiles.length === 0 && archivedProviderProfiles.length === 0 ? (
              <p className="settings-note">No profiles saved yet.</p>
            ) : (
              <div className="provider-profiles-list" style={{ marginTop: "1rem" }}>
                {providerProfiles.map((p) => {
                  const isNarratorActive = selectedProviderProfileId === p.id;
                  const isUpdaterActive = selectedStateUpdaterProfileId === p.id;
                  const isActive = isNarratorActive || isUpdaterActive;
                  return (
                    <div key={p.id} className="profile-list-item" style={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                      padding: "0.5rem 0",
                      borderBottom: "1px solid var(--border-color, #333)",
                    }}>
                      <div>
                        <strong style={{ color: "var(--text-color, #eee)" }}>{p.name}</strong>
                        <span style={{ fontSize: "0.8rem", color: "var(--text-muted, #888)", marginLeft: "0.5rem" }}>
                          ({p.model})
                        </span>
                        {isNarratorActive && <span className="provider-status-pill" style={{ marginLeft: "0.5rem", fontSize: "0.7rem", padding: "2px 6px" }}>Active Narrator</span>}
                        {isUpdaterActive && <span className="provider-status-pill" style={{ marginLeft: "0.5rem", fontSize: "0.7rem", padding: "2px 6px" }}>Active Updater</span>}
                        <span style={{
                          fontSize: "0.75rem",
                          marginLeft: "0.5rem",
                          padding: "1px 6px",
                          borderRadius: "4px",
                          backgroundColor: "rgba(255,255,255,0.05)",
                          color: p.evaluator_compatibility_status === 1 ? "#4caf50" : p.evaluator_compatibility_status === 2 ? "#f44336" : "var(--text-muted, #888)",
                          border: `1px solid ${p.evaluator_compatibility_status === 1 ? "#4caf50" : p.evaluator_compatibility_status === 2 ? "#f44336" : "#888888"}33`
                        }}>
                          Evaluator: {p.evaluator_compatibility_status === 1 ? "Schema enforced" : p.evaluator_compatibility_status === 3 ? "JSON object only" : p.evaluator_compatibility_status === 2 ? "Failed" : p.evaluator_compatibility_status === 4 ? "Stale prompt" : "Untested"}
                        </span>
                      </div>
                      <div style={{ display: "flex", gap: "0.5rem" }}>
                        <button
                          type="button"
                          className="ghost-action compact-ghost"
                          onClick={() => void handleRunContractTest(p.id)}
                          disabled={busy}
                          title="Run compatibility contract test"
                          style={{ fontSize: "0.8rem", padding: "2px 8px" }}
                        >
                          <Play size={12} style={{ marginRight: "4px" }} />
                          <span>Test</span>
                        </button>
                        <button
                          type="button"
                          className="ghost-action compact-ghost"
                          onClick={() => handleArchiveProviderProfile(p.id)}
                          disabled={busy || isActive}
                          title={isActive ? "Cannot archive active profile" : "Archive profile"}
                          style={{ fontSize: "0.8rem", padding: "2px 8px" }}
                        >
                          <Archive size={12} style={{ marginRight: "4px" }} />
                          <span>Archive</span>
                        </button>
                      </div>
                    </div>
                  );
                })}

                {archivedProviderProfiles.length > 0 && (
                  <div style={{ marginTop: "1.5rem" }}>
                    <span className="eyebrow">Archived</span>
                    <h4 style={{ margin: "0.5rem 0", color: "var(--text-muted, #888)" }}>Archived Profiles</h4>
                    {archivedProviderProfiles.map((p) => (
                      <div key={p.id} className="profile-list-item" style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                        padding: "0.5rem 0",
                        borderBottom: "1px solid var(--border-color, #333)",
                      }}>
                        <div>
                          <span style={{ color: "var(--text-muted, #888)", textDecoration: "line-through" }}>{p.name}</span>
                          <span style={{ fontSize: "0.8rem", color: "var(--text-muted, #666)", marginLeft: "0.5rem" }}>
                            ({p.model})
                          </span>
                        </div>
                        <button
                          type="button"
                          className="ghost-action compact-ghost"
                          onClick={() => handleRestoreProviderProfile(p.id)}
                          disabled={busy}
                          style={{ fontSize: "0.8rem", padding: "2px 8px" }}
                        >
                          <RefreshCcw size={12} style={{ marginRight: "4px" }} />
                          <span>Restore</span>
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </section>
        </>
      ) : null}
      {benchmarkRunnerPanel}
    </div>
  );
  const settingsDrawerToggle = (
    <button
      type="button"
      className={`settings-drawer-toggle ${settingsDrawerOpen ? "open" : ""}`}
      onClick={() => {
        setSettingsDrawerOpen((open) => !open);
        if (!settingsDrawerOpen) setDevConsoleOpen(false);
      }}
    >
      <Database size={16} />
      <span>Settings</span>
    </button>
  );
  const settingsDrawerPanel = settingsDrawerOpen ? (
    <aside className="settings-drawer-panel" aria-label="Settings">
      <header className="settings-drawer-header">
        <div>
          <span className="eyebrow">Preferences</span>
          <h2>Settings</h2>
        </div>
        <button type="button" onClick={() => setSettingsDrawerOpen(false)}>
          Close
        </button>
      </header>
      <nav className="settings-drawer-tabs" aria-label="Settings categories">
        {(["ai", "chat", "library", "dev"] as SettingsTab[]).map((tab) => (
          <button
            key={tab}
            type="button"
            className={settingsTab === tab ? "selected" : ""}
            onClick={() => setSettingsTab(tab)}
          >
            {tab.toUpperCase()}
          </button>
        ))}
      </nav>
      <div className="settings-drawer-body">
        {settingsTab === "ai" ? providerSettingsPanel : null}
        {settingsTab === "chat" ? (
          <div className="settings-tab-panel">
            <section className="settings-section">
              <div className="settings-section-heading">
                <div>
                  <span className="eyebrow">Sessions</span>
                  <h3>Chat Defaults</h3>
                </div>
              </div>
              <div className="chat-start-options settings-radio-group" role="radiogroup" aria-label="Default chat start mode">
                <label>
                  <input
                    type="radio"
                    name="settings-chat-start-mode"
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
                    name="settings-chat-start-mode"
                    value="fresh"
                    checked={chatStartMode === "fresh"}
                    onChange={() => setChatStartMode("fresh")}
                    disabled={busy}
                  />
                  <span>New isolated Session</span>
                </label>
              </div>
              <label className="toggle-row">
                <input
                  type="checkbox"
                  checked={showArchivedSessions}
                  onChange={(event) => setShowArchivedSessions(event.target.checked)}
                />
                <span>Show archived sessions by default</span>
              </label>
              <p className="settings-note">Composer shortcut: Enter to send, Shift+Enter for a new line.</p>
            </section>
            <section className="settings-section">
              <div className="settings-section-heading">
                <div>
                  <span className="eyebrow">Current Chat</span>
                  <h3>Quick Actions</h3>
                </div>
              </div>
              <div className="settings-action-list">
                <button type="button" className="ghost-action" onClick={handleRestoreHiddenTurns} disabled={busy || !currentConversationId}>
                  <RefreshCcw size={16} />
                  <span>Restore Hidden Turns</span>
                </button>
                <button type="button" className="ghost-action" onClick={handleExportVisibleChatLog} disabled={busy || activeMessages.length === 0}>
                  <FileDown size={16} />
                  <span>Export Visible Chat</span>
                </button>
                <button type="button" className="ghost-action" onClick={handleExportCurrentSessionMne} disabled={busy || !currentConversationId}>
                  <FileDown size={16} />
                  <span>Export Session .mne</span>
                </button>
              </div>
            </section>
          </div>
        ) : null}
        {settingsTab === "library" ? (
          <div className="settings-tab-panel">
            <section className="settings-section">
              <div className="settings-section-heading">
                <div>
                  <span className="eyebrow">Library</span>
                  <h3>Import And Export</h3>
                </div>
              </div>
              <div className="settings-action-list">
                <button type="button" className="ghost-action" onClick={() => settingImportInputRef.current?.click()} disabled={busy}>
                  <FileUp size={16} />
                  <span>Import World JSON</span>
                </button>
                <button type="button" className="ghost-action" onClick={() => importInputRef.current?.click()} disabled={busy}>
                  <FileUp size={16} />
                  <span>Import Character JSON</span>
                </button>
                <button type="button" className="ghost-action" onClick={handleImportMne} disabled={busy}>
                  <FileUp size={16} />
                  <span>Import .mne Bundle</span>
                </button>
                <button type="button" className="ghost-action" onClick={handleExportSettingMne} disabled={!setting || busy}>
                  <FileDown size={16} />
                  <span>Export World .mne</span>
                </button>
                <button type="button" className="ghost-action" onClick={handleExportSoulMne} disabled={!soul || busy}>
                  <FileDown size={16} />
                  <span>Export Character .mne</span>
                </button>
                <button type="button" className="ghost-action" onClick={handleExportScenarioMne} disabled={!soul || !setting || busy}>
                  <FileDown size={16} />
                  <span>Export Scenario .mne</span>
                </button>
              </div>
            </section>
            <section className="settings-section">
              <div className="settings-section-heading">
                <div>
                  <span className="eyebrow">Editors</span>
                  <h3>World And Character</h3>
                </div>
              </div>
              <div className="settings-action-list">
                <button type="button" className="ghost-action" onClick={() => setSettingEditorOpen(true)}>
                  <Database size={16} />
                  <span>Open World Editor</span>
                </button>
                <button type="button" className="ghost-action" onClick={() => setSoulEditorOpen(true)}>
                  <Brain size={16} />
                  <span>Open Character Editor</span>
                </button>
              </div>
              <p className="settings-note">The launcher stays focused on choosing a world, primary character, and session; larger forms stay collapsed until opened.</p>
            </section>
          </div>
        ) : null}
        {settingsTab === "dev" ? (
          <div className="settings-tab-panel">
            <section className="settings-section">
              <div className="settings-section-heading">
                <div>
                  <span className="eyebrow">Diagnostics</span>
                  <h3>Dev Console Defaults</h3>
                </div>
              </div>
              <div className="settings-grid">
                <label className="field">
                  <span>Level Filter</span>
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
                <label className="field">
                  <span>Category Filter</span>
                  <select
                    value={devLogCategoryFilter}
                    onChange={(event) => setDevLogCategoryFilter(event.target.value as DevLogCategory | "all")}
                  >
                    <option value="all">All</option>
                    {DEV_LOG_CATEGORIES.map((category) => (
                      <option key={category} value={category}>
                        {category}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              <label className="toggle-row">
                <input
                  type="checkbox"
                  checked={devConsolePaused}
                  onChange={(event) => setDevConsolePaused(event.target.checked)}
                />
                <span>Pause Dev Console scroll by default</span>
              </label>
              <div className="settings-action-list">
                <button
                  type="button"
                  className="ghost-action"
                  onClick={() => {
                    setDevConsoleOpen(true);
                    setSettingsDrawerOpen(false);
                  }}
                >
                  <Terminal size={16} />
                  <span>Open Dev Console</span>
                </button>
                <button type="button" className="ghost-action" onClick={handleExportLlmPayloadHistory} disabled={busy || !currentConversationId}>
                  <FileDown size={16} />
                  <span>Export Payload History</span>
                </button>
              </div>
            </section>
          </div>
        ) : null}
      </div>
    </aside>
  ) : null;
  const devConsoleToggle = (
    <button
      type="button"
      className={`dev-console-toggle ${devConsoleOpen ? "open" : ""}`}
      onClick={() => {
        setDevConsoleOpen((open) => !open);
        if (!devConsoleOpen) setSettingsDrawerOpen(false);
        logDev("info", "app", devConsoleOpen ? "Dev Console closed" : "Dev Console opened");
      }}
    >
      <Terminal size={16} />
      <span>Dev Console</span>
      <strong>{devLogs.length}</strong>
    </button>
  );
  const devConsolePanel = devConsoleOpen ? (
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
      {latestPipelineTrace && (
        <div className="dev-pipeline-trace-panel" style={{ padding: "12px 16px", backgroundColor: "#111827", borderBottom: "1px solid #1f2937", fontSize: "12px", fontFamily: "monospace", color: "#d1d5db" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "8px" }}>
            <span style={{ color: "#10b981", fontWeight: "bold", fontSize: "13px" }}>
              Turn Pipeline Trace (Status: {latestPipelineTrace.final_status})
            </span>
            <button
              type="button"
              style={{ padding: "2px 8px", fontSize: "11px", backgroundColor: "#374151", color: "#f3f4f6", border: "none", borderRadius: "3px", cursor: "pointer" }}
              onClick={() => {
                void navigator.clipboard.writeText(JSON.stringify(latestPipelineTrace, null, 2));
              }}
            >
              Copy Trace
            </button>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "6px", marginBottom: "8px", fontSize: "11px", color: "#9ca3af" }}>
            <div>Request ID: <span style={{ color: "#f3f4f6" }}>{latestPipelineTrace.request_id.slice(0, 8)}...</span></div>
            <div>Elapsed: <span style={{ color: "#f3f4f6" }}>{latestPipelineTrace.total_elapsed_ms}ms</span></div>
            {latestPipelineTrace.failing_stage && (
              <div style={{ color: "#f87171", fontWeight: "bold" }}>Failing Stage: {latestPipelineTrace.failing_stage}</div>
            )}
          </div>
          {latestPipelineTrace.token_usage && (
            <div style={{ display: "flex", gap: "12px", marginBottom: "8px", fontSize: "11px", color: "#9ca3af" }}>
              <div>
                Narrator tokens:{" "}
                <span style={{ color: "#f3f4f6" }}>
                  {latestPipelineTrace.token_usage.narrator_prompt_tokens ?? "?"} in /{" "}
                  {latestPipelineTrace.token_usage.narrator_completion_tokens ?? "?"} out
                  {latestPipelineTrace.token_usage.narrator_estimated ? " (est.)" : ""}
                </span>
              </div>
              <div>
                Evaluator tokens:{" "}
                <span style={{ color: "#f3f4f6" }}>
                  {latestPipelineTrace.token_usage.evaluator_prompt_tokens ?? "?"} in /{" "}
                  {latestPipelineTrace.token_usage.evaluator_completion_tokens ?? "?"} out
                  {latestPipelineTrace.token_usage.evaluator_estimated ? " (est.)" : ""}
                </span>
              </div>
            </div>
          )}
          {latestPipelineTrace.suggested_debug_action && (
            <div style={{ padding: "6px 8px", backgroundColor: "#7f1d1d", color: "#fca5a5", borderRadius: "4px", marginBottom: "8px", fontSize: "11px" }}>
              <strong>Debug Suggestion:</strong> {latestPipelineTrace.suggested_debug_action}
            </div>
          )}
          <div style={{ display: "flex", gap: "6px", overflowX: "auto", paddingBottom: "4px", maxHeight: "120px" }}>
            {latestPipelineTrace.stages.map((stage) => {
              let badgeColor = "#10b981";
              if (stage.status === "failed") badgeColor = "#ef4444";
              else if (stage.status === "warning") badgeColor = "#f59e0b";
              else if (stage.status === "skipped") badgeColor = "#6b7280";

              return (
                <div key={stage.stage_name} style={{ flexShrink: 0, padding: "6px 8px", backgroundColor: "#1f2937", border: `1px solid ${stage.status === "failed" ? "#ef4444" : "#374151"}`, borderRadius: "4px", width: "160px" }}>
                  <div style={{ fontWeight: "bold", textOverflow: "ellipsis", overflow: "hidden", whiteSpace: "nowrap", color: "#f3f4f6" }} title={stage.stage_name}>
                    {stage.stage_name}
                  </div>
                  <div style={{ fontSize: "10px", marginTop: "2px", display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: badgeColor }}>{stage.status}</span>
                    <span>{stage.elapsed_ms}ms</span>
                  </div>
                  {stage.error_message && (
                    <div style={{ fontSize: "9px", color: "#fca5a5", marginTop: "4px", maxHeight: "40px", overflow: "hidden", textOverflow: "ellipsis" }} title={stage.error_message}>
                      {stage.error_message}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
      {import.meta.env.DEV && renderTrace && (
        <div className="dev-render-trace-banner" style={{ padding: "10px 16px", backgroundColor: "#1e293b", borderBottom: "1px solid #334155", fontSize: "11px", fontFamily: "monospace", color: "#94a3b8" }}>
          <div style={{ color: "#38bdf8", fontWeight: "bold", marginBottom: "4px" }}>
            Render Source: prepareMessagesForRender (Deduplicated)
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "4px 12px" }}>
            <div>Saved Count: <span style={{ color: "#f8fafc" }}>{renderTrace.saved_message_count}</span></div>
            <div>Pending Count: <span style={{ color: "#f8fafc" }}>{renderTrace.pending_message_count}</span></div>
            <div>Dup Saved Suppressed: <span style={{ color: "#f8fafc" }}>{renderTrace.duplicate_saved_suppressed}</span></div>
            <div>Dup Pending Suppressed: <span style={{ color: "#f8fafc" }}>{renderTrace.duplicate_pending_suppressed}</span></div>
            <div>Pending Replaced: <span style={{ color: "#f8fafc" }}>{renderTrace.pending_replaced_by_saved}</span></div>
            <div>Active Listeners: <span style={{ color: "#f8fafc" }}>{renderTrace.active_listener_count}</span></div>
            <div>Visual Pair: <span style={{ color: "#f8fafc" }}>{renderTrace.duplicate_visual_pair ? "yes" : "no"}</span></div>
            <div>Saved DB Dup: <span style={{ color: "#f8fafc" }}>{renderTrace.duplicate_saved_db_assistant_detected ? "yes" : "no"}</span></div>
          </div>
          <div style={{ marginTop: "6px", maxHeight: "96px", overflow: "auto" }}>
            {renderTrace.visible_bubble_trace.map((row) => (
              <div key={`${row.render_index}-${row.message_id}-${row.render_source}`}>
                #{row.render_index} {row.role} {row.render_source} id={row.message_id} req={row.request_id || "-"} hash={row.content_hash}
                {row.duplicate_visual_pair ? ` duplicate=${row.duplicate_render_sources?.join("+")}` : ""}
              </div>
            ))}
          </div>
        </div>
      )}
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
        <button type="button" onClick={handleExportLlmPayloadHistory} disabled={busy || !currentConversationId}>
          <FileDown size={14} />
          <span>Export Payload</span>
        </button>
        <button type="button" onClick={handleClearDevLogs} disabled={devLogs.length === 0}>
          <Trash2 size={14} />
          <span>Clear</span>
        </button>
      </div>
      <div className="dev-console-actions" style={{ borderTop: "1px solid #374151", paddingTop: "8px", marginTop: "8px", gap: "8px" }}>
        <button type="button" onClick={handleValidateMne}>
          <Clipboard size={14} />
          <span>Validate .mne</span>
        </button>
        <button type="button" onClick={handlePreviewMne}>
          <FileDown size={14} />
          <span>Preview .mne</span>
        </button>
        <button type="button" onClick={handleImportMneAsNew}>
          <FileUp size={14} />
          <span>Import as new copy</span>
        </button>
      </div>
      {import.meta.env.DEV ? (
        <section className="dev-command-console" aria-label="Dev Command Console">
          <div className="dev-command-header">
            <div>
              <span className="eyebrow">Whitelisted commands</span>
              <h3>Command Runner</h3>
            </div>
            <button
              type="button"
              onClick={() =>
                void handleRunDevCommand("dedupe_active_adjacent_user_messages", {
                  conversationId: currentConversationId,
                })
              }
              disabled={devCommandRunning || !currentConversationId}
            >
              Repair Duplicate Turns
            </button>
          </div>
          <div className="dev-command-grid">
            <label>
              <span>Command</span>
              <select
                value={devCommandName}
                onChange={(event) => {
                  const nextName = event.target.value as DevCommandName;
                  setDevCommandName(nextName);
                  setDevCommandArgs(
                    DEV_COMMAND_OPTIONS.find((option) => option.name === nextName)?.defaultArgs ??
                      "{}",
                  );
                }}
              >
                {DEV_COMMAND_OPTIONS.map((option) => (
                  <option key={option.name} value={option.name}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>JSON Args</span>
              <textarea
                value={devCommandArgs}
                onChange={(event) => setDevCommandArgs(event.target.value)}
                spellCheck={false}
                rows={4}
              />
            </label>
          </div>
          <button
            className="dev-command-run"
            type="button"
            onClick={() => void handleRunDevCommand()}
            disabled={devCommandRunning || !currentConversationId}
          >
            <Play size={14} />
            <span>{devCommandRunning ? "Running..." : "Run"}</span>
          </button>
          {devCommandResult ? (
            <div className="dev-command-result">
              <span>Result</span>
              <pre>{devCommandResult}</pre>
            </div>
          ) : null}
          {devCommandError ? (
            <div className="dev-command-error">
              <span>Error</span>
              <pre>{devCommandError}</pre>
            </div>
          ) : null}
        </section>
      ) : null}
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
  ) : null;
  if (disclaimerMode) {
    return disclaimerScreen;
  }

  if (view === "chat") {
    const currentConversation = conversations.find((c) => c.conversation_id === currentConversationId);
    const isCurrentSessionArchived = currentConversation
      ? (Boolean(currentConversation.archived_at) || currentConversation.title.startsWith("[Archived] "))
      : currentSessionTitle.startsWith("[Archived] ");

    return (
      <main className="chat-only-shell">
        <header className="chat-only-header">
          <button className="ghost-action" onClick={() => setView("library")}>
            <ArrowLeft size={18} />
            <span>Library</span>
          </button>
          <SoulAvatar soulName={soul?.character_name ?? "Mnemosyne"} asset={selectedAvatarAsset} />
          <div>
            <span className="eyebrow">
              {setting?.setting_name ?? "Local Setting"} / {provider} / {mode}
            </span>
            <h1>
              {currentSessionTitle || "New Session"}
              <button
                className="inline-icon-button"
                type="button"
                title="Rename session"
                onClick={handleRenameCurrentSession}
                disabled={busy}
              >
                <Pencil size={14} />
              </button>
            </h1>
            <p>{soul?.character_name ?? "Mnemosyne"}</p>
            <p className="session-state-label">{sessionContinuityLabel}</p>
          </div>
          <div className="chat-top-actions">
            {isCurrentSessionArchived ? (
              <button
                className="ghost-action"
                type="button"
                title="Restore session"
                onClick={() => handleRestoreSession()}
                disabled={busy}
              >
                <RefreshCcw size={16} />
                <span>Restore Session</span>
              </button>
            ) : (
              <button
                className="ghost-action danger"
                type="button"
                title="Archive session"
                onClick={() => handleDeleteChat()}
                disabled={busy}
              >
                <Trash2 size={16} />
                <span>Archive Session</span>
              </button>
            )}
            <button
              className="ghost-action"
              type="button"
              title="Restore turns hidden by delete/rewind"
              onClick={handleRestoreHiddenTurns}
              disabled={busy || !currentConversationId}
            >
              <RefreshCcw size={16} />
              <span>Restore Turns</span>
            </button>
            <button
              className="ghost-action"
              type="button"
              title="Export current session checkpoint as .mne"
              onClick={handleExportCurrentSessionMne}
              disabled={busy || !currentConversationId}
            >
              <FileDown size={16} />
              <span>Session .mne</span>
            </button>
            <div className="token-pill">
              {context?.estimated_tokens ?? 0}
              <span>tok</span>
            </div>
            {settingsDrawerToggle}
            {devConsoleToggle}
          </div>
        </header>

        <section className="chat-only-scroll" ref={chatOnlyBodyRef}>
          <div className="chat-only-body">
          {activeMessages.length === 0 ? (
            <div className="empty-state">
              <MessageSquareText size={34} />
              <p>No messages yet.</p>
            </div>
          ) : (
            activeMessages.map((message) => {
              const variants = variantsByMessage[message.id] ?? [];
              const selectedIndex = selectedVariantIndex(variants);
              const canSelectVariant = message.id === latestAssistantMessageId;
              const canGenerateFromUser = canGenerateFromUserMessage(message);
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
                            onClick={() => handleSelectVariant(message, -1)}
                            disabled={turnInProgress || !canSelectVariant || variants.length <= 1 || selectedIndex <= 0}
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
                              turnInProgress ||
                              !canSelectVariant ||
                              variants.length <= 1 ||
                              selectedIndex >= variants.length - 1
                            }
                          >
                            <ArrowLeft size={13} className="next-variant-icon" />
                          </button>
                        </div>
                        <button
                          title="Hide/Rewind response"
                          onClick={() => handleDeleteChatMessage(message)}
                          disabled={turnInProgress}
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    ) : (
                      <div className="message-tools">
                        <button
                          className="message-tool-action"
                          title={canGenerateFromUser ? "Regenerate response from this user message" : olderGenerationTitle}
                          onClick={() => handleRegenerateFromUserMessage(message)}
                          disabled={turnInProgress || !canGenerateFromUser}
                        >
                          <RefreshCcw size={14} />
                          <span>Regenerate</span>
                        </button>
                        <button
                          className="message-tool-action"
                          title={canGenerateFromUser ? "Fix response with instruction" : olderGenerationTitle}
                          onClick={() => handleFixFromUserMessage(message)}
                          disabled={turnInProgress || !canGenerateFromUser}
                        >
                          <span>Fix</span>
                        </button>
                        <button
                          title="Edit this message"
                          onClick={() => handleEditUserMessage(message)}
                          disabled={turnInProgress}
                        >
                          <Pencil size={14} />
                        </button>
                        <button
                          title="Hide/Rewind message"
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
                  {message.attachments?.length ? (
                    <div className="message-attachments">
                      {message.attachments.map((attachment) => (
                        <button
                          className="image-attachment"
                          type="button"
                          key={attachment.id}
                          onClick={() => setPreviewImageAsset(attachment.image)}
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
          <div ref={chatBottomRef} aria-hidden="true" />
          </div>
        </section>

        {showEvaluatorJobBanner && activeEvaluatorJob ? (
          <section className={`evaluator-job-banner ${activeEvaluatorJob.status}`}>
            <button
              type="button"
              className="evaluator-job-banner-close"
              aria-label="Close state updater status"
              title="Close"
              onClick={handleDismissEvaluatorJobBanner}
            >
              <X size={14} />
            </button>
            <div>
              <strong>{evaluatorJobBannerTitle(activeEvaluatorJob)}</strong>
              <span>
                {activeEvaluatorJob.model || "Evaluator"} / {activeEvaluatorJob.status}
                {activeEvaluatorJob.elapsed_ms ? ` / ${activeEvaluatorJob.elapsed_ms}ms` : ""}
              </span>
              {activeEvaluatorJob.error_message ? <small>{activeEvaluatorJob.error_message}</small> : null}
            </div>
            <div className="evaluator-job-actions">
              {activeEvaluatorJobIsLive ? (
                <button type="button" onClick={handleCancelEvaluatorJob}>
                  Cancel
                </button>
              ) : null}
              {activeEvaluatorJob.status === "failed" ||
              activeEvaluatorJob.status === "canceled" ||
              activeEvaluatorJob.status === "timed_out" ? (
                <button type="button" onClick={handleRetryEvaluatorJob}>
                  Retry
                </button>
              ) : null}
              {activeEvaluatorJobIsLive && effectiveStateUpdaterSettings.allow_send_with_stale_state ? (
                <button type="button" onClick={handleProceedWithStaleState}>
                  Proceed
                </button>
              ) : null}
            </div>
          </section>
        ) : null}

        {personaModalMode ? (
          <section className="persona-modal-backdrop" role="dialog" aria-modal="true">
            <div className="persona-modal">
              <header>
                <div>
                  <span className="eyebrow">Persona</span>
                  <h2>
                    {personaModalMode === "list"
                      ? "Player Persona"
                      : personaModalMode === "add"
                        ? "Add Persona"
                        : "Edit Persona"}
                  </h2>
                </div>
                <button type="button" title="Close" onClick={closePersonaModal}>
                  <X size={16} />
                </button>
              </header>
              {personaModalMode === "list" ? (
                <div className="persona-list">
                  {playerPersonas.map((persona) => (
                    <article
                      key={persona.persona_id}
                      className={
                        activePlayerPersona?.persona_id === persona.persona_id
                          ? "persona-row selected"
                          : "persona-row"
                      }
                    >
                      <div>
                        <strong>{persona.display_name}</strong>
                        <span>{persona.persona_id}</span>
                        <p>{persona.description}</p>
                      </div>
                      <div className="persona-row-actions">
                        <button type="button" onClick={() => handleSelectPersona(persona.persona_id)}>
                          Select
                        </button>
                        {!persona.is_builtin ? (
                          <button type="button" onClick={() => openPersonaEdit(persona.persona_id)}>
                            Edit
                          </button>
                        ) : null}
                      </div>
                    </article>
                  ))}
                  <div className="persona-list-actions">
                    <button type="button" className="persona-add-button" onClick={openPersonaAdd}>
                      Add Persona
                    </button>
                    {personaListConfirmRequired ? (
                      <div className="persona-form-actions">
                        <button type="button" className="persona-cancel-button" onClick={closePersonaModal}>
                          Cancel
                        </button>
                        <button
                          type="button"
                          className="persona-confirm-button"
                          onClick={handleConfirmPersonaList}
                          disabled={!activePlayerPersona}
                        >
                          Confirm Persona
                        </button>
                      </div>
                    ) : null}
                  </div>
                </div>
              ) : (
                <div className="persona-form">
                  <label>
                    <span>Name</span>
                    <input
                      value={personaForm.display_name}
                      onChange={(event) =>
                        setPersonaForm((current) => ({ ...current, display_name: event.target.value }))
                      }
                    />
                  </label>
                  <label>
                    <span>Gender code</span>
                    <input
                      value={personaForm.gender_code}
                      onChange={(event) =>
                        setPersonaForm((current) => ({ ...current, gender_code: event.target.value }))
                      }
                    />
                  </label>
                  <label>
                    <span>Pronouns</span>
                    <input
                      value={personaForm.pronouns}
                      onChange={(event) =>
                        setPersonaForm((current) => ({ ...current, pronouns: event.target.value }))
                      }
                    />
                  </label>
                  <label>
                    <span>Description</span>
                    <textarea
                      value={personaForm.description}
                      onChange={(event) =>
                        setPersonaForm((current) => ({ ...current, description: event.target.value }))
                      }
                    />
                  </label>
                  <label>
                    <span>Appearance</span>
                    <textarea
                      value={personaForm.appearance ?? ""}
                      onChange={(event) =>
                        setPersonaForm((current) => ({ ...current, appearance: event.target.value }))
                      }
                    />
                  </label>
                  <label>
                    <span>Notes</span>
                    <textarea
                      value={personaForm.notes ?? ""}
                      onChange={(event) =>
                        setPersonaForm((current) => ({ ...current, notes: event.target.value }))
                      }
                    />
                  </label>
                  <div className="persona-form-actions">
                    <button type="button" onClick={() => setPersonaModalMode("list")}>
                      Cancel
                    </button>
                    <button type="button" onClick={handleSavePersona}>
                      Save
                    </button>
                  </div>
                </div>
              )}
            </div>
          </section>
        ) : null}

        <form className="chat-only-composer" onSubmit={handleSubmit}>
          <input
            ref={chatImageInputRef}
            className="hidden-file"
            type="file"
            accept="image/png,image/jpeg,image/webp,image/gif,.png,.jpg,.jpeg,.webp,.gif"
            onChange={handleChatImageSelected}
          />
          <div className="composer-input-shell">
            <textarea
              value={draft}
              onChange={(event) => handleDraftChange(event.target.value)}
              onKeyDown={handleComposerKeyDown}
              placeholder="Type message... Enter to send, Shift+Enter for a new line."
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
                    className={index === slashSelectedIndex ? "selected" : ""}
                    role="option"
                    aria-selected={index === slashSelectedIndex}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => insertSlashCommand(item.command)}
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
            <button type="button" aria-label="Stop generation" onClick={handleStopGeneration}>
              <Square size={16} />
            </button>
          ) : (
            <>
              <button
                type="button"
                aria-label="Attach image"
                title="Attach image"
                onClick={() => chatImageInputRef.current?.click()}
                disabled={!soul || stateUpdating}
              >
                <ImageIcon size={18} />
              </button>
              <button aria-label="Send message" disabled={!draft.trim() || !soul || stateUpdating}>
                <Play size={18} />
              </button>
            </>
          )}
        </form>
        <ImagePreviewModal asset={previewImageAsset} onClose={() => setPreviewImageAsset(null)} />
        {settingsDrawerPanel}
        {devConsolePanel}
      </main>
    );
  }

  return (
    <main className="app-shell launcher-shell">
      <header className="launcher-header">
        <div>
          <span className="eyebrow">Launcher</span>
          <h1>Choose World, Primary Character, Session</h1>
          <p>
            {setting?.setting_name ?? "No world selected"} / {soul?.character_name ?? "No primary character"} /{" "}
            {selectedCharacterCount} selected
          </p>
        </div>
        <div className="launcher-actions">
          {settingsDrawerToggle}
          {devConsoleToggle}
        </div>
      </header>

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

          <div className="action-grid compact-actions">
            <input
              ref={settingImportInputRef}
              className="hidden-file"
              type="file"
              accept="application/json,.json,.setting"
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
            <button title="Export World as .mne" onClick={handleExportSettingMne} disabled={!setting || busy}>
              <FileDown size={18} />
              <span>.mne</span>
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
              title="Archive selected Setting"
              onClick={handleArchiveSetting}
              disabled={!setting || busy}
            >
              <Archive size={18} />
              <span>Archive</span>
            </button>
          </div>

          <section className="compact-list library-list world-picker-list" aria-label="Saved worlds">
            {settings.length === 0 ? (
              <p className="muted">No saved Settings yet.</p>
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

          <section className={`collapsible-section ${settingEditorOpen ? "open" : ""}`}>
            <button
              className="section-toggle studio-toggle"
              type="button"
              onClick={() => setSettingEditorOpen((open) => !open)}
              aria-expanded={settingEditorOpen}
            >
              <span>
                <span className="eyebrow">World Editor</span>
                <strong>{setting?.setting_name ?? "Create or edit a world"}</strong>
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
              <span className="eyebrow">Primary Character</span>
              <h2>{soul?.character_name ?? "Select a Character"}</h2>
              <p>{primaryCharacterDescription}</p>
            </div>
            <SoulAvatar soulName={soul?.character_name ?? "Mnemosyne"} asset={selectedAvatarAsset} />
          </header>

          <div className="action-grid compact-actions">
            <input
              ref={importInputRef}
              className="hidden-file"
              type="file"
              accept="application/json,.json,.soul"
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
            <button title="Snapshot current Soul into the library" onClick={handleCreateSnapshot} disabled={!soul || busy}>
              <Save size={18} />
              <span>Snapshot</span>
            </button>
            <button title="Export Current Soul without modifying the app library" onClick={handleSaveSoul} disabled={!soul || busy}>
              <FileDown size={18} />
              <span>Export JSON</span>
            </button>
            <button title="Export Soul as .mne" onClick={handleExportSoulMne} disabled={!soul || busy}>
              <FileDown size={18} />
              <span>Export .mne</span>
            </button>
            <button title="Export Soul + World as .mne" onClick={handleExportScenarioMne} disabled={!soul || !setting || busy}>
              <FileDown size={18} />
              <span>Scenario .mne</span>
            </button>
            <button title="Import .mne bundle" onClick={handleImportMne} disabled={busy}>
              <FileUp size={18} />
              <span>Import .mne</span>
            </button>
            <button title="Run consolidation" onClick={handleConsolidate} disabled={!soul || busy}>
              <RefreshCcw size={18} />
              <span>Sleep</span>
            </button>
            <button
              className="danger-button"
              title="Archive selected Soul"
              onClick={handleDeleteSoul}
              disabled={!soul || busy}
            >
              <Trash2 size={18} />
              <span>Archive</span>
            </button>
          </div>

          <section className="character-grid" aria-label="Saved characters">
            {souls.length === 0 ? (
              <p className="muted">No saved Souls yet.</p>
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

          <section className={`collapsible-section ${soulEditorOpen ? "open" : ""}`}>
            <button
              className="section-toggle studio-toggle"
              type="button"
              onClick={() => setSoulEditorOpen((open) => !open)}
              aria-expanded={soulEditorOpen}
            >
              <span>
                <span className="eyebrow">Character Editor</span>
                <strong>{soul?.character_name ?? "Create or edit a character"}</strong>
              </span>
              <ChevronDown size={18} aria-hidden="true" />
            </button>

            {soulEditorOpen ? (
              <div className="collapsible-content">
                <div className="form-grid single-column-form">
                  <section className="avatar-editor">
                    <SoulAvatar soulName={characterName} asset={draftAvatarAsset} />
                    <div>
                      <strong>Draft Profile Picture</strong>
                      <p>{draftAvatarAsset ? imageAssetMeta(draftAvatarAsset) : "Draft has no profile picture"}</p>
                      <div className="inline-actions">
                        <input
                          ref={avatarInputRef}
                          className="hidden-file"
                          type="file"
                          accept="image/png,image/jpeg,image/webp,image/gif,.png,.jpg,.jpeg,.webp,.gif"
                          onChange={handleAvatarImageSelected}
                        />
                        <button type="button" onClick={() => avatarInputRef.current?.click()} disabled={busy}>
                          <ImageIcon size={16} />
                          <span>{draftAvatarAsset ? "Change" : "Upload"}</span>
                        </button>
                        <button type="button" onClick={handleRemoveAvatarImage} disabled={busy || !draftAvatarImageId}>
                          <X size={16} />
                          <span>Remove</span>
                        </button>
                      </div>
                    </div>
                  </section>
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
        </section>
      </section>

      <section className="workspace-card provider-card">
        <header className="panel-header">
          <div>
            <span className="eyebrow">Connection</span>
            <h2>Provider Settings</h2>
          </div>
          <div className="panel-header-actions">
            <button type="button" className="ghost-action" onClick={handleViewDisclaimer}>
              <span>View Disclaimer</span>
            </button>
            <Sparkles aria-hidden="true" />
          </div>
        </header>

        <div className="session-strip launcher-provider-strip">
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
                    <span>Narrator Timeout (seconds)</span>
                    <input
                      type="number"
                      min="0"
                      value={Math.round((apiSettings.narrator_timeout_ms ?? 0) / 1000)}
                      onChange={(event) =>
                        setApiSettings((current) => ({
                          ...current,
                          narrator_timeout_ms: Number(event.target.value) > 0 ? Number(event.target.value) * 1000 : null,
                        }))
                      }
                      placeholder="0 = provider default"
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
                      <span>Custom Narrator Prompt</span>
                      <textarea
                        value={apiSettings.system_prompt}
                        onChange={(event) =>
                          setApiSettings((current) => ({
                            ...current,
                            system_prompt: event.target.value,
                          }))
                        }
                        placeholder="Replaces default narrator + mode prompts when filled. Use Custom mode. Leave empty for default Reader narration."
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
                    <Archive size={16} />
                    <span>Archive Profile</span>
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
                <div className="provider-pass-grid">
                  <label className="field">
                    <span>State Updater Timeout (seconds)</span>
                    <input
                      type="number"
                      min="0"
                      value={Math.round((effectiveStateUpdaterSettings.evaluator_timeout_ms ?? 25_000) / 1000)}
                      onChange={(event) =>
                        updateEffectiveStateUpdaterSettings({
                          evaluator_timeout_ms: Math.max(0, Number(event.target.value) || 0) * 1000,
                        })
                      }
                      disabled={busy}
                    />
                  </label>
                  <label className="field">
                    <span>Timeout Mode</span>
                    <select
                      value={effectiveStateUpdaterSettings.evaluator_timeout_mode ?? "finite"}
                      onChange={(event) =>
                        updateEffectiveStateUpdaterSettings({
                          evaluator_timeout_mode: event.target.value,
                        })
                      }
                      disabled={busy}
                    >
                      <option value="finite">Finite app timeout</option>
                      <option value="no_app_timeout">No app timeout</option>
                    </select>
                  </label>
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
                      <option value="evaluator_structured_v1">Structured Ops Evaluator</option>
                    </select>
                  </label>
                  <label className="field">
                    <span>Structured Evaluator Policy</span>
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
                <label className="toggle-row">
                  <input
                    type="checkbox"
                    checked={devOverrideActive}
                    onChange={(event) => setDevOverrideActive(event.target.checked)}
                    disabled={busy}
                  />
                  <span>Developer override (skip evaluator gates)</span>
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
                        <Archive size={16} />
                        <span>Archive Profile</span>
                      </button>
                      <button
                        type="button"
                        className="ghost-action"
                        onClick={() => void handleRunContractTest(selectedStateUpdaterProfileId)}
                        disabled={busy || !selectedStateUpdaterProfileId}
                      >
                        <Play size={16} />
                        <span>Run Contract Test</span>
                      </button>
                      <button
                        type="button"
                        className="ghost-action"
                        onClick={() => void handleRunStructuredDiagnostic()}
                        disabled={busy || structuredDiagnosticRunning || (!selectedStateUpdaterProfileId && !selectedProviderProfileId)}
                      >
                        <Play size={16} />
                        <span>Run Structured Evaluator Diagnostic</span>
                      </button>
                    </div>
                  </>
                )}
                {useNarratorProviderForUpdater && (
                  <div className="button-row">
                    <button
                      type="button"
                      className="ghost-action"
                      onClick={() => void handleRunStructuredDiagnostic()}
                      disabled={busy || structuredDiagnosticRunning || (!selectedStateUpdaterProfileId && !selectedProviderProfileId)}
                    >
                      <Play size={16} />
                      <span>Run Structured Evaluator Diagnostic</span>
                    </button>
                  </div>
                )}
                {(structuredDiagnosticResult || structuredDiagnosticError) && (
                  <div className="provider-note">
                    {structuredDiagnosticResult ? (
                      <>
                        <strong>
                          {structuredDiagnosticResult.resolved_evaluator_source === "evaluator_structured_v1" &&
                          structuredDiagnosticResult.evaluator_mode_per_run.every((mode) => mode === "evaluator_structured_v1") &&
                          structuredDiagnosticResult.runs.every((run) => !run.error)
                            ? "PASS"
                            : "FAIL"}
                        </strong>{" "}
                        {structuredDiagnosticResult.provider_model} · enforcement{" "}
                        {structuredDiagnosticResult.structured_enforcement_per_run.join(", ") || "none"} · fallback{" "}
                        {structuredDiagnosticResult.fallback_paths.map((path) => path.join(" > ")).join(" | ") || "none"}
                        <br />
                        Payload: {structuredDiagnosticResult.payload_history_path}
                        <br />
                        MNE: {structuredDiagnosticResult.mne_checkpoint_path}
                        <br />
                        Summary: {structuredDiagnosticResult.summary_json_path}
                      </>
                    ) : (
                      <><strong>FAIL</strong> {structuredDiagnosticError}</>
                    )}
                  </div>
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
                <dt>Anti-Replay</dt>
                <dd>
                  {lastTurnDebug
                    ? lastTurnDebug.replay_detected
                      ? `Triggered (${lastTurnDebug.replay_score.toFixed(2)})`
                      : `Passed (${lastTurnDebug.replay_score.toFixed(2)})`
                    : "-"}
                </dd>
              </div>
              <div>
                <dt>Replay Reason</dt>
                <dd>{lastTurnDebug?.replay_reason ?? "-"}</dd>
              </div>
              <div>
                <dt>Output Guard</dt>
                <dd>{lastTurnDebug?.output_contract_warning ?? "-"}</dd>
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
                  <span>Export Payload History</span>
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
                <dt>Custom Prompt</dt>
                <dd>
                  {llmPayload?.custom_prompt_status ??
                    (mode === "Custom" ? (apiSettings.system_prompt.trim() ? "included" : "empty") : "inactive")}
                </dd>
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
            {llmPayload?.memory_slot_debug?.length ? (
              <>
                <h3>Memory Slot Debug</h3>
                <pre>
                  {llmPayload.memory_slot_debug
                    .filter((trace) => trace.action === "selected")
                    .map(
                      (trace) =>
                        `${trace.slot}: ${trace.memory_id} / ${trace.reason} / score ${Math.round(trace.final_score)} / ${trace.source_type} / ${trace.truth_status}`,
                    )
                    .join("\n")}
                </pre>
              </>
            ) : null}
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
            <dl className="diagnostic-grid context-source-grid">
              <div>
                <dt>WORLD SNAPSHOT source</dt>
                <dd>session_world / setting.world, legacy soul.world fallback only</dd>
              </div>
              <div>
                <dt>CHARACTER SNAPSHOT source</dt>
                <dd>soul.profile/global/trauma/arousal</dd>
              </div>
              <div>
                <dt>MEMORY SLOTS source</dt>
                <dd>soul.memory, slot-scored by entity/plot/source/truth</dd>
              </div>
              <div>
                <dt>RELATIONSHIPS source</dt>
                <dd>soul.relationships</dd>
              </div>
              <div>
                <dt>LATEST EXCHANGE source</dt>
                <dd>visible conversation messages</dd>
              </div>
            </dl>
            <div className="repair-actions" aria-label="Debug repair tools">
              <button className="ghost-action" type="button" onClick={() => handleSoulRepair("world")} disabled={busy || !soul}>
                <Trash2 size={16} />
                <span>Clear World State</span>
              </button>
              <button className="ghost-action" type="button" onClick={() => handleSoulRepair("scenario")} disabled={busy || !soul}>
                <Trash2 size={16} />
                <span>Clear Scenario</span>
              </button>
              <button className="ghost-action" type="button" onClick={() => handleSoulRepair("events")} disabled={busy || !soul}>
                <Trash2 size={16} />
                <span>Clear Recent Events</span>
              </button>
              <button className="ghost-action danger" type="button" onClick={() => handleSoulRepair("memories")} disabled={busy || !soul}>
                <Trash2 size={16} />
                <span>Clear Memories</span>
              </button>
            </div>
            <pre>{context?.text ?? "No context compiled yet."}</pre>
          </section>
        </section>

        <footer className="status-line">
          <span>{status}</span>
          {latestPipelineTrace &&
            (status.toLowerCase().includes("skipped") || status.toLowerCase().includes("failed")) && (
              <>
                {latestPipelineTrace.failing_stage ? (
                  <span
                    className="status-stage-tag"
                    style={{ marginLeft: "10px", color: "#f87171", fontWeight: "bold" }}
                  >
                    (Failing Stage: {latestPipelineTrace.failing_stage})
                  </span>
                ) : (
                  (() => {
                    const warningStage = latestPipelineTrace.stages.find((s) => s.status === "warning");
                    return warningStage ? (
                      <span
                        className="status-stage-tag"
                        style={{ marginLeft: "10px", color: "#fbbf24", fontWeight: "bold" }}
                      >
                        (Warning Stage: {warningStage.stage_name})
                      </span>
                    ) : null;
                  })()
                )}
                <button
                  type="button"
                  style={{
                    marginLeft: "8px",
                    padding: "2px 6px",
                    fontSize: "11px",
                    border: "1px solid #4b5563",
                    background: "none",
                    color: "#9ca3af",
                    cursor: "pointer",
                    borderRadius: "3px",
                  }}
                  onClick={() => setDevConsoleOpen(true)}
                >
                  Open trace
                </button>
              </>
            )}
        </footer>
      </section>
      <ImagePreviewModal asset={previewImageAsset} onClose={() => setPreviewImageAsset(null)} />
      {settingsDrawerPanel}
      {devConsolePanel}
    </main>
  );
}

function hasAcceptedDisclaimerVersion() {
  try {
    const raw = localStorage.getItem(DISCLAIMER_STORAGE_KEY);
    if (!raw) return false;
    const parsed = JSON.parse(raw) as {
      accepted?: boolean;
      disclaimer_version?: number;
    };
    return parsed.accepted === true && (parsed.disclaimer_version ?? 0) >= DISCLAIMER_VERSION;
  } catch {
    return false;
  }
}

function loadStoredCustomNarratorPrompt() {
  try {
    return localStorage.getItem(CUSTOM_NARRATOR_PROMPT_STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
}

function loadStoredBoolean(key: string, fallback: boolean) {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    return raw === "true";
  } catch {
    return fallback;
  }
}

function loadStoredChatStartMode(): ChatStartMode {
  try {
    const raw = localStorage.getItem(CHAT_START_MODE_STORAGE_KEY);
    return raw === "continue" || raw === "fresh" ? raw : "fresh";
  } catch {
    return "fresh";
  }
}

function loadStoredSettingsTab(): SettingsTab {
  try {
    const raw = localStorage.getItem(SETTINGS_DRAWER_TAB_STORAGE_KEY);
    return raw === "ai" || raw === "chat" || raw === "library" || raw === "dev" ? raw : "ai";
  } catch {
    return "ai";
  }
}

function loadStoredDevLogLevelFilter(): DevLogLevel | "all" {
  try {
    const raw = localStorage.getItem(DEV_LOG_LEVEL_FILTER_STORAGE_KEY);
    return raw === "all" || DEV_LOG_LEVELS.includes(raw as DevLogLevel) ? (raw as DevLogLevel | "all") : "all";
  } catch {
    return "all";
  }
}

function loadStoredDevLogCategoryFilter(): DevLogCategory | "all" {
  try {
    const raw = localStorage.getItem(DEV_LOG_CATEGORY_FILTER_STORAGE_KEY);
    return raw === "all" || DEV_LOG_CATEGORIES.includes(raw as DevLogCategory)
      ? (raw as DevLogCategory | "all")
      : "all";
  } catch {
    return "all";
  }
}

function DisclaimerScreen({
  mode,
  understood,
  remember,
  onUnderstoodChange,
  onRememberChange,
  onAccept,
  onClose,
}: {
  mode: DisclaimerMode;
  understood: boolean;
  remember: boolean;
  onUnderstoodChange: (value: boolean) => void;
  onRememberChange: (value: boolean) => void;
  onAccept: () => void;
  onClose: () => void;
}) {
  const isLaunch = mode === "launch";

  return (
    <main className="disclaimer-screen">
      <section className="disclaimer-card" role="dialog" aria-modal={isLaunch} aria-labelledby="disclaimer-title">
        <div className="disclaimer-heading">
          <span className="eyebrow">Before you continue</span>
          <h1 id="disclaimer-title">Mnemosyne Experimental Use Disclaimer</h1>
        </div>

        <div className="disclaimer-copy">
          <p>Mnemosyne is experimental open-source AI roleplay software.</p>
          <p>
            Mnemosyne creates fictional continuity through memory, state tracking, and AI-generated narration. Characters may
            appear persistent, emotionally responsive, or self-aware, but the software does not verify consciousness,
            sentience, or real personhood.
          </p>
          <p>
            You are responsible for how you use this software, what models/providers you connect, what content you generate,
            and how you interpret fictional character continuity.
          </p>
          <p>
            Mnemosyne is not therapy, medical care, legal advice, crisis support, or a substitute for real-world relationships
            or professional help.
          </p>
          <p>
            If you use external API providers, your prompts, character data, and generated responses may be sent to those
            providers according to their own policies. Local use depends on your own configuration.
          </p>
          <p>
            This software may produce emotionally intense, disturbing, intimate, fictional, or misleading outputs. Use personal
            caution, especially during long sessions or emotionally heavy roleplay.
          </p>
          <p>
            By continuing, you acknowledge that Mnemosyne is an experimental fiction and roleplay engine, and that you use it
            at your own discretion.
          </p>
        </div>

        {isLaunch ? (
          <div className="disclaimer-options">
            <label>
              <input
                type="checkbox"
                checked={understood}
                onChange={(event) => onUnderstoodChange(event.target.checked)}
              />
              <span>I understand and want to continue.</span>
            </label>
            <label>
              <input
                type="checkbox"
                checked={remember}
                onChange={(event) => onRememberChange(event.target.checked)}
              />
              <span>Do not show this again</span>
            </label>
          </div>
        ) : null}

        <div className="disclaimer-actions">
          {isLaunch ? (
            <>
              <button type="button" className="ghost-action disclaimer-exit" onClick={() => window.close()}>
                Exit
              </button>
              <button type="button" className="start-chat-button" onClick={onAccept} disabled={!understood}>
                Accept and Continue
              </button>
            </>
          ) : (
            <button type="button" className="start-chat-button" onClick={onClose}>
              Close
            </button>
          )}
        </div>
      </section>
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
  const memorySlotDebug = payload.memory_slot_debug?.length
    ? `\n\n=== MEMORY SLOT DEBUG ===\n${payload.memory_slot_debug
        .filter((trace) => trace.action === "selected")
        .map(
          (trace) =>
            `${trace.slot}: ${trace.memory_id} / ${trace.reason} / score ${Math.round(trace.final_score)} / ${trace.source_type} / ${trace.truth_status}`,
        )
        .join("\n")}`
    : "";
  return `=== SYSTEM MESSAGE ===
${payload.system_message}

=== CONTEXT, already included inside SYSTEM MESSAGE ===
${payload.context}
${memorySlotDebug}
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
Custom Prompt: ${payload.custom_prompt_status}
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

function SoulAvatar({ soulName, asset }: { soulName: string; asset?: ImageAsset | null }) {
  return (
    <div className={`avatar ${asset ? "image-avatar" : ""}`} aria-hidden="true">
      {asset ? <AssetImage asset={asset} alt="" /> : soulName.slice(0, 1) || "M"}
    </div>
  );
}

function AssetImage({ asset, alt }: { asset: ImageAsset; alt: string }) {
  const [src, setSrc] = useState("");
  useEffect(() => {
    let cancelled = false;
    getImageAssetDataUrl(asset.id)
      .then((dataUrl) => {
        if (!cancelled) setSrc(dataUrl);
      })
      .catch(() => {
        if (!cancelled) setSrc("");
      });
    return () => {
      cancelled = true;
    };
  }, [asset.id]);
  return src ? <img src={src} alt={alt} /> : <span className="image-loading">Loading image</span>;
}

function ImagePreviewModal({
  asset,
  onClose,
}: {
  asset: ImageAsset | null;
  onClose: () => void;
}) {
  if (!asset) return null;
  return (
    <div className="image-preview-backdrop" role="dialog" aria-modal="true" onClick={onClose}>
      <section className="image-preview-modal" onClick={(event) => event.stopPropagation()}>
        <button type="button" className="image-preview-close" onClick={onClose} aria-label="Close image preview">
          <X size={18} />
        </button>
        <AssetImage asset={asset} alt="Image preview" />
        <div className="image-preview-meta">
          <strong>{asset.source}</strong>
          <span>{imageAssetMeta(asset)}</span>
          {asset.prompt ? <p>{asset.prompt}</p> : null}
        </div>
      </section>
    </div>
  );
}

function imageAssetMeta(asset: ImageAsset) {
  const dimensions = asset.width && asset.height ? `${asset.width}x${asset.height}` : "dimensions unknown";
  const mime = asset.mime_type ?? "image";
  return `${mime} / ${dimensions}`;
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
          ? { ...message, content: "", pending: true }
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
          pending: true,
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
      pending: true,
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
  let pendingAssistantReplacedBySaved = 0;
  if (existingIndex >= 0) {
    const next = [...messages];
    next[existingIndex] = savedMessage;
    const cleaned = removeDuplicateStreamingAssistants(next, savedMessage.conversation_id, savedMessage.id);
    return {
      messages: cleaned,
      trace: messageRenderTrace(cleaned, messages.length - cleaned.length, 0),
    };
  }

  if (savedMessage.role === "assistant") {
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      const message = messages[index];
      if (
        message.conversation_id === savedMessage.conversation_id &&
        message.role === "assistant" &&
        message.pending &&
        pendingMatchesSavedAssistant(message, savedMessage, null)
      ) {
        const next = [...messages];
        next[index] = savedMessage;
        pendingAssistantReplacedBySaved = 1;
        const cleaned = removeDuplicateStreamingAssistants(next, savedMessage.conversation_id, savedMessage.id);
        return {
          messages: cleaned,
          trace: messageRenderTrace(cleaned, messages.length - cleaned.length, pendingAssistantReplacedBySaved),
        };
      }
    }
  }

  const cleaned = removeDuplicateStreamingAssistants(
    [...messages, savedMessage].sort((left, right) => left.id - right.id),
    savedMessage.conversation_id,
    savedMessage.id,
  );
  return {
    messages: cleaned,
    trace: messageRenderTrace(cleaned, messages.length + 1 - cleaned.length, pendingAssistantReplacedBySaved),
  };
}

function pendingMatchesSavedAssistant(
  pending: ChatMessage,
  savedMessage: ChatMessage,
  activeGeneration?: ActiveGeneration | null,
) {
  if (!pending.pending && pending.id > 0) return false;
  if (pending.assistant_message_id && pending.assistant_message_id === savedMessage.id) return true;
  if (pending.request_id && savedMessage.request_id && pending.request_id === savedMessage.request_id) return true;
  if (
    activeGeneration &&
    pending.generation_id === activeGeneration.id &&
    activeGeneration.conversationId === savedMessage.conversation_id &&
    !activeGeneration.knownAssistantIds.has(savedMessage.id)
  ) {
    return true;
  }
  return pending.id < 0 && savedMessage.id > 0;
}

function messageRenderTrace(
  messages: ChatMessage[],
  duplicateSavedSuppressed: number,
  pendingAssistantReplacedBySaved: number,
): MessageRenderTrace {
  return buildMessageRenderTrace(messages, {
    activeListenerCount: 0,
    duplicateSavedSuppressed,
    duplicatePendingSuppressed: 0,
    pendingReplacedBySaved: pendingAssistantReplacedBySaved,
  });
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
        message.pending &&
        savedMessageId > 0
      ),
  );
}

function prepareMessagesForRender(messages: ChatMessage[], activeListenerCount: number = 0) {
  const active = messages.filter((message) => message.role !== "system");

  // First, split active into saved and pending
  const saved = active.filter((msg) => msg.id > 0 && !msg.pending);
  const pending = active.filter((msg) => msg.pending || msg.id < 0);

  const dedupedSaved: ChatMessage[] = [];
  const seenSavedIds = new Set<number>();
  const seenSavedRequestIds = new Set<string>();
  let duplicateSavedSuppressed = 0;
  let duplicateSavedDbAssistantDetected = false;

  for (const msg of saved) {
    if (seenSavedIds.has(msg.id)) {
      duplicateSavedSuppressed += 1;
      continue;
    }
    seenSavedIds.add(msg.id);

    if (msg.request_id) {
      if (seenSavedRequestIds.has(msg.request_id)) {
        if (msg.role === "assistant") {
          duplicateSavedDbAssistantDetected = true;
        } else {
          duplicateSavedSuppressed += 1;
          continue;
        }
      }
      seenSavedRequestIds.add(msg.request_id);
    }

    const isCloseDuplicate = dedupedSaved.some(
      (existing) =>
        existing.role === msg.role &&
        existing.content === msg.content &&
        Math.abs(existing.created_at - msg.created_at) < 10
    );
    if (isCloseDuplicate) {
      if (msg.role === "assistant" && msg.id !== dedupedSaved.find((existing) => existing.content === msg.content)?.id) {
        duplicateSavedDbAssistantDetected = true;
      } else {
        duplicateSavedSuppressed += 1;
        continue;
      }
    }

    dedupedSaved.push(msg);
  }

  const visiblePending: ChatMessage[] = [];
  let duplicatePendingSuppressed = 0;
  let pendingReplacedBySaved = 0;

  for (const pendingMsg of pending) {
    const hasMatchingSaved = dedupedSaved.some((savedMsg) => {
      if (pendingMsg.assistant_message_id && pendingMsg.assistant_message_id === savedMsg.id) {
        return true;
      }
      if (pendingMsg.request_id && savedMsg.request_id && pendingMsg.request_id === savedMsg.request_id) {
        return true;
      }
      if (
        pendingMsg.role === savedMsg.role &&
        pendingMsg.content.trim() &&
        contentHash(pendingMsg.content) === contentHash(savedMsg.content)
      ) {
        return true;
      }
      return false;
    });

    if (hasMatchingSaved) {
      pendingReplacedBySaved += 1;
      continue;
    }

    const isPendingDuplicate = visiblePending.some(
      (existing) =>
        (pendingMsg.assistant_message_id &&
          existing.assistant_message_id &&
          pendingMsg.assistant_message_id === existing.assistant_message_id) ||
        (pendingMsg.request_id && existing.request_id && pendingMsg.request_id === existing.request_id) ||
        (pendingMsg.role === existing.role &&
          pendingMsg.content === existing.content &&
          Math.abs(pendingMsg.created_at - existing.created_at) < 10)
    );
    if (isPendingDuplicate) {
      duplicatePendingSuppressed += 1;
      continue;
    }

    visiblePending.push(pendingMsg);
  }

  const rendered = [...dedupedSaved, ...visiblePending].sort((left, right) => {
    if (left.id < 0 && right.id > 0) return 1;
    if (left.id > 0 && right.id < 0) return -1;
    if (left.id < 0 && right.id < 0) {
      return left.created_at - right.created_at;
    }
    return left.id - right.id;
  });

  const savedMessageCount = dedupedSaved.length;
  const pendingMessageCount = visiblePending.length;
  const renderedMessageCount = rendered.length;
  const trace = buildMessageRenderTrace(rendered, {
    activeListenerCount,
    duplicateSavedSuppressed,
    duplicatePendingSuppressed,
    pendingReplacedBySaved,
    savedMessageCount,
    pendingMessageCount,
    renderedMessageCount,
    duplicateSavedDbAssistantDetected,
  });

  return {
    messages: rendered,
    trace,
  };
}

function contentHash(content: string) {
  let hash = 5381;
  const normalized = content.trim().replace(/\s+/g, " ");
  for (let index = 0; index < normalized.length; index += 1) {
    hash = (hash * 33) ^ normalized.charCodeAt(index);
  }
  return `h${(hash >>> 0).toString(16)}`;
}

function renderSourceForMessage(message: ChatMessage): VisibleBubbleRenderSource {
  if (message.id > 0 && !message.pending) return "saved_db";
  if (message.role === "assistant" && message.pending && message.content.trim()) return "streaming_overlay";
  if (message.role === "assistant" && message.pending) return "pending_overlay";
  if (message.id < 0) return "local_optimistic";
  return "unknown";
}

function buildVisibleBubbleTrace(messages: ChatMessage[]) {
  const trace: VisibleBubbleTraceRow[] = messages.map((message, index) => ({
    render_index: index,
    role: message.role,
    render_source: renderSourceForMessage(message),
    message_id: message.id,
    request_id: message.request_id ?? undefined,
    assistant_message_id: message.assistant_message_id ?? (message.role === "assistant" && message.id > 0 ? message.id : undefined),
    turn_id: message.turn_id ?? undefined,
    content_hash: contentHash(message.content),
    created_at: message.created_at,
    status: message.status,
    origin: message.origin,
  }));

  const assistantGroups = new Map<string, VisibleBubbleTraceRow[]>();
  for (const row of trace) {
    if (row.role !== "assistant") continue;
    const keys = [
      row.assistant_message_id ? `assistant:${row.assistant_message_id}` : "",
      row.content_hash ? `hash:${row.content_hash}` : "",
    ].filter(Boolean);
    for (const key of keys) {
      const group = assistantGroups.get(key) ?? [];
      group.push(row);
      assistantGroups.set(key, group);
    }
  }
  for (const group of assistantGroups.values()) {
    if (group.length < 2) continue;
    const sources = [...new Set(group.map((row) => row.render_source))];
    for (const row of group) {
      row.duplicate_visual_pair = true;
      row.duplicate_render_sources = sources;
    }
  }
  return trace;
}

function buildMessageRenderTrace(
  messages: ChatMessage[],
  options: {
    activeListenerCount: number;
    duplicateSavedSuppressed: number;
    duplicatePendingSuppressed: number;
    pendingReplacedBySaved: number;
    savedMessageCount?: number;
    pendingMessageCount?: number;
    renderedMessageCount?: number;
    duplicateSavedDbAssistantDetected?: boolean;
  },
): MessageRenderTrace {
  const rendered = messages.filter((message) => message.role !== "system");
  const pendingAssistantCount = rendered.filter((message) => message.role === "assistant" && message.pending).length;
  const renderedSavedMessageCount = rendered.filter((message) => message.id > 0 && !message.pending).length;
  const renderedPendingMessageCount = rendered.filter((message) => message.pending || message.id < 0).length;
  const visibleBubbleTrace = buildVisibleBubbleTrace(rendered);
  return {
    frontend_message_render_count: options.renderedMessageCount ?? rendered.length,
    saved_message_count: options.savedMessageCount ?? renderedSavedMessageCount,
    pending_message_count: options.pendingMessageCount ?? renderedPendingMessageCount,
    rendered_message_count: options.renderedMessageCount ?? rendered.length,
    duplicate_saved_suppressed: options.duplicateSavedSuppressed,
    duplicate_pending_suppressed: options.duplicatePendingSuppressed,
    pending_replaced_by_saved: options.pendingReplacedBySaved,
    pending_assistant_replaced_by_saved: options.pendingReplacedBySaved,
    active_listener_count: options.activeListenerCount,
    pending_assistant_count: pendingAssistantCount,
    rendered_saved_message_count: renderedSavedMessageCount,
    rendered_pending_message_count: renderedPendingMessageCount,
    duplicate_render_suppressed_count:
      options.duplicateSavedSuppressed + options.duplicatePendingSuppressed + options.pendingReplacedBySaved,
    duplicate_visual_pair: visibleBubbleTrace.some((row) => row.duplicate_visual_pair),
    duplicate_saved_db_assistant_detected: options.duplicateSavedDbAssistantDetected ?? false,
    visible_bubble_trace: visibleBubbleTrace,
  };
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
        !replacement.pending &&
        replacement.content.trim() &&
        replacement.content !== activeGeneration.replacementOriginalContent,
    );
  }

  return messages.some(
    (message) =>
      message.role === "assistant" &&
      message.id > 0 &&
      !message.pending &&
      message.content.trim() &&
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
  const statusBlocks = [...content.matchAll(/```status[\s\S]*?```/gi)];
  if (!statusBlocks.length) return content;
  const status = statusBlocks[statusBlocks.length - 1][0].trim();
  const body = content.replace(/```status[\s\S]*?```/gi, "").trimEnd();
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

function cloneForUi<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function formatSnapshotTimestamp(date: Date) {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(
    date.getHours(),
  )}${pad(date.getMinutes())}`;
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
  const openingNarratorMessage =
    stringFrom(profile.opening_narrator_message) ||
    stringFrom(record.opening_narrator_message) ||
    stringFrom(record.first_message) ||
    stringFrom(record.initial_message);
  const avatarImageId = stringFrom(profile.avatar_image_id) || stringFrom(record.avatar_image_id);
  const location = stringFrom(world.location) || scenario || base.world.location;
  const core = stringArrayFrom(isRecord(memory) ? memory.core : undefined);

  return {
    ...base,
    ...record,
    schema_version: Number(record.schema_version) || base.schema_version,
    character_id: stringFrom(record.character_id) || base.character_id,
    character_name: importedName || base.character_name,
    soul_kind: stringFrom(record.soul_kind) || "imported_package",
    source_soul_id: stringFrom(record.source_soul_id) || null,
    source_savepoint_id: stringFrom(record.source_savepoint_id) || null,
    created_from_name: stringFrom(record.created_from_name) || null,
    profile: {
      description,
      appearance,
      personality,
      scenario,
      opening_narrator_message: openingNarratorMessage,
      avatar_image_id: avatarImageId || null,
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
    scenario: stringFrom(record.scenario) || stringFrom(world.scenario),
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
