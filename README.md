# Container Manage Center (CMC)

Aplicación de escritorio para administrar Docker Engine en Debian y Ubuntu, en
este equipo y en servidores remotos por SSH: contenedores, imágenes, redes,
volúmenes, directorios vinculados y proyectos Docker Compose, mostrando en todo
momento las operaciones reales que se ejecutan.

**Versión 1.7.0** (identidad Container Manage Center, licencia definitiva
GPL-3.0-or-later y avisos de terceros completos). Antes se llamaba *Docker
Control Center*: [cómo se actualiza](docs/MIGRATION-CMC.md), sin perder datos.
Tauri 2 · Rust · React · TypeScript · bollard.

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
- **Herramientas enlazadas** (1.6.0): la auditoría muestra, para cada
  contenedor, si la monitorización lo vigila y qué copia lo protege (por su
  id), y desde ahí abre sus gráficas o prepara una copia.
- **Monitorización** (1.5.0): CPU, memoria, red, disco y procesos de cada
  contenedor como `docker stats`, sin instalar nada en los servidores; gráficos
  de 5 minutos a 24 horas, comparación de contenedores, huecos visibles cuando
  no hay conexión, y alertas con histéresis y notificaciones de escritorio.
- **Copias de seguridad** (1.4.0): de volúmenes, bases de datos (MariaDB, MySQL,
  PostgreSQL, Redis, MongoDB, con su propio volcado), configuración de
  contenedores, Compose y la aplicación; con manifiesto y SHA-256, comprobación
  de integridad, restauración completa o selectiva con vista previa, retención y
  tareas programadas con temporizadores de usuario de systemd (solo con tu
  autorización).
- **Auditoría de seguridad** (1.3.0): revisa contenedores, imágenes y proyectos
  Compose sin cambiar nada, explica cada hallazgo, propone correcciones con vista
  previa, analiza vulnerabilidades con Trivy o Grype si están instalados y
  exporta el informe.
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

O directamente: `sudo apt install ./container-manage-center_1.9.0_amd64.deb`
(sustituye a una versión anterior si estaba instalada y pasa tus datos al nombre nuevo).
Después se abre desde el menú de aplicaciones o con `container-manage-center`
(desde el código fuente: `bash scripts/run.sh`). Requisitos, acceso a Docker,
compilación y diagnóstico en [INSTALL.md](INSTALL.md).

**Windows 10 y 11** (en pruebas, 1.9.0): instalador para el usuario actual, con
Docker Desktop y el OpenSSH de Windows. Todavía no se ha probado en un Windows
real y, hasta resolver una cuestión de licencia de un componente de Microsoft
(WebView2Loader), el instalador es solo para pruebas y no se redistribuye.
Detalles en [INSTALL.md](INSTALL.md#opción-d-windows-10-y-11-x86_64--en-pruebas).

## Documentación

| Documento | Contenido |
|---|---|
| [INSTALL.md](INSTALL.md) | Instalación, requisitos, dependencias y compilación |
| [USER_GUIDE.md](USER_GUIDE.md) | Uso de la aplicación |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Arquitectura técnica y pruebas |
| [DECISIONS.md](DECISIONS.md) | Decisiones técnicas |
| [STATUS.md](STATUS.md) | Estado del proyecto, fases y resultados de las pruebas |
| [CHANGELOG.md](CHANGELOG.md) | Cambios de cada versión |
| [docs/roadmap/PROJECT_EVOLUTION_PLAN.md](docs/roadmap/PROJECT_EVOLUTION_PLAN.md) | Plan de evolución (etapas E0–E6) |
| [docs/security/](docs/security/) | Auditoría de seguridad, plan de corrección y lista de comprobación |
| [docs/validation/VALIDACION-GUIADA-1.6.0.md](docs/validation/VALIDACION-GUIADA-1.6.0.md) | Validación manual guiada (todo lo pendiente, en un solo recorrido) |
| [docs/MIGRATION-CMC.md](docs/MIGRATION-CMC.md) | Actualizar desde una versión anterior a la 1.8.0 (paso de los datos al nombre nuevo) |
| [docs/legal/](docs/legal/) | Licencia, copyright, auditoría de dependencias, avisos de terceros y cumplimiento al distribuir |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Cómo contribuir (licencia, encabezados SPDX, dependencias) |

## Comprobación

```bash
bash scripts/verify-evolution-1.9.0.sh --deb   # regresión completa, paquete, licencias y migración
cargo run -q -p cmc-engine --bin cmc-diagnose   # diagnóstico de la conexión con Docker
npm run lint                                    # ESLint (configuración mínima, reglas de seguridad)
cargo deny check                                # dependencias de Rust (deny.toml)
python3 scripts/third-party-licenses.py --check  # avisos de terceros y auditoría al día
python3 scripts/spdx-headers.py --check         # encabezados SPDX del código propio
```

## Legal

Container Manage Center · Copyright © 2026 Sergio Alfaro Lorite.
Licencia **GPL-3.0-or-later** ([LICENSE](LICENSE)): puedes redistribuirlo y
modificarlo según los términos de la GNU General Public License, versión 3 o
(a tu elección) cualquier versión posterior, publicada por la Free Software
Foundation. Se distribuye **sin ninguna garantía**. Detalles en
[docs/legal/LICENSE_OVERVIEW.md](docs/legal/LICENSE_OVERVIEW.md) y
[docs/legal/COPYRIGHT.md](docs/legal/COPYRIGHT.md).

Los componentes de terceros conservan sus licencias y avisos:
[docs/legal/THIRD_PARTY_NOTICES.md](docs/legal/THIRD_PARTY_NOTICES.md).

Container Manage Center es un proyecto independiente: no está afiliado a
Docker, Inc. ni respaldado por ella. «Docker» es una marca de Docker, Inc. y
aquí solo se usa para nombrar la tecnología con la que trabaja la aplicación.
