## 1. Què és Container Manage Center {#about}

Container Manage Center (**CMC**) és una aplicació d’escriptori per administrar **Docker Engine** sense dependre del terminal, en aquest equip i en servidors remots per SSH. Tot el que mostra són dades reals que demana a Docker en aquell moment, i cada acció és una operació real: no se simula res.

- Es comunica amb Docker Engine amb la seva **API oficial**: pel sòcol Unix a Linux, per la canonada amb nom de Docker Desktop a Windows i per SSH amb els servidors remots.
- Executa **Docker Compose** amb el seu programa oficial, amb arguments separats i sense shell.
- Cada operació apareix al **tauler d’activitat** amb l’ordre `docker` equivalent, perquè sàpigues exactament què s’ha fet.

No cal saber fer servir Docker des del terminal: cada apartat explica què fa abans de fer-ho, i les operacions que esborren o canvien alguna cosa demanen confirmació. Si un terme no et resulta familiar, consulta el **Glossari** (secció 33).

## 2. Requisits del sistema {#requirements}

| Element | Requisit |
|---|---|
| Sistema (Linux) | Debian 12 o superior, o Ubuntu 22.04 o superior, de 64 bits (x86_64) |
| Sistema (Windows, en proves) | Windows 10 o 11 de 64 bits amb Docker Desktop (secció 28) |
| Docker | Docker Engine 20.10 o posterior (API 1.41+), en mode normal o rootless |
| Docker Compose | v2 o posterior, només per a l’apartat Docker Compose (`docker compose` o `docker-compose` v2). Compose v1 no s’admet |
| Biblioteques (Linux) | WebKitGTK 4.1, GTK 3 i glibc. Les instal·la APT amb el paquet |
| Servidors remots | El client SSH del sistema (`ssh`) i, al servidor, Docker Engine 20.10 o posterior |

Docker i Docker Compose són **requisits externs**: l’aplicació no els instal·la ni els configura, però et diu com fer-ho si en falta algun.

L’aplicació no necessita Internet. Només les operacions que fa Docker Engine contra registres remots (baixar imatges, cercar a Docker Hub, construir amb imatges base que no siguin a l’equip) necessiten que Docker tingui xarxa.

## 3. Instal·lació i primera execució {#install}

A Debian i Ubuntu, instal·la el paquet amb APT, que resol les dependències:

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

Per a Debian 12 i Ubuntu 22.04 fes servir el paquet compatible, el nom del qual acaba en `-compat-debian12-ubuntu2204.deb`. Abans d’instal·lar pots comprovar la baixada amb el fitxer `SHA256SUMS` que es publica amb cada versió (`sha256sum -c SHA256SUMS`).

L’aplicació apareix al menú com a **Container Manage Center**. S’executa amb el teu usuari normal: **no l’executis amb sudo** (es nega a arrencar com a root).

**Primera execució.** En obrir-la, busca Docker Engine automàticament. Si el troba, el **Tauler** mostra el seu estat; si no, mostra el diagnòstic amb la causa i els passos per resoldre-la. No cal configurar res més per començar.

**Actualitzar.** Instal·la el paquet nou de la mateixa manera: es conserven la teva configuració i el teu historial.

La instal·lació a Windows s’explica a la secció 28.

## 4. Finestra principal i navegació {#interface}

La finestra té quatre zones:

- **Barra lateral** (a l’esquerra): els apartats, agrupats. Es plega amb **Ctrl+B** o amb **Plegar la barra lateral**, a la part inferior.
- **Barra superior**: el títol de l’apartat, el **selector de servidor** (en quin Docker treballes: aquest equip o un servidor remot), l’**indicador de connexió** i el botó **Activitat**.
- **Àrea de treball**: el contingut de l’apartat triat.
- **Tauler d’activitat** (a la dreta): les operacions executades. S’obre i es tanca amb **Ctrl+Maj+A** o amb el botó **Activitat**.

| Grup | Apartats |
|---|---|
| (a dalt) | Tauler, Servidors |
| Recursos | Contenidors, Gestió massiva, Imatges, Xarxes, Volums, Directoris vinculats |
| Orquestració | Docker Compose |
| Eines | Terminals, Auditoria, Còpies |
| Supervisió | Monitorització, Esdeveniments i registres |
| (a baix) | Configuració |

Aquest manual és a **Configuració › Ajuda i documentació**. L’índex de l’esquerra porta a cada secció, **Anterior** i **Següent** recorren el manual en ordre i el cercador mostra només les seccions que contenen el text que escriguis.

| Drecera | Acció |
|---|---|
| Ctrl+B | Plegar o desplegar la barra lateral |
| Ctrl+Maj+A | Obrir o tancar el tauler d’activitat |
| Escapada | Tancar un diàleg o un menú |
| Fletxes, Inici i Fi | Moure’s pels menús i moure els separadors de Terminals |

Els colors d’estat són sempre els mateixos: **verd** per al que funciona o està en marxa, **ambre** per als avisos i **vermell** per als errors.

## 5. Connexió amb Docker Engine {#connection}

L’**indicador de connexió** de la barra superior mostra l’estat:

| Aspecte | Significat |
|---|---|
| Punt verd i `Docker 29.x` | Connectat. En passar-hi el ratolí es veu el sòcol |
| `S’està connectant amb Docker…` | S’està detectant o connectant |
| Vermell, `Sense connexió amb Docker` | Sense connexió. Prem-lo per veure el diagnòstic |

