import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  Relationship,
  RecentMemory,
  SchemaMemory,
  PlotEntry,
  WorldState,
  Soul,
  SettingSoul,
  SoulSummary,
  ConversationSummary,
  SessionStateHubItem,
  StateMapSceneItem,
  StateMapCharacterItem,
  StateMapRelationshipItem,
  StateMapObjectItem,
  StateMapTimelineItem,
  StateMapMemoryItem,
  SessionStateMap,
  PlayerPersona,
  PlayerPersonaInput,
  RestoreTurnsPreview,
  RestoreTurnsResult,
  DedupeAdjacentUserMessagesResult,
  MneBundleManifest,
  MneExportResult,
  MneImportResult,
  MneValidationSummary,
  MneValidationReport,
  SessionStartResult,
  SettingSummary,
  ChatMessage,
  ImageAsset,
  MessageAttachment,
  DevLogLevel,
  DevLogCategory,
  DevLogEntry,
  AssistantMessageVariant,
  VariantSelectionResult,
  TurnResult,
  TurnDebug,
  ContextPreview,
  MemorySlotTrace,
  LlmPayloadTokenEstimate,
  LlmPayloadPreview,
  ApiPayloadMessage,
  LlmPayloadLog,
  BranchPatchDebug,
  ExportResult,
  ApiProviderSettings,
  ContextMode,
  ProviderProfile,
  EvaluatorJobStatus,
  EvaluatorJob,
} from "./tauri/contracts";
export * from "./tauri/contracts";

import {
  BUILT_IN_PLAYER_PERSONAS,
  previewState,
  assistantRecentPreviewExcerpt,
  buildNarratorSystemPrompt,
  buildUserTextWithCorrection,
  chatCompletionsUrl,
  classifyPreviewTag,
  compilePreviewContext,
  consolidatePreviewSoul,
  customPromptStatusFor,
  debugFromHiddenState,
  deepClone,
  deletePreviewAssistantVariant,
  downloadPreviewExport,
  ensurePreviewBaseVariant,
  ensurePreviewConversation,
  estimateTokens,
  excerptText,
  fileToBase64,
  generatedPreviewApiHiddenState,
  headTailExcerptText,
  hydratePreviewMessage,
  inferPreviewMemorySource,
  isGenericFillerMemoryText,
  isNearEmptyGenericSchema,
  listPreviewAssistantVariants,
  makeFreshPreviewSoul,
  makePreviewContextMessage,
  makePreviewImageAsset,
  makePreviewMessage,
  makePreviewMneExport,
  makePreviewSetting,
  makePreviewSoul,
  makePreviewVariant,
  mimeFromPreviewPath,
  modePromptFor,
  normalizeAssistantDisplayForExport,
  normalizePreviewTag,
  normalizeTimeElapsedForPreview,
  parsePreviewHiddenState,
  previewConversationIdForSoul,
  previewTemplateFor,
  renderPreviewPayloadHistory,
  renderPreviewResponse,
  renderPreviewVisibleChatLog,
  sanitizeAssistantPreview,
  sanitizePreviewConversationTitle,
  selectPreviewAssistantVariant,
  sendPreviewApiTurn,
  sendPreviewTurn,
  stripHiddenPreview,
  summarizePreviewConversation,
  summarizeSetting,
  summarizeSoul,
  tailExcerptText,
  upsertPreviewAssistantMessage,
} from "./tauri/previewRuntime";

function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invokeOrPreview<T>(
  command: string,
  args: Record<string, unknown>,
  fallback: () => T | Promise<T>,
): Promise<T> {
  if (hasTauriRuntime()) {
    return invoke<T>(command, args);
  }
  return fallback();
}

export function createDefaultSoul(characterName: string): Promise<Soul> {
  return invokeOrPreview("create_default_soul", { characterName }, () =>
    makePreviewSoul(characterName),
  );
}

export function createSessionSoulClone(
  soulId: string,
  settingId?: string,
  title?: string,
): Promise<SessionStartResult> {
  return invokeOrPreview(
    "create_session_soul_from_savepoint",
    { sourceSoulId: soulId, settingId: settingId ?? null, title: title ?? null },
    () => {
      const base = previewState.souls.find((item) => item.character_id === soulId);
      if (!base) throw new Error("Soul not found");
      const fresh = makeFreshPreviewSoul(base);
      previewState.souls.unshift(fresh);
      const resolvedConversationId = settingId
        ? `local-mock-${settingId}-${fresh.character_id}`
        : previewConversationIdForSoul(fresh.character_id);
      const conversation = ensurePreviewConversation(
        resolvedConversationId,
        fresh.character_id,
        title?.trim() || `${base.character_name} Session`,
      );
      const opening = fresh.profile.opening_narrator_message?.trim();
      if (opening && !previewState.messages.some((message) => message.conversation_id === resolvedConversationId)) {
        previewState.messages.push(makePreviewMessage(resolvedConversationId, "assistant", opening));
      }
      return {
        soul: fresh,
        conversation: summarizePreviewConversation(conversation),
        messages: previewState.messages.filter((message) => message.conversation_id === resolvedConversationId),
      };
    },
  );
}

export const createFreshScenarioSoul = createSessionSoulClone;

export function saveSessionAsNewSoul(
  sessionSoulId: string,
  name: string,
  soulKind: "savepoint" | "checkpoint" = "checkpoint",
): Promise<Soul> {
  return invokeOrPreview(
    "save_session_as_new_soul",
    { sessionSoulId, name, soulKind },
    () => {
      const session = previewState.souls.find((item) => item.character_id === sessionSoulId);
      if (!session) throw new Error("Soul not found");
      const savepoint: Soul = {
        ...deepClone(session),
        character_id: crypto.randomUUID(),
        character_name: name.trim() || `${session.character_name} Checkpoint`,
        soul_kind: soulKind,
        source_soul_id: session.character_id,
        source_savepoint_id: session.source_savepoint_id ?? null,
        created_from_name: session.character_name,
        last_updated: Math.floor(Date.now() / 1000),
      };
      previewState.souls.unshift(savepoint);
      return savepoint;
    },
  );
}

export function createDefaultSetting(settingName: string): Promise<SettingSoul> {
  return invokeOrPreview("create_default_setting", { settingName }, () =>
    makePreviewSetting(settingName),
  );
}

export function listSouls(): Promise<SoulSummary[]> {
  return invokeOrPreview("list_souls", {}, () =>
    previewState.souls.filter((soul) => soul.soul_kind !== "session_clone" && !(soul as any).archived_at).map(summarizeSoul),
  );
}

export function listSoulsDebug(): Promise<SoulSummary[]> {
  return invokeOrPreview("list_souls_debug", {}, () =>
    previewState.souls.filter((soul) => !(soul as any).archived_at).map(summarizeSoul),
  );
}

export function listConversations(): Promise<ConversationSummary[]> {
  return invokeOrPreview("list_conversations", {}, () =>
    [...previewState.conversations]
      .filter((conversation) => !(conversation as any).archived_at)
      .sort((a, b) => b.updated_at - a.updated_at || b.created_at - a.created_at)
      .map(summarizePreviewConversation),
  );
}

export function touchConversationAccess(conversationId: string): Promise<ConversationSummary> {
  return invokeOrPreview("touch_conversation_access", { conversationId }, () => {
    const conversation = previewState.conversations.find((item) => item.conversation_id === conversationId);
    if (!conversation || (conversation as any).archived_at) {
      throw new Error(`Conversation not found: ${conversationId}`);
    }
    conversation.updated_at = Math.floor(Date.now() / 1000);
    previewState.conversations = [
      conversation,
      ...previewState.conversations.filter((item) => item.conversation_id !== conversationId),
    ];
    return summarizePreviewConversation(conversation);
  });
}

export function listSessionStateHub(): Promise<SessionStateHubItem[]> {
  return invokeOrPreview("list_session_state_hub", {}, () =>
    previewState.conversations.map((conversation) => {
      const summary = summarizePreviewConversation(conversation);
      const soul = previewState.souls.find((item) => item.character_id === summary.soul_id);
      const setting = previewState.settings.find((item) => item.setting_id === summary.source_setting_id);
      const world = soul?.world as (WorldState & { scene_state?: { current_scene?: string; focus?: string }; object_states?: unknown[] }) | undefined;
      const relationships = soul ? Object.values(soul.relationships ?? {}) : [];
      return {
        conversation: summary,
        soul_name: soul?.character_name ?? "Unknown soul",
        setting_name: setting?.setting_name ?? "Unknown setting",
        location: world?.location ?? "",
        time_elapsed: world?.time_elapsed ?? "",
        current_scene: world?.scene_state?.current_scene ?? "",
        focus: world?.scene_state?.focus ?? "",
        turn_counter: soul?.turn_counter ?? summary.message_count,
        memory_count: (soul?.memory.core.length ?? 0) + (soul?.memory.recent.length ?? 0) + (soul?.memory.schemas.length ?? 0),
        core_memory_count: soul?.memory.core.length ?? 0,
        recent_memory_count: soul?.memory.recent.length ?? 0,
        schema_count: soul?.memory.schemas.length ?? 0,
        relationship_count: relationships.length,
        positive_relationship_count: relationships.filter(
          (relationship) =>
            relationship.trust > 35 ||
            relationship.affection > 35 ||
            relationship.intimacy > 35 ||
            relationship.desire > 35,
        ).length,
        object_count: Math.max(world?.key_objects.length ?? 0, world?.object_states?.length ?? 0),
        event_count: world?.recent_events.length ?? 0,
        active_plot_count: world?.active_plots.length ?? 0,
      };
    }),
  );
}

