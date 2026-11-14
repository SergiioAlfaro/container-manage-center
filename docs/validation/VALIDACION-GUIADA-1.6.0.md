# Validación manual guiada · Docker Control Center 1.6.0

Esta guía reúne en **un solo recorrido** todas las comprobaciones manuales que
siguen pendientes:

- las de la Fase 10 (F1–F8);
- las de las Etapas 1 a 5 (1.1.0–1.5.0);
- las nuevas de la Etapa 6 (1.6.0), que van marcadas como **E6**.

Así no hace falta seguir seis listas por separado. Las comprobaciones repetidas
entre listas se han unido.

**Antes de empezar:**

- Hazlo en Debian 13 con tu usuario normal (no como root), en un equipo donde
  puedas crear y borrar contenedores de prueba. **Nada de lo que se pide toca
  datos reales.** Todo lo de prueba empieza por `prueba-` y se borra al final
  (apartado 10).
- Primero, la parte automática, desde el código fuente:

  ```bash
  bash scripts/verify-evolution-1.6.0.sh --deb --net
  ```

  Sigue solo si termina con «parte automática superada».
- Ve anotando el resultado en la hoja del final (correcto, o qué ves distinto).
  Una captura ayuda cuando algo no cuadra.
- Tiempo aproximado: 2 horas. Puedes parar entre apartados.

Entre corchetes, el origen de cada comprobación (por ejemplo, [E3-M2]).

---

## 1. Instalación y preferencias

| # | Qué hacer | Qué debe pasar |
|---|---|---|
| 1.1 | Cierra la aplicación. Deja en una carpeta los dos `.deb`, `install.sh` y `SHA256SUMS`, y ejecuta `bash install.sh`. [F1, E2-M1] | «Suma SHA-256 correcta». Pide la contraseña solo para `apt`. Termina con «Docker Control Center 1.6.0 instalado» y comprueba Docker, Compose, SSH y el llavero sin cambiar nada. |
| 1.2 | Ábrela desde el menú de aplicaciones. Ayuda → Acerca de. [F1, E1-M1] | El icono es morado. La versión es **1.6.0**. |
| 1.3 | Revisa tus preferencias. [F2, E1-M1] | Siguen tu idioma, tu tema, tus servidores remotos y tus proyectos Compose. |
| 1.4 | En la barra lateral. | En Herramientas aparecen «Auditoría» y «Copias»; en Supervisión, «Monitorización» antes de «Eventos». |
| 1.5 | `sudo docker-control-center` [E2-M7] | Termina con «no debe ejecutarse como root». |
| 1.6 | Para un contenedor cualquiera desde la terminal con `docker stop` (y vuelve a arrancarlo). [E2-M1] | Su estado cambia solo en la lista: los permisos mínimos bastan. |

## 2. Aspecto

| # | Qué hacer | Qué debe pasar |
|---|---|---|
| 2.1 | Configuración → Apariencia. Ayuda → «Configuración, temas e idiomas». [F3] | Solo Tema e Idioma; no hay «Color principal» ni opciones verdes. La ayuda dice que el color principal es el morado. |
| 2.2 | En tema claro y oscuro, en Contenedores y en «Crear contenedor». [F4] | Hay respuesta visible al pasar el ratón. El foco con Tab es morado y visible. El botón principal se oscurece al pulsarlo. Lo elegido se marca en morado, lo no disponible se ve atenuado y la carga muestra su indicador. No queda nada en verde azulado. |
| 2.3 | Indicadores. [F5] | «En ejecución» y «Conectado» en verde, avisos en ámbar, errores en rojo. |
| 2.4 | Abre una terminal de un contenedor. [F6] | El cursor y la selección son morados, y también el logotipo. |

## 3. Avisos de riesgo al crear, editar y desplegar

