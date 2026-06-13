# Dev Note — 2026-06-12 — Real tool-calling transport for the evaluator

Triggered by a live payload/checkpoint analysis showing `evaluator_structured_v1`
was returning malformed freeform JSON text (no provider `tool_calls`), on a weak
free model (`liquid/lfm-2.5-1.2b-thinking:free`) that fails both form and
structured output. The ask: make tool calling real, not just
`response_format: json_schema`.

## What was already there (uncommitted in the tree at session start)

A LOT. The per-op structured pipeline already existed and was wired:

- `state_engine/src/evaluator_structured.rs` (was untracked): `EvaluatorOp`
  enum (add_memory, relationship_event, update_object_state, update_scene_state,
  add_world_event, no_op), `compile_evaluator_ops_to_engine_patch`, semantic
  validation (entity/evidence/stable-object-id), `evaluator_ops_json_schema()`,
  relationship axes→delta conversion, tests.
- `providers/api.rs`: `StructuredEnforcement::{ToolCall, Grammar}` variants,
  `StructuredEvaluatorPolicy` (required/prefer/allow_fallback), the
  `complete_structured_prompt` json_schema→json_object→none ladder.
- `commands.rs`: `structured_enforcement_requested / _validated /
  schema_validation_status` trace fields, ops wiring at the evaluator runtime.

So the ChatGPT analysis was right that tool_calls weren't happening, but wrong
that the ops architecture didn't exist — it did, just driven through
`response_format` instead of tools. The gap was ONLY the transport.

This session's commit `9e7cea4` necessarily folded that uncommitted foundation
in with the new work (interleaved in the same files, couldn't cleanly split).

## Done this session (commit "Add real tool-calling transport…")

- `ChatCompletionRequest` carries optional `tools` / `tool_choice`
  (skip_serializing_if None; derives Default so the 5 literals use
  `..Default::default()`). Response structs parse `tool_calls[].function.arguments`.
- `complete_tool_call`: defines `evaluator_ops_json_schema()` as the parameters
  of ONE forced function (`submit_evaluator_ops`, tool_choice pins it), returns
  the arguments JSON as `raw_text`. The existing `EvaluatorStructuredOutputV1`
  parser + `compile_evaluator_ops_to_engine_patch` consume it UNCHANGED — no new
  conversion layer. Single-tool (not per-op multi-tool) on purpose: the model
  emits one call carrying all ops, reusing the tested compiler with zero new
  surface. A prose answer (no tool_calls) = tool-call failure.
- `complete_structured_prompt` ladder is now tool_call → json_schema →
  json_object → prompt-only, gated by transport + policy. `StructuredEnforcement`
  is reported as `tool_call` ONLY when real tool_calls came back.
- `structured_evaluator_transport` setting (api.rs `ApiProviderSettings` + TS):
  auto | tool_call | json_schema | json_object | prompt_json. Auto tries tool
  calls first. `tool_call` is strict (fails fast if the provider has no tools).
- `is_tool_call_rejection` degrades on 4xx/501 naming tools/tool_choice/function
  or "no tool_calls"; genuine 5xx/network errors propagate.
- Frontend: "Structured Evaluator Transport" select (localStorage-backed,
  mirrors the execution-mode pattern), injected at the `sendApiTurn` call site —
  deliberately NOT a provider-profile column (no migration, no carry-forward
  risk), same choice as `evaluator_execution_mode`.
- default_player leak fix: `default_speaker_resolution` now defaults an unlabeled
  user turn to the ACTIVE player persona (e.g. preset_male) via
  `db::get_active_player_persona`, and `SpeakerResolution::summary_line` for
  NoLabel reports the resolved entity. The OOC-channel path still maps to
  default_player (correct).
- Tests (api.rs): transport selector, tool-call request serialization,
  `first_tool_call_arguments` extraction, content-only/empty-args degradation,
  rejection detection. 351 app + 295 engine green.

## Why this should fix the live failure (once on a capable model)

The reported run used `liquid/lfm-2.5-1.2b-thinking:free` — 60s, half-JSON,
empty `{ "no_op_reason":` blobs. No transport fixes a model that can't follow a
schema. The user already agreed to switch models. On a tool-calling model
(OpenRouter: most OpenAI/Anthropic/Mistral-class models) with transport=auto or
tool_call, the evaluator now forces a `submit_evaluator_ops` function call and
parses the arguments. Enforcement will read `tool_call` and
`structured_schema_validation_status: validated` only when ops actually parsed
and compiled.

## Next steps / not done

1. **Live re-run REQUIRED** — I cannot exercise the real provider. Run a
   diagnostic with transport=tool_call, policy=required, on a tool-capable
   model. Pass criteria (from the ChatGPT spec): payload shows tools sent,
   response has tool_calls, add_memory/update_object/relationship_event ops
   compile, object_state_count > 0, memory_recent_count > 0, enforcement=
   tool_call, no default_player leak, no form fallback in strict mode.
2. **Trace richness**: the analysis asked for tool_calls_present / tool_call_count
   / tool_call_names fields. I did NOT add these per-call counters — the honest
   signal (enforcement==tool_call + validated) is already recorded. Add the
   counters if the diagnostic needs finer visibility.
3. **Strict tool diagnostic mode**: `diagnostic_structured_settings_from_profile`
   currently defaults transport to auto (None). For a TRUE pass/fail tool probe,
   thread transport=tool_call + policy=required so it fails fast instead of
   degrading. Small change, deferred until the live re-run proves the path.
4. **Phase-4 form fallback ladder** (from the earlier discussion) is still
   unbuilt: structured/tool failure doesn't yet retry through evaluator_form_v1
   in the same turn. Separate from this work.
5. Per-op multi-tool transport (one function per op type) was considered and
   rejected — single forced `submit_evaluator_ops` reuses the tested compiler.
   Revisit only if a provider handles many small tool calls better than one.

## Gotchas

- `ChatCompletionRequest` now derives Default; the 5 construction sites use
  `..ChatCompletionRequest::default()`. Adding a field no longer breaks them.
- `ApiProviderSettings` gained `structured_evaluator_transport`; two server-side
  literal builders (`diagnostic_structured_settings_from_profile`, the contract-
  test settings) needed the field. It is NOT on `ProviderProfile` (frontend-only,
  via localStorage), so profile literals were untouched.
- PowerShell 5.1 `git commit` with `@'...'@` here-strings failed parsing this
  session; use `[System.IO.File]::WriteAllText` to a temp file + `git commit -F`
  (and .NET WriteAllText to avoid the UTF-8 BOM that `Set-Content -Encoding utf8`
  injects into the commit title).