export function listSessionStateMap(): Promise<SessionStateMap> {
  return invokeOrPreview("list_session_state_map", {}, async () => {
    const sessions = await listSessionStateHub();
    return {
      sessions,
      scenes: sessions.map((item) => ({
        session_id: item.conversation.conversation_id,
        session_title: item.conversation.title || "Untitled session",
        soul_name: item.soul_name,
        setting_name: item.setting_name,
        turn_counter: item.turn_counter,
        location: item.location,
        time_elapsed: item.time_elapsed,
        current_scene: item.current_scene,
        focus: item.focus,
        last_user_action: item.conversation.last_message_preview ?? "",
        pressure_point: item.focus,
      })),
      characters: sessions.map((item) => ({
        session_id: item.conversation.conversation_id,
        session_title: item.conversation.title || "Untitled session",
        name: item.soul_name,
        role: "session soul",
        detail: `${item.memory_count} memories / ${item.relationship_count} relationships`,
      })),
      relationships: [],
      objects: [],
      timeline: sessions
        .filter((item) => item.conversation.last_message_preview)
        .map((item) => ({
          session_id: item.conversation.conversation_id,
          session_title: item.conversation.title || "Untitled session",
          turn_counter: item.turn_counter,
          content: item.conversation.last_message_preview ?? "",
        })),
      memories: sessions.map((item) => ({
        session_id: item.conversation.conversation_id,
        session_title: item.conversation.title || "Untitled session",
        soul_name: item.soul_name,
        content: item.current_scene || item.focus || item.conversation.last_message_preview || "No memory preview yet.",
        tag: "session",
        source_turn: null,
        confidence: null,
        truth_status: "preview",
        source_type: "preview",
        is_pinned: false,
        is_active: true,
      })),
      memory_v2: [],
    };
  });
}

export function listPlayerPersonas(): Promise<PlayerPersona[]> {
  return invokeOrPreview("list_player_personas", {}, () =>
    [...BUILT_IN_PLAYER_PERSONAS, ...previewState.playerPersonas.filter((persona) => !persona.is_archived)],
  );
}

export function listArchivedPlayerPersonas(): Promise<PlayerPersona[]> {
  return invokeOrPreview("list_archived_player_personas", {}, () =>
    previewState.playerPersonas.filter((persona) => persona.is_archived),
  );
}

export function getActivePlayerPersona(conversationId: string): Promise<PlayerPersona> {
  return invokeOrPreview("get_active_player_persona", { conversationId }, () => {
    const conversation = previewState.conversations.find((item) => item.conversation_id === conversationId);
    const personaId = conversation?.active_player_persona_id ?? "preset_male";
    return (
      [...BUILT_IN_PLAYER_PERSONAS, ...previewState.playerPersonas].find(
        (persona) => persona.persona_id === personaId,
      ) ?? BUILT_IN_PLAYER_PERSONAS[0]
    );
  });
}

export function setActivePlayerPersona(
  conversationId: string,
  personaId: string,
): Promise<PlayerPersona> {
  return invokeOrPreview("set_active_player_persona", { conversationId, personaId }, () => {
    const persona = [...BUILT_IN_PLAYER_PERSONAS, ...previewState.playerPersonas].find(
      (item) => item.persona_id === personaId && !item.is_archived,
    );
    if (!persona) throw new Error(`Persona not found: ${personaId}`);
    const conversation = ensurePreviewConversation(conversationId, "", undefined);
    conversation.active_player_persona_id = persona.persona_id;
    conversation.updated_at = Math.floor(Date.now() / 1000);
    return persona;
  });
}

export function upsertPlayerPersona(input: PlayerPersonaInput): Promise<PlayerPersona> {
  return invokeOrPreview("upsert_player_persona", { input }, () => {
    const now = Math.floor(Date.now() / 1000);
    const personaId =
      input.persona_id?.trim() ||
      `persona_${input.display_name
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "_")
        .replace(/^_+|_+$/g, "") || now}`;
    if (BUILT_IN_PLAYER_PERSONAS.some((persona) => persona.persona_id === personaId)) {
      throw new Error("Built-in personas cannot be edited.");
    }
    const persona: PlayerPersona = {
      persona_id: personaId,
      display_name: input.display_name.trim(),
      description: input.description.trim(),
      gender_code: input.gender_code.trim(),
      pronouns: input.pronouns.trim(),
      appearance: input.appearance?.trim() || null,
      voice_style: input.voice_style?.trim() || null,
      boundaries: input.boundaries?.trim() || null,
      notes: input.notes?.trim() || null,
      is_builtin: false,
      is_archived: false,
      created_at: now,
      updated_at: now,
    };
    const index = previewState.playerPersonas.findIndex((item) => item.persona_id === personaId);
    if (index >= 0) previewState.playerPersonas[index] = persona;
    else previewState.playerPersonas.push(persona);
    return persona;
  });
}

export function archivePlayerPersona(personaId: string): Promise<boolean> {
  return invokeOrPreview("archive_player_persona", { personaId }, () => {
    const persona = previewState.playerPersonas.find((item) => item.persona_id === personaId);
    if (!persona) return false;
    persona.is_archived = true;
    persona.updated_at = Math.floor(Date.now() / 1000);
    return true;
  });
}

export function restorePlayerPersona(personaId: string): Promise<boolean> {
  return invokeOrPreview("restore_player_persona", { personaId }, () => {
    const persona = previewState.playerPersonas.find((item) => item.persona_id === personaId);
    if (!persona) return false;
    persona.is_archived = false;
    persona.updated_at = Math.floor(Date.now() / 1000);
    return true;
  });
}

export function listArchivedSessions(): Promise<ConversationSummary[]> {
  return invokeOrPreview("list_archived_sessions", {}, () =>
    previewState.conversations.filter(c => c.title.startsWith("[Archived] ") || (c as any).archived_at).map(summarizePreviewConversation),
  );
}

export function renameConversation(
  conversationId: string,
  title: string,
  soulId?: string | null,
): Promise<ConversationSummary> {
  return invokeOrPreview("rename_conversation", { conversationId, title, soulId: soulId ?? null }, () => {
    const conversation = ensurePreviewConversation(conversationId, soulId ?? "", title);
    conversation.title = sanitizePreviewConversationTitle(title);
    conversation.updated_at = Math.floor(Date.now() / 1000);
    return summarizePreviewConversation(conversation);
  });
}

export function listSettings(): Promise<SettingSummary[]> {
  return invokeOrPreview("list_settings", {}, () =>
    previewState.settings.filter((item) => !(item as any).archived_at).map(summarizeSetting),
  );
}

export function upsertSoul(soul: Soul): Promise<SoulSummary> {
  return invokeOrPreview("upsert_soul", { soul }, () => {
    const index = previewState.souls.findIndex((item) => item.character_id === soul.character_id);
    if (index >= 0) {
      previewState.souls[index] = soul;
    } else {
      previewState.souls.unshift(soul);
    }
    return summarizeSoul(soul);
  });
}

export function upsertSetting(setting: SettingSoul): Promise<SettingSummary> {
  return invokeOrPreview("upsert_setting", { setting }, () => {
    const index = previewState.settings.findIndex((item) => item.setting_id === setting.setting_id);
    if (index >= 0) {
      previewState.settings[index] = setting;
    } else {
      previewState.settings.unshift(setting);
    }
    return summarizeSetting(setting);
  });
}

export function getSoul(soulId: string): Promise<Soul> {
  return invokeOrPreview("get_soul", { soulId }, () => {
    const soul = previewState.souls.find((item) => item.character_id === soulId);
    if (!soul) throw new Error("Soul not found");
    return soul;
  });
}

function updatePreviewSoul(soul: Soul): Soul {
  const index = previewState.souls.findIndex((item) => item.character_id === soul.character_id);
  if (index >= 0) previewState.souls[index] = soul;
  return soul;
}

export function clearSoulWorldState(soulId: string): Promise<Soul> {
  return invokeOrPreview("clear_soul_world_state", { soulId }, () => {
    const soul = getPreviewSoulForRepair(soulId);
    soul.world = {
      location: "Unspecified starting scene.",
      active_plots: ["Establish the first scene"],
      recent_events: [],
      key_objects: [],
      time_elapsed: "Session start",
    };
    return updatePreviewSoul(soul);
  });
}

