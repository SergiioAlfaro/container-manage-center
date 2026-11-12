# Docker Control Center

Aplicación de escritorio para administrar Docker Engine en Debian y Ubuntu, en
este equipo y en servidores remotos por SSH: contenedores, imágenes, redes,
volúmenes, directorios vinculados y proyectos Docker Compose, mostrando en todo
momento las operaciones reales que se ejecutan.

**Versión 1.0.0** (Fase 10: pruebas finales y distribución). Tauri 2 · Rust ·
React · TypeScript · bollard.

## Qué hace

- **Contenedores**: crear con un asistente, iniciar, detener, pausar, reiniciar,
  eliminar, duplicar, editar la configuración (con recreación segura y copia),
  crear una imagen a partir de un contenedor, registros, estadísticas, inspección
  y terminal.
- **Imágenes**: descargar con progreso, buscar, construir, etiquetar, ver capas y
  uso, eliminar y limpiar.
- **Redes, volúmenes y directorios vinculados**: crear, conectar, inspeccionar,
  eliminar y limpiar con avisos de uso.
- **Docker Compose**: importar o crear proyectos, editor con validación,
  desplegar, detener, construir, servicios y registros.
- **Terminales**: varias sesiones a la vez, en contenedores y en servidores.
- **Servidores remotos por SSH**: varios servidores a la vez con la misma
  funcionalidad que en local, huella del servidor verificada y credenciales en
  el almacén seguro del sistema.
- **Panel de actividad**: cada operación con la petición real (API o CLI), su
  orden `docker` equivalente, el servidor, la duración y el resultado. Eventos de
  Docker en tiempo real e historial.
- **Ayuda integrada** y ocho idiomas: English, Español, Català, Galego, Euskara,
  Français, Italiano y Português. Tema claro, oscuro o del sistema; color
  principal morado.

La aplicación nunca instala Docker, nunca cambia permisos ni grupos del sistema y
no se ejecuta como root.

## Instalar

En Debian 12+ o Ubuntu 22.04+ (64 bits), con los paquetes `.deb` junto al
script o compilados en este código fuente:

```bash
bash scripts/install.sh          # elige el paquete para tu sistema y lo instala con apt
bash scripts/install.sh --check  # solo comprueba el sistema
bash scripts/install.sh --uninstall
```

O directamente: `sudo apt install ./docker-control-center_1.0.0_amd64.deb`.
Después se abre desde el menú de aplicaciones o con `docker-control-center`
(desde el código fuente: `bash scripts/run.sh`). Requisitos, acceso a Docker,
compilación y diagnóstico en [INSTALL.md](INSTALL.md).

## Documentación

| Documento | Contenido |
|---|---|
| [INSTALL.md](INSTALL.md) | Instalación, requisitos, dependencias y compilación |
| [USER_GUIDE.md](USER_GUIDE.md) | Uso de la aplicación |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Arquitectura técnica y pruebas |
| [DECISIONS.md](DECISIONS.md) | Decisiones técnicas |
| [STATUS.md](STATUS.md) | Estado del proyecto, fases y resultados de las pruebas |
| [CHANGELOG.md](CHANGELOG.md) | Cambios de cada versión |
| [packaging/THIRD-PARTY.md](packaging/THIRD-PARTY.md) | Componentes de terceros y sus licencias |

## Comprobación

```bash
bash scripts/verify-phase-10.sh --deb   # regresión completa, paquete y comprobaciones de la Fase 10
cargo run -q -p dcc-engine --bin dcc-diagnose   # diagnóstico de la conexión con Docker
```

## Licencia

GPL-3.0-or-later ([LICENSE](LICENSE)), provisional hasta la confirmación del
propietario, igual que el nombre del producto. «Docker» es una marca registrada
de Docker, Inc.; este proyecto no está afiliado a Docker, Inc.
