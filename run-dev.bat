@echo off
setlocal EnableExtensions
cd /d "%~dp0"

set "PROGRAM_NPM=C:\Program Files\nodejs\npm.cmd"
if exist "%PROGRAM_NPM%" (
  set "NPM_CMD=%PROGRAM_NPM%"
  echo [Mnemosyne] Using Node npm: "%PROGRAM_NPM%"
) else (
  set "NPM_CMD=npm"
  echo [Mnemosyne] Program Files Node not found; using npm from PATH.
)
echo [Mnemosyne] Repo root: %CD%
echo.

rem Tauri/Cargo build metadata contains absolute paths to generated plugin
rem permissions. If this checkout is moved or copied, Cargo may otherwise reuse
rem a "fresh" artifact that still points at the previous checkout.
set "CACHE_ROOT_FILE=src-tauri\target\.mnemosyne-repo-root.txt"
set "CACHED_REPO_ROOT="
if exist "%CACHE_ROOT_FILE%" set /p CACHED_REPO_ROOT=<"%CACHE_ROOT_FILE%"

if exist "src-tauri\target\debug\" if /I not "%CACHED_REPO_ROOT%"=="%CD%" (
  echo [Mnemosyne] Rust dev cache belongs to another repo location.
  echo [Mnemosyne] Cleaning stale Tauri dev artifacts...
  cargo clean --manifest-path "src-tauri\Cargo.toml" --profile dev
  if errorlevel 1 (
    echo.
    echo [Mnemosyne] ERROR: Could not clean stale Rust dev artifacts.
    pause
    exit /b 1
  )
  echo [Mnemosyne] Stale Tauri dev artifacts cleared.
  echo.
)

if not exist "src-tauri\target\" mkdir "src-tauri\target"
> "%CACHE_ROOT_FILE%" echo(%CD%

if not exist "node_modules\" (
  echo [Mnemosyne] node_modules not found - running npm install...
  call "%NPM_CMD%" install
  if errorlevel 1 (
    echo.
    echo [Mnemosyne] ERROR: npm install failed. See messages above.
    pause
    exit /b 1
  )
  echo [Mnemosyne] npm install finished.
  echo.
)

echo [Mnemosyne] Starting Tauri dev ^(npm run dev^)...
echo.
call "%NPM_CMD%" run dev
if errorlevel 1 (
  echo.
  echo [Mnemosyne] ERROR: dev exited with an error. See messages above.
  pause
  exit /b 1
)

endlocal
exit /b 0
