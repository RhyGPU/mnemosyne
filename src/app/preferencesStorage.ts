import {
  DEFAULT_GENERATION_PREFERENCES,
  DEFAULT_READING_PREFERENCES,
  type ChatStartMode,
  type GenerationPreferences,
  type GenerationPresetName,
  type ReadingPreferences,
  type SettingsTab,
} from "../settings/preferences";

export const NARRATOR_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:narrator_provider_profile_id";
export const UPDATER_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:state_updater_provider_profile_id";
export const REPAIR_PROVIDER_PROFILE_STORAGE_KEY = "mnemosyne:repair_provider_profile_id";
export const EMBEDDED_MODEL_PATH_STORAGE_KEY = "mnemosyne:embedded_repair_model_path";
export const USE_NARRATOR_FOR_UPDATER_STORAGE_KEY = "mnemosyne:use_narrator_provider_for_updater";
export const CUSTOM_NARRATOR_PROMPT_STORAGE_KEY = "mnemosyne:custom_narrator_prompt";
export const SETTINGS_DRAWER_OPEN_STORAGE_KEY = "mnemosyne:settings_drawer_open";
export const SETTINGS_DRAWER_TAB_STORAGE_KEY = "mnemosyne:settings_drawer_tab";
export const SETTINGS_FIRST_LAUNCH_SEEN_STORAGE_KEY = "mnemosyne:settings_first_launch_seen_v1";
export const CHAT_START_MODE_STORAGE_KEY = "mnemosyne:chat_start_mode";
export const SHOW_ARCHIVED_SESSIONS_STORAGE_KEY = "mnemosyne:show_archived_sessions";
export const EVALUATOR_EXECUTION_MODE_STORAGE_KEY = "mnemosyne:evaluator_execution_mode";
export const STRUCTURED_EVALUATOR_TRANSPORT_STORAGE_KEY = "mnemosyne:structured_evaluator_transport";
export const GENERATION_PREFERENCES_STORAGE_KEY = "mnemosyne:generation_preferences_v1";
export const READING_PREFERENCES_STORAGE_KEY = "mnemosyne:reading_preferences_v1";
export const SESSIONS_PER_PAGE = 10;
export const DISCLAIMER_STORAGE_KEY = "mnemosyne_disclaimer_accepted_v1";
export const DISCLAIMER_VERSION = 1;
export const DEV_LOG_LIMIT = 1000;

/**
 * How long an evaluator call may run before the app gives up on it.
 *
 * This is the only evaluator timeout a person sets; the structured and
 * diagnostic timeouts stay unset unless a profile names one, so that whatever
 * is configured here is what actually reaches the provider call. A reasoning
 * model spends most of its budget before the first visible token, and that cost
 * grows with the transcript it is reading: glm-5.3-flash walked 49s to 106s over
 * five turns of one session. The ceiling has to sit above where that curve is
 * going, or a slow-but-correct model reads as a broken one.
 */
export const DEFAULT_EVALUATOR_TIMEOUT_MS = 180_000;

/**
 * How many turns in a row may lose their state before a benchmark gives up.
 *
 * One failure is a slow call. Three in a row is an evaluator that is not
 * working, and every further turn spends money measuring nothing.
 */
export const MAX_CONSECUTIVE_EVALUATOR_FAILURES = 3;

/**
 * The extraction pipeline a session uses unless a profile names another.
 *
 * The perception compiler is the post-overhaul path: the evaluator describes
 * what was perceived and Rust decides what persists, rather than the model
 * proposing state changes directly.
 */
export const DEFAULT_EVALUATOR_MODE = "evaluator_perception_v2";

/**
 * Whether a turn waits for the previous turn's state to commit.
 *
 * These three move together or not at all. With the evaluator inline, a reader
 * waits out the whole extraction before seeing prose; with it in the background
 * but the gate still closed, they wait for it before the next turn instead; and
 * opening the gate without allowing a stale send just fails the turn outright
 * ("State update in progress and stale send is not allowed").
 *
 * So the default is the non-blocking triple: extract behind the scene, let the
 * next turn go, and narrate from state that may be a turn behind. Waiting is
 * what a benchmark wants, because it needs to attribute state to a turn. It is
 * not what reading wants.
 */
export const DEFAULT_EVALUATOR_BACKGROUND_ENABLED = true;
export const DEFAULT_WAIT_FOR_EVALUATOR_BEFORE_NEXT_TURN = false;
export const DEFAULT_ALLOW_SEND_WITH_STALE_STATE = true;

