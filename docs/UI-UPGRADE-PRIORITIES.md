# Mnemosyne UI Upgrade Priorities

Updated: 2026-07-24

## Objective

Improve Mnemosyne as a focused AI roleplay application with three deliberate UI registers:

- Paper/editorial for Home, Library, State Map, and Settings.
- Book-like, low-distraction reading and writing for Play.
- Terminal-only diagnostics and process control for Dev Mode.

SillyTavern is a research reference for feature coverage, configuration structure, and mature roleplay workflows. We learn from its solved problems and user-facing organization. We do not copy its source, markup, styling, wording, or component structure.

## Priority 1: Complete Settings And Dev Separation

Status: completed

Remove developer-only controls from normal Settings while replacing the resulting gaps with useful user-facing settings.

Normal Settings should provide:

- AI connection profiles and model selection.
- Narration style and context behavior.
- Generation controls with clear defaults and reset behavior.
- Roleplay response controls such as response length, temperature, sampling, repetition handling, and stop behavior when supported by the selected provider.
- State updater and repair provider assignment under a clearly labeled advanced AI section.
- Session and data preferences.
- Appearance, reading, composer, and accessibility preferences that Mnemosyne actually supports.
- About, disclaimer, version, and storage information.

Dev Mode should exclusively contain:

- Developer override controls.
- Evaluator contract tests and compatibility diagnostics.
- Benchmarks and dry runs.
- Raw payload, prompt, response, and context inspection.
- Repair diagnostics and destructive state tools.
- Pipeline logs and background-job telemetry.

The compact chat Settings drawer should expose only the highest-frequency session controls. The full Settings page owns complete configuration.

Completed in this pass:

- Added dedicated AI, Generation, Reading, Data, and About categories.
- Added persisted generation presets, creativity, response length, and advanced supported sampling controls.
- Connected generation settings to real narrator requests.
- Added persisted Play prose size, line height, reading width, and message-spacing controls.
- Replaced the chat drawer's full-page category UI with compact session controls.
- Moved evaluator contract, transport, fallback, background, stale-state, and override controls into Dev Mode.
- Verified normal Settings contains no benchmark, payload, contract-test, developer-override, or structured-transport UI.

## Priority 2: Complete Dev Telemetry

Status: completed

Generalize the current benchmark meter into a real background-job feedback system:

- Concurrent job rows with running, succeeded, failed, cancelled, and queued states.
- Per-job phase, current cycle, total cycles, elapsed time, and estimated remaining time.
- Live success/failure/recovery counts.
- Expandable per-cycle or per-turn history.
- Per-job cancellation where the engine supports it.
- Persistent completion summaries until dismissed.
- A shared event model so benchmark, evaluator, repair, state rebuild, import, and export progress use the same UI.

Completed in this pass:

- Added a shared `background-job-progress` event contract for Rust and frontend-owned work.
- Replaced the dedicated benchmark meter with one concurrent Dev job monitor.
- Added phase, cycle/total, elapsed time, ETA, success/failure/recovery counts, expandable history, cancellation where supported, and dismissal.
- Wired headless and visible benchmarks, evaluator jobs, form-eval, session repair, structured diagnostics, Dev commands, ledger rebuild, validation, import, and export workflows.
- Added per-turn structured-diagnostic and form-eval feedback instead of start/finish-only reporting.
- Verified a ledger rebuild appears, completes, persists, and dismisses through the live Dev UI.

## Priority 3: Finish View Extraction

Status: in progress

`App.tsx` still owns most rendered view bodies and workflow state.

- Extract the full Chat workspace, not only its shell.
- Extract Library launcher and Library editor bodies.
- Extract Settings form sections and provider profile management.
- Extract Dev diagnostics, benchmark controls, and pipeline presentation.
- Keep shared engine state in `App.tsx` until view boundaries are stable.

Completed in the current extraction pass:

- Moved generation and reading preference types, presets, and defaults into `src/settings/preferences.ts`.
- Extracted Generation, Reading, Data, About, and compact chat Settings panels into typed components.
- Made `SettingsPageView` own the full-page category navigation and panel composition.
- Extracted narrator, state-updater, repair-model, and saved-profile management panels with typed callbacks.
- Replaced provider-list inline presentation rules with reusable paper-theme classes.
- Extracted the Dev background-job monitor, turn pipeline rail, and live stream presentation.
- Extracted the Play header, transcript, response variants, updater-status banner, and composer into typed chat components.
- Moved hidden-state and trailing-status prose cleanup into the transcript component that owns display behavior.
- Made `ChatView` own the complete Play workspace shell instead of acting as a pass-through wrapper.
- Reduced `App.tsx` from 9,419 to 8,298 lines while keeping workflow state and engine actions in place.
- Verified every Settings category, the compact chat Settings drawer, AI provider forms, Play header, slash-command keyboard flow, chat menu dismissal, Dev entry, and a completed ledger-rebuild job in the live app.

Next extraction boundaries:

- Library launcher and Library editor bodies.
- Remaining Dev diagnostics and benchmark-control presentation.

## Priority 4: Accessibility And Responsive Navigation

Status: partially implemented

- Give collapsed and mobile rail buttons persistent accessible names.
- Remove duplicate accessible brand text.
- Verify focus return, Escape, backdrop, and focus trapping for every modal.
- Verify keyboard navigation and outside-click behavior for every menu.
- Test populated Chat and Settings at desktop, tablet, and mobile widths.

Verified in this pass:

- Collapsed rail destinations retain `Home`, `Play`, `State Map`, `Library`, and `Settings` accessible names and native tooltips.
- The visual brand exposes one semantic `Mnemosyne` label.
- Chat action menu closes on outside click and Escape.
- Persona dialog closes on Escape.
- Dev panel tabs remain visible while the panel scrolls independently.

## Priority 5: Theme, Type, And Control Consolidation

Status: in progress

- Replace remaining live hardcoded colors with semantic CSS variables.
- Consolidate font sizes into the documented human and terminal scales.
- Remove inline legacy colors and layout styling from live components.
- Keep human-facing metadata at readable size and contrast.
- Standardize primary, secondary, ghost, destructive, icon, toggle, slider, select, and segmented controls.

## Priority 6: Remove Implementation Narration

Status: pending

Replace development-facing copy such as future-feature explanations and placeholder implementation notes with concise, useful empty states. Do not expose roadmap language in the product UI.

## Priority 7: Populated Tauri Smoke Test

Status: in progress

Verify with real local session data:

- Play opens the most recently accessed active chat.
- Chat header, Settings drawer, composer, variants, and menus remain stable.
- Dev Mode has one non-overlapping control area while scrolling.
- State Map aggregates the five most recently updated active sessions into shared scene, character, relationship, object, timeline, and memory panels.
- Taskbar, window, installer, and executable icons use the current multi-resolution icon set.

Preview smoke verified:

- Play reopened the most recently accessed active chat.
- Chat header, quick Settings drawer, composer, menu dismissal, and Dev entry remained stable.
- Settings controls persisted through reload and reset to defaults.
- Dev Mode kept one non-overlapping control area while its panel scrolled.
- State Map rendered the recent-session aggregate after creating a session.

Still requires packaged Tauri verification:

- Repeat with the user's populated production database and connected providers.
- Verify taskbar, window, installer, executable, and high-DPI icon selection in the packaged build.

## Research Notes To Capture

For each mature roleplay app reviewed, record:

- The user problem being solved.
- The control or workflow pattern used.
- What is essential versus expert-only.
- Where the pattern belongs in Mnemosyne's information architecture.
- What should be intentionally omitted because it conflicts with Mnemosyne's focused editorial direction.

Reference findings become requirements or design principles, never copied implementation.