En mode **automàtic**, l’aplicació prova en aquest ordre: la variable `DOCKER_HOST` (només `unix://`), el context actiu de Docker, el teu sòcol rootless i el sòcol del sistema `/var/run/docker.sock`. Vigila la connexió cada 10 segons i torna a connectar sola quan Docker torna a estar disponible.

L’aplicació no demana mai sudo, no canvia permisos ni grups i no executa per tu les instruccions del diagnòstic. El teu usuari ha de poder obrir el sòcol: amb Docker rootless o pertanyent al grup `docker` (cosa que a la pràctica equival a tenir privilegis d’administrador).

La configuració es desa al moment a `~/.config/container-manage-center/config.toml`. Si prefereixes indicar un sòcol concret, consulta la secció següent.

## 6. Docker rootless i sòcols personalitzats {#rootless}

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

En un servidor remot també funcionen l’**Auditoria**, les **Còpies** i la **Monitorització** (seccions 23 a 25).

## 8. Tauler i diagnòstic {#dashboard}

Amb connexió, el **Tauler** mostra dades obtingudes en aquell moment: versió de Docker Engine i de l’API, sòcol i origen de la connexió, contenidors (en execució, en pausa, aturats), imatges, dades de l’equip (sistema, nucli, CPU, memòria), emmagatzematge i execució (controlador, directori de dades, cgroups, registres) i els avisos del dimoni. **Actualitzar** les torna a demanar.

Sense connexió mostra el **diagnòstic**: la causa concreta, els passos per resoldre-la (amb les ordres per copiar en un terminal), els sòcols comprovats amb el resultat i el que s’ha detectat instal·lat. Els botons **Tornar a connectar** i **Configurar la connexió** són a la capçalera.

Les causes més habituals d’una fallada de connexió i la seva solució són a la secció 30.

## 9. Gestió de contenidors {#containers}

L’apartat **Contenidors** llista tots els contenidors, com `docker ps -a`: estat, nom i ID curt, projecte Compose, imatge, estat de Docker, ports publicats i antiguitat. Pots filtrar per **Tots / En execució / Aturats** i cercar per nom, imatge, ID o projecte. El menú **⋯** de cada fila (o el botó dret) ofereix **Editar la configuració** i **Crear una imatge a partir d’aquest contenidor**.

| Estat | Accions disponibles |
|---|---|
| En execució | Aturar, Reiniciar, Pausar, Eliminar |
| En pausa | Reprendre, Aturar, Eliminar |
| Creat o aturat | Iniciar, Eliminar |

Aturar i reiniciar esperen 10 segons que el procés acabi abans de forçar-lo, com Docker. Després de cada acció es consulta l’estat real del contenidor. Si Docker rebutja una acció, el seu missatge apareix damunt de la taula i al tauler d’activitat. Les llistes s’actualitzen soles quan alguna cosa canvia a Docker, també des del terminal.

Per aplicar una mateixa acció a molts contenidors alhora, fes servir la **Gestió massiva** (secció 13).

## 10. Crear, duplicar, inspeccionar i eliminar contenidors {#container-lifecycle}

**Crear un contenidor** obre un assistent amb sis pestanyes (només la imatge és obligatòria): General, Entorn, Xarxa, Emmagatzematge, Recursos i Avançat.

- **Ordre** i **Punt d’entrada** s’escriuen com en un terminal i es divideixen en arguments, que veus a sota. No s’executa cap intèrpret.
- El quadre **Equivalent** mostra el `docker run` equivalent i el cos exacte que s’envia a l’API.
- **Revisió** marca els errors (impedeixen crear) i les advertències (ports privilegiats, carpetes delicades amb escriptura, mode privilegiat…).
- **Crear** crea el contenidor sense iniciar-lo; **Crear i iniciar** també l’inicia.

En prémer el nom d’un contenidor s’obre el seu **detall**: Resum, Registres, Terminal, Estadístiques i Inspecció JSON (la resposta completa de `docker inspect`, amb el botó Copiar).

**Duplicar** obre l’assistent amb la configuració del contenidor i el nom `<nom>-copia`, i avisa dels paràmetres que no es traslladen.

**Eliminar** demana confirmació. Un contenidor en execució o en pausa només s’elimina si marques **Aturar-lo a la força i eliminar-lo**. Opcionalment s’eliminen els seus volums anònims; els volums amb nom i les carpetes vinculades no es toquen. No es pot desfer.

Els registres, les estadístiques i el terminal del detall s’expliquen a la secció 21.

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
- **Descripció**: es desa a l’etiqueta `cmc.description` del contenidor.

**Revisar els canvis** compara la configuració actual amb la nova: què canvia (abans i després, element per element), què es manté i què cal recrear. No s’aplica res fins que ho confirmes.

**Recreació segura.** Quan un canvi exigeix recrear, l’assistent:

1. Comprova la imatge, els conflictes de nom i de ports amb altres contenidors i els motius que impedeixen recrear amb seguretat: contenidors de Compose o Swarm, creats amb `--rm`, en pausa o reiniciant-se. En aquests casos no deixa continuar.
2. Mostra els riscos que has d’acceptar un per un: fitxers escrits fora dels volums (no passen al contenidor nou), muntatges que deixen d’estar muntats, contenidors que fan servir la xarxa d’aquest…
3. En confirmar: atura el contenidor si està en marxa, li canvia el nom a `<nom>-cmc-backup-<data>` (còpia de seguretat), crea el nou amb **tota** la configuració original més els teus canvis, en connecta les xarxes i, si estava en marxa, l’inicia i comprova que continua funcionant uns segons.
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