export function hasAcceptedDisclaimerVersion() {
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

export function loadStoredCustomNarratorPrompt() {
  try {
    return localStorage.getItem(CUSTOM_NARRATOR_PROMPT_STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
}

export function loadStoredBoolean(key: string, fallback: boolean) {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    return raw === "true";
  } catch {
    return fallback;
  }
}

export function loadStoredChatStartMode(): ChatStartMode {
  try {
    const raw = localStorage.getItem(CHAT_START_MODE_STORAGE_KEY);
    return raw === "continue" || raw === "fresh" ? raw : "fresh";
  } catch {
    return "fresh";
  }
}

export function loadStoredSettingsTab(): SettingsTab {
  try {
    const raw = localStorage.getItem(SETTINGS_DRAWER_TAB_STORAGE_KEY);
    if (raw === "chat") {
      return "data";
    }
    return raw === "ai" ||
      raw === "generation" ||
      raw === "reading" ||
      raw === "data" ||
      raw === "about"
      ? raw
      : "ai";
  } catch {
    return "ai";
  }
}

export function clampNumber(value: number, min: number, max: number, fallback = min) {
  if (!Number.isFinite(value)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, value));
}

export function loadStoredGenerationPreferences(): GenerationPreferences {
  try {
    const raw = localStorage.getItem(GENERATION_PREFERENCES_STORAGE_KEY);
    if (!raw) {
      return DEFAULT_GENERATION_PREFERENCES;
    }
    const parsed = JSON.parse(raw) as Partial<GenerationPreferences>;
    const preset: GenerationPresetName =
      parsed.preset === "balanced" ||
      parsed.preset === "creative" ||
      parsed.preset === "focused" ||
      parsed.preset === "custom"
        ? parsed.preset
        : DEFAULT_GENERATION_PREFERENCES.preset;
    return {
      preset,
      temperature: clampNumber(
        Number(parsed.temperature),
        0,
        2,
        DEFAULT_GENERATION_PREFERENCES.temperature,
      ),
      topP: clampNumber(Number(parsed.topP), 0.01, 1, DEFAULT_GENERATION_PREFERENCES.topP),
      frequencyPenalty: clampNumber(
        Number(parsed.frequencyPenalty),
        -2,
        2,
        DEFAULT_GENERATION_PREFERENCES.frequencyPenalty,
      ),
      presencePenalty: clampNumber(
        Number(parsed.presencePenalty),
        -2,
        2,
        DEFAULT_GENERATION_PREFERENCES.presencePenalty,
      ),
      maxTokens:
        parsed.maxTokens === null || parsed.maxTokens === undefined
          ? null
          : Math.round(clampNumber(Number(parsed.maxTokens), 64, 32768, 700)),
      // Floor matches the engine's: below it the brief cannot hold its
      // constraint sections, and the engine ignores the value rather than
      // shipping a prompt that looks complete and is not.
      contextMaxTokens:
        parsed.contextMaxTokens === null || parsed.contextMaxTokens === undefined
          ? null
          : Math.round(clampNumber(Number(parsed.contextMaxTokens), 1200, 128000, 6000)),
    };
  } catch {
    return DEFAULT_GENERATION_PREFERENCES;
  }
}

export function loadStoredReadingPreferences(): ReadingPreferences {
  try {
    const raw = localStorage.getItem(READING_PREFERENCES_STORAGE_KEY);
    if (!raw) {
      return DEFAULT_READING_PREFERENCES;
    }
    const parsed = JSON.parse(raw) as Partial<ReadingPreferences>;
    return {
      proseSize: clampNumber(
        Number(parsed.proseSize),
        15,
        24,
        DEFAULT_READING_PREFERENCES.proseSize,
      ),
      lineHeight: clampNumber(
        Number(parsed.lineHeight),
        1.4,
        2.1,
        DEFAULT_READING_PREFERENCES.lineHeight,
      ),
      columnWidth: clampNumber(
        Number(parsed.columnWidth),
        680,
        1080,
        DEFAULT_READING_PREFERENCES.columnWidth,
      ),
      compactSpacing:
        typeof parsed.compactSpacing === "boolean"
          ? parsed.compactSpacing
          : DEFAULT_READING_PREFERENCES.compactSpacing,
    };
  } catch {
    return DEFAULT_READING_PREFERENCES;
  }
}
