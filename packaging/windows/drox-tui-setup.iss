; Drox TUI — installateur Windows (Inno Setup 6)
; Compile via packaging/build-and-pack.ps1

#ifndef MyAppVersion
  #define MyAppVersion "2.0.2"
#endif

#define MyAppName "Drox TUI"
#define MyAppPublisher "Drox"
#define MyAppURL "https://github.com/DroxKiwi/Drox---TUI---OR"
#define MyAppExeName "drox-tui.exe"

[Setup]
AppId={{8F4A2C15-3D5E-4F90-9ABC-020012345678}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\Programs\DroxTUI
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\..\dist
OutputBaseFilename=drox-tui-{#MyAppVersion}-windows-x64-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\bin\{#MyAppExeName}
LicenseFile=..\..\packaging\LICENSE-MIT.txt
InfoBeforeFile=..\..\packaging\README-INSTALL.txt

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "addpath"; Description: "Ajouter Drox TUI au PATH utilisateur"; GroupDescription: "Options:"; Flags: checkedonce

[Files]
Source: "..\..\dist\stage-windows\bin\{#MyAppExeName}"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "..\..\dist\stage-windows\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\dist\stage-windows\VERSION"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\bin\{#MyAppExeName}"; Parameters: "--workspace ""{userdocs}"""
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}\bin"; Tasks: addpath; Check: NeedsAddPath(ExpandConstant('{app}\bin'))

[Code]
function NeedsAddPath(Param: string): Boolean;
var
  OrigPath: string;
begin
  if not WizardIsTaskSelected('addpath') then
  begin
    Result := False;
    Exit;
  end;
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    Result := True;
    Exit;
  end;
  Result := Pos(';' + Param + ';', ';' + OrigPath + ';') = 0;
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('addpath') then
      MsgBox('Installation terminee.' + #13#10 + #13#10 +
        'Ouvrez un nouveau terminal puis lancez :' + #13#10 +
        '  drox-tui --workspace C:\chemin\vers\projet' + #13#10 + #13#10 +
        'Prerequis : Ollama sur http://127.0.0.1:11434',
        mbInformation, MB_OK);
  end;
end;