| # | Qué hacer | Qué debe pasar |
|---|---|---|
| 3.1 | Contenedores → Crear: imagen `nginx`; Almacenamiento → `/var/run/docker.sock` en `/var/run/docker.sock`, «Solo lectura». [E1-M2] | Aparece «Riesgos para el equipo» con un aviso Crítico que explica que «solo lectura» no lo impide. «Crear» está desactivado hasta marcar «Entiendo el riesgo…». Cancela. |
| 3.2 | Mismo asistente: Avanzado → capacidades `SYS_ADMIN` y `NET_RAW`; red `host`; PID `host`; puerto 8080→80 sin IP. [E1-M3, E2-M2] | `SYS_ADMIN` Crítico, `NET_RAW` Bajo sin casilla, red y PID `host` Alto, y el puerto Bajo («todas las interfaces»). Con la IP 127.0.0.1 desaparece el aviso del puerto. Cancela. |
| 3.3 | Configuración legítima: `nginx` con una carpeta tuya en solo lectura y el puerto `127.0.0.1:8080`. Créalo con el nombre `prueba-legitimo`. [E1-M4] | No aparece «Riesgos para el equipo». |
| 3.4 | En `prueba-legitimo`: Editar configuración → añade `/etc` en `/host-etc` (solo lectura) → Revisar cambios. [E1-M5] | Riesgo Alto de credenciales; no se aplica sin aceptarlo. Vuelve atrás sin aplicar. |
| 3.5 | Compose: proyecto de prueba con un servicio que monte el socket en solo lectura, `security_opt: [seccomp=unconfined]` y `devices: [/dev/sda]` → Desplegar. [E1-M6, E2-M3] | El diálogo lista cada riesgo con el nombre del servicio, y «Desplegar» queda desactivado hasta aceptarlos. Cancela. |
| 3.6 | Compose → Abrir archivo: `~/.bashrc` y luego un `compose.yaml` normal (si es un enlace simbólico, guárdalo). [E2-M4] | `~/.bashrc` se rechaza con la explicación. El `compose.yaml` se abre y se guarda; un enlace simbólico lo sigue siendo (`ls -l`). |

## 4. Secretos, terminales, imágenes y servidor intermedio

| # | Qué hacer | Qué debe pasar |
|---|---|---|
| 4.1 | Crea `prueba-secretos` con `MYSQL_PWD=prueba1` y `DATABASE_URL=postgres://app:prueba2@db/app`. Mira el panel de actividad y el detalle. [E1-M7] | El panel muestra `MYSQL_PWD=********` y `postgres://app:********@db/app`. «prueba1» y «prueba2» no aparecen en el panel ni en el historial. En el detalle siguen ocultos hasta pulsar «Mostrar». |
| 4.2 | En una terminal: `printf '\e]8;;https://example.com\e\\enlace\e]8;;\e\\\n'` y pulsa el enlace. Prueba también `file:///etc/passwd`. [E2-M6] | Se pide confirmación con la dirección completa: «Cancelar» no abre nada y «Abrir en el navegador» la abre. Con `file://` no pasa nada. |
| 4.3 | Imágenes → Descargar en un servidor remoto (si tienes). [E2-M5] | Aparece la casilla de enviar credenciales, desmarcada. Sin marcarla, una imagen privada falla por permisos. En «Este equipo» no aparece. |
| 4.4 | Perfil con «Servidor intermedio» (solo si usas uno) → «Probar conexión». [E1-M8] | Pide la huella del intermedio (con su nombre) y después la del servidor. Si el intermedio solo admite contraseña, el error pide clave o agente y no se envía la contraseña del servidor. |
| 4.5 | Ayuda → Actualizaciones → «Ver la versión». [E2-M8] | Abre la página de github.com. |
| 4.6 | Conecta uno de tus servidores guardados y abre Contenedores en él. [F7] | Funciona como antes. |

## 5. Preparación de lo que usan los apartados 6 a 9

Ejecuta en una terminal:

```bash
docker volume create prueba-datos
docker run --rm -v prueba-datos:/d alpine sh -c 'echo hola > /d/a.txt; mkdir /d/sub; head -c 5M /dev/urandom > /d/sub/b.bin'
docker run -d --name prueba-mal --cap-add SYS_ADMIN -p 8088:80 -v /var/run/docker.sock:/var/run/docker.sock:ro -v prueba-datos:/datos nginx
docker run -d --name prueba-bien --user 101 --read-only --tmpfs /tmp --tmpfs /var/cache/nginx --tmpfs /var/run --memory 128m --cpus 0.5 --pids-limit 100 --security-opt no-new-privileges:true -p 127.0.0.1:8089:8080 nginxinc/nginx-unprivileged
docker run -d --name prueba-pg -e POSTGRES_PASSWORD=prueba -e POSTGRES_DB=app postgres:17
docker run -d --name prueba-cpu --cpus 1 alpine sh -c 'while :; do :; done'
```

En la aplicación, «Copias» → «Elegir carpeta» → crea y elige `~/CopiasPrueba`.

## 6. Auditoría

| # | Qué hacer | Qué debe pasar |
|---|---|---|
| 6.1 | Auditoría → Auditar. [E3-M2] | `prueba-mal` sale como Crítico (socket y `SYS_ADMIN`), y cada hallazgo tiene «Por qué importa» y «Recomendación». `prueba-bien` sale «Sin hallazgos». |
| 6.2 | Filtro «Crítico»; búsqueda «socket». [E3-M3] | Quedan solo esos hallazgos. |
| 6.3 | «Proponer correcciones» en `prueba-mal` y descarta. [E3-M4] | Se abre «Editar configuración» con el puerto ya en 127.0.0.1 y el aviso de la auditoría. Al descartar, `docker inspect prueba-mal` sigue igual. |
| 6.4 | Exporta a HTML y a Markdown. [E3-M5] | Pide dónde guardar; los archivos tienen `-rw-------` y el HTML se abre en el navegador. |
| 6.5 | Vulnerabilidades. [E3-M6] | Sin Trivy ni Grype, dice que no están y el resultado es «No analizado». Con Trivy y su base, cada imagen muestra recuentos y la fecha de la base. |
| 6.6 | «Auditorías anteriores»; con un servidor remoto, auditar. [E3-M7] | La de 6.1 aparece y se abre. En remoto se ven sus contenedores y las vulnerabilidades quedan desactivadas. |

