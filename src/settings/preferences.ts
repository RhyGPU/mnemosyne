export type NarrativeMode =
  | "Realistic"
  | "Reader"
  | "Active Director"
  | "GM Simulation"
  | "Custom";

export type ChatStartMode = "continue" | "fresh";
export type SettingsTab = "ai" | "generation" | "reading" | "data" | "about";
export type GenerationPresetName = "balanced" | "creative" | "focused" | "custom";

export type GenerationPreferences = {
  preset: GenerationPresetName;
  temperature: number;
  topP: number;
  frequencyPenalty: number;
  presencePenalty: number;
  maxTokens: number | null;
};

export type ReadingPreferences = {
  proseSize: number;
  lineHeight: number;
  columnWidth: number;
  compactSpacing: boolean;
};

export const GENERATION_PRESETS: Record<
  Exclude<GenerationPresetName, "custom">,
  Omit<GenerationPreferences, "preset" | "maxTokens">
> = {
  balanced: {
    temperature: 0.85,
    topP: 0.95,
    frequencyPenalty: 0.1,
    presencePenalty: 0.1,
  },
  creative: {
    temperature: 1.1,
    topP: 0.98,
    frequencyPenalty: 0.05,
    presencePenalty: 0.25,
  },
  focused: {
    temperature: 0.65,
    topP: 0.9,
    frequencyPenalty: 0.2,
    presencePenalty: 0,
  },
};

export const DEFAULT_GENERATION_PREFERENCES: GenerationPreferences = {
  preset: "balanced",
  ...GENERATION_PRESETS.balanced,
  maxTokens: null,
};

export const DEFAULT_READING_PREFERENCES: ReadingPreferences = {
  proseSize: 18,
  lineHeight: 1.72,
  columnWidth: 920,
  compactSpacing: false,
};
