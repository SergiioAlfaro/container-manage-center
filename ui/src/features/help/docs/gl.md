## 1. Que é Container Manage Center {#about}

Container Manage Center é unha aplicación de escritorio para administrar **Docker Engine** en Linux sen depender do terminal. Todo o que amosa son datos reais que lle pide a Docker nese momento, e cada acción é unha operación real: non se simula nada.

- Fala con Docker Engine polo seu **socket Unix**, coa API oficial.
- Executa **Docker Compose** co seu programa oficial, con argumentos separados e sen shell.
- Cada operación aparece no **panel de actividade** coa súa orde `docker` equivalente, para que saibas exactamente que se fixo.

A xanela ten unha **barra lateral** cos apartados (contráese con Ctrl+B), a **área de traballo**, unha **barra superior** co indicador de conexión e o botón **Actividade**, e o **panel de actividade** á dereita (Ctrl+Maiús+A).

## 2. Requisitos do sistema e dependencias {#requirements}

- **Sistema**: Debian 12 ou superior, ou Ubuntu 22.04 ou superior, de 64 bits (x86_64).
- **Docker Engine 20.10 ou posterior** (API 1.41+), en modo normal ou rootless.
- **Docker Compose v2 ou posterior**, só para o apartado Docker Compose (como complemento `docker compose` ou binario `docker-compose` v2). Compose v1 non se admite.
- **Dependencias de execución**: WebKitGTK 4.1, GTK 3, glibc e bibliotecas relacionadas. Instálaas APT co paquete.

A aplicación non necesita Internet. Só as operacións que fai Docker Engine contra rexistros remotos (descargar imaxes, buscar en Docker Hub, construír con imaxes base que non estean no equipo) precisan que Docker teña rede.

## 3. Instalación e primeira execución {#install}

Instala o paquete con APT, que resolve as dependencias:

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

A aplicación aparece no menú como **Container Manage Center**. Execútase co teu usuario normal: **non a executes con sudo** (négase a arrancar como root).

Ao abrila, busca Docker Engine automaticamente. Se o atopa, o **Panel** amosa o seu estado; se non, amosa o diagnóstico coa causa e os pasos para resolvela.

Para actualizar, instala o paquete novo da mesma forma: consérvanse a túa configuración e o teu historial.

## 4. Configuración inicial {#first-setup}

Non cómpre configurar nada para comezar: a conexión con Docker é automática. En **Configuración** podes escoller:

- O **tema** (Sistema, Claro ou Escuro) e o **idioma**. A cor principal é o morado.
- Se o panel de actividade se abre só cando falla unha operación e cantas operacións conserva.
- Se se garda o historial de operacións e eventos entre sesións.
- Un socket concreto de Docker, se non queres a detección automática.

Todo se garda ao momento en `~/.config/container-manage-center/config.toml`.

## 5. Conexión con Docker Engine {#connection}

O **indicador de conexión** da barra superior amosa o estado:

| Aspecto | Significado |
|---|---|
| Punto verde e `Docker 29.x` | Conectado. Ao pasar o rato vese o socket |
| `Conectando con Docker…` | Detectando ou conectando |
| Vermello, `Sen conexión con Docker` | Sen conexión. Prémeo para ver o diagnóstico |

En modo **automático**, a aplicación proba nesta orde: a variable `DOCKER_HOST` (só `unix://`), o contexto activo de Docker, o teu socket rootless e o socket do sistema `/var/run/docker.sock`. Vixía a conexión cada 10 segundos e volve conectar soa cando Docker volve estar dispoñible.

A aplicación nunca pide sudo, non cambia permisos nin grupos e non executa por ti as instrucións do diagnóstico. O teu usuario necesita poder abrir o socket: con Docker rootless ou pertencendo ao grupo `docker` (o que na práctica equivale a ter privilexios de administración).

## 6. Docker rootless e conexións mediante socket {#rootless}

Con **Docker rootless** o daemon execútase co teu usuario e o socket está en `$XDG_RUNTIME_DIR/docker.sock` (normalmente `/run/user/1000/docker.sock`). A aplicación detéctao soa, tamén a través do contexto `rootless`, e indícao coa etiqueta **rootless**. É a opción recomendada por seguridade.

En **Configuración › Conexión con Docker** podes:

- Escoller **Socket personalizado** e escribir unha ruta absoluta.
- **Probar a conexión**: comproba a ruta sen gardala.
- **Gardar e conectar**: aplica a configuración.
- **Volver conectar**: repite a detección.

As conexións `tcp://` non se admiten. Os servidores remotos administranse por SSH desde **Servidores** (sección seguinte).

## 7. Servidores remotos por SSH {#remote-servers}

Desde **Servidores** podes administrar o Docker doutros equipos Linux aos que teñas acceso por **SSH**, á vez que o deste equipo. A conexión usa o cliente SSH do sistema (`ssh`) e executa `docker system dial-stdio` no servidor: Docker **nunca** se expón na rede (nada de TCP 2375) e non se abre ningún porto.

