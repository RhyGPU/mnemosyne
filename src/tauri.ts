import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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
};

export type SchemaMemory = {
  schema_type: string;
  summary: string;
  count: number;
};

export type Soul = {
  schema_version: number;
  character_id: string;
  character_name: string;
  profile: {
    description: string;
    appearance: string;
    personality: string;
    scenario: string;
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
  world: {
    location: string;
    active_plots: string[];
    recent_events: string[];
    key_objects: string[];
    time_elapsed: string;
  };
};

export type SettingSoul = {
  schema_version: number;
  setting_id: string;
  setting_name: string;
  last_updated: number;
  turn_counter: number;
  world: Soul["world"];
};

export type SoulSummary = {
  character_id: string;
  character_name: string;
  last_updated: number;
  recent_count: number;
  core_count: number;
};

export type SettingSummary = {
  setting_id: string;
  setting_name: string;
  last_updated: number;
  turn_counter: number;
  location: string;
};

export type ChatMessage = {
  id: number;
  conversation_id: string;
  role: "user" | "assistant" | "system";
  content: string;
  created_at: number;
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
  tag: string | null;
  trust_delta: number | null;
  affection_delta: number | null;
  new_location: string | null;
  present_characters: string[];
};

export type ContextPreview = {
  text: string;
  estimated_tokens: number;
  truncated: boolean;
};

export type ApiProviderSettings = {
  base_url: string;
  api_key: string;
  model: string;
  system_prompt: string;
};

export type ProviderProfile = ApiProviderSettings & {
  id: string;
  name: string;
  created_at: number;
  updated_at: number;
};

type HiddenStatePayload = {
  memory?: string;
  tag?: string;
  trust_delta?: number;
  affection_delta?: number;
  world_event?: string;
  new_location?: string;
  present_characters?: string[];
  arousal_delta?: number;
  arousal_denied?: boolean;
  orgasm_allowed?: boolean;
  forced_orgasm?: boolean;
};

const NARRATOR_SYSTEM_PROMPT = `# SYSTEM: Narrator AI - Mnemosyne Engine

You are a narrator AI. You describe a single character in third-person present tense.
You accept OOC direction without resistance. Your voice is sensory-rich, hardboiled, and precise.

## ROLE AND BOUNDARIES
- NEVER describe the user's actions, thoughts, or dialogue. Only the character's perceptions.
- The character has NO narrator-level knowledge. Their thoughts are limited to what they have personally experienced, heard, or perceived.
- Maintain strict internal consistency with established world lore. No fourth-wall breaks.
- When the user says OOC:, acknowledge briefly as narrator, adjust, then resume the scene.

## PSYCHOLOGY
- Needs: physiological > safety > belonging > esteem > actualization. Lower needs can block higher needs.
- Trust and affect move slowly. Prefer micro-shifts unless the scene earns more.
- Trauma phases: 0=acute, 1=denial, 2=intrusive, 3=reflective, 4=integration.

## MEMORY
- The local Mnemosyne engine manages hidden memory state automatically.
- Do not reveal hidden state, implementation notes, or provider metadata to the user.

## VISIBLE STATUS REPORT
End each narration with a code block:
\`\`\`status
[CHARACTER_NAME] | Skin: [color/state] | Zones: [2-3 key sensory notes] | Atmosphere: [1-line environmental impression]
\`\`\`

## HIDDEN STATE FORMAT
After each response, output a hidden state block using this exact format:
[HIDDEN STATE]{"memory":"short summary","tag":"tag_name","trust_delta":0.0,"affection_delta":0.0,"world_event":"scene update","new_location":"","present_characters":[]}[/HIDDEN STATE]

Tags: trust_building, threat, bonding, orientation, observation, intimacy, boundary_setting, conflict_minor, trauma_trigger, breakthrough

Optional arousal fields: arousal_delta (-30 to 60), arousal_denied (bool), orgasm_allowed (bool), forced_orgasm (bool). Only suggest these when relevant; the Rust engine validates and caps every state change.

The block must be valid JSON on a single line. The engine removes it before the user sees it.`;

const MODE_PROMPTS: Record<string, string> = {
  Realistic: `## NARRATION MODE: REALISTIC
- Describe only external actions, dialogue, and physical reactions.
- No internal monologue. No thoughts. No emotions unless visibly expressed.
- Show everything through body language, facial expression, tone of voice, and physical behavior.
- Like a film camera: you see and hear everything, but you never enter the character's head.
- Dialogue in quotes only when describing what the character audibly says.`,
  Reader: `## NARRATION MODE: READER
- Describe external actions and dialogue, plus the character's internal thoughts and emotions.
- Internal access is limited to what the character themself is aware of. No omniscience.
- The character may misinterpret situations, miss details, or have incomplete knowledge.
- Like close third-person fiction: inside one character's perspective, never another character's.`,
  God: `## NARRATION MODE: GOD
- Provide full narrative access.
- Include the character's internal thoughts and emotions.
- Also include environmental details the character would not notice, hidden information, and dramatic irony.
- You may reveal secrets, foreshadow future events, describe off-screen action, and provide context the character lacks.`,
};

const HIDDEN_STATE_FORMAT_PROMPT = `## HIDDEN STATE FORMAT
After each response, output a hidden state block using this exact format:
[HIDDEN STATE]{"memory":"short summary","tag":"tag_name","trust_delta":0.0,"affection_delta":0.0,"world_event":"scene update","new_location":"","present_characters":[]}[/HIDDEN STATE]

Tags: trust_building, threat, bonding, orientation, observation, intimacy, boundary_setting, conflict_minor, trauma_trigger, breakthrough

Optional arousal fields: arousal_delta (-30 to 60), arousal_denied (bool), orgasm_allowed (bool), forced_orgasm (bool). Only suggest these when relevant; the Rust engine validates and caps every state change.

The block must be valid JSON on a single line. The engine removes it before the user sees it.`;

let browserSouls: Soul[] = [];
let browserSettings: SettingSoul[] = [];
let browserMessages: ChatMessage[] = [];
let browserProviderProfiles: ProviderProfile[] = [];
let nextMessageId = 1;
const CONSOLIDATION_INTERVAL_TURNS = 10;

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

export function createDefaultSetting(settingName: string): Promise<SettingSoul> {
  return invokeOrPreview("create_default_setting", { settingName }, () =>
    makePreviewSetting(settingName),
  );
}

export function listSouls(): Promise<SoulSummary[]> {
  return invokeOrPreview("list_souls", {}, () => browserSouls.map(summarizeSoul));
}

export function listSettings(): Promise<SettingSummary[]> {
  return invokeOrPreview("list_settings", {}, () => browserSettings.map(summarizeSetting));
}

export function upsertSoul(soul: Soul): Promise<SoulSummary> {
  return invokeOrPreview("upsert_soul", { soul }, () => {
    const index = browserSouls.findIndex((item) => item.character_id === soul.character_id);
    if (index >= 0) {
      browserSouls[index] = soul;
    } else {
      browserSouls.unshift(soul);
    }
    return summarizeSoul(soul);
  });
}

export function upsertSetting(setting: SettingSoul): Promise<SettingSummary> {
  return invokeOrPreview("upsert_setting", { setting }, () => {
    const index = browserSettings.findIndex((item) => item.setting_id === setting.setting_id);
    if (index >= 0) {
      browserSettings[index] = setting;
    } else {
      browserSettings.unshift(setting);
    }
    return summarizeSetting(setting);
  });
}

export function getSoul(soulId: string): Promise<Soul> {
  return invokeOrPreview("get_soul", { soulId }, () => {
    const soul = browserSouls.find((item) => item.character_id === soulId);
    if (!soul) throw new Error("Soul not found");
    return soul;
  });
}

export function getSetting(settingId: string): Promise<SettingSoul> {
  return invokeOrPreview("get_setting", { settingId }, () => {
    const setting = browserSettings.find((item) => item.setting_id === settingId);
    if (!setting) throw new Error("Setting not found");
    return setting;
  });
}

export function deleteSoul(soulId: string): Promise<boolean> {
  return invokeOrPreview("delete_soul", { soulId }, () => {
    const beforeCount = browserSouls.length;
    browserSouls = browserSouls.filter((item) => item.character_id !== soulId);
    browserMessages = browserMessages.filter(
      (message) => message.conversation_id !== previewConversationIdForSoul(soulId),
    );
    return browserSouls.length !== beforeCount;
  });
}

export function deleteSetting(settingId: string): Promise<boolean> {
  return invokeOrPreview("delete_setting", { settingId }, () => {
    const beforeCount = browserSettings.length;
    browserSettings = browserSettings.filter((item) => item.setting_id !== settingId);
    browserMessages = browserMessages.filter(
      (message) => !message.conversation_id.startsWith(`local-mock-${settingId}-`),
    );
    return browserSettings.length !== beforeCount;
  });
}

export function sendMockTurn(
  conversationId: string,
  soulId: string,
  userText: string,
  mode: string,
  replacementAssistantId?: number,
): Promise<TurnResult> {
  return invokeOrPreview("send_mock_turn", { conversationId, soulId, userText, mode, replacementAssistantId: replacementAssistantId ?? null }, () =>
    sendPreviewTurn(conversationId, soulId, userText, mode, replacementAssistantId),
  );
}

export function sendApiTurn(
  conversationId: string,
  soulId: string,
  userText: string,
  mode: string,
  settings: ApiProviderSettings,
  signal?: AbortSignal,
  replacementAssistantId?: number,
): Promise<TurnResult> {
  return invokeOrPreview(
    "send_api_turn",
    { conversationId, soulId, userText, mode, settings, replacementAssistantId: replacementAssistantId ?? null },
    () => sendPreviewApiTurn(conversationId, soulId, userText, mode, settings, signal, replacementAssistantId),
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

export function listProviderProfiles(): Promise<ProviderProfile[]> {
  return invokeOrPreview("list_provider_profiles", {}, () => browserProviderProfiles);
}

export function getProviderProfile(profileId: string): Promise<ProviderProfile> {
  return invokeOrPreview("get_provider_profile", { profileId }, () => {
    const profile = browserProviderProfiles.find((item) => item.id === profileId);
    if (!profile) throw new Error("Provider profile not found");
    return profile;
  });
}

export function upsertProviderProfile(profile: ProviderProfile): Promise<ProviderProfile> {
  return invokeOrPreview("upsert_provider_profile", { profile }, () => {
    const now = Math.floor(Date.now() / 1000);
    const saved = { ...profile, created_at: profile.created_at || now, updated_at: now };
    const index = browserProviderProfiles.findIndex((item) => item.id === profile.id);
    if (index >= 0) {
      browserProviderProfiles[index] = saved;
    } else {
      browserProviderProfiles.unshift(saved);
    }
    return saved;
  });
}

export function deleteProviderProfile(profileId: string): Promise<boolean> {
  return invokeOrPreview("delete_provider_profile", { profileId }, () => {
    const before = browserProviderProfiles.length;
    browserProviderProfiles = browserProviderProfiles.filter((item) => item.id !== profileId);
    return browserProviderProfiles.length !== before;
  });
}

export function listConversationMessages(conversationId: string): Promise<ChatMessage[]> {
  return invokeOrPreview("list_conversation_messages", { conversationId }, () =>
    browserMessages.filter((message) => message.conversation_id === conversationId),
  );
}

export function deleteConversation(conversationId: string): Promise<boolean> {
  return invokeOrPreview("delete_conversation", { conversationId }, () => {
    const beforeCount = browserMessages.length;
    browserMessages = browserMessages.filter(
      (message) => message.conversation_id !== conversationId,
    );
    return browserMessages.length !== beforeCount;
  });
}

export function deleteMessage(conversationId: string, messageId: number): Promise<boolean> {
  return invokeOrPreview("delete_message", { conversationId, messageId }, () => {
    const beforeCount = browserMessages.length;
    browserMessages = browserMessages.filter(
      (message) => !(message.conversation_id === conversationId && message.id === messageId),
    );
    return browserMessages.length !== beforeCount;
  });
}

export function compileContext(
  soulId: string,
  conversationId: string,
): Promise<ContextPreview> {
  return invokeOrPreview("compile_context", { soulId, conversationId }, () => {
    const soul = browserSouls.find((item) => item.character_id === soulId);
    if (!soul) throw new Error("Soul not found");
    return compilePreviewContext(soul, conversationId);
  });
}

export function runConsolidation(soulId: string): Promise<Soul> {
  return invokeOrPreview("run_consolidation", { soulId }, () => {
    const soul = browserSouls.find((item) => item.character_id === soulId);
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

function makePreviewSoul(characterName: string): Soul {
  const now = Math.floor(Date.now() / 1000);
  return {
    schema_version: 1,
    character_id: crypto.randomUUID(),
    character_name: characterName.trim() || "Unnamed Character",
    profile: {
      description: "",
      appearance: "",
      personality: "",
      scenario: "",
    },
    last_updated: now,
    turn_counter: 0,
    turns_since_consolidation: 0,
    global: {
      dev_stage: 6,
      attach_style: 2,
      fear_baseline: 15,
      resolve: 40,
      shame: 45,
      openness: 45,
      maslow: [60, 50, 40, 30, 20],
      sdt: [70, 40, 10],
    },
    trauma: {
      phase: 2,
      symptoms: {
        hypervigilance: 10,
        flashbacks: 10,
        numbing: 10,
        avoidance: 10,
      },
    },
    arousal: {
      body_sex: "Female",
      phase: "Neutral",
      level: 0,
      frustration: 0,
      sensitivity: 1,
      refractory_turns_remaining: 0,
      orgasm_count: 0,
      denied_peak_turns: 0,
    },
    relationships: {
      user: {
        trust: 10,
        affection: 200,
        intimacy: 10,
        passion: 10,
        commitment: 10,
        fear: 10,
        desire: 20,
        love_type: "",
      },
    },
    memory: {
      core: ["The Soul file has just been initialized; enduring identity is still forming."],
      recent: [],
      schemas: [],
    },
    world: {
      location: "Unspecified starting scene.",
      active_plots: ["Establish the first scene"],
      recent_events: [],
      key_objects: [],
      time_elapsed: "Session start",
    },
  };
}

function makePreviewSetting(settingName: string): SettingSoul {
  return {
    schema_version: 1,
    setting_id: crypto.randomUUID(),
    setting_name: settingName.trim() || "Untitled Setting",
    last_updated: Math.floor(Date.now() / 1000),
    turn_counter: 0,
    world: {
      location: "Unspecified starting scene.",
      active_plots: ["Establish the first scene"],
      recent_events: [],
      key_objects: [],
      time_elapsed: "Session start",
    },
  };
}

function summarizeSoul(soul: Soul): SoulSummary {
  return {
    character_id: soul.character_id,
    character_name: soul.character_name,
    last_updated: soul.last_updated,
    recent_count: soul.memory.recent.length,
    core_count: soul.memory.core.length,
  };
}

function summarizeSetting(setting: SettingSoul): SettingSummary {
  return {
    setting_id: setting.setting_id,
    setting_name: setting.setting_name,
    last_updated: setting.last_updated,
    turn_counter: setting.turn_counter,
    location: setting.world.location,
  };
}

function sendPreviewTurn(
  conversationId: string,
  soulId: string,
  userText: string,
  mode: string,
  replacementAssistantId?: number,
): TurnResult {
  let soul = browserSouls.find((item) => item.character_id === soulId);
  if (!soul) {
    soul = makePreviewSoul("Aurora Schwarz");
    browserSouls.push(soul);
  }

  if (!replacementAssistantId) {
    browserMessages.push(makePreviewMessage(conversationId, "user", userText));
  }
  const template = previewTemplateFor(classifyPreviewTag(userText));
  const visibleResponse = renderPreviewResponse(soul, userText, template, mode);
  upsertPreviewAssistantMessage(conversationId, visibleResponse, replacementAssistantId);
  const debug = debugFromHiddenState(
    "Mock",
    {
      tag: template.tag,
      trust_delta: template.trustDelta,
      affection_delta: template.affectionDelta,
      present_characters: [soul.character_name],
    },
    true,
    false,
  );

  const relationship = soul.relationships.user;
  relationship.trust = Math.min(300, relationship.trust + template.trustDelta);
  relationship.affection = Math.min(300, relationship.affection + template.affectionDelta);
  soul.turn_counter += 1;
  soul.turns_since_consolidation += 1;
  soul.memory.recent.unshift({
    id: `mem_${crypto.randomUUID()}`,
    timestamp: Math.floor(Date.now() / 1000),
    content: `${template.memoryFrame} for ${soul.character_name}. User turn: ${userText}.`,
    salience: template.salience,
    tag: template.tag,
    retrieval_strength: template.salience,
  });
  soul.memory.recent = soul.memory.recent.slice(0, 12);
  soul.world.recent_events.push(`${template.worldFrame}: ${userText}`);
  soul.world.recent_events = soul.world.recent_events.slice(-12);
  soul.last_updated = Math.floor(Date.now() / 1000);

  const consolidation_ran = soul.turns_since_consolidation >= CONSOLIDATION_INTERVAL_TURNS;
  if (consolidation_ran) consolidatePreviewSoul(soul);

  return {
    conversation_id: conversationId,
    soul,
    visible_response: visibleResponse,
    context_preview: compilePreviewContext(soul, conversationId),
    messages: browserMessages.filter((message) => message.conversation_id === conversationId),
    consolidation_ran,
    debug,
  };
}

async function sendPreviewApiTurn(
  conversationId: string,
  soulId: string,
  userText: string,
  mode: string,
  settings: ApiProviderSettings,
  signal?: AbortSignal,
  replacementAssistantId?: number,
): Promise<TurnResult> {
  const soul = browserSouls.find((item) => item.character_id === soulId);
  if (!soul) throw new Error("Soul not found");
  if (!settings.api_key.trim()) throw new Error("API key is required for API provider mode");
  if (!settings.model.trim()) throw new Error("Model is required for API provider mode");
  if (!settings.base_url.trim()) throw new Error("Base URL is required for API provider mode");

  if (!replacementAssistantId) {
    browserMessages.push(makePreviewMessage(conversationId, "user", userText));
  }
  const context = compilePreviewContext(soul, conversationId);
  const response = await fetch(chatCompletionsUrl(settings.base_url), {
    method: "POST",
    signal,
    headers: {
      Authorization: `Bearer ${settings.api_key.trim()}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      model: settings.model.trim(),
      temperature: 0.85,
      messages: [
        {
          role: "system",
          content: buildNarratorSystemPrompt(settings.system_prompt, mode, soul, context.text),
        },
        { role: "user", content: userText },
      ],
    }),
  });

  if (!response.ok) {
    throw new Error(`API request failed with ${response.status}: ${await response.text()}`);
  }

  const body = await response.json();
  const rawResponse = body?.choices?.[0]?.message?.content?.trim();
  if (!rawResponse) throw new Error("API response did not include assistant content");
  const parsed = parsePreviewHiddenState(rawResponse);
  const hiddenStateFound = parsed.hiddenState !== null;
  const hiddenState = parsed.hiddenState ?? generatedPreviewApiHiddenState(soul, userText, parsed.visibleText);
  const visibleResponse = parsed.visibleText;
  upsertPreviewAssistantMessage(conversationId, visibleResponse, replacementAssistantId);

  const template = previewTemplateFor(normalizePreviewTag(hiddenState.tag, userText));
  const relationship = soul.relationships.user;
  relationship.trust = Math.min(
    300,
    relationship.trust + (hiddenState.trust_delta ?? template.trustDelta),
  );
  relationship.affection = Math.min(
    300,
    relationship.affection + (hiddenState.affection_delta ?? template.affectionDelta),
  );
  soul.turn_counter += 1;
  soul.turns_since_consolidation += 1;
  soul.memory.recent.unshift({
    id: `mem_${crypto.randomUUID()}`,
    timestamp: Math.floor(Date.now() / 1000),
    content:
      hiddenState.memory ||
      `${soul.character_name} responded through the API provider after the user said: ${userText}.`,
    salience: template.salience,
    tag: template.tag,
    retrieval_strength: template.salience,
  });
  soul.memory.recent = soul.memory.recent.slice(0, 12);
  soul.world.recent_events.push(
    hiddenState.world_event || `The API-driven exchange moved around: ${userText}`,
  );
  soul.world.recent_events = soul.world.recent_events.slice(-12);
  if (hiddenState.new_location?.trim()) {
    soul.world.location = hiddenState.new_location.trim();
  }
  soul.last_updated = Math.floor(Date.now() / 1000);

  const consolidation_ran = soul.turns_since_consolidation >= CONSOLIDATION_INTERVAL_TURNS;
  if (consolidation_ran) consolidatePreviewSoul(soul);

  return {
    conversation_id: conversationId,
    soul,
    visible_response: visibleResponse,
    context_preview: compilePreviewContext(soul, conversationId),
    messages: browserMessages.filter((message) => message.conversation_id === conversationId),
    consolidation_ran,
    debug: debugFromHiddenState("API", hiddenState, hiddenStateFound, !hiddenStateFound),
  };
}

function upsertPreviewAssistantMessage(
  conversationId: string,
  content: string,
  replacementAssistantId?: number,
) {
  if (replacementAssistantId) {
    const message = browserMessages.find(
      (item) =>
        item.conversation_id === conversationId &&
        item.id === replacementAssistantId &&
        item.role === "assistant",
    );
    if (message) {
      message.content = content;
      message.created_at = Math.floor(Date.now() / 1000);
      return;
    }
  }
  browserMessages.push(makePreviewMessage(conversationId, "assistant", content));
}

function makePreviewMessage(
  conversationId: string,
  role: ChatMessage["role"],
  content: string,
): ChatMessage {
  return {
    id: nextMessageId++,
    conversation_id: conversationId,
    role,
    content,
    created_at: Math.floor(Date.now() / 1000),
  };
}

function consolidatePreviewSoul(soul: Soul) {
  const promoted = soul.memory.recent.filter((memory) => memory.retrieval_strength > 70);
  for (const memory of promoted) {
    soul.memory.core.push(`${memory.tag.replace(/_/g, " ")}: ${memory.content}`);
  }

  const middle = soul.memory.recent.filter(
    (memory) => memory.retrieval_strength >= 30 && memory.retrieval_strength <= 70,
  );
  const byTag = new Map<string, RecentMemory[]>();
  for (const memory of middle) {
    byTag.set(memory.tag, [...(byTag.get(memory.tag) ?? []), memory]);
  }
  for (const [tag, memories] of byTag.entries()) {
    if (memories.length >= 3) {
      soul.memory.schemas.push({
        schema_type: tag,
        summary: `${tag.replace(/_/g, " ")} recurring pattern across ${memories.length} memories.`,
        count: memories.length,
      });
    }
  }

  soul.memory.recent = middle
    .sort((left, right) => right.salience - left.salience)
    .slice(0, 4);
  soul.turns_since_consolidation = 0;
  soul.last_updated = Math.floor(Date.now() / 1000);
}

function compilePreviewContext(soul: Soul, conversationId: string): ContextPreview {
  const recentEvents = soul.world.recent_events.slice(-5).map((event) => `- ${event}`);
  const recentChat = browserMessages
    .filter((message) => message.conversation_id === conversationId)
    .slice(-5)
    .map((message) => `${message.role}: ${message.content}`);
  const profileLines = [
    soul.profile.description ? `Description: ${soul.profile.description}` : "",
    soul.profile.appearance ? `Appearance: ${soul.profile.appearance}` : "",
    soul.profile.personality ? `Personality: ${soul.profile.personality}` : "",
    soul.profile.scenario ? `Scenario: ${soul.profile.scenario}` : "",
  ].filter(Boolean);
  let text = [
    `[CURRENT STATE]\nLocation: ${soul.world.location}\nActive Plot: ${soul.world.active_plots.join(". ")}\nTime: ${soul.world.time_elapsed}.`,
    profileLines.length ? `[CHARACTER PROFILE]\n${profileLines.join("\n")}` : "",
    `[CHARACTER MEMORY]\n${soul.memory.core.slice(0, 5).map((memory) => `Core: ${memory}`).join("\n")}`,
    `[RECENT EVENTS]\n${recentEvents.join("\n") || "- No major recent events yet."}`,
    `[RELATIONSHIP]\nTrust toward user: ${soul.relationships.user.trust}. Affection: ${soul.relationships.user.affection}. Fear: ${soul.relationships.user.fear}. Desire: ${soul.relationships.user.desire}.`,
    `[AROUSAL]\nArousal: ${soul.arousal.phase} phase, level ${Math.round(soul.arousal.level)}/100, frustration ${Math.round(soul.arousal.frustration)}/100, sensitivity ${soul.arousal.sensitivity.toFixed(2)}, refractory ${soul.arousal.refractory_turns_remaining} turns.`,
    recentChat.length ? `[RECENT CHAT]\n${recentChat.join("\n")}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");
  let truncated = false;
  while (estimateTokens(text) > 2000) {
    truncated = true;
    text = text.slice(0, text.lastIndexOf("\n"));
  }
  return { text, estimated_tokens: estimateTokens(text), truncated };
}

function estimateTokens(text: string) {
  return Math.max(1, Math.floor(text.length / 4));
}

type PreviewTag = "trust_building" | "threat" | "bonding" | "orientation" | "observation";

type PreviewTemplate = {
  tag: PreviewTag;
  trustDelta: number;
  affectionDelta: number;
  salience: number;
  readerLine: string;
  realisticLine: string;
  godLine: string;
  memoryFrame: string;
  worldFrame: string;
};

function classifyPreviewTag(text: string): PreviewTag {
  const lower = text.toLowerCase();
  if (lower.includes("trust") || lower.includes("promise") || lower.includes("safe")) {
    return "trust_building";
  }
  if (lower.includes("hurt") || lower.includes("blood") || lower.includes("danger")) {
    return "threat";
  }
  if (lower.includes("remember") || lower.includes("childhood") || lower.includes("together")) {
    return "bonding";
  }
  if (lower.includes("where") || lower.includes("look") || lower.includes("room")) {
    return "orientation";
  }
  return "observation";
}

function normalizePreviewTag(tag: string | undefined, fallbackText: string): PreviewTag {
  if (
    tag === "trust_building" ||
    tag === "threat" ||
    tag === "bonding" ||
    tag === "orientation" ||
    tag === "observation"
  ) {
    return tag;
  }
  if (tag === "intimacy") return "bonding";
  if (tag === "boundary_setting" || tag === "conflict_minor" || tag === "trauma_trigger") {
    return "threat";
  }
  if (tag === "breakthrough") return "trust_building";
  return classifyPreviewTag(fallbackText);
}

function generatedPreviewApiHiddenState(
  soul: Soul,
  userText: string,
  visibleText: string,
): HiddenStatePayload {
  const tag = classifyPreviewTag(userText);
  const template = previewTemplateFor(tag);
  return {
    memory: `${soul.character_name} responded through the API provider after the user said: ${userText}. Assistant cue: ${visibleText.slice(0, 180).trim()}`,
    tag,
    trust_delta: template.trustDelta,
    affection_delta: template.affectionDelta,
    world_event: `The API-driven exchange moved around: ${userText}`,
    present_characters: [soul.character_name],
  };
}

function debugFromHiddenState(
  provider: string,
  hiddenState: HiddenStatePayload,
  hiddenStateFound: boolean,
  fallbackHiddenStateGenerated: boolean,
): TurnDebug {
  return {
    provider,
    hidden_state_found: hiddenStateFound,
    fallback_hidden_state_generated: fallbackHiddenStateGenerated,
    tag: hiddenState.tag ?? null,
    trust_delta: hiddenState.trust_delta ?? null,
    affection_delta: hiddenState.affection_delta ?? null,
    new_location: hiddenState.new_location?.trim() || null,
    present_characters: hiddenState.present_characters ?? [],
  };
}

function previewTemplateFor(tag: PreviewTag): PreviewTemplate {
  const templates: Record<PreviewTag, PreviewTemplate> = {
    trust_building: {
      tag,
      trustDelta: 3,
      affectionDelta: 1,
      salience: 73,
      readerLine: "The promise lands softly; she tests whether it can hold weight.",
      realisticLine: "She studies the promise for a long second before letting her shoulders loosen.",
      godLine: "Trust advances, but only by a careful increment; the scene remains emotionally fragile.",
      memoryFrame: "A safety promise shifted the emotional baseline",
      worldFrame: "A small promise of safety changed the room's emotional pressure",
    },
    threat: {
      tag,
      trustDelta: 0,
      affectionDelta: 0,
      salience: 64,
      readerLine: "Her attention snaps sharp, and old alarm-patterns wake behind her eyes.",
      realisticLine: "She goes still and starts cataloging exits, distance, and anything that could become cover.",
      godLine: "Threat pressure rises; immediate survival logic begins overriding softer goals.",
      memoryFrame: "A perceived danger forced a defensive read of the scene",
      worldFrame: "The scene tightened around a possible danger",
    },
    bonding: {
      tag,
      trustDelta: 1,
      affectionDelta: 3,
      salience: 70,
      readerLine: "The shared thread of memory draws guarded warmth into her posture.",
      realisticLine: "She lets the memory sit between you, guarded but visibly affected by it.",
      godLine: "Bonding increases; shared memory becomes a usable emotional anchor.",
      memoryFrame: "A shared memory created a warmer bond",
      worldFrame: "The exchange became more intimate through remembered detail",
    },
    orientation: {
      tag,
      trustDelta: 1,
      affectionDelta: 0.5,
      salience: 60,
      readerLine: "She follows the details carefully, building a map from each concrete cue.",
      realisticLine: "She asks for specifics, anchoring herself in location, exits, and visible objects.",
      godLine: "Orientation improves; the character has more usable scene information.",
      memoryFrame: "New scene information improved orientation",
      worldFrame: "The scene gained clearer spatial definition",
    },
    observation: {
      tag,
      trustDelta: 1,
      affectionDelta: 1,
      salience: 55,
      readerLine: "She listens, not fully relaxed, but present enough to stay in the exchange.",
      realisticLine: "She acknowledges the turn with measured focus and keeps the exchange grounded.",
      godLine: "A neutral exchange is recorded; no major state axis shifts dramatically.",
      memoryFrame: "A neutral exchange added texture to the relationship",
      worldFrame: "The conversation continued without a major rupture",
    },
  };
  return templates[tag];
}

function renderPreviewResponse(
  soul: Soul,
  userText: string,
  template: PreviewTemplate,
  mode: string,
) {
  const normalizedMode = mode.toLowerCase();
  const line =
    normalizedMode === "god"
      ? template.godLine
      : normalizedMode === "realistic"
        ? template.realisticLine
        : template.readerLine;
  const responseNote = userText.endsWith("?")
    ? "The question narrows her attention; uncertainty stays visible, but she keeps tracking the scene."
    : "The turn lands; she absorbs it without retreating from the moment.";

  if (normalizedMode === "god") {
    return `${line}\n\n${soul.character_name} steadies in the scene. ${responseNote}`;
  }
  if (normalizedMode === "realistic") {
    return `${line}\n\n${soul.character_name} answers only through visible restraint: a controlled breath, a lowered chin, eyes measuring the room. ${responseNote}`;
  }
  return `${line}\n\n${soul.character_name}'s awareness stays close to the surface of the scene. ${responseNote}`;
}

function previewConversationIdForSoul(soulId: string) {
  return `local-mock-${soulId}`;
}

function chatCompletionsUrl(baseUrl: string) {
  const trimmed = baseUrl.trim().replace(/\/+$/, "");
  return trimmed.endsWith("/chat/completions") ? trimmed : `${trimmed}/chat/completions`;
}

function buildNarratorSystemPrompt(
  customPrompt: string,
  mode: string,
  soul: Soul,
  context: string,
) {
  const trimmedCustom = customPrompt.trim();
  const base =
    mode === "Custom" && trimmedCustom
      ? `${trimmedCustom}\n\n${HIDDEN_STATE_FORMAT_PROMPT}`
      : `${NARRATOR_SYSTEM_PROMPT}\n\n${MODE_PROMPTS[mode] ?? MODE_PROMPTS.Reader}`;
  return `${base}\n\nCharacter: ${soul.character_name}\n\n${context}`;
}

function parsePreviewHiddenState(raw: string): {
  visibleText: string;
  hiddenState: HiddenStatePayload | null;
} {
  const startMarker = "[HIDDEN STATE]";
  const endMarker = "[/HIDDEN STATE]";
  const start = raw.indexOf(startMarker);
  if (start < 0) return { visibleText: raw.trim(), hiddenState: null };

  const hiddenStart = start + startMarker.length;
  const end = raw.indexOf(endMarker, hiddenStart);
  const json = raw.slice(hiddenStart, end >= 0 ? end : undefined).trim();
  try {
    return {
      visibleText: raw.slice(0, start).trim(),
      hiddenState: JSON.parse(json) as HiddenStatePayload,
    };
  } catch {
    return { visibleText: raw.slice(0, start).trim(), hiddenState: null };
  }
}
