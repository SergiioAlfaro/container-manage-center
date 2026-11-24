## 1. Zer da Container Manage Center {#about}

Container Manage Center (**CMC**) **Docker Engine** administratzeko mahaigaineko aplikazio bat da, terminalaren mende egon gabe, ordenagailu honetan eta SSH bidezko urruneko zerbitzarietan. Erakusten duen guztia Dockerri une horretan eskatzen dizkion datu errealak dira, eta ekintza bakoitza eragiketa erreal bat da: ez da ezer simulatzen.

- Docker Enginerekin bere **API ofizialaren** bidez komunikatzen da: Linuxen Unix socketaren bidez, Windowsen Docker Desktopen izendun kanalizazioaren bidez eta urruneko zerbitzariekin SSH bidez.
- **Docker Compose** bere programa ofizialarekin exekutatzen du, argumentuak bereizita eta shellik gabe.
- Eragiketa bakoitza **jarduera-panelean** agertzen da, bere `docker` komando baliokidearekin, zer egin den zehazki jakin dezazun.

Ez da beharrezkoa Docker terminaletik erabiltzen jakitea: atal bakoitzak zer egiten duen azaltzen du egin aurretik, eta zerbait ezabatzen edo aldatzen duten eragiketek berrespena eskatzen dute. Termino bat ezaguna ez bazaizu, kontsultatu **Glosarioa** (33. atala).

## 2. Sistemaren betekizunak {#requirements}

| Elementua | Betekizuna |
|---|---|
| Sistema (Linux) | Debian 12 edo berriagoa, edo Ubuntu 22.04 edo berriagoa, 64 bitekoa (x86_64) |
| Sistema (Windows, probetan) | Windows 10 edo 11, 64 bitekoa, Docker Desktoprekin (28. atala) |
| Docker | Docker Engine 20.10 edo berriagoa (API 1.41+), modu arruntean edo rootless moduan |
| Docker Compose | v2 edo berriagoa, Docker Compose atalerako soilik (`docker compose` edo `docker-compose` v2). Compose v1 ez da onartzen |
| Liburutegiak (Linux) | WebKitGTK 4.1, GTK 3 eta glibc. APTk instalatzen ditu paketearekin |
| Urruneko zerbitzariak | Sistemaren SSH bezeroa (`ssh`) eta, zerbitzarian, Docker Engine 20.10 edo berriagoa |

Docker eta Docker Compose **kanpoko betekizunak** dira: aplikazioak ez ditu instalatzen ez konfiguratzen, baina falta badira nola egin esaten dizu.

Aplikazioak ez du Interneten beharrik. Docker Enginek urruneko erregistroen aurka egiten dituen eragiketek bakarrik (irudiak deskargatzea, Docker Hub-en bilatzea, ordenagailuan ez dauden oinarrizko irudiekin eraikitzea) behar dute Dockerrek sarea izatea.

## 3. Instalazioa eta lehen exekuzioa {#install}

Debianen eta Ubuntun, instalatu paketea APTrekin, mendekotasunak ebazten baititu:

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

Debian 12 eta Ubuntu 22.04rako erabili pakete bateragarria, zeinaren izena `-compat-debian12-ubuntu2204.deb` amaitzen den. Instalatu aurretik, deskarga egiazta dezakezu bertsio bakoitzarekin argitaratzen den `SHA256SUMS` fitxategiarekin (`sha256sum -c SHA256SUMS`).

Aplikazioa menuan agertzen da **Container Manage Center** izenarekin. Zure erabiltzaile arruntarekin exekutatzen da: **ez exekutatu sudo-rekin** (ez da root gisa abiarazten).

**Lehen exekuzioa.** Irekitzean, Docker Engine automatikoki bilatzen du. Aurkitzen badu, **Arbelak** bere egoera erakusten du; bestela, diagnostikoa erakusten du, kausarekin eta konpontzeko urratsekin. Ez da beste ezer konfiguratu behar hasteko.

**Eguneratzea.** Instalatu pakete berria modu berean: zure konfigurazioa eta historia gordetzen dira.

Windowsen instalazioa 28. atalean azaltzen da.

## 4. Leiho nagusia eta nabigazioa {#interface}

Leihoak lau eremu ditu:

- **Alboko barra** (ezkerrean): atalak, taldekatuta. **Ktrl+B** sakatuz edo behealdeko **Tolestu alboko barra** botoiarekin tolesten da.
- **Goiko barra**: atalaren izenburua, **zerbitzari-hautatzailea** (zein Dockerrekin ari zaren lanean: ordenagailu hau edo urruneko zerbitzari bat), **konexio-adierazlea** eta **Jarduera** botoia.
- **Lan-eremua**: aukeratutako atalaren edukia.
- **Jarduera-panela** (eskuinean): exekutatutako eragiketak. **Ktrl+Maius+A** sakatuz edo **Jarduera** botoiarekin irekitzen eta ixten da.

| Taldea | Atalak |
|---|---|
| (goian) | Arbela, Zerbitzariak |
| Baliabideak | Edukiontziak, Kudeaketa masiboa, Irudiak, Sareak, Bolumenak, Lotutako direktorioak |
| Orkestrazioa | Docker Compose |
| Tresnak | Terminalak, Auditoria, Babeskopiak |
| Gainbegiratzea | Monitorizazioa, Gertaerak eta erregistroak |
| (behean) | Konfigurazioa |

Eskuliburu hau **Konfigurazioa › Laguntza eta dokumentazioa** atalean dago. Ezkerreko aurkibideak atal bakoitzera eramaten du, **Aurrekoa** eta **Hurrengoa** botoiek eskuliburua ordenan zeharkatzen dute eta bilatzaileak idatzitako testua duten atalak bakarrik erakusten ditu.

| Lasterbidea | Ekintza |
|---|---|
| Ktrl+B | Alboko barra tolestu edo zabaldu |
| Ktrl+Maius+A | Jarduera-panela ireki edo itxi |
| Ihes | Elkarrizketa-koadro edo menu bat itxi |
| Geziak, Hasiera eta Amaiera | Menuetan mugitu eta Terminalak ataleko bereizleak mugitu |

Egoera-koloreak beti berberak dira: **berdea** dabilenarentzat edo exekutatzen ari denarentzat, **anbarra** oharrentzat eta **gorria** erroreentzat.

## 5. Docker Enginerekiko konexioa {#connection}

Goiko barrako **konexio-adierazleak** egoera erakusten du:

| Itxura | Esanahia |
|---|---|
| Puntu berdea eta `Docker 29.x` | Konektatuta. Sagua gainetik pasatzean socketa ikusten da |
| `Dockerrekin konektatzen…` | Detektatzen edo konektatzen |
| Gorria, `Ez dago Dockerrekiko konexiorik` | Konexiorik gabe. Sakatu diagnostikoa ikusteko |

Modu **automatikoan**, aplikazioak ordena honetan probatzen du: `DOCKER_HOST` aldagaia (`unix://` soilik), Dockerren testuinguru aktiboa, zure rootless socketa eta sistemaren `/var/run/docker.sock` socketa. Konexioa 10 segundoro zaintzen du, eta bere kabuz konektatzen da berriro Docker berriro erabilgarri dagoenean.

Aplikazioak ez du inoiz sudo eskatzen, ez ditu baimenak eta taldeak aldatzen, eta ez ditu zure ordez exekutatzen diagnostikoko argibideak. Zure erabiltzaileak socketa ireki ahal izan behar du: Docker rootless-ekin edo `docker` taldeko kide izanda (praktikan, administratzaile-pribilegioak edukitzearen baliokidea da).

Konfigurazioa berehala gordetzen da `~/.config/container-manage-center/config.toml` fitxategian. Socket jakin bat adierazi nahi baduzu, kontsultatu hurrengo atala.

## 6. Docker rootless eta socket pertsonalizatuak {#rootless}

**Docker rootless**-ekin, deabrua zure erabiltzailearekin exekutatzen da eta socketa `$XDG_RUNTIME_DIR/docker.sock` bidean dago (normalean `/run/user/1000/docker.sock`). Aplikazioak bere kabuz detektatzen du, `rootless` testuinguruaren bidez ere bai, eta **rootless** etiketarekin adierazten du. Segurtasunagatik gomendatutako aukera da.

**Konfigurazioa › Dockerrekiko konexioa** atalean hau egin dezakezu:

- Aukeratu **Socket pertsonalizatua** eta idatzi bide absolutu bat.
- **Probatu konexioa**: bidea egiaztatzen du gorde gabe.
- **Gorde eta konektatu**: konfigurazioa aplikatzen du.
- **Konektatu berriro**: detekzioa errepikatzen du.

`tcp://` konexioak ez dira onartzen. Urruneko zerbitzariak SSH bidez administratzen dira **Zerbitzariak** ataletik (hurrengo atala).

## 7. Urruneko zerbitzariak SSH bidez {#remote-servers}

