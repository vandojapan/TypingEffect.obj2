[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$ModulePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Read-U16 {
    param(
        [byte[]]$Bytes,
        [int]$Offset
    )

    if (($Offset -lt 0) -or (($Offset + 2) -gt $Bytes.Length)) {
        throw "Unexpected end of file while reading a 16-bit value."
    }
    return [BitConverter]::ToUInt16($Bytes, $Offset)
}

function Read-U32 {
    param(
        [byte[]]$Bytes,
        [int]$Offset
    )

    if (($Offset -lt 0) -or (($Offset + 4) -gt $Bytes.Length)) {
        throw "Unexpected end of file while reading a 32-bit value."
    }
    return [BitConverter]::ToUInt32($Bytes, $Offset)
}

function Read-AsciiZ {
    param(
        [byte[]]$Bytes,
        [int]$Offset
    )

    if (($Offset -lt 0) -or ($Offset -ge $Bytes.Length)) {
        throw "An export name points outside the file."
    }

    $end = $Offset
    while (($end -lt $Bytes.Length) -and ($Bytes[$end] -ne 0)) {
        $end++
    }
    if ($end -ge $Bytes.Length) {
        throw "An export name is not null terminated."
    }

    return [Text.Encoding]::ASCII.GetString($Bytes, $Offset, $end - $Offset)
}

function Convert-RvaToFileOffset {
    param(
        [UInt32]$Rva,
        [object[]]$Sections
    )

    foreach ($section in $Sections) {
        $start = [UInt64]$section.VirtualAddress
        $span = [Math]::Max([UInt64]$section.VirtualSize, [UInt64]$section.RawSize)
        $end = $start + $span
        $value = [UInt64]$Rva

        if (($value -ge $start) -and ($value -lt $end)) {
            return [Int64]$section.RawPointer + ([Int64]$Rva - [Int64]$section.VirtualAddress)
        }
    }

    throw ("RVA 0x{0:X8} does not belong to a PE section." -f $Rva)
}

if (-not (Test-Path -LiteralPath $ModulePath -PathType Leaf)) {
    throw "The mod2 file was not found: $ModulePath"
}

$resolvedPath = (Resolve-Path -LiteralPath $ModulePath).Path
$bytes = [IO.File]::ReadAllBytes($resolvedPath)
if ($bytes.Length -lt 64) {
    throw "The output file is too small to be a PE binary."
}
if ((Read-U16 -Bytes $bytes -Offset 0) -ne 0x5A4D) {
    throw "The output file does not have an MZ header."
}

$peOffset = [int](Read-U32 -Bytes $bytes -Offset 0x3C)
if (($peOffset -lt 0) -or (($peOffset + 24) -gt $bytes.Length)) {
    throw "The PE header offset is invalid."
}
if ((Read-U32 -Bytes $bytes -Offset $peOffset) -ne 0x00004550) {
    throw "The output file does not have a PE signature."
}

$machine = Read-U16 -Bytes $bytes -Offset ($peOffset + 4)
if ($machine -ne 0x8664) {
    throw ("The output file is not an x64 PE binary. Machine=0x{0:X4}" -f $machine)
}

$numberOfSections = [int](Read-U16 -Bytes $bytes -Offset ($peOffset + 6))
$sizeOfOptionalHeader = [int](Read-U16 -Bytes $bytes -Offset ($peOffset + 20))
$optionalHeaderOffset = $peOffset + 24
if (($optionalHeaderOffset + $sizeOfOptionalHeader) -gt $bytes.Length) {
    throw "The optional header extends beyond the end of the file."
}
if ((Read-U16 -Bytes $bytes -Offset $optionalHeaderOffset) -ne 0x020B) {
    throw "The output file is not a PE32+ binary."
}

$exportDirectoryEntryOffset = $optionalHeaderOffset + 112
if (($exportDirectoryEntryOffset + 8) -gt ($optionalHeaderOffset + $sizeOfOptionalHeader)) {
    throw "The PE optional header does not contain an export data directory."
}

$exportRva = Read-U32 -Bytes $bytes -Offset $exportDirectoryEntryOffset
$exportSize = Read-U32 -Bytes $bytes -Offset ($exportDirectoryEntryOffset + 4)
if (($exportRva -eq 0) -or ($exportSize -eq 0)) {
    throw "The output file does not contain an export table."
}

$sectionTableOffset = $optionalHeaderOffset + $sizeOfOptionalHeader
$sections = @()
for ($i = 0; $i -lt $numberOfSections; $i++) {
    $sectionOffset = $sectionTableOffset + ($i * 40)
    if (($sectionOffset + 40) -gt $bytes.Length) {
        throw "The PE section table extends beyond the end of the file."
    }

    $sections += [PSCustomObject]@{
        VirtualSize = Read-U32 -Bytes $bytes -Offset ($sectionOffset + 8)
        VirtualAddress = Read-U32 -Bytes $bytes -Offset ($sectionOffset + 12)
        RawSize = Read-U32 -Bytes $bytes -Offset ($sectionOffset + 16)
        RawPointer = Read-U32 -Bytes $bytes -Offset ($sectionOffset + 20)
    }
}

$exportOffset = [int](Convert-RvaToFileOffset -Rva $exportRva -Sections $sections)
if (($exportOffset -lt 0) -or (($exportOffset + 40) -gt $bytes.Length)) {
    throw "The export directory points outside the file."
}

$numberOfNames = [int](Read-U32 -Bytes $bytes -Offset ($exportOffset + 24))
$addressOfNamesRva = Read-U32 -Bytes $bytes -Offset ($exportOffset + 32)
if (($numberOfNames -le 0) -or ($addressOfNamesRva -eq 0)) {
    throw "The export table does not contain named exports."
}

$nameTableOffset = [int](Convert-RvaToFileOffset -Rva $addressOfNamesRva -Sections $sections)
$exportNames = @()
for ($i = 0; $i -lt $numberOfNames; $i++) {
    $entryOffset = $nameTableOffset + ($i * 4)
    $nameRva = Read-U32 -Bytes $bytes -Offset $entryOffset
    $nameOffset = [int](Convert-RvaToFileOffset -Rva $nameRva -Sections $sections)
    $exportNames += Read-AsciiZ -Bytes $bytes -Offset $nameOffset
}

$requiredExports = @(
    "GetScriptModuleTable",
    "InitializePlugin",
    "UninitializePlugin"
)

foreach ($name in $requiredExports) {
    if ($exportNames -notcontains $name) {
        throw "A required export is missing: $name"
    }
}

Write-Host "ABI check: OK"
Write-Host "PE parser: built-in PowerShell implementation"
Write-Host "x64 exports: GetScriptModuleTable / InitializePlugin / UninitializePlugin"