**Requisitos do servidor**: servidor SSH en marcha, Docker Engine 20.10 ou posterior (API 1.41) e un usuario que poida usar Docker. Para Compose, `docker compose` ou `docker-compose` no servidor. A aplicación **non instala Docker, non cambia permisos, non toca a configuración SSH nin abre portos**: se falta algo, dicheche que e como resolvelo.

**Engadir servidor** (asistente): datos xerais (nome, anfitrión, usuario, porto 22), autenticación, Docker (predeterminado, un socket concreto ou un contexto) e **Probar conexión**, que comproba sete pasos: DNS, conexión SSH, identidade do servidor, autenticación, Docker dispoñible, permisos e compatibilidade da API.

- **Autenticación**: axente SSH (recomendado), chave privada, chave con frase de paso ou contrasinal (desaconsellado). A chave privada **nunca se copia**: só se garda a súa ruta.
- **Credenciais**: a frase de paso ou o contrasinal pídense ao conectar. Se escolles **lembralo**, gárdase no **almacén seguro do sistema** (GNOME Keyring, KWallet…); se non hai ningún, pídese cada vez e **nunca** se garda en texto plano. Podes borralo ao editar o servidor.
- **Identidade do servidor**: a primeira vez amósase a **pegada** (`SHA256:…`) para que a comprobes e a confirmes. Se cambia, a conexión detense cun aviso de posible **ataque de intermediario**; só se substitúe despois de revisala. Se a chave anterior está no teu `~/.ssh/known_hosts`, a aplicación non modifica ese ficheiro e dáche a orde para facelo ti.

**Varios servidores**: o **selector** da barra superior indica en que servidor traballas, co seu estado, e distingue os remotos. Ao cambiar de servidor, toda a pantalla pasa a ese servidor. **Vista xeral** amosa todos á vez (estado, versión, contedores e recursos). Cada servidor é independente: un caído non afecta aos demais, e cada operación vai ao servidor no que a iniciaches. As confirmacións indican **servidor e enderezo**, e o panel de actividade amosa o servidor de cada operación e permite filtrar por el.

**Nun servidor remoto** funcionan contedores, imaxes, redes, volumes, terminais e Compose como en local. As rutas (ficheiros Compose, cartafoles dos bind mounts) son **do servidor**: escríbense a man e compróbanse alí; antes de escribir un ficheiro no servidor confírmase o destino. En **Terminais** tamén podes abrir a **shell SSH do servidor**, un tipo de sesión á parte.

**Desconectar** pecha a sesión SSH e os seus terminais; **eliminar o perfil** borra só os datos da aplicación (perfil, credenciais e identidade gardada). Ningunha das dúas cousas toca os contedores, imaxes, redes, volumes nin datos do servidor.

Pertencer ao grupo `docker` do servidor equivale a ter acceso de **root** nel: decídeo co seu administrador; Docker rootless é a alternativa máis segura.

## 8. Panel e diagnóstico {#dashboard}

Con conexión, o **Panel** amosa datos obtidos nese momento: versión de Docker Engine e da API, socket e orixe da conexión, contedores (en execución, en pausa, detidos), imaxes, datos do equipo (sistema, núcleo, CPU, memoria), almacenamento e execución (controlador, directorio de datos, cgroups, rexistros) e os avisos do daemon. **Actualizar** volve pedilos.

Sen conexión amosa o **diagnóstico**: a causa concreta, os pasos para resolvela (coas ordes para copiar nun terminal), os sockets comprobados co seu resultado e o que se detectou instalado. Os botóns **Volver conectar** e **Configurar a conexión** están na cabeceira.

## 9. Xestión de contedores {#containers}

O apartado **Contedores** lista todos os contedores, como `docker ps -a`: estado, nome e ID curto, proxecto Compose, imaxe, estado de Docker, portos publicados e antigüidade. Podes filtrar por **Todos / En execución / Detidos** e buscar por nome, imaxe, ID ou proxecto. O menú **⋯** de cada fila (ou o botón dereito) ofrece **Editar a configuración** e **Crear unha imaxe a partir deste contedor**.

| Estado | Accións dispoñibles |
|---|---|
| En execución | Deter, Reiniciar, Pausar, Eliminar |
| En pausa | Retomar, Deter, Eliminar |
| Creado ou detido | Iniciar, Eliminar |

Deter e reiniciar agardan 10 segundos a que o proceso remate antes de forzalo, como Docker. Tras cada acción consúltase o estado real do contedor. Se Docker rexeita unha acción, a súa mensaxe aparece enriba da táboa e no panel de actividade. As listas actualízanse soas cando algo cambia en Docker, tamén desde o terminal.

## 10. Creación, duplicación, inspección e eliminación de contedores {#container-lifecycle}

**Crear contedor** abre un asistente con seis lapelas (só a imaxe é obrigatoria): Xeral, Ambiente, Rede, Almacenamento, Recursos e Avanzado.

