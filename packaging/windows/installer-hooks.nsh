; SPDX-License-Identifier: GPL-3.0-or-later
; SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
;
; Ganchos del instalador NSIS de Windows (1.9.1).
;
; Al desinstalar (no al actualizar a otra versión), quita del Programador de
; tareas las copias programadas propias de Container Manage Center: si no, se
; quedarían apuntando a un programa que ya no existe. Los datos, la
; configuración y las copias hechas se conservan, como en Linux.

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    nsExec::Exec '"$INSTDIR\${MAINBINARYNAME}.exe" --remove-scheduled-tasks'
    Pop $0
  ${EndIf}
!macroend
