# Validación manual de Container Manage Center 1.7.0 (Debian 13)

Solo lo nuevo de la 1.7.0: nombre, paquete, actualización, licencia y
copyright. Las fases anteriores ya están validadas y **no se repiten**.

Tiempo estimado: 30–40 minutos. Con tu usuario normal; `sudo` solo donde se
indica (apt). Nada de esto modifica Docker ni tus contenedores.

## 0. Antes de empezar

- [ ] La **1.6.0** está instalada y la has usado: tienes al menos un servidor
      guardado, alguna copia hecha y, si las usas, una tarea de copia
      programada y la monitorización activada.
- [ ] Anota lo que hay ahora (para comparar después):
      ```bash
      dpkg -l docker-control-center | tail -1
      ls -la ~/.config/docker-control-center ~/.local/state/docker-control-center
      find ~/.config/docker-control-center ~/.local/state/docker-control-center -type f -exec sha256sum {} + | sort -k2 > /tmp/cmc-antes.txt
      systemctl --user list-timers 'dcc-backup-*' --all
      ```
- [ ] `bash scripts/verify-evolution-1.7.0.sh --deb --net` en el código de la
      1.7.0 termina con «parte automática superada».
- [ ] Tienes los archivos de la entrega en una carpeta: los dos `.deb`,
      `install.sh`, `SHA256SUMS` y el `.tar.gz`.

## 1. Distribución (antes de instalar)

| # | Paso | Resultado esperado |
|---|---|---|
| D1 | `sha256sum -c SHA256SUMS` | Todos «La suma coincide» / «OK» |
| D2 | `dpkg-deb -f container-manage-center_1.7.0_amd64.deb Package Version Conflicts Replaces Provides` | `container-manage-center`, `1.7.0` y `docker-control-center` en Conflicts, Replaces y Provides |
| D3 | `dpkg-deb -c container-manage-center_1.7.0_amd64.deb \| grep usr/share/doc` | `copyright`, `LICENSE`, `THIRD_PARTY_NOTICES.md`, `LICENSE_OVERVIEW.md`, `COPYRIGHT.md`, `DISTRIBUTION_COMPLIANCE.md`, `MIGRATION.md`, README, INSTALL, USER_GUIDE, changelog |
| D4 | `tar -tzf container-manage-center-1.7.0-src.tar.gz \| head` y `tar -tzf … \| grep -Ei '\.(pem\|key\|env)$\|id_rsa\|id_ed25519'` | Árbol del proyecto bajo `container-manage-center-1.7.0/`; la segunda orden no muestra nada |

## 2. Instalación y actualización desde la 1.6.0

| # | Paso | Resultado esperado |
|---|---|---|
| U1 | Cierra la aplicación. `bash install.sh` (o `sudo apt install ./container-manage-center_1.7.0_amd64.deb`) | El instalador dice que sustituye a Docker Control Center 1.6.0; apt muestra que **quita** `docker-control-center` e **instala** `container-manage-center`, y nada más |
| U2 | `dpkg -l 'docker-control-center' 'container-manage-center' 2>/dev/null \| grep ^ii` | Solo `container-manage-center 1.7.0` |
| U3 | `ls -l /usr/bin/container-manage-center /usr/bin/docker-control-center` y `cat /usr/bin/docker-control-center` | Los dos existen; el segundo es un script corto que hace `exec /usr/bin/container-manage-center "$@"` |
| U4 | Menú de aplicaciones: busca «container», «docker» y «cmc» | Aparece **un solo** lanzador, «Container Manage Center», con el icono morado. No queda «Docker Control Center» |

## 3. Identidad

| # | Paso | Resultado esperado |
|---|---|---|
| I1 | Abre la aplicación desde el menú | Título de la ventana «Container Manage Center»; mismo diseño y color morado |
| I2 | Recorre la barra lateral (Contenedores, Imágenes, Compose, Auditoría, Copias, Monitorización…) | Todo funciona igual que en la 1.6.0 |
| I3 | Configuración › Sistema | Versión 1.7.0; «Paquete .deb»: 1.7.0 |
| I4 | Cambia el idioma a inglés y a otro más, y vuelve | El nombre es «Container Manage Center» en todos |
| I5 | Si tienes alertas de monitorización: provoca una (o usa «Probar notificación») | La notificación de escritorio aparece como «Container Manage Center» |
| I6 | Abre `seahorse` (Contraseñas y claves) si guardaste una contraseña de servidor | La entrada existente sigue ahí; las nuevas se etiquetan «Container Manage Center» |

