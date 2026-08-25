export type AssistantDisplay = {
  prose: string;
  status: string | null;
};

export function splitAssistantDisplay(content: string): AssistantDisplay {
  const withoutHiddenState = stripHiddenStateBlocks(content);
  const statusBlocks = [
    ...withoutHiddenState.matchAll(/```status\s*\n?([\s\S]*?)(?:```|$)/gi),
  ];
  const lastStatus = statusBlocks[statusBlocks.length - 1]?.[1]?.trim() || null;
  const prose = withoutHiddenState
    .replace(/```status\s*\n?[\s\S]*?(?:```|$)/gi, "")
    .trimEnd();

  return {
    prose,
    status: lastStatus,
  };
}

function stripHiddenStateBlocks(content: string) {
  let cleaned = content;
  cleaned = cleaned.replace(/\[HIDDEN STATE\][\s\S]*?(?:\[\/HIDDEN STATE\]|$)/g, "");
  cleaned = cleaned.replace(/\[HIDDEN STATE[\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[HIDDEN_STATE\][\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[HIDDEN_STATE[\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[\/HIDDEN STATE[\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[\/HIDDEN_STATE[\s\S]*$/g, "");
  cleaned = cleaned.replace(/\[\s*$/g, "");
  return cleaned.trimEnd();
}
