export type Relationship = {
  trust: number;
  affection: number;
  intimacy: number;
  passion: number;
  commitment: number;
  fear: number;
  desire: number;
  love_type: string;
};

export type RecentMemory = {
  id: string;
  timestamp: number;
  content: string;
  salience: number;
  tag: string;
  retrieval_strength: number;
  source_type?: string;
  source_session_id?: string | null;
  source_conversation_id?: string | null;
  source_message_id?: number | null;
  source_entity_id?: string | null;
  is_lived_experience?: boolean;
  is_imported_context?: boolean;
  perceived_by_entity_id?: string | null;
  target_entity_ids?: string[];
  interpretation?: string | null;
  confidence?: number | null;
  objective_event_id?: string | null;
  truth_status?:
    | "fiction"
    | "scene_event"
    | "character_belief"
    | "narrator_claim"
    | "user_claimed"
    | "verified_engine"
    | "actual_system_event"
    | "unknown";
  is_pinned?: boolean;
  is_active?: boolean;
  archived?: boolean;
  architecture_verified?: boolean;
};

export type SchemaMemory = {
  schema_type: string;
  summary: string;
  count: number;
  schema_id?: string;
  owner_soul_id?: string | null;
  target_entity_ids?: string[];
  trigger_tags?: string[];
  salience?: number;
  reinforcement_count?: number;
  decay?: number;
  last_reinforced_turn?: number;
};

export type PlotEntry = {
  plot_id: string;
  title: string;
  summary?: string;
  status?: "dominant" | "background" | "resolved" | "stale" | "unknown" | string;
  salience?: number;
  started_turn?: number;
  last_touched_turn?: number;
  related_entities?: string[];
  related_world_id?: string | null;
  unresolved_questions?: string[];
  resolution_summary?: string | null;
};

export type WorldState = {
  location: string;
  active_plots: string[];
  recent_events: string[];
  key_objects: string[];
  time_elapsed: string;
  dominant_current_plot?: PlotEntry | null;
  background_plots?: PlotEntry[];
  resolved_plots?: PlotEntry[];
  stale_plot_decay?: number;
};

export type Soul = {
  schema_version: number;
  character_id: string;
  character_name: string;
  soul_kind?: "savepoint" | "session_clone" | "imported_package" | "checkpoint" | string;
  source_soul_id?: string | null;
  source_savepoint_id?: string | null;
  created_from_name?: string | null;
  profile: {
    description: string;
    appearance: string;
    personality: string;
    scenario: string;
    opening_narrator_message?: string;
    avatar_image_id?: string | null;
  };
  last_updated: number;
  turn_counter: number;
  turns_since_consolidation: number;
  global: {
    dev_stage: number;
    attach_style: number;
    fear_baseline: number;
    resolve: number;
    shame: number;
    openness: number;
    maslow: number[];
    sdt: number[];
  };
  trauma: {
    phase: number;
    symptoms: Record<string, number>;
  };
  relationships: Record<string, Relationship>;
  arousal: {
    body_sex: "Male" | "Female";
    phase: "Neutral" | "Aware" | "Warm" | "Ready" | "Plateau" | "Peak" | "Orgasm";
    level: number;
    frustration: number;
    sensitivity: number;
    refractory_turns_remaining: number;
    orgasm_count: number;
    denied_peak_turns: number;
  };
  memory: {
    core: string[];
    recent: RecentMemory[];
    schemas: SchemaMemory[];
  };
  debug_memory_layer_replies?: Array<{
    nonce: string;
    content: string;
    created_at: number;
    architecture_verified: boolean;
  }>;
  world: WorldState;
};

export type SettingSoul = {
  schema_version: number;
  setting_id: string;
  setting_name: string;
  scenario?: string;
  last_updated: number;
  turn_counter: number;
  world: Soul["world"];
};

export type SoulSummary = {
  character_id: string;
  character_name: string;
  soul_kind: string;
  source_soul_id: string | null;
  source_savepoint_id: string | null;
  avatar_image_id?: string | null;
  last_updated: number;
  recent_count: number;
  core_count: number;
  archived_at?: number | null;
};

export type ConversationSummary = {
  conversation_id: string;
  title: string;
  soul_id: string;
  source_savepoint_id: string | null;
  world_id?: string | null;
  source_setting_id?: string | null;
  active_player_persona_id: string;
  created_at: number;
  updated_at: number;
  last_message_preview: string | null;
  message_count: number;
  archived_at?: number | null;
  active_evaluator_profile_id?: string | null;
  is_benchmark?: boolean;
};

