[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$sourceRoot = $PSScriptRoot
$packageRoot = Split-Path -Path $sourceRoot -Parent
$target = "x86_64-pc-windows-msvc"
$builtDll = Join-Path -Path $sourceRoot -ChildPath "target\$target\release\typing_effect.dll"
$outputMod2 = Join-Path -Path $packageRoot -ChildPath "TypingEffect.mod2"
$syntaxCheckScript = Join-Path -Path $sourceRoot -ChildPath "check-syntax.ps1"
$verifyScript = Join-Path -Path $sourceRoot -ChildPath "verify-exports.ps1"

if (-not (Get-Command -Name cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found. Install the Rust stable toolchain with rustup."
}
if (-not (Get-Command -Name rustup -ErrorAction SilentlyContinue)) {
    throw "rustup was not found. Install rustup first."
}

& $syntaxCheckScript

rustup target add $target
if ($LASTEXITCODE -ne 0) {
    throw "rustup target add failed with exit code $LASTEXITCODE."
}

Push-Location -Path $sourceRoot
try {
    cargo test --target $target
    if ($LASTEXITCODE -ne 0) {
        throw "cargo test failed with exit code $LASTEXITCODE."
    }

    cargo build --release --target $target
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $builtDll -PathType Leaf)) {
    throw "The built DLL was not found: $builtDll"
}

Copy-Item -LiteralPath $builtDll -Destination $outputMod2 -Force
& $verifyScript -ModulePath $outputMod2

Write-Host "Built: $outputMod2"
Write-Host "Full Rust build completed. No additional runtime DLL is required."
