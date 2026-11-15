## 1. Che cos’è Container Manage Center {#about}

Container Manage Center è un’applicazione desktop per amministrare **Docker Engine** su Linux senza dipendere dal terminale. Tutto ciò che mostra sono dati reali richiesti a Docker in quel momento, e ogni azione è un’operazione reale: nulla viene simulato.

- Comunica con Docker Engine tramite il suo **socket Unix**, con l’API ufficiale.
- Esegue **Docker Compose** con il suo programma ufficiale, con argomenti separati e senza shell.
- Ogni operazione compare nel **pannello attività** con il comando `docker` equivalente, così sai esattamente che cosa è stato fatto.

La finestra ha una **barra laterale** con le sezioni (si comprime con Ctrl+B), l’**area di lavoro**, una **barra superiore** con l’indicatore di connessione e il pulsante **Attività**, e il **pannello attività** a destra (Ctrl+Maiusc+A).

## 2. Requisiti di sistema e dipendenze {#requirements}

- **Sistema**: Debian 12 o successivo, oppure Ubuntu 22.04 o successivo, a 64 bit (x86_64).
- **Docker Engine 20.10 o successivo** (API 1.41+), in modalità normale o rootless.
- **Docker Compose v2 o successivo**, solo per la sezione Docker Compose (come plugin `docker compose` o binario `docker-compose` v2). Compose v1 non è supportato.
- **Dipendenze di esecuzione**: WebKitGTK 4.1, GTK 3, glibc e librerie correlate. APT le installa con il pacchetto.

L’applicazione non ha bisogno di Internet. Solo le operazioni che Docker Engine esegue verso registri remoti (scaricare immagini, cercare su Docker Hub, compilare con immagini di base non presenti sul computer) richiedono che Docker abbia accesso alla rete.

## 3. Installazione e primo avvio {#install}

Installa il pacchetto con APT, che risolve le dipendenze:

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

L’applicazione compare nel menu come **Container Manage Center**. Si esegue con il tuo utente normale: **non avviarla con sudo** (si rifiuta di partire come root).

All’apertura cerca Docker Engine automaticamente. Se lo trova, la **Dashboard** ne mostra lo stato; altrimenti mostra la diagnosi con la causa e i passaggi per risolverla.

Per aggiornare, installa il nuovo pacchetto allo stesso modo: la configurazione e la cronologia vengono conservate.

## 4. Configurazione iniziale {#first-setup}

Per iniziare non serve configurare nulla: la connessione a Docker è automatica. In **Impostazioni** puoi scegliere:

- Il **tema** (Sistema, Chiaro o Scuro) e la **lingua**. Il colore principale è il viola.
- Se il pannello attività si apre da solo quando un’operazione non riesce e quante operazioni conserva.
- Se la cronologia delle operazioni e degli eventi viene conservata tra una sessione e l’altra.
- Un socket di Docker specifico, se non vuoi il rilevamento automatico.

Tutto viene salvato subito in `~/.config/container-manage-center/config.toml`.

## 5. Connessione a Docker Engine {#connection}

L’**indicatore di connessione** nella barra superiore mostra lo stato:

| Aspetto | Significato |
|---|---|
| Punto verde e `Docker 29.x` | Connesso. Passando il mouse si vede il socket |
| `Connessione a Docker…` | Rilevamento o connessione in corso |
| Rosso, `Nessuna connessione a Docker` | Non connesso. Fai clic per vedere la diagnosi |

In modalità **automatica**, l’applicazione prova in quest’ordine: la variabile `DOCKER_HOST` (solo `unix://`), il contesto attivo di Docker, il tuo socket rootless e il socket di sistema `/var/run/docker.sock`. Controlla la connessione ogni 10 secondi e si riconnette da sola quando Docker torna disponibile.

L’applicazione non chiede mai sudo, non modifica permessi né gruppi e non esegue al posto tuo le istruzioni della diagnosi. Il tuo utente deve poter aprire il socket: con Docker rootless oppure appartenendo al gruppo `docker` (il che in pratica equivale ad avere privilegi di amministratore).

## 6. Docker rootless e connessioni tramite socket {#rootless}

Con **Docker rootless** il demone viene eseguito con il tuo utente e il socket si trova in `$XDG_RUNTIME_DIR/docker.sock` (di solito `/run/user/1000/docker.sock`). L’applicazione lo rileva da sola, anche tramite il contesto `rootless`, e lo indica con l’etichetta **rootless**. È l’opzione consigliata per la sicurezza.

In **Impostazioni › Connessione a Docker** puoi:

- Scegliere **Socket personalizzato** e scrivere un percorso assoluto.
- **Prova connessione**: verifica il percorso senza salvarlo.
- **Salva e connetti**: applica la configurazione.
- **Riconnetti**: ripete il rilevamento.

Le connessioni `tcp://` non sono supportate. I server remoti si amministrano tramite SSH da **Server** (sezione successiva).

## 7. Server remoti tramite SSH {#remote-servers}

Da **Server** puoi amministrare il Docker di altri computer Linux a cui hai accesso tramite **SSH**, insieme a quello di questo computer. La connessione usa il client SSH del sistema (`ssh`) ed esegue `docker system dial-stdio` sul server: Docker non viene **mai** esposto sulla rete (niente TCP 2375) e non si apre nessuna porta.

**Requisiti del server**: server SSH in esecuzione, Docker Engine 20.10 o successivo (API 1.41) e un utente che possa usare Docker. Per Compose, `docker compose` o `docker-compose` sul server. L'applicazione **non installa Docker, non modifica permessi, non tocca la configurazione SSH e non apre porte**: se manca qualcosa, ti dice cosa e come risolverlo.

**Aggiungi server** (procedura guidata): dati generali (nome, host, utente, porta 22), autenticazione, Docker (predefinito, un socket specifico o un contesto) e **Prova connessione**, che verifica sette passaggi: DNS, connessione SSH, identità del server, autenticazione, Docker disponibile, permessi e compatibilità dell'API.

- **Autenticazione**: agente SSH (consigliato), chiave privata, chiave con passphrase o password (sconsigliata). La chiave privata **non viene mai copiata**: si salva solo il percorso.
- **Credenziali**: la passphrase o la password vengono chieste alla connessione. Se scegli di **ricordarla**, viene salvata nell'**archivio sicuro del sistema** (GNOME Keyring, KWallet…); se non c'è, viene chiesta ogni volta e non viene **mai** salvata in chiaro. Puoi eliminarla modificando il server.
- **Identità del server**: la prima volta viene mostrata l'**impronta** (`SHA256:…`) perché tu la verifichi e la confermi. Se cambia, la connessione si interrompe con l'avviso di un possibile **attacco man-in-the-middle**; viene sostituita solo dopo averla verificata. Se la chiave precedente è nel tuo `~/.ssh/known_hosts`, l'applicazione non modifica quel file e ti fornisce il comando per farlo tu.

**Più server**: il **selettore** nella barra superiore indica su quale server stai lavorando, con il suo stato, e distingue quelli remoti. Cambiando server, tutta la schermata passa a quel server. **Panoramica** li mostra tutti insieme (stato, versione, container e risorse). Ogni server è indipendente: uno non raggiungibile non influisce sugli altri, e ogni operazione va al server in cui l'hai avviata. Le conferme indicano **server e indirizzo**, e il pannello attività mostra il server di ogni operazione e permette di filtrare per server.

**Su un server remoto** container, immagini, reti, volumi, terminali e Compose funzionano come in locale. I percorsi (file Compose, cartelle dei bind mount) sono **del server**: si scrivono a mano e vengono verificati lì; prima di scrivere un file sul server si conferma la destinazione. In **Terminali** puoi aprire anche la **shell SSH del server**, un tipo di sessione a parte.

**Disconnetti** chiude la sessione SSH e i suoi terminali; **elimina profilo** cancella solo i dati dell'applicazione (profilo, credenziali e identità salvata). Nessuna delle due tocca container, immagini, reti, volumi o dati del server.

Appartenere al gruppo `docker` del server equivale ad avere accesso **root** su di esso: decidilo con il suo amministratore; Docker rootless è l'alternativa più sicura.

## 8. Dashboard e diagnosi {#dashboard}

Con la connessione attiva, la **Dashboard** mostra dati ottenuti in quel momento: versioni di Docker Engine e dell’API, socket e origine della connessione, container (in esecuzione, in pausa, arrestati), immagini, dati del computer (sistema, kernel, CPU, memoria), archiviazione ed esecuzione (driver, directory dei dati, cgroups, log) e gli avvisi del demone. **Aggiorna** li richiede di nuovo.

Senza connessione mostra la **diagnosi**: la causa precisa, i passaggi per risolverla (con i comandi da copiare in un terminale), i socket controllati con il loro esito e ciò che è stato rilevato come installato. I pulsanti **Riconnetti** e **Configura connessione** sono nell’intestazione.

## 9. Gestione dei container {#containers}