export type SessionStateHubItem = {
  conversation: ConversationSummary;
  soul_name: string;
  setting_name: string;
  location: string;
  time_elapsed: string;
  current_scene: string;
  focus: string;
  turn_counter: number;
  memory_count: number;
  core_memory_count: number;
  recent_memory_count: number;
  schema_count: number;
  relationship_count: number;
  positive_relationship_count: number;
  object_count: number;
  event_count: number;
  active_plot_count: number;
};

export type StateMapSceneItem = {
  session_id: string;
  session_title: string;
  soul_name: string;
  setting_name: string;
  turn_counter: number;
  location: string;
  time_elapsed: string;
  current_scene: string;
  focus: string;
  last_user_action: string;
  pressure_point: string;
};

export type StateMapCharacterItem = {
  session_id: string;
  session_title: string;
  name: string;
  role: string;
  detail: string;
};

export type StateMapRelationshipItem = {
  session_id: string;
  session_title: string;
  soul_name: string;
  target: string;
  love_type: string;
  trust: number;
  affection: number;
  intimacy: number;
  fear: number;
  desire: number;
};

export type StateMapObjectItem = {
  session_id: string;
  session_title: string;
  name: string;
  kind: string;
  owner: string;
  location: string;
  status: string;
  summary: string;
  confidence: number;
};

export type StateMapTimelineItem = {
  session_id: string;
  session_title: string;
  turn_counter: number;
  content: string;
};

export type StateMapMemoryItem = {
  session_id: string;
  session_title: string;
  soul_name: string;
  content: string;
  tag: string;
  source_turn: number | null;
  confidence: number | null;
  truth_status: string;
  source_type: string;
  is_pinned: boolean;
  is_active: boolean;
};

export type StateMapMemoryV2Item = {
  session_id: string;
  session_title: string;
  memory_id: string;
  layer: "raw" | "derived" | string;
  memory_kind: string;
  validity: "valid" | "stale" | "superseded" | "invalidated" | string;
  content: string;
  confidence: number;
  truth_status: string;
  source_patch_id: string | null;
  source_turn_id: string | null;
  source_quote: string | null;
  source_memory_ids: string[];
  supporting_evidence_count: number;
  contradicting_evidence_count: number;
};

export type SessionStateMap = {
  sessions: SessionStateHubItem[];
  scenes: StateMapSceneItem[];
  characters: StateMapCharacterItem[];
  relationships: StateMapRelationshipItem[];
  objects: StateMapObjectItem[];
  timeline: StateMapTimelineItem[];
  memories: StateMapMemoryItem[];
  memory_v2: StateMapMemoryV2Item[];
};

export type PlayerPersona = {
  persona_id: string;
  display_name: string;
  description: string;
  gender_code: string;
  pronouns: string;
  is_builtin: boolean;
  is_archived: boolean;
  created_at: number;
  updated_at: number;
  appearance?: string | null;
  voice_style?: string | null;
  boundaries?: string | null;
  notes?: string | null;
};

export type PlayerPersonaInput = {
  persona_id?: string | null;
  display_name: string;
  description: string;
  gender_code: string;
  pronouns: string;
  appearance?: string | null;
  voice_style?: string | null;
  boundaries?: string | null;
  notes?: string | null;
};

export type RestoreTurnsPreview = {
  restored_message_ids: number[];
  skipped_duplicate_ids: number[];
  skipped_pending_ids: number[];
  skipped_failed_ids: number[];
  skipped_retry_attempt_ids: number[];
  skipped_regenerated_discarded_ids: number[];
};

export type RestoreTurnsResult = {
  messages: ChatMessage[];
  preview: RestoreTurnsPreview;
};

export type DedupeAdjacentUserMessagesResult = {
  canonical_user_message_ids: number[];
  hidden_duplicate_user_message_ids: number[];
};

export type MneBundleManifest = {
  mne_version: number;
  bundle_id: string;
  bundle_type: "character_soul" | "world_setting" | "scenario_bundle" | "session_checkpoint" | string;
  title: string;
  description: string;
  author?: string | null;
  created_at: number;
  app: string;
  schema_version: number;
  conversation_id?: string | null;
  soul_id?: string | null;
  world_id?: string | null;
  source_savepoint_id?: string | null;
  source_setting_id?: string | null;
  contents: {
    souls: string[];
    worlds: string[];
    images: string[];
    conversation?: string | null;
  };
};

export type MneExportResult = {
  path: string;
  manifest: MneBundleManifest;
};

export type MneImportResult = {
  bundle_id: string;
  bundle_type: string;
  imported_soul_ids: string[];
  imported_setting_ids: string[];
  remapped_ids: Record<string, string>;
  summary: string;
};

