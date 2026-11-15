# Lista de comprobación de seguridad

Se usa antes de cada versión y al revisar cada cambio. La marca indica el
estado comprobado en la auditoría de la versión 1.0.0 (12-11-2026), actualizado
con la **1.1.0** (Etapa 1 de evolución, 12-11-2026) y la **1.2.0** (Etapa 2,
13-11-2026), la **1.3.0** (Etapa 3, auditoría de seguridad), la **1.4.0**
(Etapa 4, copias de seguridad), la **1.5.0** (Etapa 5, monitorización) y la
**1.6.0** (Etapa 6, integración). Las líneas cambiadas llevan su evidencia
entre corchetes (prueba automática que lo demuestra; `verify-evolution-1.8.0.sh`
las ejecuta todas):

- ✅ cumple;
- ⚠️ cumple en parte (ver hallazgo);
- ❌ no cumple;
- ➖ no aplica.

Referencias: OWASP ASVS 5.0.0, modelo de seguridad de Tauri 2, guía de
seguridad de Docker Engine y RustSec.

## Tauri y la interfaz

- ✅ CSP sin `unsafe-eval` ni orígenes remotos en `script-src`. `connect-src`
  limitado a IPC.
- ✅ `style-src 'unsafe-inline'` se mantiene **justificado** (CMC-SEC-15, R-17,
  D-091): React usa atributos `style` para tamaños calculados; `script-src`
  sigue sin `unsafe-inline` ni `unsafe-eval`, así que no permite ejecutar código.
- ✅ `freezePrototype: true` y `withGlobalTauri: false`.
- ✅ Capacidades mínimas: solo `core:event:allow-listen`/`allow-unlisten`, el
  diálogo de abrir, el portapapeles y el enlace del autor (**CMC-SEC-17**,
  corregido en la 1.2.0, D-091) [`only_the_minimal_tauri_permissions_are_granted`,
  con la ACL real].
- ✅ `opener` limitado a URL concretas. Ninguna capacidad de `shell` ni `fs`.
- ✅ No hay `dangerouslySetInnerHTML`, `innerHTML`, `eval`, `new Function` ni
  `href` dinámico. El Markdown se convierte en elementos de React.
- ✅ La interfaz no navega a orígenes remotos.
- ✅ Enlaces de la salida de terminal (OSC 8) con un manejador propio: solo
  http(s), confirmación con la dirección completa y apertura validada en Rust
  (**CMC-SEC-12**, 1.2.0) [`terminal_links_are_http_only`,
  `ui/src/shared/ui/terminalLinks.test.tsx`].
- ✅ La página de una versión solo se abre desde `github.com` o la fuente de
  actualizaciones (**CMC-SEC-16**, 1.2.0)
  [`release_pages_only_from_github_or_the_own_source`, IPC `phase9_`].
- ✅ ESLint con reglas de seguridad (`no-eval`, `no-new-func`, sin `innerHTML` ni
  `dangerouslySetInnerHTML`) en la verificación (1.2.0) [`npm run lint`].
- ✅ La salida de la terminal se trata como datos (base64 → xterm) y nunca se
  ejecuta.

## Órdenes IPC y el proceso Rust

- ✅ Cada orden valida sus argumentos en Rust (tipos, longitudes, nombres,
  referencias de imagen).
- ✅ Las órdenes de archivos limitan las rutas a lo necesario: solo YAML, sin
  carpetas ocultas ni del sistema, también tras los enlaces, en el equipo y en
  los servidores (**CMC-SEC-06**, 1.2.0, D-089) [`compose::file::tests`,
  `compose_files_via_ipc_create_read_save_and_conflict`,
  `live_ssh::compose_runs_on_the_server_with_server_files`].
- ✅ El guardado de Compose conserva los enlaces simbólicos (**CMC-SEC-13**,
  1.2.0) [`symlinks_are_checked_by_their_real_target_and_kept_on_save`, prueba
  real en el servidor].