La sezione **Container** elenca tutti i container, come `docker ps -a`: stato, nome e ID breve, progetto Compose, immagine, stato di Docker, porte pubblicate ed età. Puoi filtrare per **Tutti / In esecuzione / Arrestati** e cercare per nome, immagine, ID o progetto. Il menu **⋯** di ogni riga (o il clic destro) offre **Modifica configurazione** e **Crea immagine da questo container**.

| Stato | Azioni disponibili |
|---|---|
| In esecuzione | Arresta, Riavvia, Metti in pausa, Elimina |
| In pausa | Riprendi, Arresta, Elimina |
| Creato o arrestato | Avvia, Elimina |

Arresta e riavvia attendono 10 secondi che il processo termini prima di forzarlo, come Docker. Dopo ogni azione viene interrogato lo stato reale del container. Se Docker rifiuta un’azione, il suo messaggio compare sopra la tabella e nel pannello attività. Gli elenchi si aggiornano da soli quando qualcosa cambia in Docker, anche dal terminale.

## 10. Creazione, duplicazione, ispezione ed eliminazione dei container {#container-lifecycle}

**Crea container** apre una procedura guidata con sei schede (solo l’immagine è obbligatoria): Generale, Ambiente, Rete, Archiviazione, Risorse e Avanzate.

- **Comando** ed **Entrypoint** si scrivono come in un terminale e vengono divisi in argomenti, mostrati sotto. Non viene eseguita alcuna shell.
- Il pannello **Equivalente** mostra il `docker run` equivalente e il corpo esatto inviato all’API.
- **Revisione** segnala gli errori (impediscono la creazione) e gli avvisi (porte privilegiate, cartelle delicate montate in scrittura, modalità privilegiata…).
- **Crea** crea il container senza avviarlo; **Crea e avvia** lo avvia anche.

Facendo clic sul nome di un container si apre il suo **dettaglio**: Riepilogo, Log, Terminale, Statistiche e Ispezione JSON (la risposta completa di `docker inspect`, con il pulsante Copia).

**Duplica** apre la procedura guidata con la configurazione del container e il nome `<nome>-copia`, e avvisa delle impostazioni che non vengono riportate.

**Elimina** chiede conferma. Un container in esecuzione o in pausa viene eliminato solo se selezioni **Arrestalo forzatamente ed eliminalo**. Facoltativamente vengono eliminati anche i suoi volumi anonimi; i volumi con nome e le cartelle collegate non vengono toccati. Non si può annullare.

## 11. Modificare la configurazione di un container {#container-edit}

**Modifica configurazione** si trova nel menu **⋯** di ogni container (anche con il clic destro sulla riga) e nell’intestazione del dettaglio. La configurazione è organizzata in schede: **Generale**, **Rete**, **Archiviazione**, **Variabili**, **Porte**, **Risorse** e **Avanzate**. Ogni modifica indica come viene applicata:

| Modifica | Come si applica |
|---|---|
| Nome | Diretta (`docker rename`) |
| Criterio di riavvio, CPU, memoria e memoria riservata | Diretta (`docker update`), senza riavviare il container |
| Collegare o scollegare reti | Diretta e immediata (`docker network connect` / `disconnect`) |
| Variabili d’ambiente, porte, volumi e cartelle, descrizione | Richiede di **ricreare** il container |
| Rimuovere un limite di CPU o di memoria | Richiede la ricreazione (`docker update` non sa rimuoverlo) |
| Immagine, comando, utente, directory di lavoro e le altre opzioni di **Avanzate** | Sola lettura: Docker non permette di modificarle. Usa **Duplica** |

- **Rete**: mostra le reti, l’IP, il MAC e gli alias. Docker non permette di modificare gli alias né l’IP di una connessione esistente: bisogna scollegare e ricollegare. Se è l’unica rete del container, viene mostrato un avviso prima di scollegarla.
- **Variabili**: i valori che sembrano segreti vengono nascosti sullo schermo e non vengono mai salvati nel pannello attività.
- **Descrizione**: viene salvata nell’etichetta `cmc.description` del container.

**Rivedi modifiche** confronta la configurazione attuale con quella nuova: cosa cambia (prima e dopo, elemento per elemento), cosa resta invariato e cosa richiede la ricreazione. Non viene applicato nulla finché non confermi.

**Ricreazione sicura.** Quando una modifica richiede la ricreazione, la procedura guidata:

1. Verifica l’immagine, i conflitti di nome e di porte con altri container e i motivi che impediscono una ricreazione sicura: container di Compose o Swarm, creati con `--rm`, in pausa o in riavvio. In questi casi non permette di continuare.
2. Mostra i rischi da accettare uno per uno: file scritti fuori dai volumi (non passano al nuovo container), mount che non saranno più montati, container che usano la rete di questo…
3. Alla conferma: ferma il container se è in esecuzione, lo rinomina `<nome>-cmc-backup-<data>` (copia di sicurezza), crea quello nuovo con **tutta** la configurazione originale più le tue modifiche, collega le sue reti e, se era in esecuzione, lo avvia e verifica che continui a funzionare per qualche secondo.
4. Se qualcosa non va, elimina il nuovo container (senza toccarne i volumi), restituisce il nome all’originale e lo riavvia.

**Non vengono mai eliminati volumi, cartelle o immagini.** I volumi anonimi vengono rimontati con il loro contenuto. La copia resta ferma in **Generale › Copie di sicurezza**, da dove puoi **Ripristinarla** (il container attuale viene messo da parte, non eliminato) o **Eliminarla** quando non ti serve più. Tutte le operazioni restano nel pannello attività, senza valori sensibili.

## 12. Creare un’immagine da un container {#container-commit}

**Crea immagine da questo container** (menu **⋯** dell’elenco e intestazione del dettaglio) equivale a `docker commit`: salva lo stato attuale del file system del container come una nuova immagine.

- Indica il **repository** e il **tag** (vuoto = `latest`); facoltativamente, **autore** e **messaggio**. Il nome completo viene mostrato e verificato prima della creazione.
- **Non include i volumi né le cartelle montate**: solo il livello scrivibile del container. La finestra elenca i mount esclusi.
- L’immagine **salva invece la configurazione del container**, comprese le sue variabili d’ambiente (anche quelle che contengono password o token): non pubblicarla in un registro se contiene segreti.
- Se il container è in esecuzione, per impostazione predefinita viene **messo in pausa** per qualche secondo durante la creazione dell’immagine, perché sia coerente; puoi disattivare l’opzione. Non viene mai fermato.
- Se esiste già un’immagine con quel nome, viene mostrato un avviso e bisogna accettarlo espressamente: la precedente non viene eliminata, resta solo senza tag.
- Il container originale non viene modificato. Alla fine, **Vedi l’immagine** apre il suo dettaglio in Immagini.

## 13. Log, statistiche e terminali {#logs-stats-terminal}

Nel dettaglio di un container:

- **Log**: output in diretta (stderr in rosso). Puoi scegliere quante righe precedenti caricare, filtrare, mostrare l’ora, seguire la fine, copiare ciò che è visibile e smettere di seguire. Vengono conservate le ultime 10.000 righe.
- **Statistiche**: CPU, memoria, rete, disco e processi ogni secondo, calcolati come `docker stats`.
- **Terminale**: **Apri terminale** avvia una shell con `docker exec -it` (Automatica usa `bash` se esiste, altrimenti `sh`; puoi anche sceglierne una). **Chiudi sessione** termina la shell e i processi che ha lasciato nel container.

Sono operazioni reali che compaiono nel pannello attività finché sono aperte e si chiudono quando esci dal dettaglio. Per lavorare con più terminali contemporaneamente, usa la sezione **Terminali**.

## 14. Gestione delle immagini {#images}

La sezione **Immagini** elenca le immagini locali, come `docker image ls`: tag, ID breve, dimensione, età e quali container le usano. Filtri **Tutte / In uso / Inutilizzate / Senza tag** e ricerca per tag o ID.

- **Dettaglio**: Riepilogo (con i valori sensibili delle variabili nascosti), Livelli (come `docker image history`), Container che la usano e Ispezione JSON.
- **Aggiungi tag** aggiunge un nome `repository:tag`.
- **Elimina**: se l’immagine ha più tag puoi rimuoverne solo uno; l’immagine intera richiede di forzare. Docker non elimina le immagini dei container in esecuzione.
- **Pulisci** elimina le immagini senza tag non usate da alcun container oppure, con una conferma aggiuntiva, tutte quelle non usate.

## 15. Download e compilazione delle immagini {#pull-build}

**Scarica immagine** chiede il riferimento (`nginx:1.27`, `utente/app`, `registro.esempio.com:5000/app:v2`; senza tag si usa `latest`). Credenziali:

| Opzione | Che cosa fa |
|---|---|
| Quelle di Docker | Usa quanto salvato con `docker login` (o il suo *credential helper*) |
| Nessuna | Download anonimo |
| Nome utente e password | Solo per questo download: non vengono salvati e non compaiono nel pannello attività |

