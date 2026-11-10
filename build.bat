@echo off
rem Builds the portable app into release\Scrappy.exe. Installers for Windows, macOS and Linux come in M14.
setlocal
cd /d "%~dp0"

call pnpm install --frozen-lockfile || goto :fail
call pnpm tauri build --no-bundle || goto :fail

if not exist release mkdir release
copy /y target\release\scrappy.exe release\Scrappy.exe >nul || goto :fail
echo.
echo Built release\Scrappy.exe
exit /b 0

:fail
echo Build failed.
pause
exit /b 1
