# Pruebas y control de calidad

![Capas de pruebas, de un cambio en el código a la validación manual.](diagramas/11-pruebas.pdf){width=80%}

## Organización de las pruebas

Las pruebas están junto al código que comprueban:

| Dónde | Qué contiene |
|---|---|
| `crates/cmc-engine/src/**` (módulos `tests`) | Lógica del motor: especificaciones, riesgos, rutas, Compose, SSH, copias, procesos, conexión con Docker |
| `crates/cmc-engine/tests/live_*.rs` | Integración con Docker real (contenedores, edición, imágenes, redes, volúmenes, Compose, eventos, terminales, monitorización, copias, auditoría, SSH) |
| `crates/cmc-store/src/**` | Configuración, perfiles, diarios con rotación, migración de datos |
| `crates/cmc-activity/src/**` | Registro de actividad y ocultación de secretos |
| `src-tauri/src/ipc_tests.rs` | Órdenes IPC de la interfaz con Tauri simulado (sin ventana) |
| `src-tauri/src/**` (otros `tests`) | Copias programadas, llavero, información del sistema |
| `ui/src/**/*.test.ts(x)` | Pruebas de la interfaz con Vitest y jsdom |
| `scripts/verify-evolution-1.11.0.sh` | Verificación completa: compila, prueba, empaqueta y comprueba los requisitos de cada versión |
| `docs/validation/` | Matriz de la auditoría de Windows y guía de validación manual |

## Pruebas de Rust

```bash
cargo test --workspace                      # sin Docker real
CMC_DOCKER_TESTS=1 cargo test --workspace   # con Docker real
```

- **Unitarias y de protocolo**: no necesitan Docker; usan servidores HTTP
  simulados sobre un socket local para reproducir respuestas y errores de la
  API (incluidas carreras de estado y cortes de conexión).
- **IPC**: llaman a las órdenes de Tauri como lo haría la interfaz, con
  configuración temporal, y comprueban permisos, validación, confirmaciones y
  que ningún secreto aparezca en la respuesta.
- **Llavero**: en Linux, con un Secret Service temporal; en Windows, con el
  Administrador de credenciales.
- Las pruebas que dependen del sistema están separadas con `#[cfg(unix)]` o
  `#[cfg(windows)]`, y cada plataforma tiene su variante (por ejemplo,
  cancelar un árbol de procesos con grupos de procesos en Linux y con un *job
  object* en Windows).

## Pruebas del frontend

```bash
npm run typecheck   # TypeScript estricto
npm run lint        # ESLint
npm test            # Vitest
```

Cubren la traducción de errores en ocho idiomas, el diagnóstico de la
conexión, los textos de cada plataforma, la ocultación de secretos en la
interfaz, los formularios de creación y edición, el tema morado y su
contraste. En la versión 1.11.1 pasan **503 pruebas** (entre ellas, que el manual tiene las 34 secciones en los ocho idiomas y que ninguna clave de la caché se pide con dos funciones distintas).

## Pruebas de integración

Con `CMC_DOCKER_TESTS=1`, las pruebas usan un Docker real: crean y eliminan
contenedores, redes, volúmenes e imágenes **con nombres propios de la
prueba**, nunca tocan recursos ajenos y limpian al terminar. La verificación
completa las ejecuta contra **Docker 29, 26 y 24** y un laboratorio de
servidores SSH en contenedores (huellas, contraseñas, claves, saltos,
servidor caído, Docker detenido, permisos insuficientes).

## Pruebas multiplataforma

- **Linux**: todo lo anterior.
- **Windows**: el código se compila de forma cruzada para
  `x86_64-pc-windows-gnu` y las pruebas unitarias y de IPC se ejecutan con
  **Wine**; además, la integración continua las ejecuta en un runner de
  Windows (1.11.1: Windows Server 2025, Rust 1.100, 692 pruebas superadas;
  una prueba de IPC con un límite de 10 s falló una vez por tiempo y pasó al
  repetirla). Wine **no es Windows**: no tiene Docker Desktop, WebView2 Runtime,
  Programador de tareas ni un Administrador de credenciales completo, así que
  esas partes quedan para la validación manual.

## Pruebas de empaquetado

- `.deb`: dependencias calculadas con `dpkg-shlibdeps`, versión de glibc que
  exige el binario y contenido del paquete (`verify-evolution-1.11.0.sh`).
- Instalación limpia (`scripts/dev/clean-install-test.sh`): en sistemas
  Ubuntu 24.04 y 22.04 recién creados con `debootstrap`, `install.sh` como
  usuario normal elige el paquete por la glibc (el normal en 24.04, el
  compatible en 22.04), lo instala, la aplicación arranca con ese usuario y se
  niega a hacerlo como root, y la desinstalación quita el programa y conserva
  los datos del usuario. La actualización desde una versión anterior no se
  ha probado ni en la 1.9.1, ni en la 1.10.0, ni en la 1.11.0 ni en la 1.11.1.