export type MneValidationSummary = {
  soul_name?: string | null;
  soul_id?: string | null;
  world_name?: string | null;
  world_id?: string | null;
  conversation_title?: string | null;
  conversation_id?: string | null;
  message_count: number;
  memory_count: number;
  recent_event_count: number;
  object_state_count: number;
  relationship_count: number;
  payload_log_count: number;
};

export type MneValidationReport = {
  valid: boolean;
  errors: string[];
  warnings: string[];
  summary: MneValidationSummary;
};

export type SessionStartResult = {
  soul: Soul;
  conversation: ConversationSummary;
  messages: ChatMessage[];
};

export type SettingSummary = {
  setting_id: string;
  setting_name: string;
  last_updated: number;
  turn_counter: number;
  location: string;
  archived_at?: number | null;
};

export type ChatMessage = {
  id: number;
  conversation_id: string;
  role: "user" | "assistant" | "system";
  content: string;
  created_at: number;
  channel?: string;
  status?: "active" | "hidden" | "pending" | "failed" | "retry_attempt" | "regenerated_discarded" | string;
  origin?: "active" | "restored" | string;
  attachments?: MessageAttachment[];
  pending?: boolean;
  request_id?: string | null;
  generation_id?: number | null;
  assistant_message_id?: number | null;
  turn_id?: string | null;
};

export type ImageAsset = {
  id: string;
  file_path: string;
  thumbnail_path?: string | null;
  source: "uploaded" | "generated" | "imported" | string;
  mime_type?: string | null;
  width?: number | null;
  height?: number | null;
  prompt?: string | null;
  provider?: string | null;
  model?: string | null;
  linked_soul_id?: string | null;
  linked_conversation_id?: string | null;
  linked_message_id?: number | null;
  created_at: number;
};

export type MessageAttachment = {
  id: number;
  message_id: number;
  image_asset_id: string;
  created_at: number;
  image: ImageAsset;
};

export type DevLogLevel = "info" | "warn" | "error" | "debug" | "success";
export type DevLogCategory =
  | "app"
  | "db"
  | "api"
  | "narrator"
  | "state_updater"
  | "context"
  | "stream"
  | "performance"
  | "error"
  | "warning"
  | "success";

export type DevLogEntry = {
  id: string;
  timestamp: number;
  level: DevLogLevel;
  category: DevLogCategory;
  message: string;
  details?: Record<string, unknown> | null;
};

export type AssistantMessageVariant = {
  id: number | null;
  message_id: number;
  conversation_id: string;
  content: string;
  created_at: number;
  label: string | null;
  source: string | null;
  is_selected: boolean;
  soul_snapshot_json: string | null;
  debug_json: string | null;
};

export type VariantSelectionResult = {
  variants: AssistantMessageVariant[];
  messages: ChatMessage[];
};

export type TurnResult = {
  conversation_id: string;
  soul: Soul;
  visible_response: string;
  context_preview: ContextPreview;
  messages: ChatMessage[];
  consolidation_ran: boolean;
  debug: TurnDebug;
};

export type TurnDebug = {
  provider: string;
  hidden_state_found: boolean;
  fallback_hidden_state_generated: boolean;
  narrator_response_saved: boolean;
  assistant_message_id: number | null;
  selected_variant_id: number | null;
  state_updater_status: string;
  replay_detected: boolean;
  replay_score: number;
  replay_reason: string | null;
  replay_compared_against_message_id: number | null;
  output_contract_warning: string | null;
  tag: string | null;
  trust_delta: number | null;
  affection_delta: number | null;
  new_location: string | null;
  present_characters: string[];
  request_id?: string | null;
  turn_id?: string | null;
  state_patch_id?: string | null;
  simulated_response?: boolean;
  fallback_used?: boolean;
  fallback_reason?: string | null;
};

export type ContextPreview = {
  text: string;
  estimated_tokens: number;
  truncated: boolean;
  memory_slot_debug?: Array<{
    slot: string;
    memory_id: string;
    action: string;
    reason: string;
    source_type: string;
    truth_status: string;
    entity_match: boolean;
    plot_match: boolean;
    salience: number;
    final_score: number;
  }>;
};

export type MemorySlotTrace = NonNullable<ContextPreview["memory_slot_debug"]>[number];

export type LlmPayloadTokenEstimate = {
  system: number;
  context: number;
  user: number;
  total: number;
};

