param(
    [string]$Output = "target/test-packages/windows-x64",
    [Parameter(Mandatory = $true)][string]$Notices,
    [string]$Iscc = ""
)
$ErrorActionPreference = "Stop"
$arguments = @("$PSScriptRoot/package_windows.py", "--output", $Output, "--notices", $Notices)
if ($Iscc) { $arguments += @("--iscc", $Iscc) }
python @arguments
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