- ✅ Las operaciones destructivas requieren confirmación (`confirmed`) y
  bloqueo cuando procede.
- ✅ Los procesos se lanzan con `Command::new` y una lista de argumentos, nunca
  con `sh -c` y entrada del usuario.
- ✅ Los bloques `unsafe` se limitan a llamadas a libc (`getuid`, PTY, `kill`,
  `ioctl`) con argumentos controlados.
- ✅ La negativa a ejecutarse como root no se puede desactivar en el binario de
  publicación (**CMC-SEC-11**, 1.2.0) [`root_tests`; con `--deb` como root, el
  binario de publicación con `CMC_ALLOW_ROOT=1` termina con código 1].
- ✅ Hay límites de tamaño: archivos Compose de 1 MiB, respuestas de
  actualizaciones de 1 MiB, 5000 eventos, 10 000 líneas de registro, tamaño de
  terminal acotado.
- ⚠️ Límite de sesiones de terminal y flujos simultáneos (mejora P3).

## Docker Engine

- ✅ Solo socket Unix: no hay `tcp://` sin TLS.
- ✅ Aviso de montajes sensibles con rutas normalizadas, subrutas, enlaces
  simbólicos y sockets de *runtime*, aunque sean de solo lectura
  (**CMC-SEC-01**, corregido en la 1.1.0, D-086) [`cmc-engine risk::`,
  `spec::tests::mounts_are_validated`, `binds::tests::risks`,
  `live_e1_risks_and_secrets_via_ipc`].
- ✅ Aviso de `privileged` (ahora crítico y con aceptación).
- ✅ Aviso de capacidades peligrosas (`ALL`, `SYS_ADMIN`, `SYS_PTRACE`,
  `NET_ADMIN`, `DAC_OVERRIDE`, `NET_RAW`…) y de `network=host`
  (**CMC-SEC-03**, corregido en la 1.1.0) [`risk::tests::capabilities`,
  `spec::tests::resources_restart_and_security_are_validated`].
- ✅ Los riesgos altos y críticos exigen una aceptación explícita que el motor
  vuelve a comprobar en crear, editar y Compose `up` [`live_e1_risks_and_secrets_via_ipc`,
  `risk::tests::acceptance`, pruebas de interfaz del asistente, la edición y
  Compose].
- ✅ Sin falsos positivos en las configuraciones legítimas del plan (§9.5)
  [`legitimate_configurations_have_no_host_risks`,
  `compose_services_risks_without_false_positives`].
- ✅ Aviso de puertos en todas las interfaces (**CMC-SEC-08**, 1.2.0, D-088)
  [`risk::tests::namespaces_security_options_devices_and_ports`,
  `spec::tests::ports_are_validated`].
- ✅ Análisis de riesgos de Compose antes de desplegar: montajes, capacidades,
  privilegiado y red del equipo (1.1.0); `pid`/`ipc`/`userns_mode`/`uts`/`cgroup`,
  `security_opt`, `devices`, `cgroup_parent` y puertos (**CMC-SEC-04**, 1.2.0)
  [`compose_namespaces_security_devices_and_ports`, sin falsos positivos en
  `compose_services_risks_without_false_positives`].
- ✅ La recreación segura conserva una copia y se puede deshacer (D-070).
- ✅ El contexto de construcción no sigue enlaces y respeta `.dockerignore`.
- ✅ Credenciales de registro: las de este equipo no se envían a servidores
  remotos sin permiso explícito en cada operación (**CMC-SEC-07**, 1.2.0, D-090)
  [`local_credentials_reach_remote_servers_only_with_consent`, real
  `live_ssh::local_registry_credentials_reach_a_server_only_with_consent`,
  `ImagesView.test.tsx`].

## SSH y administración remota

- ✅ `StrictHostKeyChecking=yes` con `known_hosts` propio (0600) y huella
  confirmada por el usuario.
