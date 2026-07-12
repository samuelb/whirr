; NSIS installer for whirr (system-tray player for internet radio streams).
; Build with:  makensis -DVERSION=0.1.0 -DSRCEXE=target\release\whirr.exe packaging\windows\installer.nsi

!ifndef VERSION
  !define VERSION "0.4.0"
!endif
!ifndef SRCEXE
  !define SRCEXE "..\..\target\release\whirr.exe"
!endif

!include "MUI2.nsh"

Name "Whirr"
OutFile "whirr-setup-${VERSION}.exe"
Unicode True
InstallDir "$PROGRAMFILES64\Whirr"
InstallDirRegKey HKLM "Software\Whirr" "InstallDir"
RequestExecutionLevel admin

!define MUI_ICON "..\..\assets\icons\whirr.ico"
!define MUI_UNICON "..\..\assets\icons\whirr.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Whirr" SecMain
  SetOutPath "$INSTDIR"
  File "/oname=whirr.exe" "${SRCEXE}"
  File "/oname=whirr.ico" "..\..\assets\icons\whirr.ico"

  CreateDirectory "$SMPROGRAMS\Whirr"
  CreateShortcut "$SMPROGRAMS\Whirr\Whirr.lnk" "$INSTDIR\whirr.exe" "" "$INSTDIR\whirr.ico"

  WriteRegStr HKLM "Software\Whirr" "InstallDir" "$INSTDIR"

  ; Add/Remove Programs entry.
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Whirr" \
    "DisplayName" "Whirr"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Whirr" \
    "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Whirr" \
    "DisplayIcon" '"$INSTDIR\whirr.ico"'
  ; Quoted: $INSTDIR defaults to Program Files, which contains a space.
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Whirr" \
    "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Whirr" \
    "URLInfoAbout" "https://github.com/samuelb/whirr"
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\whirr.exe"
  Delete "$INSTDIR\whirr.ico"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Whirr\Whirr.lnk"
  RMDir "$SMPROGRAMS\Whirr"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Whirr"
  DeleteRegKey HKLM "Software\Whirr"
  ; Remove the "start on login" entry the app may have created for this user.
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Whirr"
SectionEnd
