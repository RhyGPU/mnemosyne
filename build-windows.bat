@echo off
setlocal EnableExtensions EnableDelayedExpansion
cd /d "%~dp0"
set "MNEM_EXIT=0"

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

if not exist "node_modules\" (
  echo [Mnemosyne] node_modules not found - running npm install...
  call "%NPM_CMD%" install
  if errorlevel 1 (
    echo.
    echo [Mnemosyne] ERROR: npm install failed.
    set "MNEM_EXIT=1"
    goto WRAPUP
  )
  echo.
)

echo [Mnemosyne] Step 1/4: npm run typecheck...
call "%NPM_CMD%" run typecheck
if errorlevel 1 (
  echo [Mnemosyne] ERROR: typecheck failed.
  set "MNEM_EXIT=1"
  goto WRAPUP
)

echo.
echo [Mnemosyne] Step 2/4: npm run build:frontend...
call "%NPM_CMD%" run build:frontend
if errorlevel 1 (
  echo [Mnemosyne] ERROR: frontend build failed.
  set "MNEM_EXIT=1"
  goto WRAPUP
)

echo.
echo [Mnemosyne] Step 3/4: npm run test:rust...
call "%NPM_CMD%" run test:rust
if errorlevel 1 (
  echo [Mnemosyne] ERROR: Rust tests failed.
  set "MNEM_EXIT=1"
  goto WRAPUP
)

echo.
echo [Mnemosyne] Step 4/4: npm run build ^(Tauri production build^)...
call "%NPM_CMD%" run build
if errorlevel 1 (
  echo [Mnemosyne] ERROR: Tauri build failed.
  set "MNEM_EXIT=1"
  goto WRAPUP
)

echo.
echo ============================================================
echo [Mnemosyne] Build completed successfully.
echo.
echo Likely Windows outputs ^(from tauri.conf.json productName: Mnemosyne^):
echo   EXE:     %CD%\src-tauri\target\release\Mnemosyne.exe
echo            %CD%\src-tauri\target\release\mnemosyne.exe  ^(lowercase fallback^)
echo   Bundles: %CD%\src-tauri\target\release\bundle\
echo            ^(NSIS / MSI installers and related artifacts, depending on config^)
echo ============================================================

:WRAPUP
echo.
if "!MNEM_EXIT!"=="1" echo [Mnemosyne] Finished with errors. Review the log above.
pause
if "!MNEM_EXIT!"=="1" (
  endlocal
  exit /b 1
)
endlocal
exit /b 0