export function clearSoulProfileScenario(soulId: string): Promise<Soul> {
  return invokeOrPreview("clear_soul_profile_scenario", { soulId }, () => {
    const soul = getPreviewSoulForRepair(soulId);
    soul.profile.scenario = "";
    return updatePreviewSoul(soul);
  });
}

export function clearSoulRecentEvents(soulId: string): Promise<Soul> {
  return invokeOrPreview("clear_soul_recent_events", { soulId }, () => {
    const soul = getPreviewSoulForRepair(soulId);
    soul.world.recent_events = [];
    return updatePreviewSoul(soul);
  });
}

export function clearSoulMemories(soulId: string): Promise<Soul> {
  return invokeOrPreview("clear_soul_memories", { soulId }, () => {
    const soul = getPreviewSoulForRepair(soulId);
    soul.memory = { core: [], recent: [], schemas: [] };
    return updatePreviewSoul(soul);
  });
}

function getPreviewSoulForRepair(soulId: string): Soul {
  const soul = previewState.souls.find((item) => item.character_id === soulId);
  if (!soul) throw new Error("Soul not found");
  return structuredClone(soul);
}

export function getSetting(settingId: string): Promise<SettingSoul> {
  return invokeOrPreview("get_setting", { settingId }, () => {
    const setting = previewState.settings.find((item) => item.setting_id === settingId);
    if (!setting) throw new Error("Setting not found");
    return setting;
  });
}

export function exportCharacterSoulMne(soulId: string, outputPath: string): Promise<MneExportResult> {
  return invokeOrPreview("export_character_soul_mne", { soulId, outputPath }, () => {
    const soul = previewState.souls.find((item) => item.character_id === soulId);
    if (!soul) throw new Error("Soul not found");
    return makePreviewMneExport(outputPath, "character_soul", soul.character_name, [`souls/${soulId}.json`], []);
  });
}

export function exportWorldSettingMne(settingId: string, outputPath: string): Promise<MneExportResult> {
  return invokeOrPreview("export_world_setting_mne", { settingId, outputPath }, () => {
    const setting = previewState.settings.find((item) => item.setting_id === settingId);
    if (!setting) throw new Error("Setting not found");
    return makePreviewMneExport(outputPath, "world_setting", setting.setting_name, [], [`worlds/${settingId}.json`]);
  });
}

export function exportScenarioBundleMne(
  soulId: string,
  worldId: string,
  outputPath: string,
): Promise<MneExportResult> {
  return invokeOrPreview("export_scenario_bundle_mne", { soulId, worldId, outputPath }, () => {
    const soul = previewState.souls.find((item) => item.character_id === soulId);
    const setting = previewState.settings.find((item) => item.setting_id === worldId);
    if (!soul || !setting) throw new Error("Soul or Setting not found");
    return makePreviewMneExport(outputPath, "scenario_bundle", `${soul.character_name} + ${setting.setting_name}`, [`souls/${soulId}.json`], [`worlds/${worldId}.json`]);
  });
}

export function exportCurrentSessionCheckpointMne(
  conversationId: string,
  outputPath: string,
): Promise<MneExportResult> {
  return invokeOrPreview("export_current_session_checkpoint_mne", { conversationId, outputPath }, () =>
    makePreviewMneExport(outputPath, "session_checkpoint", conversationId, [], []),
  );
}

export function importMneBundle(filePath: string): Promise<MneImportResult> {
  return invokeOrPreview("import_mne_bundle", { filePath }, () => ({
    bundle_id: crypto.randomUUID(),
    bundle_type: "preview",
    imported_soul_ids: [],
    imported_setting_ids: [],
    remapped_ids: {},
    summary: "Preview import is only available in the desktop app",
  }));
}

export function validateMneBundle(filePath: string): Promise<MneValidationReport> {
  return invokeOrPreview("validate_mne_bundle", { filePath }, () => ({
    valid: true,
    errors: [],
    warnings: [],
    summary: {
      message_count: 0,
      memory_count: 0,
      recent_event_count: 0,
      object_state_count: 0,
      relationship_count: 0,
      payload_log_count: 0,
    },
  }));
}

export function previewMneImport(filePath: string): Promise<MneValidationReport> {
  return invokeOrPreview("preview_mne_import", { filePath }, () => ({
    valid: true,
    errors: [],
    warnings: [],
    summary: {
      message_count: 0,
      memory_count: 0,
      recent_event_count: 0,
      object_state_count: 0,
      relationship_count: 0,
      payload_log_count: 0,
    },
  }));
}

export function importMneAsNew(filePath: string): Promise<MneImportResult> {
  return invokeOrPreview("import_mne_as_new", { filePath }, () => ({
    bundle_id: crypto.randomUUID(),
    bundle_type: "preview",
    imported_soul_ids: [],
    imported_setting_ids: [],
    remapped_ids: {},
    summary: "Mock import as new copy",
  }));
}

export function archiveSoul(soulId: string): Promise<boolean> {
  return invokeOrPreview("archive_soul", { soulId }, () => {
    const soul = previewState.souls.find((item) => item.character_id === soulId);
    if (!soul) return false;
    (soul as any).archived_at = Math.floor(Date.now() / 1000);
    return true;
  });
}

export function purgeSoul(soulId: string): Promise<boolean> {
  return invokeOrPreview("purge_soul", { soulId }, () => {
    const index = previewState.souls.findIndex((item) => item.character_id === soulId);
    if (index === -1) return false;
    previewState.souls.splice(index, 1);
    return true;
  });
}

export function restoreSoul(soulId: string): Promise<boolean> {
  return invokeOrPreview("restore_soul", { soulId }, () => {
    const soul = previewState.souls.find((item) => item.character_id === soulId);
    if (!soul) return false;
    (soul as any).archived_at = null;
    return true;
  });
}

export function listArchivedSouls(): Promise<SoulSummary[]> {
  return invokeOrPreview("list_archived_souls", {}, () =>
    previewState.souls.filter((soul) => soul.soul_kind !== "session_clone" && (soul as any).archived_at).map(summarizeSoul),
  );
}

export function archiveSavepoint(soulId: string): Promise<boolean> {
  return archiveSoul(soulId);
}

export function restoreSavepoint(soulId: string): Promise<boolean> {
  return restoreSoul(soulId);
}

export function listArchivedSavepoints(): Promise<SoulSummary[]> {
  return invokeOrPreview("list_archived_savepoints", {}, () =>
    previewState.souls.filter((soul) => soul.soul_kind !== "session_clone" && (soul as any).archived_at).map(summarizeSoul),
  );
}

export function deleteSoul(soulId: string): Promise<boolean> {
  return archiveSoul(soulId);
}

export function deleteSetting(settingId: string): Promise<boolean> {
  return invokeOrPreview("delete_setting", { settingId }, () => {
    return Promise.reject(new Error("delete_setting is deprecated; use archive_setting with active/default setting guard."));
  });
}

export function archiveSetting(settingId: string, activeOrDefaultIds: string[]): Promise<boolean> {
  return invokeOrPreview("archive_setting", { settingId, activeOrDefaultIds }, () => {
    if (activeOrDefaultIds.includes(settingId)) {
      return Promise.reject(new Error("Cannot archive the active/default setting. Switch settings first."));
    }
    const index = previewState.settings.findIndex((item) => item.setting_id === settingId);
    if (index >= 0) {
      (previewState.settings[index] as any).archived_at = Math.floor(Date.now() / 1000);
      return Promise.resolve(true);
    }
    return Promise.resolve(false);
  });
}

export function purgeSetting(settingId: string, activeOrDefaultIds: string[]): Promise<boolean> {
  return invokeOrPreview("purge_setting", { settingId, activeOrDefaultIds }, () => {
    if (activeOrDefaultIds.includes(settingId)) {
      return Promise.reject(new Error("Cannot purge the active/default setting. Switch settings first."));
    }
    const index = previewState.settings.findIndex((item) => item.setting_id === settingId);
    if (index === -1) return Promise.resolve(false);
    previewState.settings.splice(index, 1);
    return Promise.resolve(true);
  });
}

export function restoreSetting(settingId: string): Promise<boolean> {
  return invokeOrPreview("restore_setting", { settingId }, () => {
    const index = previewState.settings.findIndex((item) => item.setting_id === settingId);
    if (index >= 0) {
      (previewState.settings[index] as any).archived_at = null;
      return Promise.resolve(true);
    }
    return Promise.resolve(false);
  });
}

export function listArchivedSettings(): Promise<SettingSummary[]> {
  return invokeOrPreview("list_archived_settings", {}, () => {
    return previewState.settings.filter((item) => !!(item as any).archived_at).map(summarizeSetting);
  });
}