**Zerbitzariak** ataletik **SSH** bidez sar zaitezkeen beste Linux ordenagailu batzuen Dockerra administra dezakezu, ordenagailu honetakoarekin batera. Konexioak sistemaren SSH bezeroa (`ssh`) erabiltzen du eta `docker system dial-stdio` exekutatzen du zerbitzarian: Docker ez da **inoiz** sarean jartzen (ez dago TCP 2375rik) eta ez da atakarik irekitzen.

**Zerbitzariaren baldintzak**: martxan dagoen SSH zerbitzari bat, Docker Engine 20.10 edo berriagoa (API 1.41) eta Docker erabil dezakeen erabiltzaile bat. Compose-rako, `docker compose` edo `docker-compose` zerbitzarian. Aplikazioak **ez du Docker instalatzen, ez ditu baimenak aldatzen, ez du SSH konfigurazioa ukitzen eta ez du atakarik irekitzen**: zerbait falta bada, zer den eta nola konpondu esaten dizu.

**Gehitu zerbitzaria** (laguntzailea): datu orokorrak (izena, ostalaria, erabiltzailea, 22 ataka), autentifikazioa, Docker (lehenetsia, socket jakin bat edo testuinguru bat) eta **Probatu konexioa**, zazpi urrats egiaztatzen dituena: DNS, SSH konexioa, zerbitzariaren identitatea, autentifikazioa, Docker erabilgarri, baimenak eta APIaren bateragarritasuna.

- **Autentifikazioa**: SSH agentea (gomendatua), gako pribatua, pasaesaldidun gakoa edo pasahitza (ez gomendagarria). Gako pribatua **ez da inoiz kopiatzen**: bide-izena bakarrik gordetzen da.
- **Kredentzialak**: pasaesaldia edo pasahitza konektatzean eskatzen dira. **Gogoratzea** aukeratzen baduzu, **sistemaren biltegi seguruan** gordetzen da (GNOME Keyring, KWallet…); biltegirik ez badago, aldi bakoitzean eskatzen da eta **inoiz** ez da testu arruntean gordetzen. Zerbitzaria editatzean ezaba dezakezu.
- **Zerbitzariaren identitatea**: lehen aldian **hatz-marka** (`SHA256:…`) erakusten da, egiaztatu eta berretsi dezazun. Aldatzen bada, konexioa gelditu egiten da **erdiko gizonaren eraso** posible baten abisuarekin; berrikusi ondoren bakarrik ordezten da. Gako zaharra zure `~/.ssh/known_hosts` fitxategian badago, aplikazioak ez du fitxategi hori aldatzen eta zeuk egiteko komandoa ematen dizu.

**Zenbait zerbitzari**: goiko barrako **hautatzaileak** zein zerbitzaritan ari zaren adierazten du, bere egoerarekin, eta urrunekoak bereizten ditu. Zerbitzaria aldatzean, pantaila osoa zerbitzari horretara pasatzen da. **Ikuspegi orokorrak** denak batera erakusten ditu (egoera, bertsioa, edukiontziak eta baliabideak). Zerbitzari bakoitza independentea da: bat erorita egoteak ez die besteei eragiten, eta eragiketa bakoitza hasi zenuen zerbitzarira joaten da. Berrespenek **zerbitzaria eta helbidea** adierazten dituzte, eta jarduera-panelak eragiketa bakoitzaren zerbitzaria erakusten du eta haren arabera iragaz daiteke.

**Urruneko zerbitzari batean** edukiontziak, irudiak, sareak, bolumenak, terminalak eta Compose lokalean bezala dabiltza. Bide-izenak (Compose fitxategiak, bind mount-en karpetak) **zerbitzarikoak** dira: eskuz idazten dira eta bertan egiaztatzen dira; zerbitzarian fitxategi bat idatzi aurretik helburua berresten da. **Terminalak** atalean **zerbitzariaren SSH shell-a** ere ireki dezakezu, saio mota bereizi bat.

**Deskonektatzeak** SSH saioa eta bere terminalak ixten ditu; **profila ezabatzeak** aplikazioaren datuak bakarrik ezabatzen ditu (profila, kredentzialak eta gordetako identitatea). Bietako batek ere ez ditu ukitzen zerbitzariko edukiontziak, irudiak, sareak, bolumenak edo datuak.

Zerbitzariko `docker` taldeko kide izatea bertan **root** sarbidea edukitzearen baliokidea da: erabaki bere administratzailearekin; Docker rootless da aukera seguruena.

Urruneko zerbitzari batean **Auditoria**, **Babeskopiak** eta **Monitorizazioa** ere badabiltza (23.–25. atalak).

## 8. Arbela eta diagnostikoa {#dashboard}

Konexioarekin, **Arbelak** une horretan lortutako datuak erakusten ditu: Docker Engineren eta APIaren bertsioa, socketa eta konexioaren jatorria, edukiontziak (exekutatzen, pausatuta, geldituta), irudiak, ordenagailuaren datuak (sistema, nukleoa, PUZa, memoria), biltegiratzea eta exekuzioa (kontrolatzailea, datu-direktorioa, cgroups, erregistroak) eta deabruaren abisuak. **Eguneratu** botoiak berriro eskatzen ditu.

Konexiorik gabe, **diagnostikoa** erakusten du: zergati zehatza, konpontzeko urratsak (terminal batean kopiatzeko komandoekin), egiaztatutako socketak haien emaitzarekin eta detektatutako instalazioa. **Konektatu berriro** eta **Konfiguratu konexioa** botoiak goiburuan daude.

Konexio-hutsegite baten kausa ohikoenak eta haien konponbidea 30. atalean daude.

## 9. Edukiontzien kudeaketa {#containers}

**Edukiontziak** atalak edukiontzi guztiak zerrendatzen ditu, `docker ps -a` bezala: egoera, izena eta ID laburra, Compose proiektua, irudia, Dockerren egoera, argitaratutako atakak eta antzinatasuna. **Guztiak / Exekutatzen / Geldituta** iragazi dezakezu, eta izenaren, irudiaren, IDaren edo proiektuaren arabera bilatu. Errenkada bakoitzaren **⋯** menuak (edo eskuineko botoiak) **Editatu konfigurazioa** eta **Sortu irudia edukiontzi honetatik** eskaintzen ditu.

| Egoera | Ekintza erabilgarriak |
|---|---|
| Exekutatzen | Gelditu, Berrabiarazi, Pausatu, Ezabatu |
| Pausatuta | Berrekin, Gelditu, Ezabatu |
| Sortuta edo geldituta | Abiarazi, Ezabatu |

Gelditzeak eta berrabiarazteak 10 segundo itxaroten dute prozesua amaitu arte, behartu aurretik, Dockerrek bezala. Ekintza bakoitzaren ondoren, edukiontziaren benetako egoera kontsultatzen da. Dockerrek ekintza bat baztertzen badu, haren mezua taularen gainean eta jarduera-panelean agertzen da. Zerrendak bere kabuz eguneratzen dira Dockerren zerbait aldatzen denean, terminaletik ere bai.

Ekintza bera edukiontzi askori aldi berean aplikatzeko, erabili **Kudeaketa masiboa** (13. atala).

## 10. Edukiontziak sortu, bikoiztu, ikuskatu eta ezabatu {#container-lifecycle}

**Sortu edukiontzia** botoiak sei fitxa dituen laguntzaile bat irekitzen du (irudia bakarrik da derrigorrezkoa): Orokorra, Ingurunea, Sarea, Biltegiratzea, Baliabideak eta Aurreratua.

- **Komandoa** eta **Sarrera-puntua** terminal batean bezala idazten dira eta argumentutan zatitzen dira; azpian ikusten dituzu. Ez da shellik exekutatzen.
- **Baliokidea** panelak `docker run` baliokidea eta APIra bidaltzen den gorputz zehatza erakusten ditu.
- **Berrikuspenak** erroreak (sortzea eragozten dute) eta abisuak (ataka pribilegiatuak, idazteko baimena duten karpeta sentikorrak, modu pribilegiatua…) markatzen ditu.
- **Sortu** botoiak edukiontzia sortzen du abiarazi gabe; **Sortu eta abiarazi** botoiak abiarazi ere egiten du.

Edukiontzi baten izena sakatzean, haren **xehetasuna** irekitzen da: Laburpena, Erregistroak, Terminala, Estatistikak eta JSON ikuskapena (`docker inspect`-en erantzun osoa, Kopiatu botoiarekin).

**Bikoiztu** botoiak laguntzailea irekitzen du edukiontziaren konfigurazioarekin eta `<izena>-copia` izenarekin, eta eramaten ez diren ezarpenen berri ematen du.

**Ezabatu** botoiak berrespena eskatzen du. Exekutatzen edo pausatuta dagoen edukiontzi bat **Behartuta gelditu eta ezabatu** markatzen baduzu bakarrik ezabatzen da. Aukeran, haren bolumen anonimoak ezabatzen dira; izena duten bolumenak eta lotutako karpetak ez dira ukitzen. Ezin da desegin.

Xehetasunaren erregistroak, estatistikak eta terminala 21. atalean azaltzen dira.

## 11. Edukiontzi baten konfigurazioa editatzea {#container-edit}

