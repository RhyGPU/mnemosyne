# Dev Note — 2026-06-11 — Engine fixes toward the schema-enforced evaluator

Session scope: start executing the roadmap (docs/state-map-roadmap.md) in
priority order — dev-speed/quality fixes first, then the structured-output
foundation for Pillar 1. All work is committed on `main`, one commit per fix.
Full workspace test suite is green after every commit (325 app + 290 engine
as of item 7).

## Done this session

### 1. Primary-slot memory dedup (commit "Add primary-slot memory dedup…")

Pillar 3 item. The same memory can no longer appear in multiple prompt
sections.

- `context_compiler.rs`: before slot filling, every memory is assigned ONE
  primary slot. Ranking: evaluator-assigned `memory_slot` label > explicit tag
  affinity (`slot_tag_affinity`, mirrors the tag checks in
  `slot_matches_memory`) > slot score; ties keep `MemorySlot::all()` order.
- Slot traces record `deduplicated_primary_slot_elsewhere` for losing slots,
  so the Dev Console can show why a memory was not in a section.
- Two source-gating tests were asserting incidental section placement from the
  old duplicate-everywhere behavior; they now assert visibility + labeling
  across the whole compiled context (the actual intent).
- New regression test: `memory_appears_in_at_most_one_prompt_section`.

### 2. Archival eviction replaces the destructive memory cap (commit "Archive memory overflow…")

Pillar 3 item. Lower-salience memories are no longer hard-deleted.

- `MemoryEntry` gains `archived: bool` (serde-default false — old souls and
  `.mne` bundles load unchanged).
- `patch.rs` apply: active pool still capped at `MAX_RECENT_MEMORIES = 12`
  (salience-sorted), but overflow is marked `archived = true, is_active =
  false` instead of truncated. Everything that filters on `is_active` behaves
  exactly as before; archived memories stay stored/exportable/restorable.
- `MAX_STORED_MEMORIES = 200` is a hard ceiling backstop (salience-sorted
  truncate) so a soul cannot grow without bound.
- Semantics: `archived` = evicted by cap, restorable; `is_active = false`
  without `archived` = invalidated/retconned. The future Memory Inspector can
  distinguish them.
- New test: `memory_overflow_is_archived_not_deleted`.

### 3. Memory provenance stamping (commit "Stamp memory provenance…")

Pillar 3 / Memory Inspector prerequisite.

- New `stamp_memory_provenance` in commands.rs: every new memory in an
  evaluator patch gets `source_conversation_id` + `source_message_id` (the
  assistant message of the creating exchange) filled if missing. Evaluator-
  provided provenance is never overwritten.
- Wired in BOTH evaluator paths: synchronous (after
  `sanitize_state_updater_patch`, ~line 8140) and background job (~line 12830).
- New test: `memory_provenance_is_stamped_without_overwriting`.
- Still unpopulated: `source_session_id`, branch/turn IDs, `confidence` for
  most paths. Add when the Memory Inspector needs them.

### 4. Structured-output provider foundation (commit "Add structured-output foundation…")

Pillar 1 step 1. The provider layer can now request provider-enforced JSON.

- `ChatCompletionRequest` gains optional `response_format` (skipped when None;
  narrator/streaming paths unchanged).
- `ApiProvider::complete_structured_prompt(settings, system, user, temp,
  timeout, schema_name, schema)` — degradation ladder:
  1. `response_format: {type: json_schema, strict: true}` (schema-validated)
  2. `response_format: {type: json_object}` (valid JSON guaranteed)
  3. no enforcement (prompt-only)
  Returns `StructuredCompletion { raw_text, enforcement }` so callers can put
  the achieved level in the pipeline trace. `is_response_format_rejection`
  distinguishes "provider rejected the parameter" (degrade) from real failures
  (propagate) — 4xx + names the field.
- `evaluator_patch_json_schema()` — strict-mode JSON Schema for the evaluator
  engine patch: every object closed (`additionalProperties: false`), every
  property required, optionality via nullable types (what OpenAI-strict
  demands). Covers soul_patch (relationship_deltas, new_memories), world_patch
  (location, time, scene_state, event_operations, correction_note), body_patch.
- Tests: serialization with/without response_format, rejection detection,
  recursive strict-mode invariant check, and round-trip proof that
  schema-shaped output parses into `EnginePatch` with serde alone — no repair
  layer involved.

### 5. Memory pinning + archive restore (commit "Add memory pinning…")

Pillar 3 / Memory Inspector prerequisite.

- `MemoryEntry` gains `is_pinned: bool` (serde-default false). Pinned memories
  are exempt from archival eviction and the stored-pool hard ceiling, and do
  not consume active-cap slots.
- Engine helpers in `memory.rs`: `set_memory_pinned` (pinning an archived
  memory also restores it) and `restore_archived_memory` (refuses
  invalidated/retconned memories — those are corrections, not evictions).
- IMPORTANT DESIGN CONSTRAINT: pin/restore are wired as **patch operations**
  (`operation: "pin" | "unpin" | "restore_archived"` in
  `memory_operations`/`new_memories`), NOT direct soul mutation, because the
  materialized soul is rebuilt by replaying the patch ledger — direct mutation
  would be silently lost on rebuild. Any future UI command for pin/edit/
  invalidate must write a patch through the ledger, not poke the soul.
- Tests: `pin_and_restore_flow_through_patch_operations` (ledger replay
  reproduces pin state), `pinned_memory_is_exempt_from_archival_eviction`,
  plus helper tests in memory.rs.

