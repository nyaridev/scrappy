@echo off
rem Builds Scrappy for Windows into release\: the installer, the portable app (Scrappy.exe with scrappy-host.exe
rem beside it, also as a zip), and the browser extension's zips for Chrome, Brave, Edge and Firefox.
rem macOS and Linux are built on GitHub: push a v* tag (.github/workflows/release.yml).
setlocal
cd /d "%~dp0"

call pnpm install --frozen-lockfile || goto :fail
for /f %%v in ('node -p "require('./package.json').version"') do set VERSION=%%v
for /f "tokens=2" %%t in ('rustc -vV ^| findstr /b host:') do set TARGET=%%t

call pnpm tauri build --config src-tauri\tauri.release.conf.json --bundles nsis || goto :fail
call pnpm -C extension zip || goto :fail
call pnpm -C extension zip:firefox || goto :fail

if not exist release mkdir release
copy /y target\release\bundle\nsis\Scrappy_%VERSION%_*-setup.exe release\ >nul || goto :fail
copy /y target\release\scrappy.exe release\Scrappy.exe >nul || goto :fail
copy /y src-tauri\binaries\scrappy-host-%TARGET%.exe release\scrappy-host.exe >nul || goto :fail
powershell -NoProfile -Command "Compress-Archive -Force -Path release\Scrappy.exe,release\scrappy-host.exe -DestinationPath release\Scrappy_%VERSION%_windows_portable.zip" || goto :fail
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
