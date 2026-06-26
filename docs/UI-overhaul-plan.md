# Mnemosyne UI Overhaul — Master Plan

Status: approved, in progress (started 2026-06-25)

---

## 0. Product philosophy (the frame for every decision)

Mnemosyne is an **engine and a playground, not a content catalog.** It ships no
characters. The user brings any character (import/create); the value is what the
engine does with them — **persistent memory, evolving state, multi-character
relationships, evaluation + repair.** The emotional center is the *simulation
depth*, not a curated roster.

Design consequences:
- The main page is for **recognizing and selecting your own cast** quickly
  (portrait + description = a handle to grab), *not* a storefront. Do not
  over-design character tiles into something that implies the app supplies
  characters.
- "Get more characters" is an **outbound web link** to a site where users
  download cards — precisely because the platform doesn't provide them.
- The app should make the engine *legible*: that a character remembers, has
  state, and lives in a world with others.

---

## 1. Diagnosis (why it's cluttered)

The whole frontend is two files: `src/App.tsx` (9,369 lines) and
`src/tauri.ts` (140 KB). Nearly everything is one ~8,000-line `App` component.
Two views (`library`, `chat`) + a settings drawer (4 tabs) + a dev-console panel.

The "library" view is one mega-scroll of **9 unrelated cards**: launcher header,
World picker+editor, Character picker+editor+psyche, **Provider Settings
(duplicated in the drawer)**, Launch+sessions, World environment editor, Soul
editor, Memory dump, API Debug panel. Four different jobs — **Play, Create,
Configure, Inspect** — are mashed onto one surface.

Core problems: no persistent navigation / no sense of place; selecting competes
with editing and debugging; duplicated controls (providers, dev settings); no
first-run path; the turn pipeline is a black box for normal use; archive ≠
visibly gone; chat scroll isn't magnetic; single-character only.

---

## 2. System inventory (backend capabilities)

Characters (Souls), Worlds (Settings), Sessions (Conversations), Player Personas,
AI Providers (narrator/updater/repair + embedded model), Memory (core/schemas/
recent/consolidation/curate), Evaluator/Repair (jobs, ops, auto-fallback),
Benchmark (run/prepare/generate/summary/finalize), Dev/Debug (log stream,
payload inspector, context preview, branch debug, 9 dev commands), Import/Export
(`.mne` bundles, JSON, image assets / multimodal).

### 2a. Built-but-invisible features (surface these during the overhaul)
- **Hard delete** — `deleteConversation` / `deleteSoul` / `deleteSetting` exist;
  UI only archives. (The real fix for "archive doesn't remove it.")
- **Savepoint / checkpoint system** — `archiveSavepoint` / `restoreSavepoint` /
  `listArchivedSavepoints`, plus unwrapped `create_session_soul_from_savepoint`,
  `create_fresh_scenario_soul`, `create_backup`. Zero UI.
- **Character & persona archiving** — `archiveSoul`/`restoreSoul`/
  `listArchivedSouls`, `archivePlayerPersona`/`restorePlayerPersona`. No UI.
- **Manual memory curation** — `curateMemory`. No UI.
- **Payload history** — `listLlmPayloadLogs`/`getLlmPayloadLog`. No UI.
- **Granular turn hide/restore** — `hide_turn_range`/`restore_turn_range`/
  `list_hidden_turns` (unwrapped). UI only does bulk restore.
- **Alt paths** — `loadSoulFile`/`loadSettingFile`, `deleteAssistantMessageVariant`.

---

## 3. Target information architecture

**No top-level tab bar.** The app is two full-screen destinations you move
between by *action*, plus a Settings drawer over the top and an in-session Dev
toggle. Decluttering happens by **relocation**, not tabs: AI/provider settings
live in the drawer, debug/memory live in in-session Dev mode, and creation lives
on a separate Editor screen — so Home is automatically clean.

```
HOME (Library)  pick world(1) + characters(n) + session → dive in     [select/play]
   │  "New / Edit" →  EDITOR        create/edit characters & worlds     [create]
   │  "Settings"  →  drawer overlay AI providers, chat, data, about     [configure]
   └  "Dive in"   →  CHAT (session) play; Dev toggle re-skins to matrix [play/inspect]
```

### 3.1 HOME / Library (selection-first; pick & dive in)
The single entry point. **No editors, no provider forms, no debug.**
- World picker — choose **exactly one**.
- Character picker — choose **one or more** (multi-character, built fully).
  Portrait + name + short description, scannable grid; recognize-your-cast.
- Session strip — recent sessions, **Active ⇄ Archived toggle** that filters;
  exposes hard-delete.
- Quick actions — Import (`.mne`/JSON), Export, **"Get more characters" → web**,
  **New / Edit** (→ Editor), **Settings** (→ drawer).
- Launch — start mode (continue / fresh) + Start (→ Chat).

