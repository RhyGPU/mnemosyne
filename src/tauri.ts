import { invoke } from "@tauri-apps/api/core";

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

export type SoulSummary = {
  character_id: string;
  character_name: string;
  last_updated: number;
  recent_count: number;
  core_count: number;
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
};

export type ContextPreview = {
  text: string;
  estimated_tokens: number;
  truncated: boolean;
};

let browserSouls: Soul[] = [];
let browserMessages: ChatMessage[] = [];
let nextMessageId = 1;

function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invokeOrPreview<T>(
  command: string,
  args: Record<string, unknown>,
  fallback: () => T,
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

export function listSouls(): Promise<SoulSummary[]> {
  return invokeOrPreview("list_souls", {}, () => browserSouls.map(summarizeSoul));
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

export function getSoul(soulId: string): Promise<Soul> {
  return invokeOrPreview("get_soul", { soulId }, () => {
    const soul = browserSouls.find((item) => item.character_id === soulId);
    if (!soul) throw new Error("Soul not found");
    return soul;
  });
}

export function sendMockTurn(
  conversationId: string,
  userText: string,
): Promise<TurnResult> {
  return invokeOrPreview("send_mock_turn", { conversationId, userText }, () =>
    sendPreviewTurn(conversationId, userText),
  );
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

export function saveSoulFile(path: string, soul: Soul): Promise<void> {
  return invokeOrPreview("save_soul_file", { path, soul }, () => {
    console.info(`Preview mode cannot write ${path}`, soul);
  });
}

function makePreviewSoul(characterName: string): Soul {
  const now = Math.floor(Date.now() / 1000);
  return {
    schema_version: 1,
    character_id: crypto.randomUUID(),
    character_name: characterName.trim() || "Unnamed Character",
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

function summarizeSoul(soul: Soul): SoulSummary {
  return {
    character_id: soul.character_id,
    character_name: soul.character_name,
    last_updated: soul.last_updated,
    recent_count: soul.memory.recent.length,
    core_count: soul.memory.core.length,
  };
}

function sendPreviewTurn(conversationId: string, userText: string): TurnResult {
  let soul = browserSouls[0];
  if (!soul) {
    soul = makePreviewSoul("Aurora Schwarz");
    browserSouls.push(soul);
  }

  browserMessages.push(makePreviewMessage(conversationId, "user", userText));
  const tag = classifyPreviewTag(userText);
  const visibleResponse = `${soul.character_name} listens closely, letting the moment settle before answering. "I heard you. That matters more than I expected."`;
  browserMessages.push(makePreviewMessage(conversationId, "assistant", visibleResponse));

  const relationship = soul.relationships.user;
  relationship.trust = Math.min(300, relationship.trust + (tag === "trust_building" ? 3 : 1));
  relationship.affection = Math.min(300, relationship.affection + (tag === "bonding" ? 3 : 1));
  soul.turn_counter += 1;
  soul.turns_since_consolidation += 1;
  soul.memory.recent.unshift({
    id: `mem_${crypto.randomUUID()}`,
    timestamp: Math.floor(Date.now() / 1000),
    content: `${soul.character_name} responded to the user's turn: ${userText}`,
    salience: tag === "trust_building" ? 73 : 55,
    tag,
    retrieval_strength: tag === "trust_building" ? 73 : 55,
  });
  soul.memory.recent = soul.memory.recent.slice(0, 12);
  soul.world.recent_events.push(`The exchange shifted around: ${userText}`);
  soul.world.recent_events = soul.world.recent_events.slice(-12);
  soul.last_updated = Math.floor(Date.now() / 1000);

  const assistantTurns = browserMessages.filter(
    (message) => message.conversation_id === conversationId && message.role === "assistant",
  ).length;
  const consolidation_ran =
    (assistantTurns > 0 && assistantTurns % 10 === 0) ||
    soul.turns_since_consolidation >= 10;
  if (consolidation_ran) consolidatePreviewSoul(soul);

  return {
    conversation_id: conversationId,
    soul,
    visible_response: visibleResponse,
    context_preview: compilePreviewContext(soul, conversationId),
    messages: browserMessages.filter((message) => message.conversation_id === conversationId),
    consolidation_ran,
  };
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
  let text = [
    `[CURRENT STATE]\nLocation: ${soul.world.location}\nActive Plot: ${soul.world.active_plots.join(". ")}\nTime: ${soul.world.time_elapsed}.`,
    `[CHARACTER MEMORY]\n${soul.memory.core.slice(0, 5).map((memory) => `Core: ${memory}`).join("\n")}`,
    `[RECENT EVENTS]\n${recentEvents.join("\n") || "- No major recent events yet."}`,
    `[RELATIONSHIP]\nTrust toward user: ${soul.relationships.user.trust}. Affection: ${soul.relationships.user.affection}. Fear: ${soul.relationships.user.fear}. Desire: ${soul.relationships.user.desire}.`,
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

function classifyPreviewTag(text: string) {
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
