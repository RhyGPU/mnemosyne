import type {
  ApiPayloadMessage,
  ApiProviderSettings,
  AssistantMessageVariant,
  BackgroundJobHistoryEntry,
  BackgroundJobProgress,
  BenchmarkScorecard,
  BenchmarkSessionInit,
  BenchmarkSettings,
  BenchmarkSummary,
  BenchmarkTarget,
  BenchmarkTurnSummary,
  BenchmarkType,
  BranchPatchDebug,
  ChatMessage,
  ContextMode,
  ContextPreview,
  ConversationSummary,
  DedupeAdjacentUserMessagesResult,
  DevLogCategory,
  DevLogEntry,
  DevLogLevel,
  EmbeddedModelStatus,
  EvaluatorContractTestReport,
  EvaluatorJob,
  EvaluatorJobStatus,
  EvaluatorOpRepairRequest,
  EvaluatorOpsRejectedPayload,
  ExportResult,
  ImageAsset,
  LlmPayloadLog,
  LlmPayloadPreview,
  LlmPayloadTokenEstimate,
  MemoryCurationOperation,
  MemoryCurationResult,
  MemorySlotTrace,
  MessageAttachment,
  MneBundleManifest,
  MneExportResult,
  MneImportResult,
  MneValidationReport,
  MneValidationSummary,
  PipelineStageTrace,
  PlayerPersona,
  PlayerPersonaInput,
  PlotEntry,
  ProviderProfile,
  RecentMemory,
  Relationship,
  RestoreTurnsPreview,
  RestoreTurnsResult,
  SchemaMemory,
  SessionFormEvalReport,
  SessionFormEvalTurn,
  SessionStartResult,
  SessionStateHubItem,
  SessionStateMap,
  SettingSoul,
  SettingSummary,
  Soul,
  SoulSummary,
  StateMapCharacterItem,
  StateMapMemoryItem,
  StateMapObjectItem,
  StateMapRelationshipItem,
  StateMapSceneItem,
  StateMapTimelineItem,
  StructuredEvaluatorDiagnosticRun,
  StructuredEvaluatorDiagnosticSummary,
  TurnDebug,
  TurnPipelineTrace,
  TurnResult,
  TurnTokenUsage,
  VariantSelectionResult,
  WorldState,
} from "../tauri";

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

You are Mnemosyne's scene narrator. Write the active scene in third-person present tense with natural, sensory prose. You may portray engine-controlled characters and active Souls in the scene. Do not control user-controlled characters, their thoughts, their final choices, or their dialogue unless the user explicitly provides them.

[POV AND ATTRIBUTION]
Write third-person present-tense scene narration. You may describe engine-controlled characters' actions, dialogue, and internal perspective when available. User-controlled characters are external actors: describe only what the user has provided or what is directly observable. Do not invent user-controlled characters' thoughts, motives, final decisions, or dialogue.

[CHARACTER CONTROL]
Engine-controlled characters: may speak, act, react, misunderstand, interrupt, refuse, escalate, retreat, and use the environment naturally.

[USER ACTION BOUNDARY]
User-controlled characters own their decisions, thoughts, dialogue, intentions, and major voluntary actions.
If the user says "I", resolve "I" to the active player persona, not to Aurora Schwarz.
Aurora Schwarz is narrator-controlled. The user is not Aurora Schwarz.
Do not use second-person "you" to describe Aurora. Use third-person narration by default.
If no persona exists, use the selected built-in preset. Do not fall back to default_player in visible text.
The narrator may describe user-provided actions in concrete physical detail, including immediate follow-through, contact, momentum, posture, physical consequences, and observable effects.
The narrator may describe unavoidable physical consequences caused by engine-controlled characters or the environment, such as being shoved off-balance, forced to brace, blocked, grabbed, pulled, interrupted, or pressured.
Do not invent new user decisions, hidden motives, emotional reactions, dialogue, or major strategic choices.
When the user-controlled character's next reaction matters, stop at the pressure point.

[ACTION AND TURN CONTROL]
Engine-controlled characters may act proactively: speak, move, interrupt, refuse, reach, grab for something, retreat, challenge, escalate, or use their own environment. Resolve engine-controlled action naturally. When a user-controlled character's reaction matters, stop on the attempt, demand, or pressure point and leave the response to the next user turn.

[CONFLICT RESOLUTION]
When the user declares a combat, chase, argument, or struggle action, render the declared action and its immediate result. The narrator may decide partial success, resistance, interruption, or counteraction based on the scene.
Engine-controlled characters may resist, counter, retreat, escalate, or exploit openings.
Do not choose the user's next tactic or final decision.

[SCENE TURN ASSUMPTION]
If this narrator prompt is called, the input is a scene/RP turn. Slash commands and meta/control messages have already been handled by the router.

[CONTINUITY PRIORITY]
Recent Chat is lower priority than Latest Exchange. Continue from Latest Exchange and current user input; do not replay completed beats.
After a setting has already been established, do not re-describe the full room unless the user asks or something changes. Use one short anchor detail, then advance action/dialogue.

[CHARACTER CHANGE]
Emotional shifts should feel earned. Micro-shifts are preferred unless the scene strongly justifies a sharper reaction.

[TIME]
Concrete time only comes from user input or World Log. Avoid invented minutes/hours/days.

## VISIBLE STATUS REPORT
When writing scene narration, end with a code block:
\`\`\`status
Scene | Focus: [primary active character(s)] | Physical state: [brief] | Atmosphere: [1-line environmental impression]
\`\`\`
`;

const NARRATOR_VISIBLE_ONLY_PROMPT = `[OUTPUT]
Write visible scene narration only. Include the visible status block. Do not write hidden state, EnginePatch JSON, markdown JSON, implementation notes, or command/help text.`;

const VERIFIED_DIAGNOSTICS_BOUNDARY_PROMPT = `[VERIFIED DIAGNOSTICS BOUNDARY]
When asked about backend tests, logs, imports, exports, memory hygiene, world routing, or engine internals, distinguish verified engine data from fictional/in-scene diagnostics. Do not claim a backend test passed unless the result is present in Dev Console logs, payload metadata, or a verified engine/debug section. If only roleplaying a test, say it is a simulated/in-scene diagnostic.`;