### 6. New evaluator mode `evaluator_structured_v1` (commit "Add evaluator_structured_v1 mode…")

Pillar 1 payoff, steps 1–4 of the original wiring plan.

- `EVALUATOR_MODE_STRUCTURED_V1 = "evaluator_structured_v1"` selectable per
  profile; `selected_evaluator_source` now three-way (form / structured / v1).
- `complete_evaluator_with_config` returns `EvaluatorCompletion { raw_text,
  structured_enforcement }` and routes structured mode through
  `complete_structured_prompt` + `evaluator_patch_json_schema()`.
- System prompt: `build_structured_evaluator_prompt` = state-updater rules +
  `[CURRENT STATE]` WITHOUT the embedded example patch JSON (refactored out
  of `build_state_updater_prompt` as `state_updater_current_state_block`).
- `compile_evaluator_structured_runtime`: serde-only parse straight into
  `EnginePatch`, no EvaluatorOutputV1 stage, no conversion layer. Under
  `json_schema` enforcement ALL syntactic repair is skipped (strict-parse
  failure = contract break, hard error); at weaker levels the code-fence
  stripping of `parse_engine_patch_json` (now a runtime fn, no longer
  cfg(test)) remains as fallback. Sanitize / strip-premature-events /
  provenance stamping still run on the parsed patch in both paths.
- `structured_enforcement` label recorded in all four evaluator trace builds
  (sync+background × success+failure) and the background
  `evaluator_response_received` stage output.
- Contract test pins its LLM call to form mode — it exercises the FORM
  contract and must not route through the structured path.
- Tests: mode selection/labels, prompt omits embedded schema, strict parse
  without conversion, repair-skip under json_schema vs salvage under
  json_object, routing through `compile_selected_evaluator_runtime`.

### 7. Contract test probes structured support (commit "Probe structured-output support…")

Step 5 of the wiring plan.

- `run_evaluator_contract_test` additionally probes
  `complete_structured_prompt` and persists the achieved level on the
  profile: `structured_output_support` — 0 untested/failed, 1 prompt-only,
  2 json_object, 3 json_schema. Level counts only if the probe output parsed
  into `EnginePatch`. Informational; never flips form-contract pass/fail.
- New `provider_profiles.structured_output_support INTEGER NOT NULL DEFAULT
  0` via `add_column_if_missing`; `#[serde(default)]` on the struct field.
- IMPORTANT: frontend profile saves (App.tsx, both narrator + updater save
  handlers) must carry `structured_output_support` forward from the existing
  profile — an omitted field deserializes to 0 and would silently wipe the
  probed value on every settings edit. Both handlers do this now; any new
  profile-save path must too.

## Next steps, in order (the wiring plan)

### A. Remaining structured-evaluator work (Pillar 1)

1. Once stable, make `evaluator_structured_v1` the default for profiles that
   probe at `json_schema` level (`structured_output_support == 3`); form_v1
   remains the fallback for the rest. There is no UI for selecting the mode
   or showing the probed level yet — only the TS types know about it.
2. Long-term: schema grows to cover object_observation_operations and the
   remaining world fields, then the form layer (~10k lines in
   evaluator_form/) shrinks to semantic validation only.

### B. Conditional evaluator + modes (Pillar 2)

- Gate: skip the evaluator for dialogue-only turns (signal: narrator's
  ```status``` block + cheap user-text heuristic), batch catch-up on scene
  boundary. Expose Fast / Balanced / Long Context. The skip plumbing already
  exists for slash commands (`evaluator_skip` reasons) — reuse it.
- Output-token + cost estimate in `TurnPipelineTrace` (input-side estimates
  already exist in the payload panel).

### C. Memory follow-ups (Pillar 3 remainder)

- Decay: implement real salience decay (consolidation.rs has a `decay: 0.0`
  placeholder) + reinforcement on retrieval.
- Pinning: `is_pinned: bool` on MemoryEntry; pinned memories are exempt from
  archival eviction and decay.
- Restore path: DONE at the engine level (see item 5). Still needed: a Tauri
  command that writes the pin/restore patch through the ledger + UI buttons.

### D. Memory Inspector / State Map V1 (Pillar 4)

Provenance + archival + slot traces from this session are the data layer.
UI: list all memories (active/archived/invalidated), click-through to source
message via `source_message_id`, pin/edit/invalidate actions, and the
`deduplicated_primary_slot_elsewhere` / `selected` slot trace as "why is this
in my prompt".

### E. Decomposition (Pillar 5, ride-along)

commands.rs is ~22k lines. When doing (A), extract the evaluator job
lifecycle (spawn/retry/fallback/streak) into `evaluator_job.rs` — it is the
most self-contained slice and (A) touches all of it anyway.

## Notes / gotchas discovered

- `cargo fmt -p state_engine` fails from `src-tauri/` ("not a member of the
  workspace") — run it from `src-tauri/state_engine/`.
- Test memories with near-identical wording get MERGED by
  `mergeable_memory_index` (jaccard > 0.64 on token sets) instead of added —
  test fixtures need lexically distinct contents.
- Slot scoring bonuses are mostly slot-independent; only the evaluator
  `memory_slot` label (+36) and `slot_matches` (+24) differ per slot. That is
  why primary-slot assignment needed the tag-affinity tie-break — raw argmax
  degenerates to first-slot-wins.
- The `#[derive]` above `SseStreamMetadata` is easy to orphan when inserting
  code above that struct (E0774) — insert below the impl block instead.
