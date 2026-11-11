## 1. Què és Docker Control Center {#about}

Docker Control Center és una aplicació d’escriptori per administrar **Docker Engine** a Linux sense dependre del terminal. Tot el que mostra són dades reals que demana a Docker en aquell moment, i cada acció és una operació real: no se simula res.

- Parla amb Docker Engine pel seu **sòcol Unix**, amb l’API oficial.
- Executa **Docker Compose** amb el seu programa oficial, amb arguments separats i sense intèrpret d’ordres.
- Cada operació apareix al **tauler d’activitat** amb la seva ordre `docker` equivalent, perquè sàpigues exactament què s’ha fet.

La finestra té una **barra lateral** amb els apartats (es plega amb Ctrl+B), l’**àrea de treball**, una **barra superior** amb l’indicador de connexió i el botó **Activitat**, i el **tauler d’activitat** a la dreta (Ctrl+Maj+A).

## 2. Requisits del sistema i dependències {#requirements}

- **Sistema**: Debian 12 o superior, o Ubuntu 22.04 o superior, de 64 bits (x86_64).
- **Docker Engine 20.10 o posterior** (API 1.41+), en mode normal o rootless.
- **Docker Compose v2 o posterior**, només per a l’apartat Docker Compose (com a connector `docker compose` o binari `docker-compose` v2). Compose v1 no s’admet.
- **Dependències d’execució**: WebKitGTK 4.1, GTK 3, glibc i biblioteques relacionades. Les instal·la APT amb el paquet.

L’aplicació no necessita Internet. Només les operacions que fa Docker Engine contra registres remots (baixar imatges, cercar a Docker Hub, construir amb imatges base que no siguin a l’equip) necessiten que Docker tingui xarxa.

## 3. Instal·lació i primera execució {#install}

Instal·la el paquet amb APT, que resol les dependències:

```
sudo apt install ./docker-control-center_X.Y.Z_amd64.deb
```

L’aplicació apareix al menú com a **Docker Control Center**. S’executa amb el teu usuari normal: **no l’executis amb sudo** (es nega a arrencar com a root).

En obrir-la, busca Docker Engine automàticament. Si el troba, el **Tauler** en mostra l’estat; si no, mostra el diagnòstic amb la causa i els passos per resoldre-ho.

Per actualitzar, instal·la el paquet nou de la mateixa manera: es conserven la teva configuració i el teu historial.

## 4. Configuració inicial {#first-setup}

No cal configurar res per començar: la connexió amb Docker és automàtica. A **Configuració** pots triar:

- El **tema** (Sistema, Clar o Fosc), el **color principal** (verd blavós o morat) i l’**idioma**.
- Si el tauler d’activitat s’obre sol quan una operació falla i quantes operacions conserva.
- Si es desa l’historial d’operacions i esdeveniments entre sessions.
- Un sòcol concret de Docker, si no vols la detecció automàtica.

Tot es desa a l’instant a `~/.config/docker-control-center/config.toml`.

## 5. Connexió amb Docker Engine {#connection}

L’**indicador de connexió** de la barra superior mostra l’estat:

| Aspecte | Significat |
|---|---|
| Punt verd i `Docker 29.x` | Connectat. En passar-hi el ratolí es veu el sòcol |
| `S’està connectant amb Docker…` | S’està detectant o connectant |
| Vermell, `Sense connexió amb Docker` | Sense connexió. Prem-lo per veure el diagnòstic |

En mode **automàtic**, l’aplicació prova en aquest ordre: la variable `DOCKER_HOST` (només `unix://`), el context actiu de Docker, el teu sòcol rootless i el sòcol del sistema `/var/run/docker.sock`. Vigila la connexió cada 10 segons i torna a connectar sola quan Docker torna a estar disponible.

L’aplicació no demana mai sudo, no canvia permisos ni grups i no executa per tu les instruccions del diagnòstic. El teu usuari ha de poder obrir el sòcol: amb Docker rootless o pertanyent al grup `docker` (cosa que a la pràctica equival a tenir privilegis d’administrador).

## 6. Docker rootless i connexions mitjançant sòcol {#rootless}

Amb **Docker rootless** el dimoni s’executa amb el teu usuari i el sòcol és a `$XDG_RUNTIME_DIR/docker.sock` (normalment `/run/user/1000/docker.sock`). L’aplicació el detecta sola, també a través del context `rootless`, i ho indica amb l’etiqueta **rootless**. És l’opció recomanada per seguretat.