- ✅ `UpdateHostKeys=no`, `ForwardAgent=no`, `ForwardX11=no`,
  `ClearAllForwardings=yes` y `PermitLocalCommand=no` en la línea de órdenes.
- ✅ `--` antes del host. El usuario, el host, el puerto y el salto se validan.
- ✅ Órdenes remotas entrecomilladas para POSIX, con una prueba de ida y vuelta
  por `sh`.
- ✅ Requisito de shell POSIX documentado en INSTALL y la guía (CMC-SEC-18,
  1.2.0).
- ✅ El servidor intermedio (ProxyJump) recibe las mismas opciones de seguridad,
  su huella se confirma en la aplicación, solo admite clave o agente y no
  recibe el askpass (**CMC-SEC-02**, corregido en la 1.1.0, D-087). Antes de
  corregirlo se reprodujo la fuga en el laboratorio (`Failed password` en el
  registro del intermedio) [`ssh::args::tests::proxy_command_roundtrips…`,
  `live_ssh::proxy_jump_checks_both_identities_and_never_shares_the_target_secret`].
- ✅ Askpass:
  - socket 0600 en un directorio 0700;
  - comprobación de `peer_cred`;
  - nunca responde a preguntas de sí o no;
  - el secreto nunca va en argumentos ni en el entorno.
- ✅ El askpass responde solo a la pregunta del destino exacto (usuario y
  servidor, o la clave del perfil) [`ssh::askpass::tests`]. El número de
  respuestas se mantiene (64) por los canales adicionales de OpenSSH (D-087).
- ✅ Un salto definido en `~/.ssh/config` no se usa con contraseña o frase de
  paso (`jumpFromSshConfig`).
- ✅ Los diálogos muestran el servidor de destino de la operación.

## Auditoría de seguridad (1.3.0)

- ✅ Solo lectura: ninguna petición de escritura a Docker durante la auditoría y
  los contenedores no cambian (D-093) [`live_audit::audit_detects_dangerous_settings…`
  con el registro del proxy, IPC `live_e3_audit_via_ipc_is_read_only_and_saved`,
  `live_ssh::audit_of_a_remote_server_reads_its_containers_only`].
- ✅ Las correcciones solo se proponen en «Editar configuración»; nada se aplica
  sin revisar y confirmar [`AuditView.test.tsx`, `edit.test.tsx`].
- ✅ Contenedores peligrosos con todas las comprobaciones y legítimos sin
  hallazgos [`audit::checks::tests`, `live_audit`].
- ✅ Sin herramienta de vulnerabilidades, sin base o con fallo: «no analizado» o
  «falló», nunca 0; Trivy y Grype sin red ni actualizaciones (D-094)
  [`audit::vuln::tests`].
- ✅ Informe e historial sin secretos; exportación 0600 en una ruta elegida en el
  diálogo nativo (D-095) [`commands::audit::tests`,
  `audit_history_and_get_without_docker`, `AuditView.test.tsx`].

## Secretos y datos sensibles

- ✅ Contraseñas y frases de paso en Secret Service, nunca en `servers.toml`.
- ✅ Configuración, perfiles e historial en archivos 0600 dentro de
  directorios 0700, con escritura atómica.
- ✅ El historial persistente no guarda cuerpos, órdenes equivalentes ni
  salidas. Los `buildargs` se ocultan.
- ✅ Ocultación de secretos centralizada en el panel, sus eventos, el
  historial, la orden equivalente, los argumentos, las salidas, los errores
  IPC y los registros de la aplicación (`*_PWD`, `*_PASS`, `*_AUTH`, URL con
  credenciales, claves privadas, `Authorization`) (**CMC-SEC-05**, corregido en
  la 1.1.0, D-085) [`cmc-activity redact::`,
  `secrets_never_reach_records_events_or_history`, `error::tests`,
  `live_e1_risks_and_secrets_via_ipc`, `ui/src/shared/secrets.test.ts`].
