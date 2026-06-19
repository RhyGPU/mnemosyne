# Mnemosyne Roadmap Status

Last updated: 2026-06-19

Mnemosyne is a local-first universal RP state map: a campaign brain for long-form roleplay.

Chat is only the visible interaction layer. The product is the durable state graph underneath it: characters, relationships, objects, locations, events, factions, secrets, unresolved tensions, memories, and continuity facts that survive long sessions, model swaps, imports, corrections, and branches.

This file is the current roadmap and status tracker. Older roadmap text is archived under `docs/Old plans/`.

---

## Status Legend

| Status | Meaning |
|---|---|
| `[DONE]` | Implemented in code and/or documented as passing tests. |
| `[PARTIAL]` | Significant implementation exists, but exit criteria are not fully proven. |
| `[PENDING]` | Not built yet or not found in current code/docs. |
| `[BLOCKED]` | Waiting on an external/runtime dependency or live validation. |
| `[VERIFY]` | Code exists, but needs a real GUI/provider smoke test before treating it as done. |

---

## Product Thesis

Mnemosyne should not compete with JanitorAI or SillyTavern as another chat frontend. Its differentiator is state.

The engine should answer:

- Who exists?
- Where is everyone?
- What happened?
- Who knows what?
- Who believes something false?
- What changed this turn?
- What object/faction/location/relationship was affected?
- Why did the AI remember this?
- Can the user correct it and continue?

The milestone is a 50+ turn session where the user can inspect the state map, trace memories to source turns, correct wrong facts, continue the RP with the correction respected, and export/import the session as a `.mne` bundle without loss.

---

## Current Verified / Recently Built

### Core app foundation

Status: `[DONE]`

Implemented:

- Tauri desktop shell.
- React + TypeScript UI.
- Rust `state_engine` crate.
- SQLite persistence for Souls, settings, conversations, messages, provider profiles, turn snapshots, image assets, LLM payload logs, branches, turn commits, and state patches.
- Soul savepoints, session clones, checkpoints, and fresh scenario sessions.
- Setting/world management with session-specific world state.
- Mock provider and OpenAI-compatible API provider paths.
- Streaming narrator output.
- Separate narrator / evaluator / repair model profile direction.
- EnginePatch application and materialized state rebuild.
- Relationship deltas.
- Memory scoring, salience, duplicate handling, and consolidation direction.
- Context compiler.
- Anti-replay and narrator output contract guards.
- Assistant response variants, regenerate/fix flows, and turn snapshots.
- Debug logs, payload preview, payload history export, and visible chat export.
- `.mne` bundle validation and session checkpoint import/export.
- Image attachment import and local image asset storage.

### Data safety / recovery commands

Status: `[PARTIAL]` / `[VERIFY]`

Implemented or registered:

- `archive_soul`
- `restore_soul`
- `list_archived_souls`
- `archive_savepoint`
- `restore_savepoint`
- `list_archived_savepoints`
- `archive_setting`
- `restore_setting`
- `list_archived_settings`
- `archive_session`
- `restore_session`
- `list_archived_sessions`
- `hide_turn_range`
- `restore_turn_range`
- `list_hidden_turns`
- `create_backup`
- `dedupe_active_adjacent_user_messages`
- `inspect_turn_branch_integrity`
- `repair_accidental_normal_send_variants`
- `rebuild_session_from_ledger`

Database support exists for archive/hidden semantics:

- `conversations.archived_at`
- `messages.is_active`
- `messages.message_status`
- `messages.hidden_at`
- `souls.archived_at`
- `settings.archived_at`
- patch/branch metadata for ledger rebuild and invalidation.

Still needs verification:

- Destructive-path live smoke test with a DB backup created first.
- Delete/hide one message must not hide/delete the whole session.
- Archive one session must not affect sibling sessions.
- Restore turn range must skip pending, failed, retry, duplicate, discarded, or non-canonical messages.
- Payload logs must remain linked to restored visible messages.
- Soul/savepoint archive/restore must preserve linked world/session references.

---

## Priority 1: Schema-Enforced Evaluator

