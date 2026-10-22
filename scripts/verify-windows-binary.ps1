param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = "Stop"
$info = (Get-Item -LiteralPath $Path).VersionInfo
if ($info.ProductVersion -ne $Version -or $info.FileVersion -ne "$Version.0") {
    throw "Executable version does not match Cargo: product=$($info.ProductVersion), file=$($info.FileVersion)"
}
if ($info.ProductName -ne "FlowTorrent") { throw "Unexpected executable product identity" }
$info | Select-Object FileVersion, ProductVersion, ProductName | ConvertTo-Json