A **Configuració › Connexió amb Docker** pots:

- Triar **Sòcol personalitzat** i escriure una ruta absoluta.
- **Provar la connexió**: comprova la ruta sense desar-la.
- **Desar i connectar**: aplica la configuració.
- **Tornar a connectar**: repeteix la detecció.

Les connexions `tcp://` no s’admeten. Els servidors remots s’administren per SSH des de **Servidors** (secció següent).

## 7. Servidors remots per SSH {#remote-servers}

Des de **Servidors** pots administrar el Docker d'altres equips Linux als quals tinguis accés per **SSH**, alhora que el d'aquest equip. La connexió fa servir el client SSH del sistema (`ssh`) i executa `docker system dial-stdio` al servidor: Docker **mai** no s'exposa a la xarxa (res de TCP 2375) i no s'obre cap port.

**Requisits del servidor**: servidor SSH en marxa, Docker Engine 20.10 o posterior (API 1.41) i un usuari que pugui fer servir Docker. Per a Compose, `docker compose` o `docker-compose` al servidor. L'aplicació **no instal·la Docker, no canvia permisos, no toca la configuració SSH ni obre ports**: si falta alguna cosa, et diu què i com resoldre-ho.

**Afegeix un servidor** (assistent): dades generals (nom, amfitrió, usuari, port 22), autenticació, Docker (predeterminat, un sòcol concret o un context) i **Prova la connexió**, que comprova set passos: DNS, connexió SSH, identitat del servidor, autenticació, Docker disponible, permisos i compatibilitat de l'API.

- **Autenticació**: agent SSH (recomanat), clau privada, clau amb frase de pas o contrasenya (desaconsellada). La clau privada **mai no es copia**: només se'n desa el camí.
- **Credencials**: la frase de pas o la contrasenya es demanen en connectar. Si tries **recordar-la**, es desa al **magatzem segur del sistema** (GNOME Keyring, KWallet…); si no n'hi ha, es demana cada vegada i **mai** no es desa en text pla. La pots esborrar en editar el servidor.
- **Identitat del servidor**: la primera vegada es mostra l'**empremta** (`SHA256:…`) perquè la comprovis i la confirmis. Si canvia, la connexió s'atura amb un avís de possible **atac d'intermediari**; només se substitueix després de revisar-la. Si la clau anterior és al teu `~/.ssh/known_hosts`, l'aplicació no modifica aquest fitxer i et dona l'ordre per fer-ho tu.

**Diversos servidors**: el **selector** de la barra superior indica en quin servidor treballes, amb el seu estat, i distingeix els remots. En canviar de servidor, tota la pantalla passa a aquest servidor. **Vista general** els mostra tots alhora (estat, versió, contenidors i recursos). Cada servidor és independent: un de caigut no afecta els altres, i cada operació va al servidor on la vas iniciar. Les confirmacions indiquen **servidor i adreça**, i el tauler d'activitat mostra el servidor de cada operació i permet filtrar-hi.

**En un servidor remot** funcionen contenidors, imatges, xarxes, volums, terminals i Compose com en local. Els camins (fitxers Compose, carpetes dels bind mounts) són **del servidor**: s'escriuen a mà i s'hi comproven; abans d'escriure un fitxer al servidor es confirma la destinació. A **Terminals** també pots obrir la **shell SSH del servidor**, un tipus de sessió a part.

**Desconnectar** tanca la sessió SSH i els seus terminals; **eliminar el perfil** esborra només les dades de l'aplicació (perfil, credencials i identitat desada). Cap de les dues coses toca els contenidors, imatges, xarxes, volums ni dades del servidor.

Pertànyer al grup `docker` del servidor equival a tenir-hi accés de **root**: decideix-ho amb el seu administrador; Docker rootless és l'alternativa més segura.

## 8. Tauler i diagnòstic {#dashboard}

Amb connexió, el **Tauler** mostra dades obtingudes en aquell moment: versió de Docker Engine i de l’API, sòcol i origen de la connexió, contenidors (en execució, en pausa, aturats), imatges, dades de l’equip (sistema, nucli, CPU, memòria), emmagatzematge i execució (controlador, directori de dades, cgroups, registres) i els avisos del dimoni. **Actualitzar** les torna a demanar.