Status: `[PARTIAL]` / `[VERIFY]`

Goal: evaluator output is constrained by a JSON schema or tool definition at the provider/decoder level.

### Done

- `evaluator_structured_v1` exists as the main experimental evaluator direction.
- Real tool-call transport was added for structured evaluator ops through forced `submit_evaluator_ops` calls.
- Structured evaluator ops compile into `EnginePatch`.
- Structured evaluator transport/policy settings, retry handling, timeout controls, and trace fields exist.
- Entity aliases exist for evaluator ops, including:
  - `active_soul`
  - `active_player`
  - `latest_speaker`
  - `session_world`
- Structured evaluator diagnostics, semantic validation, retry repair prompts, and payload-history export were hardened.
- `evaluator_form_v1` remains available as legacy fallback.
- Partial-accept now exists: one bad evaluator op no longer throws away the whole turn if other ops are valid.
- Rejected ops are captured with reasons and can be repaired by a background op-repair worker.
- Player/persona aliases in soul-only evaluator fields are coerced to active Soul instead of causing all-or-nothing patch rejection.

### Partial / not done

- Broad provider-side structured-output support is not complete across all targets:
  - OpenAI/OpenRouter `response_format: json_schema`
  - Anthropic tool use
  - Google structured output
  - grammar-constrained local JSON
- Grammar-constrained embedded local evaluator is planned, not fully shipped.
- Old syntactic repair paths are still present and should not be deleted until schema enforcement proves stable.
- Needs a real 20-turn diagnostic run with a tool-capable model.
- Needs proof that syntactic repair activations drop to zero under schema-enforced mode.

### Done when

- An evaluator model that never learned the old form prompt passes the contract test.
- It writes correct patches for 20 consecutive normal RP turns.
- Syntactic repair paths report zero activations.
- State commits continue through memory, relationship, object, scene, and event patches.

---

## Priority 2: Cheap And Conditional Evaluator

Status: `[PARTIAL]`

Goal: evaluator overhead stops doubling every turn.

### Done

- Narrator, evaluator, and repair model roles are now conceptually separated.
- Separate repair model profile selector exists.
- Repair model can fall back through this precedence:
  1. selected Repair profile
  2. embedded local model if ready
  3. evaluator settings
- Dev embedded repair model launcher exists using a local llamafile/server-style process.
- Evaluator job lifecycle has background job tracking, retry, cancellation, and repair paths.
- Benchmark runner can exercise the real chat path and expose evaluator behavior.
- Event-driven benchmark pacing replaces slow polling between benchmark turns.

### Partial / not done

- Fast / Balanced / Long Context modes are not fully implemented as product presets.
- Conditional evaluator gating is not complete:
  - OOC/dialogue-only skip
  - scene-boundary catch-up
  - significance-based trigger
- Per-turn estimated cost and output-token accounting are not fully proven as user-visible mode feedback.
- Native bundled local evaluator runtime is not production-ready.
- Embedded local model is dev/manual-path only.

### Done when

- Fast mode evaluates at most 50% of turns in a 50-turn session.
- State map continuity does not regress.
- Cost differences are visible in the trace UI.
- Cheap/local evaluator path is reliable enough for default updater/repair use.

---

## Priority 3: Memory Pool Overhaul

Status: `[PARTIAL]`

Goal: store more, inject less.

Do not solve continuity by dumping more memories into prompts. Store memory generously, retrieve selectively.

### Done

- Memory salience and retrieval strength are now treated as separate concepts.
- Active non-core memories decay over turns.
- Core memories are protected from decay.
- Reinforcement can increase salience and refresh retrieval strength.
- Overflow/eviction direction is archive-not-delete rather than silent destruction.
- Partial-accept evaluator behavior prevents one bad op from discarding valid memory writes.
- Memory provenance foundation exists:
  - `source_conversation_id`
  - `source_message_id`
  - `source_session_id`
  - `source_quote`
- Evidence quotes from evaluator ops can be persisted as `source_quote`.
- Repair worker can use failed op JSON + reasons + source line to retry dropped evaluator ops.

### Partial / not done

