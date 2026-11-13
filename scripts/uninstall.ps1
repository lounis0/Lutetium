# Lutetium uninstaller.
#
# Removes:
#   - the Lutetium installation directory (%LOCALAPPDATA%\Programs\Lutetium)
#   - the Lutetium entry from the user PATH (only that entry)
#
# Never removes: the project repository, the Rust toolchain, or any unrelated
# PATH entries.

$ErrorActionPreference = "Stop"

$InstallRoot = Join-Path $env:LOCALAPPDATA "Programs\Lutetium"
$InstallBin  = Join-Path $InstallRoot "bin"

Write-Host "== Lutetium uninstaller =="
Write-Host "This will remove:"
Write-Host "  - directory: $InstallRoot"
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$HasPathEntry = $false
if ($UserPath) {
    $HasPathEntry = ($UserPath -split ";" | Where-Object { $_.TrimEnd('\') -ieq $InstallBin }).Count -gt 0
    if ($HasPathEntry) {
        Write-Host "  - user PATH entry: $InstallBin"
    }
}
Write-Host ""
Write-Host "It will NOT remove the project repository or the Rust toolchain."
$answer = Read-Host "Proceed? (y/N)"
if ($answer -ne "y" -and $answer -ne "Y") {
    Write-Host "Aborted. Nothing was removed."
    exit 0
}

if (Test-Path $InstallRoot) {
    Remove-Item -Recurse -Force $InstallRoot
    Write-Host "[ok] removed $InstallRoot"
} else {
    Write-Host "[ok] $InstallRoot does not exist (nothing to remove)"
}

if ($HasPathEntry) {
    $Entries = $UserPath -split ";" | Where-Object { $_ -ne "" -and $_.TrimEnd('\') -ine $InstallBin }
    [Environment]::SetEnvironmentVariable("Path", ($Entries -join ";"), "User")
    Write-Host "[ok] removed the Lutetium entry from the user PATH"
}

Write-Host "Uninstallation complete. Open a new terminal session for PATH to refresh."
