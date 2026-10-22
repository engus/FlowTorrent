param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = "Stop"
Import-Module "$PSScriptRoot/windows-version.psm1" -Force
$info = (Get-Item -LiteralPath $Path).VersionInfo
Test-FlowTorrentVersionInfo -Info $info -Version $Version
$info | Select-Object FileVersion, ProductVersion, ProductName, FileMajorPart, FileMinorPart, FileBuildPart, FilePrivatePart, ProductMajorPart, ProductMinorPart, ProductBuildPart, ProductPrivatePart | ConvertTo-Json