- **Orde** e **Punto de entrada** escríbense como nun terminal e divídense en argumentos, que ves debaixo. Non se executa ningunha shell.
- O panel **Equivalente** amosa o `docker run` equivalente e o corpo exacto que se envía á API.
- **Revisión** marca os erros (impiden crear) e as advertencias (portos privilexiados, cartafoles delicados con escritura, modo privilexiado…).
- **Crear** crea o contedor sen inicialo; **Crear e iniciar** tamén o inicia.

Ao premer o nome dun contedor ábrese o seu **detalle**: Resumo, Rexistros, Terminal, Estatísticas e Inspección JSON (a resposta completa de `docker inspect`, co botón Copiar).

**Duplicar** abre o asistente coa configuración do contedor e o nome `<nome>-copia`, e avisa dos axustes que non se trasladan.

**Eliminar** pide confirmación. Un contedor en execución ou en pausa só se elimina se marcas **Detelo á forza e eliminalo**. Opcionalmente elimínanse os seus volumes anónimos; os volumes con nome e os cartafoles vinculados non se tocan. Non se pode desfacer.

## 11. Editar a configuración dun contedor {#container-edit}

**Editar a configuración** está no menú **⋯** de cada contedor (tamén co botón dereito sobre a fila) e na cabeceira do detalle. A configuración organízase en lapelas: **Xeral**, **Rede**, **Almacenamento**, **Variables**, **Portos**, **Recursos** e **Avanzado**. Cada cambio indica como se aplica:

| Cambio | Como se aplica |
|---|---|
| Nome | Directo (`docker rename`) |
| Política de reinicio, CPU, memoria e memoria reservada | Directo (`docker update`), sen reiniciar o contedor |
| Conectar ou desconectar redes | Directo e ao momento (`docker network connect` / `disconnect`) |
| Variables de ambiente, portos, volumes e cartafoles, descrición | Require **recrear** o contedor |
| Quitar un límite de CPU ou de memoria | Require recrealo (`docker update` non o sabe quitar) |
| Imaxe, orde, usuario, directorio de traballo e as demais opcións de **Avanzado** | Só lectura: Docker non permite cambialas. Usa **Duplicar** |

- **Rede**: amosa as redes, a IP, a MAC e os alias. Docker non permite cambiar os alias nin a IP dunha conexión existente: hai que desconectar e volver conectar. Se é a única rede do contedor, avísase antes de desconectala.
- **Variables**: os valores que parecen segredos ocúltanse na pantalla e nunca se gardan no panel de actividade.
- **Descrición**: gárdase na etiqueta `cmc.description` do contedor.

**Revisar os cambios** compara a configuración actual coa nova: que cambia (antes e despois, elemento a elemento), que se mantén e que require recrear. Non se aplica nada ata que o confirmes.

**Recreación segura.** Cando un cambio esixe recrear, o asistente:

1. Comproba a imaxe, os conflitos de nome e de portos con outros contedores e os motivos que impiden recrear con seguridade: contedores de Compose ou Swarm, creados con `--rm`, en pausa ou reiniciándose. Neses casos non deixa continuar.
2. Amosa os riscos que debes aceptar un a un: ficheiros escritos fóra dos volumes (non pasan ao contedor novo), montaxes que deixan de estar montadas, contedores que usan a rede deste…
3. Ao confirmar: detén o contedor se está en marcha, cámbialle o nome a `<nome>-cmc-backup-<data>` (copia de seguridade), crea o novo con **toda** a configuración orixinal máis os teus cambios, conecta as súas redes e, se estaba en marcha, inicia o contedor e comproba que segue funcionando uns segundos.
4. Se algo falla, elimina o contedor novo (sen tocar os seus volumes), devólvelle o nome ao orixinal e volve inicialo.

**Nunca se eliminan volumes, cartafoles nin imaxes.** Os volumes anónimos vólvense montar co seu contido. A copia queda detida en **Xeral › Copias de seguridade**, desde onde podes **Restaurala** (o contedor actual apártase, non se elimina) ou **Eliminala** cando xa non a necesites. Todas as operacións quedan no panel de actividade, sen valores sensibles.

## 12. Crear unha imaxe a partir dun contedor {#container-commit}

**Crear unha imaxe a partir deste contedor** (menú **⋯** da lista e cabeceira do detalle) equivale a `docker commit`: garda o estado actual do sistema de ficheiros do contedor como unha imaxe nova.

- Indica o **repositorio** e a **etiqueta** (baleira = `latest`); opcionalmente, o **autor** e unha **mensaxe**. Antes de crear vese o nome completo e valídase.
- **Non inclúe os volumes nin os cartafoles montados**: só a capa de escritura do contedor. O diálogo lista as montaxes que quedan fóra.
- A imaxe **si garda a configuración do contedor**, incluídas as súas variables de ambiente (tamén as que conteñen contrasinais ou tokens): non a publiques nun rexistro se contén segredos.
- Se o contedor está en execución, de forma predeterminada **pórase en pausa** uns segundos mentres se crea a imaxe, para que sexa coherente; podes desmarcalo. Nunca se detén.
- Se xa existe unha imaxe con ese nome, avísase e hai que aceptalo expresamente: a anterior non se elimina, queda sen etiqueta.
- O contedor orixinal non se modifica. Ao rematar, **Ver a imaxe** abre o seu detalle en Imaxes.

