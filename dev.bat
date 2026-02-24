@echo off
echo === GeminiHydra v15 DEV ===

:: Kill old backend on port 8081
echo [RESTART] Stopping old backend on port 8081...
for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":8081 " ^| findstr LISTENING') do taskkill /f /pid %%a >nul 2>&1
timeout /t 1 /nobreak >nul

:: Start new backend
echo [START] Backend (cargo run)...
start "GeminiHydra Backend" /min cmd /c "cd /d %~dp0backend && cargo run"

:: Open Chrome after delay
start /b cmd /c "timeout /t 5 /nobreak >nul && start chrome --new-window http://localhost:5176"

:: Start frontend dev server
echo [DEV] Starting frontend dev server...
npm run dev
