# Dev Note — 2026-06-18 — Living memory lifecycle (decay / reinforce / core)

## Diagnosis (the recurring "something's wrong")
Junna's devlogs 000–002 spec a salience-scored memory where **meaningful memories
become permanent core and non-core decay over time unless reinforced** ("a repeated
meal is forgettable; repeated kindness can become trust").

The previous implementation built only the first half:
- `memory::MemoryScorer` scores a new memory once (emotional tag + novelty + goal),
- `patch.rs apply_memories` sorts `recent` by salience and caps the active pool to
  `MAX_RECENT_MEMORIES = 12` (overflow archived, not deleted), pinned exempt,
- near-duplicate **merge** bumped salience/retrieval_strength to ≥55.

**Missing:** any decay over time. `salience`/`retrieval_strength` were **frozen at
birth** and never moved. So retention was a *static ranking* + a hard cap — nothing
faded, "core" was just "currently top-12" (not an earned permanent tier), and
"unless reinforced" had no teeth (nothing decayed for reinforcement to save).

Root pattern (same as the relationship axes): **the engine computes snapshots, not
dynamics.** Relationship deltas are a one-shot linear nudge; memory salience is
frozen at creation. Neither evolves over time, which is the whole point of the vision.

## What changed (this note) — memory only
Slow-burn living lifecycle in `state_engine/src/patch.rs`, **no schema change**
(reuses `salience` = stable importance, `retrieval_strength` = living presence):

- Constants: `CORE_SALIENCE_THRESHOLD = 85`, `MEMORY_FADE_BASE = 12`,
  `MEMORY_FADE_FLOOR = 8`, `MEMORY_REINFORCE_CLIMB = 9`.
- `is_core_memory()` = salience ≥ 85 → permanent (never ages, never fades out).
- `age_memories(soul)` called once at the top of `apply_memories` (≈ once per turn;
  replaying a session reproduces the fade). Active, non-pinned, non-core memories
  lose `MEMORY_FADE_BASE * (1 - salience/85)` retrieval_strength per turn (higher
  salience fades slower); below `MEMORY_FADE_FLOOR` they archive (not delete).
- Reinforcement (merge path): a recurring near-duplicate now **climbs** salience by
  `+9` (capped 100) and refreshes retrieval_strength to ≥70 — so a small recurring
  moment can graduate into core over ~4 reinforcements ("repeated kindness → trust"),
  and un-fades if it just crossed into core.
- Feel chosen by Junna: **slow burn** (~10 turns for a middling salience-50 memory to
  fade) + **two roads to core** (high-impact at birth OR earned via reinforcement).

Tests (307 engine pass): `ordinary_memory_fades_out_of_active_pool_over_turns`,
`core_memory_never_fades`, `higher_salience_fades_slower`. The two existing cap tests
(`memory_overflow_is_archived_not_deleted`, `pinned_memory_is_exempt_from_archival_eviction`)
were converted from multi-turn loops to single-turn batch applies so they isolate
**cap archival** from the new per-turn fade (their actual intent) — assertions
unchanged in strength.

## Tunable knobs (plain-language → constant)
- "How fast ordinary memories fade" → `MEMORY_FADE_BASE` (higher = faster).
- "What counts as permanent core" → `CORE_SALIENCE_THRESHOLD` (lower = more becomes core).
- "How fast a recurring thing climbs to core" → `MEMORY_REINFORCE_CLIMB`.
- "When a faded memory drops from active" → `MEMORY_FADE_FLOOR`.

## Partial-accept: the eval no longer throws everything away on one fumble

Junna: "the memory eval is not solved." Correct — the decay lifecycle was
*downstream* of extraction. The eval itself (`compile_evaluator_ops_to_engine_patch`)
was **all-or-nothing in two passes**: `resolve_output_entity_aliases` aborted on
the first bad alias, and the per-op validate/compile loop aborted on the first
bad op (`?`). So a single fumbled field discarded the *entire* turn's ops →
form-v1 fallback → often noop → no memory. The `perceived_by` coercion fixed one
cause; this is the general fix.

Refactor (no behavior change for clean turns):
- Folded resolve + validate + compile into a per-op unit. `resolve_output_entity_aliases`
  became `resolve_op_entity_aliases(op, index, ...)`; the compile loop clones each
  op, resolves it, validates+compiles it inside an immediately-invoked closure, and
  on `Err` records an `EvaluatorCandidateRejection { candidate_id, reason }` and
  continues. Valid ops still land.
- `rejected_candidates` (a report field that was always empty) now carries the
  dropped ops + reasons → visible in the trace, not silently lost.
- Fallback ladder preserved: only a **total miss** (nothing compiled AND ops
  failed) returns `Err`, so the caller still falls through to evaluator_form_v1.
  Partial success returns `Ok` with whatever landed.
- Existing single-op failure tests (`invalid_entity_fails_semantic_validation`,
  `unknown_alias_fails_clearly`, the OOC latest_speaker test) still pass — a lone
  failing op = total miss = `Err`, unchanged. New test
  `partial_accept_keeps_valid_ops_and_drops_the_bad_one`. 308 engine tests green.

Net: a weak model that nails 4 of 5 ops now commits the 4 instead of losing all 5.
This is the robustness half of "solve the eval"; the speed/quality half is still
the local-model + grammar direction.

## Address/quote provenance (foundation for repair + recall)

Goal: every memory carries a system-set *address* (which session / chat log /
line) + the *quote* (exact source line), for error tracing, references, a future
recall system, and — immediately — feeding the repair worker the original text.

Division of labor (Junna's call, correct): **address = system-stamped** (the
model can't know ids), **quote = model-proposed + system-validated** (already
happens). What existed: `stamp_memory_provenance` already stamped
`source_conversation_id` + `source_message_id`. Gaps: session id never stamped;
the validated `evidence_quote` was discarded (no field to hold it).

Done:
- `MemoryEntry` + `MemoryPatch` gained `source_quote: Option<String>` (`#[serde(default)]`
  → old saves migrate clean). The AddMemory compile arm now persists the
  validated `evidence_quote` → `source_quote` (carried through `apply_memories`).
- `stamp_memory_provenance` now also stamps `source_session_id` from
  `ledger_branch_id`, at *both* call sites (inline eval + background job). Still
  only fills blanks, so explicit model values win.
- Tests: `add_memory_persists_evidence_quote_as_source_quote` (engine),
  `memory_provenance_is_stamped_without_overwriting` extended for session id.
  309 engine tests + app + tsc green. ~10 `MemoryEntry` literals updated for the
  new field (no `Default` on the struct).

This is the foundation the background repair worker leans on: address
(conversation + message + session) lets the worker fetch the original turn;
`source_quote` pins the exact line → far more accurate local-model repairs.

## Background op-repair worker — BUILT (callable; auto-trigger pending)

Built the greenlit repair worker ("repair logic now, configurable endpoint, 5
attempts"). Design: reuse the *proven* background-evaluator-job apply path — do
not hand-roll ledger writes.

- `start_background_evaluator_job` / `run_background_evaluator_job` gained an
  optional `repair_user_message_override: Option<String>`. When set, it replaces
  the evaluator *user* message with a focused "fix only these failed ops"
  request; the system rules, compile/partial-accept, provenance stamp, ledger
  apply, status, and async spawn are all the unchanged proven path. `None` for
  normal eval (both existing callers pass `None`).
- `repair_evaluator_ops` command (modeled on `retry_evaluator_job`): loads the
  turn context, builds the focused repair message via `build_op_repair_user_message`
  (each failed op JSON — which already carries its evidence_quote/source line —
  + its failure reason, anchored to the turn text), forces structured mode +
  `structured_evaluator_max_retries = 5` + background, and spawns the job against
  a **caller-supplied (configurable/local) `repair_settings`** endpoint.
  Non-blocking; failures stay dropped.
- `EvaluatorOpRepairRequest { op_json, reason }` is the contract. Tauri binding
  `repairEvaluatorOps(...)` added. Registered in lib.rs. Test:
  `op_repair_message_focuses_on_failed_ops_with_reasons`. 383 app + 309 engine +
  tsc green.

**Auto-trigger — DONE (capture + output + fire).** The failure verdict is the
*system's* (`compile`'s `rejected_candidates`), not the tool-call's, so we can
always reconstruct the failed ops. `rejected_ops_for_repair` snapshots the raw
ops (`runtime.normalized_json`) and pairs each `rejected_candidate` (`op:N` +
reason) into `EvaluatorOpRepairRequest { op_json, reason }`. After a normal eval
(guard: `repair_user_message_override.is_none()` → no repair-of-repair), the
background job emits a dev-log (`evaluator_ops_rejected`, visible) AND a
`evaluator-ops-rejected` window event with the failed ops. Frontend listener
(`listenEvaluatorOpsRejected`) auto-fires `repairEvaluatorOps` against
`repairSettingsRef` (current evaluator/updater settings; point at local) — it
runs as its OWN background job *after* the original eval committed, so there's no
overlap/race on the turn's write. Best-effort: failures stay dropped. 383 app +
309 engine + tsc green.

Note: emit is on the **background** eval path (what the benchmark + normal turns
use). The inline eval path doesn't emit the event yet — add the same block at the
inline compile site if inline eval should also auto-repair. Repair currently uses
the updater settings by default; a dedicated local "repair endpoint" setting is
the natural follow-up so repair never touches the paid narrator/eval model.

(superseded note) the worker is callable but not yet auto-fired. The
failed-op *JSON* isn't cleanly available at the background spawn point (ops are
already converted to the form `EvaluatorOutput` by then; only
`conversion.rejected_candidates` reasons remain), and wiring an auto-spawn into
the eval hot path blind (can't runtime-verify here) risks the core path. Options
for the trigger (next, ideally with the app runnable): (a) surface the rejected
ops' raw JSON in the turn result and have the frontend fire `repairEvaluatorOps`
after a turn; or (b) capture the structured ops + rejected indices earlier in the
job and auto-spawn a guarded repair (guard: skip when `repair_user_message_override`
is already set, to prevent recursion). Until then, `repair_evaluator_ops` is
invocable directly with the failed ops.

## Repair as its own role — Repair Model profile selector (DONE)

Architecture is three roles, three models: decent **narrator**, smart **evaluator**,
light **local repair**. The repair was reusing the eval's endpoint (shortcut); now
it has its own slot. `selectedRepairProfileId` (localStorage
`mnemosyne:repair_provider_profile_id`), a "Repair Model (light/local)" `<select>`
beside the State Updater provider selector, and the `repairSettingsRef` sync now
resolves: **selected Repair profile → else evaluator settings**. The auto-fire
listener already reads that ref. `ProviderProfile` is a superset of
`ApiProviderSettings` and the backend has no `deny_unknown_fields`, so a profile is
passed straight through as the repair endpoint. tsc green. Point a profile at a
local llama.cpp/LM Studio/Ollama to make repair a true separate local model.

### Embedded local model — dev version BUILT (manual spawn, hardcoded path)

Junna wanted a local AI living in the project NOW (no installer) to watch repair
work. Built a llamafile spawn:
- `AppState.local_model: Mutex<Option<EmbeddedModel{child,url,port,model}>>`.
- Commands `start_embedded_repair_model(binary_path, port?, model_name?)` (spawns
  `<file> --server --host 127.0.0.1 --port <8080> --nobrowser` via
  `std::process::Command`, stores the child, returns immediately),
  `stop_embedded_repair_model`, `embedded_repair_model_status` (one `/health`
  probe; clears a crashed child). Killed on `RunEvent::Exit` (switched lib.rs to
  `.build()?.run(|_,event| …)`).
- Frontend Repair Model card: a path input + Start/Stop + status; polls status
  while starting. Repair precedence is now **selected profile → embedded (ready)
  → evaluator settings**. Path persisted (`mnemosyne:embedded_repair_model_path`).
- NOT runtime-tested (can't spawn here). llamafile arg set + port 8080 are
  hardcoded; if a llamafile build rejects those flags, adjust. Embedded controls
  added to the main Repair Model card (the `settings-section` one), not the second
  duplicate card. 383 app + tsc green.

This is the dev/no-installer embed. The bundled sidecar (externalBin +
download-on-first-run) is still the eventual turnkey distribution step.

### Embedded local model (bundled/installer) — plan, not yet built
Precedence: **selected Repair profile > embedded local model > evaluator settings**.
Embedded = a Tauri **sidecar**: ship a `llama-server` binary (per-OS, via
`tauri.conf bundle.externalBin`) + a small GGUF (bundle, or download-on-first-run
into app-data with a progress UI — recommended, keeps the installer small). On
launch, spawn it on a free localhost port, health-check, expose the URL as the
default repair endpoint; kill on exit. I can write the spawn module + config +
status UI; the actual binaries/model + runtime testing are on Junna's machine
(can't ship multi-GB from here). The Repair profile selector is the override on
top of this default.

## Event-driven benchmark pacing (DONE)

Symptom (Junna): manually pausing → waiting for reply+eval → re-triggering the
benchmark was *faster* than letting it run. Cause: `waitForBenchmarkEvaluatorJob`
polled `getLatestEvaluatorJob` every 700ms between turns, so each turn ate up to
~700ms of poll lag waiting to notice the eval had committed. Fix: made it
**event-driven** — it now resolves the instant the `evaluator-job-status-changed`
event reports this conversation's job terminal (≈0 lag), with a fast-path check
(skip if nothing in flight), and only a slow 3s poll + 300s cap + Stop-check as
safety nets. Frontend-only; tsc green. (Model-call times themselves — player line,
narrator, eval — are unchanged; this only removes the between-turn waiting.)

## Dueling comparison benchmark — Stage 1 + Mode A (DONE)

Stage 1 — **traditional RP generator** (`generate_traditional_rp_turn` +
`generate_traditional_rp_message` command): full raw transcript (up to 400 msgs)
+ a "you are a Character.AI/Janitor-style partner with ONLY the transcript"
prompt, NO Soul/memory/scene/evaluator. Same streaming transport + bounded
retry/timeout as the player-sim. The control side of the comparison.

Mode A — **traditional opponent toggle**: a "Traditional RP opponent" checkbox in
the Benchmark Runner. When on, the live loop's opposing/user side uses
`generateTraditionalRpMessage` instead of the player-sim (carried on
`BenchmarkLiveContext.traditionalOpponent`); status shows "Traditional RP
thinking…". So your memory narrator converses live with a no-memory full-chat
engine — read the transcript, compare continuity. 383 app + tsc green.

Still TODO — **Mode B (two characters head-to-head)**: both sides are characters,
one memory-driven, one traditional-driven, alternating. Needs inserting the
traditional character's turns as conversation messages WITHOUT running the memory
pipeline (new plumbing) + two personas + 🧠/📜 per-turn labels. Bigger; deferred.

## Next steps / not done
0. **Mode B of the dueling benchmark** (two characters head-to-head, above).
0z. **(earlier framing)** Dueling comparison benchmark (my system vs traditional, talking live) —
   Junna picked "both modes, toggle": (A) traditional engine plays the user side
   vs memory narrator; (B) two characters head-to-head, one per engine. Plan:
   traditional RP generator (full chat, no Soul/memory/evaluator — a reframe of
   the player-sim) → Mode A toggle + 🧠/📜 labels → Mode B (needs inserting a
   second character's turns without the memory pipeline). Mode A is ~80% already
   present (player-sim is effectively traditional-no-memory on the user side).
   Recommended to smoke-test the existing uncommitted pile before stacking this on
   the fragile live loop.
0a. **Embedded repair sidecar** (above) — bundle/download + spawn + default URL.
0a. **Auto-trigger for the repair worker** (above) — surface failed-op JSON +
   fire, or backend auto-spawn (guarded). Needs runtime observation.
0b. **Original (superseded) plan:** on partial-accept's
   `rejected_candidates`, spawn a background job (reuse the bg-evaluator-job
   infra) that, per failed op, builds a *focused* repair prompt (broken op +
   reason + that op's schema + valid entity ids + the original turn text via the
   address + the `source_quote`), calls a **configurable** repair endpoint
   (provider profile / base_url → a local llama.cpp the user runs), up to **5
   attempts**, re-validates via the per-op compile path, and applies successes as
   a follow-up ledger patch. Non-blocking; failures stay dropped + logged.
   Decision (Junna): build the logic now against a configurable endpoint
   (test against a manually-run Qwen2.5-3B), bundled auto-sidecar later.
1. **Relationship axes still have the same disease** — `relationship_delta_from_op`
   ([evaluator_structured.rs]) is a one-shot linear nudge that collapses 12 axes into
   6 scalars and discards `costliness`/`repetition`; it cannot hold "asshole but has
   your back". The faceted, non-linear, costly-signaling dynamical-system overhaul
   (per the earlier design discussion) is the next big piece. Memory was done first as
   the more self-contained place to land the first "dynamics over snapshots" change.
2. **Decay cadence**: aging runs per memory-apply (≈ per state-changing turn).
   Pure-dialogue turns the evaluator skips won't age memory. Fine for v1; revisit if
   long dialogue stretches should still age memory.
3. **Reinforcement reach**: only near-duplicate merge reinforces today. Thematic
   reinforcement (different events, same relational theme) isn't modeled yet.
4. Earlier-session benchmark work (live self-play, owl-alpha resilience, eval bg-job,
   perceived_by coercion) is in `2026-06-17-live-self-play-benchmark.md`.
5. **Local-eval + grammar** direction (bundled llama.cpp sidecar, Qwen2.5-3B,
   GBNF) still pending; prove the 3B's nuance before building the packaging.

## Gotcha
Cap tests now apply memories in a single batch on purpose — if you add a new
overflow/eviction test, decide whether you're testing cap (single batch) or fade
(multi-turn via repeated `age_memories`/applies), since both now archive.
