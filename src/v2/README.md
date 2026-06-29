# UX v2 prototype (`src/v2/`)

A runnable, mock-data prototype of the new session-purpose-driven UX.
Spec: `docs/UX-plan-v2.md`. Theme: `docs/UI-theme-direction.md`.

## Run it
```
npm run dev:frontend
# then open: http://127.0.0.1:1420/?v2
```
The existing app is the default and is **untouched**; `?v2` mounts the prototype
(see the flag in `src/main.tsx`). Remove `?v2` to get the normal app.

## What's here
- `sessionPurpose.ts` — Purpose as **composable toggles** + 5 starting bundles
  (Immersive / Director / Tactical / Ensemble / Author), mode rank, ceiling clamp.
- `redaction.ts` — pure `fieldVisibility(mode, ownership, toggles, field) →
  show | redact | omit`. The backend stores everything; this only hides at the
  presentation layer.
- `theme.ts` — book/editorial paper tokens + shared button/chip styles.
- `mockData.ts` — Ashgate sample (souls, scene, memories, relationships,
  biography, worlds).
- `AppV2.tsx` — shell + nav rail + Home + Play + Purpose composer.
- (Within `AppV2.tsx`) State Map, living-memory panel, Soul biography, Library.

## What it demonstrates
- Persistent **nav rail** (recedes in Play).
- **Composable Purpose** — open "Purpose…" to flip toggles live; emphasis,
  censorship, sensory callbacks, living memory, biography all react.
- **Mode lens** (Realistic / Reader / Director / GM) with ceiling-clamp + GM seat.
- **State Map redaction** — change the mode and watch NPC secrets omit → redact
  (black bars, click to reveal) → plaintext; your own character's blind spots
  stay hidden until god mode.
- **Living memory** — weight/age styling, salience, provenance + evidence.
- **Soul biography** — trust-over-sessions arc, trauma phase, identity drift.

## Stubbed / next
- Settings, real backend wiring, persistence, multi-character ensemble graph.
- Port these patterns into the real app per `docs/UI-overhaul-plan.md` (needs the
  Phase-0 monolith split first).

## Verification status (2026-06-27)
- `redaction.ts` logic: unit-tested, 12/12 passing.
- Full-project `npx tsc --noEmit`: passed clean at integration (core + shell).
- The final Biography/Library additions were written after the sandbox mount went
  stale, so they weren't re-checked by tsc in-session (they reuse the same typed
  patterns). **Please run `npm run typecheck` once to confirm**, then `?v2`.
