# Docker Control Center

Aplicación de escritorio para administrar Docker Engine en Debian y Ubuntu:
contenedores, imágenes, redes, volúmenes, directorios vinculados y proyectos
Docker Compose, mostrando en todo momento las operaciones reales que se ejecutan.

Tauri 2 · Rust · React · TypeScript.

| Documento | Contenido |
|---|---|
| [STATUS.md](STATUS.md) | Estado del proyecto, fases y pruebas |
| [INSTALL.md](INSTALL.md) | Instalación y compilación |
| [USER_GUIDE.md](USER_GUIDE.md) | Uso de la aplicación |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Arquitectura técnica |
| [DECISIONS.md](DECISIONS.md) | Decisiones técnicas |

Estado: versión 0.9.2 (actualización previa a la Fase 10). Detalle en
[STATUS.md](STATUS.md). Comprobación en tu equipo:

```bash
bash scripts/verify-update-0.9.2.sh     # añade --deb para construir el paquete
```

Diagnóstico de la conexión desde terminal:

```bash
cargo run -q -p dcc-engine --bin dcc-diagnose
```
