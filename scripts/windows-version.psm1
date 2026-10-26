function Test-FlowTorrentVersionInfo {
    param([Parameter(Mandatory = $true)]$Info, [Parameter(Mandatory = $true)][string]$Version)
    if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Expected numeric Cargo major.minor.patch version" }
    $numbers = @($Version.Split('.') | ForEach-Object { [int]$_ }) + @(0)
    $expected = $numbers -join '.'
    $fileNumbers = @($Info.FileMajorPart, $Info.FileMinorPart, $Info.FileBuildPart, $Info.FilePrivatePart) -join '.'
    $productNumbers = @($Info.ProductMajorPart, $Info.ProductMinorPart, $Info.ProductBuildPart, $Info.ProductPrivatePart) -join '.'
    if ($fileNumbers -ne $expected -or $productNumbers -ne $expected) {
        throw "Numeric executable version does not match Cargo: product=$productNumbers, file=$fileNumbers, expected=$expected"
    }
    # Inno reserves fixed-width fields and pads the textual values on the right.
    $productText = ([string]$Info.ProductVersion).TrimEnd()
    $fileText = ([string]$Info.FileVersion).TrimEnd()
    $productName = ([string]$Info.ProductName).TrimEnd()
    if ($productText -ne $Version -or $fileText -ne "$Version.0") {
        throw "Executable version does not match Cargo: product=$($Info.ProductVersion), file=$($Info.FileVersion)"
    }
    if ($productName -ne "FlowTorrent") { throw "Unexpected executable product identity" }
}
Export-ModuleMember -Function Test-FlowTorrentVersionInfo