**Editatu konfigurazioa** edukiontzi bakoitzaren **⋯** menuan dago (baita errenkadaren gainean eskuineko botoia sakatuta ere) eta xehetasunaren goiburuan. Konfigurazioa fitxatan antolatzen da: **Orokorra**, **Sarea**, **Biltegiratzea**, **Aldagaiak**, **Atakak**, **Baliabideak** eta **Aurreratua**. Aldaketa bakoitzak nola aplikatzen den adierazten du:

| Aldaketa | Nola aplikatzen den |
|---|---|
| Izena | Zuzenean (`docker rename`) |
| Berrabiarazte-politika, PUZa, memoria eta erreserbatutako memoria | Zuzenean (`docker update`), edukiontzia berrabiarazi gabe |
| Sareak konektatzea edo deskonektatzea | Zuzenean eta berehala (`docker network connect` / `disconnect`) |
| Ingurune-aldagaiak, atakak, bolumenak eta karpetak, deskribapena | Edukiontzia **birsortu** behar da |
| PUZ edo memoria-muga bat kentzea | Birsortu behar da (`docker update`-k ez daki kentzen) |
| Irudia, komandoa, erabiltzailea, laneko direktorioa eta **Aurreratua** fitxako gainerako aukerak | Irakurtzeko soilik: Dockerrek ez du haiek aldatzen uzten. Erabili **Bikoiztu** |

- **Sarea**: sareak, IPa, MACa eta aliasak erakusten ditu. Dockerrek ez du uzten lehendik dagoen konexio baten aliasak edo IPa aldatzen: deskonektatu eta konektatu berriro behar da. Edukiontziaren sare bakarra bada, deskonektatu aurretik abisatzen da.
- **Aldagaiak**: sekretuak diruditen balioak pantailan ezkutatzen dira eta ez dira inoiz jarduera-panelean gordetzen.
- **Deskribapena**: edukiontziaren `cmc.description` etiketan gordetzen da.

**Berrikusi aldaketak** botoiak uneko konfigurazioa eta berria alderatzen ditu: zer aldatzen den (aurretik eta ondoren, elementuz elementu), zer mantentzen den eta zerk eskatzen duen birsortzea. Ez da ezer aplikatzen zuk berretsi arte.

**Birsortze segurua.** Aldaketa batek birsortzea eskatzen duenean, laguntzaileak:

1. Irudia, beste edukiontzi batzuekiko izen- eta ataka-gatazkak eta segurtasunez birsortzea eragozten duten arrazoiak egiaztatzen ditu: Compose edo Swarm edukiontziak, `--rm` aukerarekin sortuak, pausatuta edo berrabiarazten daudenak. Kasu horietan ez du jarraitzen uzten.
2. Banan-banan onartu behar dituzun arriskuak erakusten ditu: bolumenetatik kanpo idatzitako fitxategiak (ez dira edukiontzi berrira pasatzen), muntatuta egoteari uzten dioten muntaketak, honen sarea erabiltzen duten edukiontziak…
3. Berrestean: edukiontzia gelditzen du martxan badago, izena `<izena>-cmc-backup-<data>` izatera aldatzen dio (babeskopia), berria sortzen du jatorrizko konfigurazio **osoarekin** gehi zure aldaketak, haren sareak konektatzen ditu eta, martxan bazegoen, abiarazi eta segundo batzuetan funtzionatzen jarraitzen duela egiaztatzen du.
4. Zerbaitek huts egiten badu, edukiontzi berria ezabatzen du (haren bolumenak ukitu gabe), jatorrizkoari bere izena itzultzen dio eta berriro abiarazten du.

**Ez da inoiz bolumenik, karpetarik edo irudirik ezabatzen.** Bolumen anonimoak berriro muntatzen dira beren edukiarekin. Kopia geldituta geratzen da **Orokorra › Babeskopiak** atalean; hortik **Leheneratu** dezakezu (uneko edukiontzia baztertu egiten da, ez da ezabatzen) edo **Ezabatu**, behar ez duzunean. Eragiketa guztiak jarduera-panelean geratzen dira, balio sentikorrik gabe.

## 12. Irudi bat edukiontzi batetik sortzea {#container-commit}

**Sortu irudia edukiontzi honetatik** (zerrendako **⋯** menua eta xehetasunaren goiburua) `docker commit`-en baliokidea da: edukiontziaren fitxategi-sistemaren uneko egoera irudi berri gisa gordetzen du.

- Adierazi **biltegia** eta **etiketa** (hutsik = `latest`); aukeran, **egilea** eta **mezua**. Sortu aurretik izen osoa ikusten da eta balidatu egiten da.
- **Ez ditu barne hartzen bolumenak eta muntatutako karpetak**: edukiontziaren idazketa-geruza bakarrik. Elkarrizketa-koadroak kanpoan geratzen diren muntaketak zerrendatzen ditu.
- Irudiak **edukiontziaren konfigurazioa gordetzen du**, ingurune-aldagaiak barne (pasahitzak edo tokenak dituztenak ere bai): ez argitaratu erregistro batean sekreturik badu.
- Edukiontzia exekutatzen ari bada, lehenespenez segundo batzuetan **pausatzen** da irudia sortzen den bitartean, koherentea izan dadin; desmarkatu dezakezu. Ez da inoiz gelditzen.
- Izen hori duen irudi bat badago, abisatu egiten da eta esplizituki onartu behar da: aurrekoa ez da ezabatzen, etiketarik gabe geratzen da.
- Jatorrizko edukiontzia ez da aldatzen. Amaitzean, **Ikusi irudia** botoiak haren xehetasuna irekitzen du Irudiak atalean.

## 13. Kudeaketa masiboa {#bulk}

**Kudeaketa masiboa** («Baliabideak» atalean) eragiketa edo komando bera aplikatzen die edukiontzi askori aldi berean, baita zerbitzari batzuetan ere. Ordenagailu honetako eta **konektatutako** urruneko zerbitzarietako edukiontziak biltzen ditu; besteak ohar batean adierazten dira. Fitxatan banatuta dago: **Laburpena**, **Edukiontziak**, **Eragiketak**, **Konfigurazio masiboa**, **Komando masiboak** eta **Historia**.

- **Laburpena**: guztira, exekutatzen, geldituta, pausatuta eta errorearekin. Kontagailu batean sakatuz egoera horren arabera iragazten da. **Eguneratu** botoiak eta eguneratze automatikoak (5 s-tik 5 min-ra, desaktiba daiteke) datuak berriro eskatzen dituzte.
- **Edukiontziak**: bilatu izenaren edo IDaren arabera; iragazi egoeraren, irudiaren, zerbitzariaren eta etiketaren arabera (`gakoa` edo `gakoa=balioa`); ordenatu zutabe baten izenburuan sakatuz. Errenkada bakoitzeko kontrol-laukiarekin, orrialdekoarekin edo **iragazitako guztia** aukeratuz hautatzen da.
- **Eragiketak**: abiarazi, gelditu, berrabiarazi, pausatu, berrekin eta ezabatu. Uneko egoeran aplikatu ezin dena **saltatu** egiten da eta azaltzen da.
- **Konfigurazio masiboa**: hautapenaren berrabiarazte-politika erakusten du eta denetan aldatzeko aukera ematen du (beroan, `docker update` erabiliz; ez `--rm` aukerarekin sortutakoetan). **Etiketak** erakutsi bakarrik egiten dira: Dockerrek ez du uzten dagoeneko existitzen den edukiontzi batean aldatzen (berriro sortu beharko litzateke). Edukiontzi jakin baterako, erabili **Bikoiztu**.
- **Historia**: aurreko eragiketa eta komando masiboak, edukiontzi bakoitzean egindakoarekin.

**Aplikatu aurretik** berrespen bat erakusten da: eragiketa, zerbitzariak, eragindako eta saltatutako edukiontziak, ondorioak eta aldi berean zenbat eragiketa egiten diren (1etik 16ra). **Ezabatu** aukerak edukiontzi kopurua idazteko eskatzen du eta **ez ditu inoiz bolumenak ez irudiak ezabatzen**.

**Emaitzak**: edukiontzi bakoitzak berea du: zuzena (Dockerrek berretsi du), huts egina (errorearekin), saltatua, bertan behera utzia edo **ezezaguna** (konexioa berrespenaren aurretik eten da; eguneratu egoera ikusteko). Batek huts egiten badu, besteek jarraitzen dute; ez da ezer desegiten ez automatikoki berriro saiatzen. **Utzi bertan behera zain dagoena** aukerak aginduak bidaltzeari uzten dio. Agindu bakoitza jarduera-panelean erregistratzen da «Masiboa» bereizgarriarekin, eta **Ikusi jardueran** aukerak eragiketa hori bakarrik erakusten du.

**Komando masiboek** komando bera exekutatzen dute exekutatzen ari diren hainbat edukiontziren barruan, Docker Exec erabiliz, ordenagailu honetako inolako shelletatik pasatu gabe:

1. Aukeratu modua: **Exekutagarria argumentuekin** (argumentu bat lerro bakoitzeko, dagoen bezala bidalia) edo **Sekuentzia interpretatzaile batekin** (hainbat lerro `/bin/sh`, `bash`, `python3`… interpretatzaileentzat). Adibide bat edo gordetako komando ohiko bat ere karga dezakezu.
2. Behar izanez gero, adierazi erabiltzailea, lan-direktorioa, gehieneko denbora (1etik 3600 s-ra) eta aldi berean zenbat exekuzio egiten diren.
3. Berrikusi **Aurrebista**: `docker exec` baliokidea, Dockerrek jasotzen dituen argumentu zehatzak eta arrisku-maila (Informatiboa, Aldaketa, Administratiboa edo Suntsitzailea izan daiteke). Sailkapena orientagarria da: ez du komandoa berrikustea ordezkatzen.
4. Berretsi. Arrisku administratiboko edo suntsitzaileko komandoek edukiontzi kopurua idazteko eskatzen dute.

Edukiontzi bakoitzak bere egoera, irteera-kodea, iraupena, eta `stdout` eta `stderr` bereizita ematen ditu (kopiatu eta Markdown edo JSON formatura esporta daitezke, sekretuak ezkutatuta). **Utzi bertan behera zain daudenak** aukerak ez du komandoa abiarazten oraindik hasi ez direnetan; **Eten aktiboak** aukerak, gainera, martxan daudenen zain egoteari uzten dio. Dockerrek ez du uzten jada abiarazitako prozesu bat geldiarazten: denbora agortzean edo etetean, prozesuak edukiontziaren barruan jarrai dezake, eta CMCk **inoiz ez** du edukiontzia gelditzen ez berrabiarazten hura geldiarazteko. Komando ohikoak gorde daitezke, **inoiz ez sekretuekin**.

## 14. Irudiak {#images}

**Irudiak** atalak tokiko irudiak zerrendatzen ditu, `docker image ls` bezala: etiketak, ID laburra, tamaina, antzinatasuna eta zer edukiontzik erabiltzen dituzten. **Guztiak / Erabiltzen / Erabili gabe / Etiketarik gabe** iragazkiak eta etiketaren edo IDaren araberako bilaketa.

- **Xehetasuna**: Laburpena (aldagaien balio sentikorrak ezkutatuta), Geruzak (`docker image history` bezala), hura erabiltzen duten Edukiontziak eta JSON ikuskapena.
- **Etiketatu** botoiak `biltegia:etiketa` izen bat gehitzen du.
- **Ezabatu**: irudiak hainbat etiketa baditu, bakarra ken dezakezu; irudi osoa ezabatzeko behartu egin behar da. Dockerrek ez ditu ezabatzen exekutatzen ari diren edukiontzien irudiak.
- **Garbitu** botoiak edukiontzirik erabiltzen ez dituen etiketarik gabeko irudiak ezabatzen ditu edo, berrespen gehigarri batekin, erabiltzen ez diren guztiak.

## 15. Irudiak deskargatu eta eraiki {#pull-build}

**Deskargatu irudia** botoiak erreferentzia eskatzen du (`nginx:1.27`, `erabiltzailea/app`, `erregistroa.adibidea.com:5000/app:v2`; etiketarik gabe, `latest` erabiltzen da). Kredentzialak:

| Aukera | Zer egiten duen |
|---|---|
| Dockerrenak | `docker login`-ekin gordetakoa erabiltzen du (edo haren *credential helper*-a) |
| Bat ere ez | Deskarga anonimoa |
| Erabiltzailea eta pasahitza | Deskarga honetarako bakarrik: ez dira gordetzen eta ez dira jarduera-panelean agertzen |

**Deskargak** panelak aurrerapen osoa eta geruzakoa erakusten ditu; bitartean atalez alda dezakezu eta **Utzi** sakatu. **Bilatu Docker Hub-en** botoiak `docker search` bezala bilatzen du (Docker Enginek egiten du, eta Internet behar du).

**Eraiki irudia** botoiak laguntzaile bat irekitzen du: testuingurua (bidaltzen den karpeta, `.dockerignore` errespetatuz) eta Dockerfile-a, izenak (`--tag`), argumentuak (`--build-arg`), metadatuak (`--label`), azken etapa, cacherik gabe, oinarrizko irudi berriak bilatu eta eraikitzailea (**BuildKit** edo **klasikoa**). Urratsak beren egoerarekin ikusten dira, eta irteera `docker build --progress=plain` bezala; bertan behera utz daiteke.

## 16. Sareak {#networks}

**Sareak** atalak sareak zerrendatzen ditu, `docker network ls` bezala, kontrolatzailearekin, azpisareekin eta edukiontziekin. **bridge**, **host** eta **none** sareak aurredefinituak dira eta ezin dira ezabatu.

- **Sortu sarea**: izena, kontrolatzailea (`bridge`, `macvlan`, `ipvlan`, `overlay` edo beste bat), barneko sarea, IPv4 helbideak (azpisarea, atebidea eta tartea) eta IPv6, kontrolatzailearen aukerak eta metadatuak. Komando baliokidea eta eskaera zehatza ikusten dira, eta erroreak markatu egiten dira Dockerri ezer bidali aurretik.
- **Konektatu** edukiontzi bat (martxan edo geldituta), aukerako aliasekin eta IP finkoarekin. **Deskonektatu** botoiak berrespena eskatzen du.
- **Ezabatu** eta **Garbitu** botoiek abisatu egiten dute sarea konfiguratuta duten eta gero abiarazi ezin izango diren geldituta dauden edukiontziak badaude.

## 17. Bolumenak {#volumes}

**Bolumenak** atalak bolumenak zerrendatzen ditu, `docker volume ls` bezala, haien kontrolatzailearekin eta muntatzen dituzten edukiontziekin. Anonimoak horrela markatzen dira. **Kalkulatu tamainak** botoiak bakoitzaren tokia eskatzen dio Dockerri (`docker system df -v` bezala); ez da automatikoki egiten, denbora behar baitezake.

- **Sortu bolumena**: izena (aukerakoa) eta mota: tokikoa, tmpfs, NFS, ordenagailuko karpeta bat edo beste kontrolatzaile bat.
- **Xehetasuna**: direktorioa, aukerak, metadatuak, tamaina, edukiontziak haien bidearekin eta moduarekin, eta JSON ikuskapena. **Sortu edukiontzia harekin** botoiak `/data` bidean muntatzen du.
- **Ezabatu**: Dockerrek ez du ezabatzen edozein edukiontzik muntatzen duen bolumenik, geldituta egon arren.
- **Garbitu**: erabili gabeko bolumen anonimoak ezabatzen ditu edo, berrespenarekin, erabili gabeko bolumen guztiak.

**Ezabatutako bolumen baten datuak ezin dira berreskuratu.** Behar badituzu, egin lehenik babeskopia bat (24. atala).

## 18. Lotutako direktorioak {#binds}

Atal honek edukiontzien barruan muntatutako zure ordenagailuko karpetak eta fitxategiak erakusten ditu, bidearen arabera taldekatuta: mota, baimenak eta jabea, badauden, zer edukiontzik muntatzen dituzten, non eta zer sarbiderekin. Arreta behar dutenak seinalatzen dira: **Dockerren socketa** (Dockerren kontrola ematen dio edukiontziari), sistemaren erroa edo direktorioak eta jada existitzen ez diren bideak.

Dockerrek ez du uzten karpeta bat gehitzen lehendik dagoen edukiontzi bati. **Lotu karpeta bat…** botoiak karpeta-hautatzailea irekitzen du eta, ondoren, edukiontzien laguntzailea, karpeta hori `/mnt/<izena>` bidean muntatuta duela.

## 19. Docker Compose: proiektuak {#compose}

**Docker Compose** atalak Compose v2 edo berriagoa behar du; instalatuta ez badago, nola instalatu azaltzen du. Eragiketa guztiek Docker Compose ofiziala exekutatzen dute eta jarduera-panelean agertzen dira komando zehatzarekin.

Zerrendak Dockerren edukiontziak dituzten proiektuak erakusten ditu (terminaletik hedatutakoak ere bai) eta zuk gehitzen dituzunak, haien egoerarekin (martxan, partziala, geldituta edo hedatu gabe), karpetarekin eta zerbitzuekin.

- **Ireki proiektua…**: aukeratu haren `compose.yaml` (eta fitxategi gehigarriak, konbinatzen diren ordenan).
- **Proiektu berria**: aukeratu karpeta bat, idatzi edukia (txantiloi batetik abiatzen da) eta **Sortu proiektua**: Docker Composerekin balidatzen da `compose.yaml` sortu aurretik.
- **Kendu zerrendatik** botoiak zerrendatik kentzen du bakarrik; ez ditu ukitzen Docker eta fitxategiak.

## 20. Docker Compose: zerbitzuak, hedapena eta profilak {#compose-projects}

Proiektu baten xehetasunean:

- **Zerbitzuak**: zerbitzu bakoitza bere irudiarekin, egoerarekin, edukiontziekin, atakekin eta **profilekin**. Errenkada bakoitzean zerbitzu hori hedatu, abiarazi, gelditu, berrabiarazi edo haren erregistroak ikus daitezke.
- **Hedatu** (`docker compose up --detach`): zerbitzu guztiak edo aukeratutakoak, aktibatu beharreko **profilak**, aurretik eraiki, irudiak deskargatzeko politika, edukiontziak berriro sortu eta umezurtzak ezabatu. Elkarrizketa-koadroak komando zehatza erakusten du.
- **Abiarazi, Gelditu, Berrabiarazi**, **Deskargatu irudiak** eta **Eraiki**.
- **Ezabatu** (`docker compose down`): proiektuaren edukiontziak eta sareak; aukeran, haren izendun bolumenak (haien datuak galtzen dira) eta irudiak. Berretsi egin behar da.
- Abian dagoen eragiketaren **Irteera** (**Eten** daiteke) eta zerbitzuen **Erregistroak** zuzenean.
- **Fitxategia**: fitxategiak editatzen ditu; **Balidatu** botoiak Docker Composerekin egiaztatzen ditu gorde gabe, eta **Gorde** botoiak aurretik balidatzen ditu eta ez ditu gainidazten beste programa batek aldatu baditu.
- **Konfigurazioa**: azken konfigurazioa, Docker Composeren arabera. Sekretuak izan ditzake; beraz, ezkutatuta dago zuk eskatu arte, eta ez da jarduera-panelean gordetzen.

## 21. Edukiontzi baten erregistroak, estatistikak eta terminala {#logs-stats-terminal}

Edukiontzi baten xehetasunean:

- **Erregistroak**: irteera zuzenean (stderr gorriz). Aukera dezakezu aurreko zenbat lerro kargatu, iragazi, denbora-zigiluak ikusi, amaiera jarraitu, ikusgai dagoena kopiatu eta jarraitzeari utzi. Azken 10 000 lerroak gordetzen dira.
- **Estatistikak**: PUZa, memoria, sarea, diskoa eta prozesuak segundoro, `docker stats`-ek bezala kalkulatuta.
- **Terminala**: **Ireki terminala** botoiak shell bat abiarazten du `docker exec -it` bidez (Automatikoak `bash` erabiltzen du badago, eta bestela `sh`; bat aukera dezakezu ere). **Itxi saioa** botoiak shella eta edukiontzi barruan utzitako prozesuak amaitzen ditu.

Eragiketa errealak dira, irekita dauden bitartean jarduera-panelean agertzen dira eta xehetasunetik irtetean ixten dira. Hainbat terminalekin aldi berean lan egiteko, erabili **Terminalak** atala.

Erabileraren bilakaera denboran zehar ikusteko eta alertak jasotzeko, erabili **Monitorizazioa** (25. atala).

## 22. Aldibereko terminalak {#terminals}

**Terminalak** atalak hainbat terminal erreal (`docker exec -it`) aldi berean ireki eta ikusteko aukera ematen du, exekutatzen ari diren edukiontzietan, edukiontzi berean edo beste batzuetan.

- **Terminal berria**: aukeratu exekutatzen ari den edukiontzi bat, shella (Automatikoa, bash edo sh) eta, nahi baduzu, saioarentzako izen bat.
- Panel bakoitzak **edukiontzia**, **saioaren izena** eta **konexio-adierazle** bat (Konektatzen, Konektatuta, Amaituta edo Errorea) erakusten ditu, **izena aldatzeko**, **maximizatzeko** eta **ixteko** botoiekin. Izenaren gainean klik bikoitza eginez ere aldatzen da izena.
- **Banaketa**: terminal batek toki osoa hartzen du; bi, bi zutabetan; hiru, bat ezkerrean eta bi eskuinean; lau, 2×2 saretan; lau baino gehiago, biko errenkadetan, korritze bertikalarekin.
- **Bereizleak** saguarekin arrastatzen dira. Fokua bereizle batean dagoela, geziek mugitzen dute, Hasiera eta Amaiera teklek muturretara eramaten dute, eta klik bikoitzak tokia berdin banatzen du.
- Saio bakoitza independentea da: bere shella, sarrera eta irteera, tamaina eta egoera ditu. Batean egiten duzunak ez die besteei eragiten.
- Saioak **irekita jarraitzen dute atalez aldatzean**. **Itxi guztiak** botoiak berrespena eskatzen du. Gehienez 12 saio onartzen dira aldi berean.
- Saio bat **ixtean**, haren shella eta edukiontzi barruan atzeko planoan utzitako prozesuak amaitzen dira. Edukiontzia **gelditu** edo **ezabatzen** bada, panelak adierazi egiten du; amaitutako saio bat **berriro ireki** daiteke edukiontzia oraindik badago.
- **Aplikazioa ixtean**, saio guztiak amaitzen dira.

Jarduera-panelak saio bakoitzaren irekiera, itxiera eta erroreak erregistratzen ditu, baina ez idazten dena eta terminalaren irteera.

## 23. Segurtasun-auditoria {#audit}

**Tresnak › Auditoria** atalak aukeratutako zerbitzariaren segurtasuna berrikusten du (ordenagailu hau edo urruneko bat) eta **ez du ezer aldatzen**: konfigurazioa irakurri bakarrik egiten du.

1. Aukeratu **zer auditatu**: edukiontzi guztiak (exekutatzen ari direnak eta geldituak) edo bat; baita irudiak eta Compose proiektuak ere (haien konfigurazioa `docker compose config` erabiliz irakurtzen da, hedatu gabe).
2. Sakatu **Auditatu**. Aurrerapena ikusten da eta bertan behera utz daiteke.
3. Berrikusi **aurkikuntzak**, mailaren arabera (kritikoa, altua, ertaina, baxua) eta edukiontziaren, zerbitzuaren edo irudiaren arabera taldekatuta. Bakoitzak azaltzen du zer gertatzen den, **zergatik den garrantzitsua** eta zer **gomendatzen den**. Mailaren, motaren eta testuaren arabera iragaz daitezke.

Ordenagailuarentzako arriskuak egiaztatzen dira (Docker socket muntatua, karpeta sentikorrak, gaitasunak, modu pribilegiatua, ordenagailuaren sarea, PID eta IPC partekatuak, seccomp, AppArmor edo SELinux desaktibatuta, gailuak eta interfaze guztietan irekitako atakak) eta gogortzea (prozesua root gisa, fitxategi-sistema aldagarria, memoria-, CPU- edo prozesu-mugarik gabe, `no-new-privileges` gabe eta urtebete baino zaharragoak diren irudiak). Docker rootless edo `userns-remap` erabiliz, edukiontziaren root-a baxu gisa markatzen da.

- **Proposatu zuzenketak** (edukiontziak; botoiak zenbat diren adierazten du): **Editatu konfigurazioa** irekitzen du, aldaketak zirriborroan jarrita. Berrikusi eta aplikatu beste edozein edizio bezala (11. atala); baztertzen badituzu, ez da ezer aldatzen.
- **Ahultasunak**: **Trivy** edo **Grype** instalatuta badituzu haien datu-basea deskargatuta, markatu laukia eta irudi bakoitzak zenbat ahultasun ezagun dituen erakusten du, larritasunaren arabera. Aplikazioak ez ditu tresna horiek instalatzen. Horiek gabe, edo urruneko zerbitzari batean, txostenak «Aztertu gabe» dio, inoiz ez «0».
- **Esportatu** JSON, Markdown edo HTML formatura: fitxategia zure erabiltzaileak bakarrik irakur dezake eta sekretuak ezkutatzen dira.
- **Aurreko auditoriak**: zerbitzari bakoitzaren azken 200ak, berriro irekitzeko.
- **Edukiontziak beste tresnetan**: auditatutako edukiontzi bakoitzerako, bere aurkikuntzarik okerrena, monitorizazioak zaintzen duen, bere babeskopia berriena eta hura barne hartzen duten babeskopia-atazak, **Ikusi Monitorizazioan** eta **Prestatu kopia bat** lasterbideekin.

## 24. Babeskopiak {#backup}

**Tresnak › Babeskopiak** atalak aukeratutako zerbitzariaren babeskopiak egiten ditu (ordenagailu hau edo urruneko bat) eta leheneratzen ditu.

1. **Helburuko karpeta**: **Aukeratu karpeta** botoiak sistemaren elkarrizketa-koadroa irekitzen du. Barruan `container-manage-center-copias/<zerbitzaria>/<kopia>/` sortzen da, zure erabiltzailearentzat bakarrik. Ezin dira sistemaren karpetak erabili; kanpoko disko bat edo jada muntatutako sareko karpeta bat, bai.
2. **Kopia berria**: markatu kopiatu nahi duzuna.
3. **Egin kopia orain** botoiak aurrerapena erakusten du eta bertan behera utz daiteke. Bertan behera utzitako edo etendako kopia bat ez da inoiz ontzat ematen.