## 13. Gestió massiva {#bulk}

La **Gestió massiva** (a «Recursos») aplica una mateixa operació o una mateixa ordre a molts contenidors alhora, també de diversos servidors. Reuneix els contenidors d’aquest equip i dels servidors remots **connectats**; els altres s’indiquen en un avís. Està dividida en pestanyes: **Resum**, **Contenidors**, **Operacions**, **Configuració massiva**, **Ordres massives** i **Historial**.

- **Resum**: total, en execució, aturats, en pausa i amb error. Prémer un comptador filtra per aquest estat. **Actualitzar** i l’actualització automàtica (de 5 s a 5 min, es pot desactivar) tornen a demanar les dades.
- **Contenidors**: cerca per nom o ID; filtra per estat, imatge, servidor i etiqueta (`clau` o `clau=valor`); ordena prement el títol d’una columna. Se selecciona amb la casella de cada fila, la de la pàgina o **tot el filtrat**.
- **Operacions**: iniciar, aturar, reiniciar, posar en pausa, reprendre i eliminar. El que no es pot aplicar en l’estat actual s’**omet** i s’explica.
- **Configuració massiva**: mostra la política de reinici de la selecció i permet canviar-la en tots (en calent, amb `docker update`; no en els creats amb `--rm`). Les **etiquetes** només es mostren: Docker no permet canviar-les en un contenidor que ja existeix (caldria tornar-lo a crear). Per a un contenidor concret, fes servir **Duplicar**.
- **Historial**: les operacions i les ordres massives anteriors, amb el que s’ha fet a cada contenidor.

**Abans d’aplicar** es mostra una confirmació amb l’operació, els servidors, els contenidors afectats i els omesos, les conseqüències i quantes operacions van alhora (d’1 a 16). **Eliminar** demana escriure el nombre de contenidors i **mai no elimina volums ni imatges**.

**Resultats**: cada contenidor té el seu: correcte (Docker ho ha confirmat), fallit (amb l’error), omès, cancel·lat o **desconegut** (la connexió s’ha tallat abans de la confirmació; actualitza per veure’n l’estat). Si un falla, els altres continuen; no es desfà ni es reintenta res automàticament. **Cancel·la el pendent** deixa d’enviar ordres. Cada ordre es registra al tauler d’activitat amb el distintiu «Massiva», i **Mostra a l’activitat** mostra només aquesta operació.

Les **Ordres massives** executen una mateixa ordre dins de diversos contenidors en execució amb Docker Exec, sense passar per cap shell d’aquest equip:

1. Tria la modalitat: **Executable amb arguments** (un argument per línia, enviat tal qual) o **Seqüència amb un intèrpret** (diverses línies per a `/bin/sh`, `bash`, `python3`…). També pots carregar un exemple o una ordre freqüent desada.
2. Si cal, indica l’usuari, el directori de treball, el temps màxim (d’1 a 3600 s) i quantes execucions van alhora.
3. Revisa la **Previsualització**: el `docker exec` equivalent, els arguments exactes que rep Docker i el nivell de risc (Informatiu, Modificació, Administratiu o Potencialment destructiu). La classificació és orientativa: no substitueix revisar l’ordre.
4. Confirma. Les ordres de risc administratiu o destructiu demanen escriure el nombre de contenidors.

Cada contenidor dona el seu estat, codi de sortida, durada, i `stdout` i `stderr` per separat (copiables i exportables a Markdown o JSON amb els secrets ocults). **Cancel·la les pendents** no llança l’ordre en els que encara no han començat; **Interromp les actives**, a més, deixa d’esperar els que estan en marxa. Docker no permet aturar un procés ja llançat: amb el temps esgotat o en interrompre, el procés pot continuar dins del contenidor, i CMC **mai** no atura ni reinicia el contenidor per aturar-lo. Les ordres freqüents es poden desar, **mai amb secrets**.

## 14. Imatges {#images}

L’apartat **Imatges** llista les imatges locals, com `docker image ls`: etiquetes, ID curt, mida, antiguitat i quins contenidors les fan servir. Filtres **Totes / En ús / Sense ús / Sense etiqueta** i cerca per etiqueta o ID.

- **Detall**: Resum (amb els valors sensibles de les variables amagats), Capes (com `docker image history`), Contenidors que la fan servir i Inspecció JSON.
- **Etiquetar** afegeix un nom `repositori:etiqueta`.
- **Eliminar**: si la imatge té diverses etiquetes en pots treure només una; la imatge completa exigeix forçar-ho. Docker no elimina imatges de contenidors en execució.
- **Netejar** elimina les imatges sense etiqueta que no fa servir cap contenidor o, amb una confirmació addicional, totes les que no es fan servir.

## 15. Baixar i construir imatges {#pull-build}

**Baixar una imatge** demana la referència (`nginx:1.27`, `usuari/app`, `registre.exemple.com:5000/app:v2`; sense etiqueta es fa servir `latest`). Credencials:

| Opció | Què fa |
|---|---|
| Les de Docker | Fa servir el que s’ha desat amb `docker login` (o el seu *credential helper*) |
| Cap | Baixada anònima |
| Usuari i contrasenya | Només per a aquesta baixada: no es desen ni apareixen al tauler d’activitat |

El quadre **Baixades** mostra el progrés total i per capa; mentrestant pots canviar d’apartat i **Cancel·lar**. **Cercar a Docker Hub** cerca com `docker search` (ho fa Docker Engine, que necessita Internet).