## 13. Rexistros, estatísticas e terminais {#logs-stats-terminal}

No detalle dun contedor:

- **Rexistros**: saída en directo (stderr en vermello). Podes escoller cantas liñas anteriores cargar, filtrar, ver as marcas de tempo, seguir o final, copiar o visible e deixar de seguir. Consérvanse as últimas 10 000 liñas.
- **Estatísticas**: CPU, memoria, rede, disco e procesos cada segundo, calculados como `docker stats`.
- **Terminal**: **Abrir terminal** inicia unha shell con `docker exec -it` (Automática usa `bash` se existe e, se non, `sh`; tamén podes escoller unha). **Pechar a sesión** remata a shell e os procesos que deixou dentro do contedor.

Son operacións reais que aparecen no panel de actividade mentres están abertas e péchanse ao saír do detalle. Para traballar con varios terminais á vez, usa o apartado **Terminais**.

## 14. Xestión de imaxes {#images}

O apartado **Imaxes** lista as imaxes locais, como `docker image ls`: etiquetas, ID curto, tamaño, antigüidade e que contedores as usan. Filtros **Todas / En uso / Sen uso / Sen etiqueta** e busca por etiqueta ou ID.

- **Detalle**: Resumo (cos valores sensibles das variables ocultos), Capas (como `docker image history`), Contedores que a usan e Inspección JSON.
- **Etiquetar** engade un nome `repositorio:etiqueta`.
- **Eliminar**: se a imaxe ten varias etiquetas podes quitar só unha; a imaxe completa esixe forzar. Docker non elimina imaxes de contedores en execución.
- **Limpar** elimina as imaxes sen etiqueta que non usa ningún contedor ou, cunha confirmación adicional, todas as que non se usan.

## 15. Descarga e construción de imaxes {#pull-build}

**Descargar imaxe** pide a referencia (`nginx:1.27`, `usuario/app`, `rexistro.exemplo.com:5000/app:v2`; sen etiqueta úsase `latest`). Credenciais:

| Opción | Que fai |
|---|---|
| As de Docker | Usa o gardado con `docker login` (ou o seu *credential helper*) |
| Ningunha | Descarga anónima |
| Usuario e contrasinal | Só para esta descarga: non se gardan nin aparecen no panel de actividade |

O panel **Descargas** amosa o progreso total e por capa; mentres tanto podes cambiar de apartado e **Cancelar**. **Buscar en Docker Hub** busca como `docker search` (faino Docker Engine, que necesita Internet).

**Construír imaxe** abre un asistente: contexto (o cartafol que se envía, respectando `.dockerignore`) e Dockerfile, nomes (`--tag`), argumentos (`--build-arg`), metadatos (`--label`), etapa final, sen caché, buscar imaxes base novas e construtor (**BuildKit** ou **clásico**). Vense os pasos co seu estado e a saída como `docker build --progress=plain`; pódese cancelar.

## 16. Redes de Docker {#networks}

O apartado **Redes** lista as redes, como `docker network ls`, co controlador, as subredes e os contedores. As redes **bridge**, **host** e **none** son predefinidas e non se poden eliminar.

- **Crear rede**: nome, controlador (`bridge`, `macvlan`, `ipvlan`, `overlay` ou outro), rede interna, enderezos IPv4 (subrede, pasarela e rango) e IPv6, opcións do controlador e metadatos. Vense a orde equivalente e a petición exacta, e os erros márcanse antes de enviar nada a Docker.
- **Conectar** un contedor (en marcha ou detido), con alias e IP fixa opcionais. **Desconectar** pide confirmación.
- **Eliminar** e **Limpar** avisan se hai contedores detidos que teñen configurada a rede e que despois non poderán arrancar.

## 17. Volumes {#volumes}

O apartado **Volumes** lista os volumes, como `docker volume ls`, co seu controlador e os contedores que os montan. Os anónimos márcanse como tales. **Calcular os tamaños** pídelle a Docker o espazo de cada un (como `docker system df -v`); non se fai automaticamente porque pode tardar.

- **Crear volume**: nome (opcional) e tipo: local, tmpfs, NFS, un cartafol do equipo ou outro controlador.
- **Detalle**: directorio, opcións, metadatos, tamaño, contedores coa súa ruta e modo, e inspección JSON. **Crear un contedor con el** móntao en `/data`.
- **Eliminar**: Docker non elimina un volume que monte calquera contedor, aínda que estea detido.
- **Limpar**: elimina os volumes anónimos sen uso ou, con confirmación, todos os volumes sen uso.