| Elementua | Nola kopiatzen den |
|---|---|
| Bolumenak | Dockerren APIaren bidez, inoiz abiarazten ez den edukiontzi laguntzaile batekin. Martxan dagoen edukiontzi batek erabiltzen badu, aukeratu **beroan** (kopia ez bermatua) edo **edukiontziak geldituz** (kopiatzen den bitartean gelditzen dira eta berriro abiarazten dira) |
| Datu-baseak | Martxan dauden MariaDB, MySQL, PostgreSQL, Redis edo MongoDB: irauleketa motorraren beraren tresnak egiten du edukiontziaren barruan, bere aldagaietako pasahitzarekin. Koherentea da idazketak egon arren |
| Edukiontzien konfigurazioa | Berriro sortzeko, sekretuak dituzten aldagairik gabe |
| Compose proiektuak | Ordenagailu honetan bakarrik: haien fitxategiak. `.env` fitxategiak «Sartu .env fitxategiak» markatzen baduzu bakarrik |
| Aplikazioaren konfigurazioa | Hobespenak, zerbitzariak eta atazak (pasahitzak giltzatakoan daude eta ez dira kopiatzen) |

- **Babeskopiak**: zerbitzariarenak, beren egoerarekin (Osoa, Osatugabea), elementuak, tamaina eta oharrak. **Egiaztatu** botoiak fitxategi bakoitzaren SHA-256 batura berriro kalkulatzen du eta esaten du ea bat aldatuta dagoen, falta den edo soberan dagoen.
- **Leheneratu**: aukeratu zein elementu eta nora, eta sakatu **Aurrebista**, osotasuna egiaztatzen duena eta bakoitzarekin zer egingo den esaten duena. Bolumen bat **berri** batean edo huts batean leheneratzen da, inoiz ez datuak dituen baten gainean. Edukiontzi bat berria sortzen da, beste izen batekin eta abiarazi gabe. Fitxategiak karpeta berri batean idazten dira.
- **Datu-base bat leheneratzeak bere uneko datuak ordezten ditu**: «leheneratu» idatzi behar da berresteko.
- **Programatutako atazak**: **Gorde ataza gisa** botoiak hautapena gordetzen du izen batekin, maiztasun batekin (egunerokoa, astekoa edo hilekoa) eta atxikipen batekin (lehenespenez, 7 egunero, 4 astero eta 3 hilero). Atxikipenak **ataza horren** kopia zaharrak bakarrik ezabatzen ditu. **Programatu** aukerak systemd-ren erabiltzaile-tenporizadore bat sortzen du (Windowsen, Ataza-programatzailearen ataza bat), aplikazioa itxita egon arren funtzionatzen duena; aurretik zer sortuko duen erakusten dizu eta baimena ematen baduzu bakarrik sortzen du. Ez da administratzaile izan behar. Linuxeko erabiltzaile-tenporizadoreak saioa hasita dagoenean bakarrik exekutatzen dira, zuk `loginctl enable-linger` aktibatzen ez baduzu behintzat.
- **Historia**: kopia bakoitza, eskuzkoa edo programatua, bere emaitzarekin eta atxikipenak ezabatutako kopia zaharrekin.

Babeskopiak **ez daude zifratuta**: zure datuak dauden bezala dituzte. Ordenagailutik kanpo gordetzen badituzu, erabili disko zifratu bat.

## 25. Monitorizazioa eta alertak {#monitoring}

**Gainbegiratzea › Monitorizazioa** atalak aukeratutako zerbitzariko edukiontzien erabilera erakusten du denboran zehar. Ez du ezer instalatzen zerbitzarian: `docker stats`-ek bezala galdetzen dio Dockerri, baita SSH bidez ere.

- **Aktibatu**: markatu «Monitorizatu zerbitzari hau». Zerbitzari bakoitzean bereiz aktibatzen da eta aktibo jarraitzen du aplikazioa berriro irekitzean. **Tartea** (2tik 60 s-ra) guztientzat berdina da; **Pausatu** botoiak lagin guztiak gelditzen ditu.
- **Laburpena**: ordenagailuaren CPU eta memoria Dockerren arabera, martxan, pausatuta eta geldituta dauden edukiontziak, eta erabilera osoa grafikoetan. Taulak, edukiontzi bakoitzeko, CPU, memoria eta haren muga, sarea, diskoa, prozesuak, martxan daraman denbora eta berrabiarazteak erakusten ditu. Sakatu izen batean bere grafikoak ikusteko. CPUa CPU baten %-tan dago, `docker stats`-en bezala: bi CPU oso erabiltzen dituen edukiontzi batek % 200 markatzen du.
- **Epea**: 5, 15 eta 30 minutu memoriatik datoz (aplikazioa ireki zenuenetik); 1, 6 eta 24 ordu, zure ordenagailuan gordetzen den minutuko laburpenetik.
- **Konparatu**: martxan dauden 4 edukiontzi arte eta metrika bat grafiko berean.
- **Konexiorik gabe**: zerbitzariak erantzuteari uzten badio, grafikoek tarte marratu bat erakusten dute. Ez da daturik asmatzen.

**Alertak**: CPU altua eta memoria altua (abisu-atalase batekin eta ebazpen-atalase batekin, igoera txiki bakoitzean ez abisatzeko), edukiontzia erori eta Dockerrek berriro abiarazten du, memoria falta (OOM), erroreekin amaitzen da inork gelditu gabe eta zerbitzariarekiko konexioa galtzen da. Zuk edukiontzi bat gelditzea ez da errorea. Abisu baten ondoren, alerta bera ez da errepikatzen isiltasun-minutuetan (15 lehenespenez). **Mahaigaineko jakinarazpenak** ez dira ordenagailutik ateratzen; **Probatu jakinarazpena** botoiak proba bat bidaltzen du. Alerta guztiak **Alerten historian** geratzen dira.

Monitorizazioak ez du lagin bakoitza jarduera-panelean idazten: zerbitzari bakoitzean noiz hasten eta noiz amaitzen den bakarrik.

## 26. Gertaerak, jarduera eta eragiketen historia {#activity}

Eragiketa bakoitza **jarduera-panelean** agertzen da: ordua, **API** bereizgarria (Docker Engineri egindako eskaera), **CLI** (prozesua, `docker compose` edo `apt-get` bezala) edo **APP** (aplikazioaren beraren eragiketa, hizkuntza-aldaketa bat bezala, edo urratsak bereiz agertzen diren birsortze baten laburpena), mota, baliabidea, zerbitzaria, iraupena eta egoera. Lerro nagusia komando baliokidea da, kopia dezakezuna. Sarrera bat zabaltzean benetan exekutatutako eragiketa, emaitza, errorea eta haren HTTP kodea (huts egin badu) eta prozesuen irteerak ikusten dira. Balio sekretuak `********` erabiliz ezkutatzen dira.

**Gertaerak eta erregistroak** atalak lau fitxa ditu:

- **Zuzenean**: saio honetako Docker gertaerak (abiarazten den edukiontzi bat, deskargatutako irudi bat…), aplikazio honetatik edo beste edozein tresnatatik etorrita ere. Iragazi eta **Pausatu** daitezke.
- **Gordeak**: aplikazioa irekita zegoela jasotako gertaerak, baita aurreko saioetakoak ere (20.000 berrienak).
- **Kontsultatu Dockerri**: epe bateko gertaerak (`docker events --since … --until …`). Aplikazioa itxita zegoela gertatutakoa barne hartzen du, baina Dockerrek azken 256ak bakarrik gordetzen ditu.
- **Eragiketen historia**: aplikazioak egin duena, baita aurreko saioetan ere (10.000 berrienak).

Historiak zer egin den, noiz eta zer emaitzarekin gordetzen du, baina inoiz ez eskaeren gorputza, komandoak edo irteerak, pasahitzak izan ditzaketelako. `~/.local/state/container-manage-center/` direktorioan dago, zure erabiltzaileak bakarrik irakur dezakeena. Fitxa bakoitzetik ezaba dezakezu edo Konfigurazioan gordetzeari utz diezaiokezu.

## 27. Aplikazioaren konfigurazioa {#settings}

- **Itxura**: gaia (Sistema, Argia, Iluna) eta hizkuntza, ordena honetan: **English**, **Español** (lehenetsia), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** eta **Português**. Hizkuntza bakoitza bere izenarekin agertzen da. Hizkuntza gorde egiten da eta mantendu egiten da aplikazioa berriro irekitzean. Kolore nagusia **morea** da eta ezin da aldatu.
- **Jarduera-panela**: eragiketa batek huts egiten duenean bakarrik ireki, gordetako eragiketak (50–10.000) eta **Gorde historia saioen artean**.
- **Dockerrekiko konexioa** (5. eta 6. atalak) eta **Docker Compose**-ren egiaztapena.
- **Eguneratzeak**: bertsioak non argitaratzen diren eta **Egiaztatu orain**. Aplikazioak ez du inoiz ezer deskargatzen ez instalatzen.
- **Sistema**: aplikazioaren, paketearen, WebKitGTK eta glibc-ren, sistemaren eta Dockerren bertsioak; **Kopiatu** botoiak datu sentikorrik gabeko txosten bat sortzen du.
- **Desinstalazioa eta osagaiak** (29. atala), **Laguntza eta dokumentazioa** (eskuliburu hau) eta **Lege-oharrak** (34. atala).
- **Berrezarri hobespenak**, berretsi ondoren. Ez du Dockerrekiko konexioa aldatzen.

