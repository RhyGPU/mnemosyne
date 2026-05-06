@echo off
setlocal EnableExtensions
cd /d "%~dp0"

set "EXE_PRIMARY=src-tauri\target\release\Mnemosyne.exe"
set "EXE_FALLBACK=src-tauri\target\release\mnemosyne.exe"

if exist "%EXE_PRIMARY%" (
  echo [Mnemosyne] Launching "%CD%\%EXE_PRIMARY%"
  start "" "%CD%\%EXE_PRIMARY%"
  endlocal
  exit /b 0
)

if exist "%EXE_FALLBACK%" (
  echo [Mnemosyne] Launching "%CD%\%EXE_FALLBACK%"
  start "" "%CD%\%EXE_FALLBACK%"
  endlocal
  exit /b 0
)

echo [Mnemosyne] No release EXE found at:
echo   %CD%\%EXE_PRIMARY%
echo   %CD%\%EXE_FALLBACK%
echo.
echo Run build-windows.bat first to produce src-tauri\target\release\Mnemosyne.exe
echo ^(product name comes from src-tauri\tauri.conf.json^).
echo.
pause
endlocal
exit /b 1