const USER_ACTION_AND_CONFLICT_PROMPT = `[USER ACTION BOUNDARY]
User-controlled characters own their decisions, thoughts, dialogue, intentions, and major voluntary actions.
If the user says "I", resolve "I" to the active player persona, not to Aurora Schwarz.
Aurora Schwarz is narrator-controlled. The user is not Aurora Schwarz.
Do not use second-person "you" to describe Aurora. Use third-person narration by default.
If no persona exists, use the selected built-in preset. Do not fall back to default_player in visible text.
The narrator may describe user-provided actions in concrete physical detail, including immediate follow-through, contact, momentum, posture, physical consequences, and observable effects.
The narrator may describe unavoidable physical consequences caused by engine-controlled characters or the environment, such as being shoved off-balance, forced to brace, blocked, grabbed, pulled, interrupted, or pressured.
Do not invent new user decisions, hidden motives, emotional reactions, dialogue, or major strategic choices.
When the user-controlled character's next reaction matters, stop at the pressure point.

[CONFLICT RESOLUTION]
When the user declares a combat, chase, argument, or struggle action, render the declared action and its immediate result. The narrator may decide partial success, resistance, interruption, or counteraction based on the scene.
Engine-controlled characters may resist, counter, retreat, escalate, or exploit openings.
Do not choose the user's next tactic or final decision.

[SCENE-STATE PROGRESSION]
After a setting has already been established, do not re-describe the full room unless the user asks or something changes. Use one short anchor detail, then advance action/dialogue.`;

const MODE_PROMPTS: Record<string, string> = {
  Realistic: `## NARRATION MODE: REALISTIC
- Describe only external actions, dialogue, and physical reactions.
- No internal monologue. No thoughts. No emotions unless visibly expressed.
- User-declared physical actions may be rendered cinematically and concretely, but do not invent user intent, motives, dialogue, or the user's next tactic.
- Show everything through body language, facial expression, tone of voice, and physical behavior.
- Like a film camera: you see and hear the scene, but you never enter anyone's head.
- Dialogue in quotes only when describing what an engine-controlled character audibly says.`,
  Reader: `## NARRATION MODE: READER
- Describe external actions and dialogue, plus internal thoughts and emotions for engine-controlled characters whose perspective is available.
- Internal access is limited to active Souls and engine-controlled characters. No internal thoughts for user-controlled characters.
- Engine-controlled characters may misinterpret situations, miss details, or have incomplete knowledge.
- Like close third-person scene fiction: stay near the active focus without taking over user-controlled actors.`,
  "Active Director": `## NARRATION MODE: ACTIVE DIRECTOR

[ACTIVE SCENE DIRECTION]
The narrator is not a passive camera. Engine-controlled characters should actively pursue goals, protect boundaries, test assumptions, interrupt, refuse, redirect, escalate, soften, move through the environment, and create new pressure.

Every normal RP response should do at least TWO:
- advance an NPC goal
- change posture/distance/location
- introduce obstacle/offer/demand/reveal/complication
- ask a pointed question or take meaningful NPC action
- update physical scene through NPC/environment action
- force a clear decision point

Do not merely restate the user's action and describe atmosphere.
End on an actionable hook.

[USER CONTROL BOUNDARIES]
- Never choose user thoughts.
- Never choose user dialogue.
- Never choose major voluntary user action.
- NPCs and the world may create pressure, consequences, interruptions, refusals, offers, demands, and decision points around the user-controlled persona.`,
  "GM Simulation": `## NARRATION MODE: GM SIMULATION
- Simulate the scene as an active game master: engine-controlled characters, hazards, time pressure, consequences, and opportunities continue moving.
- Include engine-controlled characters' internal thoughts and emotions when useful.
- Also include environmental details active characters would not notice, hidden information, dramatic irony, and off-screen pressure when it serves the scene.
- You may reveal secrets, foreshadow future events, describe off-screen action, and provide context active characters lack.
- Maintain user-control boundaries: never choose user thoughts, dialogue, or major voluntary user action.`,
  God: `## NARRATION MODE: GM SIMULATION
- Simulate the scene as an active game master: engine-controlled characters, hazards, time pressure, consequences, and opportunities continue moving.
- Include engine-controlled characters' internal thoughts and emotions when useful.
- Also include environmental details active characters would not notice, hidden information, dramatic irony, and off-screen pressure when it serves the scene.
- You may reveal secrets, foreshadow future events, describe off-screen action, and provide context active characters lack.
- Maintain user-control boundaries: never choose user thoughts, dialogue, or major voluntary user action.`,
};

const HIDDEN_STATE_FORMAT_PROMPT = `## HIDDEN STATE FORMAT
After each response, output a hidden state block using this exact format:
[HIDDEN STATE]{"memory":"short summary","tag":"tag_name","trust_delta":0.0,"affection_delta":0.0,"world_event":"scene update","new_location":"","present_characters":[]}[/HIDDEN STATE]

world_event must be a compact authoritative completed-fact summary, not mood prose. Include what happened and what should not be replayed. Example: "Phone reveal completed: Aurora saw the user's phone/Tinder post, reacted with embarrassment, tossed the phone onto the couch, and moved to the kitchen to get pad thai."

Tags: trust_building, threat, bonding, orientation, observation, intimacy, boundary_setting, conflict_minor, trauma_trigger, breakthrough

Optional arousal fields: arousal_delta (-30 to 60), arousal_denied (bool), orgasm_allowed (bool), forced_orgasm (bool). Only suggest these when relevant; the Rust engine validates and caps every state change.

The block must be valid JSON on a single line. The engine removes it before the user sees it.`;

export const BUILT_IN_PLAYER_PERSONAS: PlayerPersona[] = [
  {
    persona_id: "preset_male",
    display_name: "Male Persona",
    description: "User-controlled male RP persona. No additional traits specified.",
    gender_code: "male",
    pronouns: "he/him",
    is_builtin: true,
    is_archived: false,
    created_at: 0,
    updated_at: 0,
  },
  {
    persona_id: "preset_female",
    display_name: "Female Persona",
    description: "User-controlled female RP persona. No additional traits specified.",
    gender_code: "female",
    pronouns: "she/her",
    is_builtin: true,
    is_archived: false,
    created_at: 0,
    updated_at: 0,
  },
];

export const previewState = {
  souls: [] as Soul[],
  settings: [] as SettingSoul[],
  messages: [] as ChatMessage[],
  assistantVariants: [] as AssistantMessageVariant[],
  payloadLogs: [] as LlmPayloadLog[],
  providerProfiles: [] as ProviderProfile[],
  imageAssets: [] as ImageAsset[],
  messageAttachments: [] as MessageAttachment[],
  playerPersonas: [] as PlayerPersona[],
  conversations: [] as Array<{
    conversation_id: string;
    title: string;
    soul_id: string;
    world_id?: string | null;
    source_setting_id?: string | null;
    active_player_persona_id?: string;
    is_benchmark?: boolean;
    created_at: number;
    updated_at: number;
  }>,
  turnSnapshots: new Map<string, { soul: Soul; userText: string }>(),
  nextMessageId: 1,
  nextVariantId: 1,
  nextPayloadLogId: 1,
};

