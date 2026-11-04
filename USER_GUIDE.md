# Guía de usuario

> Estado: Fase 1. La aplicación todavía no se conecta a Docker. Esta guía
> describe lo que ya funciona y se amplía con cada fase.

## La ventana principal

La ventana tiene tres zonas:

1. **Barra lateral (izquierda).** Los apartados de la aplicación, agrupados en
   Recursos, Orquestación y Supervisión, con Configuración abajo. El botón
   **Contraer barra lateral** la reduce a iconos para ganar espacio; al pasar el
   ratón sobre un icono se ve su nombre.
2. **Área de trabajo (centro).** Muestra el apartado seleccionado. Los apartados
   que llegan en fases posteriores indican la fase y lo que permitirán hacer.
3. **Panel de actividad (derecha).** Se abre y se cierra con el botón
   **Actividad** de la parte superior, sin salir del apartado en el que estás.

## Panel de actividad

Cada vez que la aplicación ejecute algo sobre Docker aparecerá aquí una entrada
con:

- Hora, área y tipo de operación, y el recurso afectado.
- Un distintivo **API** (petición a Docker Engine a través del socket) o **CLI**
  (proceso lanzado, como `docker compose`).
- El comando como línea principal, que puedes copiar con el icono de la derecha.
- El estado: En curso (franja azul animada), Correcto, Error o Interrumpida.

Haz clic en una entrada para ver sus detalles:

- **Operación ejecutada**: lo que se hizo realmente, por ejemplo
  `POST /v1.47/containers/web/start` sobre `/var/run/docker.sock`.
- **Comando equivalente**: cómo harías lo mismo con el CLI de Docker. Cuando la
  operación fue por API se indica que es una traducción orientativa: la app no
  ejecutó ese comando.
- Cuerpo de la petición, salida estándar, salida de error, mensaje de error,
  código HTTP o de salida, inicio, fin y duración.

Las operaciones con error se muestran desplegadas. Puedes buscar por texto
(comandos, recursos, errores), filtrar por área y por estado, y limpiar el
historial visual con el icono de papelera (las operaciones en curso se mantienen).

## Configuración

- **Tema**: Sistema (sigue el de tu escritorio), Claro u Oscuro.
- **Idioma**: Español o English. El cambio es inmediato.
- **Abrir el panel cuando una operación falle**: activado por defecto.
- **Operaciones conservadas**: entre 50 y 10.000 (500 por defecto).

Los cambios se guardan al momento en `~/.config/docker-control-center/config.toml`.
Si no se pueden guardar, la aplicación lo indica y deshace el cambio. También se
recuerdan entre sesiones el estado de la barra lateral y del panel de actividad.

## Atajos de teclado

| Atajo | Acción |
|---|---|
| Ctrl+B | Contraer o expandir la barra lateral |
| Ctrl+Mayús+A | Abrir o cerrar el panel de actividad |
