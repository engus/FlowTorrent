param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = "Stop"
$info = (Get-Item -LiteralPath $Path).VersionInfo
$numbers = @($Version.Split('.') | ForEach-Object { [int]$_ }) + @(0)
if ($numbers.Count -ne 4) { throw "Expected numeric Cargo major.minor.patch version" }
$expected = $numbers -join '.'
$fileNumbers = @($info.FileMajorPart, $info.FileMinorPart, $info.FileBuildPart, $info.FilePrivatePart) -join '.'
$productNumbers = @($info.ProductMajorPart, $info.ProductMinorPart, $info.ProductBuildPart, $info.ProductPrivatePart) -join '.'
if ($fileNumbers -ne $expected -or $productNumbers -ne $expected) {
    throw "Numeric executable version does not match Cargo: product=$productNumbers, file=$fileNumbers, expected=$expected"
}
if ($info.ProductVersion -ne $Version -or $info.FileVersion -ne "$Version.0") {
    throw "Executable version does not match Cargo: product=$($info.ProductVersion), file=$($info.FileVersion)"
}
if ($info.ProductName -ne "FlowTorrent") { throw "Unexpected executable product identity" }
$info | Select-Object FileVersion, ProductVersion, ProductName, FileMajorPart, FileMinorPart, FileBuildPart, FilePrivatePart, ProductMajorPart, ProductMinorPart, ProductBuildPart, ProductPrivatePart | ConvertTo-Json