Il pannello **Download** mostra l’avanzamento totale e per livello; nel frattempo puoi cambiare sezione e **Annullare**. **Cerca su Docker Hub** cerca come `docker search` (lo fa Docker Engine, che ha bisogno di Internet).

**Compila immagine** apre una procedura guidata: contesto (la cartella inviata, rispettando `.dockerignore`) e Dockerfile, nomi (`--tag`), argomenti (`--build-arg`), metadati (`--label`), fase finale, senza cache, cerca immagini di base più recenti e compilatore (**BuildKit** o **classico**). Si vedono i passaggi con il loro stato e l’output come `docker build --progress=plain`; si può annullare.

## 16. Reti Docker {#networks}

La sezione **Reti** elenca le reti, come `docker network ls`, con driver, sottoreti e container. Le reti **bridge**, **host** e **none** sono predefinite e non possono essere eliminate.

- **Crea rete**: nome, driver (`bridge`, `macvlan`, `ipvlan`, `overlay` o un altro), rete interna, indirizzi IPv4 (sottorete, gateway e intervallo) e IPv6, opzioni del driver e metadati. Si vedono il comando equivalente e la richiesta esatta, e gli errori vengono segnalati prima di inviare qualsiasi cosa a Docker.
- **Connetti** un container (in esecuzione o arrestato), con alias e IP fisso facoltativi. **Disconnetti** chiede conferma.
- **Elimina** e **Pulisci** avvisano se ci sono container arrestati che hanno la rete configurata e che dopo non potranno avviarsi.

## 17. Volumi {#volumes}

La sezione **Volumi** elenca i volumi, come `docker volume ls`, con il loro driver e i container che li montano. Quelli anonimi sono contrassegnati come tali. **Calcola dimensioni** chiede a Docker lo spazio di ciascuno (come `docker system df -v`); non viene fatto automaticamente perché può richiedere tempo.

- **Crea volume**: nome (facoltativo) e tipo: local, tmpfs, NFS, una cartella del computer o un altro driver.
- **Dettaglio**: directory, opzioni, metadati, dimensione, container con il loro percorso e la modalità, e ispezione JSON. **Crea container con questo volume** lo monta in `/data`.
- **Elimina**: Docker non elimina un volume montato da un container, anche se arrestato.
- **Pulisci**: elimina i volumi anonimi inutilizzati oppure, con conferma, tutti i volumi inutilizzati.

## 18. Directory collegate (Bind Mounts) {#binds}

Questa sezione mostra le cartelle e i file del tuo computer montati all’interno dei container, raggruppati per percorso: tipo, permessi e proprietario, se esistono, quali container li montano, dove e con quale accesso. Vengono segnalati quelli che richiedono attenzione: il **socket di Docker** (dà al container il controllo di Docker), la radice o le directory di sistema e i percorsi che non esistono più.

Docker non permette di aggiungere una cartella a un container esistente. **Collega una cartella…** apre il selettore di cartelle e poi la procedura guidata dei container con quella cartella montata in `/mnt/<nome>`.

## 19. Docker Compose {#compose}

La sezione **Docker Compose** richiede Compose v2 o successivo; se non è installato, spiega come installarlo. Tutte le operazioni eseguono il Docker Compose ufficiale e compaiono nel pannello attività con il comando esatto.

L’elenco mostra i progetti con container in Docker (anche quelli distribuiti dal terminale) e quelli che aggiungi, con il loro stato (in esecuzione, parziale, arrestato o non distribuito), cartella e servizi.

- **Apri progetto…**: scegli il suo `compose.yaml` (e i file aggiuntivi nell’ordine in cui vengono combinati).
- **Nuovo progetto**: scegli una cartella, scrivi il contenuto (partendo da un modello) e **Crea progetto**: viene convalidato con Docker Compose prima di creare `compose.yaml`.
- **Rimuovi dall’elenco** lo toglie solo dall’elenco; non tocca Docker né i file.

## 20. Gestione di progetti e profili Compose {#compose-projects}

Nel dettaglio di un progetto:

- **Servizi**: ogni servizio con la sua immagine, stato, container, porte e **profili**. Ogni riga permette di distribuire, avviare, arrestare, riavviare o vedere i log di quel servizio.
- **Distribuisci** (`docker compose up --detach`): tutti i servizi o quelli scelti, **profili** da attivare, compilare prima, criterio di download delle immagini, ricreare i container ed eliminare gli orfani. La finestra di dialogo mostra il comando esatto.
- **Avvia, Arresta, Riavvia**, **Scarica immagini** e **Compila**.
- **Elimina** (`docker compose down`): container e reti del progetto; facoltativamente i suoi volumi con nome (i loro dati vanno persi) e le sue immagini. Va confermato.
- **Output** dell’operazione in corso (si può **Interrompere**) e **Log** dei servizi in diretta.
- **File**: modifica i file; **Convalida** li verifica con Docker Compose senza salvarli, e **Salva** li convalida prima e non sovrascrive se un altro programma li ha modificati.
- **Configurazione**: la configurazione finale secondo Docker Compose. Può contenere segreti, quindi resta nascosta finché non la richiedi e non viene salvata nel pannello attività.

