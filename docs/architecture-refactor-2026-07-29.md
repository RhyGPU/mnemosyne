# ADR: Feature-Oriented Application Boundaries

- Date: 2026-07-29
- Status: Accepted and implemented
- Scope: structural refactoring only; no intentional product behavior changes

## Context

Mnemosyne has a stable `state_engine` crate and a broad working product surface,
but application orchestration accumulated in four oversized files:

| File | Baseline lines | Mixed responsibilities |
|---|---:|---|
| `src/App.tsx` | 8,298 | navigation, state ownership, workflows, rendering, diagnostics |
| `src/tauri.ts` | 4,159 | IPC types, commands, browser fallback, event listeners |
| `src-tauri/src/commands.rs` | 32,791 | command adapters, workflows, providers, repair, benchmarks, tests |
| `src-tauri/src/db/mod.rs` | 8,283 | schema, migrations, repositories, tests |

Moving JSX or Rust functions into arbitrary files would reduce line counts without
creating useful boundaries. The refactor therefore follows feature ownership on
the frontend and adapter/application/domain boundaries on the backend.

## Decision

Frontend code is organized by feature ownership:

```text
src/
|-- app/                  app composition and cross-feature infrastructure
|-- features/
|   |-- benchmark/
|   |-- chat/
|   |-- dev/
|   |-- library/
|   `-- settings/
|-- tauri/                browser preview runtime and shared contracts
|-- tauri.ts              typed IPC adapter surface
`-- components/           reusable or feature-facing presentation components
```

Feature folders may contain presentation, state/reducer logic, and controllers.
They must not call raw `invoke` directly; typed IPC remains behind `src/tauri`.

Backend code moves toward these boundaries:

```text
src-tauri/src/
|-- commands.rs           chat-turn pipeline and tightly coupled guards
|-- commands/
|   |-- evaluator.rs      evaluator/provider command application service
|   `-- session.rs        session/library/archive/image command adapters
|-- commands_tests.rs     command characterization tests
|-- embedded_model.rs     local model process lifecycle
|-- benchmark/            benchmark application service and contracts
|-- mne/                  archive, contracts, and import/export service
|-- db/
|   |-- models.rs         persistence contracts
|   |-- schema.rs         schema, migrations, and backfills
|   |-- repositories/     aggregate repositories
|   |-- mod.rs            repository surface and compatibility exports
|   `-- tests.rs          database characterization tests
|-- providers/            external model transport
`-- state_engine/         domain library; no Tauri or SQLite dependency
```

New crates are created only when a module has an independent reuse or lifecycle
boundary. The existing `state_engine` crate remains the domain boundary.

## Implemented Boundaries

- Chat message streaming and persistence reconciliation moved to
  `features/chat/model/messageLifecycle.ts`.
- Evaluator status presentation and conversation identity rules moved under
  `features/chat/model`.
- Dev job tracking and logging moved under `features/dev`.
- Dialog orchestration and local preference persistence moved under `app`.
- Soul/World editor mapping and import normalization moved under
  `features/library/model`.
- Chat generation/viewport lifecycle, provider settings, embedded repair-model
  lifecycle, and benchmark state moved to feature controllers.
- Shared IPC contract types moved to `tauri/contracts.ts`.
- Stateful browser fallbacks moved to `tauri/previewRuntime.ts`.
- Embedded model process ownership moved out of the command monolith.
- Session/library/archive/image, evaluator/provider, benchmark, and MNE command
  workflows moved to owned Rust modules.
- DB records, migrations, provider profiles, and evaluator jobs moved to owned
  modules.
- Command and DB tests moved out of production implementation files.
- Fast frontend characterization tests were added for extracted pure rules.

After this pass, the orchestration files are:

| File | Current lines | Reduction |
|---|---:|---:|
| `src/App.tsx` | 6,625 | 1,673 |
| `src/tauri.ts` | 2,030 | 2,129 |
| `src-tauri/src/commands.rs` | 14,268 | 18,523 |
| `src-tauri/src/db/mod.rs` | 3,744 | 4,539 |

The remaining large modules are cohesive composition/pipeline owners. Line count
alone is no longer used as the extraction criterion.

## Invariants

- Existing Tauri command names and serialized payload shapes stay unchanged.
- Existing database schema and migration behavior stay unchanged.
- Existing browser-preview fallbacks stay available.
- Existing provider and evaluator behavior stays unchanged.
- Existing user changes in the dirty worktree are preserved.
- File movement is not combined with product behavior changes.

## Refactoring Method

1. Establish typecheck/build/test baselines.
2. Extract pure or already-characterized code first.
3. Keep compatibility exports while imports migrate.
4. Move one owned feature or lifecycle boundary at a time.
5. Run proportional verification after every boundary.
6. Remove a compatibility surface only after all consumers use the new boundary.

## Rejected Alternatives

- Big-bang rewrite: too much regression risk in session, evaluator, and branch
  workflows.
- Layer-only frontend folders: global hooks/reducers/services would scatter one
  feature across the tree.
- One file per function: navigation overhead without ownership.
- Immediate state-management dependency: focused hooks and pure functions are
  enough before adding another runtime dependency.
- `include!`-based Rust file splitting: reduces file size without creating real
  module visibility or dependency boundaries.
- Immediate multi-crate backend workspace: more crates would currently be empty
  ceremony around the existing `state_engine` boundary.

## Verification

- `node node_modules/typescript/bin/tsc --noEmit`: pass
- `node scripts/test-frontend-models.mjs`: pass
- `node scripts/test-slash-commands.mjs`: pass
- `node node_modules/vite/bin/vite.js build`: pass
- `cargo check --manifest-path src-tauri/Cargo.toml --lib --jobs 2`: pass
- `cargo test --manifest-path src-tauri/Cargo.toml --lib --no-run --jobs 2`: pass
  (416 tests compiled)
- Full Rust library suite: 415 passed, 1 ignored. Two source-path
  characterization tests initially identified stale file ownership assumptions;
  both now inspect the owning modules and pass.

## Future Change Rule

Do not split a module only because it is long. Create another boundary when code
has its own lifecycle, persistence aggregate, transport, or independently
characterizable policy. Keep command names, payload shapes, and database
migrations stable across such moves.

## Rollback

Every extraction keeps the public command name or a compatibility export. The
changes can therefore be reverted boundary by boundary without migrating user
data.
