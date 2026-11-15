# ============================================================
# Isolate Client — Windows release build (Step 4)
#
# Produces a standalone installer (.exe via NSIS, .msi via WiX) and a
# portable .exe under src-tauri\target\release\bundle\ and
# src-tauri\target\release\.
#
# Run from the repo root in PowerShell:
#   .\scripts\build-windows.ps1
#   .\scripts\build-windows.ps1 -Quick        # skip LTO
#   .\scripts\build-windows.ps1 -Portable     # also zip the raw .exe
#
# Requires: Rust (msvc toolchain), Node.js, Visual Studio Build Tools
# (or run inside GitHub Actions windows-latest, which has all three).
# ============================================================
[CmdletBinding()]
param(
    [switch]$Quick,
    [switch]$Portable,
    [string]$Bundles = "nsis,msi"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

# ---- preflight ------------------------------------------------
foreach ($tool in @("cargo", "npx", "rustc")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Error "$tool not found — install Rust + Node.js first"
    }
}
Write-Host "[build] rustc $(rustc --version)"
Write-Host "[build] bundles=$Bundles quick=$Quick"

# ---- low-RAM build knobs --------------------------------------
# Cap Cargo parallelism so a 4 GB box survives LTO/codegen.
$cores = [Environment]::ProcessorCount
$maxJobs = if ($cores -gt 4) { 4 } else { $cores }
$env:CARGO_BUILD_JOBS = "$maxJobs"
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "1"
if ($Quick) {
    $env:CARGO_PROFILE_RELEASE_LTO = "false"
    Write-Host "[build] quick mode: LTO off"
}
$env:NODE_OPTIONS = "--max-old-space-size=2048"
Write-Host "[build] jobs=$maxJobs"

# ---- frontend -------------------------------------------------
Write-Host "[build] 1/3  vite build"
npm run build
if ($LASTEXITCODE -ne 0) { Write-Error "frontend build failed" }

# ---- rust bundle ---------------------------------------------
Write-Host "[build] 2/3  cargo bundle ($Bundles)"
npx tauri build --bundles $Bundles
if ($LASTEXITCODE -ne 0) { Write-Error "tauri build failed" }

# ---- optional portable exe ------------------------------------
if ($Portable) {
    $exe = Join-Path $Root "src-tauri\target\release\Isolate Client.exe"
    if (-not (Test-Path $exe)) { $exe = Join-Path $Root "src-tauri\target\release\isolate-client.exe" }
    if (Test-Path $exe) {
        $outDir = Join-Path $Root "dist-portable"
        New-Item -ItemType Directory -Force -Path $outDir | Out-Null
        $zip = Join-Path $outDir "IsolateClient-windows-x64.zip"
        if (Test-Path $zip) { Remove-Item $zip -Force }
        Compress-Archive -Path $exe -DestinationPath $zip
        Write-Host "[build] portable → $zip"
    } else {
        Write-Host "[build] portable exe not found (looked for Isolate Client.exe)"
    }
}

# ---- report ---------------------------------------------------
Write-Host "[build] 3/3  artifacts"
$bundleDir = Join-Path $Root "src-tauri\target\release\bundle"
$found = $false
if (Test-Path $bundleDir) {
    Get-ChildItem -Path $bundleDir -Recurse -Include *.exe, *.msi | ForEach-Object {
        $mb = [math]::Round($_.Length / 1MB, 1)
        Write-Host ("  OK {0,8} MB  {1}" -f $mb, $_.FullName.Substring($Root.Length + 1))
        $found = $true
    }
}
if (-not $found) { Write-Error "no bundle produced — check cargo output above" }
Write-Host "[build] done."
