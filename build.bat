@echo off
rem Builds Scrappy for Windows into release\: the installer and the browser extension's zips for Chrome, Brave, Edge
rem and Firefox. Only the release workflow signs updates (ADR-0037).
rem macOS and Linux are built on GitHub: push a v* tag (.github/workflows/release.yml).
setlocal
cd /d "%~dp0"

call pnpm install --frozen-lockfile || goto :fail
for /f %%v in ('node -p "require('./package.json').version"') do set VERSION=%%v

call pnpm tauri build --config src-tauri\tauri.release.conf.json --bundles nsis || goto :fail
call pnpm -C extension zip || goto :fail
call pnpm -C extension zip:firefox || goto :fail

if not exist release mkdir release
copy /y target\release\bundle\nsis\Scrappy_%VERSION%_*-setup.exe release\ >nul || goto :fail
copy /y extension\.output\scrappy-extension-%VERSION%-chrome.zip release\ >nul || goto :fail
copy /y extension\.output\scrappy-extension-%VERSION%-firefox.zip release\ >nul || goto :fail

echo.
echo Built in release\:
dir /b release
exit /b 0

:fail
echo Build failed.
pause
exit /b 1
