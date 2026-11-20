# Validación manual de Container Manage Center 1.11.0 (comandos masivos)

Solo lo nuevo de la 1.11.0: los apartados de «Gestión masiva» y los comandos
masivos. Las operaciones masivas de la 1.10.0 tienen su guía
([VALIDACION-1.10.0.md](VALIDACION-1.10.0.md)); si no la has hecho, hazla antes
con la 1.11.0 instalada.

Tiempo estimado: 20–30 minutos. Con tu usuario normal; `sudo` solo para apt.

**Usa solo contenedores de prueba creados para esto**, nunca los tuyos reales.

## 0. Antes de empezar

- [ ] `bash scripts/verify-evolution-1.11.0.sh --deb --net` en el código de la
      1.11.0 termina con «parte automática superada» (o solo con fallos que
      entiendas y aceptes, como el aviso de `cargo deny` sobre chrono).
- [ ] Instala la 1.11.0.
- [ ] Crea los contenedores de prueba (con la etiqueta `cmc.manual=1.11.0`):
      ```bash
      docker run -d --name cmc-cmd-debian --label cmc.manual=1.11.0 debian:stable-slim sleep infinity
      docker run -d --name cmc-cmd-alpine --label cmc.manual=1.11.0 alpine:3 sleep infinity
      docker create --name cmc-cmd-parado --label cmc.manual=1.11.0 alpine:3 sleep infinity
      ```

## 1. Apartados

| # | Paso | Resultado esperado |
|---|---|---|
| A1 | Abre **Gestión masiva** | Seis apartados: Resumen, Contenedores, Operaciones, Configuración masiva, Comandos masivos, Historial |
| A2 | **Resumen** | Una fila por servidor con sus contadores, que coinciden con `docker ps -a` |
| A3 | Selecciona los tres de prueba y abre **Configuración masiva** | Las políticas actuales; los botones de etiquetas, desactivados con la explicación |

## 2. Comandos masivos (con los tres de prueba seleccionados)

| # | Paso | Resultado esperado |
|---|---|---|
| C1 | **Comandos masivos**, ejemplo «Versión del sistema» | Vista previa: `cat /etc/os-release`, riesgo «Informativo». «cmc-cmd-parado» aparece como «no se ejecutará» |
| C2 | Ejecuta | Dos correctos, cada uno con su `stdout` (Debian y Alpine distintos), código 0 |
| C3 | Secuencia con `/bin/bash`: `echo hola` | Debian correcto; Alpine «Error» con «Docker no pudo lanzar el proceso» (Alpine no trae bash) |
| C4 | Secuencia con `/bin/sh`, varias líneas: `echo uno` / `echo dos >&2` / `exit 3` | Los dos con «Error», código 3, `stdout` «uno» y `stderr` «dos» |
| C5 | Ejecutable `id`, usuario `nobody` | `uid=65534(nobody)` en los dos |
| C6 | Ejecutable `pwd`, directorio `/tmp` | `/tmp` |
| C7 | Ejecutable `sleep`, argumento `30`, tiempo máximo 3 s | «Tiempo agotado» y «el proceso sigue»; `docker top cmc-cmd-debian` muestra el `sleep 30` hasta que termina. Los contenedores siguen en marcha |
| C8 | `sleep 30`, ejecuciones a la vez = 1, y **Interrumpir activas** | El primero «Cancelación solicitada», el segundo «No ejecutado» |
| C9 | Secuencia `apt-get update` | Riesgo «Administrativo»; la confirmación pide escribir el número |
| C10 | Exportar (Markdown) | Se guarda un archivo con la salida de cada contenedor |
| C11 | Guarda un comando frecuente; intenta guardar otro con `--password secreto` | El primero se guarda y se puede volver a cargar; el segundo se rechaza |
| C12 | **Ver en actividad** | El resumen «Comando masivo» y una entrada por contenedor con el `docker exec` equivalente; sin la salida |
| C13 | **Historial** | La operación aparece; al desplegarla, las órdenes de cada contenedor |

## 3. Limpieza

```bash
docker rm -f $(docker ps -aq --filter label=cmc.manual=1.11.0)
```

## Hoja de resultados

| Bloque | Correcto | Observaciones |
|---|---|---|
| 1. Apartados | ☐ | |
| 2. Comandos masivos | ☐ | |
