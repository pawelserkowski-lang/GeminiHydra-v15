@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"
set "LIB=%~dp0..\jaskier-lib.bat"

:: Init colors
call "%LIB%" :init_colors
echo !BOLD!!MAGENTA!=== GeminiHydra v15 Release ===!RESET!

:: Log init
call "%LIB%" :log_init "geminihydra" "release"

:: Validate .env
call "%LIB%" :env_check "%~dp0.env" "GOOGLE_API_KEY ANTHROPIC_API_KEY"
call "%LIB%" :env_check "%~dp0backend\.env" "DATABASE_URL GOOGLE_API_KEY ANTHROPIC_API_KEY"

:: Docker DB check
call "%LIB%" :docker_db_check "geminihydra-pg" "%~dp0backend"

:: Cargo build pre-check
call "%LIB%" :cargo_check "%~dp0backend" "geminihydra-backend.exe"

:: Kill old processes (graceful)
call "%LIB%" :kill_port 8081 "backend"
call "%LIB%" :wait_port_free 8081
call "%LIB%" :kill_port 4176 "preview"

:: Partner check
call "%LIB%" :partner_check 8082 "ClaudeHydra"

:: Start backend
echo !CYAN![START]!RESET! Backend ^(release binary^)...
start "GeminiHydra Backend" /min cmd /c "cd /d %~dp0backend && target\release\geminihydra-backend.exe"
%SYSTEMROOT%\System32\timeout.exe /t 2 /nobreak >nul

:: Health check
call "%LIB%" :health_check 8081 15

:: Build frontend
echo !CYAN![BUILD]!RESET! Building frontend...
call npm run build

:: Open Chrome in app mode
start "" chrome --app=http://localhost:4176

:: Toast notification
call "%LIB%" :toast "GeminiHydra v15" "Release preview on port 4176"

:: Start preview
echo !CYAN![PREVIEW]!RESET! Starting preview on port 4176...
endlocal && cd /d "%~dp0" && npm run preview
