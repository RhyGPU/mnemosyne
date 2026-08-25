# Mnemosyne Usability Audit

- Date: 2026-07-29
- Build exercised: `src-tauri/target/debug/mnemosyne.exe`
- Flow: disclaimer, Home, resume session, `/help`, State Map, Library,
  Settings/AI, Settings/Generation
- Method: direct Windows UI interaction at 1280 x 864

This list tracks findings from using the product, rather than findings inferred
only from source code. Items stay in the list after completion so the reason for
each behavior change remains visible.

| ID | Priority | Area | Finding | Status | Acceptance |
|---|---|---|---|---|---|
| ux-001 | P0 | slash commands | `/help` calls the configured command LLM; an unavailable model leaves a provider error instead of help | resolved | `/help` is deterministic, performs no provider request, and always renders the local command list |
| ux-002 | P0 | chat | Narrator messages render the literal `status` code fence and engine status line | resolved | Visible prose is clean; state metadata is rendered as a separate, collapsed `Scene state` element |
| ux-003 | P0 | provider errors | Raw provider envelopes, including provider-side identifiers, can be persisted in visible chat | resolved | Command-provider failures are reduced to categorized recovery guidance before visible or diagnostic persistence; raw envelopes and identifiers are discarded |
| ux-004 | P0 | State Map | Session clones and players repeat per session and leak identifiers such as `session_clone` and `preset_male` | resolved | Characters, relationships, and objects merge into a latest-value presentation by stable display identity; session counts preserve provenance and internal IDs become `Soul`, `You`, `Player`, or `Unassigned` |
| ux-005 | P1 | provider setup | An unavailable active model appears healthy until the first request fails | queued | Profiles expose connection/model validation, last checked state, and an actionable unavailable warning |
| ux-006 | P1 | session resume | Resume opens away from the latest turn and an old evaluator banner appears as current status | queued | Resume lands at the latest turn unless a saved reading position is explicit; terminal banners do not resurrect as live alerts |
| ux-007 | P1 | Library | Same-name Souls and worlds are visually indistinguishable; image loading can remain as a broken placeholder | queued | Duplicate names receive stable disambiguation and image states have intentional loading/error placeholders |
| ux-008 | P1 | Home | Recent sessions can show `Unknown soul`; duplicate recommended Soul cards have no distinguishing context | queued | Every session resolves a user-facing Soul label or a clear recovery state |
| ux-009 | P1 | Settings | Reading tab did not activate through repeated accessibility and coordinate clicks during the audit | queued | Reading tab changes panel reliably by mouse and keyboard and exposes the selected tab semantics |
| ux-010 | P2 | Play | Full pipeline terminology occupies a large permanent column in the normal reading surface | queued | Normal mode shows a compact human status; detailed stage names are available in Dev Mode |
| ux-011 | P2 | slash menu | `/help` appeared twice and required one Enter to close/select the menu and another to submit | queued | Suggestions are unique and Enter behavior is predictable and explained |
| ux-012 | P2 | settings language | `Generation Preset`, `Narrator Style`, and `Mnemosyne Brief` lack enough distinction for a new user | queued | Each control has a short outcome-oriented explanation or preview |
| ux-013 | P1 | State Map | On a fresh app load, opening State Map can show “No active sessions” until Refresh is pressed | queued | Entering State Map loads current data automatically and distinguishes loading from a genuinely empty state |
| ux-014 | P1 | State Map | Core memories and timeline facts still repeat across recent session clones | queued | Equivalent memory/event content collapses into one item with source-session count while detailed provenance remains inspectable |

## Positive Baseline

- The product has a distinctive and consistent visual identity.
- Home, Play, State Map, Library, and Settings form a comprehensible top-level
  information architecture.
- Narrator, Evaluator, and State Map make the product model legible.
- Provenance and memory inspection are meaningful differentiators.
- Generation controls are grouped more clearly than the underlying provider
  settings.

## Repayment Order

Work proceeds in the table order unless a fix reveals a shared lower-level cause.
Every item requires a focused regression test and a direct UI verification when
the behavior is visible.
