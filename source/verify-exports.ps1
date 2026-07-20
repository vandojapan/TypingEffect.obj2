param(
    [Parameter(Mandatory = $true)]
    [string]$ModulePath
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $ModulePath)) {
    throw "mod2が見つかりません: $ModulePath"
}
if (-not (Get-Command dumpbin.exe -ErrorAction SilentlyContinue)) {
    throw "dumpbin.exeが見つかりません。x64 Native Tools PowerShell for VS 2022で実行してください。"
}

$headers = (& dumpbin.exe /headers $ModulePath | Out-String)
if ($headers -notmatch "8664 machine \(x64\)") {
    throw "生成物がx64 PEではありません。"
}

$exports = (& dumpbin.exe /exports $ModulePath | Out-String)
foreach ($name in @("GetScriptModuleTable", "InitializePlugin", "UninitializePlugin")) {
    if ($exports -notmatch "\b$([regex]::Escape($name))\b") {
        throw "必須エクスポートがありません: $name"
    }
}

Write-Host "ABI check: OK"
Write-Host "x64 exports: GetScriptModuleTable / InitializePlugin / UninitializePlugin"
