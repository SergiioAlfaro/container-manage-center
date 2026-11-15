# Validación manual de Container Manage Center 1.8.0 (Debian 13)

Solo lo nuevo de la 1.8.0: el nombre anterior desaparece y tus datos pasan al
nombre nuevo. Lo demás ya está validado y **no se repite**.

Tiempo estimado: 20–30 minutos. Con tu usuario normal; `sudo` solo para apt.

## 0. Antes de empezar (con la 1.7.0 instalada)

- [ ] Usa la 1.7.0 con tus datos reales: al menos un servidor guardado (mejor
      con la contraseña o frase de paso recordada en el llavero), alguna copia
      hecha y, si las usas, una tarea de copia programada.
- [ ] Anota lo que hay:
      ```bash
      find ~/.config/docker-control-center ~/.local/state/docker-control-center -type f -exec sha256sum {} + | sed 's#docker-control-center#NOMBRE#' | sort -k2 > /tmp/cmc-antes.txt
      systemctl --user list-timers 'dcc-backup-*' --all
      ls ~/Copias 2>/dev/null   # o la carpeta de destino de tus copias
      ```
- [ ] `bash scripts/verify-evolution-1.8.0.sh --deb --net` en el código de la
      1.8.0 termina con «parte automática superada».

## 1. Actualización y migración

| # | Paso | Resultado esperado |
|---|---|---|
| U1 | Cierra la aplicación. `bash install.sh` | apt sustituye la 1.7.0 por la 1.8.0; al final, «Datos de versiones anteriores: al día» |
| U2 | `find ~/.config/container-manage-center ~/.local/state/container-manage-center -type f -exec sha256sum {} + \| sed 's#container-manage-center#NOMBRE#' \| sort -k2 \| diff /tmp/cmc-antes.txt -` | Sin diferencias |
| U3 | `ls -d ~/.config/docker-control-center ~/.local/state/docker-control-center ~/.local/share/org.dockercontrolcenter.app ~/.cache/org.dockercontrolcenter.app 2>&1` | «No existe el archivo o el directorio» para las cuatro |
| U4 | `systemctl --user list-timers 'cmc-backup-*' --all` y `systemctl --user list-timers 'dcc-backup-*' --all` | Tus tareas con el nombre `cmc-backup-…`, activadas igual que antes; ninguna `dcc-backup-…` |
| U5 | `ls <destino de tus copias>` | `container-manage-center-copias`; ya no está `docker-control-center-copias` |
| U6 | `ls /usr/bin/ \| grep -i control-center; ls /usr/share/applications \| grep -i control-center` | Nada |

## 2. Todo sigue funcionando

| # | Paso | Resultado esperado |
|---|---|---|
| F1 | Abre la aplicación desde el menú | «Container Manage Center», con tus ajustes (tema, idioma) y el servidor que tenías elegido |
| F2 | Servidores: conecta uno con contraseña o frase de paso recordada | Conecta sin pedirla. En `seahorse`, la entrada se llama «Container Manage Center — …» y ya no hay ninguna «Docker Control Center — …» |
| F3 | Copias: historial y «Comprobar» en una copia anterior | Aparecen todas; la comprobación es correcta |
| F4 | Tarea programada: `systemctl --user start cmc-backup-<id>.service` y luego `journalctl --user -u cmc-backup-<id> -n 20` | La copia se hace bien; aparece en el historial de copias |
| F5 | Monitorización e historial de actividad | El historial anterior sigue ahí |
| F6 | Un contenedor con descripción puesta con una versión anterior: Editar configuración | Se ve su descripción. Cámbiala y aplica: `docker inspect -f '{{json .Config.Labels}}' <contenedor>` muestra `cmc.description` y no la etiqueta antigua |
| F7 | Panel de actividad, primera vez tras actualizar con apt a mano (si no usaste `install.sh`) | Aparecen las operaciones de systemd que pasaron las tareas al nombre nuevo |

## Hoja de resultados

| Apartado | Pasos | Resultado (OK / fallo + nota) |
|---|---|---|
| Actualización y migración | U1–U6 | |
| Todo sigue funcionando | F1–F7 | |
