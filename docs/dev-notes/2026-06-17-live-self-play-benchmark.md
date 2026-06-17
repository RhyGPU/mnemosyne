# Dev Note — 2026-06-17 — Live AI-vs-AI benchmark (visible chat self-play)

## The ask

"Fix benchmark feature. I need AI to talk to AI in visible chat instead of me,
like I would input it, so I can stress-test the system. It ain't working at
all." The whole benchmark feature was uncommitted WIP (not in any commit) and
compiled fine — so the problem was behavioral, not a build error.

## Root cause

`run_benchmark` was **one blocking Tauri command**. It looped through every
turn server-side (generate AI player message → `send_api_turn` → evaluator) and
the frontend only reloaded the chat **after the whole run finished**. So during
a run the UI just sat on "Running benchmark…" and then dumped a scorecard. You
never saw the AI-vs-AI exchange happen live in the chat — which is the entire
point of watching a stress test.

Meanwhile the app already had the perfect live path: `executeTurn(text)` — the
exact function that runs when you type a message (seeds the user bubble, streams
the narrator reply, runs the evaluator). The benchmark just wasn't using it.

## What changed — frontend-driven live loop (decision: live + keep scorecard)

`visible_ai_chat` on the API provider now drives turns from the **frontend**,
one `executeTurn` per turn, so each AI player message is sent exactly as if the
user typed it and the narrator streams live. Other modes (scripted / headless /
mock) keep the blocking `run_benchmark` path unchanged.

### Backend (`commands.rs`, `lib.rs`)
Split the blocking orchestration into reusable pieces and exposed four commands:
- `prepare_benchmark_session` → resolves/creates the conversation, marks it
  benchmark, snapshots initial memory/object/relationship counts, returns
  `BenchmarkSessionInit { benchmark_id, conversation_id, session_soul_id,
  started_at, initial_*_count }`. (Extracted from `run_benchmark`'s setup block
  into `prepare_benchmark_conversation`, shared by both paths.)
- `generate_benchmark_player_message` → thin async wrapper over the existing
  `generate_benchmark_player_turn`; returns only the next user text.
- `benchmark_turn_summary` → wraps `build_benchmark_turn_summary` so the
  frontend captures per-turn counts/trace after each live turn commits.
- `finalize_benchmark` → builds the summary from the recorded per-turn results,
  then runs exports + scorecard via the new shared `finalize_benchmark_summary`
  (also extracted from `run_benchmark`'s tail, so both paths emit identical
  artifacts: payload history, .mne, summary JSON, dev-log).

`run_benchmark` itself is unchanged in behavior — it now just calls the two
extracted helpers.

### Frontend (`App.tsx`, `tauri.ts`)
- `executeTurn` gained an optional 5th arg `updaterOverride?: Partial<ApiProviderSettings>`,
  merged last into the evaluator settings at the `sendApiTurn` call site. Normal
  sends pass nothing (no behavior change); the benchmark passes the strict
  tool-calling + wait-each-turn overrides so the evaluator commits synchronously
  before the next player message.
- New live state: `benchmarkLiveActive`, `benchmarkTurnsRemaining`, and refs
  `benchmarkCtxRef` (mutable `BenchmarkLiveContext`), `benchmarkTurnInFlightRef`
  (re-entry guard), `benchmarkStopRef`.
- An effect drives the loop: when the chat is settled (`!busy && !stateUpdating`),
  the benchmark conversation is active, and the soul matches, it fires the next
  turn (or finalizes when turns are exhausted / stop requested). One turn at a
  time, guarded by `benchmarkTurnInFlightRef`. Depends on `messages` so it wakes
  after each streamed turn settles.
- `handleRunBenchmark` branches to `startLiveBenchmark` for
  `visible_ai_chat` + API; everything else keeps the blocking call.
- `runOneBenchmarkTurn`: generate player text → `executeTurn(...)` (live) →
  `benchmark_turn_summary` → decrement. On failure: record error turn, set stop.
- `finishBenchmarkLive`: `finalize_benchmark` → set result, reload messages,
  show PASS/FAIL.
- Wired the previously-dead **Stop Benchmark** button (`handleStopBenchmark`):
  stops after the current turn, then finalizes what completed.

## Faithful-repro refinement (same session, after first pass)

Architect feedback: "it's to be just like when I type so I can export payload,
session and all details to see the issue underneath." The first pass forced the
strict tool-calling evaluator override onto every live turn, which would mask
the real pipeline being diagnosed. Fixed so a live run hits the **identical**
pipeline as a typed turn:

- `benchmarkLiveUpdaterOverride` now only injects evaluator mode/transport/
  policy/retries when **Strict Tool Evaluator** is explicitly ON. OFF (the new
  default) passes no evaluator override — your exact chat settings flow through
  `executeTurn` untouched. The only non-strict override is the optional
  turn-sequencing wait (pure timing; doesn't change payload content).
- Default of `benchmarkStrictToolEvaluator` flipped to `false` so the
  out-of-the-box Visible AI Chat run is a faithful repro. Strict is now labeled
  "diagnostic probe — overrides your chat evaluator settings".
- Only the *source* of the user message differs from manual typing (AI player
  vs. human); the narrator + evaluator + payload-logging pipeline is identical,
  so the exports below reflect real behavior.

Exports (always on for benchmark runs, produced in `finalize_benchmark`, and
still produced on Stop / failure): payload history `.md` (every turn's full
LLM request/response + `pipeline_trace_json`), `.mne` session checkpoint, and
summary JSON (per-turn tool-call/retry/fallback + memory/object/relationship
deltas). Paths surface in the Benchmark Runner panel. Because the run is a real
conversation, the app's normal export buttons also work on it afterward.

## First live run: 0/5, root-caused to a non-streaming decode bug

First real run (provider owl-alpha, conversation id just happens to contain
"local-mock" — it is NOT the mock provider) failed 0/5. Summary turn 0:
`simulated_user_message: ""`, `narrator_error: "API response parse failed:
error decoding response body"`. Confirmed the **live path ran** — that empty
user message + parse error is the live loop's signature when the player-line
generation throws before `executeTurn`.

Root cause: the player line is generated by `generate_benchmark_player_turn` →
`complete_prompt_with_usage` → `complete_prompt_with_format`, which decoded the
response with `response.json::<ChatCompletionResponse>()`. The strict struct
choked on owl-alpha's body shape (most likely `content` as an array of typed
parts, or a 200 with an `{ "error": ... }` envelope). The narrator survives the
same model only because it uses the **streaming** SSE parser, not this one. And
this path logged nothing, so the raw body was discarded — invisible.

Fix (`providers/api.rs`): added `read_chat_completion` + lenient helpers
(`lenient_chat_content`, `content_value_to_text`, `provider_error_message`,
`lenient_token_usage`, `truncate_for_error`). It tries the strict struct first,
then falls back to a `Value` walk that handles content-as-array, `delta`, and
200-error envelopes. On a genuinely unparseable body it returns the **raw body
(truncated 2000 chars) in the error**, so it surfaces in `narrator_error` and
the panel. Applied to both non-streaming sites (`complete` and
`complete_prompt_with_format`); removed the now-dead `prompt_completion_trace`.
6 new unit tests cover the shapes. This is the boring decode bug — not the
inject-and-respond loop, which already worked.

If the next run still fails, `narrator_error` will now carry owl-alpha's actual
body. If that shows the model only behaves on streaming, the follow-up is to
route the player line through `complete_streaming_messages` (same transport as
the narrator) with a no-op chunk sink.

## Second live run: 1/5, pipeline proven, killed by a transient provider error

Re-run after the decode fix: **turn 0 is a full success** — player line →
streaming narrator → evaluator (structured tool_call, one retry, succeeded) →
memory 4→8. The inject-and-respond loop is proven end to end.

It stopped on turn 1. The decode fix did its job: `narrator_error` now reads
`"API provider returned an error in a 200 OK body: Provider returned error"` —
owl-alpha intermittently returns a 200 wrapping `{ "error": … }`. The turn-1
`simulated_user_message` being byte-identical to turn 0 was a red herring: the
**player-line** call failed (before `executeTurn`), and the catch logged the
stale `lastPlayerText`. So it was the same transient class, not a duplicate
generation and not a narrator failure.

Fixes:
- `generate_benchmark_player_turn` retries the player-line call up to 3× with
  backoff on **transient** errors (`is_transient_provider_error`: 200-error
  envelope, timeout, transport drop, 429/5xx). Persistent/shape errors (bad
  JSON, missing content, 4xx auth) are NOT retried — they still surface. So a
  single owl-alpha hiccup no longer aborts a multi-turn run, but a real problem
  is still visible. (`std::thread::sleep` for backoff — no tokio dep; matches
  the existing `wait_for_benchmark_evaluators` pattern.)
- Frontend `runOneBenchmarkTurn` now tracks a per-turn `playerText` (null until
  generated), so a generation failure records an **empty** message + a stage-
  tagged error (`"player line generation failed: …"` vs `"narrator turn
  failed: …"`) instead of the previous turn's text.