- ✅ Reglas de secretos idénticas en Rust y en la interfaz
  [`redact::tests::interface_rules_match`].
- ➖ Registros de los contenedores y salida de las terminales: no se alteran
  por decisión del propietario (datos del usuario; D-085).
- ✅ Las credenciales de registro manuales solo viven en memoria. La salida de
  los *credential helpers* nunca se registra.
- ✅ Los documentos y los informes no contienen secretos.

## Dependencias y cadena de suministro

- ✅ `cargo audit`: 0 vulnerabilidades.
- ✅ Crates transitivas sin mantenimiento con excepciones justificadas en
  `deny.toml` (CMC-SEC-14, 1.2.0, D-092) [`cargo deny check`].
- ✅ `cargo deny`: licencias y orígenes correctos (solo crates.io).
- ✅ `npm audit --omit=dev`: 0. `npm audit` completo: 0 con vitest 5 y
  `minimatch` 11.0.8 fijado (CMC-SEC-10, 1.2.0).
- ✅ Comprobación automática de dependencias en la verificación: `cargo deny`,
  `npm audit` y ESLint (R-08, 1.2.0).
- ✅ Archivos de bloqueo versionados y usados con `--locked`.
- ⚠️ Las herramientas externas que se integren tendrán la versión fijada y la
  firma verificada (Trivy v0.69.4, CVE-2026-33634).

## Empaquetado y distribución

- ✅ El `.deb` contiene la licencia, el copyright y las licencias de terceros.
- ✅ `install.sh` usa `sudo` solo para `apt-get` y no ejecuta la aplicación como
  root.
- ⚠️ `SHA256SUMS` verificado por `install.sh` (no instala si no coincide o falta
  la entrada) ✅ desde la 1.2.0 [paso de `verify-evolution-1.8.0.sh --deb`];
  **sin firmar**: la firma GPG necesita la clave del propietario (CMC-SEC-09,
  decisión pendiente).
- ✅ La aplicación no instala Docker ni cambia grupos ni permisos del sistema.
- ✅ La documentación avisa de que el grupo `docker` equivale a root y
  recomienda rootless.

## Proceso para cada cambio

- [ ] Pruebas unitarias o IPC de la validación nueva.
- [ ] `bash scripts/verify-evolution-1.8.0.sh --deb` (o su sucesor) sin regresiones,
      con Docker 24, 26 y 29.
- [ ] `cargo audit`, `cargo deny check`, `npm audit` y `npm run lint` sin
      hallazgos nuevos (las excepciones de `deny.toml`, revisadas).
- [ ] Sin secretos en registros, panel, historial, capturas ni documentación.
- [ ] Traducciones en los ocho idiomas para cada aviso nuevo.
- [ ] CHANGELOG.md y DECISIONS.md actualizados. Esta lista revisada.

## Copias de seguridad y restauración (1.4.0)

- ✅ Copias solo para el usuario: carpetas 0700 y archivos 0600; destino fuera de
  las carpetas del sistema (D-096) [`backup::store::tests::complete_backup_is_private…`,
  `live_backup` (`assert_private`), `backups::tests`].
- ✅ Una copia interrumpida o cancelada nunca se da por buena ni se restaura;
  una suma alterada se detecta y la copia no se restaura (D-096)
  [`backup::store::tests::interrupted…`, `altered_missing_and_extra…`,
  `live_backup::volumes_round_trip_and_bad_backups_are_never_used`].
- ✅ Rutas del manifiesto sin `..` ni absolutas; nombres de servidor e id
  saneados [`backup::store::tests::interrupted_backups…`].
- ✅ El contenedor auxiliar nunca se arranca, no descarga ninguna imagen y no
  deja volúmenes anónimos (D-097) [`live_backup`: ningún `POST …/start` de un
  auxiliar en el registro, sin auxiliares ni volúmenes anónimos restantes].
