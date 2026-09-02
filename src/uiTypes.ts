// Local UI-only types extracted from App.tsx (Phase 0 refactor).
// These describe front-end view/draft state, distinct from the backend
// data contracts defined in ./tauri.
import {
  ApiProviderSettings,
  BenchmarkSettings,
  BenchmarkTurnSummary,
  ChatMessage,
} from "./tauri";

export type ProviderKind = "Mock" | "API";
export type BenchmarkTurnPhase =
  | "player_generation"
  | "execute_turn"
  | "evaluator_wait"
  | "turn_summary"
  | "completed";
export type NarrativeMode = "Realistic" | "Reader" | "Active Director" | "GM Simulation" | "Custom";
export type AppView = "home" | "library" | "editor" | "chat" | "statemap" | "settings";
export type ChatStartMode = "continue" | "fresh";
export type DisclaimerMode = "launch" | "manual" | null;
export type SettingsTab = "ai" | "chat" | "library" | "dev";
export type DevCommandName =
  | "dedupe_active_adjacent_user_messages"
  | "restore_inactive_messages"
  | "get_branch_patch_debug"
  | "rebuild_session_from_ledger"
  | "inspect_turn_branch_integrity"
  | "repair_accidental_normal_send_variants"
  | "export_visible_chat_log"
  | "export_llm_payload_history"
  | "run_benchmark";

export type ActiveGeneration = {
  id: number;
  conversationId: string;
  narratorSaved: boolean;
  knownAssistantIds: Set<number>;
  replacementAssistantId?: number;
  replacementOriginalContent?: string;
};

// Mutable state for a live AI-vs-AI benchmark run. Held in a ref (not React
// state) so the per-turn effect reads/mutates it without re-render churn.
export type BenchmarkLiveContext = {
  benchmarkId: string;
  conversationId: string;
  soulId: string;
  startedAt: number;
  playerProfileId: string;
  playerGoal: string;
  playerCharacterSoulId: string | null;
  /** Opposing/user side uses the traditional RP engine (full chat, no memory)
   * instead of the player simulator — the comparison-benchmark control. */
  traditionalOpponent: boolean;
  settings: BenchmarkSettings;
  narratorSettings: ApiProviderSettings;
  updaterSettings: ApiProviderSettings;
  initialMemoryCount: number;
  initialObjectCount: number;
  initialRelationshipCount: number;
  relationshipTargetChecked: string;
  initialActivePlayerRelationship: Record<string, unknown> | null;
  perTurn: BenchmarkTurnSummary[];
  narratorFailures: number;
  completedTurns: number;
  nextTurnIndex: number;
  lastPlayerText: string;
};

export interface MessageRenderTrace {
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

export type VisibleBubbleRenderSource =
  | "saved_db"
  | "pending_overlay"
  | "streaming_overlay"
  | "local_optimistic"
  | "unknown";

export interface VisibleBubbleTraceRow {
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

export type PsychePresetName =
  | "Stranger"
  | "Traumatized Survivor"
  | "Trusting Friend"
  | "Devoted Partner"
  | "Hostile Rival"
  | "Custom";

export type PsycheDraft = {
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

export type WorldDraft = {
  location: string;
  activePlots: string;
  keyObjects: string;
  timeElapsed: string;
};
