; Hooks for Tauri's Windows installer (ADR-0041). Tauri reads this before its pages, so the first page's settings
; defined here go to the Welcome page: it gets a "Start Scrappy with Windows" box, checked. The installer only hands
; the choice to the app, which saves it as its preference and owns the start entry (ADR-0036).

!include LogicLib.nsh
!include nsDialogs.nsh

; Where the choice waits for the app; `scrappy_core::app::take_installer_choice` reads and removes it.
!define SCRAPPY_CHOICE_KEY "Software\com.nyaridev.scrappy"
!define SCRAPPY_CHOICE_VALUE "StartWithSystem"

Var ScrappyStartBox
; Empty until the Welcome page shows: passive updates and silent installs skip it, and hand nothing over.
Var ScrappyStartWithSystem

; The Welcome page: the box under its text, checked unless the user unchecked it before going back.
!define MUI_PAGE_CUSTOMFUNCTION_SHOW ScrappyWelcomeShow
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE ScrappyWelcomeLeave

Function ScrappyWelcomeShow
  ${NSD_CreateCheckbox} 120u 172u 195u 12u "Start Scrappy with Windows"
  Pop $ScrappyStartBox
  SetCtlColors $ScrappyStartBox "" "FFFFFF"
  ${If} $ScrappyStartWithSystem != ${BST_UNCHECKED}
    ${NSD_Check} $ScrappyStartBox
  ${EndIf}
FunctionEnd

Function ScrappyWelcomeLeave
  ${NSD_GetState} $ScrappyStartBox $ScrappyStartWithSystem
FunctionEnd

!macro NSIS_HOOK_POSTINSTALL
  ${If} $ScrappyStartWithSystem == ${BST_CHECKED}
    WriteRegDWORD HKCU "${SCRAPPY_CHOICE_KEY}" "${SCRAPPY_CHOICE_VALUE}" 1
  ${ElseIf} $ScrappyStartWithSystem == ${BST_UNCHECKED}
    WriteRegDWORD HKCU "${SCRAPPY_CHOICE_KEY}" "${SCRAPPY_CHOICE_VALUE}" 0
  ${EndIf}
!macroend

; A real uninstall removes the start entry the app made; an update keeps it.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "${SCRAPPY_CHOICE_KEY}" "${SCRAPPY_CHOICE_VALUE}"
  ${EndIf}
!macroend
