param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$Version,
    [switch]$DisposableProfile
)
$ErrorActionPreference = "Stop"
if (-not $DisposableProfile -or $env:GITHUB_ACTIONS -ne "true" -or -not $env:RUNNER_TEMP) {
    throw "This test changes HKCU and application-data fixtures. Run only on a disposable GitHub Windows runner with -DisposableProfile."
}
$Installer = (Resolve-Path -LiteralPath $Installer).Path
& "$PSScriptRoot/../verify-windows-binary.ps1" -Path $Installer -Version $Version
$stateRoot = Join-Path $env:LOCALAPPDATA "engus/flowtorrent"
$configRoot = Join-Path $env:APPDATA "engus/flowtorrent"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\com.engus.flowtorrent_is1"
$shortcut = Join-Path ([Environment]::GetFolderPath("Programs")) "FlowTorrent/FlowTorrent.lnk"
foreach ($existing in @($stateRoot, $configRoot, $uninstallKey, $shortcut)) {
    if (Test-Path -LiteralPath $existing) { throw "Disposable profile contains existing FlowTorrent state: $existing" }
}
$id = [Guid]::NewGuid().ToString("N")
$root = Join-Path $env:RUNNER_TEMP "FlowTorrent installer test $id"
$app = Join-Path $root "Installed app"
$payloadRoot = Join-Path ([Environment]::GetFolderPath("UserProfile")) "Downloads/FlowTorrent-installer-test-$id"
$association = "HKCU:\Software\Classes\.torrent"
$oldDefault = if (Test-Path -LiteralPath $association) { (Get-Item -LiteralPath $association).GetValue("") } else { $null }
$testDefault = "FlowTorrent.InstallTest.$id"
New-Item -ItemType Directory -Force -Path $root, "$stateRoot/data", "$configRoot/config", $payloadRoot | Out-Null
$session = "$stateRoot/data/session.json"
$settings = "$configRoot/config/settings.json"
$payload = Join-Path $payloadRoot "keep.bin"
[IO.File]::WriteAllText($session, "session sentinel $id")
[IO.File]::WriteAllText($settings, "settings sentinel $id")
[IO.File]::WriteAllBytes($payload, [byte[]](1, 3, 5, 7))
$expected = @{}
foreach ($file in @($session, $settings, $payload)) { $expected[$file] = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash }
New-Item -Path $association -Force | Out-Null
Set-Item -LiteralPath $association -Value $testDefault
function Assert-Preserved {
    foreach ($file in $expected.Keys) {
        if (-not (Test-Path -LiteralPath $file) -or (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $expected[$file]) {
            throw "Installer changed session/settings/payload fixture: $file"
        }
    }
    if ((Get-Item -LiteralPath $association).GetValue("") -ne $testDefault) { throw "Installer changed the default torrent handler" }
}
try {
    foreach ($pass in @("install", "upgrade")) {
        $result = Start-Process -FilePath $Installer -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/DIR=`"$app`"", "/LOG=`"$root/$pass.log`"") -Wait -PassThru
        if ($result.ExitCode -ne 0) { throw "$pass failed: $($result.ExitCode)" }
        & "$PSScriptRoot/../verify-windows-binary.ps1" -Path "$app/flowtorrent.exe" -Version $Version
        Assert-Preserved
    }
    $command = (Get-Item -LiteralPath "HKCU:\Software\Classes\FlowTorrent.Torrent\shell\open\command").GetValue("")
    if ($command -ne "`"$app\flowtorrent.exe`" `"%1`"") { throw "File-open command is not safely quoted: $command" }
    if (-not (Test-Path -LiteralPath $shortcut)) { throw "Start menu shortcut is missing" }
    $shell = New-Object -ComObject WScript.Shell
    if ($shell.CreateShortcut($shortcut).TargetPath -ne "$app\flowtorrent.exe") { throw "Shortcut points to another executable" }
    $desktop = New-Object -ComObject Shell.Application
    $item = $desktop.NameSpace((Split-Path $shortcut)).ParseName((Split-Path $shortcut -Leaf))
    if ($item.ExtendedProperty("System.AppUserModel.ID") -ne "com.engus.flowtorrent") { throw "Notification identity is missing from shortcut" }
    [IO.File]::WriteAllText("$app/keep-user-file.txt", "user file inside install directory")
    $result = Start-Process -FilePath "$app/unins000.exe" -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART") -Wait -PassThru
    if ($result.ExitCode -ne 0) { throw "Uninstall failed: $($result.ExitCode)" }
    Assert-Preserved
    if (-not (Test-Path -LiteralPath "$app/keep-user-file.txt")) { throw "Uninstaller deleted an unowned file" }
    if (Test-Path -LiteralPath "$app/flowtorrent.exe") { throw "Installed executable remains after uninstall" }
    if (Test-Path -LiteralPath "HKCU:\Software\Classes\FlowTorrent.Torrent") { throw "Owned ProgID remains after uninstall" }
    $evidence = @{ installer_test = "passed"; version = $Version; host = [Environment]::OSVersion.Version.ToString(); checks = @("per-user install", "same-version reinstall", "quoted Open With command", "default handler preserved", "shortcut AppUserModelID", "state/payload hashes preserved", "unowned file preserved on uninstall"); physical_windows_10_11_validation = $false }
    $evidence | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path (Split-Path $Installer) "installer-smoke.json") -Encoding utf8
} finally {
    if (Test-Path -LiteralPath "$app/unins000.exe") {
        Start-Process -FilePath "$app/unins000.exe" -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART") -Wait | Out-Null
    }
    if ($null -eq $oldDefault) {
        # Registry provider Get-Item returns a read-only handle.
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Software\Classes\.torrent", $true)
        try { $key.DeleteValue("", $false) } finally { $key.Dispose() }
    }
    else { Set-Item -LiteralPath $association -Value $oldDefault }
    # These roots were absent at the preflight and created by this disposable test.
    Remove-Item -LiteralPath $stateRoot, $configRoot, $payloadRoot, $root -Recurse -Force -ErrorAction SilentlyContinue
}
Get-Content -LiteralPath (Join-Path (Split-Path $Installer) "installer-smoke.json")