## 7. Copias de seguridad

| # | Qué hacer | Qué debe pasar |
|---|---|---|
| 7.1 | Nueva copia: `prueba-datos`, la base de datos `prueba-pg`, la configuración de `prueba-mal` y la de la aplicación → «Hacer copia ahora». [E4-M3] | Sale «Completa»; «Comprobar» dice «Integridad correcta». `ls -lR ~/CopiasPrueba` muestra `drwx------` y `-rw-------`. |
| 7.2 | Lanza otra copia y cancélala enseguida. Altera un byte de un `.tar` de la copia de 7.1 y pulsa «Comprobar». [E4-M4] | La cancelada no aparece. La alterada se marca así y «Restaurar» se niega. |
| 7.3 | Haz otra copia buena y restaura solo el volumen con el nombre propuesto. Intenta restaurarlo sobre `prueba-datos`. [E4-M5] | `docker run --rm -v prueba-datos-restaurado:/d alpine ls -lR /d` muestra `a.txt` y `sub/b.bin`. Sobre `prueba-datos` no se puede, porque tiene datos. |
| 7.4 | En `prueba-pg`: `docker exec prueba-pg psql -U postgres -d app -c 'create table t(x int); insert into t values (1)'`. Haz una copia, borra la tabla y restaura solo la base de datos. [E4-M6] | Pide escribir «restaurar»; la tabla vuelve con su fila. |
| 7.5 | Guarda `prueba-datos` como tarea diaria (retención 1/0/0) → «Programar». [E4-M7] | Muestra los dos archivos de systemd y las órdenes; sin la casilla no se crea. `systemctl --user list-timers 'dcc-backup-*'` la lista. |
| 7.6 | Cierra la aplicación; `systemctl --user start dcc-backup-<tarea>.service`; ábrela. Repite. [E4-M8] | En «Historial» aparece la copia «Programada». Solo queda una de la tarea; las manuales siguen. |
| 7.7 | Desactiva, activa y quita la programación. **No elimines la tarea todavía** (se usa en el apartado 9). [E4-M9] | Los archivos `~/.config/systemd/user/dcc-backup-*` desaparecen al quitarla. |

## 8. Monitorización

| # | Qué hacer | Qué debe pasar |
|---|---|---|
| 8.1 | Monitorización → «Monitorizar este servidor». Compara con `docker stats --no-stream`. [E5-M2] | Aparecen «Última muestra…», el equipo según Docker y la tabla. La CPU de `prueba-cpu` está cerca del 100 % (±15 puntos). La memoria y el límite de `prueba-bien` coinciden (±5 %) y los PIDs son iguales. |
| 8.2 | Pulsa `prueba-cpu`; intervalo 2 s y periodo 5 min. «Comparar»: dos contenedores, métrica «Memoria». [E5-M3] | Se ven las gráficas de CPU, memoria, red, disco y PIDs, y la línea avanza cada 2 s. Al comparar, cada línea tiene su color y su trazo; con 4 marcados no deja más. |
| 8.3 | «Pausar», «Reanudar»; cierra y abre la aplicación. [E5-M4] | En pausa no hay muestras. Al reabrir, sigue activada. |
| 8.4 | `sudo systemctl stop docker`; 30 s; `sudo systemctl start docker`. [E5-M5] | No se bloquea; dice «Sin conexión». Vuelve sola y en «1 h» se ve un tramo rayado. |
| 8.5 | Alertas: CPU 50/20/2 → Guardar → «Probar notificación». [E5-M6] | Aparece el aviso de prueba en el escritorio. Llega **una** «CPU alta en prueba-cpu» y no se repite. |
| 8.6 | `docker run -d --name prueba-falla --restart on-failure:2 alpine sh -c 'sleep 3; exit 3'` [E5-M7] | Llegan un aviso de error y uno de reinicio. Los siguientes quedan «en silencio», también tras reabrir la aplicación. |
| 8.7 | Tras unos minutos: periodo «1 h»; `ls -l ~/.local/state/docker-control-center/monitor/local/`; panel de actividad; `top`. [E5-M8] | Hay datos en «1 h». Los archivos tienen `-rw-------`. En el panel, solo `monitor.start`/`monitor.stop`. La aplicación usa pocos % de CPU. |

