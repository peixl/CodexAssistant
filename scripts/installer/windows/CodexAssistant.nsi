Unicode true
ManifestDPIAware true
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define ROOT "..\..\.."
!define MUTEX_NAME "Global\CodexAssistantInstaller"
!define APP_NAME "CodexAssistant"
!define PUBLISHER "IFQ.AI"
!define APP_URL "https://github.com/peixl/CodexAssistant"

Name "${APP_NAME}"
BrandingText "${APP_NAME} ${VERSION}"
OutFile "${ROOT}\dist\windows\${APP_NAME}-${VERSION}-windows-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"
InstallDirRegKey HKCU "Software\${APP_NAME}" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 64
SetDateSave on
AllowSkipFiles off

VIProductVersion "${VERSION}.0"
VIFileVersion    "${VERSION}.0"
VIAddVersionKey  "ProductName"     "${APP_NAME}"
VIAddVersionKey  "ProductVersion"  "${VERSION}"
VIAddVersionKey  "FileVersion"     "${VERSION}"
VIAddVersionKey  "CompanyName"     "${PUBLISHER}"
VIAddVersionKey  "LegalCopyright"  "© ${PUBLISHER}"
VIAddVersionKey  "FileDescription" "${APP_NAME} Setup"
VIAddVersionKey  "OriginalFilename" "${APP_NAME}-${VERSION}-windows-x64-setup.exe"

!define MUI_ICON "${ROOT}\apps\codex-assistant-manager\src-tauri\icons\icon.ico"
!define MUI_UNICON "${ROOT}\apps\codex-assistant-manager\src-tauri\icons\icon.ico"
!define MUI_ABORTWARNING

; Remember the chosen UI language across runs
!define MUI_LANGDLL_REGISTRY_ROOT      HKCU
!define MUI_LANGDLL_REGISTRY_KEY       "Software\${APP_NAME}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "InstallerLanguage"

; Offer to run the manager after install
!define MUI_FINISHPAGE_RUN "$INSTDIR\codex-assistant-manager.exe"
!define MUI_FINISHPAGE_RUN_TEXT "启动 CodexAssistant 管理工具"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

; -----------------------------------------------------------------------------
; Helper: kill running app instances and wait for handles to release
; -----------------------------------------------------------------------------
!macro KillRunningInstances
  nsExec::Exec 'taskkill /IM codex-assistant.exe /F'
  Pop $0
  nsExec::Exec 'taskkill /IM codex-assistant-manager.exe /F'
  Pop $0
  ; Give the OS time to release the executable handles before we touch the files
  Sleep 1500
!macroend

Function .onInit
  ; Single-instance guard: refuse to launch a second installer at the same time
  System::Call 'kernel32::CreateMutexW(i 0, i 1, w "${MUTEX_NAME}") i .r0 ?e'
  Pop $1
  ${If} $1 = 183  ; ERROR_ALREADY_EXISTS
    MessageBox MB_OK|MB_ICONEXCLAMATION "${APP_NAME} 安装程序已在运行。"
    Abort
  ${EndIf}
  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd

Function un.onInit
  !insertmacro MUI_UNGETLANGUAGE
FunctionEnd

Section "Install"
  SetOutPath "$INSTDIR"
  SetOverwrite on

  !insertmacro KillRunningInstances

  ; Delete first so File can write even if a stale handle lingers
  Delete "$INSTDIR\codex-assistant.exe"
  Delete "$INSTDIR\codex-assistant-manager.exe"

  File "${ROOT}\dist\windows\app\codex-assistant.exe"
  File "${ROOT}\dist\windows\app\codex-assistant-manager.exe"

  ; Clean up legacy GBK-mangled shortcut filenames from older installs
  Delete "$DESKTOP\CodexAssistant 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\CodexAssistant 绠＄悊宸ュ叿.lnk"

  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\codex-assistant.exe" "" "$INSTDIR\codex-assistant.exe"
  CreateShortcut "$DESKTOP\${APP_NAME} 管理工具.lnk" "$INSTDIR\codex-assistant-manager.exe" "" "$INSTDIR\codex-assistant-manager.exe"
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\codex-assistant.exe" "" "$INSTDIR\codex-assistant.exe"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME} 管理工具.lnk" "$INSTDIR\codex-assistant-manager.exe" "" "$INSTDIR\codex-assistant-manager.exe"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\卸载 ${APP_NAME}.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Compute installed size (in KB) for Programs & Features
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2

  WriteRegStr HKCU "Software\${APP_NAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\${APP_NAME}" "Version"    "${VERSION}"

  !define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
  WriteRegStr   HKCU "${UNINST_KEY}" "DisplayName"     "${APP_NAME}"
  WriteRegStr   HKCU "${UNINST_KEY}" "DisplayVersion"  "${VERSION}"
  WriteRegStr   HKCU "${UNINST_KEY}" "Publisher"       "${PUBLISHER}"
  WriteRegStr   HKCU "${UNINST_KEY}" "DisplayIcon"     "$INSTDIR\codex-assistant-manager.exe"
  WriteRegStr   HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr   HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr   HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr   HKCU "${UNINST_KEY}" "URLInfoAbout"    "${APP_URL}"
  WriteRegStr   HKCU "${UNINST_KEY}" "HelpLink"        "${APP_URL}"
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize"   "$0"
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify"        1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair"        1
SectionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /IM codex-assistant.exe /F'
  Pop $0
  nsExec::Exec 'taskkill /IM codex-assistant-manager.exe /F'
  Pop $0
  Sleep 1000

  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$DESKTOP\${APP_NAME} 管理工具.lnk"
  Delete "$DESKTOP\CodexAssistant 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME} 管理工具.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\CodexAssistant 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\卸载 ${APP_NAME}.lnk"
  RMDir  "$SMPROGRAMS\${APP_NAME}"

  Delete "$INSTDIR\codex-assistant.exe"
  Delete "$INSTDIR\codex-assistant-manager.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir  "$INSTDIR"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
  DeleteRegKey HKCU "Software\${APP_NAME}"
SectionEnd