## 18. Directorios vinculados (bind mounts) {#binds}

Este apartado amosa os cartafoles e ficheiros do teu equipo montados dentro de contedores, agrupados por ruta: tipo, permisos e propietario, se existen, que contedores os montan, onde e con que acceso. Sinálanse os que requiren atención: o **socket de Docker** (dálle o control de Docker ao contedor), a raíz ou directorios do sistema e as rutas que xa non existen.

Docker non permite engadir un cartafol a un contedor que xa existe. **Vincular un cartafol…** abre o selector de cartafoles e despois o asistente de contedores con ese cartafol montado en `/mnt/<nome>`.

## 19. Docker Compose {#compose}

O apartado **Docker Compose** necesita Compose v2 ou posterior; se non está instalado, explica como instalalo. Todas as operacións executan o Docker Compose oficial e aparecen no panel de actividade coa orde exacta.

A lista amosa os proxectos con contedores en Docker (tamén os despregados desde o terminal) e os que engades, co seu estado (en marcha, parcial, detido ou sen despregar), o cartafol e os servizos.

- **Abrir proxecto…**: escolle o seu `compose.yaml` (e os ficheiros adicionais na orde na que se combinan).
- **Novo proxecto**: escolle un cartafol, escribe o contido (pártese dun modelo) e **Crear proxecto**: valídase con Docker Compose antes de crear `compose.yaml`.
- **Quitar da lista** só o quita da lista; non toca Docker nin os ficheiros.

## 20. Xestión de proxectos e perfís de Compose {#compose-projects}

No detalle dun proxecto:

- **Servizos**: cada servizo coa súa imaxe, estado, contedores, portos e **perfís**. Cada fila permite despregar, iniciar, deter, reiniciar ou ver os rexistros dese servizo.
- **Despregar** (`docker compose up --detach`): todos os servizos ou os escollidos, **perfís** que activar, construír antes, política de descarga de imaxes, volver crear contedores e eliminar orfos. O diálogo amosa a orde exacta.
- **Iniciar, Deter, Reiniciar**, **Descargar imaxes** e **Construír**.
- **Eliminar** (`docker compose down`): contedores e redes do proxecto; opcionalmente os seus volumes con nome (pérdense os seus datos) e as súas imaxes. Hai que confirmalo.
- **Saída** da operación en curso (pódese **Interromper**) e **Rexistros** dos servizos en directo.
- **Ficheiro**: edita os ficheiros; **Validar** compróbaos con Docker Compose sen gardar, e **Gardar** valida antes e non sobrescribe se outro programa os cambiou.
- **Configuración**: a configuración final segundo Docker Compose. Pode conter segredos, así que está oculta ata que a pides e non se garda no panel de actividade.

## 21. Terminais simultáneos {#terminals}

O apartado **Terminais** permite abrir e ver á vez varios terminais reais (`docker exec -it`) en contedores en execución, no mesmo contedor ou noutros.

- **Novo terminal**: escolle un contedor en execución, a shell (Automática, bash ou sh) e, se queres, un nome para a sesión.
- Cada panel amosa o **contedor**, o **nome da sesión** e un **indicador de conexión** (Conectando, Conectado, Rematado ou Erro), con botóns para **cambiar o nome**, **maximizar** e **pechar**. Un dobre clic no nome tamén o cambia.
- **Distribución**: un terminal ocupa todo o espazo; dous van en dúas columnas; tres, un á esquerda e dous á dereita; catro, en grade 2×2; con máis de catro, en filas de dous con desprazamento vertical.
- Os **separadores** arrástranse co rato. Co foco nun separador, as frechas móveno, Inicio e Fin lévano aos extremos e o dobre clic reparte o espazo a partes iguais.
- Cada sesión é independente: a súa propia shell, a súa entrada e saída, o seu tamaño e o seu estado. O que fas nunha non afecta ás demais.
- As sesións **seguen abertas ao cambiar de apartado**. **Pechalos todos** pide confirmación. Admítense ata 12 sesións á vez.
- Ao **pechar** unha sesión remata a súa shell e os procesos que deixou en segundo plano dentro do contedor. Se o contedor se **detén** ou se **elimina**, o panel indícao; unha sesión rematada pódese **volver abrir** se o contedor segue existindo.
- Ao **pechar a aplicación** rematan todas as sesións.

O panel de actividade rexistra a apertura, o peche e os erros de cada sesión, pero non o que se escribe nin a saída do terminal.

## 22. Panel de actividade e rexistro de operacións {#activity}

Cada operación aparece no **panel de actividade**: hora, distintivo **API** (petición a Docker Engine polo socket), **CLI** (proceso, como `docker compose` ou `apt-get`) ou **APP** (operación da propia aplicación, como un cambio de idioma, ou o resumo dunha recreación cuxos pasos aparecen á parte), tipo, recurso, duración e estado. A liña principal é a orde equivalente, que podes copiar. Ao despregar unha entrada ves a operación executada de verdade, o resultado, o erro e o seu código HTTP se fallou, e as saídas dos procesos.

