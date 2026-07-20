$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$bridge = Join-Path $PSScriptRoot "lindera_bridge"
$cpp = Join-Path $PSScriptRoot "TypingEffect.cpp"

Push-Location $bridge
try {
    cargo build --release --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
}

Copy-Item (Join-Path $bridge "target\x86_64-pc-windows-msvc\release\TypingEffectLindera.dll") $root -Force
cl /nologo /LD /MT /std:c++17 /EHsc /utf-8 /O2 $cpp /Fe:(Join-Path $root "TypingEffect.mod2")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Built: TypingEffect.mod2 + TypingEffectLindera.dll"