Sense connexió mostra el **diagnòstic**: la causa concreta, els passos per resoldre-la (amb les ordres per copiar en un terminal), els sòcols comprovats amb el resultat i el que s’ha detectat instal·lat. Els botons **Tornar a connectar** i **Configurar la connexió** són a la capçalera.

## 9. Gestió de contenidors {#containers}

L’apartat **Contenidors** llista tots els contenidors, com `docker ps -a`: estat, nom i ID curt, projecte Compose, imatge, estat de Docker, ports publicats i antiguitat. Pots filtrar per **Tots / En execució / Aturats** i cercar per nom, imatge, ID o projecte. El menú **⋯** de cada fila (o el botó dret) ofereix **Editar la configuració** i **Crear una imatge a partir d’aquest contenidor**.

| Estat | Accions disponibles |
|---|---|
| En execució | Aturar, Reiniciar, Pausar, Eliminar |
| En pausa | Reprendre, Aturar, Eliminar |
| Creat o aturat | Iniciar, Eliminar |

Aturar i reiniciar esperen 10 segons que el procés acabi abans de forçar-lo, com Docker. Després de cada acció es consulta l’estat real del contenidor. Si Docker rebutja una acció, el seu missatge apareix damunt de la taula i al tauler d’activitat. Les llistes s’actualitzen soles quan alguna cosa canvia a Docker, també des del terminal.

## 10. Creació, duplicació, inspecció i eliminació de contenidors {#container-lifecycle}

**Crear un contenidor** obre un assistent amb sis pestanyes (només la imatge és obligatòria): General, Entorn, Xarxa, Emmagatzematge, Recursos i Avançat.

- **Ordre** i **Punt d’entrada** s’escriuen com en un terminal i es divideixen en arguments, que veus a sota. No s’executa cap intèrpret.
- El quadre **Equivalent** mostra el `docker run` equivalent i el cos exacte que s’envia a l’API.
- **Revisió** marca els errors (impedeixen crear) i les advertències (ports privilegiats, carpetes delicades amb escriptura, mode privilegiat…).
- **Crear** crea el contenidor sense iniciar-lo; **Crear i iniciar** també l’inicia.

En prémer el nom d’un contenidor s’obre el seu **detall**: Resum, Registres, Terminal, Estadístiques i Inspecció JSON (la resposta completa de `docker inspect`, amb el botó Copiar).

**Duplicar** obre l’assistent amb la configuració del contenidor i el nom `<nom>-copia`, i avisa dels paràmetres que no es traslladen.

**Eliminar** demana confirmació. Un contenidor en execució o en pausa només s’elimina si marques **Aturar-lo a la força i eliminar-lo**. Opcionalment s’eliminen els seus volums anònims; els volums amb nom i les carpetes vinculades no es toquen. No es pot desfer.

## 11. Editar la configuració d’un contenidor {#container-edit}

**Editar la configuració** és al menú **⋯** de cada contenidor (també amb el botó dret sobre la fila) i a la capçalera del detall. La configuració s’organitza en pestanyes: **General**, **Xarxa**, **Emmagatzematge**, **Variables**, **Ports**, **Recursos** i **Avançat**. Cada canvi indica com s’aplica:

| Canvi | Com s’aplica |
|---|---|
| Nom | Directe (`docker rename`) |
| Política de reinici, CPU, memòria i memòria reservada | Directe (`docker update`), sense reiniciar el contenidor |
| Connectar o desconnectar xarxes | Directe i a l’instant (`docker network connect` / `disconnect`) |
| Variables d’entorn, ports, volums i carpetes, descripció | Cal **recrear** el contenidor |
| Treure un límit de CPU o de memòria | Cal recrear-lo (`docker update` no el sap treure) |
| Imatge, ordre, usuari, directori de treball i la resta d’opcions d’**Avançat** | Només lectura: Docker no permet canviar-les. Fes servir **Duplicar** |

- **Xarxa**: mostra les xarxes, la IP, la MAC i els àlies. Docker no permet canviar els àlies ni la IP d’una connexió existent: cal desconnectar i tornar a connectar. Si és l’única xarxa del contenidor, s’avisa abans de desconnectar-la.
- **Variables**: els valors que semblen secrets s’amaguen a la pantalla i no es desen mai al tauler d’activitat.
- **Descripció**: es desa a l’etiqueta `dcc.description` del contenidor.