export function sendMockTurn(
  conversationId: string,
  soulId: string,
  userText: string,
  mode: string,
  replacementAssistantId?: number,
  correctionInstruction?: string,
): Promise<TurnResult> {
  return invokeOrPreview("send_mock_turn", { conversationId, soulId, userText, mode, replacementAssistantId: replacementAssistantId ?? null, correctionInstruction: correctionInstruction ?? null }, () =>
    sendPreviewTurn(conversationId, soulId, userText, mode, replacementAssistantId, correctionInstruction),
  );
}

export function sendApiTurn(
  conversationId: string,
  soulId: string,
  userText: string,
  mode: string,
  narratorSettings: ApiProviderSettings,
  stateUpdaterSettings: ApiProviderSettings,
  contextMode: ContextMode,
  signal?: AbortSignal,
  replacementAssistantId?: number,
  correctionInstruction?: string,
): Promise<TurnResult> {
  return invokeOrPreview(
    "send_api_turn",
    { conversationId, soulId, userText, mode, narratorSettings, stateUpdaterSettings, replacementAssistantId: replacementAssistantId ?? null, correctionInstruction: correctionInstruction ?? null, contextMode },
    () => sendPreviewApiTurn(conversationId, soulId, userText, mode, narratorSettings, signal, replacementAssistantId, correctionInstruction),
  );
}

export function listenApiStream(
  callback: (payload: { conversation_id: string; chunk: string }) => void,
): Promise<() => void> {
  if (!hasTauriRuntime()) return Promise.resolve(() => undefined);
  return listen<{ conversation_id: string; chunk: string }>(
    "api-chunk",
    (event) => callback(event.payload),
  );
}

export function listenChatMessageSaved(
  callback: (payload: { conversation_id: string; message: ChatMessage }) => void,
): Promise<() => void> {
  if (!hasTauriRuntime()) return Promise.resolve(() => undefined);
  return listen<{ conversation_id: string; message: ChatMessage }>(
    "chat-message-saved",
    (event) => callback(event.payload),
  );
}

export function listenDevLog(callback: (payload: DevLogEntry) => void): Promise<() => void> {
  if (!hasTauriRuntime()) return Promise.resolve(() => undefined);
  return listen<DevLogEntry>("dev-log", (event) => callback(event.payload));
}

export function listenEvaluatorJobStatusChanged(
  callback: (payload: EvaluatorJob) => void,
): Promise<() => void> {
  if (!hasTauriRuntime()) return Promise.resolve(() => undefined);
  return listen<EvaluatorJob>("evaluator-job-status-changed", (event) => callback(event.payload));
}

export type PipelineStageTrace = {
  stage_id: string;
  stage_name: string;
  status: "success" | "warning" | "skipped" | "failed";
  elapsed_ms: number;
  input_summary?: string | null;
  output_summary?: string | null;
  error_code?: string | null;
  error_message?: string | null;
  repair_action?: string | null;
  artifact_ref?: string | null;
};

export type TurnTokenUsage = {
  narrator_prompt_tokens?: number | null;
  narrator_completion_tokens?: number | null;
  narrator_estimated: boolean;
  evaluator_prompt_tokens?: number | null;
  evaluator_completion_tokens?: number | null;
  evaluator_estimated: boolean;
};

export type TurnPipelineTrace = {
  request_id: string;
  turn_id?: string | null;
  conversation_id: string;
  started_at: number;
  total_elapsed_ms: number;
  final_status: "success" | "partial_success" | "failed" | "canceled" | "running";
  failing_stage?: string | null;
  suggested_debug_action?: string | null;
  stages: PipelineStageTrace[];
  token_usage?: TurnTokenUsage | null;
};

export function listenPipelineTraceUpdated(
  callback: (payload: TurnPipelineTrace) => void,
): Promise<() => void> {
  if (!hasTauriRuntime()) return Promise.resolve(() => undefined);
  return listen<TurnPipelineTrace>("pipeline-trace-updated", (event) => callback(event.payload));
}

export type BackgroundJobHistoryEntry = {
  index: number;
  label: string;
  status: string;
  detail?: string | null;
  elapsed_ms?: number | null;
};

export type BackgroundJobProgress = {
  job_id: string;
  kind: string;
  label: string;
  status: "queued" | "running" | "succeeded" | "failed" | "canceled" | string;
  phase: string;
  current: number;
  total: number;
  succeeded: number;
  failed: number;
  recovered: number;
  started_at: number;
  updated_at: number;
  elapsed_ms: number;
  estimated_remaining_ms?: number | null;
  detail?: string | null;
  cancellable: boolean;
  history: BackgroundJobHistoryEntry[];
};

export function listenBackgroundJobProgress(
  callback: (payload: BackgroundJobProgress) => void,
): Promise<() => void> {
  if (!hasTauriRuntime()) return Promise.resolve(() => undefined);
  return listen<BackgroundJobProgress>("background-job-progress", (event) => callback(event.payload));
}

export function listenEvaluatorAutoFallbackTriggered(
  callback: (payload: { conversation_id: string; profile_id: string }) => void,
): Promise<() => void> {
  if (!hasTauriRuntime()) return Promise.resolve(() => undefined);
  return listen<{ conversation_id: string; profile_id: string }>(
    "evaluator_auto_fallback_triggered",
    (event) => callback(event.payload),
  );
}

export type EmbeddedModelStatus = {
  running: boolean;
  ready: boolean;
  url?: string | null;
  model?: string | null;
};

/** Launch a single-file local model (llamafile) as the embedded repair model. */
export function startEmbeddedRepairModel(
  binaryPath: string,
  port?: number | null,
  modelName?: string | null,
): Promise<EmbeddedModelStatus> {
  return invokeOrPreview(
    "start_embedded_repair_model",
    { binaryPath, port: port ?? null, modelName: modelName ?? null },
    () => {
      throw new Error("Embedded repair model requires the Tauri runtime.");
    },
  );
}

export function stopEmbeddedRepairModel(): Promise<void> {
  return invokeOrPreview("stop_embedded_repair_model", {}, () => undefined);
}

export function embeddedRepairModelStatus(): Promise<EmbeddedModelStatus> {
  return invokeOrPreview("embedded_repair_model_status", {}, () => ({
    running: false,
    ready: false,
    url: null,
    model: null,
  }));
}

export type EvaluatorOpsRejectedPayload = {
  conversation_id: string;
  assistant_message_id: number;
  failed_ops: EvaluatorOpRepairRequest[];
  /** "fix_rejected" (correct the failed ops) or "reextract" (no ops; the repair
   * model re-extracts the whole turn because the evaluator produced nothing). */
  repair_kind?: string;
};

/** Fired by the backend when the evaluator dropped one or more ops (its own
 * validation verdict). The frontend uses this to auto-fire a background repair. */
export function listenEvaluatorOpsRejected(
  callback: (payload: EvaluatorOpsRejectedPayload) => void,
): Promise<() => void> {
  if (!hasTauriRuntime()) return Promise.resolve(() => undefined);
  return listen<EvaluatorOpsRejectedPayload>("evaluator-ops-rejected", (event) =>
    callback(event.payload),
  );
}


export function listProviderProfiles(): Promise<ProviderProfile[]> {
  return invokeOrPreview("list_provider_profiles", {}, () => previewState.providerProfiles);
}

export function getProviderProfile(profileId: string): Promise<ProviderProfile> {
  return invokeOrPreview("get_provider_profile", { profileId }, () => {
    const profile = previewState.providerProfiles.find((item) => item.id === profileId);
    if (!profile) throw new Error("Provider profile not found");
    return profile;
  });
}

export function upsertProviderProfile(profile: ProviderProfile): Promise<ProviderProfile> {
  return invokeOrPreview("upsert_provider_profile", { profile }, () => {
    const now = Math.floor(Date.now() / 1000);
    const saved = { ...profile, created_at: profile.created_at || now, updated_at: now };
    const index = previewState.providerProfiles.findIndex((item) => item.id === profile.id);
    if (index >= 0) {
      previewState.providerProfiles[index] = saved;
    } else {
      previewState.providerProfiles.unshift(saved);
    }
    return saved;
  });
}

export function deleteProviderProfile(profileId: string): Promise<boolean> {
  return invokeOrPreview("delete_provider_profile", { profileId }, () => {
    const before = previewState.providerProfiles.length;
    previewState.providerProfiles = previewState.providerProfiles.filter((item) => item.id !== profileId);
    return previewState.providerProfiles.length !== before;
  });
}

export function archiveProviderProfile(profileId: string, activeIds: string[]): Promise<boolean> {
  return invokeOrPreview("archive_provider_profile", { profileId, activeIds }, () => {
    if (activeIds.includes(profileId)) {
      return Promise.reject(new Error("Cannot archive the active provider profile. Switch profiles first."));
    }
    const index = previewState.providerProfiles.findIndex((item) => item.id === profileId);
    if (index >= 0) {
      previewState.providerProfiles[index] = { ...previewState.providerProfiles[index], archived_at: Math.floor(Date.now() / 1000) };
      return true;
    }
    return false;
  });
}

