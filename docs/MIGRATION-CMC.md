# Actualizar desde una versión anterior a la 1.8.0

Hasta la 1.7.0, la aplicación guardaba sus datos con el nombre que tuvo hasta
la 1.6.0 (*Docker Control Center*). Desde la 1.8.0 ese nombre ya no se usa en
ningún sitio: al actualizar, tus datos pasan a las carpetas y nombres nuevos
**sin perder nada**, y la aplicación ya no crea ni instala nada con el nombre
anterior.

## Qué hay que hacer

Instala el paquete nuevo como cualquier actualización, **preferiblemente con
`install.sh`**:

```bash
./install.sh
# o a mano:
sudo apt install ./container-manage-center_1.8.0_amd64.deb
# (en Debian 12 / Ubuntu 22.04, el paquete …_amd64-compat-debian12-ubuntu2204.deb)
```

- **Con `install.sh`**: después de instalar, pasa tus datos al nombre nuevo
  con tu usuario (`container-manage-center --migrate`).
- **Con `apt` a mano**: abre la aplicación una vez después de instalar; al
  arrancar pasa los datos. Hasta entonces, las **copias programadas** creadas
  antes de la 1.8.0 no se ejecutan, porque llaman al programa con el nombre
  anterior, que ya no existe. También puedes ejecutar
  `container-manage-center --migrate` en una terminal (con tu usuario, sin `sudo`).

apt sustituye el paquete anterior (`docker-control-center` de la 1.6.0 o
`container-manage-center` de la 1.7.0) en una sola operación.

## Qué se pasa al nombre nuevo

| Dato | Antes | Ahora | Cuándo |
|---|---|---|---|
| Configuración, servidores, huellas SSH, alertas, tareas de copia | `~/.config/docker-control-center/` | `~/.config/container-manage-center/` | Al arrancar o con `--migrate` |
| Historial de actividad, auditorías, monitorización | `~/.local/state/docker-control-center/` | `~/.local/state/container-manage-center/` | Al arrancar o con `--migrate` |
| Datos de la interfaz (WebKit) | `~/.local/share/org.dockercontrolcenter.app/`, `~/.cache/org.dockercontrolcenter.app/` | `…/org.containermanagecenter.app/` | Al arrancar o con `--migrate` |
| Tareas programadas (systemd de usuario) | `dcc-backup-<tarea>.service/.timer` | `cmc-backup-<tarea>.service/.timer` | Al arrancar o con `--migrate` |
| Copias de seguridad | `<destino>/docker-control-center-copias/` | `<destino>/container-manage-center-copias/` | Al arrancar o con `--migrate` (los destinos de tus tareas y el predeterminado); cualquier otro, al usarlo |
| Contraseñas y frases de paso guardadas | Almacén del escritorio, atributo `application=docker-control-center` | `application=container-manage-center` | Al arrancar (las del llavero desbloqueado) o al usarlas |
| Servidor elegido en la barra superior | Preferencia de la interfaz | Clave nueva | Al abrir la aplicación |

## Cómo se protege que no se pierda nada

- Las carpetas se **renombran** dentro de la misma carpeta padre: es una
  operación atómica, así que en todo momento existe una de las dos completa.
  Si no se puede renombrar (por ejemplo, porque la carpeta antigua es un enlace
  a otro disco), se **copia**, se **comprueba** que la copia es idéntica y solo
  entonces se borra la antigua.
- Si ya existen las dos (la nueva y la antigua), solo se mueve lo que falta
  en la nueva; **nada se sobrescribe**. Lo que no se puede mover se queda en la
  carpeta antigua y la aplicación lo avisa al arrancar.
- Las tareas programadas se crean y se activan con el nombre nuevo **antes**
  de desactivar y quitar las antiguas, y conservan su estado (activada o no).
  Las de tareas que ya no existen se quitan. Las unidades de systemd que no
  son de la aplicación no se tocan.
- Cada contraseña se guarda con los atributos nuevos **antes** de borrar la
  antigua. Las del llavero bloqueado se pasan cuando se usan, para no pedir
  que lo desbloquees al arrancar.
- Repetir la migración no hace nada.
- Cuando la migración la hace la aplicación al arrancar, los cambios en systemd
  quedan en el panel de actividad.

## Contenedores creados con versiones anteriores

En Docker no se cambia nada: los contenedores, imágenes, redes y volúmenes
son tuyos. La aplicación sigue reconociendo lo que crearon versiones
anteriores:

- la **descripción** guardada en la etiqueta antigua de un contenedor se
  muestra igual y, si la editas, se guarda con la etiqueta nueva;
- los contenedores apartados al editar (`<nombre>-dcc-backup-<fecha>`) se
  siguen reconociendo como copias de la edición.

## Volver a una versión anterior

Ya no es transparente: la 1.7.0 o la 1.6.0 buscarían los datos en las
carpetas antiguas y no los encontrarían (no se han borrado: están en las
nuevas). Si necesitas volver atrás, renombra a mano las carpetas de la tabla
anterior a su nombre antiguo antes de abrir la versión anterior.

## Desinstalar

`sudo apt remove container-manage-center` (o desde Configuración ›
Desinstalación y componentes). Tus datos se conservan en las carpetas nuevas.
