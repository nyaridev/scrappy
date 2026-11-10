@echo off
rem Runs Scrappy in its own window with hot reload (needs Node 22+, pnpm and Rust).
cd /d "%~dp0"

netstat -ano | findstr /r /c:":4430 .*LISTENING" >nul
if not errorlevel 1 (
    echo Port 4430 is already in use, probably by another Scrappy dev server or "pnpm dev".
    echo Close it and run this again.
    pause
    exit /b 1
)

if not exist node_modules (
    call pnpm install
    if errorlevel 1 goto :fail
)
call pnpm tauri dev
if errorlevel 1 goto :fail
exit /b 0

:fail
echo.
echo Scrappy dev failed. See the messages above.
pause
exit /b 1