export function restoreProviderProfile(profileId: string): Promise<boolean> {
  return invokeOrPreview("restore_provider_profile", { profileId }, () => {
    const index = previewState.providerProfiles.findIndex((item) => item.id === profileId);
    if (index >= 0) {
      previewState.providerProfiles[index] = { ...previewState.providerProfiles[index], archived_at: null };
      return true;
    }
    return false;
  });
}

export function listArchivedProviderProfiles(): Promise<ProviderProfile[]> {
  return invokeOrPreview("list_archived_provider_profiles", {}, () => {
    return previewState.providerProfiles.filter((item) => !!item.archived_at);
  });
}

export function getLatestEvaluatorJob(conversationId: string): Promise<EvaluatorJob | null> {
  return invokeOrPreview("get_latest_evaluator_job", { conversationId }, () => null);
}

export function cancelEvaluatorJob(jobId: string): Promise<void> {
  return invokeOrPreview("cancel_evaluator_job", { jobId }, () => undefined);
}

export function retryEvaluatorJob(
  conversationId: string,
  assistantMessageId: number,
  stateUpdaterSettings: ApiProviderSettings,
): Promise<void> {
  return invokeOrPreview(
    "retry_evaluator_job",
    { conversationId, assistantMessageId, stateUpdaterSettings },
    () => undefined,
  );
}

export type EvaluatorContractTestReport = {
  passed: boolean;
  errors: string[];
  raw_response: string;
  /** 0 untested/failed, 1 prompt-only, 2 json_object, 3 json_schema. */
  structured_output_support?: number;
  evaluator_compatibility_status?: number;
  evaluator_compatibility_status_label?: string;
};

export function runEvaluatorContractTest(profileId: string): Promise<EvaluatorContractTestReport> {
  return invokeOrPreview("run_evaluator_contract_test", { profileId }, () => {
    return {
      passed: true,
      errors: [],
      raw_response: "{}",
      structured_output_support: 0,
      evaluator_compatibility_status: 0,
      evaluator_compatibility_status_label: "untested"
    };
  });
}

export type SessionFormEvalTurn = {
  turn_index: number;
  user_excerpt: string;
  form_passed: boolean;
  form_rows_accepted: number;
  form_error: string | null;
  repair_attempted: boolean;
  repair_ops: number;
  repair_recovered: boolean;
  repair_error: string | null;
};

export type SessionFormEvalReport = {
  conversation_id: string;
  model: string;
  repair_model: string;
  turns_total: number;
  form_passed: number;
  form_failed: number;
  repair_recovered: number;
  per_turn: SessionFormEvalTurn[];
};

/** Dev-mode: replay the open session's chat log through the FORM evaluator + repair,
 * dry-run validated (nothing applied). `repairSettings` is the CONFIGURED repair
 * endpoint (repair profile / embedded local model) — without it the repair stage
 * falls back to the eval profile, which defeats the weak-eval→repair architecture. */
export function runSessionFormEvalBenchmark(
  conversationId: string,
  profileId: string,
  repairSettings?: ApiProviderSettings,
): Promise<SessionFormEvalReport> {
  return invokeOrPreview(
    "run_session_form_eval_benchmark",
    { conversationId, profileId, repairSettings },
    () => {
      throw new Error("Session form-eval benchmark requires the Tauri runtime.");
    },
  );
}

export type StructuredEvaluatorDiagnosticRun = {
  turn_index: number;
  user_message: string;
  narrator_response: string;
  evaluator_mode: string;
  enforcement_level: string;
  structured_enforcement_requested: string;
  structured_enforcement_validated: boolean;
  structured_schema_validation_status: string;
  structured_schema_validation_error?: string | null;
  fallback_path: string[];
  failure_reasons: string[];
  ops_count: number;
  compiled_patch_summary: unknown;
  syntactic_repair_used: boolean;
  memory_ops_count: number;
  relationship_event_ops_count: number;
  object_update_ops_count: number;
  scene_update_ops_count: number;
  state_patch_id?: string | null;
  error?: string | null;
  tool_calls_present: boolean;
  tool_call_count: number;
  tool_call_names: string[];
  raw_content_present: boolean;
  raw_tool_calls_present: boolean;
  structured_retry_count: number;
  structured_retry_reasons: string[];
  structured_retry_succeeded?: boolean | null;
  structured_retry_final_error?: string | null;
  perception_v2_shadow: PerceptionV2ShadowTrace;
};

export type PerceptionV2ShadowTrace = {
  attempted: boolean;
  commit_allowed: boolean;
  commit_count: number;
  schema_version: number;
  compiler_version: number;
  prompt_version: string;
  enforcement_level: string;
  schema_validated: boolean;
  status: string;
  error?: string | null;
  source_hash?: string | null;
  candidate_count: number;
  candidate_ids: string[];
  kind_counts: Record<string, number>;
  semantic_accepted: number;
  semantic_rejected: number;
  effect_count: number;
  engine_patch_summary: unknown;
  unsupported_effect_count: number;
  simulation_decision: string;
  diagnostic_codes: string[];
  v1_ops_count?: number | null;
  elapsed_ms: number;
  prompt_tokens?: number | null;
  completion_tokens?: number | null;
};

export type StructuredEvaluatorDiagnosticSummary = {
  conversation_id: string;
  provider_profile_id: string;
  provider_model: string;
  base_url_redacted: string;
  structured_mode_requested: string;
  structured_mode_resolved: string;
  resolved_evaluator_source: string;
  structured_policy: string;
  structured_evaluator_policy: string;
  evaluator_mode: string;
  strict_tool_diagnostic: boolean;
  strict_tool_passed: boolean;
  fallback_used: boolean;
  failure_turns: number[];
  structured_schema_version: number;
  perception_v2_schema_version: number;
  perception_v2_compiler_version: number;
  perception_v2_shadow_attempted: number;
  perception_v2_shadow_validated: number;
  perception_v2_shadow_candidates: number;
  perception_v2_shadow_commit_count: number;
  runs: StructuredEvaluatorDiagnosticRun[];
  enforcement_levels: string[];
  evaluator_mode_per_run: string[];
  structured_enforcement_per_run: string[];
  structured_enforcement_requested_per_run: string[];
  structured_enforcement_validated_per_run: boolean[];
  structured_schema_validation_status_per_run: string[];
  failure_reasons: string[];
  fallback_paths: string[][];
  ops_counts: number[];
  memory_ops_count: number;
  relationship_event_ops_count: number;
  object_update_ops_count: number;
  scene_update_ops_count: number;
  syntactic_repair_used: boolean;
  final_memory_count: number;
  final_relationship_target_ids: string[];
  final_object_states: unknown[];
  final_scene_participants: string[];
  default_player_leaked_into_normal_rp_state: boolean;
  default_player_in_relationship_context: boolean;
  payload_history_path: string;
  mne_checkpoint_path: string;
  summary_json_path: string;
};

export type BenchmarkType =
  | "visible_ai_chat"
  | "scripted_visible_replay"
  | "headless_regression"
  | "multi_agent_visible_chat";

export type BenchmarkTarget =
  | "current_session"
  | "new_benchmark_session_from_current_soul"
  | "new_benchmark_session_from_selected_soul_world";

export type BenchmarkSettings = {
  benchmark_type: BenchmarkType;
  target: BenchmarkTarget;
  current_conversation_id?: string | null;
  turn_count: number;
  narrator_style: string;
  evaluator_mode?: string | null;
  structured_evaluator_transport?: string | null;
  structured_evaluator_policy?: string | null;
  structured_evaluator_max_retries?: number | null;
  player_simulator_profile_id?: string | null;
  player_goal: string;
  export_payload_history: boolean;
  export_mne: boolean;
  export_summary_json: boolean;
  strict_tool_evaluator: boolean;
  wait_for_evaluator_each_turn: boolean;
};

