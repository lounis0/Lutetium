# Lutetium installer (user-level, no administrator privileges required).
#
# Usage:
#   .\scripts\install.ps1
#
# What it does:
#   1. Locates the project root (this script's parent directory).
#   2. Checks Rust + the pinned x86_64-pc-windows-gnu toolchain.
#   3. Builds Lutetium in release mode.
#   4. Verifies the compiler executable exists.
#   5. Installs it to %LOCALAPPDATA%\Programs\Lutetium\bin\lutetium.exe.
#   6. Adds that bin directory to the *user* PATH (no duplicates, nothing
#      else is modified).
#   7. Verifies the installed executable actually runs.
#
# A new terminal session is required for PATH to take effect.

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$InstallRoot = Join-Path $env:LOCALAPPDATA "Programs\Lutetium"
$InstallBin  = Join-Path $InstallRoot "bin"
$InstallLib  = Join-Path $InstallRoot "lib"
$InstallStd  = Join-Path $InstallRoot "std"
$InstallConfig = Join-Path $InstallRoot "config"

Write-Host "== Lutetium installer =="
Write-Host "Project root: $ProjectRoot"
Write-Host "Install target: $InstallRoot"
Write-Host ""

# --- 1. Toolchain check ------------------------------------------------------
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo was not found on PATH. Install Rust (rustup) first."
    exit 1
}
$RustTarget = "x86_64-pc-windows-gnu"
$InstalledTargets = & rustup target list --installed 2>$null
if ($LASTEXITCODE -ne 0 -or -not ($InstalledTargets -contains $RustTarget)) {
    Write-Error "The Rust target $RustTarget is required but not installed. Run: rustup target add $RustTarget"
    exit 1
}
Write-Host "[ok] Rust toolchain and $RustTarget target present"

# --- 2. Release build ---------------------------------------------------------
Write-Host "[..] building Lutetium (release)..."
Push-Location $ProjectRoot
try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "cargo build --release failed."
        exit 1
    }
} finally {
    Pop-Location
}

# --- 3. Verify the compiler executable ---------------------------------------
$BuiltExe = Join-Path $ProjectRoot "target\x86_64-pc-windows-gnu\release\lutetium.exe"
if (-not (Test-Path $BuiltExe)) {
    Write-Error "expected compiler executable was not produced: $BuiltExe"
    exit 1
}
$VersionOutput = & $BuiltExe --version
if ($LASTEXITCODE -ne 0 -or -not ($VersionOutput -like "Lutetium*")) {
    Write-Error "the built executable did not report a version correctly."
    exit 1
}
Write-Host "[ok] built $VersionOutput"

# --- 4. Install ---------------------------------------------------------------
New-Item -ItemType Directory -Force -Path $InstallBin | Out-Null
New-Item -ItemType Directory -Force -Path $InstallLib | Out-Null
New-Item -ItemType Directory -Force -Path $InstallStd | Out-Null
New-Item -ItemType Directory -Force -Path $InstallConfig | Out-Null

Copy-Item -Force $BuiltExe (Join-Path $InstallBin "lutetium.exe")
if (-not (Test-Path (Join-Path $InstallBin "lutetium.exe"))) {
    Write-Error "the executable was not copied to the installation directory."
    exit 1
}
# Example programs ship with the installation (useful reference + smoke tests).
$ExamplesDest = Join-Path $InstallRoot "examples"
if (Test-Path $ExamplesDest) { Remove-Item -Recurse -Force $ExamplesDest }
Copy-Item -Recurse (Join-Path $ProjectRoot "examples") $ExamplesDest
Write-Host "[ok] installed to $InstallRoot"

# --- 5. User PATH (no duplicates) ----------------------------------------------
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$Entries = @()
if ($UserPath) { $Entries = $UserPath -split ";" | Where-Object { $_ -ne "" } }
$AlreadyPresent = $Entries | Where-Object { $_.TrimEnd('\') -ieq $InstallBin }
if (-not $AlreadyPresent) {
    $Entries += $InstallBin
    $NewPath = ($Entries -join ";")
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    Write-Host "[ok] added $InstallBin to the user PATH"
} else {
    Write-Host "[ok] PATH already contains $InstallBin (no duplicate added)"
}

# --- 6. Verify the installed executable runs ----------------------------------
$InstalledExe = Join-Path $InstallBin "lutetium.exe"
$Check = & $InstalledExe --version
if ($LASTEXITCODE -ne 0 -or -not ($Check -like "Lutetium*")) {
    Write-Error "the installed executable failed to run."
    exit 1
}
Write-Host "[ok] installed executable verified: $Check"

Write-Host ""
Write-Host "Installation complete."
Write-Host "  Location: $InstallRoot"
Write-Host "  Executable: $InstalledExe"
Write-Host ""
Write-Host "NOTE: open a NEW PowerShell window (or re-log in) so PATH picks up the"
Write-Host "change. Then verify with:"
Write-Host "    lutetium --version"
Write-Host "    where.exe lutetium"
