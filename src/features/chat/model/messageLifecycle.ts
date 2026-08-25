import type { ChatMessage } from "../../../tauri";

export type ActiveGeneration = {
  id: number;
  conversationId: string;
  narratorSaved: boolean;
  knownAssistantIds: Set<number>;
  replacementAssistantId?: number;
  replacementOriginalContent?: string;
};

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

export function seedStreamingTurn(
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

export function appendStreamingChunk(
  messages: ChatMessage[],
  conversationId: string,
  chunk: string,
) {
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

export function upsertSavedChatMessage(messages: ChatMessage[], savedMessage: ChatMessage) {
  const existingIndex = messages.findIndex(
    (message) =>
      message.conversation_id === savedMessage.conversation_id && message.id === savedMessage.id,
  );
  let pendingAssistantReplacedBySaved = 0;
  if (existingIndex >= 0) {
    const next = [...messages];
    next[existingIndex] = savedMessage;
    const cleaned = removeDuplicateStreamingAssistants(
      next,
      savedMessage.conversation_id,
      savedMessage.id,
    );
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
        const cleaned = removeDuplicateStreamingAssistants(
          next,
          savedMessage.conversation_id,
          savedMessage.id,
        );
        return {
          messages: cleaned,
          trace: messageRenderTrace(
            cleaned,
            messages.length - cleaned.length,
            pendingAssistantReplacedBySaved,
          ),
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
    trace: messageRenderTrace(
      cleaned,
      messages.length + 1 - cleaned.length,
      pendingAssistantReplacedBySaved,
    ),
  };
}

function pendingMatchesSavedAssistant(
  pending: ChatMessage,
  savedMessage: ChatMessage,
  activeGeneration?: ActiveGeneration | null,
) {
  if (!pending.pending && pending.id > 0) return false;
  if (pending.assistant_message_id && pending.assistant_message_id === savedMessage.id) return true;
  if (pending.request_id && savedMessage.request_id && pending.request_id === savedMessage.request_id) {
    return true;
  }
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

export function prepareMessagesForRender(
  messages: ChatMessage[],
  activeListenerCount: number = 0,
) {
  const active = messages.filter((message) => message.role !== "system");
  const saved = active.filter((message) => message.id > 0 && !message.pending);
  const pending = active.filter((message) => message.pending || message.id < 0);

  const dedupedSaved: ChatMessage[] = [];
  const seenSavedIds = new Set<number>();
  const seenSavedRequestIds = new Set<string>();
  let duplicateSavedSuppressed = 0;
  let duplicateSavedDbAssistantDetected = false;

  for (const message of saved) {
    if (seenSavedIds.has(message.id)) {
      duplicateSavedSuppressed += 1;
      continue;
    }
    seenSavedIds.add(message.id);

    if (message.request_id) {
      if (seenSavedRequestIds.has(message.request_id)) {
        if (message.role === "assistant") {
          duplicateSavedDbAssistantDetected = true;
        } else {
          duplicateSavedSuppressed += 1;
          continue;
        }
      }
      seenSavedRequestIds.add(message.request_id);
    }

    const closeDuplicate = dedupedSaved.some(
      (existing) =>
        existing.role === message.role &&
        existing.content === message.content &&
        Math.abs(existing.created_at - message.created_at) < 10,
    );
    if (closeDuplicate) {
      const matchingMessage = dedupedSaved.find(
        (existing) => existing.content === message.content,
      );
      if (message.role === "assistant" && message.id !== matchingMessage?.id) {
        duplicateSavedDbAssistantDetected = true;
      } else {
        duplicateSavedSuppressed += 1;
        continue;
      }
    }

    dedupedSaved.push(message);
  }

  const visiblePending: ChatMessage[] = [];
  let duplicatePendingSuppressed = 0;
  let pendingReplacedBySaved = 0;

  for (const pendingMessage of pending) {
    const hasMatchingSaved = dedupedSaved.some((savedMessage) => {
      if (
        pendingMessage.assistant_message_id &&
        pendingMessage.assistant_message_id === savedMessage.id
      ) {
        return true;
      }
      if (
        pendingMessage.request_id &&
        savedMessage.request_id &&
        pendingMessage.request_id === savedMessage.request_id
      ) {
        return true;
      }
      return (
        pendingMessage.role === savedMessage.role &&
        Boolean(pendingMessage.content.trim()) &&
        contentHash(pendingMessage.content) === contentHash(savedMessage.content)
      );
    });

    if (hasMatchingSaved) {
      pendingReplacedBySaved += 1;
      continue;
    }

    const isPendingDuplicate = visiblePending.some(
      (existing) =>
        (pendingMessage.assistant_message_id &&
          existing.assistant_message_id &&
          pendingMessage.assistant_message_id === existing.assistant_message_id) ||
        (pendingMessage.request_id &&
          existing.request_id &&
          pendingMessage.request_id === existing.request_id) ||
        (pendingMessage.role === existing.role &&
          pendingMessage.content === existing.content &&
          Math.abs(pendingMessage.created_at - existing.created_at) < 10),
    );
    if (isPendingDuplicate) {
      duplicatePendingSuppressed += 1;
      continue;
    }

    visiblePending.push(pendingMessage);
  }

  const rendered = [...dedupedSaved, ...visiblePending].sort((left, right) => {
    if (left.id < 0 && right.id > 0) return 1;
    if (left.id > 0 && right.id < 0) return -1;
    if (left.id < 0 && right.id < 0) return left.created_at - right.created_at;
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

  return { messages: rendered, trace };
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
  if (message.role === "assistant" && message.pending && message.content.trim()) {
    return "streaming_overlay";
  }
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
    assistant_message_id:
      message.assistant_message_id ??
      (message.role === "assistant" && message.id > 0 ? message.id : undefined),
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
  const pendingAssistantCount = rendered.filter(
    (message) => message.role === "assistant" && message.pending,
  ).length;
  const renderedSavedMessageCount = rendered.filter(
    (message) => message.id > 0 && !message.pending,
  ).length;
  const renderedPendingMessageCount = rendered.filter(
    (message) => message.pending || message.id < 0,
  ).length;
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
      options.duplicateSavedSuppressed +
      options.duplicatePendingSuppressed +
      options.pendingReplacedBySaved,
    duplicate_visual_pair: visibleBubbleTrace.some((row) => row.duplicate_visual_pair),
    duplicate_saved_db_assistant_detected:
      options.duplicateSavedDbAssistantDetected ?? false,
    visible_bubble_trace: visibleBubbleTrace,
  };
}

export function reconcilePersistedMessages(
  current: ChatMessage[],
  persisted: ChatMessage[],
) {
  if (!persisted.length) return current;
  const persistedConversationId = persisted[0].conversation_id;
  return [
    ...current.filter((message) => message.conversation_id !== persistedConversationId),
    ...persisted,
  ].sort((left, right) => left.id - right.id);
}

export function hasSavedAssistantForGeneration(
  messages: ChatMessage[],
  activeGeneration: ActiveGeneration,
) {
  if (activeGeneration.replacementAssistantId) {
    const replacement = messages.find(
      (message) =>
        message.id === activeGeneration.replacementAssistantId &&
        message.role === "assistant",
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
      Boolean(message.content.trim()) &&
      !activeGeneration.knownAssistantIds.has(message.id),
  );
}

export function clearFailedStreamingTurn(
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
    if (message.id < 0) return [];
    return [message];
  });
}
