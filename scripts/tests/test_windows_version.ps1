$ErrorActionPreference = "Stop"
Import-Module "$PSScriptRoot/../windows-version.psm1" -Force
function New-VersionFixture([string]$Padding = "") {
    [pscustomobject]@{
        FileVersion = "0.1.0.0$Padding"; ProductVersion = "0.1.0$Padding"; ProductName = "FlowTorrent$Padding"
        FileMajorPart = 0; FileMinorPart = 1; FileBuildPart = 0; FilePrivatePart = 0
        ProductMajorPart = 0; ProductMinorPart = 1; ProductBuildPart = 0; ProductPrivatePart = 0
    }
}
function Assert-Rejected($Info, [string]$Pattern) {
    try {
        Test-FlowTorrentVersionInfo -Info $Info -Version "0.1.0"
        throw "Invalid metadata was accepted"
    } catch {
        if ($_.Exception.Message -notmatch $Pattern) { throw }
    }
}
Test-FlowTorrentVersionInfo -Info (New-VersionFixture) -Version "0.1.0"
Test-FlowTorrentVersionInfo -Info (New-VersionFixture "                ") -Version "0.1.0"
$bad = New-VersionFixture; $bad.ProductMinorPart = 2
Assert-Rejected $bad "Numeric executable version"
$bad = New-VersionFixture; $bad.ProductVersion = "0.1.0-next"
Assert-Rejected $bad "Executable version does not match"
$bad = New-VersionFixture; $bad.FileVersion = " 0.1.0.0"
Assert-Rejected $bad "Executable version does not match"
$bad = New-VersionFixture; $bad.ProductName = "Another application"
Assert-Rejected $bad "Unexpected executable product identity"
Write-Output "Windows version metadata: padded/unpadded accepted; wrong numeric/text/identity rejected"
