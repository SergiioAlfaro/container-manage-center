# Sobre este manual {.unnumbered}

Este documento tiene dos partes:

- **Parte I, Manual de usuario.** Es el mismo manual que trae la aplicación en
  **Configuración › Ayuda y documentación**, con el mismo texto y la misma
  numeración: el capítulo 13 del PDF es la sección 13 de la aplicación. Se
  genera a partir del mismo archivo, así que ambos no pueden contradecirse. En
  el PDF se añaden capturas de pantalla reales de la versión actual.
- **Parte II, Anexos técnicos.** Arquitectura, estructura del código,
  integración con Docker, seguridad, instalación detallada, desarrollo,
  pruebas, GitHub, distribución, información legal completa y referencias.
  Está pensada para quien quiera compilar, revisar o distribuir el programa.

Las capturas se tomaron de la aplicación real con contenedores, redes y
volúmenes creados para la ocasión en un entorno de pruebas (Ubuntu 24.04 con
Docker 29). Ninguna muestra datos de usuarios reales.

**Funciones disponibles y requisitos externos.** Todo lo que se describe en la
Parte I está implementado en esta versión. Docker Engine, Docker Compose, el
cliente SSH, las herramientas de análisis de vulnerabilidades (Trivy o Grype)
y Docker Desktop en Windows son programas externos: la aplicación los usa si
están instalados, pero no los instala. La versión para Windows está **en
pruebas** y no se ha validado todavía en un Windows real.
