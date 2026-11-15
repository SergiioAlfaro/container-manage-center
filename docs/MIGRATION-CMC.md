# De Docker Control Center a Container Manage Center (1.7.0)

Desde la versión 1.7.0, **Docker Control Center** se llama **Container Manage
Center** (CMC). Es el mismo programa, con las mismas funciones; cambian el
nombre, el paquete y la licencia pasa de provisional a definitiva
(GPL-3.0-or-later).

## Qué hay que hacer

Nada especial: instala el paquete nuevo como cualquier actualización.

```bash
sudo apt install ./container-manage-center_1.7.0_amd64.deb
# o, en Debian 12 / Ubuntu 22.04:
sudo apt install ./container-manage-center_1.7.0_amd64-compat-debian12-ubuntu2204.deb
# o con el instalador:
./install.sh
```

apt sustituye el paquete `docker-control-center` por `container-manage-center`
en una sola operación (el nuevo declara `Conflicts`, `Replaces` y `Provides:
docker-control-center`). No hace falta desinstalar antes el anterior, y no
quedan dos programas ni dos lanzadores.

## Qué se conserva

Todo. El cambio de nombre **no mueve ni modifica ningún dato**:

| Dato | Dónde está (sin cambios) |
|---|---|
| Configuración, servidores, alertas, tareas de copia | `~/.config/docker-control-center/` |
| Historial de actividad | `~/.local/state/docker-control-center/` |
| Contraseñas y frases de paso guardadas | Almacén de contraseñas del escritorio (atributo `application=docker-control-center`) |
| Copias de seguridad | `~/Copias/docker-control-center-copias/` (o la carpeta que elegiste) |
| Tareas programadas (systemd de usuario) | `~/.config/systemd/user/dcc-backup-*` |
| Datos locales de la interfaz (WebKit) | Carpetas con el identificador `org.dockercontrolcenter.app` |
| Etiquetas en Docker | `dcc.*` |

Esos nombres internos se mantienen a propósito (decisión D-110): cambiarlos
obligaría a mover datos del usuario, a reescribir unidades de systemd y a
volver a guardar contraseñas, con riesgo de perder algo, sin ninguna ventaja
para quien usa la aplicación. Como no se mueve nada, **no hay migración que
pueda fallar a medias** ni que haya que deshacer.

## Qué cambia

| | Antes (≤ 1.6.0) | Ahora (1.7.0) |
|---|---|---|
| Nombre | Docker Control Center | Container Manage Center (CMC) |
| Paquete | `docker-control-center` | `container-manage-center` |
| Programa | `/usr/bin/docker-control-center` | `/usr/bin/container-manage-center` |
| Lanzador | `docker-control-center.desktop` | `container-manage-center.desktop` |
| Documentación | `/usr/share/doc/docker-control-center/` | `/usr/share/doc/container-manage-center/` |
| Licencia | GPL-3.0-or-later (provisional) | GPL-3.0-or-later, © 2026 Sergio Alfaro Lorite |
| Avisos de terceros | `THIRD-PARTY.md` (solo una tabla) | `THIRD_PARTY_NOTICES.md` (textos completos) |

`/usr/bin/docker-control-center` **sigue existiendo** como un pequeño script
que abre `/usr/bin/container-manage-center` con los mismos argumentos. Así
siguen funcionando las tareas de copia programadas antes de la 1.7.0 (sus
unidades de systemd ejecutan `/usr/bin/docker-control-center --run-backup …`)
y cualquier atajo o script tuyo que use el nombre antiguo. Las tareas que
crees o edites a partir de ahora usan el nombre nuevo.

## Después de actualizar

- En el menú de aplicaciones aparece **Container Manage Center** (con el
  mismo icono morado). Si tenías el lanzador antiguo anclado en el panel del
  escritorio, puede que tengas que anclar el nuevo.
- En **Configuración › Legal** están la licencia y el copyright.
- `apt list --installed 'container-manage-center'` muestra el paquete nuevo;
  `docker-control-center` ya no aparece como instalado.

## Desinstalar o volver atrás

- **Desinstalar**: `sudo apt remove container-manage-center` (o desde
  Configuración › Desinstalación y componentes). Tus datos se conservan, igual
  que antes.
- **Volver a la 1.6.0**: `sudo apt install ./docker-control-center_1.6.0_amd64.deb`.
  apt quita el paquete nuevo y pone el antiguo; como los datos no se han
  movido, la 1.6.0 los encuentra donde estaban.