export type BenchmarkScorecard = {
  visible_chat_messages_created: boolean;
  normal_pipeline_used: boolean;
  visible_turns_requested: number;
  visible_turns_completed: number;
  visible_user_messages_created: number;
  visible_assistant_messages_created: number;
  unique_user_message_ids: number;
  unique_assistant_message_ids: number;
  internal_evaluator_retry_count: number;
  internal_evaluator_retry_payload_count: number;
  duplicate_turn_rows_detected: boolean;
  duplicate_turn_message_pairs: string[];
  player_simulator_payload_count: number;
  turn_count_requested: number;
  turn_count_completed: number;
  player_simulator_calls: number;
  narrator_calls: number;
  evaluator_calls: number;
  evaluator_waited_each_turn: boolean;
  memory_updated: boolean;
  object_state_updated: boolean;
  relationship_updated: boolean;
  relationship_target_checked?: string | null;
  relationship_changed_from?: Record<string, unknown> | null;
  relationship_changed_to?: Record<string, unknown> | null;
  relationship_delta_patch_ids: string[];
  relationship_delta_sources: string[];
  evaluator_provider_failures: number;
  structured_provider_429_count: number;
  evaluator_response_failed_count: number;
  evaluator_empty_patch_count: number;
  form_rows_rejected_count: number;
  local_repair_invoked_count: number;
  local_reextract_invoked_count: number;
  local_repair_payload_count: number;
  local_repair_response_count: number;
  local_repair_state_patch_count: number;
  payload_history_export_succeeded: boolean;
  narrator_visible_response_each_turn: boolean;
  narrator_provider_error?: string | null;
  stop_reason?: string | null;
  failed_stage?: string | null;
  evaluator_used_tool_call_where_required: boolean;
  no_evaluator_form_v1_fallback_in_strict_mode: boolean;
  syntactic_repair_unused_in_strict_mode: boolean;
  strict_tool_evaluator: boolean;
  evaluator_mode_actual: string;
  local_repair_recovered_state_when_warranted: boolean;
  local_repair_unavailable: boolean;
  memories_increased_over_time: boolean;
  active_player_relationship_changed_when_warranted: boolean;
  object_ids_stable: boolean;
  default_player_not_normal_rp_relationship_target: boolean;
  mne_export_succeeded: boolean;
  pass: boolean;
  failure_reasons: string[];
};

export type BenchmarkTurnSummary = {
  turn_index: number;
  stage: string;
  simulated_user_message: string;
  narrator_response_present: boolean;
  narrator_error?: string | null;
  evaluator_mode: string;
  structured_transport_actual?: string | null;
  tool_calls_present: boolean;
  tool_call_count: number;
  structured_retry_count: number;
  fallback_path: string[];
  syntactic_repair_used: boolean;
  memory_count_after: number;
  object_count_after: number;
  relationship_summary_after: string;
};

export type BenchmarkSummary = {
  benchmark_id: string;
  benchmark_type: string;
  conversation_id: string;
  started_at: number;
  completed_at: number;
  turn_count_requested: number;
  turn_count_completed: number;
  narrator_model: string;
  evaluator_model: string;
  player_simulator_model?: string | null;
  narrator_failures: number;
  evaluator_failures: number;
  tool_call_success_count: number;
  tool_call_failure_count: number;
  retry_count: number;
  retry_success_count: number;
  fallback_count: number;
  syntactic_repair_count: number;
  default_player_leak_detected: boolean;
  duplicate_relationship_context_detected: boolean;
  final_memory_count: number;
  final_object_state_count: number;
  final_relationship_count: number;
  visible_turns_requested: number;
  visible_turns_completed: number;
  visible_user_messages_created: number;
  visible_assistant_messages_created: number;
  unique_user_message_ids: number;
  unique_assistant_message_ids: number;
  internal_evaluator_retry_count: number;
  internal_evaluator_retry_payload_count: number;
  duplicate_turn_rows_detected: boolean;
  duplicate_turn_message_pairs: string[];
  player_simulator_payload_count: number;
  per_turn: BenchmarkTurnSummary[];
  object_identity_checks: Array<{ label: string; expected_object_id: string; found: boolean }>;
  mne_export_path?: string | null;
  payload_history_path?: string | null;
  summary_json_path?: string | null;
  scorecard: BenchmarkScorecard;
};

export function runStructuredEvaluatorDiagnostic(
  profileId?: string | null,
): Promise<StructuredEvaluatorDiagnosticSummary> {
  return invokeOrPreview("run_structured_evaluator_diagnostic", { profileId }, () => {
    throw new Error("Structured evaluator diagnostics require the Tauri runtime.");
  });
}

export type MemoryCurationOperation = "pin" | "unpin" | "restore_archived";

export type MemoryCurationResult = {
  patch_id: string;
  turn_id: string;
  branch_id: string;
  memory_id: string;
  operation: MemoryCurationOperation | string;
  soul: Soul;
};

/** Pin/unpin/restore a memory as a ledger patch (replay-safe curation). */
export function curateMemory(
  conversationId: string,
  soulId: string,
  memoryId: string,
  operation: MemoryCurationOperation
): Promise<MemoryCurationResult> {
  return invokeOrPreview("curate_memory", { conversationId, soulId, memoryId, operation }, () => {
    const soul = previewState.souls.find((item) => item.character_id === soulId);
    if (!soul) throw new Error("Soul not found");
    return Promise.resolve({
      patch_id: "preview-patch",
      turn_id: "preview-turn",
      branch_id: "preview-branch",
      memory_id: memoryId,
      operation,
      soul,
    });
  });
}

export function setActiveEvaluatorProfile(conversationId: string, profileId: string | null): Promise<void> {
  return invokeOrPreview("set_active_evaluator_profile", { conversationId, profileId }, () => {
    const conv = previewState.conversations.find(c => c.conversation_id === conversationId);
    if (conv) {
      (conv as any).active_evaluator_profile_id = profileId;
    }
    return Promise.resolve();
  });
}

export function listConversationMessages(conversationId: string): Promise<ChatMessage[]> {
  return invokeOrPreview("list_conversation_messages", { conversationId }, () =>
    previewState.messages
      .filter((message) => message.conversation_id === conversationId)
      .map(hydratePreviewMessage),
  );
}

export function importImageAsset(args: {
  path: string;
  linkedSoulId?: string | null;
  linkedConversationId?: string | null;
  linkedMessageId?: number | null;
  source?: "uploaded" | "generated" | "imported";
}): Promise<ImageAsset> {
  return invokeOrPreview(
    "import_image_asset",
    {
      path: args.path,
      linkedSoulId: args.linkedSoulId ?? null,
      linkedConversationId: args.linkedConversationId ?? null,
      linkedMessageId: args.linkedMessageId ?? null,
      source: args.source ?? "uploaded",
    },
    () => makePreviewImageAsset(args.path, args.source ?? "uploaded", {
      linked_soul_id: args.linkedSoulId ?? null,
      linked_conversation_id: args.linkedConversationId ?? null,
      linked_message_id: args.linkedMessageId ?? null,
    }),
  );
}

export async function importImageAssetFromFile(args: {
  file: File;
  linkedSoulId?: string | null;
  linkedConversationId?: string | null;
  linkedMessageId?: number | null;
  source?: "uploaded" | "generated" | "imported";
}): Promise<ImageAsset> {
  const dataBase64 = await fileToBase64(args.file);
  return invokeOrPreview(
    "import_image_asset_bytes",
    {
      fileName: args.file.name,
      dataBase64,
      linkedSoulId: args.linkedSoulId ?? null,
      linkedConversationId: args.linkedConversationId ?? null,
      linkedMessageId: args.linkedMessageId ?? null,
      source: args.source ?? "uploaded",
    },
    () =>
      makePreviewImageAsset(args.file.name, args.source ?? "uploaded", {
        linked_soul_id: args.linkedSoulId ?? null,
        linked_conversation_id: args.linkedConversationId ?? null,
        linked_message_id: args.linkedMessageId ?? null,
      }),
  );
}

export function getImageAsset(imageAssetId: string): Promise<ImageAsset> {
  return invokeOrPreview("get_image_asset", { imageAssetId }, () => {
    const asset = previewState.imageAssets.find((item) => item.id === imageAssetId);
    if (!asset) throw new Error("Image asset not found");
    return asset;
  });
}

export function getImageAssetDataUrl(imageAssetId: string): Promise<string> {
  return invokeOrPreview("get_image_asset_data_url", { imageAssetId }, () => {
    const asset = previewState.imageAssets.find((item) => item.id === imageAssetId);
    if (!asset) throw new Error("Image asset not found");
    return asset.file_path;
  });
}

export function createUserImageMessage(
  conversationId: string,
  path: string,
  content?: string,
): Promise<ChatMessage[]> {
  return invokeOrPreview(
    "create_user_image_message",
    { conversationId, path, content: content ?? null },
    () => {
      const message = makePreviewMessage(conversationId, "user", content?.trim() || "[Image]");
      previewState.messages.push(message);
      const asset = makePreviewImageAsset(path, "uploaded", {
        linked_conversation_id: conversationId,
        linked_message_id: message.id,
      });
      previewState.messageAttachments.push({
        id: previewState.messageAttachments.length + 1,
        message_id: message.id,
        image_asset_id: asset.id,
        created_at: Math.floor(Date.now() / 1000),
        image: asset,
      });
      return previewState.messages
        .filter((item) => item.conversation_id === conversationId)
        .map(hydratePreviewMessage);
    },
  );
}