## 21. Terminali simultanei {#terminals}

La sezione **Terminali** permette di aprire e vedere contemporaneamente più terminali reali (`docker exec -it`) in container in esecuzione, nello stesso container o in altri.

- **Nuovo terminale**: scegli un container in esecuzione, la shell (Automatica, bash o sh) e, se vuoi, un nome per la sessione.
- Ogni pannello mostra il **container**, il **nome della sessione** e un **indicatore di connessione** (Connessione…, Connessa, Terminata o Errore), con i pulsanti per **rinominare**, **ingrandire** e **chiudere**. Anche il doppio clic sul nome permette di rinominarla.
- **Disposizione**: un terminale occupa tutto lo spazio; due vanno su due colonne; tre, uno a sinistra e due a destra; quattro, in una griglia 2×2; con più di quattro, in righe da due con scorrimento verticale.
- I **divisori** si trascinano con il mouse. Con il focus su un divisore, le frecce lo spostano, Inizio e Fine lo portano agli estremi e il doppio clic divide lo spazio in parti uguali.
- Ogni sessione è indipendente: la propria shell, input e output, dimensione e stato. Ciò che fai in una non influisce sulle altre.
- Le sessioni **restano aperte quando cambi sezione**. **Chiudi tutte** chiede conferma. Sono consentite fino a 12 sessioni contemporanee.
- **Chiudere** una sessione termina la sua shell e i processi che ha lasciato in background nel container. Se il container viene **arrestato** o **eliminato**, il pannello lo indica; una sessione terminata si può **riaprire** se il container esiste ancora.
- **Chiudere l’applicazione** termina tutte le sessioni.

Il pannello attività registra l’apertura, la chiusura e gli errori di ogni sessione, ma non ciò che viene digitato né l’output del terminale.

## 22. Pannello attività e registro delle operazioni {#activity}

Ogni operazione compare nel **pannello attività**: ora, contrassegno **API** (richiesta a Docker Engine tramite il socket), **CLI** (processo, come `docker compose` o `apt-get`) o **APP** (operazione dell’applicazione stessa, come un cambio di lingua, o il riepilogo di una ricreazione i cui passaggi compaiono a parte), tipo, risorsa, durata e stato. La riga principale è il comando equivalente, che puoi copiare. Espandendo una voce vedi l’operazione realmente eseguita, il risultato, l’errore e il suo codice HTTP in caso di errore, e l’output dei processi.

La sezione **Eventi e registri** mostra gli eventi di Docker in diretta, quelli salvati, una richiesta a Docker per un periodo (`docker events --since … --until …`; Docker conserva solo gli ultimi 256) e la **cronologia delle operazioni** di questa e di altre sessioni.

La cronologia salva che cosa è stato fatto, quando e con quale esito, ma mai il corpo delle richieste, il comando né l’output, perché possono contenere password. Si trova in `~/.local/state/container-manage-center/`, leggibile solo dal tuo utente.

## 23. Impostazioni, temi e lingue {#settings}

- **Aspetto**: tema (Sistema, Chiaro, Scuro) e lingua, in questo ordine: **English**, **Español** (predefinita), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** e **Português**. Ogni lingua compare con il proprio nome. La lingua viene salvata, mantenuta alla successiva apertura dell’applicazione e le sue modifiche restano nel pannello attività. Il colore principale è il **viola** (dalla versione 1.0.0 non se ne può scegliere un altro; se avevi il verde acqua, passa al viola senza perdere il resto delle impostazioni).
- **Pannello attività**: aprirlo in caso di errore, operazioni conservate (50–10.000) e **Conserva la cronologia tra le sessioni**.
- **Connessione a Docker** e verifica di **Docker Compose**.
- **Aggiornamenti**: dove vengono pubblicate le versioni e **Controlla ora**. L’applicazione non scarica né installa mai nulla.
- **Sistema**: versioni dell’applicazione, del pacchetto, di WebKitGTK e glibc, del sistema e di Docker; **Copia** genera un rapporto senza dati sensibili.
- **Disinstallazione e componenti** e **Guida e documentazione**.
- **Ripristina le preferenze**, dopo la conferma. Non modifica la connessione a Docker.

