import { useLayoutEffect, useRef, useState } from "react";

import type { ChatMessage } from "../../tauri";
import type { ActiveGeneration } from "./model/messageLifecycle";

type ChatControllerOptions = {
  active: boolean;
  conversationId: string;
  messages: ChatMessage[];
};

export function useChatController({
  active,
  conversationId,
  messages,
}: ChatControllerOptions) {
  const generationAbortRef = useRef<AbortController | null>(null);
  const generationIdRef = useRef(0);
  const activeGenerationRef = useRef<ActiveGeneration | null>(null);
  const chatOnlyBodyRef = useRef<HTMLElement>(null);
  const chatBottomRef = useRef<HTMLDivElement>(null);
  const isPinnedToBottomRef = useRef(true);
  const [showJumpToLatest, setShowJumpToLatest] = useState(false);
  const [chatMoreMenuOpen, setChatMoreMenuOpen] = useState(false);
  const chatMoreMenuRef = useRef<HTMLDivElement | null>(null);
  const chatMoreButtonRef = useRef<HTMLButtonElement | null>(null);
  const personaModalRef = useRef<HTMLDivElement | null>(null);

  function scrollChatToBottom() {
    const body = chatOnlyBodyRef.current;
    if (!body) return;
    body.scrollTop = body.scrollHeight;
    chatBottomRef.current?.scrollIntoView({ block: "end" });
  }

  function handleChatScroll() {
    const body = chatOnlyBodyRef.current;
    if (!body) return;
    const distanceFromBottom = body.scrollHeight - body.scrollTop - body.clientHeight;
    const pinned = distanceFromBottom <= 80;
    isPinnedToBottomRef.current = pinned;
    setShowJumpToLatest((previous) => (previous === !pinned ? previous : !pinned));
  }

  function jumpToLatest() {
    isPinnedToBottomRef.current = true;
    setShowJumpToLatest(false);
    scrollChatToBottom();
  }

  useLayoutEffect(() => {
    if (!active) return;
    const body = chatOnlyBodyRef.current;
    if (!body) return;
    isPinnedToBottomRef.current = true;
    setShowJumpToLatest(false);
    let secondFrame = 0;
    scrollChatToBottom();
    const frame = window.requestAnimationFrame(() => {
      scrollChatToBottom();
      secondFrame = window.requestAnimationFrame(scrollChatToBottom);
    });
    return () => {
      window.cancelAnimationFrame(frame);
      window.cancelAnimationFrame(secondFrame);
    };
  }, [active, conversationId]);

  useLayoutEffect(() => {
    if (!active || !isPinnedToBottomRef.current) return;
    scrollChatToBottom();
  }, [active, messages]);

  return {
    generationAbortRef,
    generationIdRef,
    activeGenerationRef,
    chatOnlyBodyRef,
    chatBottomRef,
    isPinnedToBottomRef,
    showJumpToLatest,
    setShowJumpToLatest,
    chatMoreMenuOpen,
    setChatMoreMenuOpen,
    chatMoreMenuRef,
    chatMoreButtonRef,
    personaModalRef,
    scrollChatToBottom,
    handleChatScroll,
    jumpToLatest,
  };
}
