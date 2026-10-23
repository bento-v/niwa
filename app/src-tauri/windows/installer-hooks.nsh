; Niwa's steps around Tauri's NSIS installer: the Strict service.
;
; The service (niwa-service.exe, installed beside niwa-app.exe) runs as
; SYSTEM. That is why Niwa installs for all users into Program Files: only
; administrators can change the program the service runs.
;
; `niwa-service install | uninstall` exit with 3 when they refuse because a
; Strict session is running: a session can't be escaped by uninstalling or
; updating Niwa.

!macro NIWA_SERVICE command
  ; nsExec runs it without flashing a console window.
  nsExec::ExecToStack '"$INSTDIR\niwa-service.exe" ${command}'
  Pop $0 ; the exit code, or "error" if it couldn't be run
  Pop $1 ; what it printed
!macroend

!macro NIWA_IS_OPEN
  ; $0 is 0 while a niwa-app.exe is running.
  nsExec::ExecToStack 'cmd.exe /c tasklist /FI "IMAGENAME eq niwa-app.exe" /NH | find /I "niwa-app.exe"'
  Pop $0
  Pop $1
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; Close a running Niwa first, by asking it: it quits as its tray menu's
  ; Quit does, keeping where its windows were. The finish page's "Run Niwa"
  ; opens it again (and `/R` does for updates). If it's still open a few
  ; seconds later (mid-session, when it won't quit, or a version from before
  ; `--quit`), the installer's own check after this asks whether to close it.
  !insertmacro NIWA_IS_OPEN
  ${If} $0 = 0
  ${AndIf} ${FileExists} "$INSTDIR\niwa-app.exe"
    Exec '"$INSTDIR\niwa-app.exe" --quit'
    StrCpy $2 0
    niwa_wait_for_quit:
      Sleep 250
      IntOp $2 $2 + 1
      !insertmacro NIWA_IS_OPEN
      ${If} $0 = 0
      ${AndIf} $2 < 20
        Goto niwa_wait_for_quit
      ${EndIf}
  ${EndIf}

  ; Installing over an earlier copy: stop and remove its service so the
  ; file can be replaced. It is set up again once the files are in.
  ${If} ${FileExists} "$INSTDIR\niwa-service.exe"
    !insertmacro NIWA_SERVICE "uninstall"
    ${If} $0 == 3
      MessageBox MB_ICONSTOP|MB_OK "A Strict focus session is running. Niwa can be updated once it ends." /SD IDOK
      SetErrorLevel 3
      Abort
    ${ElseIf} $0 != 0
      MessageBox MB_ICONSTOP|MB_OK "Niwa's background service couldn't be stopped for the update:$\r$\n$\r$\n$1" /SD IDOK
      SetErrorLevel 2
      Abort
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro NIWA_SERVICE "install"
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "Niwa is installed, but Strict mode couldn't be set up:$\r$\n$\r$\n$1$\r$\n$\r$\nGentle sessions work as usual. Running this installer again may fix it." /SD IDOK
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} ${FileExists} "$INSTDIR\niwa-service.exe"
    !insertmacro NIWA_SERVICE "uninstall"
    ${If} $0 == 3
      MessageBox MB_ICONSTOP|MB_OK "A Strict focus session is running. Niwa can be removed once it ends." /SD IDOK
      SetErrorLevel 3
      Abort
    ${ElseIf} $0 != 0
      MessageBox MB_ICONSTOP|MB_OK "Niwa's background service couldn't be removed:$\r$\n$\r$\n$1" /SD IDOK
      SetErrorLevel 2
      Abort
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; "Delete the application data" also takes Niwa's own folder: the garden,
  ; stats, block lists and settings.
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    RMDir /r "$LOCALAPPDATA\Niwa"
  ${EndIf}
!macroend
