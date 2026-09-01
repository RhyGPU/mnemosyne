export type SlashCommandSuggestion = {
  command: string;
  usage: string;
  description: string;
};

export const SLASH_COMMANDS: SlashCommandSuggestion[] = [
  {
    command: "/ooc",
    usage: "/ooc <message>",
    description: "Talk outside RP with the command LLM.",
  },
  {
    command: "/setup",
    usage: "/setup <scene setup>",
    description: "Stage setup for the next RP turn without mutating canon.",
  },
  {
    command: "/state",
    usage: "/state show [target] | /state update <slot> <value> | /state review",
    description:
      "Inspect or correct state. Slots: location, scene, focus, position, outfit, room_state, active_object, misunderstanding, open_question, pressure_point, last_action — or knows/suspects/believes/unaware/hiding with \"<who> : <what>\".",
  },
  {
    command: "/persona",
    usage: "/persona list | lookup | change | add | edit",
    description: "Manage the user-controlled RP persona.",
  },
  {
    command: "/ask",
    usage: "/ask [plan|apply|diff] <request>",
    description: "Ask the Soul/state edit agent to inspect or edit state files.",
  },
  {
    command: "/help",
    usage: "/help",
    description: "Show command help.",
  },
];

export function leadingSlashToken(input: string) {
  const leadingWhitespace = input.match(/^\s*/)?.[0] ?? "";
  const trimmedStart = input.slice(leadingWhitespace.length);
  if (!trimmedStart.startsWith("/")) return null;
  const tokenEnd = trimmedStart.search(/\s/);
  const token = tokenEnd === -1 ? trimmedStart : trimmedStart.slice(0, tokenEnd);
  const body = tokenEnd === -1 ? "" : trimmedStart.slice(tokenEnd);
  return {
    leadingWhitespace,
    token,
    body,
    hasBodyText: body.trim().length > 0,
  };
}

export function filteredSlashCommands(input: string) {
  const parsed = leadingSlashToken(input);
  if (!parsed) return [];
  const token = parsed.token.toLowerCase();
  if (token === "/") return SLASH_COMMANDS;
  return SLASH_COMMANDS.filter((item) => item.command.toLowerCase().startsWith(token));
}

export function shouldOpenSlashMenu(input: string) {
  const parsed = leadingSlashToken(input);
  if (!parsed) return false;
  if (parsed.body.length > 0) return false;
  return filteredSlashCommands(input).length > 0;
}

export function canEnterSelectSlashSuggestion(input: string) {
  const parsed = leadingSlashToken(input);
  return Boolean(parsed && !parsed.hasBodyText);
}

export function nextSlashCommandIndex(currentIndex: number, commandCount: number, direction: -1 | 1) {
  if (commandCount <= 0) return 0;
  return (currentIndex + direction + commandCount) % commandCount;
}

export function completeSlashCommandInput(input: string, command: string) {
  const parsed = leadingSlashToken(input);
  if (!parsed) return `${command} `;
  const body = parsed.body.trimStart();
  return `${parsed.leadingWhitespace}${command} ${body}`.trimEnd() + " ";
}
