@echo off
title NexusKV Cluster Shutdown
cls
echo =====================================================================
echo   NexusKV Distributed Cluster Shutdown
echo   Architect: Ali Hasanifard
echo =====================================================================
echo.

echo [*] Terminating all running nexus-kv cluster instances...
taskkill /F /IM nexus-kv.exe >nul 2>&1

if %ERRORLEVEL% EQU 0 (
    echo [OK] All NexusKV server nodes stopped successfully.
) else (
    echo [INFO] No active nexus-kv instances were running.
)

echo.
pause
