@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"
set "LIB=C:\Users\BIURODOM\Desktop\ClaudeDesktop\jaskier-lib.bat"

:: Init colors
call "%LIB%" :init_colors
echo !BOLD!!MAGENTA!=== GeminiHydra v15 DEV ===!RESET!

:: Log init
call "%LIB%" :log_init "geminihydra" "dev"

:: Validate .env
call "%LIB%" :env_check "%~dp0.env" "GOOGLE_API_KEY ANTHROPIC_API_KEY"
call "%LIB%" :env_check "%~dp0backend\.env" "DATABASE_URL GOOGLE_API_KEY ANTHROPIC_API_KEY"

:: Docker DB check
call "%LIB%" :docker_db_check "geminihydra-pg" "%~dp0backend"

:: Kill old processes (graceful)
call "%LIB%" :kill_port 8081 "backend"
call "%LIB%" :kill_port 5176 "frontend dev"

:: Partner check
call "%LIB%" :partner_check 8082 "ClaudeHydra"

:: Start backend
echo !CYAN![START]!RESET! Backend ^(cargo run^)...
start "GeminiHydra Backend" /min cmd /c "cd /d %~dp0backend && cargo run"

:: Health check
call "%LIB%" :health_check 8081 30

:: Port validation
call "%LIB%" :port_validate 8081 5

:: Open Chrome in app mode
start "" chrome --app=http://localhost:5176

:: Toast notification
call "%LIB%" :toast "GeminiHydra v15" "DEV server starting on port 5176"

:: Start frontend dev server
echo !CYAN![DEV]!RESET! Starting frontend dev server on port 5176...
endlocal && cd /d "%~dp0" && npm run dev
