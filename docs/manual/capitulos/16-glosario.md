# Glosario y referencias

## Glosario

| Término | Significado |
|---|---|
| API de Docker Engine | Interfaz HTTP con la que se controla Docker (contenedores, imágenes…). La usan la CLI de Docker y CMC |
| Askpass | Programa al que `ssh` pide una contraseña o frase cuando no hay terminal (`SSH_ASKPASS`) |
| Bind mount (carpeta vinculada) | Carpeta o archivo del equipo montado dentro de un contenedor |
| bollard | Biblioteca de Rust que implementa la API de Docker |
| Canal (Tauri) | Vía de mensajes de Rust a la interfaz para un flujo concreto |
| Capacidad (Linux) | Privilegio concreto del núcleo (`NET_ADMIN`, `SYS_ADMIN`…) que se puede dar a un contenedor |
| Contexto de Docker | Configuración con nombre de a qué Docker se conecta la CLI (`docker context`) |
| ControlMaster | Opción de OpenSSH que reutiliza una conexión para varias sesiones |
| `dial-stdio` | `docker system dial-stdio`: conecta la entrada y salida estándar con la API de Docker; permite usarla por SSH |
| Docker Compose | Herramienta para definir y ejecutar aplicaciones de varios contenedores con un archivo YAML |
| Docker Desktop | Aplicación de Docker para Windows y macOS que ejecuta Docker en una máquina virtual |
| `DOCKER_HOST` | Variable de entorno que indica a qué Docker conectarse |
| Huella (fingerprint) | Resumen de la clave pública de un servidor SSH (`SHA256:…`) para verificar su identidad |
| IPC | Comunicación entre procesos; aquí, entre la interfaz web y Rust |
| *Job object* | Objeto de Windows que agrupa procesos para gestionarlos (y terminarlos) juntos |
| JSONL | Archivo con un objeto JSON por línea |
| Llavero | Almacén seguro de secretos del sistema (Secret Service, Administrador de credenciales) |
| NSIS | Nullsoft Scriptable Install System, generador de instaladores de Windows |
| Rootless | Docker ejecutado con un usuario normal, sin privilegios de administrador |
| Socket Unix | Archivo especial para comunicar procesos en el mismo equipo (`/var/run/docker.sock`) |
| SPDX | Formato estándar para indicar la licencia de un archivo |
| Tauri | Marco para aplicaciones de escritorio con interfaz web y núcleo en Rust |
| Tubería con nombre | Mecanismo de comunicación entre procesos de Windows (`\\.\pipe\docker_engine`) |
| WebKitGTK / WebView2 | Motores web del sistema que usa Tauri en Linux y en Windows |
| Wine | Capa de compatibilidad que ejecuta programas de Windows en Linux |
| WSL 2 | Subsistema de Windows para Linux, versión 2; Docker Desktop puede ejecutarse sobre él |
| zeroize | Técnica (y biblioteca) para borrar secretos de la memoria al dejar de usarlos |

## Referencias oficiales

| Tecnología | Documentación |
|---|---|
| Tauri | <https://v2.tauri.app/> |
| Rust | <https://www.rust-lang.org/learn> |
| React | <https://react.dev/> |
| TypeScript | <https://www.typescriptlang.org/docs/> |
| Vite | <https://vite.dev/guide/> |
| Docker Engine | <https://docs.docker.com/engine/> |
| API de Docker Engine | <https://docs.docker.com/reference/api/engine/> |
| Docker Compose | <https://docs.docker.com/compose/> |
| Docker Desktop para Windows | <https://docs.docker.com/desktop/setup/install/windows-install/> |
| Docker rootless | <https://docs.docker.com/engine/security/rootless/> |
| Debian | <https://www.debian.org/doc/> |
| Ubuntu | <https://help.ubuntu.com/> |
| Windows | <https://learn.microsoft.com/windows/> |
| OpenSSH para Windows | <https://learn.microsoft.com/windows-server/administration/openssh/openssh_install_firstuse> |
| WSL 2 | <https://learn.microsoft.com/windows/wsl/> |
| Microsoft Edge WebView2 | <https://learn.microsoft.com/microsoft-edge/webview2/> |
| GNU GPL v3 | <https://www.gnu.org/licenses/gpl-3.0.html> |
| Preguntas frecuentes de la GPL | <https://www.gnu.org/licenses/gpl-faq.html> |

## Documentos del repositorio

| Documento | Contenido |
|---|---|
| `README.md` | Presentación y estado de las plataformas |
| `INSTALL.md` | Instalación detallada |
| `USER_GUIDE.md` | Guía de uso completa |
| `ARCHITECTURE.md` | Arquitectura y referencia de las órdenes IPC |
| `DECISIONS.md` | Decisiones de diseño (D-001 a D-116) |
| `STATUS.md`, `CHANGELOG.md` | Estado y cambios |
| `docs/validation/AUDITORIA-WINDOWS-1.9.1.md` | Auditoría de Windows y matriz de pruebas |
| `docs/validation/VALIDACION-WINDOWS.md` | Guía de validación manual en Windows |
| `docs/security/` | Auditoría y lista de comprobación de seguridad |
| `docs/legal/` | Licencia, copyright, avisos, permiso de WebView2, cumplimiento |