- ✅ Credenciales de las bases de datos: las lee el guion dentro del contenedor y
  nunca pasan por la aplicación, por argumentos ni por el registro (D-098)
  [`backup::db::tests`, `live_backup` (contraseñas ausentes del registro)].
- ✅ Guiones de volcado y restauración fijos, sin concatenar datos del usuario
  [`backup::db::tests::scripts_are_constant…`].
- ✅ Configuración de contenedores sin variables secretas; `.env` de Compose
  solo con aceptación expresa; configuración de la aplicación sin secretos
  (D-101) [`live_backup`, `backup::run::tests`].
- ✅ Restauración: vista previa, confirmación, nunca sobre un volumen con datos
  ni sobre archivos existentes; contenedores nuevos sin arrancar y con
  aceptación de sus riesgos (D-099) [`live_backup`, `backup::restore::tests`,
  `BackupView.test.tsx`].
- ✅ Temporizadores de systemd solo con autorización expresa, solo archivos
  propios (con marca), `systemctl --user` sin shell y sin `sudo` (D-100)
  [`systemd::tests`, IPC `e4_jobs_and_timers_need_authorization…`].
- ✅ El modo sin ventana (`--run-backup`) aplica la misma negativa a ejecutarse
  como root que la ventana [`root_tests`] y no ejecuta dos veces la misma tarea a
  la vez [`live_e4_scheduled_job_runs_headless…`].
- ⚠️ Las copias **no están cifradas** (D-096): los volcados y volúmenes
  contienen los datos tal cual. Documentado; se recomienda un disco cifrado
  para copias fuera del equipo.
- ⚠️ systemd real y MongoDB no se han podido probar en el entorno de desarrollo
  (D-098, D-100): pendientes de la validación manual.

## Monitorización (1.5.0)

- ✅ Sin agentes ni nada instalado en los servidores: solo lecturas de la API de
  Docker (`containers/json`, `stats`, `inspect`, `info`), igual en local y por
  SSH; ninguna petición de escritura (D-102)
  [`live_monitor::metrics_match_docker_stats`: solo `GET` en el proxy].
- ✅ Muestreo acotado: `stats` de una lectura, 8 a la vez y 10 s de límite;
  un servidor sin conexión falla enseguida y no bloquea los comandos (D-102)
  [`live_monitor::disconnection_fails_fast_and_sampling_resumes`, IPC
  `e5_monitor_without_docker_shows_gaps_and_survives_a_restart`].
- ✅ Sin datos inventados: un corte es un hueco y un contador que vuelve a
  empezar deja el dato vacío (D-102, D-103)
  [`monitor::sampler::tests::cpu_and_rates_between_two_samples_like_the_cli`,
  `monitor::history::tests::ring_keeps_size_and_age_limits_and_gaps`].
- ✅ Datos en disco solo para el usuario y limitados: `monitor.json`,
  `metricas.jsonl` y `alertas.jsonl` 0600 en una carpeta 0700; retención de 1 a
  168 h, 64 MiB y 1000 alertas como máximo (D-103)
  [`monitor::tests::minute_store_filters_compacts_by_age_and_size_and_is_private`,
  `settings_are_validated_and_survive_a_restart`].
- ✅ Configuración validada en el backend (intervalo 2–60 s, retención,
  servidores existentes, reglas) [`settings_are_validated_and_survive_a_restart`,
  `monitor::alerts::tests::rules_are_validated`].
- ✅ Consumo de la aplicación limitado y medido con 50 contenedores y 3
  servidores (D-107) [`live_monitor::consumption_with_50_containers_and_3_servers`].
- ✅ Alertas sin repetición (histéresis y periodo de silencio, también tras reiniciar la aplicación) (D-104)
  [`monitor::alerts::tests` (incluido `silence_survives_a_restart_from_the_history`), `live_monitor::real_cpu_and_oom_alerts_fire_once`,
  IPC `live_e5_monitor_samples_minutes_and_event_alerts`].
