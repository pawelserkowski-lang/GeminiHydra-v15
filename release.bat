@echo off
echo === GeminiHydra v15 Release ===

:: Kill old backend on port 8081
echo [RESTART] Stopping old backend on port 8081...
for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":8081 " ^| findstr LISTENING') do taskkill /f /pid %%a >nul 2>&1
timeout /t 1 /nobreak >nul

:: Start new backend
echo [START] Backend (cargo run --release)...
start "GeminiHydra Backend" /min cmd /c "cd /d %~dp0backend && cargo run --release"

:: Kill old preview on port 4176
echo [RESTART] Stopping old preview on port 4176...
for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":4176 " ^| findstr LISTENING') do taskkill /f /pid %%a >nul 2>&1

:: Build frontend
echo [BUILD] Building frontend...
call npm run build

:: Open Chrome after delay
start /b cmd /c "timeout /t 3 /nobreak >nul && start chrome --new-window http://localhost:4173"

:: Start preview server
echo [PREVIEW] Starting preview...
npm run preview
