# Validación manual de Container Manage Center 1.9.0 en Windows

La versión de Windows se ha compilado (MinGW desde Linux y, si lo lanzas, MSVC
en GitHub Actions), pero **nunca se ha ejecutado en un Windows real**. Esta
guía es la primera prueba. Hazla en un equipo o máquina virtual de pruebas,
no en el equipo con tus datos de trabajo, y apunta cualquier fallo tal cual
(captura y texto del error).

> **Recordatorio legal**: el instalador es **solo para pruebas**. No lo
> publiques ni lo pases a terceros hasta que decidas lo de WebView2Loader
> (`docs/legal/DEPENDENCY_LICENSE_AUDIT-windows.md`, «Decisión pendiente»).

Tiempo estimado: 45–60 minutos.

## 0. Preparación

- [ ] Windows 10 22H2 o Windows 11, x86_64, con tu usuario normal (sin ser
      administrador si puedes).
- [ ] Docker Desktop instalado y en marcha (motor de Linux); tu usuario en el
      grupo `docker-users`. `docker version` en PowerShell responde.
- [ ] Cliente OpenSSH: `ssh -V` en PowerShell responde.
- [ ] Opcional: un servidor Linux con Docker accesible por SSH (con clave y,
      si puedes, otro con contraseña).
- [ ] Comprueba el instalador: `Get-FileHash .\<instalador>.exe -Algorithm SHA256`
      coincide con `SHA256SUMS`.

## 1. Instalación

| # | Paso | Resultado esperado |
|---|---|---|
| I1 | Ejecuta el instalador (SmartScreen avisará: el instalador no está firmado; «Más información › Ejecutar de todas formas») | Instalador en tu idioma (inglés, español, francés, italiano o portugués; si no, inglés), muestra la licencia GPL-3.0 y **no pide permisos de administrador** |
| I2 | Termina la instalación | Acceso «Container Manage Center» en el menú Inicio (carpeta «Container Manage Center») |
| I3 | `dir "$env:LOCALAPPDATA\Container Manage Center"` | `container-manage-center.exe`, `legal\` (LICENSE.txt, THIRD_PARTY_NOTICES.md, WebView2Loader.LICENSE.txt…), `docs\`; con el instalador de MinGW, también `WebView2Loader.dll` |
| I4 | Abre la aplicación | Se abre **una ventana** (sin consola negra detrás), con el tema morado y tu idioma |

## 2. Docker local (Docker Desktop)

| # | Paso | Resultado esperado |
|---|---|---|
| D1 | Panel de inicio | Conectado a Docker Desktop; versión del motor correcta |
| D2 | Contenedores: crea uno (`nginx:alpine`, puerto 8080), inícialo, mira sus registros, ábrelo en el navegador, detenlo y bórralo | Todo funciona y aparece en el panel de actividad |
| D3 | Imágenes, redes y volúmenes: lista, crea y borra uno de cada | Correcto |
| D4 | Compose: abre un proyecto con un `compose.yaml` en una carpeta tuya, levántalo y bájalo | Correcto (necesita `docker compose` de Docker Desktop) |
| D5 | Cierra Docker Desktop y vuelve a la aplicación | Diagnóstico «Docker Desktop no está en marcha» con la solución para Windows (no los pasos de Linux) |
| D6 | Configuración › Componentes | Rutas y textos de Windows (Docker Desktop, OpenSSH), no de apt/systemd |

## 3. Servidores remotos por SSH

| # | Paso | Resultado esperado |
|---|---|---|
| S1 | Añade un servidor con clave (`C:\Users\<tú>\.ssh\id_ed25519`) | Pide confirmar la huella la primera vez; conecta |
| S2 | Lista contenedores del servidor y abre los registros de uno | Correcto; sin ventanas de consola que aparecen y desaparecen |
| S3 | Servidor con contraseña o clave con frase de paso, marcando «Recordar» | Conecta; en el **Administrador de credenciales › Credenciales de Windows** hay una entrada `container-manage-center/<servidor>/…` |
| S4 | Cierra y abre la aplicación; conecta de nuevo | No vuelve a pedir la contraseña |
| S5 | Terminal del servidor | Se abre y responde (sin cambio de tamaño automático: limitación conocida) |
| S6 | Un servidor con ProxyJump | Mensaje claro de que ProxyJump no está disponible en Windows |

## 4. Copias, tareas programadas y notificaciones

| # | Paso | Resultado esperado |
|---|---|---|
| C1 | Haz una copia de un volumen a una carpeta tuya | Copia correcta; «Comprobar» la da por buena |
| C2 | Restaura esa copia en un volumen nuevo | Correcto |
| C3 | Crea una tarea programada (cada día) | En el **Programador de tareas**, carpeta `\Container Manage Center\`, aparece la tarea con su descripción |
| C4 | En el Programador de tareas, «Ejecutar» esa tarea | La copia se hace y aparece en el historial de copias de la aplicación |
| C5 | Desactívala y bórrala desde la aplicación | Cambia y desaparece también en el Programador de tareas |
| C6 | Monitorización: activa una alerta fácil de disparar (p. ej. un contenedor que se detiene) | Notificación de Windows con el nombre «Container Manage Center» |

## 5. Ayuda, legal y desinstalación

| # | Paso | Resultado esperado |
|---|---|---|
| L1 | Ayuda › «Windows» | Sección presente y en tu idioma |
| L2 | Configuración › Legal | GPL-3.0-or-later, copyright de Sergio Alfaro Lorite, ruta de los avisos de terceros (copiable) |
| L3 | Configuración de Windows › Aplicaciones › «Container Manage Center» › Desinstalar | Se desinstala; desaparece del menú Inicio |
| L4 | `dir "$env:APPDATA\container-manage-center"`, `dir "$env:LOCALAPPDATA\container-manage-center"` | Tus datos siguen ahí (la desinstalación no los borra); las tareas programadas que no borraste siguen en el Programador de tareas |

## Hoja de resultados

| Bloque | Correcto | Fallos (número de paso y texto) |
|---|---|---|
| 1. Instalación | | |
| 2. Docker local | | |
| 3. SSH | | |
| 4. Copias, tareas, notificaciones | | |
| 5. Ayuda, legal, desinstalación | | |
