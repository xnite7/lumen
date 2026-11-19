; couch's Windows installer (Inno Setup). Installs for the current user, no
; admin needed, into %LOCALAPPDATA%\Programs\couch (so couch can update itself),
; with Start menu + desktop shortcuts and an uninstaller.
#ifndef Version
  #define Version "0.0.0"
#endif

[Setup]
AppId={{6E4C9C1B-6B1D-4C55-9D8E-6D69A7C0C0C7}
AppName=midnite couch
AppVersion={#Version}
AppPublisher=midnite
DefaultDirName={localappdata}\Programs\couch
DefaultGroupName=couch
DisableProgramGroupPage=yes
DisableDirPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
OutputDir=..\..
OutputBaseFilename=couch-setup
SetupIconFile=couch.ico
UninstallDisplayIcon={app}\couch.exe
UninstallDisplayName=midnite couch
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes

[Tasks]
Name: "desktopicon"; Description: "a desktop shortcut"; GroupDescription: "shortcuts:"

[Files]
Source: "..\..\dist\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\couch"; Filename: "{app}\couch.exe"
Name: "{userdesktop}\couch"; Filename: "{app}\couch.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\couch.exe"; Description: "open couch"; Flags: nowait postinstall skipifsilent
