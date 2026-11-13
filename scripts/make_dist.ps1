# Assembles the portable Lutetium distribution.
#
# Usage:
#   .\scripts\make_dist.ps1
#
# Output:
#   dist\lutetium-windows-x64\          (portable layout)
#   dist\lutetium-windows-x64.zip       (zip archive)
#
# The portable tree runs without the source repository.

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$DistRoot    = Join-Path $ProjectRoot "dist"
$DistDir     = Join-Path $DistRoot "lutetium-windows-x64"

Write-Host "== Lutetium portable distribution =="

Push-Location $ProjectRoot
try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) { Write-Error "release build failed"; exit 1 }
} finally {
    Pop-Location
}

$BuiltExe = Join-Path $ProjectRoot "target\x86_64-pc-windows-gnu\release\lutetium.exe"
if (-not (Test-Path $BuiltExe)) {
    Write-Error "compiler executable missing: $BuiltExe"
    exit 1
}

if (Test-Path $DistDir) { Remove-Item -Recurse -Force $DistDir }
New-Item -ItemType Directory -Force -Path (Join-Path $DistDir "bin") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $DistDir "lib") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $DistDir "std") | Out-Null

Copy-Item -Force $BuiltExe (Join-Path $DistDir "bin\lutetium.exe")
Copy-Item -Recurse (Join-Path $ProjectRoot "examples") (Join-Path $DistDir "examples")
Copy-Item -Recurse (Join-Path $ProjectRoot "docs") (Join-Path $DistDir "docs")
Copy-Item -Force (Join-Path $ProjectRoot "README.md") (Join-Path $DistDir "README.md")

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

To use `lutetium` from any terminal, add the `bin` directory of this
folder to your user PATH.

Native compilation (`--target native`) additionally requires a Rust
toolchain with the x86_64-pc-windows-gnu target on the machine that
performs the compilation (the produced .exe files have no such
requirement).
"@ | Set-Content -Encoding UTF8 $Readme

$Zip = Join-Path $DistRoot "lutetium-windows-x64.zip"
if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path $DistDir -DestinationPath $Zip
Write-Host "[ok] wrote $Zip"
Write-Host "[ok] wrote $DistDir"
