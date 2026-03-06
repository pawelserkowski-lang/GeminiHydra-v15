@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"
set "LIB=C:\Users\BIURODOM\Desktop\ClaudeDesktop\jaskier-lib.bat"

:: Init colors
call "%LIB%" :init_colors
:: Kill previous instances
taskkill /F /FI "WINDOWTITLE eq [Jaskier] GeminiHydra*" >nul 2>&1
powershell -NoProfile -Command "Get-Process | Where-Object { $_.Name -eq 'powershell' -and $_.CommandLine -like '*tray-minimizer.ps1*' -and $_.CommandLine -like '*GeminiHydra Release*' } | Stop-Process -Force -ErrorAction SilentlyContinue" >nul 2>&1
title [Jaskier] GeminiHydra v15 Release
echo !BOLD!!MAGENTA!=== GeminiHydra v15 Release ===!RESET!

:: [#1] Start timer
set "_t0=%time%"

:: Log init + redirect all output to log file [#9]
call "%LIB%" :log_init "geminihydra" "release"
if not "!LOGFILE!"=="" (
    echo !CYAN![LOG]!RESET! Output also logged to !LOGFILE!
    >"!LOGFILE!" echo === GeminiHydra v15 Release — %date% %time% ===
)

:: [#5] Git changelog since last tag/release
echo.
for /f "tokens=*" %%t in ('git describe --tags --abbrev^=0 2^>nul') do set "_last_tag=%%t"
if defined _last_tag (
    echo !CYAN![GIT]!RESET! Changes since !_last_tag!:
    for /f "tokens=*" %%l in ('git log --oneline "!_last_tag!..HEAD" 2^>nul') do (
        echo   %%l
        >>"!LOGFILE!" echo   %%l
    )
) else (
    echo !CYAN![GIT]!RESET! Recent commits:
    for /f "tokens=*" %%l in ('git log --oneline -5 2^>nul') do (
        echo   %%l
        >>"!LOGFILE!" echo   %%l
    )
)
echo.

:: [#6] Validate .env
call "%LIB%" :env_check "%~dp0.env" "GOOGLE_API_KEY ANTHROPIC_API_KEY"
call "%LIB%" :env_check "%~dp0backend\.env" "DATABASE_URL GOOGLE_API_KEY ANTHROPIC_API_KEY"

:: Docker DB check
call "%LIB%" :docker_db_check "geminihydra-pg" "%~dp0backend"

:: Kill old processes FIRST (before building — exe is locked while backend runs)
:: Phase 1: Kill by image name — catches orphaned processes from cargo run, Claude sessions, etc.
taskkill /F /IM geminihydra-backend.exe >nul 2>&1 && (
    echo !YELLOW![KILL]!RESET! Killed geminihydra-backend.exe by image name
    %SYSTEMROOT%\System32\timeout.exe /t 2 /nobreak >nul
)
:: Phase 2: Kill by port — catches anything else holding the port
call "%LIB%" :kill_port 8081 "backend"
call "%LIB%" :wait_port_free 8081 30
if errorlevel 1 goto :abort
call "%LIB%" :kill_port 4176 "preview"
call "%LIB%" :wait_port_free 4176
if errorlevel 1 goto :abort
call "%LIB%" :kill_port 5176 "frontend dev"

:: Cargo build (exe now unlocked)
call "%LIB%" :cargo_check "%~dp0backend" "geminihydra-backend.exe"
if errorlevel 1 goto :abort

:: Partner check
call "%LIB%" :partner_check 8082 "ClaudeHydra"

:: Start backend
echo !CYAN![START]!RESET! Backend on port 8081...
start "[Jaskier] GeminiHydra Backend" /min cmd /c "cd /d %~dp0backend && target\release\geminihydra-backend.exe 2>&1"
%SYSTEMROOT%\System32\timeout.exe /t 2 /nobreak >nul

:: [#2] Health check — fatal on failure (abort if backend doesn't start)
call :health_check_fatal 8081 20
if errorlevel 1 goto :abort

:: [#10] Smoke test — verify backend is fully functional
call :smoke_test
>>"!LOGFILE!" echo [SMOKE] completed

:: [#3] Build frontend with timing
echo !CYAN![BUILD]!RESET! Building frontend...
set "_fe_t0=%time%"
call npm run build
if errorlevel 1 goto :abort
call :elapsed_between "!_fe_t0!" "!time!" "_fe_dur"
echo !GREEN![BUILD]!RESET! Frontend built in !_fe_dur!s

:: Start preview (BEFORE Chrome — so port is ready)
echo !CYAN![PREVIEW]!RESET! Starting preview on port 4176...
start "[Jaskier] GeminiHydra Preview" /min cmd /c "cd /d %~dp0 && npm run preview 2>&1"
%SYSTEMROOT%\System32\timeout.exe /t 2 /nobreak >nul
call "%LIB%" :port_validate 4176 10

:: [#7] Verify preview actually serves content (not just LISTENING)
call :http_ready 4176 10
if errorlevel 1 (
    echo !YELLOW![WARN]!RESET! Preview port listening but not serving HTTP yet
)

:: Open Chrome in app mode
start "" chrome --app=http://localhost:4176

:: Toast notification
call "%LIB%" :toast "GeminiHydra v15" "Release preview on port 4176"

:: [#1] Calculate total elapsed time
call :elapsed_between "!_t0!" "!time!" "_total_dur"
echo.
echo !GREEN!!BOLD![DONE]!RESET! GeminiHydra v15 release ready on !BOLD!http://localhost:4176!RESET!
echo !CYAN!       Total time: !_total_dur!s!RESET!

:: [#9] Append summary to log file
>>"!LOGFILE!" echo.
>>"!LOGFILE!" echo === Release completed: %date% %time% (total: !_total_dur!s) ===

:: [#8] Cleanup instructions + trap hint
echo.
echo !YELLOW!To stop:!RESET! Close this window, or run:
echo   taskkill /F /IM geminihydra-backend.exe
echo   taskkill /F /FI "WINDOWTITLE eq GeminiHydra Preview"
echo.

:: [#8] Wait loop with cleanup on exit
:wait_loop
echo !YELLOW!Hiding to tray... Check the system tray icon to restore or stop.!RESET!
powershell -NoProfile -ExecutionPolicy Bypass -File "C:\Users\BIURODOM\Desktop\ClaudeDesktop\tray-minimizer.ps1" -AppTitle "GeminiHydra Release" -IconPath "C:\Users\BIURODOM\Desktop\ClaudeDesktop\.jaskier-icons\geminihydra.ico" -KillExe "geminihydra-backend" -KillTitle "[Jaskier] GeminiHydra"
goto :cleanup

:cleanup
echo !YELLOW![STOP]!RESET! Shutting down services...
taskkill /F /IM geminihydra-backend.exe >nul 2>&1
REM Kill preview by window title
taskkill /F /FI "WINDOWTITLE eq GeminiHydra Preview" >nul 2>&1
REM Also kill node processes on preview port
for /f "tokens=5" %%a in ('netstat -ano 2^>nul ^| findstr ":4176 " ^| findstr LISTENING') do (
    taskkill /F /pid %%a >nul 2>&1
)
echo !GREEN![DONE]!RESET! All services stopped.
endlocal
goto :eof

:abort
call :elapsed_between "!_t0!" "!time!" "_total_dur"
echo !RED![ABORT]!RESET! Release failed after !_total_dur!s — fix issues above and retry.
>>"!LOGFILE!" echo === ABORT: %date% %time% (after !_total_dur!s) ===
endlocal
exit /b 1

:: ========================================================================
:: LOCAL SUBROUTINES (not in jaskier-lib — GeminiHydra-specific)
:: ========================================================================

:: -- [#2] Fatal health check — aborts on timeout -------------------------
:health_check_fatal
set "_hcf_port=%~1"
set "_hcf_max=%~2"
if "%_hcf_max%"=="" set "_hcf_max=20"
set /a _hcf_tries=0
echo !CYAN![WAIT]!RESET! Waiting for backend on port %_hcf_port%...
:_hcf_loop
set /a _hcf_tries+=1
curl -sf http://localhost:%_hcf_port%/api/health >nul 2>&1
if not errorlevel 1 (
    echo !GREEN![OK]!RESET! Backend healthy on port %_hcf_port% ^(took !_hcf_tries!s^)
    exit /b 0
)
if !_hcf_tries! GEQ !_hcf_max! (
    echo !RED![FAIL]!RESET! Backend not responding after !_hcf_max!s — aborting
    exit /b 1
)
%SYSTEMROOT%\System32\timeout.exe /t 1 /nobreak >nul
goto :_hcf_loop

:: -- [#10] Smoke test — quick API verification ----------------------------
:smoke_test
echo !CYAN![SMOKE]!RESET! Verifying backend...
set "_smoke_ok=1"
REM Fetch health JSON and parse key fields
for /f "tokens=*" %%j in ('curl -sf http://localhost:8081/api/health 2^>nul') do set "_health=%%j"
if not defined _health (
    echo !RED![SMOKE]!RESET! Health endpoint returned empty
    exit /b 0
)
REM Google provider (case-insensitive: JSON has "Google Gemini...")
echo !_health! | findstr /i /c:"google" >nul 2>&1
if not errorlevel 1 (
    echo !GREEN!  [OK]!RESET! Google AI provider: available
) else (
    echo !RED!  [FAIL]!RESET! Google AI provider: unavailable
    set "_smoke_ok=0"
)
REM Anthropic provider (case-insensitive: JSON has "Anthropic Claude")
echo !_health! | findstr /i /c:"anthropic" >nul 2>&1
if not errorlevel 1 (
    echo !GREEN!  [OK]!RESET! Anthropic AI provider: available
) else (
    echo !YELLOW!  [WARN]!RESET! Anthropic AI provider: unavailable
    set "_smoke_ok=0"
)
REM Models count (API returns "total_models", not "model_count")
for /f "tokens=*" %%m in ('curl -sf http://localhost:8081/api/models 2^>nul ^| findstr /c:"total_models"') do set "_models_resp=%%m"
if defined _models_resp (
    echo !GREEN!  [OK]!RESET! Model registry: cached
) else (
    echo !YELLOW!  [?]!RESET! Model registry: could not verify
)
if "!_smoke_ok!"=="1" (
    echo !GREEN![SMOKE]!RESET! All checks passed
) else (
    echo !YELLOW![SMOKE]!RESET! Some checks had warnings ^(see above^)
)
exit /b 0

:: -- [#7] HTTP ready check — verify port serves HTTP (not just LISTENING) -
:http_ready
set "_hr_port=%~1"
set "_hr_max=%~2"
if "%_hr_max%"=="" set "_hr_max=10"
set /a _hr_tries=0
:_hr_loop
set /a _hr_tries+=1
curl -sf -o nul -w "%%{http_code}" http://localhost:%_hr_port%/ 2>nul | findstr /c:"200" >nul 2>&1
if not errorlevel 1 (
    echo !GREEN![OK]!RESET! Preview serving HTTP on port %_hr_port%
    exit /b 0
)
if !_hr_tries! GEQ !_hr_max! (
    exit /b 1
)
%SYSTEMROOT%\System32\timeout.exe /t 1 /nobreak >nul
goto :_hr_loop

:: -- [#1] Elapsed time calculator (HH:MM:SS.CC difference) ----------------
:elapsed_between
set "_eb_t0=%~1"
set "_eb_t1=%~2"
set "_eb_var=%~3"
REM Parse start time
for /f "tokens=1-4 delims=:,. " %%a in ("%_eb_t0%") do (
    set /a "_eb_s0=(%%a %% 100)*3600 + %%b*60 + %%c"
)
REM Parse end time
for /f "tokens=1-4 delims=:,. " %%a in ("%_eb_t1%") do (
    set /a "_eb_s1=(%%a %% 100)*3600 + %%b*60 + %%c"
)
REM Handle midnight wrap
set /a "_eb_diff=_eb_s1 - _eb_s0"
if !_eb_diff! LSS 0 set /a "_eb_diff+=86400"
set "%_eb_var%=!_eb_diff!"
exit /b 0
