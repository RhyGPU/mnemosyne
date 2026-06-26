// App-wide constants extracted from App.tsx (Phase 0 refactor).
import { DevLogCategory, DevLogLevel } from "./tauri";
import { DevCommandName, PsychePresetName, PsycheDraft } from "./uiTypes";

export const DEFAULT_CONVERSATION_ID = "local-mock";
export const CONSOLIDATION_INTERVAL_TURNS = 10;
export const NARRATOR_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:narrator_provider_profile_id";
export const UPDATER_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:state_updater_provider_profile_id";
export const REPAIR_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:repair_provider_profile_id";
export const EMBEDDED_MODEL_PATH_STORAGE_KEY = "mnemosyne:embedded_repair_model_path";
export const REPAIR_MODEL_AUTO = "";
export const REPAIR_MODEL_EVALUATOR = "__evaluator__";
export const REPAIR_MODEL_EMBEDDED = "__embedded__";
export const USE_NARRATOR_FOR_UPDATER_STORAGE_KEY = "mnemosyne:use_narrator_provider_for_updater";
export const CUSTOM_NARRATOR_PROMPT_STORAGE_KEY = "mnemosyne:custom_narrator_prompt";
export const SETTINGS_DRAWER_OPEN_STORAGE_KEY = "mnemosyne:settings_drawer_open";
export const SETTINGS_DRAWER_TAB_STORAGE_KEY = "mnemosyne:settings_drawer_tab";
export const SETTINGS_FIRST_LAUNCH_SEEN_STORAGE_KEY = "mnemosyne:settings_first_launch_seen_v1";
export const CHAT_START_MODE_STORAGE_KEY = "mnemosyne:chat_start_mode";
export const SHOW_ARCHIVED_SESSIONS_STORAGE_KEY = "mnemosyne:show_archived_sessions";
export const EVALUATOR_EXECUTION_MODE_STORAGE_KEY = "mnemosyne:evaluator_execution_mode";
export const STRUCTURED_EVALUATOR_TRANSPORT_STORAGE_KEY = "mnemosyne:structured_evaluator_transport";
export const SESSIONS_PER_PAGE = 10;
export const DEV_CONSOLE_PAUSED_STORAGE_KEY = "mnemosyne:dev_console_paused";
export const DEV_LOG_LEVEL_FILTER_STORAGE_KEY = "mnemosyne:dev_log_level_filter";
export const DEV_LOG_CATEGORY_FILTER_STORAGE_KEY = "mnemosyne:dev_log_category_filter";
export const DISCLAIMER_STORAGE_KEY = "mnemosyne_disclaimer_accepted_v1";
export const DISCLAIMER_VERSION = 1;
export const DEV_LOG_LIMIT = 1000;
export const DEV_LOG_CATEGORIES: DevLogCategory[] = [
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
export const DEV_LOG_LEVELS: DevLogLevel[] = ["info", "warn", "error", "debug", "success"];

export const DEV_COMMAND_OPTIONS: Array<{ name: DevCommandName; label: string; defaultArgs: string }> = [
  {
    name: "dedupe_active_adjacent_user_messages",
    label: "Repair Duplicate Turns",
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
    name: "get_branch_patch_debug",
    label: "Branch Patch Debug",
    defaultArgs: "{}",
  },
];

export const PSYCHE_PRESETS: Record<PsychePresetName, PsycheDraft> = {
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