**Construir una imatge** obre un assistent: context (la carpeta que s’envia, respectant `.dockerignore`) i Dockerfile, noms (`--tag`), arguments (`--build-arg`), metadades (`--label`), etapa final, sense memòria cau, cercar imatges base noves i constructor (**BuildKit** o **clàssic**). Es veuen els passos amb el seu estat i la sortida com `docker build --progress=plain`; es pot cancel·lar.

## 16. Xarxes {#networks}

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

**Les dades d’un volum eliminat no es poden recuperar.** Si les necessites, fes abans una còpia (secció 24).

## 18. Directoris vinculats {#binds}

Aquest apartat mostra les carpetes i els fitxers del teu equip muntats dins de contenidors, agrupats per ruta: tipus, permisos i propietari, si existeixen, quins contenidors els munten, on i amb quin accés. S’assenyalen els que requereixen atenció: el **sòcol de Docker** (dona el control de Docker al contenidor), l’arrel o directoris del sistema i les rutes que ja no existeixen.

Docker no permet afegir una carpeta a un contenidor que ja existeix. **Vincular una carpeta…** obre el selector de carpetes i després l’assistent de contenidors amb aquesta carpeta muntada a `/mnt/<nom>`.

## 19. Docker Compose: projectes {#compose}

L’apartat **Docker Compose** necessita Compose v2 o posterior; si no està instal·lat, explica com instal·lar-lo. Totes les operacions executen el Docker Compose oficial i apareixen al tauler d’activitat amb l’ordre exacta.

La llista mostra els projectes amb contenidors a Docker (també els desplegats des del terminal) i els que hi afegeixes, amb el seu estat (en marxa, parcial, aturat o sense desplegar), la carpeta i els serveis.

- **Obrir un projecte…**: tria el seu `compose.yaml` (i els fitxers addicionals en l’ordre en què es combinen).
- **Projecte nou**: tria una carpeta, escriu el contingut (es parteix d’una plantilla) i **Crear el projecte**: es valida amb Docker Compose abans de crear `compose.yaml`.
- **Treure de la llista** només el treu de la llista; no toca Docker ni els fitxers.

## 20. Docker Compose: serveis, desplegament i perfils {#compose-projects}

Al detall d’un projecte:

- **Serveis**: cada servei amb la seva imatge, estat, contenidors, ports i **perfils**. Cada fila permet desplegar, iniciar, aturar, reiniciar o veure els registres d’aquest servei.
- **Desplegar** (`docker compose up --detach`): tots els serveis o els triats, **perfils** que s’activen, construir abans, política de baixada d’imatges, tornar a crear els contenidors i eliminar els orfes. El diàleg mostra l’ordre exacta.
- **Iniciar, Aturar, Reiniciar**, **Baixar les imatges** i **Construir**.
- **Eliminar** (`docker compose down`): contenidors i xarxes del projecte; opcionalment els seus volums amb nom (se’n perden les dades) i les seves imatges. Cal confirmar-ho.
- **Sortida** de l’operació en curs (es pot **Interrompre**) i **Registres** dels serveis en directe.
- **Fitxer**: edita els fitxers; **Validar** els comprova amb Docker Compose sense desar-los, i **Desar** els valida abans i no els sobreescriu si un altre programa els ha canviat.
- **Configuració**: la configuració final segons Docker Compose. Pot contenir secrets, així que està amagada fins que la demanes i no es desa al tauler d’activitat.

## 21. Registres, estadístiques i terminal d’un contenidor {#logs-stats-terminal}

Al detall d’un contenidor:

- **Registres**: sortida en directe (stderr en vermell). Pots triar quantes línies anteriors carregar, filtrar, veure les marques de temps, seguir el final, copiar el que es veu i deixar de seguir. Es conserven les últimes 10 000 línies.
- **Estadístiques**: CPU, memòria, xarxa, disc i processos cada segon, calculats com `docker stats`.
- **Terminal**: **Obrir un terminal** inicia un intèrpret amb `docker exec -it` (Automàtic fa servir `bash` si existeix i, si no, `sh`; també en pots triar un). **Tancar la sessió** acaba l’intèrpret i els processos que ha deixat dins del contenidor.

Són operacions reals que apareixen al tauler d’activitat mentre estan obertes i es tanquen en sortir del detall. Per treballar amb diversos terminals alhora, fes servir l’apartat **Terminals**.

Per veure l’evolució de l’ús al llarg del temps i rebre alertes, fes servir la **Monitorització** (secció 25).

## 22. Terminals simultanis {#terminals}

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

## 23. Auditoria de seguretat {#audit}

**Eines › Auditoria** revisa la seguretat del servidor triat (aquest equip o un de remot) i **no canvia res**: només llegeix la configuració.

1. Tria **què vols auditar**: tots els contenidors (en execució i aturats) o un; també les imatges i els projectes Compose (se’n llegeix la configuració amb `docker compose config`, sense desplegar).
2. Prem **Audita**. Es veu el progrés i es pot cancel·lar.
3. Revisa les **troballes**, per nivell (crític, alt, mitjà, baix) i agrupades per contenidor, servei o imatge. Cadascuna explica què passa, **per què és important** i què es **recomana**. Es poden filtrar per nivell, per tipus i per text.

