; Local test installer. No publication, default-app takeover, or user-data cleanup.
#ifndef AppVersion
  #error AppVersion must come from Cargo metadata
#endif
#ifndef PayloadDir
  #error PayloadDir must contain the validated package payload
#endif
#ifndef ArtifactDir
  #error ArtifactDir is required
#endif
#define AppIdentity "com.engus.flowtorrent"

[Setup]
AppId={#AppIdentity}
AppName=FlowTorrent
AppVersion={#AppVersion}
VersionInfoVersion={#AppVersion}.0
VersionInfoProductVersion={#AppVersion}.0
VersionInfoProductTextVersion={#AppVersion}
AppVerName=FlowTorrent {#AppVersion} (local test build)
AppPublisher=Engus
AppPublisherURL=https://github.com/engus/FlowTorrent
DefaultDirName={localappdata}\Programs\FlowTorrent
DefaultGroupName=FlowTorrent
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.19045
ChangesAssociations=yes
UsePreviousAppDir=yes
CloseApplications=yes
RestartApplications=no
OutputDir={#ArtifactDir}
OutputBaseFilename=FlowTorrent-{#AppVersion}-local-test-windows-x64-setup
SetupIconFile={#PayloadDir}\flow.ico
UninstallDisplayIcon={app}\flowtorrent.exe
LicenseFile={#PayloadDir}\LICENSE
InfoBeforeFile={#PayloadDir}\LOCAL_BUILD.txt
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"; InfoBeforeFile: "{#PayloadDir}\LOCAL_BUILD.ru.txt"

[Files]
Source: "{#PayloadDir}\flowtorrent.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\flow.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\Nunito-OFL.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\LOCAL_BUILD.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\LOCAL_BUILD.ru.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\INNO_SETUP_LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\ThirdParty\*"; DestDir: "{app}\ThirdParty"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\FlowTorrent"; Filename: "{app}\flowtorrent.exe"; AppUserModelID: "{#AppIdentity}"

[Registry]
Root: HKCU; Subkey: "Software\Classes\FlowTorrent.Torrent"; ValueType: string; ValueData: "BitTorrent metainfo"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\FlowTorrent.Torrent\DefaultIcon"; ValueType: string; ValueData: """{app}\flowtorrent.exe"",0"
Root: HKCU; Subkey: "Software\Classes\FlowTorrent.Torrent\shell\open\command"; ValueType: string; ValueData: """{app}\flowtorrent.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\.torrent\OpenWithProgids"; ValueType: string; ValueName: "FlowTorrent.Torrent"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Engus\FlowTorrent\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "FlowTorrent"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Engus\FlowTorrent\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "BitTorrent client"
Root: HKCU; Subkey: "Software\Engus\FlowTorrent\Capabilities\FileAssociations"; ValueType: string; ValueName: ".torrent"; ValueData: "FlowTorrent.Torrent"
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "FlowTorrent"; ValueData: "Software\Engus\FlowTorrent\Capabilities"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\flowtorrent.exe"; ValueType: string; ValueData: "{app}\flowtorrent.exe"; Flags: uninsdeletekey