Dena berehala gordetzen da `~/.config/container-manage-center/config.toml` fitxategian.

Dockerrek berak sortutako mezuak (adibidez, Docker Engineren errore bat) eta programen irteera iristen diren bezala erakusten dira, itzuli gabe. Aplikazioaren motorrak jarduera-panelean idazten dituen laburpen teknikoak eta datu zehatzak dituzten errore-mezuak gaztelaniaz sortzen dira hizkuntza guztietan.

## 28. Windows {#windows}

Windowserako bertsioa **probetan** dago: instalatzailea sortzen da, baina oraindik ez da Windows erreal batean balioztatu. Zure erabiltzailearentzako `.exe` instalatzaile bat da, administratzaile-baimenik gabe, eta ez dago sinatuta; beraz, Windowsek SmartScreen ohar bat erakus dezake irekitzean.

Windows-en, Container Manage Center Linux-en bezala dabil, desberdintasun hauekin:

- **Docker lokala**: **Docker Desktop**-ekin konektatzen da haren izendun kanalizazioaren bidez (`\\.\pipe\docker_engine`). Docker Desktop martxan egon behar da, eta zure erabiltzailea `docker-users` talde lokalean (Docker Desktop-en instalatzaileak gehitzen du).
- **Urruneko zerbitzariak SSH bidez**: Windows-en OpenSSH bezeroa erabiltzen dute (`ssh.exe`, Windows 10 eta 11-n dator). Konexio multiplexatuak onartzen ez dituenez, eragiketa bakoitzak bere konexioa irekitzen du: lehen aldian zerbitzariaren identitatea egiaztatzen da Linux-en bezala, eta gordetako pasahitza edo pasaesaldia berez erabiltzen da. **Tarteko zerbitzariak** (ProxyJump) oraindik ez daude erabilgarri, eta **zerbitzariaren terminalean** leihoaren tamaina-aldaketa ez da zerbitzarira iristen.
- **Gogoratutako pasahitzak**: Windows-en **Kredentzial-kudeatzailean** (Windows-en kredentzialak), `container-manage-center/…` izenarekin.
- **Programatutako kopiak**: **Ataza-programatzailearekin**, «Container Manage Center» karpetan, zure erabiltzailearekin eta administratzaile-pribilegiorik gabe. Saioa hasita dagoenean exekutatzen dira; ordu horretan ordenagailua itzalita bazegoen, kopia ahal bezain laster egiten da.
- **Jakinarazpenak**: sistemarenak (jakinarazpen-zentroa).
- **Datuak**: konfigurazioa `%APPDATA%\container-manage-center` karpetan eta historia `%LOCALAPPDATA%\container-manage-center` karpetan.
- **Desinstalatu**: Windows-en Ezarpenak › Aplikazioak › Instalatutako aplikazioak ataletik. Zure datuak eta Docker Desktop ez dira ukitzen. «Desinstalazioa eta osagaiak» atala Debian eta Ubuntu-n bakarrik dago.

## 29. Desinstalazioa eta osagaiak {#uninstall}

**Konfigurazioa › Desinstalazioa eta osagaiak** atalak aplikazioak zer instalatu zuen erakusten du, eta terminala erabili gabe kentzeko aukera ematen du. **Aztertu** botoiak dpkg eta APT kontsultatzen ditu, ezer aldatu gabe:

- **Aplikazioaren paketea** (`container-manage-center`), haren tokiarekin.
- **Ken daitezkeen mendekotasunak**: APTk automatikoki **aplikazioarekin batera** instalatu zituenak (haren historiaren arabera) eta beste paketerik behar ez dituenak soilik.
- **Mantentzen direnak**, arrazoiarekin: mendekotasun partekatuak (erabiltzen dituzten paketeekin), eskuz instalatuak, lehendik zeudenak, jatorri ziurgabekoak (APTren historiak aplikazioa nola instalatu zen erakusten ez badu) eta sistemaren osagaiak.
- **Kanpoko tresnak**: Docker Engine, Docker CLI eta Docker Compose. **Beti mantentzen dira.**
- **Ezabatzen ez diren datuak**: zure konfigurazioa, zure historia eta leihoaren datuak. Dockerren ere ez da ezer ukitzen: irudiak, edukiontziak, sareak, bolumenak eta Compose proiektuak.
- Desinstalazioaren **APT simulazio** osoa.

**Container Manage Center soilik** edo **ken daitezkeen mendekotasunekin** aukera dezakezu, edo paketeak banan-banan markatu; laburpenak zenbat toki berreskuratzen den adierazten du. **Berrikusi eta desinstalatu…** botoiak hautapena berriro egiaztatzen du eta komandoa zehatz-mehatz simulatzen du: APTk aukeratu ez duzun zerbait kenduko balu, ezin da jarraitu. Elkarrizketa-koadroak exekutatuko den komandoa erakusten du:

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove container-manage-center …
```

Berrestean, sistemak zure administratzaile-pasahitza eskatzen du (pkexec) **komando horretarako bakarrik**; aplikazioak pribilegiorik gabe jarraitzen du. Autentifikazioa bertan behera uzten baduzu, ez da ezer kentzen. Ez da `purge` erabiltzen. Dena jarduera-panelean geratzen da. Aplikazioa bera kentzen bada, itxi amaitzean.

`pkexec` instalatuta ez badago, aplikazioak ezin du baimenik eskatu: elkarrizketa-koadroak `sudo` duen komando baliokidea erakusten du, zuk terminal batean exekutatzeko.

## 30. Arazoen konponbidea {#troubleshooting}

| Erakutsitako zergatia | Zer egin |
|---|---|
| Badirudi Docker ez dagoela instalatuta | `sudo apt install docker.io` edo Dockerren gida ofiziala |
| Ez da Docker Engine aurkitzen / geldituta dago | `sudo systemctl enable --now docker` (rootless: `systemctl --user enable --now docker`) |
| Ez dago socketa erabiltzeko baimenik | Docker rootless, edo `docker` taldea eta saioa berriro hasi |
| Dagoeneko taldekoa zara, baina ez dabil | Itxi saioa eta sartu berriro |
| Dockerren konexio bateraezina | `tcp://` edo `ssh://` tokiko konexioan: erabili tokiko socket bat (`docker context use default`); urruneko zerbitzari baterako, gehitu **Zerbitzariak** atalean |
| Dockerrek ez du garaiz erantzuten | `sudo systemctl restart docker` |
| Dockerren bertsioa zaharregia da | Eguneratu Docker 20.10 bertsiora edo berriago batera |

- **Docker Compose ez da agertzen**: instalatu (Debian 13: `sudo apt install docker-compose`; Ubuntu: `sudo apt install docker-compose-v2`; Dockerren biltegia: `docker-compose-plugin`) eta sakatu **Egiaztatu** Konfigurazioan.
- **Deskarga edo bilaketa batek huts egiten du**: Docker Enginek Interneterako sarbidea behar du; errore-mezua Dockerrena da.
- **Terminal bat ez da irekitzen**: edukiontziak exekutatzen egon behar du eta shell bat izan (`sh` edo `bash`).
- **Ez da uzten edukiontzi bat birsortzen**: arrazoia berrikuspenean agertzen da (Compose edukiontzia, `--rm` aukerarekin sortua, pausatuta, jada existitzen ez den irudia…).
- **Desinstalazioak ez du mendekotasun bat eskaintzen**: beste programa batek erabiltzen du, lehendik instalatuta zegoen edo ezin da ziur jakin; arrazoia haren ondoan agertzen da.
- **Arazo baten berri emateko**: Konfigurazioa › Sistema › **Kopiatu** botoiak datu sentikorrik gabeko txosten bat sortzen du.

- **Urruneko zerbitzari bat ez da konektatzen**: **Probatu konexioa** botoiak zazpi urratsetatik zeinek huts egiten duen adierazten du (7. atala).
- **Komando masibo batek huts egiten du edukiontzi batean**: edukiontziak exekutatzen egon behar du eta adierazitako programa edo interpretatzailea izan behar ditu; Dockerren errorea bere emaitzan agertzen da.

## 31. Segurtasuna eta jardunbide egokiak {#security}

