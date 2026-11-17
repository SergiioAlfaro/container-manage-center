# Validación manual de Container Manage Center 1.9.1 en Windows

La versión de Windows se ha compilado y sus pruebas unitarias pasan con Wine,
pero **nunca se ha ejecutado en un Windows real**. Esta guía es la primera
prueba. Hazla en un equipo o una máquina virtual de pruebas, no en el equipo
con tus datos de trabajo, y apunta cualquier fallo tal cual (captura y texto
del error). Los identificadores (`WIN-…`) son los de la matriz de pruebas de
`AUDITORIA-WINDOWS-1.9.1.md`: al terminar, cambia allí su estado de
NOT TESTED a PASS o FAIL.

> **Licencia**: el instalador incluye el cargador de WebView2 de Microsoft,
> cubierto por el permiso adicional de la GPL v3 (`docs/legal/WEBVIEW2_EXCEPTION.md`,
> aprobado el 2026-11-17). La versión de Windows sigue **en pruebas** hasta
> completar esta guía.

Tiempo estimado: 60–90 minutos.

## 0. Preparación

- [ ] Windows 10 22H2 o Windows 11 23H2 o posterior, x86_64, Pro, Enterprise o
      Education (lo que pide Docker Desktop), con tu usuario normal.
- [ ] Docker Desktop instalado y en marcha, con el motor de Linux.
      `docker version` en PowerShell responde.
- [ ] Cliente OpenSSH: `ssh -V` en PowerShell responde.
- [ ] Opcional: un servidor Linux con Docker accesible por SSH (con clave y,
      si puedes, otro con contraseña).
- [ ] Una carpeta de pruebas, por ejemplo `C:\cmc-pruebas`, con un
      `compose.yaml` sencillo:

      ```yaml
      services:
        web:
          image: nginx:alpine
          ports: ["8081:80"]
      ```

- [ ] Comprueba el instalador:
      `Get-FileHash .\<instalador>.exe -Algorithm SHA256` coincide con `SHA256SUMS`.

## 1. Instalación

| ID | Paso | Resultado esperado |
|---|---|---|
| WIN-INS-01 | Ejecuta el instalador (SmartScreen avisará: no está firmado; «Más información › Ejecutar de todas formas») | En tu idioma (inglés, español, francés, italiano o portugués; si no, inglés), muestra la licencia GPL-3.0 y **no pide permisos de administrador** |
| WIN-INS-02 | Termina la instalación | Acceso «Container Manage Center» en el menú Inicio, con el icono morado |
| WIN-INS-03 | `dir "$env:LOCALAPPDATA\Container Manage Center"` | `container-manage-center.exe`, `legal\` (LICENSE.txt, THIRD_PARTY_NOTICES.md, WEBVIEW2_EXCEPTION.md…), `docs\`; con el instalador de MinGW, también `WebView2Loader.dll` |
| WIN-INS-04 | Abre la aplicación | **Una ventana** (sin consola negra detrás), tema morado, en tu idioma |
| WIN-INS-05 | Configuración › Sistema | Sistema «Windows 10/11 … (versión)», versión 10.0.x, «Aplicación de Windows», WebView2 con su versión; sin glibc ni escritorio de Linux |
| WIN-INS-06 | Cierra la aplicación. Clic derecho en su acceso directo › **Ejecutar como administrador** y acepta el aviso de UAC | Aviso «Container Manage Center no debe ejecutarse como administrador» y la aplicación se cierra sin abrir la ventana. Abierta después de forma normal, funciona (W-18) |

## 2. Docker local (Docker Desktop)

| ID | Paso | Resultado esperado |
|---|---|---|
| WIN-DOC-01 | Panel de inicio | Conectado a Docker Desktop; versión del motor correcta |
| WIN-DOC-02 | Contenedores: crea `nginx:alpine` con el puerto 8080, inícialo, mira sus registros y estadísticas, reinícialo, detenlo y bórralo | Todo funciona y aparece en el panel de actividad |
| WIN-DOC-03 | En ese contenedor, **Abrir en el navegador** (puerto publicado) | Se abre el navegador predeterminado en `http://localhost:8080` |
| WIN-DOC-04 | Terminal del contenedor (`sh`): `ls`, `echo ñandú`, cambia el tamaño de la ventana | Responde, muestra la `ñ` y se adapta al tamaño |
| WIN-DOC-05 | Crea un contenedor con una **carpeta vinculada** `C:\cmc-pruebas` → `/datos` (selector de carpetas) | Se crea; dentro, `ls /datos` muestra `compose.yaml` |
| WIN-DOC-06 | Crea un contenedor que monte `C:\Users\<tú>` | Aviso de riesgo alto (credenciales) antes de crear |
| WIN-DOC-07 | Edita el contenedor de WIN-DOC-05 (por ejemplo, cambia la política de reinicio) | Conserva la carpeta vinculada `C:\cmc-pruebas` |
| WIN-DOC-08 | Imágenes, redes y volúmenes: lista, crea y borra uno de cada | Correcto |
| WIN-DOC-09 | Imágenes › Construir desde la carpeta `C:\cmc-pruebas` con un `Dockerfile` (`FROM alpine`) | Se construye la imagen |
| WIN-DOC-10 | Compose: abre `C:\cmc-pruebas\compose.yaml`, levántalo, mira los registros y bájalo | Correcto |
| WIN-DOC-11 | Compose: levanta un proyecto con una imagen grande que no tengas y pulsa **Detener** mientras descarga | Se detiene en pocos segundos; en el Administrador de tareas no queda ningún `docker-compose.exe` |
| WIN-DOC-12 | Compose › Nuevo proyecto en `C:\cmc-pruebas\nuevo` | Crea `compose.yaml` |
| WIN-DOC-13 | Intenta abrir un archivo Compose dentro de `%APPDATA%` o de `C:\Windows` | Se rechaza (carpeta de configuración o del sistema) |
| WIN-DOC-14 | Cierra Docker Desktop y vuelve a la aplicación | Diagnóstico «Docker Desktop no está en marcha» con la solución de Windows (sin `systemctl` ni `sudo`) |
| WIN-DOC-15 | Vuelve a abrir Docker Desktop | La aplicación se reconecta sola o con «Reintentar» |
| WIN-DOC-16 | Configuración › Docker › ruta personalizada `\\.\pipe\docker_engine` | Se acepta y conecta |
| WIN-DOC-17 | Configuración › Componentes | Textos de Windows (Docker Desktop, OpenSSH, cómo desinstalar), no de apt ni systemd |

