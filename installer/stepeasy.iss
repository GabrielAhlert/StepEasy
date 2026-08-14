; Instalador do StepEasy (Inno Setup 6).
;
; A versão vem de fora, para o instalador nunca discordar da tag:
;
;     ISCC.exe /DMyAppVersion=0.1.0 installer\stepeasy.iss
;
; Instala por usuário, em %LOCALAPPDATA%\Programs — sem UAC, sem precisar de
; administrador. É a escolha certa para uma ferramenta de mesa: quem grava um
; passo a passo raramente tem direito de instalar em Arquivos de Programas, e
; pedir elevação afasta justamente esse público.

#define MyAppName "StepEasy"
#define MyAppPublisher "Gabriel Ahlert"
#define MyAppURL "https://github.com/GabrielAhlert/StepEasy"
#define MyAppExeName "stepeasy.exe"

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
; Só dígitos e pontos: o campo de versão do executável não aceita sufixos como
; "-rc1", e o build quebraria numa tag de pré-lançamento.
#ifndef MyAppVersionNumeric
  #define MyAppVersionNumeric MyAppVersion
#endif

[Setup]
; Nunca mude este AppId: é por ele que o Windows reconhece uma instalação
; anterior e faz a atualização no lugar de duplicar a entrada.
AppId={{5E872EEF-7389-4A37-9230-A0C298CAD23E}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
VersionInfoVersion={#MyAppVersionNumeric}

DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=stepeasy-{#MyAppVersion}-setup
SetupIconFile=..\assets\icons\stepeasy.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName} {#MyAppVersion}

PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associar"; Description: "Abrir arquivos .stepeasy com o {#MyAppName}"; GroupDescription: "Integração com o Windows"

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; HKA cai em HKCU numa instalação por usuário, que é sempre o caso aqui.
Root: HKA; Subkey: "Software\Classes\.stepeasy"; ValueType: string; ValueName: ""; ValueData: "StepEasy.Gravacao"; Flags: uninsdeletevalue; Tasks: associar
Root: HKA; Subkey: "Software\Classes\StepEasy.Gravacao"; ValueType: string; ValueName: ""; ValueData: "Gravação do StepEasy"; Flags: uninsdeletekey; Tasks: associar
Root: HKA; Subkey: "Software\Classes\StepEasy.Gravacao\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"; Tasks: associar
Root: HKA; Subkey: "Software\Classes\StepEasy.Gravacao\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: associar
; Faz o arquivo aparecer no "Abrir com" mesmo sem ser o padrão.
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; O aplicativo guarda tema, escopo e rascunhos de recuperação aqui. Some junto
; com a desinstalação para não deixar lixo no perfil.
Type: filesandordirs; Name: "{userappdata}\stepeasy"