| Scorciatoia | Azione |
|---|---|
| Ctrl+B | Comprimi o espandi la barra laterale |
| Ctrl+Maiusc+A | Apri o chiudi il pannello attività |
| Esc | Chiudi una finestra di dialogo |

I messaggi generati da Docker stesso (ad esempio un errore di Docker Engine) e l’output dei programmi vengono mostrati così come arrivano, senza traduzione. I riepiloghi tecnici che il motore dell’applicazione scrive nel pannello attività (ad esempio, come è terminato un terminale) e i messaggi di errore con dati specifici vengono generati in spagnolo in tutte le lingue.

## 24. Disinstallazione e gestione delle dipendenze {#uninstall}

**Impostazioni › Disinstallazione e componenti** mostra che cosa ha installato l’applicazione e permette di rimuoverlo senza usare il terminale. **Analizza** interroga dpkg e APT senza modificare nulla:

- **Pacchetto dell’applicazione** (`container-manage-center`), con il suo spazio.
- **Dipendenze rimovibili**: solo quelle che APT ha installato automaticamente **insieme all’applicazione** (secondo la sua cronologia) e di cui nessun altro pacchetto ha bisogno.
- **Conservate**, con il motivo: dipendenze condivise (con i pacchetti che le usano), installate esplicitamente, già presenti prima, di origine incerta (se la cronologia di APT non mostra come è stata installata l’applicazione) e componenti di sistema.
- **Strumenti esterni**: Docker Engine, Docker CLI e Docker Compose. **Conservati sempre.**
- **Dati che non vengono eliminati**: la tua configurazione, la tua cronologia e i dati della finestra. Non viene toccato nulla di Docker: immagini, container, reti, volumi e progetti Compose.
- La **simulazione di APT** completa della disinstallazione.

Puoi scegliere **Solo Container Manage Center** o **con le dipendenze rimovibili**, oppure selezionare i pacchetti uno per uno; il riepilogo indica quanto spazio si recupera. **Verifica e disinstalla…** controlla di nuovo la selezione e simula esattamente il comando: se APT dovesse rimuovere qualcosa che non hai scelto, non puoi continuare. La finestra di dialogo mostra il comando che verrà eseguito:

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove container-manage-center …
```

Alla conferma, il sistema chiede la password di amministratore (pkexec) **solo per quel comando**; l’applicazione continua a funzionare senza privilegi. Se annulli l’autenticazione non viene rimosso nulla. `purge` non viene usato. Tutto viene registrato nel pannello attività. Se viene rimossa l’applicazione stessa, chiudila al termine.

Se `pkexec` non è installato, l’applicazione non può chiedere i permessi: la finestra di dialogo mostra il comando equivalente con `sudo` perché tu lo esegua in un terminale.

## 25. Sicurezza e gestione delle credenziali {#security}

- L’applicazione **non viene eseguita come root**, non chiede mai sudo per funzionare e non modifica i permessi del socket né i gruppi.
- I comandi esterni vengono eseguiti **senza shell** e con argomenti separati: ciò che scrivi non viene mai interpretato come comandi.
- **Non salva credenziali.** Quelle di un registro servono solo per un download e non compaiono nel pannello attività. Con «Quelle di Docker» si usano quelle di `docker login`.
- I valori di variabili come `PASSWORD`, `TOKEN` o `SECRET` sono nascosti sullo schermo e nel pannello (`********`), anche se Docker li riceve per intero.
- La cronologia persistente non salva mai il corpo delle richieste, i comandi né l’output.
- Le operazioni distruttive chiedono conferma.
- La finestra blocca le connessioni esterne. L’unica connessione propria dell’applicazione è il controllo delle versioni, disattivato finché non indichi una fonte. Il pulsante GitHub della Guida può aprire solo quell’indirizzo, nel tuo browser.
- Montare il socket di Docker in un container o appartenere al gruppo `docker` dà il controllo totale del computer: l’applicazione lo segnala.

## 26. Risoluzione dei problemi frequenti {#troubleshooting}

| Causa mostrata | Che cosa fare |
|---|---|
| Docker non sembra installato | `sudo apt install docker.io` o la guida ufficiale di Docker |
| Docker Engine non trovato / arrestato | `sudo systemctl enable --now docker` (rootless: `systemctl --user enable --now docker`) |
| Nessun permesso per usare il socket | Docker rootless, oppure il gruppo `docker` e un nuovo accesso |
| Appartieni già al gruppo, ma non funziona | Esci dalla sessione e accedi di nuovo |
| Connessione di Docker non supportata | `tcp://` o `ssh://` nella connessione locale: usa un socket locale (`docker context use default`); per un server remoto, aggiungilo in **Server** |
| Docker non risponde in tempo | `sudo systemctl restart docker` |
| Versione di Docker troppo vecchia | Aggiorna a Docker 20.10 o successivo |

