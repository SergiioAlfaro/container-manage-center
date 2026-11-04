# Arquitectura de Docker Control Center

## Visión general

```
┌──────────────────────── Interfaz (React + TypeScript, WebKitGTK) ─────────────────────┐
│  Sidebar  │  Vista del apartado activo  │  Panel de actividad (cuaderno de bitácora)  │
│                      ▲ invoke() tipado            ▲ eventos activity://event           │
└──────────────────────┼────────────────────────────┼────────────────────────────────────┘
┌──────────────────────┴──────────── Backend (Rust, tokio) ─┴────────────────────────────┐
│  src-tauri: comandos IPC · estado · puente de eventos · negativa a root               │
│      │                                     ▲                                           │
│      ├── dcc-store     (config.toml)       │ broadcast                                  │
│      ├── dcc-activity  (registro de operaciones reales) ──┘                             │
│      ├── dcc-engine    (Fase 2: cliente Docker Engine API con bollard)                  │
│      └── dcc-compose   (Fase 7: `docker compose` sin shell)                             │
└───────────────┬─────────────────────────────────────────────────┬──────────────────────┘
        /var/run/docker.sock (API)                     docker compose (proceso CLI)
```

## Estructura de directorios

```
docker-control-center/
├── Cargo.toml                 Workspace Rust y perfiles de compilación
├── package.json               Frontend y CLI de Tauri
├── vite.config.ts             Vite (raíz en ui/) y Vitest
├── .cargo/config.toml         Destino de los tipos generados por ts-rs
├── crates/
│   ├── dcc-activity/          Registro de operaciones (modelo, log, entrecomillado)
│   └── dcc-store/             Configuración persistente
├── src-tauri/
│   ├── tauri.conf.json        Ventana, CSP, empaquetado .deb
│   ├── capabilities/          Permisos mínimos de la ventana
│   └── src/
│       ├── lib.rs             `configure()` y `run()`
│       ├── state.rs           Estado compartido (config + registro)
│       ├── events.rs          Puente registro → ventana
│       ├── error.rs           Error serializable de los comandos
│       ├── commands/          Un módulo por área (app, config, activity…)
│       └── ipc_tests.rs       Pruebas de integración con el runtime simulado
├── ui/
│   ├── index.html
│   └── src/
│       ├── app/               Armazón: App, Sidebar, secciones, estado global
│       ├── features/          Un directorio por apartado (activity, settings…)
│       ├── shared/            i18n, cliente IPC, tipos generados, controles
│       ├── styles/app.css     Tokens de diseño y temas
│       └── test/              Configuración y fixtures de pruebas
├── scripts/                   Verificación por fase e instalación de dependencias
└── packaging/                 Logotipo fuente
```

## Módulos

### dcc-activity
- `ActivityLog`: búfer circular protegido por `parking_lot::Mutex`, capacidad
  configurable, notificación por `tokio::sync::broadcast`.
- `ActivityLog::begin(OperationSpec) -> OperationHandle`. El handle se cierra con
  `succeed(Outcome)` o `fail(Failure)`. Si se descarta, la operación queda
  `cancelled` (`Drop`).
- `clear()` vacía el historial visual pero conserva las operaciones en curso.
- `quote::render_argv` convierte argumentos en una línea POSIX copiable. La
  misma regla está implementada en TypeScript y probada en ambos lados.

### dcc-store
- `ConfigStore::load()` devuelve la configuración y un aviso opcional.
- `ConfigStore::save()` escribe de forma atómica con permisos `0600`.

### src-tauri
Comandos IPC actuales:

| Comando | Descripción |
|---|---|
| `app_info` | Versión, ruta de configuración y aviso de arranque |
| `config_get` | Configuración actual |
| `config_set` | Normaliza, guarda y aplica (E/S fuera del hilo principal) |
| `activity_list` | Registros del más reciente al más antiguo |
| `activity_clear` | Vacía el historial visual |

Eventos emitidos:

| Evento | Carga |
|---|---|
| `activity://event` | `ActivityEvent` (`upsert` con el registro, o `cleared`) |
| `activity://resync` | Sin carga; el frontend debe recargar con `activity_list` |

Seguridad de la ventana: CSP restrictiva (`script-src 'self'`, sin recursos
externos), `freezePrototype`, y una única capacidad con `core:default` y
`clipboard-manager:allow-write-text`.

### Interfaz
- `shared/ipc/client.ts` es el único punto que llama a `invoke`/`listen`; las
  pruebas lo sustituyen con `vi.mock`.
- `app/store.ts` (Zustand): apartado activo y configuración. `updateConfig` es
  optimista y revierte si el backend falla, mostrando el error.
- `features/activity/`: lógica pura (`logic.ts`), estado (`store.ts`) y
  componentes. El panel no tiene forma de crear registros: solo refleja el backend.

## Flujo de una operación (a partir de la Fase 2)

1. La interfaz invoca un comando (p. ej. `container_start`).
2. El backend construye el `OperationSpec` con la petición real y el comando
   equivalente, y llama a `ActivityLog::begin` → evento `running`.
3. Ejecuta la petición contra Docker.
4. Cierra el handle con el resultado o el error de Docker → evento final.
5. El panel actualiza el registro en su sitio; si falló y la opción está activa,
   se abre automáticamente.

## Pruebas

| Capa | Herramienta | Qué cubre |
|---|---|---|
| Rust unitarias | `cargo test` | Registro, concurrencia, entrecomillado, configuración |
| Rust integración | Runtime simulado de Tauri | Comandos IPC reales, ACL, eventos hacia la ventana |
| Interfaz | Vitest + Testing Library | Navegación, panel, filtros, copiar, temas, idiomas, errores |
| Docker real | `cargo test` con daemon | A partir de la Fase 2 |
| Manual | `scripts/verify-phase-N.sh` | Comprobaciones en Debian/Ubuntu del propietario |