O apartado **Eventos e rexistros** amosa os eventos de Docker en directo, os gardados, a consulta a Docker dun período (`docker events --since … --until …`; Docker só conserva os últimos 256) e o **historial de operacións** desta e doutras sesións.

O historial garda que se fixo, cando e con que resultado, pero nunca o corpo das peticións, a orde nin as saídas, porque poden conter contrasinais. Está en `~/.local/state/container-manage-center/`, só lexible polo teu usuario.

## 23. Configuración, temas e idiomas {#settings}

- **Aparencia**: tema (Sistema, Claro, Escuro) e idioma, nesta orde: **English**, **Español** (predeterminado), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** e **Português**. Cada idioma aparece co seu propio nome. O idioma gárdase, mantense ao volver abrir a aplicación e os seus cambios quedan no panel de actividade. A cor principal é o **morado** (desde a versión 1.0.0 non se pode escoller outra; se tiñas o verde azulado, cámbiase a morado sen perder o resto da configuración).
- **Panel de actividade**: abrilo ante erros, operacións conservadas (50–10 000) e **Gardar o historial entre sesións**.
- **Conexión con Docker** e comprobación de **Docker Compose**.
- **Actualizacións**: onde se publican as versións e **Comprobar agora**. A aplicación nunca descarga nin instala nada.
- **Sistema**: versións da aplicación, do paquete, de WebKitGTK e glibc, do sistema e de Docker; **Copiar** xera un informe sen datos sensibles.
- **Desinstalación e compoñentes** e **Axuda e documentación**.
- **Restablecer as preferencias**, tras confirmalo. Non cambia a conexión con Docker.

| Atallo | Acción |
|---|---|
| Ctrl+B | Contraer ou expandir a barra lateral |
| Ctrl+Maiús+A | Abrir ou pechar o panel de actividade |
| Escape | Pechar un diálogo |

As mensaxes que xera o propio Docker (por exemplo, un erro de Docker Engine) e a saída dos programas amósanse tal como chegan, sen traducir. Os resumos técnicos que o motor da aplicación escribe no panel de actividade (por exemplo, como rematou un terminal) e as mensaxes de erro con datos concretos xéranse en castelán en todos os idiomas.

## 24. Desinstalación e xestión de dependencias {#uninstall}

**Configuración › Desinstalación e compoñentes** amosa que instalou a aplicación e permite quitalo sen usar o terminal. **Analizar** consulta dpkg e APT sen cambiar nada:

- **Paquete da aplicación** (`container-manage-center`), co seu espazo.
- **Dependencias que se poden quitar**: só as que APT instalou automaticamente **xunto coa aplicación** (segundo o seu historial) e que ningún outro paquete necesita.
- **Consérvanse**, co motivo: dependencias compartidas (cos paquetes que as usan), instaladas a man, que xa existían antes, de orixe incerta (se o historial de APT non amosa como se instalou a aplicación) e compoñentes do sistema.
- **Ferramentas externas**: Docker Engine, Docker CLI e Docker Compose. **Consérvanse sempre.**
- **Datos que non se borran**: a túa configuración, o teu historial e os datos da xanela. Tampouco se toca nada de Docker: imaxes, contedores, redes, volumes e proxectos Compose.
- A **simulación de APT** completa da desinstalación.

Podes escoller **Só Container Manage Center** ou **coas dependencias que se poden quitar**, ou marcar os paquetes un a un; o resumo indica canto espazo se recupera. **Revisar e desinstalar…** volve comprobar a selección e simula exactamente a orde: se APT fose quitar algo que non escolliches, non se pode continuar. O diálogo amosa a orde que se vai executar:

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove container-manage-center …
```

Ao confirmar, o sistema pide o teu contrasinal de administración (pkexec) **só para esa orde**; a aplicación segue sen privilexios. Se cancelas a autenticación non se quita nada. Non se usa `purge`. Todo queda no panel de actividade. Se se quita a propia aplicación, péchaa ao rematar.

Se `pkexec` non está instalado, a aplicación non pode pedir permisos: o diálogo amosa a orde equivalente con `sudo` para que a executes ti nun terminal.

## 25. Seguridade e tratamento de credenciais {#security}

- A aplicación **non se executa como root**, nunca pide sudo para funcionar e non cambia permisos do socket nin grupos.
- As ordes externas execútanse **sen shell** e con argumentos separados: o que escribes nunca se interpreta como ordes.
- **Non garda credenciais.** As dun rexistro úsanse só para unha descarga e non aparecen no panel de actividade. Con «As de Docker» úsanse as de `docker login`.
- Os valores de variables como `PASSWORD`, `TOKEN` ou `SECRET` ocúltanse na pantalla e no panel (`********`), aínda que Docker os recibe completos.
- O historial persistente nunca garda corpos de peticións, ordes nin saídas.
- As operacións destrutivas piden confirmación.
- A xanela bloquea as conexións externas. A única conexión propia é a comprobación de versións, desactivada mentres non indiques unha fonte. O botón de GitHub da Axuda só pode abrir ese enderezo, no teu navegador.
- Montar o socket de Docker nun contedor ou pertencer ao grupo `docker` dá control total do equipo: a aplicación sinálao.

## 26. Solución de problemas frecuentes {#troubleshooting}

| Causa amosada | Que facer |
|---|---|
| Docker non parece estar instalado | `sudo apt install docker.io` ou a guía oficial de Docker |
| Non se atopa Docker Engine / está detido | `sudo systemctl enable --now docker` (rootless: `systemctl --user enable --now docker`) |
| Sen permiso para usar o socket | Docker rootless, ou o grupo `docker` e volver iniciar sesión |
| Xa pertences ao grupo, pero non funciona | Pecha a sesión e volve entrar |
| Conexión de Docker non compatible | `tcp://` ou `ssh://` na conexión local: usa un socket local (`docker context use default`); para un servidor remoto, engádeo en **Servidores** |
| Docker non responde a tempo | `sudo systemctl restart docker` |
| Versión de Docker demasiado antiga | Actualiza a Docker 20.10 ou posterior |