**Revisar els canvis** compara la configuració actual amb la nova: què canvia (abans i després, element per element), què es manté i què cal recrear. No s’aplica res fins que ho confirmes.

**Recreació segura.** Quan un canvi exigeix recrear, l’assistent:

1. Comprova la imatge, els conflictes de nom i de ports amb altres contenidors i els motius que impedeixen recrear amb seguretat: contenidors de Compose o Swarm, creats amb `--rm`, en pausa o reiniciant-se. En aquests casos no deixa continuar.
2. Mostra els riscos que has d’acceptar un per un: fitxers escrits fora dels volums (no passen al contenidor nou), muntatges que deixen d’estar muntats, contenidors que fan servir la xarxa d’aquest…
3. En confirmar: atura el contenidor si està en marxa, li canvia el nom a `<nom>-dcc-backup-<data>` (còpia de seguretat), crea el nou amb **tota** la configuració original més els teus canvis, en connecta les xarxes i, si estava en marxa, l’inicia i comprova que continua funcionant uns segons.
4. Si alguna cosa falla, elimina el contenidor nou (sense tocar-ne els volums), torna el nom a l’original i el torna a iniciar.

**No s’eliminen mai volums, carpetes ni imatges.** Els volums anònims es tornen a muntar amb el seu contingut. La còpia queda aturada a **General › Còpies de seguretat**, des d’on la pots **Restaurar** (el contenidor actual s’aparta, no s’elimina) o **Eliminar** quan ja no la necessitis. Totes les operacions queden al tauler d’activitat, sense valors sensibles.

## 12. Crear una imatge a partir d’un contenidor {#container-commit}

**Crear una imatge a partir d’aquest contenidor** (menú **⋯** de la llista i capçalera del detall) equival a `docker commit`: desa l’estat actual del sistema de fitxers del contenidor com una imatge nova.

- Indica el **repositori** i l’**etiqueta** (buida = `latest`); opcionalment, l’**autor** i un **missatge**. Abans de crear-la es veu el nom complet i es valida.
- **No inclou els volums ni les carpetes muntades**: només la capa d’escriptura del contenidor. El diàleg llista els muntatges que en queden fora.
- La imatge **sí que desa la configuració del contenidor**, incloses les seves variables d’entorn (també les que contenen contrasenyes o tokens): no la publiquis en un registre si conté secrets.
- Si el contenidor està en execució, per defecte es **posa en pausa** uns segons mentre es crea la imatge, perquè sigui coherent; ho pots desmarcar. No s’atura mai.
- Si ja existeix una imatge amb aquest nom, s’avisa i cal acceptar-ho expressament: l’anterior no s’elimina, es queda sense etiqueta.
- El contenidor original no es modifica. En acabar, **Veure la imatge** n’obre el detall a Imatges.

## 13. Registres, estadístiques i terminals {#logs-stats-terminal}

Al detall d’un contenidor:

- **Registres**: sortida en directe (stderr en vermell). Pots triar quantes línies anteriors carregar, filtrar, veure les marques de temps, seguir el final, copiar el que es veu i deixar de seguir. Es conserven les últimes 10 000 línies.
- **Estadístiques**: CPU, memòria, xarxa, disc i processos cada segon, calculats com `docker stats`.
- **Terminal**: **Obrir un terminal** inicia un intèrpret amb `docker exec -it` (Automàtic fa servir `bash` si existeix i, si no, `sh`; també en pots triar un). **Tancar la sessió** acaba l’intèrpret i els processos que ha deixat dins del contenidor.

Són operacions reals que apareixen al tauler d’activitat mentre estan obertes i es tanquen en sortir del detall. Per treballar amb diversos terminals alhora, fes servir l’apartat **Terminals**.

## 14. Gestió d’imatges {#images}

L’apartat **Imatges** llista les imatges locals, com `docker image ls`: etiquetes, ID curt, mida, antiguitat i quins contenidors les fan servir. Filtres **Totes / En ús / Sense ús / Sense etiqueta** i cerca per etiqueta o ID.

