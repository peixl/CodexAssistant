Unicode true
!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define ROOT "..\..\.."

Name "CodexAssistant"
OutFile "${ROOT}\dist\windows\CodexAssistant-${VERSION}-windows-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\CodexAssistant"
InstallDirRegKey HKCU "Software\CodexAssistant" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "${ROOT}\apps\codex-plus-manager\src-tauri\icons\icon.ico"
!define MUI_UNICON "${ROOT}\apps\codex-plus-manager\src-tauri\icons\icon.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"

  nsExec::ExecToLog 'taskkill /IM codex-plus-plus.exe /F'
  Pop $0
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus-manager.exe /F'
  Pop $0
  ; Give Windows time to release the executable handle before File copy
  Sleep 1500

  File "${ROOT}\dist\windows\app\codex-plus-plus.exe"
  File "${ROOT}\dist\windows\app\codex-plus-plus-manager.exe"

  Delete "$DESKTOP\CodexAssistant 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\CodexAssistant\CodexAssistant 绠＄悊宸ュ叿.lnk"

  CreateShortcut "$DESKTOP\CodexAssistant.lnk" "$INSTDIR\codex-plus-plus.exe" "" "$INSTDIR\codex-plus-plus.exe"
  CreateShortcut "$DESKTOP\CodexAssistant 管理工具.lnk" "$INSTDIR\codex-plus-plus-manager.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"
  CreateDirectory "$SMPROGRAMS\CodexAssistant"
  CreateShortcut "$SMPROGRAMS\CodexAssistant\CodexAssistant.lnk" "$INSTDIR\codex-plus-plus.exe" "" "$INSTDIR\codex-plus-plus.exe"
  CreateShortcut "$SMPROGRAMS\CodexAssistant\CodexAssistant 管理工具.lnk" "$INSTDIR\codex-plus-plus-manager.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"
  CreateShortcut "$SMPROGRAMS\CodexAssistant\卸载 CodexAssistant.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\CodexAssistant" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexAssistant" "DisplayName" "CodexAssistant"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexAssistant" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexAssistant" "Publisher" "IFQ.AI"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexAssistant" "DisplayIcon" "$INSTDIR\codex-plus-plus-manager.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexAssistant" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexAssistant" "UninstallString" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus.exe /F'
  Pop $0
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus-manager.exe /F'
  Pop $0

  Delete "$DESKTOP\CodexAssistant.lnk"
  Delete "$DESKTOP\CodexAssistant 管理工具.lnk"
  Delete "$DESKTOP\CodexAssistant 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\CodexAssistant\CodexAssistant.lnk"
  Delete "$SMPROGRAMS\CodexAssistant\CodexAssistant 管理工具.lnk"
  Delete "$SMPROGRAMS\CodexAssistant\CodexAssistant 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\CodexAssistant\卸载 CodexAssistant.lnk"
  RMDir "$SMPROGRAMS\CodexAssistant"

  Delete "$INSTDIR\codex-plus-plus.exe"
  Delete "$INSTDIR\codex-plus-plus-manager.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexAssistant"
  DeleteRegKey HKCU "Software\CodexAssistant"
SectionEnd
