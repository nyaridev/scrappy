@echo off
rem Builds the portable app into release\Scrappy.exe, with scrappy-host.exe beside it for the extension. Installers
rem for Windows, macOS and Linux are built on GitHub by .github/workflows/release.yml (push a v* tag).
setlocal
cd /d "%~dp0"

call pnpm install --frozen-lockfile || goto :fail
call pnpm tauri build --no-bundle || goto :fail
call cargo build --release -p scrappy-host || goto :fail

if not exist release mkdir release
copy /y target\release\scrappy.exe release\Scrappy.exe >nul || goto :fail
copy /y target\release\scrappy-host.exe release\scrappy-host.exe >nul || goto :fail
echo.
echo Built release\Scrappy.exe and release\scrappy-host.exe
exit /b 0

:fail
echo Build failed.
pause
exit /b 1