- Need confirm all memory creations populate the full intended provenance set:
  - source message ID
  - conversation ID
  - branch ID
  - turn ID
  - confidence
  - truth status
  - evidence quote
- Need verify a 100-turn session retains active + archived memory history.
- Cross-slot deduplication is not confirmed.
- Pin/edit/invalidate controls need Memory Inspector UI.
- Real semantic retrieval / embedding backend is still not confirmed as production-ready.
- Lexical/hash fallback should be clearly labeled if still used.

### Done when

- A 100-turn session retains active + archived memory history.
- No selected memory appears twice in one prompt.
- Every new memory can be traced to the exact source turn.
- User can pin, edit, invalidate, or inspect a memory without DB surgery.

---

## Priority 4: State Map V1 And Memory Inspector

Status: `[PENDING]` / `[PARTIAL]`

Goal: make the engine's beliefs visible and correctable.

### Done

- Backend ingredients exist for a first inspector:
  - memories with salience/retrieval strength
  - source quote/provenance fields
  - relationship/object/scene state patches
  - branch/turn/payload debug tools
  - `curate_memory` command exists as a starting point for curation actions
- `.mne` export and payload history export can support debugging.

### Not done

State Map V1 UI is not complete.

Needed panels:

- **Scene State:** location, participants, positions, door/room state, active objects, current misunderstanding, open question, last action.
- **Characters:** who exists, what each knows, what each misbelieves.
- **Relationships:** current values, recent changes, source events.
- **Objects:** identity, owner, location, status. Identity stays separate from condition.
- **Timeline:** recent events, active plot threads, unresolved tensions.
- **Memory Inspector:** all memories with provenance, pin/edit/invalidate controls, and retrieval trace.

### Immediate implementation target

Build **Memory Inspector V1** before deeper evaluator work.

Minimum scope:

- List memories grouped by slot/category.
- Show memory text.
- Show salience.
- Show retrieval strength.
- Show core/pinned/archived/invalidated state.
- Show source conversation/session/message/quote.
- Show target entity / perceived_by / source_type when available.
- Open source turn/message.
- Pin/unpin memory.
- Invalidate/archive memory.
- Lower salience.
- Export selected memory trace.

### Done when

- A user can inspect a wrong fact, see which turn created it, correct or invalidate it, and continue with the correction respected.

---

## Priority 5: Visible Benchmark / Live Stress Testing

Status: `[DONE]` / `[VERIFY]`

Goal: benchmark through the exact same visible chat path as manual typing.

### Done

- Visible AI Chat benchmark mode exists.
- Benchmark now drives the real frontend `executeTurn` path instead of one blocking backend command.
- AI player message appears as a user turn.
- Narrator streams visibly.
- Evaluator can run between turns.
- Benchmark artifacts include:
  - payload history markdown
  - `.mne` session checkpoint
  - summary JSON
  - dev log
- Stop Benchmark is wired.
- Player-line generation was moved to streaming transport to avoid non-streaming decode/provider flakiness.
- Event-driven evaluator wait reduces pacing lag.

### Needs live verification

- Run a real Visible AI Chat benchmark with an API provider and Player Simulator profile.
- Confirm AI player bubbles appear in chat.
- Confirm narrator streams visibly.
- Confirm evaluator runs and commits between turns.
- Confirm memory/object/relationship counts grow.
- Confirm rejected ops emit repair events.
- Confirm Stop Benchmark finalizes completed turns correctly.

---

## Priority 6: Structural Decomposition

Status: `[PARTIAL]`

`commands.rs` remains too large. Do not do a big-bang refactor. Extract modules only while touching the relevant code.

### Done

- Some module boundaries exist:
  - `chat_commands`
  - `pipeline_trace`
  - `providers`
  - `db`
  - `state_engine`
- `lib.rs` registers many commands explicitly, including evaluator, repair, benchmark, export, restore, and embedded model commands.

### Not done

Major extraction targets remain:

- narrator turn orchestration
- evaluator job lifecycle
- provider structured-output calls
- slash command routing
- import/export and `.mne` bundles
- state map query layer

Every extraction must keep tests green.

