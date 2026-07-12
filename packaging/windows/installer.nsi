; NSIS installer for gibbon (system-tray player for internet radio streams).
; Build with:  makensis -DVERSION=0.1.0 -DSRCEXE=target\release\gibbon.exe packaging\windows\installer.nsi

!ifndef VERSION
  !define VERSION "0.3.0"
!endif
!ifndef SRCEXE
  !define SRCEXE "..\..\target\release\gibbon.exe"
!endif

!include "MUI2.nsh"

Name "Gibbon"
OutFile "gibbon-setup-${VERSION}.exe"
Unicode True
InstallDir "$PROGRAMFILES64\Gibbon"
InstallDirRegKey HKLM "Software\Gibbon" "InstallDir"
RequestExecutionLevel admin

!define MUI_ICON "..\..\assets\icons\gibbon.ico"
!define MUI_UNICON "..\..\assets\icons\gibbon.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Gibbon" SecMain
  SetOutPath "$INSTDIR"
  File "/oname=gibbon.exe" "${SRCEXE}"
  File "/oname=gibbon.ico" "..\..\assets\icons\gibbon.ico"

  CreateDirectory "$SMPROGRAMS\Gibbon"
  CreateShortcut "$SMPROGRAMS\Gibbon\Gibbon.lnk" "$INSTDIR\gibbon.exe" "" "$INSTDIR\gibbon.ico"

  WriteRegStr HKLM "Software\Gibbon" "InstallDir" "$INSTDIR"

  ; Add/Remove Programs entry.
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Gibbon" \
    "DisplayName" "Gibbon"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Gibbon" \
    "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Gibbon" \
    "DisplayIcon" '"$INSTDIR\gibbon.ico"'
  ; Quoted: $INSTDIR defaults to Program Files, which contains a space.
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Gibbon" \
    "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Gibbon" \
    "URLInfoAbout" "https://github.com/samuelb/gibbon"
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\gibbon.exe"
  Delete "$INSTDIR\gibbon.ico"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Gibbon\Gibbon.lnk"
  RMDir "$SMPROGRAMS\Gibbon"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Gibbon"
  DeleteRegKey HKLM "Software\Gibbon"
  ; Remove the "start on login" entry the app may have created for this user.
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Gibbon"
SectionEnd
