@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"
set "LIB=C:\Users\BIURODOM\Desktop\ClaudeDesktop\jaskier-lib.bat"

:: Init colors
call "%LIB%" :init_colors
:: Kill previous instances
taskkill /F /FI "WINDOWTITLE eq [Jaskier] GeminiHydra*" >nul 2>&1
powershell -NoProfile -Command "Get-Process | Where-Object { $_.Name -eq 'powershell' -and $_.CommandLine -like '*tray-minimizer.ps1*' -and $_.CommandLine -like '*GeminiHydra DEV*' } | Stop-Process -Force -ErrorAction SilentlyContinue" >nul 2>&1
title [Jaskier] GeminiHydra v15 DEV
echo !BOLD!!MAGENTA!=== GeminiHydra v15 DEV ===!RESET!

:: [#1] Start timer
set "_t0=%time%"

:: Log init + redirect [#9]
call "%LIB%" :log_init "geminihydra" "dev"
if not "!LOGFILE!"=="" (
    echo !CYAN![LOG]!RESET! Output also logged to !LOGFILE!
    >"!LOGFILE!" echo === GeminiHydra v15 DEV — %date% %time% ===
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

:: Kill old processes
:: Phase 1: Kill by image name
taskkill /F /IM geminihydra-backend.exe >nul 2>&1 && (
    echo !YELLOW![KILL]!RESET! Killed geminihydra-backend.exe by image name
    %SYSTEMROOT%\System32\timeout.exe /t 2 /nobreak >nul
)
:: Phase 2: Kill by port
call "%LIB%" :kill_port 8081 "backend"
call "%LIB%" :wait_port_free 8081 30
if errorlevel 1 goto :abort
call "%LIB%" :kill_port 5176 "frontend dev"
call "%LIB%" :wait_port_free 5176
if errorlevel 1 goto :abort

:: Partner check
call "%LIB%" :partner_check 8082 "ClaudeHydra"

:: Start backend
echo !CYAN![START]!RESET! Backend ^(cargo run^)...
start "[Jaskier] GeminiHydra Backend" /min cmd /c "cd /d %~dp0backend && cargo run 2>&1"

:: [#2] Health check — fatal on failure (abort if backend doesn't start)
call :health_check_fatal 8081 60
if errorlevel 1 goto :abort

:: [#10] Smoke test — verify backend is fully functional
call :smoke_test
>>"!LOGFILE!" echo [SMOKE] completed

:: [#1] Startup time
call :elapsed_between "!_t0!" "!time!" "_total_dur"
echo.
echo !GREEN![READY]!RESET! Backend ready in !_total_dur!s

:: [#9] Log startup
>>"!LOGFILE!" echo === Backend ready: %date% %time% (startup: !_total_dur!s) ===

:: Open Chrome in app mode
start "" chrome --app=http://localhost:5176

:: Toast notification
call "%LIB%" :toast "GeminiHydra v15" "DEV server starting on port 5176"

:: Cleanup hint before frontend foreground
echo.
echo !YELLOW!To stop backend:!RESET! In another terminal:
echo   taskkill /F /IM geminihydra-backend.exe
echo.

:: Start frontend dev server (foreground — endlocal FIRST)
echo !CYAN![DEV]!RESET! Starting frontend dev server on port 5176...
echo !YELLOW!Hiding to tray... Check the system tray icon to restore or stop.!RESET!
start "" /B powershell -NoProfile -ExecutionPolicy Bypass -File "C:\Users\BIURODOM\Desktop\ClaudeDesktop\tray-minimizer.ps1" -AppTitle "GeminiHydra DEV" -IconPath "C:\Users\BIURODOM\Desktop\ClaudeDesktop\.jaskier-icons\geminihydra.ico" -KillExe "geminihydra-backend" -KillTitle "[Jaskier] GeminiHydra"
endlocal && cd /d "%~dp0" && npm run dev
goto :eof

:abort
call :elapsed_between "!_t0!" "!time!" "_total_dur"
echo !RED![ABORT]!RESET! DEV launch failed after !_total_dur!s — fix issues above and retry.
>>"!LOGFILE!" echo === ABORT: %date% %time% (after !_total_dur!s) ===
endlocal
exit /b 1

:: ========================================================================
:: LOCAL SUBROUTINES (not in jaskier-lib — GeminiHydra DEV-specific)
:: ========================================================================

:: -- [#2] Fatal health check — aborts on timeout -------------------------
:health_check_fatal
set "_hcf_port=%~1"
set "_hcf_max=%~2"
if "%_hcf_max%"=="" set "_hcf_max=60"
set /a _hcf_tries=0
echo !CYAN![WAIT]!RESET! Waiting for backend on port %_hcf_port% ^(cargo compile + start^)...
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
for /f "tokens=*" %%j in ('curl -sf http://localhost:8081/api/health 2^>nul') do set "_health=%%j"
if not defined _health (
    echo !RED![SMOKE]!RESET! Health endpoint returned empty
    exit /b 0
)
REM Google provider
echo !_health! | findstr /c:"google" >nul 2>&1
if not errorlevel 1 (
    echo !GREEN!  [OK]!RESET! Google AI provider: available
) else (
    echo !RED!  [FAIL]!RESET! Google AI provider: unavailable
    set "_smoke_ok=0"
)
REM Anthropic provider
echo !_health! | findstr /c:"anthropic" >nul 2>&1
if not errorlevel 1 (
    echo !GREEN!  [OK]!RESET! Anthropic AI provider: available
) else (
    echo !YELLOW!  [WARN]!RESET! Anthropic AI provider: unavailable
    set "_smoke_ok=0"
)
REM Models count
for /f "tokens=*" %%m in ('curl -sf http://localhost:8081/api/models 2^>nul ^| findstr /c:"model_count"') do set "_models_resp=%%m"
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

:: -- [#1] Elapsed time calculator (HH:MM:SS.CC difference) ----------------
:elapsed_between
set "_eb_t0=%~1"
set "_eb_t1=%~2"
set "_eb_var=%~3"
for /f "tokens=1-4 delims=:,. " %%a in ("%_eb_t0%") do (
    set /a "_eb_s0=(%%a %% 100)*3600 + %%b*60 + %%c"
)
for /f "tokens=1-4 delims=:,. " %%a in ("%_eb_t1%") do (
    set /a "_eb_s1=(%%a %% 100)*3600 + %%b*60 + %%c"
)
set /a "_eb_diff=_eb_s1 - _eb_s0"
if !_eb_diff! LSS 0 set /a "_eb_diff+=86400"
set "%_eb_var%=!_eb_diff!"
exit /b 0