- **Docker Compose non compare**: installalo (Debian 13: `sudo apt install docker-compose`; Ubuntu: `sudo apt install docker-compose-v2`; repository di Docker: `docker-compose-plugin`) e premi **Verifica** in Impostazioni.
- **Un download o una ricerca non riesce**: Docker Engine ha bisogno di Internet; il messaggio di errore è quello di Docker.
- **Un terminale non si apre**: il container deve essere in esecuzione e avere una shell (`sh` o `bash`).
- **Non è consentito ricreare un container**: il motivo compare nella revisione (container di Compose, creato con `--rm`, in pausa, immagine non più presente…).
- **La disinstallazione non propone una dipendenza**: la usa un altro programma, era già installata oppure non si può stabilire con certezza; il motivo compare accanto.
- **Per segnalare un problema**: Impostazioni › Sistema › **Copia** genera un rapporto senza dati sensibili.

## 27. Domande frequenti {#faq}

**L’applicazione esegue comandi `docker`?** No: usa l’API di Docker Engine. Il comando che vedi nel pannello attività è l’equivalente, così puoi riprodurlo. Docker Compose invece viene eseguito con il suo programma ufficiale.

**Devo essere amministratore?** No. Ti basta l’accesso al socket di Docker. Solo la disinstallazione dei pacchetti chiede i permessi, e solo per quel comando.

**Funziona senza Internet?** Sì. Solo le operazioni di Docker verso registri remoti hanno bisogno della rete.

**Che cosa succede ai miei container se chiudo l’applicazione?** Continuano a funzionare: li gestisce Docker. La chiusura termina i terminali aperti e smette di seguire log e statistiche.

**Posso perdere dati modificando un container?** Non viene eliminato nessun volume, cartella o immagine, e l’originale viene conservato come copia. Solo i file scritti fuori dai volumi restano nella copia, e la revisione te lo segnala prima.

**Perdo i miei dati disinstallando?** No. La configurazione, la cronologia e tutto ciò che è in Docker vengono conservati.

**Dove vengono salvate la configurazione e la cronologia?** In `~/.config/container-manage-center/` e `~/.local/state/container-manage-center/`.

**Come cambio la lingua?** In Impostazioni › Aspetto › Lingua.

## 28. Note legali {#legal}

**Nome:** Container Manage Center (abbreviato **CMC**).

**Licenza:** GNU General Public License, versione 3 o successiva (identificatore SPDX `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

Questo programma è software libero: è possibile ridistribuirlo e/o modificarlo secondo i termini della GNU General Public License pubblicata dalla Free Software Foundation, nella versione 3 della Licenza o (a scelta) in qualsiasi versione successiva. È distribuito nella speranza che sia utile, ma SENZA ALCUNA GARANZIA; senza neppure la garanzia implicita di COMMERCIABILITÀ o IDONEITÀ PER UNO SCOPO PARTICOLARE. Per maggiori dettagli consultare la GNU General Public License.

Testo completo della licenza: https://www.gnu.org/licenses/gpl-3.0.html (anche in `/usr/share/doc/container-manage-center/LICENSE`).

**Componenti di terze parti.** L'applicazione include librerie e caratteri di terze parti che mantengono le proprie licenze (MIT, Apache-2.0, MPL-2.0, OFL-1.1 e altre) e le note di copyright dei rispettivi autori; non diventano GPL né di Sergio Alfaro Lorite. I testi completi si trovano in `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md`, e la nota di copyright del pacchetto in `/usr/share/doc/container-manage-center/copyright`.

**Codice sorgente.** Il codice sorgente corrispondente viene pubblicato con ogni versione del pacchetto (`container-manage-center-X.Y.Z-src.tar.gz`).

**Marchi.** Container Manage Center è un progetto indipendente: non è affiliato a Docker, Inc. né approvato da essa. «Docker» è un marchio di Docker, Inc. ed è usato qui solo per indicare la tecnologia con cui lavora l'applicazione (Docker Engine, la sua API, Docker Compose e la CLI di Docker).

Gli stessi dati si trovano in **Impostazioni › Note legali**.
