# Política de seguridad

## Versiones con soporte

Container Manage Center es un proyecto pequeño mantenido por una sola persona.
Solo se corrigen fallos de seguridad en la **última versión publicada**.

| Versión | Soporte |
|---|---|
| 1.9.x (Linux) | Sí |
| 1.9.x (Windows, en pruebas) | Sí, pero la versión de Windows aún no se distribuye |
| Anteriores | No: actualiza a la última versión |

## Cómo avisar de una vulnerabilidad

**No abras un issue público** para un problema de seguridad.

Usa el aviso privado de vulnerabilidades de GitHub:

1. Ve a la pestaña **Security** del repositorio
   (<https://github.com/SergiioAlfaro/container-manage-center/security>).
2. Pulsa **Report a vulnerability**.
3. Describe el problema con:
   - la versión de Container Manage Center y el sistema (Debian, Ubuntu,
     Windows) y la versión de Docker;
   - los pasos para reproducirlo y qué impacto tiene (por ejemplo, lectura de
     secretos, ejecución de órdenes, acceso al socket de Docker);
   - si lo conoces, una propuesta de corrección.

No incluyas contraseñas, claves privadas ni datos reales en el aviso: usa
valores de ejemplo.

## Qué puedes esperar

- Confirmación de que se ha recibido el aviso en cuanto sea posible.
- Una valoración del problema y, si se confirma, una corrección en una versión
  nueva, con el aviso de seguridad de GitHub correspondiente.
- Reconocimiento en el aviso, si lo deseas.

No hay plazos garantizados: es un proyecto mantenido en tiempo libre.

## Alcance

Entran en el alcance, entre otros:

- exposición de secretos (contraseñas de SSH o de registros, variables de
  entorno marcadas como secretas) en el registro de actividad, la interfaz o
  los archivos de la aplicación;
- ejecución de órdenes no previstas en este equipo o en un servidor remoto;
- formas de saltarse las confirmaciones de operaciones destructivas o los
  avisos de montajes y configuraciones peligrosas;
- verificación de la clave del servidor SSH (`known_hosts`) que se pueda eludir;
- lectura o escritura de archivos fuera de las reglas de rutas de Compose.

No entran en el alcance:

- que quien tiene acceso al socket de Docker (o es miembro del grupo `docker`
  o `docker-users`) pueda controlar el equipo: es una propiedad de Docker, no
  de la aplicación, y la aplicación lo advierte;
- problemas de Docker, Docker Desktop, OpenSSH, WebKitGTK, WebView2 u otras
  dependencias que no dependan de cómo los usa la aplicación (avísalos a sus
  proyectos).

## Documentación de seguridad

- [docs/security/SECURITY_AUDIT.md](docs/security/SECURITY_AUDIT.md): auditoría
  de seguridad del proyecto y hallazgos `CMC-SEC-*`.
- [docs/security/SECURITY_CHECKLIST.md](docs/security/SECURITY_CHECKLIST.md):
  comprobaciones automáticas y manuales.
- [docs/security/REMEDIATION_PLAN.md](docs/security/REMEDIATION_PLAN.md): plan
  de corrección.

Container Manage Center no está certificado ni auditado por terceros.
