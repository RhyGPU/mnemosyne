# Mnemosyne

Mnemosyne is an AGPL-licensed local-first campaign brain / AI roleplay continuity engine for persistent characters and long-form story creation. It separates character memory, world state, prompt context, and debugging traces so an LLM narrator can write without being asked to remember everything by itself.

It is not meant to be only a better chat frontend, JanitorAI clone, or SillyTavern alternative. The product goal is a universal RP state map: a living graph of characters, relationships, objects, locations, events, factions, secrets, unresolved tensions, and continuity facts that can support many styles of roleplay through one underlying architecture.

The core design principle is simple:

> The narrator writes. The state map remembers.

Mnemosyne combines a React/Tauri UI with a Rust state engine that manages character Souls, world continuity, memory scoring, context compilation, evaluator patches, persistence, and debug traces outside the LLM.

That state map should eventually support character RP, relationship RP, D&D-style party adventures, multi-user campaigns, political intrigue, survival/base-building stories, and alternate-history war games without rebuilding the engine around each genre.

Mnemosyne is currently a working alpha. The main narrator, memory, state, and persistence pipeline exists, but the project is not yet a polished public release.

## Links

- Portfolio case study: [https://rhygpu.dev/projects/mnemosyne/](https://rhygpu.dev/projects/mnemosyne/)
- Development journal: [https://rhygpu.dev/devlog/](https://rhygpu.dev/devlog/)
- Portfolio site: [https://rhygpu.dev/](https://rhygpu.dev/)

## For Readers

Use the portfolio case study for the high-level technical overview, screenshots, and project framing.

Use the development journal for design history: what broke, what changed, and why the architecture moved toward memory/state separation, payload inspection, fresh-session boundaries, and prompt cleanup.

Use this README for developer-facing repo context: setup, current implementation state, architecture notes, alpha limitations, roadmap, and local development commands.

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

The roadmap rests on one architectural finding: the evaluator's fragility came from enforcing its schema contract through prompt wording instead of through the provider API. Structured outputs and tool calling (Anthropic tool use, OpenAI `json_schema` response format, grammar-constrained local sampling) force the model's output to match a schema at the decoding level, which makes most of the hand-written parse/repair layer deletable and makes evaluator models swappable.

The near-term product milestone is State Map V1, not a full universal simulator: a user should be able to run a 50+ turn multi-character session, inspect what the engine believes, trace any memory to the turn that created it, correct wrong state, continue playing, and export/import the session safely.

State Map V1 should expose:

- Scene State panel
- Characters panel
- Relationships panel
- Objects panel
- Timeline panel
- Open Tensions panel
- Debug/Memory Inspector panel

The user should be able to click a character, object, location, relationship, or event and understand what the engine believes is true, where that belief came from, what changed this turn, what memories were retrieved, what relationships were updated, and what unresolved tensions remain.

Immediate engineering priorities (in order — see the roadmap doc for full detail):

- Move the evaluator to schema-enforced output (tool calling / structured outputs) and have it propose patch operations against a compact current-state JSON instead of filling a large text form. Keep semantic validation; delete syntactic repair once unreachable.
- Make the evaluator cheap and conditional: run it on a small fast model, gate it on turn significance using the narrator's existing status block, and expose Fast / Balanced / Long Context modes. Add output-token and cost estimates to the pipeline trace.
- Lift the destructive stored-memory cap (archive evicted memories instead of deleting), keep the prompt memory budget capped, and deduplicate memories across prompt sections.
- Role-specific model compatibility gates and evaluator auto-fallback are implemented; keep them as the safety net and verify them live.
- Add a compact authoritative `scene_continuity_summary` for current truth, not long-term memory. It should track location, participants, positions, door/room state, current misunderstanding, active object, open question, and last concrete action.
- Deduplicate relationship context: collapse relationship summaries by `source_soul_id + target_entity_id` and prefer latest materialized relationship state.
- Build State Map V1 UI around current scene truth, known character facts, relationship changes with evidence, object owner/location/status, timeline events, unresolved tensions, and a Memory Inspector with full provenance (every memory traceable to the turn that created it, with pin/edit/invalidate).

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
