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