- Aplikazioa **ez da root gisa exekutatzen**, ez du inoiz sudo eskatzen funtzionatzeko, eta ez ditu socketaren baimenak eta taldeak aldatzen.
- Kanpoko komandoak **shellik gabe** exekutatzen dira, argumentuak bereizita: idazten duzuna ez da inoiz komando gisa interpretatzen.
- **Ez du kredentzialik gordetzen.** Erregistro batenak deskarga baterako bakarrik erabiltzen dira eta ez dira jarduera-panelean agertzen. «Dockerrenak» aukerarekin, `docker login`-enak erabiltzen dira.
- `PASSWORD`, `TOKEN` edo `SECRET` bezalako aldagaien balioak pantailan eta panelean ezkutatzen dira (`********`), nahiz eta Dockerrek osorik jasotzen dituen.
- Historia iraunkorrak ez ditu inoiz gordetzen eskaeren gorputzak, komandoak edo irteerak.
- Eragiketa suntsitzaileek berrespena eskatzen dute.
- Leihoak kanpoko konexioak blokeatzen ditu. Aplikazioaren konexio bakarra bertsioen egiaztapena da, desgaituta dagoena iturri bat adierazten ez duzun bitartean. Laguntzako GitHub botoiak helbide hori bakarrik ireki dezake, zure nabigatzailean.
- Dockerren socketa edukiontzi batean muntatzeak edo `docker` taldeko kide izateak ordenagailuaren kontrol osoa ematen du: aplikazioak seinalatu egiten du.

**Jardunbide egokiak**: erabili Docker rootless ahal duzunean; berrikusi **Auditoria** noizean behin; egin **babeskopiak** bolumenak ezabatu edo datu-baseak eguneratu aurretik; ez argitaratu atakak interfaze guztietan ordenagailu honetan bakarrik erabiltzen badituzu; ez gorde pasahitzik irudietan ezta komando ohikoetan ere. Kudeaketa masiboan, ezabatzeak eta arrisku administratiboko edo suntsitzaileko komandoek edukiontzi kopurua idazteko eskatzen dute.

## 32. Ohiko galderak {#faq}

**Aplikazioak `docker` komandoak exekutatzen ditu?** Ez: Docker Engineren APIa erabiltzen du. Jarduera-panelean ikusten duzun komandoa baliokidea da, erreproduzitu ahal izateko. Docker Compose, berriz, bere programa ofizialarekin exekutatzen da.

**Administratzailea izan behar dut?** Ez. Dockerren socketerako sarbidea bakarrik behar duzu. Paketeen desinstalazioak bakarrik eskatzen ditu baimenak, eta komando horretarako bakarrik.

**Internetik gabe dabil?** Bai. Dockerrek urruneko erregistroen aurka egiten dituen eragiketek bakarrik behar dute sarea.

**Zer gertatzen zaie nire edukiontziei aplikazioa ixten badut?** Funtzionatzen jarraitzen dute: Dockerrek kudeatzen ditu. Ixtean, irekitako terminalak amaitzen dira, eta erregistroak eta estatistikak jarraitzeari uzten zaio.

**Datuak gal ditzaket edukiontzi bat editatzean?** Ez da bolumenik, karpetarik edo irudirik ezabatzen, eta jatorrizkoa kopia gisa gordetzen da. Bolumenetatik kanpo idatzitako fitxategiak bakarrik geratzen dira kopian, eta berrikuspenak aldez aurretik abisatzen dizu.

**Nire datuak galtzen dira desinstalatzean?** Ez. Zure konfigurazioa, zure historia eta Dockerreko guztia mantentzen dira.

**Non gordetzen dira konfigurazioa eta historia?** `~/.config/container-manage-center/` eta `~/.local/state/container-manage-center/` karpetetan.

**Nola aldatzen dut hizkuntza?** Konfigurazioa › Itxura › Hizkuntza atalean.

**Programatutako babeskopiek funtzionatzen jarraitzen dute aplikazioa itxita dagoenean?** Bai: sistemak exekutatzen ditu (systemd edo Windowseko Ataza-programatzailea), zure saioa hasita dagoenean.

## 33. Glosarioa {#glossary}

| Terminoa | Esanahia |
|---|---|
| Edukiontzia | Ordenagailuaren gainerako ataletatik isolatutako programa bat, exekutatzen ari dena, irudi batetik sortua |
| Irudia | Irakurtzeko soilik den txantiloia, edukiontziak sortzeko fitxategi-sistemarekin eta konfigurazioarekin |
| Irudiaren etiketa (*tag*) | Irudi baten bertsio baten izena, `nginx:1.27` bezala |
| Edukiontziaren etiketa (*label*) | Edukiontzi batean gordetako `gakoa=balioa` datu bat, `cmc.description` bezala |
| Erregistroa | Irudiak deskargatzen diren zerbitzaria, Docker Hub bezala |
| Bolumena | Dockerrek kudeatzen duen biltegiratzea, edukiontzia ezabatu arren gordetzen dena |
| Lotutako direktorioa (*bind mount*) | Edukiontzi baten barruan muntatutako ordenagailuko karpeta edo fitxategia |
| Sarea | Edukiontziak elkarren artean eta kanpoarekin komunikatzen dituen sare birtuala |
| Argitaratutako ataka | Edukiontziaren ataka batera daraman ordenagailuaren ataka |
| Berrabiarazte-politika | Edukiontzi bat amaitzean edo ordenagailua abiaraztean Dockerrek egiten duena |
| Docker Engine | Edukiontziak exekutatzen dituen zerbitzua; CMC berarekin komunikatzen da |
| Dockerren APIa | Docker Engine kontrolatzeko interfazea; Dockerren CLIak eta CMCk erabiltzen dute |
| Unix socketa | Ordenagailu berean Dockerrekin komunikatzeko fitxategi berezia (`/var/run/docker.sock`) |
| Rootless | Erabiltzaile arrunt batek exekutatutako Docker, administratzaile-pribilegiorik gabe |
| Dockerren testuingurua | CLIa zein Dockerrekin konektatzen den adierazten duen izendun konfigurazioa (`docker context`) |
| `DOCKER_HOST` | Zein Dockerrekin konektatu adierazten duen ingurune-aldagaia |
| Docker Compose | Edukiontzi anitzeko aplikazioak YAML fitxategi batekin definitu eta exekutatzeko tresna |
| Zerbitzua eta profila (Compose) | Zerbitzua Compose proiektu baten zati bakoitza da; profil batek aukerako zerbitzuak taldekatzen ditu |
| Docker Exec | Dagoeneko martxan dagoen edukiontzi baten barruan programa bat exekutatzea |
| `stdout` eta `stderr` | Programa baten irteera arrunta eta erroreen irteera |
| Hatz-marka (*fingerprint*) | SSH zerbitzari baten gakoaren laburpena (`SHA256:…`), haren identitatea egiaztatzeko |
| Biltegi segurua (giltzatakoa) | Pasahitzak zifratuta gordetzen diren sistemaren lekua (GNOME Keyring, KWallet, Kredentzial-kudeatzailea) |
| Docker Desktop | Windows eta macOSerako Docker aplikazioa |
| Izendun kanalizazioa | Docker Desktoprekin konektatzeko Windowsen mekanismoa (`\\.\pipe\docker_engine`) |

## 34. Lege-informazioa eta lizentzia {#legal}

**Izena:** Container Manage Center (laburtuta **CMC**).

**Lizentzia:** GNU General Public License, 3. bertsioa edo ondorengoa (SPDX identifikatzailea: `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

Programa hau software librea da: birbana eta/edo alda dezakezu Free Software Foundation-ek argitaratutako GNU Lizentzia Publiko Orokorraren baldintzen arabera, Lizentziaren 3. bertsioa edo (zure aukeran) ondorengo edozein bertsio. Erabilgarria izango delakoan banatzen da, baina INOLAKO BERMERIK GABE; MERKATURATZEKO edo HELBURU JAKIN BATERAKO EGOKITASUNAREN berme inplizitua ere gabe. Xehetasun gehiagorako, ikus GNU Lizentzia Publiko Orokorra.

Lizentziaren testu osoa: https://www.gnu.org/licenses/gpl-3.0.html (baita `/usr/share/doc/container-manage-center/LICENSE` fitxategian ere).

**Hirugarrenen osagaiak.** Aplikazioak hirugarrenen liburutegiak eta letra-tipoak dakartza, eta horiek beren lizentziak (MIT, Apache-2.0, MPL-2.0, OFL-1.1 eta beste batzuk) eta egileen egile-eskubideen oharrak gordetzen dituzte; ez dira GPL bihurtzen, ezta Sergio Alfaro Loriterenak ere. Testu osoak `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md` fitxategian daude, eta paketearen egile-eskubideen oharra `/usr/share/doc/container-manage-center/copyright` fitxategian.

**Iturburu-kodea.** Dagokion iturburu-kodea paketearen bertsio bakoitzarekin batera argitaratzen da (`container-manage-center-X.Y.Z-src.tar.gz`).

**Markak.** Container Manage Center proiektu independentea da: ez dago Docker, Inc. enpresari lotuta, eta enpresa horrek ez du babesten. «Docker» Docker, Inc.-en marka da, eta hemen aplikazioak erabiltzen duen teknologia izendatzeko soilik erabiltzen da (Docker Engine, haren APIa, Docker Compose eta Docker CLIa).

**Windows.** Windowseko instalatzaileak Microsoften WebView2 kargatzailea dakar, GPL v3-ren 7. artikuluko baimen gehigarri batek estalia.

Datu berak **Konfigurazioa › Lege-oharrak** atalean daude.
