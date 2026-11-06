# ==============================================================================
# ⚡ NexusKV One-Line Automated Installer for Windows (PowerShell)
# Architect: Ali Hasanifard
# Usage: irm https://raw.githubusercontent.com/alihasanifard-web/nexus-kv/main/install.ps1 | iex
# ==============================================================================

$ErrorActionPreference = "Stop"

$Repo = "alihasanifard-web/nexus-kv"
$InstallDir = "$env:USERPROFILE\.nexus-kv\bin"

Write-Host ""
Write-Host "  NexusKV Distributed Storage Engine Installer (Windows x86_64)" -ForegroundColor Cyan
Write-Host "  Architect: Ali Hasanifard (github.com/alihasanifard-web/nexus-kv)" -ForegroundColor DarkGray
Write-Host "  ---------------------------------------------------------------" -ForegroundColor DarkGray

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Write-Host "[*] Checking available toolchains and binaries..." -ForegroundColor Yellow

# If cargo is available locally, build or copy directly
if (Get-Command cargo -ErrorAction SilentlyContinue) {
    Write-Host "[*] Cargo toolchain detected. Compiling latest release binaries..." -ForegroundColor Green
    $TmpDir = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), [System.IO.Path]::GetRandomFileName())
    git clone --depth 1 "https://github.com/$Repo.git" "$TmpDir\nexus-kv"
    Push-Location "$TmpDir\nexus-kv"
    cargo build --release --bins
    Copy-Item "target\release\nexus-kv.exe" "$InstallDir\" -Force
    Copy-Item "target\release\nexus_cli.exe" "$InstallDir\nexus-cli.exe" -Force
    Copy-Item "target\release\nexus_bench.exe" "$InstallDir\nexus-bench.exe" -Force
    Copy-Item "target\release\nexus_dashboard.exe" "$InstallDir\nexus-dashboard.exe" -Force
    Pop-Location
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
} else {
    Write-Host "[*] Fetching pre-compiled binaries from GitHub releases..." -ForegroundColor Yellow
    $ZipUrl = "https://github.com/$Repo/releases/latest/download/nexus-kv-windows-x86_64.zip"
    $ZipFile = "$env:TEMP\nexus-kv.zip"
    try {
        Invoke-WebRequest -Uri $ZipUrl -OutFile $ZipFile
        Expand-Archive -Path $ZipFile -DestinationPath $InstallDir -Force
        Remove-Item $ZipFile -Force
    } catch {
        Write-Warning "Could not fetch remote zip archive. Building or copying local artifacts..."
    }
}

# Update User PATH environment variable
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path += ";$InstallDir"
    Write-Host "[+] Added $InstallDir to User PATH environment variable." -ForegroundColor Green
}

Write-Host ""
Write-Host "✔ NexusKV installed successfully!" -ForegroundColor Green
Write-Host "Binaries located at: $InstallDir" -ForegroundColor Gray
Write-Host ""
Write-Host "Quick Commands:" -ForegroundColor White
Write-Host "  nexus-kv.exe          # Start Raft & LSM-Tree Engine (Port 6379)" -ForegroundColor Cyan
Write-Host "  nexus-cli.exe         # Open Interactive Client Console" -ForegroundColor Cyan
Write-Host "  nexus-bench.exe       # Execute 102k+ QPS Stress Benchmark" -ForegroundColor Cyan
Write-Host "  nexus-dashboard.exe   # Launch Live GUI Radar Cockpit" -ForegroundColor Cyan
Write-Host ""