export type LlmPayloadPreview = {
  provider: string;
  mode: string;
  context_mode: ContextMode;
  custom_prompt_status: string;
  model: string;
  base_url: string;
  system_message: string;
  user_message: string;
  context: string;
  messages: ApiPayloadMessage[];
  truncated: boolean;
  estimated_tokens: LlmPayloadTokenEstimate;
  memory_slot_debug?: MemorySlotTrace[];
};

export type ApiPayloadMessage = {
  role: string;
  content: string;
};

export type LlmPayloadLog = {
  id: number;
  conversation_id: string;
  message_id: number | null;
  provider: string;
  mode: string;
  context_mode: string;
  model: string;
  base_url: string;
  system_message: string;
  user_message: string;
  context_text: string;
  estimated_system_tokens: number;
  estimated_user_tokens: number;
  estimated_total_tokens: number;
  truncated: boolean;
  created_at: number;
  branch_id?: string | null;
  active_turn_id?: string | null;
  parent_turn_id?: string | null;
  state_patch_ids_applied?: string[];
  discarded_patch_ids_skipped?: string[];
  state_rebuild_generation?: number | null;
  latest_assistant_variant_id?: number | null;
  request_id?: string | null;
  turn_id?: string | null;
  raw_provider_response?: string | null;
  normalized_response?: string | null;
  finish_reason?: string | null;
  provider_error?: string | null;
  fallback_used?: boolean;
  fallback_reason?: string | null;
  provider_request_id?: string | null;
  provider_response_id?: string | null;
  pipeline_trace_json?: string | null;
};

export type BranchPatchDebug = {
  branch_id: string;
  active_turn_id?: string | null;
  rebuild_generation: number;
  applied_patches: string[];
  skipped_discarded_patches: string[];
  invalidated_patches: string[];
};

export type ExportResult = {
  path: string;
  message: string;
};

export type ApiProviderSettings = {
  base_url: string;
  api_key: string;
  model: string;
  system_prompt: string;
  narrator_temperature?: number | null;
  narrator_max_tokens?: number | null;
  narrator_top_p?: number | null;
  narrator_frequency_penalty?: number | null;
  narrator_presence_penalty?: number | null;
  narrator_timeout_ms?: number | null;
  evaluator_timeout_ms?: number | null;
  structured_evaluator_timeout_ms?: number | null;
  diagnostic_evaluator_timeout_ms?: number | null;
  evaluator_timeout_mode?: "finite" | "no_app_timeout" | string | null;
  evaluator_mode?:
    | "evaluator_v1"
    | "evaluator_form_v1"
    | "evaluator_structured_v1"
    | "evaluator_perception_v2"
    | "dual_compare"
    | string
    | null;
  structured_evaluator_policy?: "required" | "prefer" | "allow_fallback" | string | null;
  /** Evaluator transport: "auto" tries real tool-calling first, then the response_format ladder. */
  structured_evaluator_transport?: "auto" | "tool_call" | "json_schema" | "json_object" | "prompt_json" | string | null;
  structured_evaluator_max_retries?: number | null;
  wait_for_evaluator_before_next_turn?: boolean | null;
  allow_send_with_stale_state?: boolean | null;
  evaluator_background_enabled?: boolean | null;
  anti_replay_forced_retry_enabled?: boolean | null;
  /** "fast" skips the evaluator on dialogue-only turns (catch-up later); missing/unknown = "balanced". */
  evaluator_execution_mode?: "fast" | "balanced" | "long_context" | string | null;
};

export type ContextMode = "brief" | "full_chat";

export type ProviderProfile = ApiProviderSettings & {
  id: string;
  name: string;
  created_at: number;
  updated_at: number;
  archived_at?: number | null;
  narrator_compatibility_status: number;
  evaluator_compatibility_status: number;
  command_compatibility_status: number;
  evaluator_contract_version: number;
  evaluator_prompt_version: number;
  evaluator_last_tested_at?: number | null;
  evaluator_last_failure_reason?: string | null;
  /** 0 untested/failed, 1 prompt-only, 2 json_object, 3 json_schema. */
  structured_output_support?: number;
};

export type EvaluatorJobStatus =
  | "pending"
  | "running"
  | "completed"
  | "partial_success"
  | "failed"
  | "canceled"
  | "timed_out"
  | string;

export type EvaluatorJob = {
  evaluator_job_id: string;
  conversation_id: string;
  turn_id: string;
  assistant_message_id: number;
  status: EvaluatorJobStatus;
  started_at: number;
  completed_at?: number | null;
  elapsed_ms?: number | null;
  timeout_ms?: number | null;
  timeout_mode: string;
  model?: string | null;
  provider?: string | null;
  error_message?: string | null;
  patch_applied: boolean;
};
