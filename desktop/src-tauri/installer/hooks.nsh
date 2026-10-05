; An update retains task consent. A standalone uninstall must finish owned cleanup
; before removing the executable. The CLI never opens a WebView or migrates data.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    nsExec::ExecToStack '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-cleanup'
    Pop $0
    Pop $1
    ${If} $0 != 0
      SetErrorLevel 1
      Abort "Unable to clean up this installation's background tasks. Close LLM Usage and retry."
    ${EndIf}
  ${EndIf}
!macroend
