# Mnemosyne Debt Registry

This registry is append-only. Resolved debt remains listed with an updated status
so the reason for structural work is not lost.

| ID | Type | Location | Debt | Status | Repayment |
|---|---|---|---|---|---|
| debt-001 | technical | `src/App.tsx` | App composition, workflow state, actions, and rendering share one component | resolved | Message lifecycle, generation UI state, provider settings, embedded repair, benchmark state, dev jobs/logging, dialogs, preferences, evaluator presentation, conversation identity, and editor models now have explicit owners. `App` remains the composition root. |
| debt-002 | technical | `src/tauri.ts` | IPC types, commands, events, and browser fallbacks share one module | resolved | Shared contracts live in `tauri/contracts.ts`; the stateful browser implementation lives in `tauri/previewRuntime.ts`; `tauri.ts` is the typed adapter surface. |
| debt-003 | technical | `src-tauri/src/commands.rs` | Unrelated Tauri adapters and application workflows are interleaved | resolved | Embedded model, session/library/archive/image adapters, evaluator/provider commands, benchmark workflows, MNE workflows, and command tests moved to owned modules. `commands.rs` now owns the chat-turn pipeline and its directly coupled guards. |
| debt-004 | technical | `src-tauri/src/db/mod.rs` | Schema ownership and aggregate repositories share one module | resolved | Models, schema/migrations, provider profiles, evaluator jobs, and DB tests now have module boundaries; compatibility re-exports preserve callers. |
| debt-005 | technical | frontend tests | Extracted workflow rules lack fast automated coverage | resolved | `scripts/test-frontend-models.mjs` characterizes message lifecycle, benchmark completion/fallback rules, and dev argument parsing; existing slash-command coverage remains. |
| debt-006 | intent | State Map | Read-only aggregate UI does not yet implement correction/invalidation loop | deferred | Product milestone after structural baseline |
| debt-007 | technical | build verification | Full Rust suite was not executed during the first extraction pass | resolved | Full library suite executed after updating two path-coupled tests for the new module owners: 415 passed, 1 ignored. |
| debt-008 | technical | Rust formatting | Existing modified production lines were not fully rustfmt-clean | resolved | All newly introduced Rust modules and touched test sections were rustfmt-formatted; unrelated pre-existing dirty-worktree edits were not rewritten. |
| debt-009 | authority | `state_engine/evaluator_structured.rs` | Structured evaluator schema allowed the LLM to assert `verified_engine` and `actual_system_event` truth | resolved | Provider schema no longer exposes engine-only truth values, while semantic validation also rejects legacy or adversarial payloads that contain them. |
| debt-010 | provenance | Evaluator memory lowering and `stamp_memory_provenance` | LLM-provided message/conversation/session addresses could survive into committed memories | resolved | Structured lowering discards the legacy message ID, and the application overwrites all address fields from trusted creating-turn context before ledger commit. |
| debt-011 | grounding | Structured scene updates | `update_scene_state` was the only durable structured operation without evidence grounding | resolved | New schema requires `evidence_quote`; old V1 payloads remain parseable but ungrounded scene operations are rejected semantically. |
| debt-012 | quality | Evaluator regression measurement | Safety and durable-effect behavior had unit tests but no versioned cross-case golden corpus with deterministic replay checks | resolved | Added an 11-case starter corpus and runner covering valid effects, partial acceptance, hearsay, OOC, evidence fabrication, truth escalation, provenance spoofing, and byte-stable patch replay. |
| debt-013 | architecture | `state_engine` evaluator pipeline | No explicit compiler boundary existed between LLM interpretation and engine effects | resolved | Added transport/database-independent source, perception, binding, semantic, lowering, and simulation contracts under `state_engine::compiler`. |
| debt-014 | authority | Memory Compiler V2 contracts | A single serialized type could have allowed model-written perception and engine-owned identity/effects to become conflated | resolved | Split LLM-writable `PerceptionBatchDraft` from Rust-sealed `PerceptionBatch`; unknown authority/effect fields are rejected and source/candidate/effect IDs are attached only by code. |
| debt-015 | integrity | Compiler artifacts | Cross-turn or modified compiler artifacts lacked a shared integrity contract | resolved | `SourceEnvelope` is tamper-evident, candidate/effect identities are source-bound and deterministic, and proposed transactions reject cross-source effect injection. |

## Structural Baseline After Repayment

Large files are no longer split merely by line count: each remaining large module
has one cohesive ownership boundary. Future extraction should be driven by a
behavioral change or an independently testable lifecycle, not by a target file
size.

`debt-006` is intentionally not technical debt. It records unimplemented product
intent and remains deferred until the State Map correction workflow is selected
as a product milestone.