- **Detall**: Resum (amb els valors sensibles de les variables amagats), Capes (com `docker image history`), Contenidors que la fan servir i Inspecció JSON.
- **Etiquetar** afegeix un nom `repositori:etiqueta`.
- **Eliminar**: si la imatge té diverses etiquetes en pots treure només una; la imatge completa exigeix forçar-ho. Docker no elimina imatges de contenidors en execució.
- **Netejar** elimina les imatges sense etiqueta que no fa servir cap contenidor o, amb una confirmació addicional, totes les que no es fan servir.

## 15. Baixada i construcció d’imatges {#pull-build}

**Baixar una imatge** demana la referència (`nginx:1.27`, `usuari/app`, `registre.exemple.com:5000/app:v2`; sense etiqueta es fa servir `latest`). Credencials:

| Opció | Què fa |
|---|---|
| Les de Docker | Fa servir el que s’ha desat amb `docker login` (o el seu *credential helper*) |
| Cap | Baixada anònima |
| Usuari i contrasenya | Només per a aquesta baixada: no es desen ni apareixen al tauler d’activitat |

El quadre **Baixades** mostra el progrés total i per capa; mentrestant pots canviar d’apartat i **Cancel·lar**. **Cercar a Docker Hub** cerca com `docker search` (ho fa Docker Engine, que necessita Internet).

**Construir una imatge** obre un assistent: context (la carpeta que s’envia, respectant `.dockerignore`) i Dockerfile, noms (`--tag`), arguments (`--build-arg`), metadades (`--label`), etapa final, sense memòria cau, cercar imatges base noves i constructor (**BuildKit** o **clàssic**). Es veuen els passos amb el seu estat i la sortida com `docker build --progress=plain`; es pot cancel·lar.

## 16. Xarxes de Docker {#networks}

L’apartat **Xarxes** llista les xarxes, com `docker network ls`, amb el controlador, les subxarxes i els contenidors. Les xarxes **bridge**, **host** i **none** són predefinides i no es poden eliminar.

- **Crear una xarxa**: nom, controlador (`bridge`, `macvlan`, `ipvlan`, `overlay` o un altre), xarxa interna, adreces IPv4 (subxarxa, passarel·la i rang) i IPv6, opcions del controlador i metadades. Es veuen l’ordre equivalent i la petició exacta, i els errors es marquen abans d’enviar res a Docker.
- **Connectar** un contenidor (en marxa o aturat), amb àlies i IP fixa opcionals. **Desconnectar** demana confirmació.
- **Eliminar** i **Netejar** avisen si hi ha contenidors aturats que tenen configurada la xarxa i que després no podran arrencar.

## 17. Volums {#volumes}

L’apartat **Volums** llista els volums, com `docker volume ls`, amb el controlador i els contenidors que els munten. Els anònims es marquen com a tals. **Calcular les mides** demana a Docker l’espai de cadascun (com `docker system df -v`); no es fa automàticament perquè pot trigar.

- **Crear un volum**: nom (opcional) i tipus: local, tmpfs, NFS, una carpeta de l’equip o un altre controlador.
- **Detall**: directori, opcions, metadades, mida, contenidors amb la ruta i el mode, i inspecció JSON. **Crear un contenidor amb ell** el munta a `/data`.
- **Eliminar**: Docker no elimina un volum que munti qualsevol contenidor, encara que estigui aturat.
- **Netejar**: elimina els volums anònims sense ús o, amb confirmació, tots els volums sense ús.

## 18. Directoris vinculats (bind mounts) {#binds}

Aquest apartat mostra les carpetes i els fitxers del teu equip muntats dins de contenidors, agrupats per ruta: tipus, permisos i propietari, si existeixen, quins contenidors els munten, on i amb quin accés. S’assenyalen els que requereixen atenció: el **sòcol de Docker** (dona el control de Docker al contenidor), l’arrel o directoris del sistema i les rutes que ja no existeixen.

Docker no permet afegir una carpeta a un contenidor que ja existeix. **Vincular una carpeta…** obre el selector de carpetes i després l’assistent de contenidors amb aquesta carpeta muntada a `/mnt/<nom>`.

## 19. Docker Compose {#compose}

L’apartat **Docker Compose** necessita Compose v2 o posterior; si no està instal·lat, explica com instal·lar-lo. Totes les operacions executen el Docker Compose oficial i apareixen al tauler d’activitat amb l’ordre exacta.