- **Docker Compose non aparece**: instálao (Debian 13: `sudo apt install docker-compose`; Ubuntu: `sudo apt install docker-compose-v2`; repositorio de Docker: `docker-compose-plugin`) e preme **Comprobar** en Configuración.
- **Unha descarga ou busca falla**: Docker Engine necesita acceso a Internet; a mensaxe de erro é a de Docker.
- **Un terminal non se abre**: o contedor debe estar en execución e ter unha shell (`sh` ou `bash`).
- **Non se permite recrear un contedor**: o motivo aparece na revisión (contedor de Compose, creado con `--rm`, en pausa, imaxe que xa non existe…).
- **A desinstalación non ofrece unha dependencia**: úsaa outro programa, xa estaba instalada ou non se pode saber con seguridade; o motivo aparece ao seu carón.
- **Para informar dun problema**: Configuración › Sistema › **Copiar** xera un informe sen datos sensibles.

## 27. Preguntas frecuentes {#faq}

**A aplicación executa ordes `docker`?** Non: usa a API de Docker Engine. A orde que ves no panel de actividade é a equivalente, para que a poidas reproducir. Docker Compose si se executa co seu programa oficial.

**Necesito ser administrador?** Non. Só necesitas acceso ao socket de Docker. Unicamente a desinstalación de paquetes pide permisos, e só para esa orde.

**Funciona sen Internet?** Si. Só as operacións de Docker contra rexistros remotos necesitan rede.

**Que pasa cos meus contedores se pecho a aplicación?** Seguen funcionando: xestiónaos Docker. Ao pechar rematan os terminais abertos e déixanse de seguir os rexistros e as estatísticas.

**Podo perder datos ao editar un contedor?** Non se elimina ningún volume, cartafol nin imaxe, e o orixinal consérvase como copia. Só os ficheiros escritos fóra dos volumes quedan na copia, e a revisión avísao antes.

**Pérdense os meus datos ao desinstalar?** Non. Consérvanse a túa configuración, o teu historial e todo o de Docker.

**Onde se gardan a configuración e o historial?** En `~/.config/container-manage-center/` e `~/.local/state/container-manage-center/`.

**Como cambio o idioma?** En Configuración › Aparencia › Idioma.

## 28. Legal {#legal}

**Nome:** Container Manage Center (abreviado **CMC**).

