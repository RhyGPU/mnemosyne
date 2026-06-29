// UX v2 — presentation-layer redaction.
// The backend always stores full, unredacted state (locked decision). This pure
// function decides only how a field is *shown* given the active mode, who owns
// the field, and the purpose toggles. See docs/UX-plan-v2.md §3.
import type { NarrativeMode } from "../uiTypes";
import { MODE_RANK } from "./sessionPurpose";
import type { PurposeToggles } from "./sessionPurpose";

export type Visibility = "show" | "redact" | "omit";
export type Sensitivity = "public" | "secret";
/** "self" = the player's own character (don't spoil their blind spots). */
export type Ownership = "self" | "npc";

export interface VisibilityArgs {
  sensitivity: Sensitivity;
  ownership: Ownership;
  mode: NarrativeMode;
  toggles: PurposeToggles;
}

export function fieldVisibility(args: VisibilityArgs): Visibility {
  const { sensitivity, ownership, mode, toggles } = args;

  // Public state (scene, objects, positions) is always visible.
  if (sensitivity === "public") return "show";

  const rank = MODE_RANK[mode];
  const god = rank >= 2;

  // Your own character's secrets (knows / misbelieves) are a blind-spot spoiler:
  // hidden unless you've stepped fully behind the curtain (god / author).
  if (ownership === "self") {
    return god ? "show" : "omit";
  }

  // NPC secrets — the dramatic-irony surface.
  // god → plaintext · Reader/Custom → redaction bar w/ reveal · Realistic → omit.
  let base: Visibility = god ? "show" : rank >= 1 ? "redact" : "omit";

  // Dramatic-irony toggle bumps NPC secrets one tier toward visible.
  if (!god && toggles.dramaticIrony) {
    if (base === "omit") base = "redact";
    else if (base === "redact") base = "show";
  }

  return base;
}