La llista mostra els projectes amb contenidors a Docker (també els desplegats des del terminal) i els que hi afegeixes, amb el seu estat (en marxa, parcial, aturat o sense desplegar), la carpeta i els serveis.

- **Obrir un projecte…**: tria el seu `compose.yaml` (i els fitxers addicionals en l’ordre en què es combinen).
- **Projecte nou**: tria una carpeta, escriu el contingut (es parteix d’una plantilla) i **Crear el projecte**: es valida amb Docker Compose abans de crear `compose.yaml`.
- **Treure de la llista** només el treu de la llista; no toca Docker ni els fitxers.

## 20. Gestió de projectes i perfils de Compose {#compose-projects}

Al detall d’un projecte:

- **Serveis**: cada servei amb la seva imatge, estat, contenidors, ports i **perfils**. Cada fila permet desplegar, iniciar, aturar, reiniciar o veure els registres d’aquest servei.
- **Desplegar** (`docker compose up --detach`): tots els serveis o els triats, **perfils** que s’activen, construir abans, política de baixada d’imatges, tornar a crear els contenidors i eliminar els orfes. El diàleg mostra l’ordre exacta.
- **Iniciar, Aturar, Reiniciar**, **Baixar les imatges** i **Construir**.
- **Eliminar** (`docker compose down`): contenidors i xarxes del projecte; opcionalment els seus volums amb nom (se’n perden les dades) i les seves imatges. Cal confirmar-ho.
- **Sortida** de l’operació en curs (es pot **Interrompre**) i **Registres** dels serveis en directe.
- **Fitxer**: edita els fitxers; **Validar** els comprova amb Docker Compose sense desar-los, i **Desar** els valida abans i no els sobreescriu si un altre programa els ha canviat.
- **Configuració**: la configuració final segons Docker Compose. Pot contenir secrets, així que està amagada fins que la demanes i no es desa al tauler d’activitat.

## 21. Terminals simultanis {#terminals}

L’apartat **Terminals** permet obrir i veure alhora diversos terminals reals (`docker exec -it`) en contenidors en execució, en el mateix contenidor o en d’altres.

- **Terminal nou**: tria un contenidor en execució, l’intèrpret (Automàtic, bash o sh) i, si vols, un nom per a la sessió.
- Cada quadre mostra el **contenidor**, el **nom de la sessió** i un **indicador de connexió** (S’està connectant, Connectat, Finalitzat o Error), amb botons per **canviar el nom**, **maximitzar** i **tancar**. Amb un doble clic sobre el nom també se’n canvia el nom.
- **Distribució**: un terminal ocupa tot l’espai; dos van en dues columnes; tres, un a l’esquerra i dos a la dreta; quatre, en quadrícula 2×2; amb més de quatre, en files de dos amb desplaçament vertical.
- Els **separadors** s’arrosseguen amb el ratolí. Amb el focus en un separador, les fletxes el mouen, Inici i Fi el porten als extrems i el doble clic reparteix l’espai a parts iguals.
- Cada sessió és independent: el seu propi intèrpret, la seva entrada i sortida, la seva mida i el seu estat. El que fas en una no afecta les altres.
- Les sessions **continuen obertes en canviar d’apartat**. **Tancar-los tots** demana confirmació. S’admeten fins a 12 sessions alhora.
- En **tancar** una sessió s’acaba el seu intèrpret i els processos que ha deixat en segon pla dins del contenidor. Si el contenidor s’**atura** o s’**elimina**, el quadre ho indica; una sessió acabada es pot **tornar a obrir** si el contenidor encara existeix.
- En **tancar l’aplicació** s’acaben totes les sessions.

El tauler d’activitat registra l’obertura, el tancament i els errors de cada sessió, però no el que s’hi escriu ni la sortida del terminal.

## 22. Tauler d’activitat i registre d’operacions {#activity}

Cada operació apareix al **tauler d’activitat**: hora, distintiu **API** (petició a Docker Engine pel sòcol), **CLI** (procés, com `docker compose` o `apt-get`) o **APP** (operació de la mateixa aplicació, com un canvi d’idioma o de color, o el resum d’una recreació els passos de la qual apareixen a part), tipus, recurs, durada i estat. La línia principal és l’ordre equivalent, que pots copiar. En desplegar una entrada veus l’operació executada de debò, el resultat, l’error i el seu codi HTTP si ha fallat, i les sortides dels processos.