- Instalador de Windows: bibliotecas del `.exe` (solo de Windows y
  `WebView2Loader.dll`) e inspección del script NSIS generado (instalación
  para el usuario actual, idiomas, licencia, carpeta `legal\`, gancho de
  desinstalación).
- Licencias: encabezados SPDX y avisos de terceros al día para Linux y para
  Windows.

## Pruebas manuales

Lo que no se puede automatizar en el entorno de pruebas se valida a mano con
las guías del repositorio:

- `docs/validation/VALIDACION-WINDOWS.md`: pasos numerados (WIN-INS, WIN-DOC,
  WIN-SSH, WIN-BAK, WIN-MON, WIN-ACT, WIN-LEG, WIN-UPD, WIN-UNI) con el
  resultado esperado de cada uno.
- `docs/security/`: lista de comprobación de seguridad.

## Estado de validación por plataforma

| Plataforma | Automáticas | Docker real | Paquete | Manual | Estado |
|---|---|---|---|---|---|
| Ubuntu 24.04 | Sí | Docker 29, 26 y 24 | `.deb` instalado y desinstalado (sistema limpio) | Entorno de pruebas | Verificada automáticamente |
| Ubuntu 22.04 | Compilación del paquete compatible | No | `.deb` compatible instalado y desinstalado (sistema limpio) | No | Paquete comprobado |
| Debian 12 / 13 | No | No | `.deb` compatible (exige glibc ≥ 2.34) | Pendiente | Compatible, sin validar a mano |
| Windows 10 / 11 | Runner Windows Server 2025: 692 pruebas superadas (1.11.1); Wine: 614 (1.9.1) | No | Instalador generado con MSVC en GitHub Actions, no instalado | Pendiente | **En pruebas** |

La matriz detallada de Windows (AUT-01 a AUT-22, con los estados PASS,
PARTIAL y BLOCKED, y los pasos WIN-\* NOT TESTED) está en
`docs/validation/AUDITORIA-WINDOWS-1.9.1.md`.

## Cobertura funcional

| Área | Unitarias | IPC | Docker real | Interfaz |
|---|---|---|---|---|
| Conexión y diagnóstico | Sí | Sí | Sí | Sí |
| Contenedores (crear, editar, acciones) | Sí | Sí | Sí | Sí |
| Imágenes, redes, volúmenes | Sí | Sí | Sí | Sí |
| Compose | Sí | Sí | Sí | Sí |
| Servidores SSH | Sí | Sí | Laboratorio SSH | Sí |
| Copias de seguridad y restauración | Sí | Sí | Sí | Sí |
| Monitorización | Sí | Sí | Sí | Sí |
| Actividad y ocultación de secretos | Sí | Sí | Sí | Sí |
| Riesgos para el equipo | Sí | Sí | — | Sí |

La tabla indica qué capas tienen pruebas, no un porcentaje de líneas: el
proyecto no mide la cobertura de líneas.

## Validación pendiente

**Windows** (en un Windows 10 22H2 o Windows 11 real, con Docker Desktop;
pasos completos en `docs/validation/VALIDACION-WINDOWS.md`):

1. **WIN-INS-01 a 06**: ejecutar `…_1.9.1_x64-setup.exe`, comprobar el aviso
   de SmartScreen (no está firmado), la instalación sin administrador, la
   carpeta `legal\`, el acceso directo, que la ventana se abre y que con
   «Ejecutar como administrador» no arranca.
2. **WIN-DOC-01 a 17**: conexión con Docker Desktop por `npipe`, contenedores,
   imágenes, redes, volúmenes, carpetas vinculadas `C:\…`, construir desde
   una carpeta, Compose con archivos del equipo y detener un `up` en curso.
3. **WIN-SSH-01 a 08**: OpenSSH de Windows, huella, clave elegida con el
   selector, contraseña guardada en el Administrador de credenciales.
4. **WIN-BAK-01 a 06**: copia a `C:\…`, comprobarla, restaurarla y programarla
   en el Programador de tareas.
5. **WIN-MON-01, WIN-ACT-01, WIN-LEG, WIN-UPD**: monitorización, actividad,
   avisos legales y comprobación de versiones.
6. **WIN-UNI**: desinstalar y comprobar que se eliminan las tareas
   programadas de la aplicación y se conservan los datos del usuario.
7. Capturas de Windows para este manual y para el `README.md`.

**Linux**:

1. Instalar el `.deb` compatible en un **Debian 13** real con Docker y
   repetir el recorrido de la [Parte I, Manual de usuario](#ayuda-about).
2. Probar Docker en modo *rootless* en un equipo real.

**Decisiones del titular** antes de publicar: si los paquetes se firman con
GPG y cuándo se publica el repositorio. El permiso adicional de WebView2 y el
rechazo del administrador en Windows (W-18) se aprobaron el 2026-11-17.

## Cómo informar de un fallo

Con el formulario de error del repositorio (versión, sistema, Docker, pasos,
resultado esperado y el registro de actividad, que no contiene secretos). Las
vulnerabilidades, **nunca en un issue público**: aviso privado de GitHub
(ver `SECURITY.md`).
