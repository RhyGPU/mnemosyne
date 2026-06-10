# Mnemosyne

Mnemosyne is an AGPL-licensed local-first campaign brain for persistent AI roleplay and long-form story creation. It is not meant to be only a better chat frontend, JanitorAI clone, or SillyTavern alternative. The product goal is a universal RP state map: a living graph of characters, relationships, objects, locations, events, factions, secrets, unresolved tensions, and continuity facts that can support many styles of roleplay through one underlying architecture.

Mnemosyne combines a React/Tauri UI with a Rust state engine that manages character Souls, world continuity, memory scoring, context compilation, evaluator patches, and state updates outside the LLM.

The core design principle is simple:

> The narrator writes. The state map remembers.

That state map should eventually support character RP, relationship RP, D&D-style party adventures, multi-user campaigns, political intrigue, survival/base-building stories, and alternate-history war games without rebuilding the engine around each genre.

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
- Portable `.mne` bundle validation and session checkpoint import/export
- Image attachment import and local image asset storage
- Browser preview fallback for development when Tauri runtime is unavailable

Supported provider path:

- Mnemosyne uses an OpenAI-compatible `/chat/completions` API provider.
- Hosted APIs can be used by setting the provider base URL, API key, and model.
- Local model servers can also be used if they expose an OpenAI-compatible endpoint, such as LM Studio, Ollama OpenAI-compatible mode, vLLM, llama.cpp server, or similar tools.

Not implemented yet:

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

The near-term roadmap is State Map V1, not a full universal simulator. The milestone is practical inspection and correction: a user should be able to run a 30-turn RP session, inspect what the engine believes, correct wrong memory or state, continue the RP, and export/import the session safely.

State Map V1 should expose:

- Scene State panel
- Characters panel
- Relationships panel
- Objects panel
- Timeline panel
- Open Tensions panel
- Debug/Memory Inspector panel

The user should be able to click a character, object, location, relationship, or event and understand what the engine believes is true, where that belief came from, what changed this turn, what memories were retrieved, what relationships were updated, and what unresolved tensions remain.

Immediate engineering priorities:

- Add role-specific model compatibility gates. Narrator models can be creative, evaluator models must pass strict JSON/schema contracts, and command models must be concise and instruction-following. A universal model should not silently take over all roles unless it passes each role contract.
- Slim the evaluator prompt from tutorial prose into a compact schema card. Keep the hard fillable form, remove contradictory or deprecated wording, move relationship calibration references away from output schema instructions, and prefer machine-readable constraints where possible.
- Add a compact authoritative `scene_continuity_summary` for current truth, not long-term memory. It should track location, participants, positions, door/room state, current misunderstanding, active object, open question, and last concrete action.
- Clean up memory retrieval without removing prompt memory limits. Separate stored memory capacity from prompt retrieval capacity, allow more stored memories, retrieve only the best few, prevent duplicate memories across prompt sections, and show primary slot labels while hiding secondary tags until inspection.
- Deduplicate relationship and memory context. Relationship summaries should collapse by `source_soul_id + target_entity_id` and prefer latest materialized relationship state. Memories should not repeat across current plot, character identity, world/location, unresolved tension, and recent emotional state sections.
- Build State Map V1 UI around current scene truth, known character facts, relationship changes with evidence, object owner/location/status, timeline events, unresolved tensions, and retrieved-memory evidence.

Longer-term priorities:

- Generalize the state graph across entities, edges, events, secrets, knowledge scopes, faction state, resources, morale, danger, timers, and delayed consequences.
- Support richer multi-character and multi-user campaign workflows on top of the same state map.
- Add native local model runtime support, separate from OpenAI-compatible external local servers.
- Continue improving schema versioning, migration tooling, import/export validation, and tester documentation.

See [State Map Roadmap](docs/state-map-roadmap.md) for the fuller product direction.

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
