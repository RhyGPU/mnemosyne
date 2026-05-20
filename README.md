# Mnemosyne

Mnemosyne is an AGPL-licensed local desktop client for persistent AI roleplay and long-form story creation. It combines a React/Tauri UI with a Rust state engine that manages character Souls, world continuity, memory scoring, context compilation, and state updates outside the LLM.

The core design principle is simple:

> The narrator writes. The Soul remembers.

Mnemosyne is currently a working alpha. The main narrator, memory, state, and persistence pipeline exists, but the project is not yet a polished public release.

## Current Status

Implemented:

- Tauri 2 desktop shell
- React + TypeScript + Tailwind UI
- Rust Soul schema and dedicated `src-tauri/state_engine` library crate
- SQLite persistence for Souls, settings, conversations, messages, provider profiles, turn snapshots, image assets, and LLM payload logs
- Soul savepoints, session clones, checkpoints, and fresh scenario sessions
- Setting/world management with session-specific world state
- Mock provider turn flow
- OpenAI-compatible API provider turn flow
- Streaming narrator output
- Separate narrator and state-updater model settings
- Hidden-state parsing and EnginePatch application
- Relationship deltas with clamping
- Memory scoring, duplicate rejection, generic-memory rejection, salience handling, and consolidation
- Context compiler with brief and full-chat modes
- Anti-replay guard for repeated narrator responses
- Output contract guard for narrator responses
- Assistant response variants, regenerate/fix flows, and turn snapshots
- Debug logs, performance logs, context preview, LLM payload preview, and payload history export
- Visible chat log export
- Soul and setting JSON import/export
- Image attachment import and local image asset storage
- Browser preview fallback for development when Tauri runtime is unavailable

Supported provider path:

- Mnemosyne uses an OpenAI-compatible `/chat/completions` API provider.
- Hosted APIs can be used by setting the provider base URL, API key, and model.
- Local model servers can also be used if they expose an OpenAI-compatible endpoint, such as LM Studio, Ollama OpenAI-compatible mode, vLLM, llama.cpp server, or similar tools.

Not implemented yet:

- Portable `.mne` bundle import/export
- Native embedded local model runtime inside Mnemosyne
- Browser/subscription connector for external web chat services
- Full production packaging, installer polish, and broad end-user documentation

## Performance Model

Mnemosyne uses a dual-pass turn architecture:

1. A narrator model writes the visible RP/story response.
2. A state-updater model extracts compact memory, relationship, body, and world-state changes.

This means a turn can use two model calls. The tradeoff is intentional: instead of sending the full chat history every turn, Mnemosyne sends a compact state snapshot, recent exchange, selected memories, relationships, and world context. In long RP/story sessions, this should greatly reduce prompt growth compared with full-history prompting.

The state-updater model does not need to be as powerful or literary as the narrator model. Cheap or free OpenAI-compatible models may be sufficient for updater work if they reliably return valid JSON patches.

## Known Alpha Limitations

The current hard problems are not only token cost. The harder problems are memory quality, entity separation, plot hygiene, latency, and long-session stability.

Known issues and risks:

- Memory residue and cross-session bleed can occur if memories from another session are imported, summarized, or retrieved incorrectly.
- Memory retrieval is intentionally small, which saves tokens but can miss older important facts if retrieval quality is poor.
- Low-value memories, such as routine reactions or repeated emotional descriptions, can pollute the memory layer if not aggressively filtered.
- Multi-actor scenes need stronger entity separation. The engine must distinguish the operator, player characters, NPCs, imported-log speakers, and other participants instead of collapsing them into one generic `user` relationship.
- Relationship state must be per-entity. A character should not transfer distrust, affection, fear, or conflict from one person to another unless the story actually justifies it.
- Active plots can become sticky if old conflicts are not resolved, decayed, or moved into background state.
- Anti-replay protection is necessary because compressed summaries and recent exchange text can cause the narrator to repeat old beats.
- Pasted logs can still create a large one-time token spike if they are sent directly to the narrator instead of being ingested through a summarizer/import pipeline.
- Response time depends heavily on the narrator model, provider routing, output length, and whether the updater blocks the UI before the next turn.
- Free updater models may be useful, but they can be rate-limited, unstable, or less reliable at strict JSON extraction.

## Recommended Roadmap

Short-term priorities:

- Add stronger entity and speaker registry support for multi-actor scenes.
- Store relationship, trust, conflict, affection, fear, comfort, dependency, and curiosity per entity rather than only against a generic player target.
- Add fixed memory retrieval slots, such as relationship memories, current-plot memories, identity memories, unresolved-tension memories, and world/location memories.
- Improve memory hygiene by rejecting or deprioritizing generic memories, repeated body-language notes, routine emotional reactions, and vague observations.
- Add an import-log mode that routes pasted logs to a summarizer/state-updater pipeline instead of sending large logs directly to the narrator.
- Add stronger anti-replay checks that compare a new narrator response against recent assistant responses and reject or regenerate near-duplicates.
- Add plot lifecycle management: dominant current plot, background plots, resolved plots, and stale plot decay.
- Make the state updater non-blocking where possible: stream the narrator response first, then run the updater in the background and apply the patch before the next turn.
- Add dev timing metrics: context compile time, time to first narrator token, narrator completion time, updater completion time, DB save time, and total turn time.
- Add user-facing speed presets: Fast, Balanced, and Literary, with different output-length targets.

Longer-term priorities:

- Portable `.mne` packages for sharing a complete Soul, memories, world links, metadata, and assets as one file.
- Native local model runtime support, separate from OpenAI-compatible external local servers.
- Better schema versioning, migration tooling, and import/export validation.
- Optional paid fallback model for state updating when a free updater model fails JSON validation or rate limits.
- More complete documentation for testers, including safe testing guidance, recommended models, and known failure modes.

## Alpha Warning

Mnemosyne is still alpha software. Memory isolation, persistence behavior, context residue, provider behavior, and long-session stability are still being tested.

Do not enter private, sensitive, or personally identifying information while testing. Treat all test data as experimental.

## Local Setup

On this Windows machine, `npm` may resolve to a bad shim at `C:\WINDOWS\system32\npm`. Use the real command explicitly:

```powershell
& "C:\Program Files\nodejs\npm.cmd" install
& "C:\Program Files\nodejs\npm.cmd" run dev
```

Tauri also requires Rust stable MSVC, WebView2, and Microsoft C++ build tools.

If `cargo test` fails with Windows Application Control error `4551`, the machine is blocking generated Rust build scripts. Allow Rust build outputs or move the repo/build target to a trusted development path before running full backend/Tauri verification.

## Scripts

```powershell
& "C:\Program Files\nodejs\npm.cmd" run dev
& "C:\Program Files\nodejs\npm.cmd" run build
& "C:\Program Files\nodejs\npm.cmd" run test:rust
& "C:\Program Files\nodejs\npm.cmd" run typecheck
```

## License

AGPL-3.0-or-later. See [LICENSE](LICENSE).