export async function createUserImageMessageFromFile(
  conversationId: string,
  file: File,
  content?: string,
): Promise<ChatMessage[]> {
  const dataBase64 = await fileToBase64(file);
  return invokeOrPreview(
    "create_user_image_message_bytes",
    { conversationId, fileName: file.name, dataBase64, content: content ?? null },
    () => {
      const message = makePreviewMessage(conversationId, "user", content?.trim() || "[Image]");
      previewState.messages.push(message);
      const asset = makePreviewImageAsset(file.name, "uploaded", {
        linked_conversation_id: conversationId,
        linked_message_id: message.id,
      });
      previewState.messageAttachments.push({
        id: previewState.messageAttachments.length + 1,
        message_id: message.id,
        image_asset_id: asset.id,
        created_at: Math.floor(Date.now() / 1000),
        image: asset,
      });
      return previewState.messages
        .filter((item) => item.conversation_id === conversationId)
        .map(hydratePreviewMessage);
    },
  );
}

export function imageAssetSrc(asset?: ImageAsset | null): string {
  if (!asset) return "";
  if (asset.file_path.startsWith("blob:") || asset.file_path.startsWith("data:")) return asset.file_path;
  return convertFileSrc(asset.thumbnail_path || asset.file_path);
}

export function archiveConversation(conversationId: string): Promise<boolean> {
  return invokeOrPreview("archive_session", { conversationId }, () => {
    const conversation = previewState.conversations.find(
      (item) => item.conversation_id === conversationId,
    );
    if (!conversation) return false;
    if (!conversation.title.startsWith("[Archived] ")) {
      conversation.title = `[Archived] ${conversation.title}`;
    }
    (conversation as any).archived_at = Math.floor(Date.now() / 1000);
    conversation.updated_at = Math.floor(Date.now() / 1000);
    return true;
  });
}

export function runBenchmark(
  soulId: string,
  settingId: string | null,
  provider: string,
  narratorSettings: ApiProviderSettings,
  stateUpdaterSettings: ApiProviderSettings,
  settings: BenchmarkSettings,
): Promise<BenchmarkSummary> {
  return invokeOrPreview(
    "run_benchmark",
    { soulId, settingId, provider, narratorSettings, stateUpdaterSettings, settings },
    () => {
      throw new Error("Benchmark Runner requires the Tauri runtime so it can create real visible chat turns.");
    },
  );
}

export type EvaluatorOpRepairRequest = {
  op_json: string;
  reason: string;
};

/**
 * Fire-and-forget background op-repair: re-runs ONLY the failed ops through a
 * configurable (e.g. local) repair model, up to 5 attempts, applying any that
 * now validate. Non-blocking — does not affect chat or the main eval.
 */
export function repairEvaluatorOps(
  conversationId: string,
  assistantMessageId: number,
  failedOps: EvaluatorOpRepairRequest[],
  repairSettings: ApiProviderSettings,
  repairKind?: string,
): Promise<void> {
  return invokeOrPreview(
    "repair_evaluator_ops",
    { conversationId, assistantMessageId, failedOps, repairSettings, repairKind },
    () => {
      throw new Error("Evaluator op-repair requires the Tauri runtime.");
    },
  );
}

export type BenchmarkSessionInit = {
  benchmark_id: string;
  conversation_id: string;
  session_soul_id: string;
  started_at: number;
  initial_memory_count: number;
  initial_object_count: number;
  initial_relationship_count: number;
  relationship_target_checked: string;
  initial_active_player_relationship?: Record<string, unknown> | null;
};

/** Set up a benchmark conversation for the live self-play loop (frontend-driven). */
export function prepareBenchmarkSession(
  soulId: string,
  settingId: string | null,
  settings: BenchmarkSettings,
): Promise<BenchmarkSessionInit> {
  return invokeOrPreview(
    "prepare_benchmark_session",
    { soulId, settingId, settings },
    () => {
      throw new Error("Benchmark Runner requires the Tauri runtime so it can create real visible chat turns.");
    },
  );
}

/** Generate the next AI player-side message for the live self-play loop. */
export function generateBenchmarkPlayerMessage(
  conversationId: string,
  soulId: string,
  playerProfileId: string,
  playerGoal: string,
): Promise<string> {
  return invokeOrPreview(
    "generate_benchmark_player_message",
    { conversationId, soulId, playerProfileId, playerGoal },
    () => {
      throw new Error("Player Simulator requires the Tauri runtime.");
    },
  );
}

/** Like generateBenchmarkPlayerMessage but uses the traditional RP engine
 * (full transcript, no Soul/memory) — the control side of the comparison. */
export function generateTraditionalRpMessage(
  conversationId: string,
  soulId: string,
  playerProfileId: string,
  playerGoal: string,
): Promise<string> {
  return invokeOrPreview(
    "generate_traditional_rp_message",
    { conversationId, soulId, playerProfileId, playerGoal },
    () => {
      throw new Error("Traditional RP engine requires the Tauri runtime.");
    },
  );
}

/** Capture the per-turn summary after a live self-play turn has fully applied. */
export function benchmarkTurnSummary(
  conversationId: string,
  turnIndex: number,
  userText: string,
  stage: string,
  narratorError: string | null,
  stateUpdaterSettings: ApiProviderSettings,
): Promise<BenchmarkTurnSummary> {
  return invokeOrPreview(
    "benchmark_turn_summary",
    { conversationId, turnIndex, userText, stage, narratorError, stateUpdaterSettings },
    () => {
      throw new Error("Benchmark Runner requires the Tauri runtime.");
    },
  );
}

/** Finalize a live self-play benchmark: build summary, run exports, score it. */
export function finalizeBenchmark(
  benchmarkId: string,
  conversationId: string,
  startedAt: number,
  narratorSettings: ApiProviderSettings,
  stateUpdaterSettings: ApiProviderSettings,
  settings: BenchmarkSettings,
  initialMemoryCount: number,
  initialObjectCount: number,
  initialRelationshipCount: number,
  relationshipTargetChecked: string,
  initialActivePlayerRelationship: Record<string, unknown> | null,
  turnCountCompleted: number,
  narratorFailures: number,
  perTurn: BenchmarkTurnSummary[],
): Promise<BenchmarkSummary> {
  return invokeOrPreview(
    "finalize_benchmark",
    {
      benchmarkId,
      conversationId,
      startedAt,
      narratorSettings,
      stateUpdaterSettings,
      settings,
      initialMemoryCount,
      initialObjectCount,
      initialRelationshipCount,
      relationshipTargetChecked,
      initialActivePlayerRelationship,
      turnCountCompleted,
      narratorFailures,
      perTurn,
    },
    () => {
      throw new Error("Benchmark Runner requires the Tauri runtime.");
    },
  );
}

export function deleteConversation(conversationId: string): Promise<boolean> {
  return archiveConversation(conversationId);
}

export function restoreConversation(conversationId: string): Promise<boolean> {
  return invokeOrPreview("restore_session", { conversationId }, () => {
    const conversation = previewState.conversations.find(
      (item) => item.conversation_id === conversationId,
    );
    if (!conversation) return false;
    if (conversation.title.startsWith("[Archived] ")) {
      conversation.title = conversation.title.replace("[Archived] ", "");
    }
    (conversation as any).archived_at = null;
    conversation.updated_at = Math.floor(Date.now() / 1000);
    return true;
  });
}

export function openSessionDataLocation(): Promise<string> {
  return invokeOrPreview(
    "open_session_data_location",
    {},
    () => {
      throw new Error("Opening the session data folder requires the Mnemosyne desktop app.");
    },
  );
}

export function deleteMessage(conversationId: string, messageId: number): Promise<boolean> {
  return invokeOrPreview("delete_message", { conversationId, messageId }, () => {
    const beforeCount = previewState.messages.length;
    previewState.messages = previewState.messages.filter(
      (message) => !(message.conversation_id === conversationId && message.id === messageId),
    );
    previewState.assistantVariants = previewState.assistantVariants.filter(
      (variant) => !(variant.conversation_id === conversationId && variant.message_id === messageId),
    );
    return previewState.messages.length !== beforeCount;
  });
}

export function hideLatestBenchmarkFailedUserMessage(
  conversationId: string,
  userText: string,
): Promise<number | null> {
  return invokeOrPreview(
    "hide_latest_benchmark_failed_user_message",
    { conversationId, userText },
    () => {
      const expected = userText.trim();
      if (!expected) return null;
      const latest = [...previewState.messages]
        .filter((message) => message.conversation_id === conversationId)
        .sort((left, right) => right.id - left.id)[0];
      if (!latest || latest.role !== "user" || latest.content.trim() !== expected) return null;
      previewState.messages = previewState.messages.filter((message) => message.id !== latest.id);
      return latest.id;
    },
  );
}

export function restoreInactiveMessages(conversationId: string): Promise<RestoreTurnsResult> {
  return invokeOrPreview("restore_inactive_messages", { conversationId }, () =>
    ({
      messages: previewState.messages
        .filter((message) => message.conversation_id === conversationId)
        .map(hydratePreviewMessage),
      preview: {
        restored_message_ids: [],
        skipped_duplicate_ids: [],
        skipped_pending_ids: [],
        skipped_failed_ids: [],
        skipped_retry_attempt_ids: [],
        skipped_regenerated_discarded_ids: [],
      },
    }),
  );
}

