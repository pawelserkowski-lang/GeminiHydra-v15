@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"
set "LIB=C:\Users\BIURODOM\Desktop\ClaudeDesktop\jaskier-lib.bat"

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

:: Kill old processes FIRST (before building — exe is locked while backend runs)
call "%LIB%" :kill_port 8081 "backend"
call "%LIB%" :wait_port_free 8081
if errorlevel 1 goto :abort
call "%LIB%" :kill_port 4176 "preview"
call "%LIB%" :wait_port_free 4176
if errorlevel 1 goto :abort

:: Cargo build (exe now unlocked)
call "%LIB%" :cargo_check "%~dp0backend" "geminihydra-backend.exe"
if errorlevel 1 goto :abort

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
if errorlevel 1 goto :abort

:: Start preview (BEFORE Chrome — so port is ready)
echo !CYAN![PREVIEW]!RESET! Starting preview on port 4176...
start "GeminiHydra Preview" /min cmd /c "cd /d %~dp0 && npm run preview"
%SYSTEMROOT%\System32\timeout.exe /t 2 /nobreak >nul
call "%LIB%" :port_validate 4176 10

:: Open Chrome in app mode (preview is already listening)
start "" chrome --app=http://localhost:4176

:: Toast notification
call "%LIB%" :toast "GeminiHydra v15" "Release preview on port 4176"

echo !GREEN![DONE]!RESET! GeminiHydra v15 release ready on http://localhost:4176
echo Press Ctrl+C to stop preview.
endlocal
goto :eof

:abort
echo !RED![ABORT]!RESET! Release failed — fix issues above and retry.
endlocal
exit /b 1