Es comproven els riscos per a l’equip (sòcol de Docker muntat, carpetes sensibles, capacitats, mode privilegiat, xarxa de l’equip, PID i IPC compartits, seccomp, AppArmor o SELinux desactivats, dispositius i ports oberts a totes les interfícies) i l’enduriment (procés com a root, sistema de fitxers modificable, sense límits de memòria, CPU o processos, sense `no-new-privileges` i imatges de més d’un any). Amb Docker rootless o `userns-remap`, el root del contenidor es marca com a baix.

- **Proposa correccions** (contenidors; el botó indica quantes): obre **Editar la configuració** amb els canvis ja posats a l’esborrany. Revisa’ls i aplica’ls com qualsevol edició (secció 11); si els descartes, no canvia res.
- **Vulnerabilitats**: si tens **Trivy** o **Grype** instal·lats amb la seva base baixada, marca la casella i cada imatge mostra quantes vulnerabilitats conegudes té, per gravetat. L’aplicació no instal·la aquestes eines. Sense elles, o en un servidor remot, l’informe diu «No analitzat», mai «0».
- **Exportar** a JSON, Markdown o HTML: el fitxer queda només per al teu usuari i els secrets s’amaguen.
- **Auditories anteriors**: les 200 últimes de cada servidor, per tornar-les a obrir.
- **Contenidors a les altres eines**: per a cada contenidor auditat, la seva pitjor troballa, si la monitorització el vigila, la seva còpia més recent i les tasques de còpies que l’inclouen, amb accessos a **Mostra a Monitorització** i **Prepara una còpia**.

## 24. Còpies de seguretat {#backup}

**Eines › Còpies** fa còpies de seguretat del servidor triat (aquest equip o un de remot) i les restaura.

1. **Carpeta de destinació**: **Tria una carpeta** obre el diàleg del sistema. A dins es crea `container-manage-center-copias/<servidor>/<còpia>/`, només per al teu usuari. No es poden fer servir carpetes del sistema; sí un disc extern o una carpeta de xarxa ja muntada.
2. **Còpia nova**: marca el que vulguis copiar.
3. **Fes una còpia ara** mostra el progrés i es pot cancel·lar. Una còpia cancel·lada o interrompuda mai no es dona per bona.

| Element | Com es copia |
|---|---|
| Volums | Per l’API de Docker, amb un contenidor auxiliar que no s’arrenca mai. Si un contenidor en marxa el fa servir, tria **en calent** (còpia no garantida) o **aturant els contenidors** (s’aturen mentre es copia i tornen a arrencar) |
| Bases de dades | MariaDB, MySQL, PostgreSQL, Redis o MongoDB en marxa: el bolcat el fa l’eina del mateix motor dins del contenidor, amb la contrasenya de les seves variables. És coherent encara que hi hagi escriptures |
| Configuració de contenidors | Per tornar-los a crear, sense les variables amb secrets |
| Projectes Compose | Només en aquest equip: els seus fitxers. Els `.env` només si marques «Inclou els fitxers .env» |
| Configuració de l’aplicació | Preferències, servidors i tasques (les contrasenyes són al clauer i no es copien) |

- **Còpies**: les del servidor, amb el seu estat (Completa, Incompleta), elements, mida i avisos. **Comprova** recalcula la suma SHA-256 de cada fitxer i diu si n’hi ha algun d’alterat, que falta o que sobra.
- **Restaurar**: tria quins elements i on, i prem **Previsualització**, que comprova la integritat i diu què es farà amb cadascun. Un volum es restaura en un de **nou** o buit, mai sobre un amb dades. Un contenidor es crea nou, amb un altre nom i sense arrencar. Els fitxers s’escriuen en una carpeta nova.
- **Restaurar una base de dades substitueix les seves dades actuals**: cal escriure «restaurar» per confirmar-ho.
- **Tasques programades**: **Desa com a tasca** desa la selecció amb un nom, una freqüència (diària, setmanal o mensual) i una retenció (per defecte, 7 diàries, 4 setmanals i 3 mensuals). La retenció només esborra còpies antigues **d’aquella tasca**. **Programar** crea un temporitzador d’usuari de systemd (a Windows, una tasca del Programador de tasques) que funciona encara que l’aplicació estigui tancada; abans et mostra què crearà i només ho crea si ho autoritzes. No cal ser administrador. Els temporitzadors d’usuari de Linux només s’executen amb la sessió iniciada, llevat que activis tu `loginctl enable-linger`.
- **Historial**: cada còpia, manual o programada, amb el seu resultat i les còpies antigues que ha esborrat la retenció.

Les còpies **no estan xifrades**: contenen les teves dades tal qual. Si les guardes fora de l’equip, fes servir un disc xifrat.

## 25. Monitorització i alertes {#monitoring}

**Supervisió › Monitorització** mostra l’ús dels contenidors del servidor triat al llarg del temps. No instal·la res al servidor: demana a Docker el mateix que `docker stats`, també per SSH.

- **Activar**: marca «Monitoritza aquest servidor». S’activa a cada servidor per separat i continua activa en tornar a obrir l’aplicació. L’**interval** (de 2 a 60 s) és comú a tots; **Pausa** atura totes les mostres.
- **Resum**: CPU i memòria de l’equip segons Docker, contenidors en marxa, en pausa i aturats, i l’ús total en gràfics. La taula mostra, per contenidor, CPU, memòria i el seu límit, xarxa, disc, processos, temps en marxa i reinicis. Prem un nom per veure’n els gràfics. La CPU està en % d’una CPU, com `docker stats`: un contenidor que fa servir dues CPU senceres marca 200 %.
- **Període**: 5, 15 i 30 minuts surten de la memòria (des que has obert l’aplicació); 1, 6 i 24 hores, del resum per minut que es desa a l’equip.
- **Comparar**: fins a 4 contenidors en marxa i una mètrica al mateix gràfic.
- **Sense connexió**: si el servidor deixa de respondre, els gràfics mostren un tram ratllat. No s’inventen dades.

