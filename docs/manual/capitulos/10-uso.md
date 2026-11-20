# Uso de la aplicación

Las capturas de este capítulo son **reales**: se tomaron el 16 de noviembre
de 2026 de la versión 1.9.1 de Linux (las de «Gestión masiva», el 20 de
noviembre de 2026 de la 1.10.0, con contenedores creados para la ocasión), en el entorno de pruebas (Ubuntu 24.04,
Docker 29.9.2), con contenedores, redes, volúmenes y un proyecto Compose
creados para la ocasión. Ese entorno no tiene acceso a Docker Hub, por eso las
imágenes que se ven son las de prueba del proyecto (`cmc-test/…`), y se
ejecuta como root, por eso el diagnóstico muestra `root (0)` como usuario. Las
capturas de Windows están pendientes (ver
[Validación pendiente](#validación-pendiente)).

## Primer inicio

1. Abre la aplicación (menú de aplicaciones o `container-manage-center`).
2. La aplicación busca Docker en modo automático y, si conecta, el indicador de
   la barra superior muestra `Docker 29.x` en verde y el **Dashboard** se
   llena con los datos del motor.
3. Elige el idioma y el tema en **Configuración › Apariencia** (por defecto,
   español y el tema del sistema).

![Dashboard conectado a Docker 29.](imagenes/captura-inicio.png){width=100%}

## Configuración de la conexión

Si el indicador está en rojo («Sin conexión con Docker»), el **Dashboard**
muestra el **diagnóstico**: la causa, los pasos para resolverla con las
órdenes para copiar, los candidatos que se han comprobado y lo que se ha
detectado instalado.

![Diagnóstico sin conexión: causa, pasos y candidatos comprobados.](imagenes/captura-diagnostico.png){width=100%}

Para fijar un socket concreto: **Configuración › Conexión con Docker ›
Socket personalizado**, **Probar conexión** (no guarda nada) y **Guardar y
conectar**.

## Gestión de contenedores

1. **Contenedores** lista todos los contenedores con su estado, imagen,
   puertos y proyecto Compose. Filtra con **Todos / En ejecución /
   Detenidos** o busca por nombre, imagen, ID o proyecto.
2. Los botones de cada fila son las acciones válidas para su estado (detener,
   reiniciar, pausar, iniciar, eliminar).
3. El menú **…** (o el botón derecho) ofrece **Editar configuración** y
   **Crear imagen a partir de este contenedor**.

![Lista de contenedores con filtros y acciones.](imagenes/captura-contenedores.png){width=100%}

## Gestión masiva

1. **Gestión masiva** (en «Recursos») muestra los contenedores de todos los
   servidores conectados, con los contadores arriba. Pulsa un contador para
   filtrar por ese estado, o usa la búsqueda y los filtros de imagen, etiqueta
   y servidor.
2. Marca las casillas (o la del título para toda la página) y, en la barra que
   aparece, **Seleccionar los N filtrados** si quieres todas las páginas.
3. Cada botón de la barra dice a cuántos de la selección se aplica. Al pulsarlo
   se abre la confirmación con los afectados, los omitidos y su motivo. Para
   eliminar, escribe el número de contenedores; los volúmenes y las imágenes no
   se tocan.
4. Durante la ejecución se ve el progreso; **Cancelar lo pendiente** deja de
   enviar órdenes. Al terminar, cada contenedor tiene su resultado y **Ver en
   actividad** muestra solo las órdenes de esa operación.

![Gestión masiva: indicadores, filtros y barra de acciones con seis contenedores seleccionados.](imagenes/captura-gestion-masiva.png){width=100%}

![Resultado de una detención masiva: cada contenedor con su resultado y su duración.](imagenes/captura-gestion-masiva-resultado.png){width=100%}

## Creación

1. **Contenedores › Crear contenedor** abre el asistente.
2. En **General**, elige la imagen (las locales aparecen como sugerencias) y
   el nombre; completa las demás pestañas si hace falta.
3. Revisa a la derecha el **equivalente** (`docker run`), los **problemas** y
   los **riesgos para el equipo**. Los riesgos altos y críticos hay que
   aceptarlos uno a uno.
4. Pulsa **Crear** o **Crear e iniciar**.

![Asistente de creación: montar `/etc` exige aceptar un riesgo alto antes de crear.](imagenes/captura-crear-contenedor.png){width=100%}

## Edición

1. **… › Editar configuración**. Las pestañas con cambios llevan un punto.
2. **Revisar cambios** compara la configuración actual y la nueva e indica qué
   se aplica al momento y qué exige recrear.
3. Si hay que recrear, acepta los riesgos que se muestran y confirma. La copia
   del contenedor original queda en **General › Copias de seguridad**.

## Inspección

En el detalle de un contenedor (pulsa su nombre), **Inspección JSON** muestra
la respuesta completa de `docker inspect`, con botón Copiar. Las imágenes,
redes y volúmenes tienen la misma pestaña.

## Registros

**Registros** del detalle: salida en directo (stderr en rojo), número de líneas
anteriores, filtro, marcas de tiempo, seguir el final y copiar.

![Registros en directo de un contenedor.](imagenes/captura-registros.png){width=100%}

## Terminal

- En el detalle, **Terminal › Abrir terminal** inicia una shell (`bash` o
  `sh`) en el contenedor.
- El apartado **Terminales** permite hasta 12 sesiones a la vez, en
  contenedores locales o remotos y en servidores (**Shell SSH del
  servidor**), con distribución automática en paneles.

![Dos terminales a la vez en el apartado Terminales.](imagenes/captura-terminales.png){width=100%}

## Imágenes

**Imágenes** lista las imágenes locales con su uso. **Descargar imagen** pide
la referencia y las credenciales (las de Docker, ninguna o usuario y
contraseña solo para esa descarga) y muestra el progreso por capa.
**Construir imagen** abre el asistente de construcción (contexto, nombres,
argumentos, opciones).

![Lista de imágenes con su uso.](imagenes/captura-imagenes.png){width=100%}

## Redes

**Redes › Crear red** (nombre, driver, subred, IPv6, opciones) muestra la
orden y la petición exactas antes de crear. **Conectar** une un contenedor
(con alias o IP fija); **Desconectar** pide confirmación.

![Redes con sus contenedores y subredes.](imagenes/captura-redes.png){width=100%}

## Volúmenes

**Volúmenes** lista los volúmenes y quién los monta; **Calcular tamaños** pide
a Docker el espacio de cada uno. **Crear volumen** ofrece los tipos habituales
(local, tmpfs, NFS, carpeta).

![Volúmenes con los contenedores que los montan.](imagenes/captura-volumenes.png){width=100%}

## Compose

1. **Docker Compose › Abrir proyecto…**: elige el `compose.yaml` (o
   **Nuevo proyecto** para crearlo desde una plantilla).
2. En el proyecto: **Servicios** (estado de cada uno), **Desplegar** (con el
   comando exacto y los riesgos de cada servicio), **Salida** en vivo,
   **Registros**, **Archivo** (editor con **Validar** y **Guardar**) y
   **Configuración** resuelta.
3. **Eliminar** (`docker compose down`) pide confirmación y, opcionalmente,
   borra los volúmenes con nombre.

![Proyecto Docker Compose con sus servicios.](imagenes/captura-compose.png){width=100%}

## Conexiones remotas

1. **Servidores › Añadir servidor**: nombre, dirección, usuario y puerto;
   autenticación; Docker del servidor.
2. **Probar conexión**: siete comprobaciones. La primera vez, confirma la
   **huella** del servidor comparándola con la que te dé su administrador.
3. **Conectar** y elige el servidor en el **selector** de la barra superior:
   todos los apartados pasan a ese servidor.

![Servidores: el equipo local conectado; los servidores remotos se añaden con «Añadir servidor».](imagenes/captura-servidores.png){width=100%}

## Diagnóstico

- Del equipo local: el **Dashboard** sin conexión, o **Configuración ›
  Conexión con Docker › Probar conexión**.
- De un servidor remoto: **Servidores › Diagnóstico**, que distingue servidor
  inaccesible, SSH detenido, credenciales incorrectas, huella cambiada, Docker
  detenido, socket inaccesible, falta de permisos y versión incompatible.

## Panel de actividad

El botón **Actividad** (o Ctrl+Mayús+A) abre el panel. Cada entrada muestra la
orden `docker` equivalente; al desplegarla se ven la **operación real**
(`POST /containers/<id>/stop?t=10` sobre el socket), el resultado y, si
falló, el error de Docker con su código HTTP.

![Panel de actividad con la petición real y la orden equivalente.](imagenes/captura-actividad.png){width=100%}

## Atajos de teclado

| Atajo | Acción |
|---|---|
| Ctrl+B | Contraer o expandir la barra lateral |
| Ctrl+Mayús+A | Abrir o cerrar el panel de actividad |
| Escape | Cerrar un diálogo |
