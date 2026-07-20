[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$sourceRoot = $PSScriptRoot
$scriptFiles = @(
    (Join-Path -Path $sourceRoot -ChildPath "build.ps1"),
    (Join-Path -Path $sourceRoot -ChildPath "verify-exports.ps1"),
    (Join-Path -Path $sourceRoot -ChildPath "check-syntax.ps1")
)

foreach ($scriptFile in $scriptFiles) {
    $tokens = $null
    $parseErrors = $null
    [void][System.Management.Automation.Language.Parser]::ParseFile(
        $scriptFile,
        [ref]$tokens,
        [ref]$parseErrors
    )

    if (($null -ne $parseErrors) -and ($parseErrors.Count -gt 0)) {
        $details = $parseErrors | ForEach-Object {
            "{0}:{1}: {2}" -f $_.Extent.StartLineNumber, $_.Extent.StartColumnNumber, $_.Message
        }
        throw "PowerShell syntax error in $scriptFile`n$($details -join [Environment]::NewLine)"
    }
}

$manifestPath = Join-Path -Path $sourceRoot -ChildPath "Cargo.toml"
$manifestText = Get-Content -LiteralPath $manifestPath -Raw
if ($manifestText -match 'aviutl2\s*=.*default-features\s*=\s*false') {
    throw "Do not disable aviutl2 default features. aviutl2 0.40.0 module code also requires generic and filter."
}

$verifyScriptPath = Join-Path -Path $sourceRoot -ChildPath "verify-exports.ps1"
$verifyScriptText = Get-Content -LiteralPath $verifyScriptPath -Raw
if ($verifyScriptText -match 'dumpbin(?:\.exe)?') {
    throw "verify-exports.ps1 must not require dumpbin. Use the built-in PE parser."
}

cargo metadata --manifest-path $manifestPath --no-deps --format-version 1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "Cargo.toml validation failed with exit code $LASTEXITCODE."
}

Write-Host "Syntax check: OK"
Write-Host "PowerShell scripts: OK"
Write-Host "Cargo.toml: OK"