- Unit tests: `transient_provider_errors_are_retryable` (commands) + the 6
  api.rs parsing tests.

If owl-alpha keeps erroring even with retries, it's the model, not the system —
turn 0 proved the pipeline. Use a steadier player/narrator model, or (next step)
route the player line through `complete_streaming_messages` since streaming
survives owl-alpha where non-streaming flakes.

## Third live run: 2/5, hung 30+ min, Stop didn't work — fixed

Narrator switched to `poolside/laguna-m.1:free`, player still `owl-alpha`. Two
full turns succeeded (memory→11, objects→3, relationships→2 — system clearly
works). But `completed_at - started_at` = **1907s ≈ 31.8 min**, and Stop did
nothing. Turn 2 had no per-turn entry and 0 narrator_failures: its player-line
call sat uninterruptible, Stop set the flag but couldn't break the in-flight
backend command, and when it finally returned the stop-check bailed silently.

Two root causes (both from the retry I added the prior pass):
1. **Retry amplified the hang.** 3 attempts × the profile's (large) narrator
   timeout, inside one uninterruptible Tauri command = up to ~30 min.
2. **Stop only checked between awaits** and never aborted the in-flight stream.

Fixes:
- `generate_benchmark_player_turn` caps each attempt at **90s**
  (`PLAYER_LINE_MAX_TIMEOUT_MS`, honoring a smaller profile timeout) and drops
  to **2 attempts**. Worst case ~3 min, bounded. The cap is harness-only; it does
  not touch the faithful narrator pipeline.
- `handleStopBenchmark` now calls `generationAbortRef.current?.abort()` (same
  mechanism as `handleStopGeneration`) so an in-flight narrator stream stops
  immediately.
- `runOneBenchmarkTurn` checks `benchmarkStopRef` again right after
  `executeTurn`, so a stopped/aborted turn isn't recorded as a partial/bogus
  turn — the effect just finalizes what completed.

Net: Stop is effective in seconds if the narrator is streaming, ≤~3 min if stuck
in a player-line backend call. owl-alpha remains the weak link as the *player*
model — if it keeps stalling, switch the player-sim profile to a steadier model
(laguna worked as narrator) or route the player line through
`complete_streaming_messages` (streaming survives these models; non-streaming
flakes). Code change deferred — the bounded timeout + retry makes it survivable.

## Player line moved to the streaming transport (durable owl-alpha fix)

owl-alpha kept flaking as the *player* model, so the player line now generates
through the **same streaming transport the narrator uses** instead of the
non-streaming JSON decode:
- `generate_benchmark_player_turn` calls `provider.complete_streaming(&settings,
  system_prompt, &user_prompt, |_chunk| Ok(()))` — a no-op chunk sink (nothing is
  surfaced; the player line appears as a complete user message via `executeTurn`,
  not streamed as if it were the narrator). The streaming SSE parser survives the
  200-error envelopes / odd shapes that broke the strict body parser.
- The 90s cap is now applied by setting `settings.narrator_timeout_ms` (the
  streaming transport reads its timeout from there); the 2-attempt bounded retry
  is unchanged. `is_transient_provider_error` also catches `"api stream failed"`
  (mid-stream drop). Note streaming uses a fixed temperature (0.85) vs the old
  0.7 — fine / arguably better for player-line variety.
- The lenient non-streaming `read_chat_completion` path stays (the evaluator and
  other callers still use it) — this just takes the player line off it.

## Evaluator-tracker UI made live during benchmarking (background-job + loop wait)

Symptom: the evaluator banner was dark during benchmark runs. Cause: it's fed
only by `evaluator_job_status_changed` events, which fire only for **background**
evaluator jobs. The wait-each-turn override forced **inline** eval
(`evaluator_background_enabled: false`) so no job/events existed — eval ran, just
untracked. `send_api_turn`'s background branch ([commands.rs:10458]) spawns the
job and returns immediately (it does NOT honor `wait_for_evaluator_before_next_turn`).

Fix (frontend only): when "Wait for evaluator each turn" is on,
`benchmarkLiveUpdaterOverride` now sets `evaluator_background_enabled: true`
(+`allow_send_with_stale_state: true`) instead of forcing inline, so a tracked
job is created and the banner updates live. To keep per-turn commit ordering,
`runOneBenchmarkTurn` calls the new `waitForBenchmarkEvaluatorJob` after
`executeTurn` — it polls `getLatestEvaluatorJob` (700ms) until the job is
terminal (or Stop / 300s cap), then captures the turn summary and proceeds. No
job (dialogue-only skip) → returns immediately. Net: same ordering as before,
but the evaluator banner tracks each turn's job live. (Note: with wait-each-turn
OFF, eval follows the app's background setting and the loop doesn't wait —
throughput mode.)