## 4. Migración y persistencia

| # | Paso | Resultado esperado |
|---|---|---|
| M1 | `find ~/.config/docker-control-center ~/.local/state/docker-control-center -type f -exec sha256sum {} + \| sort -k2 \| diff /tmp/cmc-antes.txt -` **antes** de abrir la aplicación (o justo tras U2) | Sin diferencias: la instalación no ha tocado tus datos |
| M2 | En la aplicación: Servidores | Los mismos servidores; se conectan como antes (la identidad SSH no se vuelve a preguntar) |
| M3 | Copias › historial | Las mismas copias; «Comprobar» en una de ellas: correcta |
| M4 | Monitorización | Sigue activada en los mismos servidores, con el historial anterior |
| M5 | Panel de actividad › historial | Las entradas de la 1.6.0 siguen ahí |
| M6 | Si tenías una tarea programada: `systemctl --user list-timers 'dcc-backup-*'` y, tras su hora (o `systemctl --user start dcc-backup-<id>.service`), `journalctl --user -u 'dcc-backup-*' -n 20` | El temporizador sigue activo; la ejecución termina bien aunque su unidad llame a `/usr/bin/docker-control-center` |
| M7 | Edita esa tarea en la aplicación y guárdala (con tu autorización); `systemctl --user cat dcc-backup-<id>.service` | La unidad nueva ejecuta `/usr/bin/container-manage-center` |
| M8 | Contraseña guardada de un servidor | Conecta sin volver a escribirla |

## 5. Legal

| # | Paso | Resultado esperado |
|---|---|---|
| L1 | Configuración › **Legal** | «Container Manage Center (CMC)», versión 1.7.0, `GPL-3.0-or-later`, «Copyright © 2026 Sergio Alfaro Lorite», la declaración GPL (sin garantía), la URL https://www.gnu.org/licenses/gpl-3.0.html, la ruta de `THIRD_PARTY_NOTICES.md` y el aviso de independencia de Docker, Inc. |
| L2 | Pulsa «Copiar» junto a la URL y pégala en el navegador | Abre la GPL v3 en gnu.org. La aplicación no abre nada por sí sola |
| L3 | Ayuda › sección **28. Legal** (y en inglés, «28. Legal») | Los mismos datos, con el nombre anterior y las rutas de `/usr/share/doc/container-manage-center/` |
| L4 | `less /usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md` | Índice de componentes y textos completos de licencia (MIT, Apache-2.0, MPL-2.0, OFL-1.1…) con los avisos de sus autores |
| L5 | `cat /usr/share/doc/container-manage-center/copyright \| head -20` | Formato DEP-5, `Copyright: 2026 Sergio Alfaro Lorite`, `License: GPL-3.0-or-later` |

## 6. Desinstalación y vuelta atrás (opcional, al final)

| # | Paso | Resultado esperado |
|---|---|---|
| R1 | Configuración › Desinstalación y componentes › Analizar | El paquete de la aplicación es `container-manage-center 1.7.0`; las dependencias que propone quitar son las mismas que con la 1.6.0 (el cambio de nombre no cuenta como desinstalación) |
| R2 | `sudo apt install ./docker-control-center_1.6.0_amd64.deb` (vuelta a la 1.6.0) y abrirla | apt quita la 1.7.0 y pone la 1.6.0; la 1.6.0 encuentra todos los datos |
| R3 | Vuelve a instalar la 1.7.0 (U1) | Igual que la primera vez |

## Hoja de resultados

Copia esta tabla, marca cada fila y envíala con `verify-evolution-1.7.0-report.txt`.

| Apartado | Pasos | Resultado (OK / fallo + nota) |
|---|---|---|
| Distribución | D1–D4 | |
| Instalación y actualización | U1–U4 | |
| Identidad | I1–I6 | |
| Migración y persistencia | M1–M8 | |
| Legal | L1–L5 | |
| Desinstalación y vuelta atrás (opcional) | R1–R3 | |
