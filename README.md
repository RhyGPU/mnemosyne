# Mnemosyne

Mnemosyne is an AGPL-licensed local RP client prototype with a Rust state engine, persistent Soul files, and a React/Tauri desktop UI.

This repository currently implements the Engine MVP:

- Tauri 2 desktop shell
- React + TypeScript + Tailwind UI
- Rust Soul schema, memory scoring, consolidation, and context compiler
- Dedicated `src-tauri/state_engine` Rust library crate for Soul state
- SQLite persistence
- Mock provider turn flow with hidden-state parsing

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
```

## License

AGPL-3.0-or-later. See [LICENSE](LICENSE).