- ✅ Notificaciones solo al bus de sesión local, sin complemento ni permiso
  nuevo en la interfaz; título y cuerpo limitados y sin caracteres de control
  (D-105) [`notify::tests::notifications_go_through_dbus`].
- ✅ El registro no se llena: las lecturas del muestreo no se apuntan una a una;
  solo `monitor.start`, `monitor.stop` y el cambio de configuración (D-102)
  [`live_monitor::metrics_match_docker_stats` (sin entradas del muestreo)].
- ⚠️ El aviso visible en GNOME/KDE no se ha podido probar en el entorno de
  desarrollo (D-105): pendiente de la validación manual (M6).

## Integración entre herramientas (1.6.0)

- ✅ Los enlaces solo leen: no cambian contenedores, copias ni configuración, y
  no se apuntan en el panel de actividad (D-108)
  [IPC `live_e6_audit_links_monitoring_and_backups_by_container_id`].
- ✅ Sin confusiones entre contenedores: monitorización por id (un contenedor
  con el mismo nombre y otro id no cuenta) y todo separado por servidor
  (las tareas de otro servidor no cuentan) [`links::tests`].
- ✅ La coincidencia por nombre (copia anterior a la 1.6.0 o contenedor
  recreado) se muestra como tal, nunca como «mismo contenedor»
  [`links::tests::a_recreated_container_is_only_matched_by_name`, IPC
  `live_e6_…`].
- ✅ Solo cuentan las copias completas (una interrumpida no protege nada)
  [`links::tests::only_complete_backups_count_and_duplicates_are_removed`].
- ✅ «Preparar una copia» no copia nada: solo marca una selección que el
  usuario revisa; las copias siguen con sus confirmaciones (D-108)
  [`BackupView.test.tsx`, `AuditView.test.tsx`].
- ✅ Los manifiestos nuevos solo añaden ids de contenedor (no son secretos) en
  sus detalles; el formato no cambia y las copias anteriores se siguen
  leyendo, comprobando y restaurando [IPC `live_e6_…`, regresión de copias de
  la 1.4.0].
- ✅ Servidor desconocido: la orden se rechaza como las demás [IPC `live_e6_…`].

## Identidad, licencia y distribución (1.7.0 y 1.8.0)
- ✅ El paso de los datos de una versión anterior al nombre nuevo (D-113) no
  pierde nada: las carpetas se renombran (atómico) o se copian, comprueban y
  solo entonces se borran; si existen las dos, nada se sobrescribe; las
  tareas programadas se activan con el nombre nuevo antes de quitar las
  antiguas; las contraseñas del keyring se crean con los atributos nuevos
  antes de borrar las antiguas, y las bloqueadas no piden desbloqueo al
  arrancar [`cmc-store::migrate`, `systemd.rs`, `clean-install-test.sh` con
  `UPGRADE_FROM`: sha256 de los datos antes y después].
- ✅ Solo se tocan unidades de systemd con la marca de la aplicación; las
  ajenas con el mismo prefijo no se tocan [`systemd.rs`].
- ✅ El paquete ya no instala ningún archivo con el nombre anterior y no
  arranca como root [`verify-evolution-1.8.0.sh --deb`, prueba de instalación].
- ✅ La desinstalación desde la aplicación reconoce en el historial de APT el
  paquete anterior y solo propone quitar dependencias que instaló la
  aplicación [`uninstall.rs`].
- ✅ Ninguna licencia de las dependencias distribuidas queda pendiente,
  potencialmente incompatible o indeterminada; `--check` impide publicar si
  aparece alguna (D-112) [`third-party-licenses.py --check`, `cargo deny`].
- ✅ El .tar.gz de código fuente es `git archive` del commit y no contiene
  claves, contraseñas, tokens, credenciales ni configuraciones locales
  [`DISTRIBUTION_COMPLIANCE.md` §4].
- ✅ El apartado Legal no abre programas ni enlaces: la URL de la licencia y
  la ruta de los avisos solo se copian [`LegalSettings.test.tsx`].