**Licenza:** GNU General Public License, versión 3 ou posterior (identificador SPDX `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

Este programa é software libre: pode redistribuílo e/ou modificalo baixo os termos da Licenza Pública Xeral de GNU publicada pola Free Software Foundation, xa sexa a versión 3 da Licenza ou (á súa elección) calquera versión posterior. Distribúese coa esperanza de que sexa útil, pero SEN NINGUNHA GARANTÍA; nin sequera a garantía implícita de COMERCIABILIDADE ou IDONEIDADE PARA UN PROPÓSITO PARTICULAR. Consulte a Licenza Pública Xeral de GNU para máis detalles.

Texto completo da licenza: https://www.gnu.org/licenses/gpl-3.0.html (tamén en `/usr/share/doc/container-manage-center/LICENSE`).

**Compoñentes de terceiros.** A aplicación inclúe bibliotecas e tipografías de terceiros que conservan as súas propias licenzas (MIT, Apache-2.0, MPL-2.0, OFL-1.1 e outras) e os avisos de dereitos de autor dos seus autores; non pasan a ser GPL nin de Sergio Alfaro Lorite. Os seus textos completos están en `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md`, e o aviso de dereitos de autor do paquete en `/usr/share/doc/container-manage-center/copyright`.

**Código fonte.** O código fonte correspondente publícase xunto con cada versión do paquete (`container-manage-center-X.Y.Z-src.tar.gz`).

**Marcas.** Container Manage Center é un proxecto independente: non está afiliado a Docker, Inc. nin avalado por ela. «Docker» é unha marca de Docker, Inc. e aquí só se usa para nomear a tecnoloxía coa que traballa a aplicación (Docker Engine, a súa API, Docker Compose e a CLI de Docker).

Os mesmos datos están en **Configuración › Legal**.

## 29. Windows {#windows}

En Windows, Container Manage Center funciona igual que en Linux, con estas diferenzas:

- **Docker local**: conéctase con **Docker Desktop** pola súa canalización con nome (`\\.\pipe\docker_engine`). Docker Desktop ten que estar en marcha, e o teu usuario, no grupo local `docker-users` (engádeo o instalador de Docker Desktop).
- **Servidores remotos por SSH**: usan o cliente OpenSSH de Windows (`ssh.exe`, incluído en Windows 10 e 11). Como non admite conexións multiplexadas, cada operación abre a súa propia conexión: a primeira vez compróbase a identidade do servidor igual que en Linux, e o contrasinal ou a frase de paso gardada úsase soa. Os **servidores intermedios** (ProxyJump) aínda non están dispoñibles, e na **terminal do servidor** o cambio de tamaño da xanela non chega ao servidor.
- **Contrasinais lembrados**: no **Administrador de credenciais** de Windows (credenciais de Windows), co nome `container-manage-center/…`.
- **Copias programadas**: co **Programador de tarefas**, no cartafol «Container Manage Center», co teu usuario e sen privilexios de administrador. Execútanse coa sesión iniciada; se o equipo estaba apagado a esa hora, a copia faise en canto se poida.
- **Notificacións**: as do sistema (centro de notificacións).
- **Datos**: configuración en `%APPDATA%\container-manage-center` e historial en `%LOCALAPPDATA%\container-manage-center`.
- **Desinstalar**: desde Configuración de Windows › Aplicacións › Aplicacións instaladas. Os teus datos e Docker Desktop non se tocan. O apartado «Desinstalación e compoñentes» só existe en Debian e Ubuntu.

## 30. Xestión masiva {#bulk}

**Xestión masiva** (en «Recursos») aplica unha mesma operación a moitos contedores á vez, tamén de varios servidores.

- **Visión xeral**: total, en execución, detidos, en pausa e con erro. Premer nun contador filtra por ese estado. Reúnense os contedores deste ordenador e dos servidores remotos **conectados**; os demais indícanse nun aviso.
- **Actualización**: botón **Actualizar** e actualización automática (de 5 s a 5 min, pódese desactivar).
- **Táboa**: busca por nome ou ID; filtra por estado, imaxe, servidor e etiqueta (`clave` ou `clave=valor`); ordena premendo no título dunha columna.
- **Selección**: caixa por fila, da páxina ou de **todo o filtrado**. A barra indica a cantos contedores da selección se aplica cada operación.
- **Operacións**: iniciar, deter, reiniciar, pausar, retomar, eliminar e cambiar a política de reinicio (en quente con `docker update`; non nos creados con `--rm`). Os que non o permiten polo seu estado **omítense**, co motivo.
- **Confirmación**: operación, servidores, contedores afectados e omitidos, consecuencias e operacións á vez (1 a 16). **Eliminar** pide escribir o número de contedores e **nunca elimina volumes nin imaxes**.
- **Etiquetas**: Docker non permite cambialas nun contedor que xa existe (habería que recrealo); o diálogo explícao e amosa as actuais. Para un contedor concreto, usa **Duplicar**.
- **Resultados**: cada contedor ten o seu: correcto (confirmado por Docker), fallido (co erro), omitido, cancelado ou **descoñecido** (a conexión cortouse antes da confirmación; actualiza para ver o seu estado). Se un falla, os demais seguen; nada se desfai nin se reintenta só. **Cancelar o pendente** deixa de enviar ordes.
- **Actividade**: cada orde rexístrase co distintivo «Masiva»; **Ver na actividade** amosa só esa operación.
- **Apartados**: Resumo, Contedores, Operacións, Configuración masiva, Comandos masivos e Historial (operacións e comandos gardados, coas ordes de cada contedor).
- **Comandos masivos**: executa un mesmo comando dentro de varios contedores en execución con Docker Exec, sen pasar por ningunha shell deste ordenador. Escolle «Executable con argumentos» (un por liña, sen shell) ou «Secuencia cun intérprete» (`/bin/sh`, `bash`, `python3`…), usuario, directorio, tempo máximo e execucións á vez. A vista previa amosa os argumentos exactos e o risco; os de risco administrativo ou destrutivo piden escribir o número de contedores. Cada contedor dá o seu estado, código de saída, `stdout` e `stderr` (copiables e exportables cos segredos ocultos). Docker non permite deter un proceso xa iniciado: co tempo esgotado ou ao interromper só se deixa de agardar. Pódense gardar comandos frecuentes (nunca con segredos).
