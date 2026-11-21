[CmdletBinding()]
param(
    [ValidateRange(1, 20)]
    [int]$Repeat = 1,
    [string]$Cli = ""
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
if (-not $Cli) {
    $Cli = Join-Path $ProjectRoot "target\x86_64-pc-windows-gnu\debug\lutetium.exe"
}
if (-not (Test-Path -LiteralPath $Cli -PathType Leaf)) {
    throw "Lutetium executable not found: $Cli. Build it first with cargo build -p lutetium-cli."
}
$Cli = (Resolve-Path -LiteralPath $Cli).Path

function Invoke-Case([string]$Name, [string[]]$Arguments, [int]$ExpectedExit, [string]$ExpectedOutput = "") {
    $clock = [System.Diagnostics.Stopwatch]::StartNew()
    $actualOutput = @(& $Cli @Arguments 2>&1 | ForEach-Object { "$($_)" }) -join "`n"
    $exitCode = $LASTEXITCODE
    $clock.Stop()
    if ($exitCode -ne $ExpectedExit) {
        throw "$Name expected exit $ExpectedExit, got $exitCode.`n$actualOutput"
    }
    if ($ExpectedOutput -and $actualOutput -cnotmatch [regex]::Escape($ExpectedOutput)) {
        throw "$Name output did not contain expected text '$ExpectedOutput'.`n$actualOutput"
    }
    [pscustomobject]@{
        Case = $Name
        Exit = $exitCode
        ElapsedMs = $clock.ElapsedMilliseconds
        Result = "PASS"
    }
}

Push-Location $ProjectRoot
try {
    $version = @(& $Cli --version 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "Could not query CLI version: $version" }
    $results = @()
    for ($iteration = 1; $iteration -le $Repeat; $iteration++) {
        $results += Invoke-Case "VM Fibonacci" @("run", "examples\fibonacci.lu") 0 "55"
        $results += Invoke-Case "VM lists/maps" @("run", "examples\maps.lu") 0 "1225"
        $results += Invoke-Case "VM GC stress" @("run", "examples\maps.lu", "--gc-stress", "--gc-stats") 0 "gc:"
        $results += Invoke-Case "JSON diagnostics" @("check", "--diagnostics=json", "examples\hello.lu") 0 '"diagnostics":[]'
        $results += Invoke-Case "Native subset rejection" @("build", "examples\maps.lu", "--target", "native") 1 "E6002"
    }
    Write-Output "Correctness benchmark (not a performance benchmark)"
    Write-Output "CLI: $Cli"
    Write-Output "Version: $($version -join ' ')"
    Write-Output "OS: $([System.Runtime.InteropServices.RuntimeInformation]::OSDescription)"
    Write-Output "Iterations: $Repeat"
    $results | Format-Table -AutoSize
} finally {
    Pop-Location
}
