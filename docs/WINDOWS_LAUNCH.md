# Windows: one-click launch and build

These scripts live in the **repository root** and are meant for double-click use in File Explorer. They prefer the real Node installer at `C:\Program Files\nodejs\npm.cmd` (see [README.md](../README.md)) and fall back to `npm` on your `PATH` if that file is missing.

## `run-dev.bat` — fast development

- Starts Mnemosyne in **Tauri dev mode** (`npm run dev`).
- Creates `node_modules` with **`npm install`** if the folder does not exist.
- If **`npm install`** or **`npm run dev`** fails, the window stays open (`pause`) so you can read the error.

Use this while you iterate on UI or Rust; it runs the dev server and Tauri alongside each other.

## `build-windows.bat` — production build

Runs a full checklist before **`npm run build`** (Tauri production bundle):

1. `npm install` — only if `node_modules` is missing  
2. `npm run typecheck`  
3. `npm run build:frontend`  
4. `npm run test:rust`  
5. `npm run build`  

The window **always pauses at the end** so you can read success or failure. On success it prints where the **`Mnemosyne.exe`** and **`bundle`** outputs are typically found under `src-tauri\target\release\`.

## `launch-built.bat` — open the release app

Looks for:

- `src-tauri\target\release\Mnemosyne.exe`  
- `src-tauri\target\release\mnemosyne.exe`  

If neither exists, it tells you to run **`build-windows.bat`** first and leaves the window open.

## Source control

Use **GitHub Desktop** (or your usual Git client) for **commit** and **push**. These batch files only run local installs and builds; they do not perform Git operations.