## The real "memory never grows" bug: soul-only field rejection (FIXED)

Run after the background-eval change: 1/5, memory did NOT grow. Root cause was
NOT a tool-call failure — laguna produced a valid, parsed patch, but it put the
player (`active_player` / `preset_male`) in `relationship_event.perceived_by_entity_id`,
a **soul-only** field. `resolve_soul_alias_field` hard-rejected it, and rejection
is **all-or-nothing**, so the perfectly good `add_memory` + `update_scene_state`
ops were thrown out with it → form_v1 fallback returned truncated JSON → noop →
nothing saved. This is *the* recurring reason memory wasn't growing.

Fix ([evaluator_structured.rs] `resolve_soul_alias_field` + `is_player_entity_id`):
narrator-first, a relationship event is recorded/perceived by the **active
Soul** — never a player. So a player alias OR raw player id in a soul-only field
(`source_soul_id` / `perceived_by_entity_id`) now **coerces to the active Soul**
(logged in the trace as a warning, so the model's mistake stays visible) instead
of rejecting the whole patch. Two regression tests added; 304 engine tests green.
This fixes it on the current remote models TODAY — provable on the next run
(memory should grow even when the model mislabels perceived_by).

(Still all-or-nothing for *other* validation errors — partial-accept remains a
possible follow-up, but the soul-field coercion was the actual culprit here.)

## Next: durable fix = constrained decoding on an embedded local model

Decision (architect): move the evaluator off flaky remote free models onto a
**bundled** local model (no user-installed deps) with **grammar-constrained
decoding** (GBNF) — the `StructuredEnforcement::Grammar` variant already exists in
api.rs but is unused. Grammar makes invalid output impossible AND can constrain
entity-id fields to the live valid ids (so `perceived_by` *cannot* be a player at
the decoding level — the bug dies in two places). Plan: embed llama.cpp in-process
(`llama-cpp-2` crate) or as a bundled sidecar; ship/download a small (1–3B Q4)
model; generate a GBNF grammar from the ops schema with entity-id allow-lists;
route the evaluator through it; partial-accept as the safety net. Fine-tuning a
phone-size model is the optional last mile (distill from a strong model via the
existing compiler as validator). Sequenced AFTER the coercion fix is proven.

## Status
- `cargo check` (app + engine), `cargo test --no-run`,
  `cargo test providers::api` (46), `cargo test` state_engine (304),
  `transient_provider_errors_are_retryable`, and `tsc --noEmit` all green.
- NOT yet exercised against a live provider (no GUI/provider here). Same caveat
  as the tool-calling note: needs a tool-capable model.

## Next steps / not done
1. **Live run REQUIRED.** Run a Visible AI Chat benchmark (API provider, a
   Player Simulator profile selected). Watch: AI player message appears as a
   user bubble, narrator streams its reply, evaluator runs, next turn fires
   automatically. Confirm the scorecard at the end matches.
2. **Layout:** confirm the chat is visible while the Benchmark Runner panel is
   open — the messages stream into the active conversation, but if the panel
   overlays the chat the user has to close it to watch. Consider showing the
   live transcript in-panel or auto-collapsing the panel on start.
3. `multi_agent_visible_chat` still routes through the blocking path (deferred
   per product-direction-narrator-first). Route it through the live loop only
   if/when multi-character ships.
4. Mock provider self-play still uses the blocking `run_benchmark` (player sim
   needs a real API profile). Fine as-is.
5. Per-turn summaries are captured live and are accurate; if a turn's evaluator
   is allowed to run in background (wait-each-turn off), the after-counts could
   lag. The override forces synchronous commit when wait-each-turn is on (the
   default), so this only matters if someone disables the wait.

## Gotchas
- The live loop relies on `executeTurn` resolving only after the turn fully
  commits (narrator + evaluator). The `updaterOverride` forces
  `wait_for_evaluator_before_next_turn: true` +
  `evaluator_background_enabled: false` so the evaluator is inline, not
  background — otherwise the next turn could start on stale state.
- `benchmarkCtxRef` is a ref, not state, on purpose: the effect mutates
  `perTurn`/counters per turn without triggering re-render storms. Only
  `benchmarkTurnsRemaining` (state) drives the effect cadence.
- The effect gates on `currentConversationId === ctx.conversationId` and
  `soul.character_id === ctx.soulId`. For the supported targets these always
  match (same soul, switched conversation); if they didn't the loop would idle
  rather than send to the wrong place.
