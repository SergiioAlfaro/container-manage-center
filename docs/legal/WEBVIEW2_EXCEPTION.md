# Permiso adicional de la GPL v3 para el cargador de WebView2 de Microsoft

> **Estado: aprobado por el titular del copyright, Sergio Alfaro Lorite, el
> 2026-11-17.** Lo redactó el asistente de desarrollo a petición del titular
> (decisión del 2026-11-16) y no es asesoramiento jurídico. Se aplica desde la
> versión 1.9.1.

## Por qué hace falta

En Windows, Tauri muestra la interfaz con Microsoft Edge WebView2. Para
localizarlo usa el **cargador de WebView2** (WebView2Loader), un componente de
Microsoft del *Microsoft Edge WebView2 SDK* que se distribuye solo en binario:

- en la compilación con MinGW, como `WebView2Loader.dll` junto al programa;
- en la compilación con MSVC, enlazado dentro del programa
  (`WebView2LoaderStatic.lib`).

Su licencia (BSD-3-Clause, © Microsoft Corporation) es compatible con la GPL,
pero Microsoft no publica su código fuente, y la GPL v3 pide que el «código
fuente correspondiente» de lo que se distribuye incluya el de las bibliotecas
con las que se enlaza, salvo las «bibliotecas del sistema». No está claro que
el cargador lo sea, porque se distribuye con el programa.

La sección 7 de la GPL v3 permite al titular del copyright añadir **permisos
adicionales** que eximan de alguna de sus condiciones. Este es el texto que se
añade, según el modelo de excepción de enlace que propone la Free Software
Foundation para bibliotecas no libres (preguntas frecuentes de la GPL, «How do
I add an exception…»). El texto en inglés es el que tiene efecto; la
traducción es informativa.

## Texto (en inglés, el que tiene efecto)

```text
Additional permission under GNU GPL version 3 section 7

If you modify this Program, or any covered work, by linking or combining it
with the Microsoft Edge WebView2 Loader (WebView2Loader.dll or
WebView2LoaderStatic.lib, part of the Microsoft Edge WebView2 SDK), or a
modified version of that library, containing parts covered by the terms of
the BSD 3-Clause License of Microsoft Corporation, the licensors of this
Program grant you additional permission to convey the resulting work.
Corresponding Source for a non-source form of such a combination shall
include the source code for the parts of this Program used as well as that of
the covered work, but need not include the source code of the Microsoft Edge
WebView2 Loader.
```

## Traducción (informativa)

> **Permiso adicional según la sección 7 de la GNU GPL versión 3**
>
> Si modificas este Programa, o cualquier obra cubierta, enlazándolo o
> combinándolo con el cargador de Microsoft Edge WebView2
> (WebView2Loader.dll o WebView2LoaderStatic.lib, parte del Microsoft Edge
> WebView2 SDK), o una versión modificada de esa biblioteca, que contiene
> partes cubiertas por la licencia BSD de 3 cláusulas de Microsoft
> Corporation, los licenciantes de este Programa te conceden permiso adicional
> para distribuir la obra resultante. El código fuente correspondiente de la
> forma no fuente de esa combinación incluirá el código fuente de las partes de
> este Programa que se usen y el de la obra cubierta, pero no tiene que incluir
> el código fuente del cargador de Microsoft Edge WebView2.

## Alcance

- Lo concede **Sergio Alfaro Lorite**, titular del copyright del código propio
  de Container Manage Center. Las dependencias de terceros que se enlazan con
  el programa tienen licencias permisivas (MIT, Apache-2.0, BSD, ISC…; ver
  `DEPENDENCY_LICENSE_AUDIT-windows.md`) y no necesitan este permiso.
- Solo afecta al cargador de WebView2. No cambia nada más de la GPL v3: el
  resto del código fuente correspondiente se sigue ofreciendo completo.
- Como permite la sección 7, quien redistribuya una versión modificada puede
  quitar este permiso de su versión.
- Las contribuciones aceptadas en el proyecto se aceptan con este permiso
  (`CONTRIBUTING.md`).

## Historial

- 2026-11-16: el titular decide añadir un permiso adicional; se redacta este
  texto.
- 2026-11-17: el titular lo aprueba. Se quita el estado de borrador de este
  documento y de `COPYRIGHT.md`, y los instaladores de Windows dejan de estar
  marcados como «solo pruebas» por motivos legales
  (`scripts/build-windows.sh`, `.github/workflows/windows-build.yml`,
  `DISTRIBUTION_COMPLIANCE.md`). La versión de Windows sigue **en pruebas**
  porque aún no se ha validado en un Windows real
  (`docs/validation/VALIDACION-WINDOWS.md`).