### 3.2 EDITOR (separate screen — create & manage)
Reached from Home via "New / Edit", **not** a permanent tab; "Back" returns Home.
World editor, Character editor + psyche (traits/needs/SDT/trauma/relationship),
personas, savepoints, archive/restore/**delete** for all entity types, memory
curation. Keeps Home uncluttered.

### 3.3 SETTINGS (drawer — from anywhere)
A drawer/sidebar, **not** a destination. AI Providers (de-duplicated — the old
launcher provider card is removed; **shown on first launch** as onboarding) ·
Chat (start mode, magnetic scroll, composer) · Data (import/export, session-data
location, backup) · About/disclaimer.

### 3.4 CHAT (session — where you play)
- **Magnetic-bottom scroll**: follow newest only while pinned to bottom; release
  on scroll-up; "jump to latest" pill. ✅ shipped.
- Header archive/restore + savepoint/checkpoint actions.
- A calm pipeline indicator (thinking → narrating → remembering → checking →
  done) for normal use.
- **Dev toggle** flips the session into Dev Mode (§4) — an in-session *mode*,
  not a separate tab.

### 3.5 DEV MODE (in-session toggle — the power surface — see §4)

---

## 4. Dev Mode spec

A **full-screen mode** (toggle re-skins the app; matrix aesthetic — monospace,
green-on-black, rigid rectangular frames), unifying everything currently smeared
across the dev console, provider settings, the library "API Debug" card, and the
chat evaluator banner.

**Layout — 3 zones + workbench:**
- **Left · Pipeline Rail.** The existing pipeline trace promoted to a persistent
  live rail: one bar per chat cycle, every stage (send → narrator → state-updater
  → evaluator → repair → done), each green ✓ / yellow ◐ / red ▲ / paused.
  History across cycles (not "latest only"); click a stage → loads it in the
  inspector.
- **Center · The Stream.** Combined terminal + chatlog. `log=` and `chatlog=`
  lines interleaved in one feed. Input is a **terminal by default**; `/chat …`
  injects an in-character turn. Command runner becomes a real typed command line
  (autocomplete over the 9+ whitelisted ops), available always — not gated to DEV
  builds.
- **Right · Inspector (tabbed).** Payload (LLM Payload Inspector + orphaned
  payload-history browser) · Turn (API Debug grid: hidden-state, fallback,
  anti-replay, trust/affection deltas, present chars) · Context (compiled
  preview) · Memory (core/schemas/recent + `curateMemory`) · Trace (full pipeline
  JSON, token usage, timings).
- **Bottom · Test & Benchmark Workbench.** Benchmark Runner (mode, target, turns,
  player profile, goal, strict eval, transport, traditional opponent, live
  turns-remaining, scorecard) · Evaluator diagnostics (Contract Test, Structured
  Diagnostic) · Evaluator config (legacy/structured, policy, transport) · live
  job control (status/cancel/retry, moved out of the chat banner) · system health
  (embedded repair model start/stop/status, provider health, `create_backup`,
  session-data location).

Promoted out of dev: the calm pipeline indicator in normal Chat (§3.4).

---

## 5. Phased implementation plan

- **Phase 0 — Refactor (prerequisite, approved).** Split `App.tsx` into
  `types.ts`, `constants.ts`, `lib/`, `components/`, `views/`, and lift shared
  state into a context/store. Behavior-preserving; typecheck green at every step.
- **Phase 1 — Screen router.** Home ⇄ Editor ⇄ Chat as full-screen views moved
  between by action (no tab bar); Settings drawer + in-session Dev toggle.
  (Revised: an earlier 4-tab rail was scrapped per the simpler IA in §3.)
- **Phase 2 — HOME redesign.** Selection-first launcher; multi-character picker;
  session strip with working Active/Archived toggle + hard-delete; quick actions
  incl. web link + New/Edit + Settings. Provider card removed from Home.
- **Phase 3 — EDITOR screen.** Move editors/psyche/personas here; surface
  savepoints, character/persona archiving, hard-delete, curateMemory.
- **Phase 4 — SETTINGS drawer + first-launch onboarding.** Consolidate the
  drawer; remove the duplicate launcher provider card.
- **Phase 5 — CHAT fixes.** Magnetic scroll + jump-to-latest ✅; calm pipeline
  indicator; savepoint actions.
- **Phase 6 — DEV MODE.** Build the §4 in-session workspace; retire the old
  console + drawer DEV tab; pull benchmark/evaluator out of provider settings.
- **Phase 7 — Multi-character backend + visual polish.** Wire true multi-active
  sessions; spacing, typography, empty states, matrix theme finalize.

---

## 6. Settled decisions
- Multi-character: **build fully** (picker is multi-select; backend gains true
  multi-active sessions — Phase 7).
- "Download web redirection": **outbound link to a site**.
- Refactor: **split the monolith first** (Phase 0).
- Mnemosyne provides **no characters**; it's a BYO-character engine/playground.
- **Aesthetic: dual.** Human surface (Play/Library/Settings/Chat) stays warm and
  intimate but *elevated* (better depth/hierarchy than the current flat cards) —
  fits the memory/soul theme and differentiates from dark-neon RP apps. **Full
  cyberpunk/matrix is reserved for Dev Mode only** (green-on-black, monospace,
  rigid shapes). The contrast = soul/fiction layer vs engine/machine layer.
