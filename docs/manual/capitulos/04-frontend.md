# Frontend

## Organización de componentes

La interfaz (`ui/src/`) está organizada **por funcionalidad**: cada apartado
de la barra lateral es una carpeta de `features/` con su vista, su estado
local, su modelo (funciones puras) y sus pruebas. Lo común está en `shared/`
y la estructura de la ventana en `app/`.

![Relación entre los componentes principales de la interfaz.](diagramas/12-frontend.pdf){width=90%}

| Carpeta de `features/` | Apartado o función |
|---|---|
| `dashboard` | Panel de inicio: versión de Docker, recursos, equipo, avisos |
| `servers` | Servidores remotos: asistente, prueba de conexión, identidad, diagnóstico, vista general |
| `containers` | Lista, asistente de creación (`create/`), detalle (`detail/`), edición (`edit/`) |
| `bulk` | Gestión masiva (1.10.0): tabla de varios servidores, filtros, selección, confirmación, progreso y resultados |
| `images` | Lista, detalle, descargas (`pull/`), construcción (`build/`), búsqueda |
| `networks` · `volumes` · `binds` | Redes, volúmenes y carpetas vinculadas |
| `compose` | Proyectos, servicios, despliegue, registros, editor de archivos |
| `terminals` | Varias terminales a la vez con distribución en paneles |
| `audit` · `backup` · `monitor` | Auditoría de seguridad, copias, monitorización |
| `events` | Eventos en directo, guardados, consulta a Docker e historial |
| `settings` | Configuración, sistema, actualizaciones, componentes, legal |
| `activity` | Panel de actividad (lateral derecho) |
| `engine` | Estado de la conexión, indicador y diagnóstico |
| `help` | Ayuda integrada (30 secciones en 8 idiomas) |
| `pending` | Pantallas de funciones no disponibles en un servidor |

## Pantallas y navegación

La ventana tiene cuatro zonas: **barra lateral** (apartados, se contrae con
Ctrl+B), **barra superior** (título, selector de servidor, indicador de
conexión y botón de actividad), **área de trabajo** y **panel de actividad**
(Ctrl+Mayús+A).

Los apartados están definidos en `app/sections.ts`, en grupos: inicio y
servidores; recursos (contenedores, imágenes, redes, volúmenes, carpetas
vinculadas); orquestación (Compose); herramientas (terminales, auditoría,
copias); supervisión (monitorización, eventos); y configuración.

No hay *router* de URL: el apartado activo es estado de la aplicación
(`app/store.ts`), y la navegación interna de cada apartado (lista, detalle,
asistente) la lleva el `store.ts` del apartado. Los apartados se cargan con
`React.lazy` y se precargan dos segundos después del arranque, de modo que el
primer cambio de apartado es inmediato.

## Estado

| Tipo de estado | Herramienta | Ejemplos |
|---|---|---|
| Datos de Docker | TanStack Query | Lista de contenedores, detalle de una imagen, proyectos Compose |
| Estado global de la interfaz | Zustand (`app/store.ts`) | Apartado, configuración, panel de actividad |
| Servidor elegido | Zustand (`app/server.tsx`) | `local` o un servidor remoto; se recuerda entre sesiones |
| Conexión | Zustand (`features/engine/store.ts`) | Conectando, conectado o desconectado con diagnóstico |
| Estado de cada apartado | Zustand (`features/*/store.ts`) | Pestaña del detalle, filtros, especificación del asistente |
| Operaciones largas | Almacenes globales | Descargas, construcciones, operaciones de Compose: siguen al cambiar de apartado |

La configuración se cambia de forma **optimista**: la interfaz aplica el
cambio, lo guarda con `config_set` y lo revierte si falla. Valores por
defecto (`app/store.ts`):

```typescript
export const DEFAULT_CONFIG: AppConfig = {
  version: 1,
  ui: { theme: "system", language: "es", accent: "purple", sidebarCollapsed: false },
  activity: { panelOpen: false, openOnError: true, maxRecords: 500, keepHistory: true },
  docker: { mode: "auto", customSocket: "" },
  compose: { projects: [] },
  updates: { checkOnStart: false, source: "" },
};
```