## 9. Enlaces entre herramientas (nuevo en la 1.6.0) · E6

La auditoría reconoce cada contenedor por su **id** en la monitorización y en
las copias. Deja la monitorización activada (apartado 8) y las copias del
apartado 7.

| # | Qué hacer | Qué debe pasar |
|---|---|---|
| 9.1 | **E6-M1** · Auditoría → Auditar. Baja hasta «Contenedores en las demás herramientas». | Hay una fila por contenedor (no por servicios ni imágenes), con su peor hallazgo, su estado en la monitorización y su copia. |
| 9.2 | **E6-M2** · Compara el id corto de la fila de `prueba-mal` con `docker inspect -f '{{.Id}}' prueba-mal`. | El id completo empieza por esos 12 caracteres. |
| 9.3 | **E6-M3** · Fila de `prueba-mal`. | «Monitorizado (running)». «Copia del …» de 7.1 (o una posterior) con «Volumen, Configuración del contenedor» y «mismo contenedor (por id)», más el aviso de copia en caliente (el volumen estaba en uso). Si guardaste la tarea de 7.5, «En la tarea «…»». |
| 9.4 | **E6-M4** · Fila de `prueba-cpu`. | «Monitorizado (running)», con la alerta de CPU activa si sigue por encima del umbral. «Sin copia». |
| 9.5 | **E6-M5** · «Ver en Monitorización» en `prueba-cpu`. | Se abre Monitorización con las gráficas de `prueba-cpu`. |
| 9.6 | **E6-M6** · Vuelve a la auditoría y pulsa «Preparar una copia» en `prueba-pg`. | Se abre Copias → «Nueva copia» con un aviso, y con la configuración y la base de datos de `prueba-pg` marcadas (su volumen es anónimo y no se marca). **No se copia nada** hasta que pulses «Hacer copia ahora»; no lo pulses. |
| 9.7 | **E6-M7** · `docker rm -f prueba-mal` y vuelve a crearlo con la misma orden del apartado 5. Audita otra vez. | La copia de `prueba-mal` sigue, pero dice «por nombre: copia anterior a la 1.6.0 o contenedor creado de nuevo»: el id ha cambiado. |
| 9.8 | **E6-M8** · Con la monitorización desactivada para este servidor, audita otra vez. Consultar los enlaces no añade nada al panel de actividad. | Cada fila dice «Monitorización desactivada en este servidor». En el panel no aparece ninguna entrada nueva al abrir la auditoría. |

## 10. Desinstalación y limpieza

| # | Qué hacer | Qué debe pasar |
|---|---|---|
| 10.1 | Desmarca «Monitorizar este servidor». Quita la tarea de copias. | — |
| 10.2 | Limpieza: `docker rm -f prueba-mal prueba-bien prueba-pg prueba-cpu prueba-falla prueba-legitimo prueba-secretos`; `docker volume rm prueba-datos prueba-datos-restaurado`; borra `~/CopiasPrueba` y el proyecto Compose de prueba. | No queda nada de prueba. |
| 10.3 | `bash install.sh --uninstall` y vuelve a instalar. [F8] | Desaparece del menú; Docker, tus contenedores y `~/.config/docker-control-center` siguen. Al reinstalar recuperas tus preferencias y servidores. |

---

## Hoja de resultados

Copia esta tabla, anota «correcto» o lo que veas distinto y envíala junto con
`verify-evolution-1.6.0-report.txt`.

| Apartado | Resultado | Notas |
|---|---|---|
| 1. Instalación y preferencias (1.1–1.6) | | |
| 2. Aspecto (2.1–2.4) | | |
| 3. Avisos de riesgo (3.1–3.6) | | |
| 4. Secretos, terminales, imágenes, intermedio (4.1–4.6) | | |
| 6. Auditoría (6.1–6.6) | | |
| 7. Copias (7.1–7.7) | | |
| 8. Monitorización (8.1–8.7) | | |
| 9. Enlaces entre herramientas · E6 (9.1–9.8) | | |
| 10. Desinstalación y limpieza (10.1–10.3) | | |

Equivalencias con las listas anteriores:
- F1–F8 están en 1.1–1.3, 2.1–2.4, 4.6 y 10.3.
- E1-M1…M8 están en 1.2, 3.1–3.5, 4.1 y 4.4.
- E2-M1…M8 están en 1.1, 1.5, 1.6, 3.2, 3.5, 3.6 y 4.2–4.5.
- E3-M1…M8 están en 1.4, el apartado 5, 6.1–6.6 y 10.2.
- E4-M1…M10 están en el apartado 5 y 7.1–7.7.
- E5-M1…M9 están en el apartado 5, 8.1–8.7 y 10.1.