## 3. Servidores remotos por SSH

| ID | Paso | Resultado esperado |
|---|---|---|
| WIN-SSH-01 | Añade un servidor con clave elegida con el **selector** (`C:\Users\<tú>\.ssh\id_ed25519`) | Se acepta la ruta; pide confirmar la huella la primera vez; conecta |
| WIN-SSH-02 | Lista contenedores del servidor y abre los registros de uno | Correcto; sin ventanas de consola que aparecen y desaparecen |
| WIN-SSH-03 | Servidor con contraseña o clave con frase de paso, marcando «Recordar» | Conecta; en el **Administrador de credenciales › Credenciales de Windows** hay una entrada `container-manage-center/<servidor>/…` |
| WIN-SSH-04 | Cierra y abre la aplicación; conecta de nuevo | No vuelve a pedir la contraseña |
| WIN-SSH-05 | Terminal del servidor | Se abre y responde (sin cambio de tamaño: limitación conocida) |
| WIN-SSH-06 | Un servidor con un nombre que no existe | Diagnóstico con `Resolve-DnsName …` y `ping -n 3 …` (PowerShell), no `getent` |
| WIN-SSH-07 | Un servidor con ProxyJump | Mensaje claro de que ProxyJump no está disponible en Windows |
| WIN-SSH-08 | Cambia la clave del servidor (o usa otro con la misma dirección) | Se detecta el cambio de huella y no se conecta |

## 4. Copias, tareas programadas, notificaciones y registros

| ID | Paso | Resultado esperado |
|---|---|---|
| WIN-BAK-01 | Haz una copia de un volumen a `C:\cmc-pruebas\copias` | Copia correcta; **Comprobar** la da por buena (sin archivos «de más») |
| WIN-BAK-02 | Restaura esa copia en un volumen nuevo | Correcto |
| WIN-BAK-03 | Intenta usar `C:\Windows\Temp` como destino de copias | Se rechaza (carpeta del sistema) |
| WIN-BAK-04 | Crea una tarea programada (cada día) | En el **Programador de tareas**, carpeta `\Container Manage Center\`, aparece la tarea con su descripción |
| WIN-BAK-05 | En el Programador de tareas, «Ejecutar» esa tarea | La copia se hace y aparece en el historial de copias de la aplicación |
| WIN-BAK-06 | Desactívala y bórrala desde la aplicación | Cambia y desaparece también en el Programador de tareas |
| WIN-MON-01 | Monitorización: una alerta fácil de disparar (un contenedor que se detiene) | Notificación de Windows con el nombre «Container Manage Center» |
| WIN-ACT-01 | Panel de actividad › Vaciar; Eventos › Vaciar el diario | Se vacían sin error |

## 5. Ayuda, legal, actualización y desinstalación

| ID | Paso | Resultado esperado |
|---|---|---|
| WIN-LEG-01 | Ayuda › «Windows» | Sección presente y en tu idioma |
| WIN-LEG-02 | Configuración › Legal | GPL-3.0-or-later, copyright de Sergio Alfaro Lorite, ruta de los avisos de terceros |
| WIN-UPD-01 | Configuración › Actualizaciones | Instrucciones de Windows (descargar y ejecutar el instalador nuevo), no `apt` |
| WIN-UPD-02 | Con una tarea programada creada, instala de nuevo el mismo instalador (o uno más nuevo) | Se actualiza; la tarea programada **sigue** en el Programador de tareas; tus datos siguen |
| WIN-UNI-01 | Configuración de Windows › Aplicaciones › «Container Manage Center» › Desinstalar | Se desinstala; desaparece del menú Inicio |
| WIN-UNI-02 | Programador de tareas, carpeta `\Container Manage Center\` | Ya no están las tareas de la aplicación |
| WIN-UNI-03 | `dir "$env:APPDATA\container-manage-center"`, `dir "$env:LOCALAPPDATA\container-manage-center"`, `C:\cmc-pruebas\copias` | Tus datos y tus copias siguen ahí; en el Administrador de credenciales siguen las contraseñas guardadas (bórralas a mano si quieres) |

## Hoja de resultados

| Bloque | Correcto | Fallos (ID y texto) |
|---|---|---|
| 1. Instalación | | |
| 2. Docker local | | |
| 3. SSH | | |
| 4. Copias, tareas, notificaciones, registros | | |
| 5. Ayuda, legal, actualización, desinstalación | | |
