# Dev Note — 2026-06-11 (session 2) — Memory curation command + Pillar 2 gate

Continues 2026-06-11-structured-engine-fixes.md. All work committed on
`main`, one commit per fix. Full suite green after every commit (336 app +
engine as of the last commit). TS typecheck clean.

## Done this session

### 1. `curate_memory` Tauri command (commit "Add curate_memory command…")

Finishes the C-item "Tauri command that writes the pin/restore patch through
the ledger". Was in-progress (uncommitted) at session start; verified + tests
+ committed.

- `curate_memory(conversation_id, soul_id, memory_id, operation)` with
  operation `pin | unpin | restore_archived`. Commits a `memory_operations`
  patch via `apply_command_patch_to_ledger` — NEVER direct soul mutation
  (ledger replay would lose it).
- Ineffective operations (missing memory, not archived, invalidated) are
  dry-run on a soul clone and rejected BEFORE anything reaches the ledger.
- Frontend binding `curateMemory()` in tauri.ts with browser-preview
  fallback. No UI buttons yet — that belongs to the Memory Inspector (D).
- Test: `memory_curation_commits_through_ledger_and_survives_rebuild`
  (pin → ledger replay reproduces it → unpin → error cases).

### 2. Token usage tracking (commit "Track provider token usage…")

Pillar 2 prerequisite (output-token tracking from the roadmap).

- `TokenUsage { prompt_tokens, completion_tokens }` parsed from the
  OpenAI-compatible `usage` block (non-streaming) and opportunistically from
  SSE chunks — some providers attach usage to the final chunk; we do NOT
  request `stream_options.include_usage` (rejection risk on strict
  OpenAI-compatible servers), so streaming narrator calls usually fall back
  to estimates.
- Carried on `ProviderCompletion`, `StructuredCompletion`, the new
  `PromptCompletion` (returned by `complete_prompt_with_usage`), and the
  evaluator's `EvaluatorCompletion`.
- `TurnPipelineTrace.token_usage: Option<TurnTokenUsage>` — narrator +
  evaluator prompt/completion counts with `*_estimated` flags; estimates via
  `estimate_tokens` when the provider reports nothing. Populated in the
  narrator save path, the sync evaluator path, and the background job (which
  restores the narrator's persisted trace, so the merge keeps both sides).
- Dev console trace panel shows "Narrator tokens: X in / Y out (est.)" +
  evaluator row.
- NOT DONE: $ cost estimate — needs per-profile input/output pricing fields
  (DB columns + the carry-forward gotcha in both App.tsx profile save
  handlers, see session-1 note item 7).

### 3. Fast-mode evaluator gate (commit "Add fast-mode evaluator gate…")

Pillar 2 Lever B, the headline item.

- `evaluator_execution_mode` on ApiProviderSettings: `fast | balanced |
  long_context`, anything else → balanced. Frontend: localStorage-backed
  select ("Execution Mode") in the State Updater settings, injected into the
  updater settings at the `sendApiTurn` call site — deliberately NOT a
  provider-profile column (no migration, no carry-forward risk).
- Gate runs in `send_api_turn` AFTER the baseline patch commits (so the
  exchange itself is never lost) and only when mode == fast &&
  is_normal_scene_turn && baseline committed.
- `classify_turn_for_evaluator_gate(user_text, current_status, prev_status)`
  → `(TurnSignificance, reason)`:
  - status signature = Focus + Physical state lines (pipe-joined AND
    per-line forms parsed; Atmosphere deliberately ignored — mood drift).
  - Focus changed → SceneBoundary; Physical state changed → SceneRelevant;
    correction keywords → SceneRelevant; user text not dialogue-like →
    SceneRelevant; else DialogueOnly.
  - dialogue-like = ≥50% of non-ws chars inside quote pairs (straight,
    curly, 「」『』) and NO `*action*` markup. Unquoted prose counts as
    action on purpose ("I draw my sword").
  - EVERY missing/unparseable signal degrades to SceneRelevant — the gate
    can only skip what it positively identified.
- Skip path: trace stage `evaluator_job_started` = skipped (gate reason),
  `state_updater_status = "skipped_dialogue_only"`, dev log
  `evaluator_skipped_dialogue_only`, exchange inserted into the new
  `evaluator_catchup_queue` table.
- Catch-up: BOTH evaluator paths (sync + background/retry) list the queue,
  append a `[CATCH-UP]` block of skipped exchanges to the evaluator user
  message, and delete the drained ids ONLY after the evaluator output parses
  — failed runs see them again. Drain is unconditional (not fast-only), so
  switching modes self-heals a leftover queue.
- SceneBoundary currently just labels the run (catch-up drains on ANY
  non-skipped turn — strictly more conservative than boundary-only).
  Long Context currently behaves exactly like balanced; it's a label until
  richer extraction / larger memory budget exist.

## Next steps (priority order)

1. **Cost estimate** (finish roadmap Pillar 2 line): optional
   input/output USD-per-1M fields on provider profiles + computed per-turn
   estimate in the trace panel. Remember the profile-save carry-forward
   gotcha (App.tsx, BOTH handlers).
2. **Field-test the gate**: run a real fast-mode session; watch
   `evaluator_gate_classified` dev logs for misclassification. The quoted
   ≥0.5 threshold and the correction-keyword list ("update", "change"…
   generous) are first guesses.
3. **C remainder — decay**: real salience decay + reinforcement on
   retrieval (consolidation.rs `decay: 0.0` placeholder). Pinned memories
   exempt.
4. **D — Memory Inspector UI**: list active/archived/invalidated memories,
   pin/unpin/restore buttons calling `curateMemory()`, click-through via
   `source_message_id`, slot-trace "why is this in my prompt".
5. **E — extract evaluator_job.rs** from commands.rs (~23k lines now) when
   next touching the job lifecycle.

## Gotchas discovered this session

- `ProviderCompletion` is constructed field-by-field in test code
  (`anti_replay_accepted_retry_payload_metadata_uses_retry_completion`) —
  adding a field breaks cfg(test) code that `cargo check` does not compile;
  use `cargo check --tests` or just run the suite.
- In the background job, `app.state::<AppState>()` + `if let Ok(conn) =
  state.conn.lock()` hits E0597 (guard outlives the temporary State) inside
  match arms; bind the lock result to a local first (see catch-up delete).
- `db::list_messages_before_id(conn, conv, before_id, limit)` exists and is
  exactly right for "previous narrator message" lookups; filter
  `role == "assistant" && channel == MESSAGE_CHANNEL_RP_SCENE`.
