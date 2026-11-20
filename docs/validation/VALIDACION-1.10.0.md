# Validación manual de Container Manage Center 1.10.0 (gestión masiva)

Solo lo nuevo de la 1.10.0: el apartado «Gestión masiva». Lo demás no cambia y
**no se repite**.

Tiempo estimado: 20–30 minutos. Con tu usuario normal; `sudo` solo para apt.

**Usa solo contenedores de prueba creados para esto**, nunca los tuyos reales:
la guía elimina contenedores.

## 0. Antes de empezar

- [ ] `bash scripts/verify-evolution-1.10.0.sh --deb --net` en el código de la
      1.10.0 termina con «parte automática superada».
- [ ] Instala la 1.10.0 (`bash scripts/install.sh` o
      `sudo apt install ./container-manage-center_1.10.0_amd64.deb`).
- [ ] Crea los contenedores de prueba (todos con la etiqueta `cmc.manual=1.10.0`):
      ```bash
      docker volume create cmc-manual-vol
      for n in 1 2 3; do docker run -d --name cmc-manual-$n --label cmc.manual=1.10.0 --label grupo=a alpine:3 sleep infinity; done
      docker run -d --name cmc-manual-4 --label cmc.manual=1.10.0 --label grupo=b -v cmc-manual-vol:/datos alpine:3 sleep infinity
      docker create --name cmc-manual-5 --label cmc.manual=1.10.0 --label grupo=b alpine:3 /no-existe
      docker run -d --rm --name cmc-manual-rm --label cmc.manual=1.10.0 alpine:3 sleep infinity
      ```

## 1. Tabla y filtros

| # | Paso | Resultado esperado |
|---|---|---|
| T1 | Abre **Gestión masiva** | Aparece en «Recursos», después de «Contenedores». Los contadores coinciden con `docker ps -a` (total, en ejecución, detenidos, en pausa) |
| T2 | Filtro de etiqueta `cmc.manual=1.10.0` | Solo los 6 de prueba |
| T3 | Filtro de etiqueta `grupo=b`; luego búsqueda `manual-4`; luego **Quitar filtros** | 2; luego 1; luego todos |
| T4 | Pulsa el título «Nombre» dos veces | Orden ascendente y descendente |
| T5 | Columna «Reinicio» | `no` en todos; `--rm` debajo en `cmc-manual-rm` |
| T6 | Si tienes un servidor remoto conectado | Sus contenedores aparecen con el nombre del servidor en la columna «Servidor»; un servidor sin conectar aparece en el aviso de arriba |

## 2. Operaciones (con el filtro `cmc.manual=1.10.0`)

| # | Paso | Resultado esperado |
|---|---|---|
| O1 | Selecciona todos los filtrados. Mira la barra | «Iniciar 1» (el 5), «Detener 5», «Pausar 5», «Reanudar 0» desactivado con su explicación |
| O2 | **Pausar** → confirma | La confirmación lista 5 afectados y 1 omitido (el 5, detenido). Resultado: 5 correctos, 1 omitido. `docker ps` los muestra «Paused» |
| O3 | Selecciona todos, **Reanudar** | 5 correctos |
| O4 | Selecciona todos, **Iniciar** | El 5 falla con el error de Docker (comando `/no-existe`); el resto, omitidos. Resultado parcial |
| O5 | **Política de reinicio** → «Si falla», 3 intentos | La vista previa muestra `no → on-failure:3`; `cmc-manual-rm` omitido por `--rm`. Después, `docker inspect -f '{{.HostConfig.RestartPolicy}}' cmc-manual-1` da `{on-failure 3}` |
| O6 | Repite O5 | Todos omitidos: «ya tiene esa política» |
| O7 | **Etiquetas** | Explica que Docker no permite cambiarlas; muestra las actuales; no hay botón para aplicar nada |
| O8 | Selecciona todos, **Detener**, y durante el progreso pulsa **Cancelar lo pendiente** (pon antes «Operaciones a la vez» = 1) | Los que estaban en curso terminan; el resto queda «Cancelado». Nada se reintenta |
| O9 | **Ver en actividad** | El panel de actividad muestra solo las órdenes de esa operación (distintivo «Masiva») y el resumen «Operación masiva» |

## 3. Eliminar

| # | Paso | Resultado esperado |
|---|---|---|
| E1 | Arranca `cmc-manual-1` (`docker start cmc-manual-1`). Selecciona los de prueba y **Eliminar** sin forzar | Los activos aparecen como omitidos («marca Forzar»). El botón no se activa hasta escribir el número exacto |
| E2 | Escribe el número y confirma | Se eliminan los detenidos; `cmc-manual-1` sigue |
| E3 | Selecciona el resto, **Eliminar** con «Forzar» | Se eliminan todos |
| E4 | `docker volume ls \| grep cmc-manual-vol` | El volumen **sigue existiendo** |

## 4. Limpieza

```bash
docker rm -f $(docker ps -aq --filter label=cmc.manual=1.10.0) 2>/dev/null
docker volume rm cmc-manual-vol
```

## Hoja de resultados

| Bloque | Correcto | Observaciones |
|---|---|---|
| 1. Tabla y filtros | ☐ | |
| 2. Operaciones | ☐ | |
| 3. Eliminar | ☐ | |
