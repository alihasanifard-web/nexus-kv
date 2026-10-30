@echo off
title NexusKV 3-Node Cluster Launcher
cls
echo =====================================================================
echo   NexusKV Distributed Storage Cluster Launcher
echo   Architect: Ali Hasanifard (ali hasanifard)
echo =====================================================================
echo.

set EXE_PATH=%~dp0nexus-kv.exe
if not exist "%EXE_PATH%" set EXE_PATH=C:\Users\V I S I O N\Desktop\nexus-kv.exe
if not exist "%EXE_PATH%" set EXE_PATH=C:\Users\V I S I O N\nexus-kv\target\release\nexus-kv.exe

if not exist "%EXE_PATH%" (
    echo [ERROR] nexus-kv.exe not found! Please build the binary first.
    pause
    exit /b 1
)

echo [*] Starting Node 1 on port 6379 (Telemetry: 7379)...
start "NexusKV - Node 1" /min "%EXE_PATH%" 6379 1 "C:\Users\V I S I O N\nexus-kv\data\cluster_node_1"

echo [*] Starting Node 2 on port 6380 (Telemetry: 7380)...
start "NexusKV - Node 2" /min "%EXE_PATH%" 6380 2 "C:\Users\V I S I O N\nexus-kv\data\cluster_node_2"

echo [*] Starting Node 3 on port 6381 (Telemetry: 7381)...
start "NexusKV - Node 3" /min "%EXE_PATH%" 6381 3 "C:\Users\V I S I O N\nexus-kv\data\cluster_node_3"

timeout /t 2 /nobreak >nul

echo.
echo  =====================================================================
echo    Cluster armed and ready!
echo    - Node 1 (Master): 127.0.0.1:6379  (Telemetry: http://127.0.0.1:7379)
echo    - Node 2 (Replica): 127.0.0.1:6380 (Telemetry: http://127.0.0.1:7380)
echo    - Node 3 (Replica): 127.0.0.1:6381 (Telemetry: http://127.0.0.1:7381)
echo  =====================================================================
echo.

set DASH_EXE=%~dp0nexus-dashboard.exe
if not exist "%DASH_EXE%" set DASH_EXE=C:\Users\V I S I O N\Desktop\nexus-dashboard.exe

if exist "%DASH_EXE%" (
    echo [*] Opening NexusKV Telemetry Dashboard...
    start "" "%DASH_EXE%"
) else (
    start "" "C:\Users\V I S I O N\Desktop\nexus_dashboard.html"
)

echo [OK] All systems operational. To stop the cluster, run stop-cluster.bat.
echo.
pause