L’apartat **Esdeveniments i registres** mostra els esdeveniments de Docker en directe, els desats, la consulta a Docker d’un període (`docker events --since … --until …`; Docker només conserva els últims 256) i l’**historial d’operacions** d’aquesta i d’altres sessions.

L’historial desa què s’ha fet, quan i amb quin resultat, però mai el cos de les peticions, l’ordre ni les sortides, perquè poden contenir contrasenyes. És a `~/.local/state/docker-control-center/`, només llegible pel teu usuari.

## 23. Configuració, temes i idiomes {#settings}

- **Aparença**: tema (Sistema, Clar, Fosc), **color principal** (verd blavós o **morat**; s’aplica a l’instant als botons, la selecció, els indicadors i la navegació) i idioma, en aquest ordre: **English**, **Español** (predeterminat), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** i **Português**. Cada idioma apareix amb el seu propi nom. L’idioma i el color es desen, es mantenen en tornar a obrir l’aplicació i els seus canvis queden al tauler d’activitat.
- **Tauler d’activitat**: obrir-lo davant d’errors, operacions conservades (50–10 000) i **Desar l’historial entre sessions**.
- **Connexió amb Docker** i comprovació de **Docker Compose**.
- **Actualitzacions**: on es publiquen les versions i **Comprovar ara**. L’aplicació no baixa ni instal·la mai res.
- **Sistema**: versions de l’aplicació, del paquet, de WebKitGTK i glibc, del sistema i de Docker; **Copiar** genera un informe sense dades sensibles.
- **Desinstal·lació i components** i **Ajuda i documentació**.
- **Restablir les preferències**, després de confirmar-ho. No canvia la connexió amb Docker.

| Drecera | Acció |
|---|---|
| Ctrl+B | Plegar o desplegar la barra lateral |
| Ctrl+Maj+A | Obrir o tancar el tauler d’activitat |
| Escapada | Tancar un diàleg |

Els missatges que genera el mateix Docker (per exemple, un error de Docker Engine) i la sortida dels programes es mostren tal com arriben, sense traduir. Els resums tècnics que el motor de l’aplicació escriu al tauler d’activitat (per exemple, com ha acabat un terminal) i els missatges d’error amb dades concretes es generen en castellà en tots els idiomes.

## 24. Desinstal·lació i gestió de dependències {#uninstall}

**Configuració › Desinstal·lació i components** mostra què ha instal·lat l’aplicació i permet treure-ho sense fer servir el terminal. **Analitzar** consulta dpkg i APT sense canviar res:

- **Paquet de l’aplicació** (`docker-control-center`), amb el seu espai.
- **Dependències que es poden treure**: només les que APT va instal·lar automàticament **juntament amb l’aplicació** (segons el seu historial) i que cap altre paquet no necessita.
- **Es conserven**, amb el motiu: dependències compartides (amb els paquets que les fan servir), instal·lades a mà, que ja existien abans, d’origen incert (si l’historial d’APT no mostra com es va instal·lar l’aplicació) i components del sistema.
- **Eines externes**: Docker Engine, Docker CLI i Docker Compose. **Es conserven sempre.**
- **Dades que no s’esborren**: la teva configuració, el teu historial i les dades de la finestra. Tampoc no es toca res de Docker: imatges, contenidors, xarxes, volums i projectes Compose.
- La **simulació d’APT** completa de la desinstal·lació.