**Alertes**: CPU alta i memòria alta (amb un llindar d’avís i un de resolució, per no avisar a cada pujada petita), el contenidor cau i Docker el torna a arrencar, falta de memòria (OOM), acaba amb error sense que ningú l’aturi i es perd la connexió amb el servidor. Aturar tu un contenidor no és un error. Després d’un avís, la mateixa alerta no es repeteix durant els minuts de silenci (15 per defecte). Les **notificacions d’escriptori** no surten de l’equip; **Prova la notificació** n’envia una de prova. Totes les alertes queden a l’**Historial d’alertes**.

La monitorització no anota cada mostra al tauler d’activitat: només quan comença i quan acaba a cada servidor.

## 26. Esdeveniments, activitat i historial d’operacions {#activity}

Cada operació apareix al **tauler d’activitat**: hora, distintiu **API** (petició a Docker Engine), **CLI** (procés, com `docker compose` o `apt-get`) o **APP** (operació de la mateixa aplicació, com un canvi d’idioma, o el resum d’una recreació els passos de la qual apareixen a part), tipus, recurs, servidor, durada i estat. La línia principal és l’ordre equivalent, que pots copiar. En desplegar una entrada veus l’operació executada de debò, el resultat, l’error i el seu codi HTTP si ha fallat, i les sortides dels processos. Els valors secrets s’amaguen amb `********`.

L’apartat **Esdeveniments i registres** té quatre pestanyes:

- **En directe**: els esdeveniments de Docker d’aquesta sessió (un contenidor que arrenca, una imatge baixada…), vinguin d’aquesta aplicació o de qualsevol altra eina. Es poden filtrar i posar en **Pausa**.
- **Desats**: els esdeveniments rebuts amb l’aplicació oberta, també de sessions anteriors (els 20.000 més recents).
- **Consultar Docker**: els esdeveniments d’un període (`docker events --since … --until …`). Inclou el que ha passat amb l’aplicació tancada, però Docker només conserva els últims 256.
- **Historial d’operacions**: el que ha fet l’aplicació, també en sessions anteriors (les 10.000 més recents).

L’historial desa què s’ha fet, quan i amb quin resultat, però mai el cos de les peticions, les ordres ni les sortides, perquè poden contenir contrasenyes. És a `~/.local/state/container-manage-center/`, només llegible pel teu usuari. El pots esborrar des de cada pestanya o deixar de desar-lo a Configuració.

## 27. Configuració de l’aplicació {#settings}

- **Aparença**: tema (Sistema, Clar, Fosc) i idioma, en aquest ordre: **English**, **Español** (predeterminat), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** i **Português**. Cada idioma apareix amb el seu propi nom. L’idioma es desa i es manté en tornar a obrir l’aplicació. El color principal és el **morat** i no es pot canviar.
- **Tauler d’activitat**: obrir-lo només quan una operació falla, operacions conservades (50–10.000) i **Desa l’historial entre sessions**.
- **Connexió amb Docker** (seccions 5 i 6) i comprovació de **Docker Compose**.
- **Actualitzacions**: on es publiquen les versions i **Comprovar ara**. L’aplicació mai no baixa ni instal·la res.
- **Sistema**: versions de l’aplicació, del paquet, de WebKitGTK i glibc, del sistema i de Docker; **Copiar** genera un informe sense dades sensibles.
- **Desinstal·lació i components** (secció 29), **Ajuda i documentació** (aquest manual) i **Legal** (secció 34).
- **Restablir les preferències**, després de confirmar-ho. No canvia la connexió amb Docker.

Tot es desa al moment a `~/.config/container-manage-center/config.toml`.

Els missatges que genera el mateix Docker (per exemple, un error de Docker Engine) i la sortida dels programes es mostren tal com arriben, sense traduir. Els resums tècnics que el motor de l’aplicació escriu al tauler d’activitat i els missatges d’error amb dades concretes es generen en castellà en tots els idiomes.

## 28. Windows {#windows}

La versió per a Windows està **en proves**: l’instal·lador es genera, però encara no s’ha validat en un Windows real. És un instal·lador `.exe` per al teu usuari, sense permisos d’administrador, i no està signat, de manera que Windows pot mostrar un avís de SmartScreen en obrir-lo.

A Windows, Container Manage Center funciona igual que a Linux, amb aquestes diferències:

