# Introducción

## Qué es Container Manage Center

**Container Manage Center** (abreviado **CMC**) es una aplicación de escritorio
de software libre para administrar **Docker Engine**: contenedores, imágenes,
redes, volúmenes, carpetas vinculadas y proyectos **Docker Compose**, tanto en
el propio equipo como en servidores Linux remotos a los que se accede por
**SSH**.

La aplicación está escrita en **Rust** (el motor y la lógica) y en **React con
TypeScript** (la interfaz), unidas con **Tauri 2**, que dibuja la interfaz con
el motor web del sistema (WebKitGTK en Linux, WebView2 en Windows) en lugar de
incluir un navegador completo. El resultado es un programa ligero (un paquete
`.deb` de unos 8 MB) que habla directamente con la API de Docker.

Este manual describe la **versión 1.10.0**. Todo lo que se cuenta está en el
código fuente del repositorio; cuando algo no se ha podido comprobar (por
ejemplo, el funcionamiento en un Windows real), se dice expresamente.

## Qué problema resuelve

Docker se administra sobre todo desde la terminal (`docker ps`, `docker run`,
`docker compose up`…). La terminal es potente, pero:

- el estado de los recursos (qué contenedor usa qué volumen, qué red está
  vacía, qué imagen no se usa) hay que reconstruirlo con varias órdenes;
- es fácil cometer errores graves sin darse cuenta: montar el socket de Docker
  en un contenedor, publicar un puerto en todas las interfaces, borrar un
  volumen con datos;
- administrar varios servidores obliga a cambiar de contexto, de sesión SSH o
  de variable `DOCKER_HOST`;
- muchas herramientas gráficas ocultan qué hacen realmente, y eso impide
  aprender y auditar.

CMC muestra los recursos y sus relaciones en una interfaz y, para cada
operación, **la petición real** que envía a Docker y **la orden `docker`
equivalente**, de modo que lo que se hace en la aplicación se puede repetir,
auditar o automatizar desde la terminal.

## Objetivos

1. **Transparencia**: nada ocurre sin quedar registrado en el *panel de
   actividad* con su petición, su resultado y su duración.
2. **Seguridad por defecto**: confirmación de las operaciones destructivas,
   avisos de configuraciones peligrosas antes de crear nada, secretos ocultos
   en pantalla y en los registros, credenciales en el almacén seguro del
   sistema, verificación estricta de la identidad de los servidores SSH.
3. **No intrusión**: la aplicación nunca instala Docker, no cambia permisos ni
   grupos del sistema, no abre puertos y se niega a ejecutarse como
   administrador.
4. **Paridad local y remota**: un servidor remoto se administra con las mismas
   funciones que el equipo local.
5. **Accesibilidad e idiomas**: ocho idiomas, tema claro y oscuro, navegación
   con teclado.

## A quién va dirigido

| Perfil | Uso típico |
|---|---|
| Desarrolladores | Levantar y depurar proyectos Compose, ver registros, abrir terminales en los contenedores, construir imágenes |
| Administradores de sistemas | Supervisar varios servidores, auditar la seguridad de los contenedores, programar copias de seguridad |
| Usuarios avanzados | Administrar Docker sin memorizar órdenes, entendiendo qué se hace |
| Estudiantes | Aprender Docker viendo la orden equivalente de cada operación |
| Colaboradores | Estudiar el código (este manual describe su organización) |

## Ventajas frente a la terminal

| Tarea | Solo con la terminal | Con CMC |
|---|---|---|
| Ver qué contenedores usan un volumen | `docker ps -a --filter volume=…` por cada volumen | Columna en la lista de volúmenes |
| Crear un contenedor | Escribir y revisar un `docker run` largo | Asistente con validación, riesgos y la orden equivalente |
| Detectar configuraciones peligrosas | Revisar `docker inspect` a mano | Avisos al crear y auditoría de seguridad |
| Varios servidores | Contextos, `DOCKER_HOST` o sesiones SSH | Selector de servidor; todas las funciones en cada uno |
| Copias de seguridad de volúmenes y bases de datos | Scripts propios | Copias con manifiesto SHA-256, restauración y tareas programadas |
| Saber qué se ha hecho | Historial del shell | Panel de actividad e historial de operaciones |

CMC **no sustituye** a la terminal: muestra siempre la orden equivalente y
trabaja sobre los mismos recursos (lo creado desde la terminal aparece al
momento en la aplicación, gracias a los eventos de Docker).

## Plataformas

| Plataforma | Estado en la 1.10.0 |
|---|---|
| Debian 13 y 12 (x86_64) | Admitidas; paquete `.deb` (en Debian 12, el paquete compatible) |
| Ubuntu 24.04 y 22.04 (x86_64) | Admitidas; regresión automática en Ubuntu 24.04 con Docker 29, 26 y 24; instalación limpia del paquete en las dos |
| Windows 10 y 11 (x86_64) | **En pruebas**: en la 1.9.1 compilaba y sus pruebas unitarias pasaban con Wine; la 1.10.0 no se ha compilado ni probado para Windows, y ninguna versión se ha ejecutado aún en un Windows real |

El capítulo [Pruebas y control de calidad](#pruebas-y-control-de-calidad) detalla qué se ha
probado en cada plataforma.

## Tecnologías

| Capa | Tecnología | Función |
|---|---|---|
| Aplicación de escritorio | Tauri 2 | Ventana, comunicación entre la interfaz y Rust, empaquetado |
| Lógica | Rust (edición 2021; mínimo 1.80, probado con 1.97), tokio | Motor de Docker, SSH, copias, auditoría, monitorización |
| API de Docker | bollard 0.21 | Cliente de la API de Docker Engine |
| Interfaz | React 19, TypeScript 5.8 | Componentes y estado |
| Compilación de la interfaz | Vite 7, Tailwind CSS 4 | Empaquetado y estilos |
| Estado de la interfaz | TanStack Query 5, Zustand 5 | Caché de datos de Docker y estado local |
| Traducciones | i18next, react-i18next | Ocho idiomas |
| Terminal | xterm.js 5.5 | Terminales de contenedores y servidores |
| Pruebas | cargo test, Vitest 5, Testing Library | Pruebas unitarias e integración |
