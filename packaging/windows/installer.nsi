; Rustfin - Windows installer
; Keep this file UTF-8 *with* BOM, otherwise NSIS misreads umlauts. (NSIS, Modern UI 2)
; Built by build-installer.sh:
;   makensis -DVERSION=x.y.z -DSTAGE=<staged dir> -DOUTFILE=<setup.exe> installer.nsi
;
; Installs per user (no administrator rights needed) into
; %LOCALAPPDATA%\Programs\Rustfin.

Unicode true
SetCompressor /SOLID lzma

!ifndef VERSION
  !error "VERSION not defined"
!endif
!ifndef STAGE
  !error "STAGE not defined"
!endif
!ifndef OUTFILE
  !define OUTFILE "Rustfin-${VERSION}-setup.exe"
!endif

!define APPNAME "Rustfin"
!define APPID "io.github.chrisby4563.Rustfin"
!define PUBLISHER "chrisby4563"
!define URL "https://github.com/chrisby4563/rustfin"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPID}"

Name "${APPNAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${APPNAME}"
InstallDirRegKey HKCU "Software\${APPNAME}" "InstallDir"
RequestExecutionLevel user
ManifestDPIAware true
BrandingText "${APPNAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "${APPNAME}"
VIAddVersionKey /LANG=1033 "FileDescription" "${APPNAME} Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "GPL-3.0-only"

!include "MUI2.nsh"
!include "FileFunc.nsh"

!define MUI_ICON "${STAGE}\rustfin.ico"
!define MUI_UNICON "${STAGE}\rustfin.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\bin\rustfin.exe"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${STAGE}\LICENSE.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "English"

LangString SecApp ${LANG_GERMAN} "Rustfin"
LangString SecApp ${LANG_ENGLISH} "Rustfin"
LangString SecDesktop ${LANG_GERMAN} "Desktop-Verknüpfung"
LangString SecDesktop ${LANG_ENGLISH} "Desktop shortcut"
LangString AppRunning ${LANG_GERMAN} "Rustfin läuft noch. Bitte schließe Rustfin und klicke auf OK."
LangString WarmUp ${LANG_GERMAN} "Rustfin wird für den ersten Start vorbereitet …"
LangString WarmUp ${LANG_ENGLISH} "Preparing Rustfin for its first start …"
LangString AppRunning ${LANG_ENGLISH} "Rustfin is still running. Please close it and click OK."

Function .onInit
  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd

; Abort if rustfin.exe is still running (files would be locked).
!macro CheckRunning
  check_running:
    nsExec::ExecToStack 'cmd /c tasklist /FI "IMAGENAME eq rustfin.exe" /NH | find /I "rustfin.exe"'
    Pop $0
    Pop $1
    StrCmp $0 "0" 0 +3
      MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION "$(AppRunning)" IDOK check_running
      Abort
!macroend

Section "$(SecApp)" SecMain
  SectionIn RO
  !insertmacro CheckRunning

  ; Replace an older installation completely (stale DLLs would be loaded).
  RMDir /r "$INSTDIR\bin"
  RMDir /r "$INSTDIR\lib"
  RMDir /r "$INSTDIR\share"

  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*.*"

  ; Start once without a window: Windows checks the new DLLs now (~15 s on
  ; the first start otherwise).
  DetailPrint "$(WarmUp)"
  nsExec::Exec '"$INSTDIR\bin\rustfin.exe" --warm-up'
  Pop $0

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\${APPNAME}" "InstallDir" "$INSTDIR"

  CreateShortcut "$SMPROGRAMS\${APPNAME}.lnk" "$INSTDIR\bin\rustfin.exe" "" "$INSTDIR\rustfin.ico"

  ; Apps & features entry
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINSTKEY}" "URLInfoAbout" "${URL}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\rustfin.ico"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" "$0"
SectionEnd

Section "$(SecDesktop)" SecDesktopLink
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\bin\rustfin.exe" "" "$INSTDIR\rustfin.ico"
SectionEnd

Section "Uninstall"
  !insertmacro CheckRunning
  Delete "$SMPROGRAMS\${APPNAME}.lnk"
  Delete "$DESKTOP\${APPNAME}.lnk"
  RMDir /r "$INSTDIR\bin"
  RMDir /r "$INSTDIR\lib"
  RMDir /r "$INSTDIR\share"
  Delete "$INSTDIR\rustfin.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegKey HKCU "Software\${APPNAME}"
  ; User settings in %APPDATA%\rustfin are kept on purpose.
SectionEnd

Function un.onInit
  !insertmacro MUI_UNGETLANGUAGE
FunctionEnd
