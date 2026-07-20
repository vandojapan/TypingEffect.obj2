$ErrorActionPreference = "Stop"

$sourceRoot = $PSScriptRoot
$packageRoot = Split-Path -Parent $sourceRoot
$target = "x86_64-pc-windows-msvc"
$builtDll = Join-Path $sourceRoot "target\$target\release\typing_effect.dll"
$outputMod2 = Join-Path $packageRoot "TypingEffect.mod2"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargoが見つかりません。RustupでRust stableをインストールしてください。"
}

rustup target add $target
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Push-Location $sourceRoot
try {
    cargo test
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    cargo build --release --target $target
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
}

Copy-Item $builtDll $outputMod2 -Force
& (Join-Path $sourceRoot "verify-exports.ps1") -ModulePath $outputMod2
Write-Host "Built: $outputMod2"
Write-Host "Full Rust構成のため、追加DLLはありません。"
