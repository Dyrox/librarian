; Inno Setup script for the Windows installer.
; Built by CI:  iscc /DAppVersion=0.1.7 /DSourceExe=..\..\target\release\librarian.exe librarian.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\..\target\release\librarian.exe"
#endif

[Setup]
AppId={{8C3E7B52-5D1A-4F0E-9A7C-2B6F1D4E9A31}
AppName=Librarian
AppVersion={#AppVersion}
AppVerName=Librarian {#AppVersion}
AppPublisher=Dyrox
AppPublisherURL=https://github.com/Dyrox/librarian
DefaultDirName={autopf}\Librarian
DefaultGroupName=Librarian
DisableProgramGroupPage=yes
; Installs for the current user without admin rights; the user can still
; choose "install for all users" in the dialog.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
OutputDir=..\..\dist
OutputBaseFilename=Librarian-{#AppVersion}-windows-x64-setup
SetupIconFile=..\..\assets\icon.ico
UninstallDisplayIcon={app}\Librarian.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "Librarian.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Librarian"; Filename: "{app}\Librarian.exe"
Name: "{autodesktop}\Librarian"; Filename: "{app}\Librarian.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Librarian.exe"; Description: "{cm:LaunchProgram,Librarian}"; Flags: nowait postinstall skipifsilent
