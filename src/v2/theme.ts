// UX v2 — theme tokens. PAPER / e-ink register (Kindle / reMarkable / editorial).
// Near-monochrome on purpose: warm paper + black ink + hairline rules. Color is
// used almost nowhere — hierarchy comes from weight, size, and whitespace, not hue.
// Primary actions read like an INK STAMP, not a colored button.
// Decided 2026-06-27 (docs/UI-theme-direction.md). Offline-friendly font stacks.
import type { CSSProperties } from "react";

export const serif =
  "'Iowan Old Style', 'Palatino Linotype', Palatino, 'Book Antiqua', Georgia, 'Times New Roman', serif";
export const sans =
  "ui-sans-serif, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif";
export const mono =
  "ui-monospace, 'SF Mono', 'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace";

// Warm paper + ink. No saturated colors.
export const c = {
  paper: "#f4f1e8", // page — soft warm eggshell, like e-ink
  panel: "#faf8f1", // barely-raised card
  panelEdge: "#e6e0d2",
  book: "#efeadd", // slightly warmer reading surface
  ink: "#221f1a", // near-black warm ink (text + primary actions)
  inkSoft: "#675f53",
  inkFaint: "#9b9384",
  rule: "#ddd6c6", // hairline
  ruleSoft: "#e9e3d5",
  accent: "#221f1a", // == ink. Primary buttons/marks are stamped ink, not red.
  accentSoft: "#8c8475", // muted graphite for subtle fills/meters
  redact: "#1a1712", // redaction bar
  good: "#55604c", // muted sage — tiny status only
  warn: "#766246", // muted sepia — tiny status only
} as const;

export const label: CSSProperties = {
  fontFamily: mono,
  fontSize: 10.5,
  letterSpacing: "0.14em",
  textTransform: "uppercase",
  color: c.inkFaint,
};

// Quiet editorial kicker — muted ink, NOT a colored accent.
export const eyebrow: CSSProperties = { ...label, color: c.inkSoft };

export function card(extra?: CSSProperties): CSSProperties {
  return {
    background: c.panel,
    border: `1px solid ${c.panelEdge}`,
    borderRadius: 3,
    padding: 18,
    ...extra,
  };
}

export function h1(extra?: CSSProperties): CSSProperties {
  return { fontFamily: serif, fontSize: 30, fontWeight: 600, color: c.ink, margin: 0, ...extra };
}