---

## Priority 7: Entity Separation And Plot Lifecycle

Status: `[PARTIAL]` / `[PENDING]`

### Entity separation

Required identities must stay separate:

- real user/operator
- player persona
- narrator-controlled NPC
- active Soul
- imported-log speaker
- previous-session persona
- OOC/GM channel

Done / partial:

- Player aliases and active Soul aliases exist in evaluator ops.
- Structured evaluator removed some default-player leakage from relationship context.
- Relationship summaries were deduplicated in the recent structured-evaluator work.
- Player alias in soul-only perceived fields is coerced to active Soul instead of hard-failing.

Not done:

- Full multi-actor/multi-user entity separation is not alpha-safe yet.
- Imported-log speaker separation is not complete.
- Relationship bleed across personas still needs explicit tests.
- Knowledge-scoped memory/facts are not complete.

### Plot lifecycle

Status: `[PENDING]`

Needed statuses:

- active
- background
- resolved
- stale
- invalidated

Done when:

- Old unresolved conflicts fade unless reinforced.
- Resolved plots stop appearing in current context.
- Active plot does not override latest exchange.

---

## Priority 8: Import / Portability / Schema Migration

Status: `[PARTIAL]`

### Done

- `.mne` export/import exists.
- Session checkpoint export exists.
- Bundle validation and preview exist.
- Session branches, turn commits, state patches, assistant variants, and payload logs exist in persistence.

### Not done / verify

- Strong `.mne` bundle V2 with full manifest and schema versioning.
- Roundtrip test proving no silent overwrite/loss.
- Imported logs should go through an intake pipeline instead of being sent directly to narrator.
- Imported memories should be labeled `source_type=imported_log`.
- Imported speakers must not merge with current user/persona unless confirmed.
- Migration dry-run/report system is not complete.

---

## Current Immediate Order

Do this exact order now:

```txt
1. Live validation pass for the latest structured evaluator + benchmark + repair path.
2. Confirm data safety commands with DB backup before destructive-path tests.
3. Build Memory Inspector V1.
4. Add/verify memory curation actions from the inspector.
5. Verify full memory provenance on all new memory writes.
6. Verify archived-memory retention across long sessions.
7. Add cross-slot memory deduplication if missing.
8. Harden entity separation with explicit tests.
9. Add plot lifecycle states and decay/resolution logic.
10. Upgrade API/model settings into user-facing Fast/Balanced/Long Context modes.
11. Package `.mne` V2 + import-log intake.
12. Prepare closed alpha tester guide.
```

---

## Closed Alpha Gate

Do not run a broader alpha until these are true:

- Normal send remains one active branch/variant unless regenerate is explicitly requested.
- Restore/soft-delete tools are proven with live DB backup.
- Soul/savepoint restore is proven.
- Memory Inspector V1 exists.
- Memory provenance works for every new memory.
- Bad memories can be invalidated from the UI.
- `.mne` export/import is safe enough for tester data.
- API/provider profiles are safe and do not overwrite unrelated profiles.
- Benchmark runner can produce usable payload/session artifacts.
- Tester guide exists.

Suggested alpha positioning:

> Mnemosyne is an early alpha for long-form RP memory and state-tracking stress tests. Expect bugs. The goal is to find continuity, memory, entity, and branch/state failures.

Do not claim production quality or guaranteed token savings.

---

## Explicit Non-Goals

- Not selling "always cheaper tokens." Short sessions may cost more than single-call frontends.
- Not competing on chat UI features alone.
- Not building a perfect physics/economy simulator.
- Not deleting old evaluator safety nets until schema-enforced mode is proven.
- Not moving to public beta before memory/state inspection and repair are user-visible.

---

## Long-Term Direction

After State Map V1:

- Native bundled local model runtime.
- Grammar-constrained local evaluator/repair model.
- Stronger semantic memory retrieval backend.
- Import-log intake mode.
- Multi-character and multi-user campaign workflows.
- Knowledge scopes, secrets, faction state, timers, delayed consequences.
- Portable `.mne` bundle V2 with assets and schema migration.
- Installer/package polish.