## Comunicación con el backend

Todas las órdenes están en `shared/ipc/client.ts`. La función `api(server)`
devuelve un objeto con una función por orden que añade el servidor elegido, de
modo que un apartado no puede olvidarse de indicarlo. Los tipos de los
argumentos y de las respuestas se **generan desde Rust** (`npm run bindings`
ejecuta las pruebas `export_bindings` de `ts-rs`), así que un cambio de tipo en
Rust rompe la compilación de TypeScript en lugar de fallar en ejecución.

Los flujos (registros, estadísticas, terminales, descargas, construcción,
Compose) usan un `Channel` de Tauri: la orden de apertura recibe el canal y
devuelve un identificador; la interfaz cierra el flujo con `stream_close`
cuando el componente se desmonta (`useStream`).

## Formularios

Los formularios grandes (crear contenedor, editar, crear red o volumen,
construir imagen, nuevo servidor, tarea de copia) siguen el mismo patrón:

1. el formulario mantiene un **borrador** en el estado del apartado;
2. con cada cambio (con 200 ms de espera) se pide al backend una **vista
   previa** que valida, calcula los riesgos y devuelve la orden equivalente y
   el cuerpo exacto de la petición, **sin hablar con Docker**;
3. los problemas se muestran junto a su campo, con el número de errores en
   cada pestaña;
4. el botón final repite la validación en el backend antes de ejecutar.

Así la validación vive **solo en Rust** y la interfaz no puede saltársela.

## Gestión de errores

- `shared/ipc/client.ts` convierte cualquier fallo de `invoke` en un
  `CommandError` y `localizeError` traduce los mensajes fijos al idioma.
- Las mutaciones muestran el error junto a la acción (por ejemplo, encima de
  la tabla) y el detalle completo queda en el panel de actividad.
- `RequireConnection` sustituye un apartado por el diagnóstico cuando no hay
  conexión con Docker.
- `ErrorBoundary` captura los errores de React; `public/boot-guard.js` muestra
  los errores anteriores al arranque de la interfaz.

## Eventos

- `engine://state`: al pasar a conectado se invalidan todas las consultas; al
  perder la conexión se descartan.
- `docker://event`: `features/events/liveRefresh.ts` traduce cada evento en las
  consultas afectadas (`invalidationsFor`) y las recarga agrupadas.
- `activity://event`: el panel de actividad se actualiza en directo.

## Interfaz de usuario e identidad visual

- **Controles comunes** en `shared/ui/`: botones, campos, diálogos
  (`feedback.tsx`), menú accesible con teclado (`Menu.tsx`), gráficas
  (`LineChart.tsx`), lista de riesgos (`HostRisks.tsx`), tema y enlaces de la
  terminal.
- **Color principal morado** en claro y oscuro (tokens `--beacon*` y `--focus`
  de `styles/app.css`); colores de señal independientes (verde: en marcha;
  ámbar: aviso; rojo: error).
- **Tipografías** IBM Plex Sans y JetBrains Mono, incluidas en la aplicación
  (`@fontsource`, licencia OFL-1.1).
- **Accesibilidad**: navegación con teclado, foco visible, menús con flechas,
  separadores de paneles manejables con el teclado; hay pruebas automáticas de
  accesibilidad básica (`app/a11y.test.tsx`).

## Sistema de traducciones

- `i18next` con `react-i18next`; archivos en `shared/i18n/locales/`
  (`en`, `es`, `ca`, `gl`, `eu`, `fr`, `it`, `pt`), en el orden del selector.
- Español por defecto; el idioma se guarda en la configuración.
- Variantes por contexto: claves `…_remote` para textos de servidores remotos y
  claves `…Win` para los textos propios de Windows (`shared/platform.ts`).
- La ayuda integrada (`features/help/docs/{idioma}.md`) se muestra con un
  intérprete de Markdown propio que no usa `innerHTML`.
- Una prueba (`i18n.test.ts`) comprueba que los ocho idiomas tienen las mismas
  claves.