- **Docker local**: es connecta amb **Docker Desktop** per la seva canonada amb nom (`\\.\pipe\docker_engine`). Docker Desktop ha d'estar en marxa, i el teu usuari, al grup local `docker-users` (l'afegeix l'instal·lador de Docker Desktop).
- **Servidors remots per SSH**: fan servir el client OpenSSH de Windows (`ssh.exe`, inclòs a Windows 10 i 11). Com que no admet connexions multiplexades, cada operació obre la seva pròpia connexió: la primera vegada es comprova la identitat del servidor igual que a Linux, i la contrasenya o la frase de pas desada s'utilitza sola. Els **servidors intermedis** (ProxyJump) encara no estan disponibles, i a la **terminal del servidor** el canvi de mida de la finestra no arriba al servidor.
- **Contrasenyes recordades**: a l'**Administrador de credencials** de Windows (credencials de Windows), amb el nom `container-manage-center/…`.
- **Còpies programades**: amb el **Programador de tasques**, a la carpeta «Container Manage Center», amb el teu usuari i sense privilegis d'administrador. S'executen amb la sessió iniciada; si l'equip estava apagat a aquella hora, la còpia es fa tan aviat com es pot.
- **Notificacions**: les del sistema (centre de notificacions).
- **Dades**: configuració a `%APPDATA%\container-manage-center` i historial a `%LOCALAPPDATA%\container-manage-center`.
- **Desinstal·lar**: des de Configuració de Windows › Aplicacions › Aplicacions instal·lades. Les teves dades i Docker Desktop no es toquen. L'apartat «Desinstal·lació i components» només existeix a Debian i Ubuntu.

## 29. Desinstal·lació i components {#uninstall}

**Configuració › Desinstal·lació i components** mostra què ha instal·lat l’aplicació i permet treure-ho sense fer servir el terminal. **Analitzar** consulta dpkg i APT sense canviar res:

- **Paquet de l’aplicació** (`container-manage-center`), amb el seu espai.
- **Dependències que es poden treure**: només les que APT va instal·lar automàticament **juntament amb l’aplicació** (segons el seu historial) i que cap altre paquet no necessita.
- **Es conserven**, amb el motiu: dependències compartides (amb els paquets que les fan servir), instal·lades a mà, que ja existien abans, d’origen incert (si l’historial d’APT no mostra com es va instal·lar l’aplicació) i components del sistema.
- **Eines externes**: Docker Engine, Docker CLI i Docker Compose. **Es conserven sempre.**
- **Dades que no s’esborren**: la teva configuració, el teu historial i les dades de la finestra. Tampoc no es toca res de Docker: imatges, contenidors, xarxes, volums i projectes Compose.
- La **simulació d’APT** completa de la desinstal·lació.

