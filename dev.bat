@echo off
title GeminiHydra DEV :5176

:: Launch Chrome in debug mode (shared, idempotent)
call "C:\Users\BIURODOM\Desktop\chrome-debug.bat"

:: Kill old dev server on port 5176
for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":5176 " ^| findstr "LISTENING"') do (
    taskkill /PID %%a /F >nul 2>&1
)

:: Start dev server
cd /d "C:\Users\BIURODOM\Desktop\GeminiHydra-v15"
pnpm dev