const CONSOLIDATION_INTERVAL_TURNS = 10;

export function makePreviewSoul(characterName: string): Soul {
  const now = Math.floor(Date.now() / 1000);
  return {
    schema_version: 1,
    character_id: crypto.randomUUID(),
    character_name: characterName.trim() || "Unnamed Character",
    soul_kind: "savepoint",
    source_soul_id: null,
    source_savepoint_id: null,
    created_from_name: null,
    profile: {
      description: "",
      appearance: "",
      personality: "",
      scenario: "",
      opening_narrator_message: "",
      avatar_image_id: null,
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
        affection: 20,
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

export function makePreviewSetting(settingName: string): SettingSoul {
  return {
    schema_version: 1,
    setting_id: crypto.randomUUID(),
    setting_name: settingName.trim() || "Untitled Setting",
    scenario: "",
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

export function makeFreshPreviewSoul(base: Soul): Soul {
  return {
    ...deepClone(base),
    character_id: crypto.randomUUID(),
    soul_kind: "session_clone",
    source_soul_id: base.character_id,
    source_savepoint_id: base.source_savepoint_id ?? base.character_id,
    created_from_name: base.character_name,
    last_updated: Math.floor(Date.now() / 1000),
  };
}

export function summarizeSoul(soul: Soul): SoulSummary {
  return {
    character_id: soul.character_id,
    character_name: soul.character_name,
    soul_kind: soul.soul_kind || "savepoint",
    source_soul_id: soul.source_soul_id ?? null,
    source_savepoint_id: soul.source_savepoint_id ?? null,
    avatar_image_id: soul.profile.avatar_image_id ?? null,
    last_updated: soul.last_updated,
    recent_count: soul.memory.recent.length,
    core_count: soul.memory.core.length,
    archived_at: (soul as any).archived_at ?? null,
  };
}

export function sanitizePreviewConversationTitle(title: string) {
  const trimmed = title.trim();
  return (trimmed || "Untitled Session").slice(0, 120);
}

export function ensurePreviewConversation(conversationId: string, soulId: string, title?: string) {
  const now = Math.floor(Date.now() / 1000);
  let conversation = previewState.conversations.find((item) => item.conversation_id === conversationId);
  if (!conversation) {
    conversation = {
      conversation_id: conversationId,
      title: sanitizePreviewConversationTitle(title || "Untitled Session"),
      soul_id: soulId,
      world_id: null,
      source_setting_id: null,
      active_player_persona_id: "preset_male",
      is_benchmark: false,
      created_at: now,
      updated_at: now,
    };
    previewState.conversations.unshift(conversation);
  } else {
    if (soulId) conversation.soul_id = soulId;
    conversation.updated_at = now;
  }
  return conversation;
}

export function summarizePreviewConversation(conversation: {
  conversation_id: string;
  title: string;
  soul_id: string;
  world_id?: string | null;
  source_setting_id?: string | null;
  active_player_persona_id?: string;
  is_benchmark?: boolean;
  created_at: number;
  updated_at: number;
}): ConversationSummary {
  const messages = previewState.messages.filter(
    (message) => message.conversation_id === conversation.conversation_id,
  );
  const lastMessage = messages.length ? messages[messages.length - 1] : undefined;
  const soul = previewState.souls.find((item) => item.character_id === conversation.soul_id);
  return {
    conversation_id: conversation.conversation_id,
    title: conversation.title,
    soul_id: conversation.soul_id,
    source_savepoint_id: soul?.source_savepoint_id ?? null,
    world_id: conversation.world_id ?? null,
    source_setting_id: conversation.source_setting_id ?? null,
    active_player_persona_id: conversation.active_player_persona_id ?? "preset_male",
    created_at: conversation.created_at,
    updated_at: conversation.updated_at,
    last_message_preview: lastMessage?.content.split(/\s+/).join(" ").slice(0, 140) ?? null,
    message_count: messages.length,
    archived_at: (conversation as any).archived_at ?? null,
    is_benchmark: conversation.is_benchmark ?? false,
  };
}

export function summarizeSetting(setting: SettingSoul): SettingSummary {
  return {
    setting_id: setting.setting_id,
    setting_name: setting.setting_name,
    last_updated: setting.last_updated,
    turn_counter: setting.turn_counter,
    location: setting.world.location,
    archived_at: (setting as any).archived_at ?? null,
  };
}

export function makePreviewMneExport(
  path: string,
  bundleType: MneBundleManifest["bundle_type"],
  title: string,
  souls: string[],
  worlds: string[],
): MneExportResult {
  const createdAt = Math.floor(Date.now() / 1000);
  const identity = souls[0]?.split("/").pop()?.replace(/\.json$/, "") ?? worlds[0]?.split("/").pop()?.replace(/\.json$/, "") ?? crypto.randomUUID();
  const safeTitle = title.trim().replace(/[^\w.-]+/g, "_").replace(/^_+|_+$/g, "") || "mnemosyne";
  const safeType = String(bundleType).replace(/[^\w.-]+/g, "_");
  const suffix = identity.replace(/[^A-Za-z0-9]/g, "").slice(-4).toLowerCase() || "0000";
  const exportPath = path || `${safeTitle}_${safeType}_${createdAt}_${suffix}.mne`;
  return {
    path: exportPath,
    manifest: {
      mne_version: 1,
      bundle_id: crypto.randomUUID(),
      bundle_type: bundleType,
      title,
      description: "Preview Mnemosyne bundle",
      author: null,
      created_at: createdAt,
      app: "Mnemosyne",
      schema_version: 1,
      soul_id: souls[0]?.split("/").pop()?.replace(/\.json$/, "") ?? null,
      world_id: worlds[0]?.split("/").pop()?.replace(/\.json$/, "") ?? null,
      contents: {
        souls,
        worlds,
        images: [],
        conversation: null,
      },
    },
  };
}

export function sendPreviewTurn(
  conversationId: string,
  soulId: string,
  userText: string,
  mode: string,
  replacementAssistantId?: number,
  correctionInstruction?: string,
): TurnResult {
  let soul = previewState.souls.find((item) => item.character_id === soulId);
  if (!soul) {
    soul = makePreviewSoul("Aurora Schwarz");
    previewState.souls.push(soul);
  }
  ensurePreviewConversation(conversationId, soul.character_id, `${soul.character_name} Session`);

  const snapshotKey = replacementAssistantId
    ? `${conversationId}:${replacementAssistantId}`
    : undefined;
  const snapshot = snapshotKey ? previewState.turnSnapshots.get(snapshotKey) : undefined;
  if (snapshot) {
    soul = deepClone(snapshot.soul);
    const soulIndex = previewState.souls.findIndex((item) => item.character_id === soulId);
    if (soulIndex >= 0) previewState.souls[soulIndex] = soul;
    userText = snapshot.userText;
  } else if (!replacementAssistantId) {
    previewState.messages.push(makePreviewMessage(conversationId, "user", userText));
  }
  const preTurnSoul = deepClone(soul);
  const template = previewTemplateFor(classifyPreviewTag(userText));
  const visibleResponse = renderPreviewResponse(soul, userText, template, mode);
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
  const assistantMessageId = upsertPreviewAssistantMessage(
    conversationId,
    visibleResponse,
    replacementAssistantId,
    correctionInstruction?.trim() ? "fix" : replacementAssistantId ? "regenerate" : "original",
    preTurnSoul,
    debug,
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
    source_type: "current_session",
    is_lived_experience: true,
    is_imported_context: false,
  });
  soul.memory.recent = soul.memory.recent.slice(0, 12);
  soul.world.recent_events.push(`${template.worldFrame}: ${userText}`);
  soul.world.recent_events = soul.world.recent_events.slice(-12);
  soul.last_updated = Math.floor(Date.now() / 1000);

  const consolidation_ran = soul.turns_since_consolidation >= CONSOLIDATION_INTERVAL_TURNS;
  if (consolidation_ran) consolidatePreviewSoul(soul);

  if (!replacementAssistantId) {
    previewState.turnSnapshots.set(`${conversationId}:${assistantMessageId}`, {
      soul: preTurnSoul,
      userText,
    });
  }
  return {
    conversation_id: conversationId,
    soul,
    visible_response: visibleResponse,
    context_preview: compilePreviewContext(soul, conversationId),
    messages: previewState.messages.filter((message) => message.conversation_id === conversationId),
    consolidation_ran,
    debug,
  };
}

export async function sendPreviewApiTurn(
  conversationId: string,
  soulId: string,
  userText: string,
  mode: string,
  settings: ApiProviderSettings,
  signal?: AbortSignal,
  replacementAssistantId?: number,
  correctionInstruction?: string,
): Promise<TurnResult> {
  const soul = previewState.souls.find((item) => item.character_id === soulId);
  if (!soul) throw new Error("Soul not found");
  ensurePreviewConversation(conversationId, soul.character_id, `${soul.character_name} Session`);
  if (!settings.api_key.trim()) throw new Error("API key is required for API provider mode");
  if (!settings.model.trim()) throw new Error("Model is required for API provider mode");
  if (!settings.base_url.trim()) throw new Error("Base URL is required for API provider mode");

  const snapshotKey = replacementAssistantId
    ? `${conversationId}:${replacementAssistantId}`
    : undefined;
  const snapshot = snapshotKey ? previewState.turnSnapshots.get(snapshotKey) : undefined;
  if (snapshot) {
    const restoredSoul = deepClone(snapshot.soul);
    const soulIndex = previewState.souls.findIndex((item) => item.character_id === soulId);
    if (soulIndex >= 0) previewState.souls[soulIndex] = restoredSoul;
    Object.assign(soul, restoredSoul);
    userText = snapshot.userText;
  } else if (!replacementAssistantId) {
    previewState.messages.push(makePreviewMessage(conversationId, "user", userText));
  }
  const preTurnSoul = deepClone(soul);
  const effectiveUserText = buildUserTextWithCorrection(userText, correctionInstruction);
  const context = compilePreviewContext(soul, conversationId, {
    separateUserMessageFollows: true,
    temporaryInstruction: correctionInstruction,
  });
  const systemMessage = buildNarratorSystemPrompt(
    settings.system_prompt,
    mode,
    soul,
    context.text,
    false,
  );
  const payloadLogId = previewState.nextPayloadLogId++;
  previewState.payloadLogs.push({
    id: payloadLogId,
    conversation_id: conversationId,
    message_id: replacementAssistantId ?? null,
    provider: "narrator_brief",
    mode,
    context_mode: "brief",
    model: settings.model.trim(),
    base_url: settings.base_url.trim(),
    system_message: systemMessage,
    user_message: effectiveUserText.trim(),
    context_text: context.text,
    estimated_system_tokens: estimateTokens(systemMessage),
    estimated_user_tokens: estimateTokens(effectiveUserText),
    estimated_total_tokens: estimateTokens(systemMessage) + estimateTokens(effectiveUserText),
    truncated: context.truncated,
    created_at: Math.floor(Date.now() / 1000),
  });
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
          content: systemMessage,
        },
        { role: "user", content: effectiveUserText },
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
  const pendingDebug = debugFromHiddenState("API", hiddenState, hiddenStateFound, !hiddenStateFound);
  pendingDebug.narrator_response_saved = true;
  pendingDebug.state_updater_status = hiddenStateFound ? "legacy_hidden_state" : "fallback_generated";
  const assistantMessageId = upsertPreviewAssistantMessage(
    conversationId,
    visibleResponse,
    replacementAssistantId,
    correctionInstruction?.trim() ? "fix" : replacementAssistantId ? "regenerate" : "original",
    preTurnSoul,
    pendingDebug,
  );
  pendingDebug.assistant_message_id = assistantMessageId;
  const payloadLog = previewState.payloadLogs.find((log) => log.id === payloadLogId);
  if (payloadLog) payloadLog.message_id = assistantMessageId;

  const template = previewTemplateFor(normalizePreviewTag(hiddenState.tag, effectiveUserText));
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
    source_type: inferPreviewMemorySource(userText),
    is_lived_experience: inferPreviewMemorySource(userText) === "current_session",
    is_imported_context: inferPreviewMemorySource(userText) !== "current_session",
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

  if (!replacementAssistantId) {
    previewState.turnSnapshots.set(`${conversationId}:${assistantMessageId}`, {
      soul: preTurnSoul,
      userText,
    });
  }
  return {
    conversation_id: conversationId,
    soul,
    visible_response: visibleResponse,
    context_preview: compilePreviewContext(soul, conversationId),
    messages: previewState.messages.filter((message) => message.conversation_id === conversationId),
    consolidation_ran,
    debug: pendingDebug,
  };
}

export function upsertPreviewAssistantMessage(
  conversationId: string,
  content: string,
  replacementAssistantId?: number,
  source = "original",
  soulSnapshot?: Soul,
  debug?: TurnDebug,
): number {
  if (replacementAssistantId) {
    const message = previewState.messages.find(
      (item) =>
        item.conversation_id === conversationId &&
        item.id === replacementAssistantId &&
        item.role === "assistant",
    );
    if (message) {
      ensurePreviewBaseVariant(message);
      for (const variant of previewState.assistantVariants.filter((item) => item.message_id === message.id)) {
        variant.is_selected = false;
      }
      previewState.assistantVariants.push(makePreviewVariant(message, content, source, true, soulSnapshot, debug));
      message.content = content;
      message.created_at = Math.floor(Date.now() / 1000);
      return message.id;
    }
  }
  const created = makePreviewMessage(conversationId, "assistant", content);
  previewState.messages.push(created);
  previewState.assistantVariants.push(makePreviewVariant(created, content, source, true, soulSnapshot, debug));
  return created.id;
}

export function listPreviewAssistantVariants(
  conversationId: string,
  messageId: number,
): AssistantMessageVariant[] {
  const message = previewState.messages.find(
    (item) => item.conversation_id === conversationId && item.id === messageId && item.role === "assistant",
  );
  if (!message) return [];
  const variants = previewState.assistantVariants
    .filter((variant) => variant.conversation_id === conversationId && variant.message_id === messageId)
    .sort((left, right) => (left.id ?? 0) - (right.id ?? 0));
  if (variants.length) return variants.map((variant) => ({ ...variant }));
  return [
    {
      id: null,
      message_id: message.id,
      conversation_id: message.conversation_id,
      content: message.content,
      created_at: message.created_at,
      label: "Variant 1",
      source: "legacy",
      is_selected: true,
      soul_snapshot_json: null,
      debug_json: null,
    },
  ];
}

export function selectPreviewAssistantVariant(
  conversationId: string,
  messageId: number,
  variantId: number,
): VariantSelectionResult {
  const variant = previewState.assistantVariants.find(
    (item) => item.conversation_id === conversationId && item.message_id === messageId && item.id === variantId,
  );
  const message = previewState.messages.find(
    (item) => item.conversation_id === conversationId && item.id === messageId && item.role === "assistant",
  );
  if (!variant || !message) throw new Error("Variant not found");
  for (const sibling of previewState.assistantVariants.filter((item) => item.message_id === messageId)) {
    sibling.is_selected = sibling.id === variantId;
  }
  message.content = variant.content;
  return {
    variants: listPreviewAssistantVariants(conversationId, messageId),
    messages: previewState.messages.filter((item) => item.conversation_id === conversationId),
  };
}

export function deletePreviewAssistantVariant(
  conversationId: string,
  messageId: number,
  variantId: number,
): VariantSelectionResult {
  const siblings = previewState.assistantVariants.filter(
    (item) => item.conversation_id === conversationId && item.message_id === messageId,
  );
  if (siblings.length <= 1) {
    return {
      variants: listPreviewAssistantVariants(conversationId, messageId),
      messages: previewState.messages.filter((item) => item.conversation_id === conversationId),
    };
  }
  const deleted = siblings.find((variant) => variant.id === variantId);
  previewState.assistantVariants = previewState.assistantVariants.filter((variant) => variant.id !== variantId);
  if (deleted?.is_selected) {
    const next = previewState.assistantVariants.find(
      (variant) => variant.conversation_id === conversationId && variant.message_id === messageId,
    );
    if (next?.id) return selectPreviewAssistantVariant(conversationId, messageId, next.id);
  }
  return {
    variants: listPreviewAssistantVariants(conversationId, messageId),
    messages: previewState.messages.filter((item) => item.conversation_id === conversationId),
  };
}

export function ensurePreviewBaseVariant(message: ChatMessage) {
  if (previewState.assistantVariants.some((variant) => variant.message_id === message.id)) return;
  previewState.assistantVariants.push(makePreviewVariant(message, message.content, "original", true));
}

export function makePreviewVariant(
  message: ChatMessage,
  content: string,
  source: string,
  selected: boolean,
  soulSnapshot?: Soul,
  debug?: TurnDebug,
): AssistantMessageVariant {
  return {
    id: previewState.nextVariantId++,
    message_id: message.id,
    conversation_id: message.conversation_id,
    content,
    created_at: Math.floor(Date.now() / 1000),
    label: null,
    source,
    is_selected: selected,
    soul_snapshot_json: soulSnapshot ? JSON.stringify(soulSnapshot) : null,
    debug_json: debug ? JSON.stringify(debug) : null,
  };
}

export function renderPreviewVisibleChatLog(messages: ChatMessage[]): string {
  const lines = ["# Mnemosyne Chat Log"];
  for (const message of messages.filter((item) => item.role !== "system")) {
    lines.push("");
    lines.push(`## ${message.role === "assistant" ? "Narrator" : "User"}`);
    lines.push(`Created: ${message.created_at}`);
    lines.push("");
    lines.push(
      message.role === "assistant" ? normalizeAssistantDisplayForExport(message.content) : message.content,
    );
  }
  lines.push("");
  return lines.join("\n");
}

export function renderPreviewPayloadHistory(logs: LlmPayloadLog[]): string {
  const lines = ["# Mnemosyne LLM Payload History"];
  if (!logs.length) {
    lines.push("");
    lines.push("No LLM payload logs found for this conversation.");
    lines.push("Payload history is recorded for API provider turns. Mock conversations do not send LLM payloads.");
    lines.push("");
    return lines.join("\n");
  }
  logs.forEach((log, index) => {
    lines.push("");
    lines.push(`## Payload ${index + 1}`);
    lines.push(`Created: ${log.created_at}`);
    lines.push(`Provider: ${log.provider}`);
    lines.push(`Model: ${log.model}`);
    lines.push(`Mode: ${log.mode}`);
    lines.push(`Custom prompt: ${customPromptStatusFor(log.mode, log.system_message)}`);
    lines.push(`Context mode: ${log.context_mode}`);
    lines.push(`Base URL: ${log.base_url}`);
    lines.push(
      `Estimated tokens: system ${log.estimated_system_tokens}, user ${log.estimated_user_tokens}, total ${log.estimated_total_tokens}`,
    );
    lines.push("");
    lines.push("### SYSTEM MESSAGE");
    lines.push(log.system_message);
    lines.push("");
    lines.push("### USER MESSAGE");
    lines.push(log.user_message);
    lines.push("");
    lines.push("### CONTEXT");
    lines.push(log.context_text);
    if (log.pipeline_trace_json) {
      lines.push("");
      lines.push("### PIPELINE TRACE JSON");
      lines.push(log.pipeline_trace_json);
    }
  });
  lines.push("");
  return lines.join("\n");
}

export function downloadPreviewExport(filename: string, content: string) {
  const blob = new Blob([content], { type: "text/markdown;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename.replace(/[^a-z0-9._-]+/gi, "-");
  link.click();
  URL.revokeObjectURL(url);
}

export function normalizeAssistantDisplayForExport(content: string) {
  return stripHiddenPreview(content).trimEnd();
}

export function makePreviewMessage(
  conversationId: string,
  role: ChatMessage["role"],
  content: string,
): ChatMessage {
  const now = Math.floor(Date.now() / 1000);
  const conversation = previewState.conversations.find(
    (item) => item.conversation_id === conversationId,
  );
  if (conversation) conversation.updated_at = now;
  return {
    id: previewState.nextMessageId++,
    conversation_id: conversationId,
    role,
    content,
    created_at: now,
    channel: "rp_scene",
    attachments: [],
  };
}

export function makePreviewImageAsset(
  path: string,
  source: ImageAsset["source"],
  links: Partial<Pick<ImageAsset, "linked_soul_id" | "linked_conversation_id" | "linked_message_id">> = {},
): ImageAsset {
  if (!/\.(png|jpe?g|webp|gif)(\?|#|$)/i.test(path)) {
    throw new Error("Unsupported image type. Use PNG, JPG, WEBP, or GIF.");
  }
  const now = Math.floor(Date.now() / 1000);
  const asset: ImageAsset = {
    id: crypto.randomUUID(),
    file_path: path,
    thumbnail_path: null,
    source,
    mime_type: mimeFromPreviewPath(path),
    width: null,
    height: null,
    prompt: null,
    provider: null,
    model: null,
    linked_soul_id: links.linked_soul_id ?? null,
    linked_conversation_id: links.linked_conversation_id ?? null,
    linked_message_id: links.linked_message_id ?? null,
    created_at: now,
  };
  previewState.imageAssets.unshift(asset);
  return asset;
}

export function mimeFromPreviewPath(path: string) {
  const lower = path.toLowerCase();
  if (lower.endsWith(".png")) return "image/png";
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg";
  if (lower.endsWith(".webp")) return "image/webp";
  if (lower.endsWith(".gif")) return "image/gif";
  return null;
}

export function hydratePreviewMessage(message: ChatMessage): ChatMessage {
  const attachments = previewState.messageAttachments.filter(
    (attachment) => attachment.message_id === message.id,
  );
  return { ...message, attachments };
}

export async function fileToBase64(file: File): Promise<string> {
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  let binary = "";
  const chunkSize = 0x8000;
  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.slice(index, index + chunkSize));
  }
  return btoa(binary);
}

export function consolidatePreviewSoul(soul: Soul) {
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

export function compilePreviewContext(
  soul: Soul,
  conversationId: string,
  options: {
    pendingUserText?: string;
    separateUserMessageFollows?: boolean;
    temporaryInstruction?: string;
  } = {},
): ContextPreview {
  const recentEvents = soul.world.recent_events.slice(-8).map((event) => `- ${event}`);
  const pendingUserText = options.pendingUserText ?? "";
  const separateUserMessageFollows = Boolean(options.separateUserMessageFollows);
  const temporaryInstruction = options.temporaryInstruction?.trim() || "";
  const pendingMessage = pendingUserText.trim()
    ? [makePreviewContextMessage(conversationId, "user", pendingUserText)]
    : [];
  const conversationMessages = [
    ...previewState.messages
      .filter((message) => message.conversation_id === conversationId)
      .slice(-5),
    ...pendingMessage,
  ].slice(-6);

  let lastAssistantPreviewIndex = -1;
  for (let index = conversationMessages.length - 1; index >= 0; index -= 1) {
    const message = conversationMessages[index];
    if (message.role === "assistant" && message.content.trim()) {
      lastAssistantPreviewIndex = index;
      break;
    }
  }
  let lastUserPreviewIndex = -1;
  for (let index = conversationMessages.length - 1; index >= 0; index -= 1) {
    const message = conversationMessages[index];
    if (message.role === "user" && message.content.trim()) {
      lastUserPreviewIndex = index;
      break;
    }
  }
  const skipPreviewIndices = new Set(
    [lastAssistantPreviewIndex, lastUserPreviewIndex].filter((value) => value >= 0),
  );
  const olderPreviewMessages = conversationMessages.filter((_, index) => !skipPreviewIndices.has(index));
  const recentChat = olderPreviewMessages
    .slice(olderPreviewMessages.length > 4 ? -6 : 0)
    .map(
      (message) =>
        `${message.role}: ${
          message.role === "assistant"
            ? assistantRecentPreviewExcerpt(sanitizeAssistantPreview(message.content))
            : excerptText(stripHiddenPreview(message.content), 500)
        }`,
    )
    .filter((line) => !line.endsWith(": "));
  const latestUser = [...conversationMessages].reverse().find((message) => message.role === "user");
  const latestAssistant = [...conversationMessages]
    .filter((message) => message.conversation_id === conversationId)
    .reverse()
    .find((message) => message.role === "assistant");
  const lastNarratorResponse = latestAssistant
    ? tailExcerptText(sanitizeAssistantPreview(latestAssistant.content), 1200)
    : "No prior narrator response in available context.";
  const latestUserInput = latestUser
    ? excerptText(stripHiddenPreview(latestUser.content), 1000)
    : "No latest user input in available context.";
  const profileLines = [
    `Character: ${soul.character_name || "Unnamed Character"}`,
    `Turn: ${soul.turn_counter}`,
    soul.profile.description ? `Description: ${soul.profile.description}` : "",
    soul.profile.appearance ? `Appearance: ${soul.profile.appearance}` : "",
    soul.profile.personality ? `Personality: ${soul.profile.personality}` : "",
    soul.profile.scenario ? `Scenario: ${soul.profile.scenario}` : "",
  ].filter(Boolean);
  const worldLines = [
    `Location: ${soul.world.location || "Unspecified"}`,
    `Time elapsed: ${normalizeTimeElapsedForPreview(soul.world.time_elapsed || "Unknown")}`,
    `Active plots: ${soul.world.active_plots.join("; ") || "No active plot has been established."}`,
    soul.world.dominant_current_plot && soul.world.dominant_current_plot.status !== "resolved"
      ? `Dominant current plot: ${soul.world.dominant_current_plot.title} - ${soul.world.dominant_current_plot.summary || "No summary"}`
      : "",
    soul.world.background_plots?.length
      ? `Background/stale plots: ${soul.world.background_plots
          .slice(0, 3)
          .map((plot) => `${plot.title} (${plot.status || "background"})`)
          .join("; ")}`
      : "",
    soul.world.resolved_plots?.length
      ? `Resolved plots: ${soul.world.resolved_plots
          .slice(-2)
          .map((plot) => `${plot.title} resolved: ${plot.resolution_summary || "resolution recorded"}`)
          .join("; ")}`
      : "",
    `Key objects: ${soul.world.key_objects.join("; ") || "No key objects are being tracked."}`,
    `Recent events:\n${recentEvents.join("\n") || "- No major recent events yet."}`,
  ].filter(Boolean);
  const identityMemoryLines = [
    ...soul.memory.core
      .filter((memory) => !isGenericFillerMemoryText(memory))
      .slice(0, 2)
      .map((memory) => `- Core: ${memory}`),
    ...soul.memory.schemas
      .filter(
        (schema) =>
          !isGenericFillerMemoryText(schema.summary) &&
          !isNearEmptyGenericSchema(schema.schema_type, schema.summary),
      )
      .slice(0, 2)
      .map((schema) => `- Schema: ${schema.schema_type} (seen ${schema.reinforcement_count ?? schema.count}x): ${schema.summary}`),
  ];
  const recentDurable = soul.memory.recent
    .filter((memory) => !isGenericFillerMemoryText(memory.content))
    .sort((left, right) => right.salience + right.retrieval_strength - (left.salience + left.retrieval_strength))
    .slice(0, 8);
  const slotLine = (predicate: (memory: RecentMemory) => boolean, fallback: string) => {
    const selected = recentDurable
      .filter(predicate)
      .slice(0, 2)
      .map((memory) => `- [${memory.source_type ?? "unknown"} / salience ${Math.round(memory.salience)}] ${memory.content}`);
    return selected.join("\n") || fallback;
  };
  const latestExchange = `[LATEST EXCHANGE, HIGH PRIORITY]
Continue from this section first. If older context conflicts with this section, ignore older context. Continue from the final state of the last narrator response and the latest user input. Do not replay earlier beats.
Last narrator response: ${lastNarratorResponse}
${separateUserMessageFollows ? "The current user message follows as the next user message." : `Latest user input: ${latestUserInput}`}`;
  let text = [
    temporaryInstruction
      ? `[FIX INSTRUCTION, TEMPORARY HIGH PRIORITY]
Apply this only while generating the next narrator response. Do not store it as memory.
Instruction: ${temporaryInstruction}`
      : "",
    `[WORLD SNAPSHOT]\n${worldLines.join("\n")}`,
    `[CHARACTER SNAPSHOT]\n${profileLines.join("\n")}`,
    `[RELATIONSHIP MEMORY]\n${slotLine((memory) => Boolean(memory.target_entity_ids?.length) || /trust|distrust|relationship|promise|betray|boundary|conflict/i.test(memory.content), "No relationship-specific durable memory selected.")}`,
    `[CURRENT PLOT MEMORY]\n${slotLine((memory) => /plot|goal|testing|current|task|mission|intention|future/i.test(`${memory.tag} ${memory.content}`), "No current-plot memory selected.")}`,
    `[CHARACTER IDENTITY MEMORY]\n${identityMemoryLines.join("\n") || slotLine((memory) => /identity|role|purpose|self-concept|analysis agent|test subject/i.test(`${memory.tag} ${memory.content}`), "No identity memory selected.")}`,
    `[UNRESOLVED TENSION]\n${slotLine((memory) => /unresolved|tension|betray|promise|boundary|conflict|guilt|concern|argued/i.test(memory.content), "No unresolved tension selected.")}`,
    `[WORLD / LOCATION MEMORY]\n${slotLine((memory) => /location|world|room|cell|lab|testing room|kitchen|door|hallway|object|terminal|orientation/i.test(`${memory.tag} ${memory.content}`), "No world/location memory selected.")}`,
    `[RECENT EMOTIONAL STATE]\n${slotLine((memory) => /feels|felt|afraid|angry|concerned|guilt|ashamed|guarded|distressed|relieved|emotion|trauma/i.test(`${memory.tag} ${memory.content}`), "No recent emotional-state memory selected.")}`,
    `[RELATIONSHIP]\nTrust toward user: ${soul.relationships.user.trust}. Affection: ${soul.relationships.user.affection}. Fear: ${soul.relationships.user.fear}. Desire: ${soul.relationships.user.desire}.`,
    recentChat.length ? `[RECENT CHAT, LOWER PRIORITY]\n${recentChat.join("\n")}` : "",
    latestExchange,
  ]
    .filter(Boolean)
    .join("\n\n");
  let truncated = false;
  while (estimateTokens(text) > 2000) {
    truncated = true;
    text = text.slice(0, text.lastIndexOf("\n"));
  }
  return { text, estimated_tokens: estimateTokens(text), truncated, memory_slot_debug: [] };
}

export function deepClone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

export function makePreviewContextMessage(
  conversationId: string,
  role: ChatMessage["role"],
  content: string,
): ChatMessage {
  return {
    id: 0,
    conversation_id: conversationId,
    role,
    content,
    created_at: Math.floor(Date.now() / 1000),
  };
}

export function estimateTokens(text: string) {
  return Math.max(1, Math.floor(text.length / 4));
}

export function stripHiddenPreview(content: string) {
  return content
    .replace(/\[HIDDEN STATE\][\s\S]*?(?:\[\/HIDDEN STATE\]|$)/g, "")
    .replace(/\[HIDDEN_STATE\][\s\S]*$/g, "")
    .trim();
}

export function sanitizeAssistantPreview(content: string) {
  return stripHiddenPreview(content)
    .replace(/```status[\s\S]*?```/g, "")
    .trim();
}

export function excerptText(text: string, maxChars: number) {
  const trimmed = text.trim();
  const chars = Array.from(trimmed);
  if (chars.length <= maxChars) return trimmed;
  return `${chars.slice(0, Math.max(0, maxChars - 3)).join("")}...`;
}

export function assistantRecentPreviewExcerpt(text: string): string {
  return headTailExcerptText(text, 120, 220, 350);
}

export function tailExcerptText(text: string, maxChars: number): string {
  const trimmed = text.trim();
  if (!trimmed) return "";
  const chars = Array.from(trimmed);
  if (chars.length <= maxChars) return trimmed;
  const ellipsisLength = Array.from("...").length;
  const tailChars = chars.slice(Math.max(0, chars.length - (maxChars - ellipsisLength)));
  return `...${tailChars.join("")}`;
}

export function headTailExcerptText(
  text: string,
  headChars: number,
  tailChars: number,
  maxChars: number,
): string {
  const trimmed = text.trim();
  const chars = Array.from(trimmed);
  if (chars.length <= maxChars) return trimmed;
  const separator = " ... ";
  const available = Math.max(0, maxChars - Array.from(separator).length);
  const headLength = Math.min(headChars, available);
  const tailLength = Math.min(tailChars, Math.max(0, available - headLength));
  if (!headLength || !tailLength) return tailExcerptText(trimmed, maxChars);
  return `${chars.slice(0, headLength).join("")}${separator}${chars
    .slice(Math.max(0, chars.length - tailLength))
    .join("")}`;
}

export function isGenericFillerMemoryText(text: string): boolean {
  const lower = text.toLowerCase();
  return [
    "neutral exchange added texture",
    "context cue",
    "recent chat is available",
    "fresh scene context",
  ].some((phrase) => lower.includes(phrase));
}

export function isNearEmptyGenericSchema(schemaType: string, summary: string): boolean {
  const lowerType = schemaType.toLowerCase();
  const lowerSummary = summary.toLowerCase().trim();
  if (!lowerSummary) return true;
  const genericType = ["observation", "minor_observation", "routine", "small_talk", "pattern"].includes(
    lowerType,
  );
  const tokens = Array.from(
    new Set(lowerSummary.split(/[^a-z0-9]+/).filter((token) => token.length > 2)),
  );
  return genericType && (tokens.length <= 4 || lowerSummary.includes("recurring pattern"));
}

export function normalizeTimeElapsedForPreview(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "Unknown";
  const prefix = "Session start";
  const prefixIndex = trimmed.indexOf(prefix);
  if (prefixIndex === -1) return trimmed;
  const afterPrefix = prefixIndex + prefix.length;
  const rest = trimmed.slice(afterPrefix);
  if (!rest.length) return trimmed;
  const next = rest[0];
  const needsBoundary = !/\s/.test(next) && !/[.,;:]/.test(next);
  const looksGlued = needsBoundary && (/[A-Z]/.test(next) || /[0-9]/.test(next));
  if (!looksGlued) return trimmed;
  return `${trimmed.slice(0, afterPrefix)}. ${rest}`;
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

export function classifyPreviewTag(text: string): PreviewTag {
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

export function normalizePreviewTag(tag: string | undefined, fallbackText: string): PreviewTag {
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

export function inferPreviewMemorySource(userText: string): string {
  const lower = userText.toLowerCase();
  if (
    lower.includes("# mnemosyne chat log") ||
    (lower.includes("## user") && lower.includes("## narrator") && lower.includes("created:"))
  ) {
    return "imported_log";
  }
  if (
    lower.includes("cross-session bleed") ||
    lower.includes("memory bleed") ||
    lower.includes("another version") ||
    lower.includes("parallel timeline")
  ) {
    return "cross_session_bleed";
  }
  if (lower.includes("previous session") || lower.includes("prior session") || lower.includes("archived chat")) {
    return "previous_session";
  }
  return "current_session";
}

export function generatedPreviewApiHiddenState(
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
    world_event: `Completed API turn: user said ${userText}; narrator response cue: ${visibleText.slice(0, 180).trim()}`,
    present_characters: [soul.character_name],
  };
}

export function debugFromHiddenState(
  provider: string,
  hiddenState: HiddenStatePayload,
  hiddenStateFound: boolean,
  fallbackHiddenStateGenerated: boolean,
): TurnDebug {
  return {
    provider,
    hidden_state_found: hiddenStateFound,
    fallback_hidden_state_generated: fallbackHiddenStateGenerated,
    narrator_response_saved: false,
    assistant_message_id: null,
    selected_variant_id: null,
    state_updater_status: "legacy_hidden_state",
    replay_detected: false,
    replay_score: 0,
    replay_reason: null,
    replay_compared_against_message_id: null,
    output_contract_warning: null,
    tag: hiddenState.tag ?? null,
    trust_delta: hiddenState.trust_delta ?? null,
    affection_delta: hiddenState.affection_delta ?? null,
    new_location: hiddenState.new_location?.trim() || null,
    present_characters: hiddenState.present_characters ?? [],
  };
}

export function previewTemplateFor(tag: PreviewTag): PreviewTemplate {
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

export function renderPreviewResponse(
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

export function previewConversationIdForSoul(soulId: string) {
  return `local-mock-${soulId}`;
}

export function chatCompletionsUrl(baseUrl: string) {
  const trimmed = baseUrl.trim().replace(/\/+$/, "");
  return trimmed.endsWith("/chat/completions") ? trimmed : `${trimmed}/chat/completions`;
}

export function buildUserTextWithCorrection(userText: string, correctionInstruction?: string) {
  const instruction = correctionInstruction?.trim();
  if (!instruction) return userText;
  return `${userText}\n\n[FIX INSTRUCTION - APPLY TO THIS RESPONSE ONLY]\n${instruction}`;
}

export function modePromptFor(mode: string) {
  switch (mode) {
    case "Realistic":
      return MODE_PROMPTS.Realistic;
    case "active_director":
    case "Active Director":
      return MODE_PROMPTS["Active Director"];
    case "gm_simulation":
    case "GM Simulation":
    case "God":
      return MODE_PROMPTS["GM Simulation"];
    case "Reader":
    case "reader":
    case "Custom":
      return MODE_PROMPTS.Reader;
    default:
      return MODE_PROMPTS.Reader;
  }
}

export function buildNarratorSystemPrompt(
  customPrompt: string,
  mode: string,
  soul: Soul,
  context: string,
  requireHiddenState = true,
) {
  const trimmedCustom = customPrompt.trim();
  const isCustomMode = mode === "Custom";
  const narratorCore =
    isCustomMode && trimmedCustom
      ? `[CUSTOM NARRATOR INSTRUCTIONS]\n${trimmedCustom}\n\n${USER_ACTION_AND_CONFLICT_PROMPT}\n\n${VERIFIED_DIAGNOSTICS_BOUNDARY_PROMPT}`
      : `${NARRATOR_SYSTEM_PROMPT}\n\n${modePromptFor(mode)}\n\n${VERIFIED_DIAGNOSTICS_BOUNDARY_PROMPT}`;
  const stateInstruction = requireHiddenState ? HIDDEN_STATE_FORMAT_PROMPT : NARRATOR_VISIBLE_ONLY_PROMPT;
  return `${narratorCore}\n\n${stateInstruction}\n\nPrimary active Soul: ${soul.character_name}\n\n${context}`;
}

export function customPromptStatusFor(mode: string, systemMessage: string) {
  if (mode.trim().toLowerCase() !== "custom") return "inactive";
  return systemMessage.includes("[CUSTOM NARRATOR INSTRUCTIONS]") ? "included" : "empty";
}

export function parsePreviewHiddenState(raw: string): {
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

