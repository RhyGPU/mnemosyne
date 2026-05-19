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
