; Per-user, offline Windows x64 installer.
; Input is a strictly staged directory, never the source tree or user data.
#ifndef AppVersion
  #define AppVersion "0.6.2"
#endif
#ifndef ReleaseLabel
  #define ReleaseLabel "0.6.2"
#endif
#ifndef StageDir
  #define StageDir "..\dist\portable\OhMemoJang"
#endif
#ifndef PackageOutputDir
  #define PackageOutputDir "..\dist"
#endif

[Setup]
AppId={{36497726-D8EF-4514-9BF2-51367779F831}
AppName=OhMemoJang
AppVersion={#AppVersion}
AppVerName=OhMemoJang {#ReleaseLabel}
AppPublisher=OhMemoJang
DefaultDirName={localappdata}\Programs\OhMemoJang
DefaultGroupName=OhMemoJang
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#PackageOutputDir}
OutputBaseFilename=OhMemoJang-{#ReleaseLabel}-Windows-x64-Setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\OhMemoJang.exe
UninstallDisplayName=OhMemoJang
VersionInfoVersion={#AppVersion}
InfoBeforeFile={#StageDir}\README.txt
CloseApplications=force
RestartApplications=no
SetupMutex=OhMemoJangPreviewInstaller

[Languages]
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "바탕화면 바로가기 만들기"; GroupDescription: "바로가기:"; Flags: unchecked

[Files]
Source: "{#StageDir}\OhMemoJang.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\README.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\SHA256SUMS.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\OhMemoJang\OhMemoJang"; Filename: "{app}\OhMemoJang.exe"; WorkingDir: "{app}"
Name: "{userdesktop}\OhMemoJang"; Filename: "{app}\OhMemoJang.exe"; WorkingDir: "{app}"; Tasks: desktopicon

; Nothing is written to the registry at install time. The program itself adds
; the "start with Windows" value when the user turns that setting on, so
; uninstalling removes it (ValueType none + uninsdeletevalue creates nothing).
[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "OhMemoJang"; Flags: dontcreatekey uninsdeletevalue

; The program keeps running in the tray: stop it before its files are removed.
; Memos are saved within two seconds of every change, so nothing is lost.
[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/im OhMemoJang.exe /f"; Flags: runhidden; RunOnceId: "StopOhMemoJang"

; No [Run], download, firewall, or security-policy actions.
; Inno Setup manages the current user's uninstall registration automatically.
