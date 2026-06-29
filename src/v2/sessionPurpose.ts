// UX v2 — Session Purpose model.
// A Purpose is a *composable bundle of toggles* (locked decision 2026-06-27):
// the five named purposes are just starting points the user can recompose, and
// Purpose is mutable mid-campaign. The backend always stores full state; these
// toggles only drive presentation. See docs/UX-plan-v2.md.
import type { NarrativeMode } from "../uiTypes";

export type PanelKey =
  | "scene"
  | "characters"
  | "relationships"
  | "objects"
  | "body"
  | "timeline"
  | "memory";

export type LivingMemoryMode = "off" | "ambient" | "full";

export interface PurposeToggles {
  /** Narrative mode a fresh session opens in. */
  defaultMode: NarrativeMode;
  /** Highest mode a normal (non-GM) player may select when visibility is asymmetric. */
  modeCeiling: NarrativeMode;
  /** NPC secrets surface one tier earlier — the dramatic-irony pleasure of reader/god. */
  dramaticIrony: boolean;
  /** GM (god-mode holder) sees everything; players are bounded by modeCeiling. */
  asymmetricVisibility: boolean;
  /** Panels floated to the top of the State Map for this purpose. */
  emphasize: PanelKey[];
  /** Inline "a smell stirred a memory" notes in Play. */
  sensoryCallbacks: boolean;
  /** Living/decaying memory surface + consolidation beat. */
  livingMemory: LivingMemoryMode;
  /** Sparse anatomical ghost on the State Map. */
  bodyGhost: boolean;
  /** Per-Soul longitudinal biography / long-arc view. */
  biography: boolean;
}

export type PurposeName = "Immersive" | "Director" | "Tactical" | "Ensemble" | "Author";

/** Mode visibility rank: higher = sees more. Custom is treated as Reader-level. */
export const MODE_RANK: Record<NarrativeMode, number> = {
  Realistic: 0,
  Reader: 1,
  Custom: 1,
  "Active Director": 2,
  "GM Simulation": 3,
};

export function isGodMode(mode: NarrativeMode): boolean {
  return MODE_RANK[mode] >= 2;
}

export const PURPOSE_BUNDLES: Record<PurposeName, PurposeToggles> = {
  Immersive: {
    defaultMode: "Reader",
    modeCeiling: "Reader",
    dramaticIrony: false,
    asymmetricVisibility: false,
    emphasize: ["relationships", "memory"],
    sensoryCallbacks: true,
    livingMemory: "ambient",
    bodyGhost: true,
    biography: true,
  },
  Director: {
    defaultMode: "Active Director",
    modeCeiling: "Reader",
    dramaticIrony: true,
    asymmetricVisibility: true,
    emphasize: ["characters", "relationships", "objects", "timeline"],
    sensoryCallbacks: true,
    livingMemory: "full",
    bodyGhost: false,
    biography: true,
  },
  Tactical: {
    defaultMode: "Realistic",
    modeCeiling: "Reader",
    dramaticIrony: false,
    asymmetricVisibility: false,
    emphasize: ["body", "objects", "scene", "timeline"],
    sensoryCallbacks: false,
    livingMemory: "off",
    bodyGhost: true,
    biography: false,
  },
  Ensemble: {
    defaultMode: "Reader",
    modeCeiling: "Reader",
    dramaticIrony: true,
    asymmetricVisibility: true,
    emphasize: ["relationships", "characters"],
    sensoryCallbacks: true,
    livingMemory: "ambient",
    bodyGhost: false,
    biography: true,
  },
  Author: {
    defaultMode: "GM Simulation",
    modeCeiling: "GM Simulation",
    dramaticIrony: true,
    asymmetricVisibility: false,
    emphasize: ["timeline", "memory", "characters"],
    sensoryCallbacks: false,
    livingMemory: "full",
    bodyGhost: false,
    biography: true,
  },
};

export const PURPOSE_BLURB: Record<PurposeName, string> = {
  Immersive: "1 player + 1 Soul. Disappear into the story.",
  Director: "A host runs a living table for player(s). GM sees all.",
  Tactical: "State and consequence. Body, objects, positions.",
  Ensemble: "Many Souls / users. Believable group dynamics.",
  Author: "Draft a manuscript. Full visibility, export-bound.",
};

export interface SessionPurpose {
  /** The starting bundle the user picked (badge / label). */
  base: PurposeName;
  /** Possibly user-recomposed toggles. */
  toggles: PurposeToggles;
}

export function purposeFrom(name: PurposeName): SessionPurpose {
  const b = PURPOSE_BUNDLES[name];
  return { base: name, toggles: { ...b, emphasize: [...b.emphasize] } };
}

/** Clamp a player-selected mode to the purpose's ceiling unless they're the GM. */
export function effectiveMode(
  selected: NarrativeMode,
  toggles: PurposeToggles,
  isGM: boolean,
): NarrativeMode {
  if (isGM || !toggles.asymmetricVisibility) return selected;
  return MODE_RANK[selected] > MODE_RANK[toggles.modeCeiling] ? toggles.modeCeiling : selected;
}
