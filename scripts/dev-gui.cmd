@echo off
setlocal enabledelayedexpansion

title HyperForm AI — Dev GUI
echo ========================================================
echo   HyperForm AI — Launching Desktop Development GUI
echo   Think Beyond. Build Beyond.
echo ========================================================
echo.

rem Move to repo root
cd /d "%~dp0\.."

rem Ensure cargo and node are in PATH
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

set "UI_DIR=crates\hyperform-gui\ui"
if not exist "%UI_DIR%\node_modules" (
    echo [1/2] Installing UI dependencies...
    pushd "%UI_DIR%"
    call npm.cmd install
    popd
)

echo [2/2] Launching Tauri Dev Server with Vite HMR...
cd /d "crates\hyperform-gui"

call ".\ui\node_modules\.bin\tauri.cmd" dev %*
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [INFO] Tauri desktop shell exited with code %ERRORLEVEL%.
    echo If native C++ build tools are missing, you can run in browser dev mode:
    echo   cd %UI_DIR% ^&^& npm run dev
    echo.
    pause
)
