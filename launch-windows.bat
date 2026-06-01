@echo off
setlocal

cd /d "%~dp0"
set "ODYSSEUS_REPO_DIR=%CD%"
set "DESKTOP_EXE=%CD%\src-tauri\target\release\odysseus-desktop.exe"

echo.
echo Odysseus Desktop Launcher
echo Repo: %ODYSSEUS_REPO_DIR%
echo.

if exist "%DESKTOP_EXE%" goto launch

echo Desktop app is not built yet.
echo.
echo If you downloaded Odysseus-Windows-Portable.zip, this file should already exist:
echo %DESKTOP_EXE%
echo.
echo This looks like a source checkout instead, so the launcher will try to build
echo the Tauri wrapper locally. This requires Node.js, Rust, and the Tauri prerequisites.
echo.

where npm >nul 2>nul
if errorlevel 1 (
  echo ERROR: npm was not found. Install Node.js, then run this file again.
  pause
  exit /b 1
)

where cargo >nul 2>nul
if errorlevel 1 (
  echo ERROR: cargo was not found. Install Rust, then run this file again.
  pause
  exit /b 1
)

if not exist "%CD%\node_modules\@tauri-apps\cli" (
  echo Installing desktop build dependencies...
  call npm install
  if errorlevel 1 (
    echo.
    echo ERROR: npm install failed.
    pause
    exit /b 1
  )
)

echo Building Odysseus desktop...
call npm run desktop:build
if errorlevel 1 (
  echo.
  echo ERROR: Tauri desktop build failed.
  pause
  exit /b 1
)

if not exist "%DESKTOP_EXE%" (
  echo.
  echo ERROR: Build finished, but the desktop executable was not found:
  echo %DESKTOP_EXE%
  pause
  exit /b 1
)

:launch
tasklist /FI "IMAGENAME eq odysseus-desktop.exe" 2>NUL | find /I "odysseus-desktop.exe" >NUL
if not errorlevel 1 (
  echo Odysseus Desktop is already running.
  exit /b 0
)

echo Launching Odysseus Desktop...
echo Tauri will start the local webserver and show startup logs if setup is needed.
start "Odysseus Desktop" "%DESKTOP_EXE%"
exit /b 0
