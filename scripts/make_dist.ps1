# Assembles the portable Lutetium distribution.
#
# Usage:
#   .\scripts\make_dist.ps1
#   .\scripts\make_dist.ps1 -BuiltExe .\path\to\release\lutetium.exe
#
# Output:
#   dist\lutetium-windows-x64\              (portable layout)
#   dist\lutetium-<version>-windows-x64.zip (zip archive)
#
# The portable tree runs without the source repository.
# An existing release build can be supplied with -BuiltExe.

[CmdletBinding()]
param(
    [string]$BuiltExe = ""
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$DistRoot    = Join-Path $ProjectRoot "dist"
$DistDir     = Join-Path $DistRoot "lutetium-windows-x64"

Write-Host "== Lutetium portable distribution =="

if (-not $BuiltExe) {
    Push-Location $ProjectRoot
    try {
        & cargo build --release
        if ($LASTEXITCODE -ne 0) { Write-Error "release build failed"; exit 1 }
    } finally {
        Pop-Location
    }
    $BuiltExe = Join-Path $ProjectRoot "target\x86_64-pc-windows-gnu\release\lutetium.exe"
}
if (-not (Test-Path -LiteralPath $BuiltExe -PathType Leaf)) {
    Write-Error "compiler executable missing: $BuiltExe"
    exit 1
}
$BuiltExe = (Resolve-Path -LiteralPath $BuiltExe).Path
$VersionLine = (& $BuiltExe --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $VersionLine -notmatch '^Lutetium \d+\.\d+\.\d+$') {
    Write-Error "compiler executable reported an invalid version: $VersionLine"
    exit 1
}
$Version = $VersionLine -replace '^Lutetium ', ''

if (Test-Path $DistDir) { Remove-Item -Recurse -Force $DistDir }
New-Item -ItemType Directory -Force -Path (Join-Path $DistDir "bin") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $DistDir "lib") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $DistDir "std") | Out-Null

Copy-Item -Force $BuiltExe (Join-Path $DistDir "bin\lutetium.exe")
Copy-Item -Recurse (Join-Path $ProjectRoot "examples") (Join-Path $DistDir "examples")
Copy-Item -Recurse (Join-Path $ProjectRoot "docs") (Join-Path $DistDir "docs")
Copy-Item -Force (Join-Path $ProjectRoot "README.md") (Join-Path $DistDir "README.md")
Copy-Item -Force (Join-Path $ProjectRoot "CHANGELOG.md") (Join-Path $DistDir "CHANGELOG.md")
Copy-Item -Force (Join-Path $ProjectRoot "LICENSE") (Join-Path $DistDir "LICENSE")
Copy-Item -Force (Join-Path $ProjectRoot "scripts\install.ps1") (Join-Path $DistDir "install.ps1")
Set-Content -Path (Join-Path $DistDir "VERSION.txt") -Value $Version -Encoding ASCII

# verify the portable compiler runs standalone
$Check = & (Join-Path $DistDir "bin\lutetium.exe") --version
if ($LASTEXITCODE -ne 0 -or -not ($Check -like "Lutetium*")) {
    Write-Error "portable executable failed verification"
    exit 1
}
Write-Host "[ok] portable compiler verified: $Check"

$Readme = Join-Path $DistDir "README.txt"
@"
Lutetium portable distribution (Windows x64)
===========================================

Run the compiler without installing:

    bin\lutetium.exe --version
    bin\lutetium.exe run examples\fibonacci.lu

To install the prebuilt CLI for your user (no administrator privileges):

    .\install.ps1 -BuiltExe .\bin\lutetium.exe

The installer verifies the executable version against VERSION.txt.

Native compilation (`--target native`) additionally requires a Rust
toolchain with the x86_64-pc-windows-gnu target on the machine that
performs the compilation (the produced .exe files have no such
requirement).
"@ | Set-Content -Encoding UTF8 $Readme

$Zip = Join-Path $DistRoot "lutetium-$Version-windows-x64.zip"
if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path $DistDir -DestinationPath $Zip
Write-Host "[ok] wrote $Zip"
Write-Host "[ok] wrote $DistDir"