Pots triar **Només Docker Control Center** o **amb les dependències que es poden treure**, o marcar els paquets un per un; el resum indica quant espai es recupera. **Revisar i desinstal·lar…** torna a comprovar la selecció i simula exactament l’ordre: si APT anés a treure alguna cosa que no has triat, no es pot continuar. El diàleg mostra l’ordre que s’executarà:

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove docker-control-center …
```

En confirmar, el sistema demana la teva contrasenya d’administrador (pkexec) **només per a aquesta ordre**; l’aplicació continua sense privilegis. Si cancel·les l’autenticació no es treu res. No es fa servir `purge`. Tot queda al tauler d’activitat. Si es treu la mateixa aplicació, tanca-la en acabar.

Si `pkexec` no està instal·lat, l’aplicació no pot demanar permisos: el diàleg mostra l’ordre equivalent amb `sudo` perquè l’executis tu en un terminal.

## 25. Seguretat i tractament de credencials {#security}

- L’aplicació **no s’executa com a root**, no demana mai sudo per funcionar i no canvia permisos del sòcol ni grups.
- Les ordres externes s’executen **sense intèrpret** i amb arguments separats: el que escrius no s’interpreta mai com a ordres.
- **No desa credencials.** Les d’un registre es fan servir només per a una baixada i no apareixen al tauler d’activitat. Amb «Les de Docker» es fan servir les de `docker login`.
- Els valors de variables com `PASSWORD`, `TOKEN` o `SECRET` s’amaguen a la pantalla i al tauler (`********`), encara que Docker els rep complets.
- L’historial persistent no desa mai cossos de peticions, ordres ni sortides.
- Les operacions destructives demanen confirmació.
- La finestra bloqueja les connexions externes. L’única connexió pròpia és la comprovació de versions, desactivada mentre no indiquis una font. El botó de GitHub de l’Ajuda només pot obrir aquesta adreça, al teu navegador.
- Muntar el sòcol de Docker en un contenidor o pertànyer al grup `docker` dona el control total de l’equip: l’aplicació ho assenyala.

## 26. Solució de problemes freqüents {#troubleshooting}

| Causa mostrada | Què cal fer |
|---|---|
| Sembla que Docker no està instal·lat | `sudo apt install docker.io` o la guia oficial de Docker |
| No es troba Docker Engine / està aturat | `sudo systemctl enable --now docker` (rootless: `systemctl --user enable --now docker`) |
| Sense permís per fer servir el sòcol | Docker rootless, o el grup `docker` i tornar a iniciar la sessió |
| Ja pertanys al grup, però no funciona | Tanca la sessió i torna a entrar |
| Connexió de Docker no compatible | `tcp://` o `ssh://` a la connexió local: fes servir un sòcol local (`docker context use default`); per a un servidor remot, afegeix-lo a **Servidors** |
| Docker no respon a temps | `sudo systemctl restart docker` |
| Versió de Docker massa antiga | Actualitza a Docker 20.10 o posterior |

- **Docker Compose no apareix**: instal·la’l (Debian 13: `sudo apt install docker-compose`; Ubuntu: `sudo apt install docker-compose-v2`; repositori de Docker: `docker-compose-plugin`) i prem **Comprovar** a Configuració.
- **Una baixada o una cerca falla**: Docker Engine necessita accés a Internet; el missatge d’error és el de Docker.
- **Un terminal no s’obre**: el contenidor ha d’estar en execució i tenir un intèrpret (`sh` o `bash`).
- **La recreació no es permet**: el motiu apareix a la revisió (contenidor de Compose, creat amb `--rm`, en pausa, imatge que ja no existeix…).
- **La desinstal·lació no ofereix una dependència**: la fa servir un altre programa, ja estava instal·lada o no es pot saber amb seguretat; el motiu hi apareix al costat.
- **Per informar d’un problema**: Configuració › Sistema › **Copiar** genera un informe sense dades sensibles.

## 27. Preguntes freqüents {#faq}

**L’aplicació executa ordres `docker`?** No: fa servir l’API de Docker Engine. L’ordre que veus al tauler d’activitat és l’equivalent, perquè la puguis reproduir. Docker Compose sí que s’executa amb el seu programa oficial.

**He de ser administrador?** No. Només necessites accés al sòcol de Docker. Únicament la desinstal·lació de paquets demana permisos, i només per a aquesta ordre.

**Funciona sense Internet?** Sí. Només les operacions de Docker contra registres remots necessiten xarxa.

**Què passa amb els meus contenidors si tanco l’aplicació?** Continuen funcionant: els gestiona Docker. En tancar s’acaben els terminals oberts i es deixen de seguir els registres i les estadístiques.

**Puc perdre dades en editar un contenidor?** No es perd cap volum, carpeta ni imatge, i l’original es conserva com a còpia. Només els fitxers escrits fora dels volums es queden a la còpia, i la revisió t’ho avisa abans.

**Es perden les meves dades en desinstal·lar?** No. Es conserven la teva configuració, el teu historial i tot el de Docker.

**On es desen la configuració i l’historial?** A `~/.config/docker-control-center/` i `~/.local/state/docker-control-center/`.

**Com canvio l’idioma?** A Configuració › Aparença › Idioma.