export function dedupeActiveAdjacentUserMessages(
  conversationId: string,
): Promise<DedupeAdjacentUserMessagesResult> {
  return invokeOrPreview("dedupe_active_adjacent_user_messages", { conversationId }, () => ({
    canonical_user_message_ids: [],
    hidden_duplicate_user_message_ids: [],
  }));
}

export function updateUserMessage(
  conversationId: string,
  messageId: number,
  content: string,
): Promise<ChatMessage[]> {
  return invokeOrPreview(
    "update_user_message",
    { conversationId, messageId, content },
    () => {
      const trimmed = content.trim();
      if (!trimmed) throw new Error("User message cannot be empty");
      const message = previewState.messages.find(
        (item) =>
          item.conversation_id === conversationId && item.id === messageId && item.role === "user",
      );
      if (!message) throw new Error("User message not found");
      message.content = trimmed;
      return previewState.messages.filter((item) => item.conversation_id === conversationId);
    },
  );
}

export function listAssistantMessageVariants(
  conversationId: string,
  messageId: number,
): Promise<AssistantMessageVariant[]> {
  return invokeOrPreview("list_assistant_message_variants", { conversationId, messageId }, () =>
    listPreviewAssistantVariants(conversationId, messageId),
  );
}

export function selectAssistantMessageVariant(
  conversationId: string,
  messageId: number,
  variantId: number,
): Promise<VariantSelectionResult> {
  return invokeOrPreview(
    "select_assistant_message_variant",
    { conversationId, messageId, variantId },
    () => selectPreviewAssistantVariant(conversationId, messageId, variantId),
  );
}

export function deleteAssistantMessageVariant(
  conversationId: string,
  messageId: number,
  variantId: number,
): Promise<VariantSelectionResult> {
  return invokeOrPreview(
    "delete_assistant_message_variant",
    { conversationId, messageId, variantId },
    () => deletePreviewAssistantVariant(conversationId, messageId, variantId),
  );
}

export function inspectTurnBranchIntegrity(conversationId: string): Promise<unknown> {
  return invokeOrPreview("inspect_turn_branch_integrity", { conversationId }, () => ({
    conversation_id: conversationId,
    preview: true,
    suspected_duplicate_branch_causes: [],
  }));
}

export function repairAccidentalNormalSendVariants(conversationId: string): Promise<unknown> {
  return invokeOrPreview("repair_accidental_normal_send_variants", { conversationId }, () => ({
    conversation_id: conversationId,
    preview: true,
    repaired: [],
  }));
}

export function listLlmPayloadLogs(conversationId: string): Promise<LlmPayloadLog[]> {
  return invokeOrPreview("list_llm_payload_logs", { conversationId }, () =>
    previewState.payloadLogs.filter((log) => log.conversation_id === conversationId),
  );
}

export function getLlmPayloadLog(logId: number): Promise<LlmPayloadLog> {
  return invokeOrPreview("get_llm_payload_log", { logId }, () => {
    const log = previewState.payloadLogs.find((item) => item.id === logId);
    if (!log) throw new Error("LLM payload log not found");
    return log;
  });
}

export function getBranchPatchDebug(conversationId: string): Promise<BranchPatchDebug> {
  return invokeOrPreview("get_branch_patch_debug", { conversationId }, () => ({
    branch_id: "preview",
    active_turn_id: null,
    rebuild_generation: 0,
    applied_patches: [],
    skipped_discarded_patches: [],
    invalidated_patches: [],
  }));
}

export function rebuildSessionFromLedger(conversationId: string): Promise<BranchPatchDebug> {
  return invokeOrPreview("rebuild_session_from_ledger", { conversationId }, () => ({
    branch_id: "preview",
    active_turn_id: null,
    rebuild_generation: 0,
    applied_patches: [],
    skipped_discarded_patches: [],
    invalidated_patches: [],
  }));
}

export function exportVisibleChatLog(conversationId: string): Promise<ExportResult> {
  return invokeOrPreview("export_visible_chat_log", { conversationId }, () => {
    const content = renderPreviewVisibleChatLog(
      previewState.messages.filter((message) => message.conversation_id === conversationId),
    );
    downloadPreviewExport(`mnemosyne-${conversationId}-visible-chat-log.md`, content);
    return { path: "browser-downloads", message: "Visible chat log exported." };
  });
}

export function exportLlmPayloadHistory(conversationId: string): Promise<ExportResult> {
  return invokeOrPreview("export_llm_payload_history", { conversationId }, () => {
    const content = renderPreviewPayloadHistory(
      previewState.payloadLogs.filter((log) => log.conversation_id === conversationId),
    );
    downloadPreviewExport(`mnemosyne-${conversationId}-llm-payload-history.md`, content);
    return {
      path: "browser-downloads",
      message: previewState.payloadLogs.some((log) => log.conversation_id === conversationId)
        ? "LLM payload history exported."
        : "No LLM payload logs found for this conversation.",
    };
  });
}

export function compileContext(
  soulId: string,
  conversationId: string,
): Promise<ContextPreview> {
  return invokeOrPreview("compile_context", { soulId, conversationId }, () => {
    const soul = previewState.souls.find((item) => item.character_id === soulId);
    if (!soul) throw new Error("Soul not found");
    return compilePreviewContext(soul, conversationId);
  });
}

export function previewApiPayload(
  conversationId: string,
  soulId: string,
  userText: string,
  mode: string,
  settings: ApiProviderSettings,
  provider: string,
  contextMode: ContextMode = "brief",
): Promise<LlmPayloadPreview> {
  return invokeOrPreview(
    "preview_api_payload",
    { conversationId, soulId, userText, mode, settings, provider, contextMode },
    () => {
      const soul = previewState.souls.find((item) => item.character_id === soulId);
      if (!soul) throw new Error("Soul not found");
      const context = compilePreviewContext(soul, conversationId, {
        separateUserMessageFollows: Boolean(userText.trim()),
      });
      const systemMessage = buildNarratorSystemPrompt(
        settings.system_prompt,
        mode,
        soul,
        context.text,
        false,
      );
      const userMessage = userText.trim();
      const systemTokens = estimateTokens(systemMessage);
      const contextTokens = estimateTokens(context.text);
      const userTokens = estimateTokens(userMessage);
      return {
        provider,
        mode,
        context_mode: contextMode,
        custom_prompt_status: customPromptStatusFor(mode, systemMessage),
        model: settings.model.trim(),
        base_url: settings.base_url.trim(),
        system_message: systemMessage,
        user_message: userMessage,
        context: context.text,
        messages: [
          { role: "system", content: systemMessage },
          { role: "user", content: userMessage },
        ],
        truncated: context.truncated,
        estimated_tokens: {
          system: systemTokens,
          context: contextTokens,
          user: userTokens,
          total: systemTokens + userTokens + contextTokens,
        },
        memory_slot_debug: context.memory_slot_debug ?? [],
      };
    },
  );
}

export function runConsolidation(soulId: string): Promise<Soul> {
  return invokeOrPreview("run_consolidation", { soulId }, () => {
    const soul = previewState.souls.find((item) => item.character_id === soulId);
    if (!soul) throw new Error("Soul not found");
    consolidatePreviewSoul(soul);
    return soul;
  });
}

export function loadSoulFile(path: string): Promise<Soul> {
  return invoke("load_soul_file", { path });
}

export function loadSettingFile(path: string): Promise<SettingSoul> {
  return invoke("load_setting_file", { path });
}

export function saveSoulFile(path: string, soul: Soul): Promise<void> {
  return invokeOrPreview("save_soul_file", { path, soul }, () => {
    const blob = new Blob([JSON.stringify(soul, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = path;
    link.click();
    URL.revokeObjectURL(url);
  });
}

export function saveSettingFile(path: string, setting: SettingSoul): Promise<void> {
  return invokeOrPreview("save_setting_file", { path, setting }, () => {
    const blob = new Blob([JSON.stringify(setting, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = path;
    link.click();
    URL.revokeObjectURL(url);
  });
}

declare global {
  interface Window {
    mnemosyneDebug?: {
      dedupeActiveAdjacentUserMessages: (
        conversationId: string,
      ) => Promise<DedupeAdjacentUserMessagesResult>;
      inspectTurnBranchIntegrity: (conversationId: string) => Promise<unknown>;
      repairAccidentalNormalSendVariants: (conversationId: string) => Promise<unknown>;
    };
  }
}

if (import.meta.env.DEV && typeof window !== "undefined") {
  window.mnemosyneDebug = {
    dedupeActiveAdjacentUserMessages,
    inspectTurnBranchIntegrity,
    repairAccidentalNormalSendVariants,
  };
}