Pots triar **Només Container Manage Center** o **amb les dependències que es poden treure**, o marcar els paquets un per un; el resum indica quant espai es recupera. **Revisar i desinstal·lar…** torna a comprovar la selecció i simula exactament l’ordre: si APT anés a treure alguna cosa que no has triat, no es pot continuar. El diàleg mostra l’ordre que s’executarà:

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove container-manage-center …
```

En confirmar, el sistema demana la teva contrasenya d’administrador (pkexec) **només per a aquesta ordre**; l’aplicació continua sense privilegis. Si cancel·les l’autenticació no es treu res. No es fa servir `purge`. Tot queda al tauler d’activitat. Si es treu la mateixa aplicació, tanca-la en acabar.

Si `pkexec` no està instal·lat, l’aplicació no pot demanar permisos: el diàleg mostra l’ordre equivalent amb `sudo` perquè l’executis tu en un terminal.

## 30. Solució de problemes {#troubleshooting}

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

- **Un servidor remot no connecta**: **Prova la connexió** indica en quin dels set passos falla (secció 7).
- **Una ordre massiva falla en un contenidor**: el contenidor ha d’estar en execució i tenir el programa o l’intèrpret indicats; l’error de Docker apareix al seu resultat.

## 31. Seguretat i bones pràctiques {#security}

- L’aplicació **no s’executa com a root**, no demana mai sudo per funcionar i no canvia permisos del sòcol ni grups.
- Les ordres externes s’executen **sense intèrpret** i amb arguments separats: el que escrius no s’interpreta mai com a ordres.
- **No desa credencials.** Les d’un registre es fan servir només per a una baixada i no apareixen al tauler d’activitat. Amb «Les de Docker» es fan servir les de `docker login`.
- Els valors de variables com `PASSWORD`, `TOKEN` o `SECRET` s’amaguen a la pantalla i al tauler (`********`), encara que Docker els rep complets.
- L’historial persistent no desa mai cossos de peticions, ordres ni sortides.
- Les operacions destructives demanen confirmació.
- La finestra bloqueja les connexions externes. L’única connexió pròpia és la comprovació de versions, desactivada mentre no indiquis una font. El botó de GitHub de l’Ajuda només pot obrir aquesta adreça, al teu navegador.
- Muntar el sòcol de Docker en un contenidor o pertànyer al grup `docker` dona el control total de l’equip: l’aplicació ho assenyala.

**Bones pràctiques**: fes servir Docker rootless sempre que puguis; revisa l’**Auditoria** de tant en tant; fes **còpies** abans d’eliminar volums o actualitzar bases de dades; no publiquis ports a totes les interfícies si només els fas servir en aquest equip; no desis contrasenyes en imatges ni en les ordres freqüents. A la gestió massiva, eliminar i les ordres de risc administratiu o destructiu demanen escriure el nombre de contenidors.

## 32. Preguntes freqüents {#faq}

**L’aplicació executa ordres `docker`?** No: fa servir l’API de Docker Engine. L’ordre que veus al tauler d’activitat és l’equivalent, perquè la puguis reproduir. Docker Compose sí que s’executa amb el seu programa oficial.

**He de ser administrador?** No. Només necessites accés al sòcol de Docker. Únicament la desinstal·lació de paquets demana permisos, i només per a aquesta ordre.

**Funciona sense Internet?** Sí. Només les operacions de Docker contra registres remots necessiten xarxa.

**Què passa amb els meus contenidors si tanco l’aplicació?** Continuen funcionant: els gestiona Docker. En tancar s’acaben els terminals oberts i es deixen de seguir els registres i les estadístiques.

**Puc perdre dades en editar un contenidor?** No es perd cap volum, carpeta ni imatge, i l’original es conserva com a còpia. Només els fitxers escrits fora dels volums es queden a la còpia, i la revisió t’ho avisa abans.

**Es perden les meves dades en desinstal·lar?** No. Es conserven la teva configuració, el teu historial i tot el de Docker.

**On es desen la configuració i l’historial?** A `~/.config/container-manage-center/` i `~/.local/state/container-manage-center/`.

**Com canvio l’idioma?** A Configuració › Aparença › Idioma.

**Les còpies programades continuen funcionant amb l’aplicació tancada?** Sí: les executa el sistema (systemd o el Programador de tasques de Windows), amb la teva sessió iniciada.

## 33. Glossari {#glossary}

| Terme | Significat |
|---|---|
| Contenidor | Un programa en execució aïllat de la resta de l’equip, creat a partir d’una imatge |
| Imatge | Plantilla de només lectura amb el sistema de fitxers i la configuració per crear contenidors |
| Etiqueta d’imatge (*tag*) | Nom d’una versió d’una imatge, com `nginx:1.27` |
| Etiqueta de contenidor (*label*) | Dada `clau=valor` desada en un contenidor, com `cmc.description` |
| Registre | Servidor del qual es baixen imatges, com Docker Hub |
| Volum | Emmagatzematge que gestiona Docker i que es conserva encara que s’elimini el contenidor |
| Directori vinculat (*bind mount*) | Carpeta o fitxer de l’equip muntat dins d’un contenidor |
| Xarxa | Xarxa virtual que comunica els contenidors entre si i amb l’exterior |
| Port publicat | Port de l’equip que porta a un port del contenidor |
| Política de reinici | Què fa Docker quan un contenidor acaba o l’equip arrenca |
| Docker Engine | El servei que executa els contenidors; CMC es comunica amb ell |
| API de Docker | Interfície amb què es controla Docker Engine; la fan servir la CLI de Docker i CMC |
| Sòcol Unix | Fitxer especial per comunicar-se amb Docker al mateix equip (`/var/run/docker.sock`) |
| Rootless | Docker executat amb un usuari normal, sense privilegis d’administrador |
| Context de Docker | Configuració amb nom que indica a quin Docker es connecta la CLI (`docker context`) |
| `DOCKER_HOST` | Variable d’entorn que indica a quin Docker connectar-se |
| Docker Compose | Eina per definir i executar aplicacions de diversos contenidors amb un fitxer YAML |
| Servei i perfil (Compose) | Un servei és cada part d’un projecte Compose; un perfil agrupa serveis opcionals |
| Docker Exec | Executar un programa dins d’un contenidor que ja està en marxa |
| `stdout` i `stderr` | Sortida normal i sortida d’errors d’un programa |
| Empremta (*fingerprint*) | Resum de la clau d’un servidor SSH (`SHA256:…`) per comprovar-ne la identitat |
| Magatzem segur (clauer) | Lloc del sistema on es desen les contrasenyes xifrades (GNOME Keyring, KWallet, Administrador de credencials) |
| Docker Desktop | Aplicació de Docker per a Windows i macOS |
| Canonada amb nom | Mecanisme de Windows amb què es connecta a Docker Desktop (`\\.\pipe\docker_engine`) |

## 34. Informació legal i llicència {#legal}

**Nom:** Container Manage Center (abreujat **CMC**).

**Llicència:** GNU General Public License, versió 3 o posterior (identificador SPDX `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

Aquest programa és programari lliure: el podeu redistribuir i/o modificar d'acord amb els termes de la Llicència Pública General de GNU publicada per la Free Software Foundation, ja sigui la versió 3 de la Llicència o (a la vostra elecció) qualsevol versió posterior. Es distribueix amb l'esperança que sigui útil, però SENSE CAP GARANTIA; ni tan sols la garantia implícita de COMERCIABILITAT o ADEQUACIÓ A UN PROPÒSIT PARTICULAR. Consulteu la Llicència Pública General de GNU per a més detalls.

Text complet de la llicència: https://www.gnu.org/licenses/gpl-3.0.html (també a `/usr/share/doc/container-manage-center/LICENSE`).

**Components de tercers.** L'aplicació inclou biblioteques i tipografies de tercers que conserven les seves pròpies llicències (MIT, Apache-2.0, MPL-2.0, OFL-1.1 i altres) i els avisos de drets d'autor dels seus autors; no passen a ser GPL ni de Sergio Alfaro Lorite. Els textos complets són a `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md`, i l'avís de drets d'autor del paquet a `/usr/share/doc/container-manage-center/copyright`.

**Codi font.** El codi font corresponent es publica juntament amb cada versió del paquet (`container-manage-center-X.Y.Z-src.tar.gz`).

**Marques.** Container Manage Center és un projecte independent: no està afiliat a Docker, Inc. ni avalat per aquesta empresa. «Docker» és una marca de Docker, Inc. i aquí només s'utilitza per anomenar la tecnologia amb què treballa l'aplicació (Docker Engine, la seva API, Docker Compose i la CLI de Docker).

**Windows.** L’instal·lador de Windows inclou el carregador de WebView2 de Microsoft, cobert per un permís addicional de la GPL v3 §7.

Les mateixes dades són a **Configuració › Legal**.
