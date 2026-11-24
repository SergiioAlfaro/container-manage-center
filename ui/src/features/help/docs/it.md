## 1. Che cos’è Container Manage Center {#about}

Container Manage Center (**CMC**) è un’applicazione desktop per amministrare **Docker Engine** senza dipendere dal terminale, su questo computer e su server remoti via SSH. Tutto ciò che mostra sono dati reali richiesti a Docker in quel momento, e ogni azione è un’operazione reale: nulla viene simulato.

- Comunica con Docker Engine tramite la sua **API ufficiale**: attraverso il socket Unix su Linux, la named pipe di Docker Desktop su Windows e SSH con i server remoti.
- Esegue **Docker Compose** con il suo programma ufficiale, con argomenti separati e senza shell.
- Ogni operazione compare nel **pannello attività** con il comando `docker` equivalente, così sai esattamente cosa è stato fatto.

Non serve saper usare Docker dal terminale: ogni sezione spiega cosa fa prima di farlo, e le operazioni che eliminano o modificano qualcosa chiedono conferma. Se un termine non ti è familiare, consulta il **Glossario** (sezione 33).

## 2. Requisiti di sistema {#requirements}

| Elemento | Requisito |
|---|---|
| Sistema (Linux) | Debian 12 o successivo, oppure Ubuntu 22.04 o successivo, a 64 bit (x86_64) |
| Sistema (Windows, in prova) | Windows 10 o 11 a 64 bit con Docker Desktop (sezione 28) |
| Docker | Docker Engine 20.10 o successivo (API 1.41+), in modalità normale o rootless |
| Docker Compose | v2 o successivo, solo per la sezione Docker Compose (`docker compose` o `docker-compose` v2). Compose v1 non è supportato |
| Librerie (Linux) | WebKitGTK 4.1, GTK 3 e glibc. Le installa APT con il pacchetto |
| Server remoti | Il client SSH del sistema (`ssh`) e, sul server, Docker Engine 20.10 o successivo |

Docker e Docker Compose sono **requisiti esterni**: l’applicazione non li installa né li configura, ma ti spiega come farlo se mancano.

L’applicazione non ha bisogno di Internet. Solo le operazioni che Docker Engine esegue verso registri remoti (scaricare immagini, cercare su Docker Hub, compilare con immagini di base non presenti sul computer) richiedono che Docker abbia accesso alla rete.

## 3. Installazione e primo avvio {#install}

Su Debian e Ubuntu, installa il pacchetto con APT, che risolve le dipendenze:

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

Per Debian 12 e Ubuntu 22.04 usa il pacchetto compatibile, il cui nome termina con `-compat-debian12-ubuntu2204.deb`. Prima di installare puoi verificare il download con il file `SHA256SUMS` pubblicato con ogni versione (`sha256sum -c SHA256SUMS`).

L’applicazione compare nel menu come **Container Manage Center**. Si esegue con il tuo utente normale: **non eseguirla con sudo** (rifiuta di avviarsi come root).

**Primo avvio.** All’apertura cerca Docker Engine automaticamente. Se lo trova, la **Dashboard** ne mostra lo stato; altrimenti mostra la diagnosi con la causa e i passi per risolverla. Non serve configurare altro per iniziare.

**Aggiornare.** Installa il nuovo pacchetto allo stesso modo: la configurazione e la cronologia vengono conservate.

L’installazione su Windows è spiegata nella sezione 28.

## 4. Finestra principale e navigazione {#interface}

La finestra ha quattro zone:

- **Barra laterale** (a sinistra): le sezioni, raggruppate. Si comprime con **Ctrl+B** o con **Comprimi barra laterale**, in basso.
- **Barra superiore**: il titolo della sezione, il **selettore del server** (su quale Docker stai lavorando: questo computer o un server remoto), l’**indicatore di connessione** e il pulsante **Attività**.
- **Area di lavoro**: il contenuto della sezione scelta.
- **Pannello attività** (a destra): le operazioni eseguite. Si apre e si chiude con **Ctrl+Maiusc+A** o con il pulsante **Attività**.

| Gruppo | Sezioni |
|---|---|
| (in alto) | Dashboard, Server |
| Risorse | Container, Gestione massiva, Immagini, Reti, Volumi, Directory collegate |
| Orchestrazione | Docker Compose |
| Strumenti | Terminali, Audit, Backup |
| Monitoraggio | Monitoraggio, Eventi e registri |
| (in basso) | Impostazioni |

Questo manuale si trova in **Impostazioni › Guida e documentazione**. L’indice a sinistra porta a ogni sezione, **Precedente** e **Successiva** percorrono il manuale in ordine e la ricerca mostra solo le sezioni che contengono il testo scritto.

| Scorciatoia | Azione |
|---|---|
| Ctrl+B | Comprimere o espandere la barra laterale |
| Ctrl+Maiusc+A | Aprire o chiudere il pannello attività |
| Esc | Chiudere una finestra di dialogo o un menu |
| Frecce, Home e Fine | Spostarsi nei menu e spostare i separatori di Terminali |

I colori di stato sono sempre gli stessi: **verde** per ciò che funziona o è in esecuzione, **ambra** per gli avvisi e **rosso** per gli errori.

## 5. Connessione a Docker Engine {#connection}

L’**indicatore di connessione** nella barra superiore mostra lo stato:

| Aspetto | Significato |
|---|---|
| Punto verde e `Docker 29.x` | Connesso. Passando il mouse si vede il socket |
| `Connessione a Docker…` | Rilevamento o connessione in corso |
| Rosso, `Nessuna connessione a Docker` | Non connesso. Fai clic per vedere la diagnosi |

In modalità **automatica**, l’applicazione prova in quest’ordine: la variabile `DOCKER_HOST` (solo `unix://`), il contesto attivo di Docker, il tuo socket rootless e il socket di sistema `/var/run/docker.sock`. Controlla la connessione ogni 10 secondi e si riconnette da sola quando Docker torna disponibile.

L’applicazione non chiede mai sudo, non modifica permessi né gruppi e non esegue al posto tuo le istruzioni della diagnosi. Il tuo utente deve poter aprire il socket: con Docker rootless oppure appartenendo al gruppo `docker` (il che in pratica equivale ad avere privilegi di amministratore).

La configurazione viene salvata subito in `~/.config/container-manage-center/config.toml`. Se preferisci indicare un socket specifico, consulta la sezione successiva.

## 6. Docker rootless e socket personalizzati {#rootless}

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

Su un server remoto funzionano anche l’**Audit**, i **Backup** e il **Monitoraggio** (sezioni da 23 a 25).

## 8. Dashboard e diagnosi {#dashboard}

Con la connessione attiva, la **Dashboard** mostra dati ottenuti in quel momento: versioni di Docker Engine e dell’API, socket e origine della connessione, container (in esecuzione, in pausa, arrestati), immagini, dati del computer (sistema, kernel, CPU, memoria), archiviazione ed esecuzione (driver, directory dei dati, cgroups, log) e gli avvisi del demone. **Aggiorna** li richiede di nuovo.

Senza connessione mostra la **diagnosi**: la causa precisa, i passaggi per risolverla (con i comandi da copiare in un terminale), i socket controllati con il loro esito e ciò che è stato rilevato come installato. I pulsanti **Riconnetti** e **Configura connessione** sono nell’intestazione.

Le cause più comuni di un errore di connessione e la loro soluzione sono nella sezione 30.

## 9. Gestione dei container {#containers}

La sezione **Container** elenca tutti i container, come `docker ps -a`: stato, nome e ID breve, progetto Compose, immagine, stato di Docker, porte pubblicate ed età. Puoi filtrare per **Tutti / In esecuzione / Arrestati** e cercare per nome, immagine, ID o progetto. Il menu **⋯** di ogni riga (o il clic destro) offre **Modifica configurazione** e **Crea immagine da questo container**.

| Stato | Azioni disponibili |
|---|---|
| In esecuzione | Arresta, Riavvia, Metti in pausa, Elimina |
| In pausa | Riprendi, Arresta, Elimina |
| Creato o arrestato | Avvia, Elimina |

Arresta e riavvia attendono 10 secondi che il processo termini prima di forzarlo, come Docker. Dopo ogni azione viene interrogato lo stato reale del container. Se Docker rifiuta un’azione, il suo messaggio compare sopra la tabella e nel pannello attività. Gli elenchi si aggiornano da soli quando qualcosa cambia in Docker, anche dal terminale.

Per applicare la stessa azione a molti container contemporaneamente, usa la **Gestione massiva** (sezione 13).

## 10. Creare, duplicare, ispezionare ed eliminare container {#container-lifecycle}

**Crea container** apre una procedura guidata con sei schede (solo l’immagine è obbligatoria): Generale, Ambiente, Rete, Archiviazione, Risorse e Avanzate.

- **Comando** ed **Entrypoint** si scrivono come in un terminale e vengono divisi in argomenti, mostrati sotto. Non viene eseguita alcuna shell.
- Il pannello **Equivalente** mostra il `docker run` equivalente e il corpo esatto inviato all’API.
- **Revisione** segnala gli errori (impediscono la creazione) e gli avvisi (porte privilegiate, cartelle delicate montate in scrittura, modalità privilegiata…).
- **Crea** crea il container senza avviarlo; **Crea e avvia** lo avvia anche.

Facendo clic sul nome di un container si apre il suo **dettaglio**: Riepilogo, Log, Terminale, Statistiche e Ispezione JSON (la risposta completa di `docker inspect`, con il pulsante Copia).

**Duplica** apre la procedura guidata con la configurazione del container e il nome `<nome>-copia`, e avvisa delle impostazioni che non vengono riportate.

**Elimina** chiede conferma. Un container in esecuzione o in pausa viene eliminato solo se selezioni **Arrestalo forzatamente ed eliminalo**. Facoltativamente vengono eliminati anche i suoi volumi anonimi; i volumi con nome e le cartelle collegate non vengono toccati. Non si può annullare.

I log, le statistiche e il terminale del dettaglio sono spiegati nella sezione 21.

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

## 13. Gestione massiva {#bulk}

La **Gestione massiva** (in «Risorse») applica una stessa operazione o uno stesso comando a molti container contemporaneamente, anche su più server. Riunisce i container di questo computer e dei server remoti **connessi**; gli altri sono indicati in un avviso. È divisa in schede: **Riepilogo**, **Container**, **Operazioni**, **Configurazione massiva**, **Comandi massivi** e **Cronologia**.

- **Riepilogo**: totale, in esecuzione, fermati, in pausa e con errore. Fare clic su un contatore filtra per quello stato. **Aggiorna** e l’aggiornamento automatico (da 5 s a 5 min, disattivabile) richiedono di nuovo i dati.
- **Container**: cerca per nome o ID; filtra per stato, immagine, server ed etichetta (`chiave` o `chiave=valore`); ordina facendo clic sul titolo di una colonna. Si seleziona con la casella di ogni riga, quella della pagina o **tutto il filtrato**.
- **Operazioni**: avviare, fermare, riavviare, mettere in pausa, riprendere ed eliminare. Ciò che non si può applicare nello stato attuale viene **saltato** e spiegato.
- **Configurazione massiva**: mostra la politica di riavvio della selezione e permette di cambiarla in tutti (a caldo, con `docker update`; non in quelli creati con `--rm`). Le **etichette** sono solo mostrate: Docker non permette di modificarle in un container esistente (bisognerebbe ricrearlo). Per un singolo container, usa **Duplica**.
- **Cronologia**: le operazioni e i comandi massivi precedenti, con ciò che è stato fatto in ogni container.

**Prima di applicare** viene mostrata una conferma con l’operazione, i server, i container interessati e quelli saltati, le conseguenze e quante operazioni vengono eseguite in parallelo (da 1 a 16). **Elimina** chiede di scrivere il numero di container e **non elimina mai volumi né immagini**.

**Risultati**: ogni container ha il proprio: riuscito (Docker l’ha confermato), fallito (con l’errore), saltato, annullato o **sconosciuto** (la connessione si è interrotta prima della conferma; aggiorna per vederne lo stato). Se uno fallisce, gli altri continuano; nulla viene annullato né ritentato automaticamente. **Annulla ciò che resta** smette di inviare ordini. Ogni ordine viene registrato nel pannello attività con il distintivo «Massiva», e **Vedi nell’attività** mostra solo quell’operazione.

I **Comandi massivi** eseguono uno stesso comando in diversi container in esecuzione con Docker Exec, senza passare da alcuna shell di questo computer:

1. Scegli la modalità: **Eseguibile con argomenti** (un argomento per riga, inviato così com’è) o **Script con un interprete** (più righe per `/bin/sh`, `bash`, `python3`…). Puoi anche caricare un esempio o un comando frequente salvato.
2. Se serve, indica l’utente, la directory di lavoro, il tempo massimo (da 1 a 3600 s) e quante esecuzioni in parallelo.
3. Controlla l’**Anteprima**: il `docker exec` equivalente, gli argomenti esatti che riceve Docker e il livello di rischio (Informativo, Modifica, Amministrativo o Potenzialmente distruttivo). La classificazione è indicativa: non sostituisce la revisione del comando.
4. Conferma. I comandi a rischio amministrativo o distruttivo chiedono di scrivere il numero di container.

Ogni container indica il proprio stato, codice di uscita, durata, e `stdout` e `stderr` separati (copiabili ed esportabili in Markdown o JSON con i segreti nascosti). **Annulla quelle in attesa** non avvia il comando nei container che non hanno ancora iniziato; **Interrompi quelle attive** smette inoltre di attendere quelli in corso. Docker non permette di fermare un processo già avviato: allo scadere del tempo o interrompendo, il processo può continuare nel container, e CMC **non** ferma né riavvia **mai** il container per bloccarlo. I comandi frequenti si possono salvare, **mai con segreti**.

## 14. Immagini {#images}

La sezione **Immagini** elenca le immagini locali, come `docker image ls`: tag, ID breve, dimensione, età e quali container le usano. Filtri **Tutte / In uso / Inutilizzate / Senza tag** e ricerca per tag o ID.

- **Dettaglio**: Riepilogo (con i valori sensibili delle variabili nascosti), Livelli (come `docker image history`), Container che la usano e Ispezione JSON.
- **Aggiungi tag** aggiunge un nome `repository:tag`.
- **Elimina**: se l’immagine ha più tag puoi rimuoverne solo uno; l’immagine intera richiede di forzare. Docker non elimina le immagini dei container in esecuzione.
- **Pulisci** elimina le immagini senza tag non usate da alcun container oppure, con una conferma aggiuntiva, tutte quelle non usate.

## 15. Scaricare e compilare immagini {#pull-build}

**Scarica immagine** chiede il riferimento (`nginx:1.27`, `utente/app`, `registro.esempio.com:5000/app:v2`; senza tag si usa `latest`). Credenziali:

| Opzione | Che cosa fa |
|---|---|
| Quelle di Docker | Usa quanto salvato con `docker login` (o il suo *credential helper*) |
| Nessuna | Download anonimo |
| Nome utente e password | Solo per questo download: non vengono salvati e non compaiono nel pannello attività |

Il pannello **Download** mostra l’avanzamento totale e per livello; nel frattempo puoi cambiare sezione e **Annullare**. **Cerca su Docker Hub** cerca come `docker search` (lo fa Docker Engine, che ha bisogno di Internet).

**Compila immagine** apre una procedura guidata: contesto (la cartella inviata, rispettando `.dockerignore`) e Dockerfile, nomi (`--tag`), argomenti (`--build-arg`), metadati (`--label`), fase finale, senza cache, cerca immagini di base più recenti e compilatore (**BuildKit** o **classico**). Si vedono i passaggi con il loro stato e l’output come `docker build --progress=plain`; si può annullare.

## 16. Reti {#networks}

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

**I dati di un volume eliminato non si possono recuperare.** Se ti servono, fai prima un backup (sezione 24).

## 18. Directory collegate {#binds}

Questa sezione mostra le cartelle e i file del tuo computer montati all’interno dei container, raggruppati per percorso: tipo, permessi e proprietario, se esistono, quali container li montano, dove e con quale accesso. Vengono segnalati quelli che richiedono attenzione: il **socket di Docker** (dà al container il controllo di Docker), la radice o le directory di sistema e i percorsi che non esistono più.

Docker non permette di aggiungere una cartella a un container esistente. **Collega una cartella…** apre il selettore di cartelle e poi la procedura guidata dei container con quella cartella montata in `/mnt/<nome>`.

## 19. Docker Compose: progetti {#compose}

La sezione **Docker Compose** richiede Compose v2 o successivo; se non è installato, spiega come installarlo. Tutte le operazioni eseguono il Docker Compose ufficiale e compaiono nel pannello attività con il comando esatto.

L’elenco mostra i progetti con container in Docker (anche quelli distribuiti dal terminale) e quelli che aggiungi, con il loro stato (in esecuzione, parziale, arrestato o non distribuito), cartella e servizi.

- **Apri progetto…**: scegli il suo `compose.yaml` (e i file aggiuntivi nell’ordine in cui vengono combinati).
- **Nuovo progetto**: scegli una cartella, scrivi il contenuto (partendo da un modello) e **Crea progetto**: viene convalidato con Docker Compose prima di creare `compose.yaml`.
- **Rimuovi dall’elenco** lo toglie solo dall’elenco; non tocca Docker né i file.

## 20. Docker Compose: servizi, distribuzione e profili {#compose-projects}

Nel dettaglio di un progetto:

- **Servizi**: ogni servizio con la sua immagine, stato, container, porte e **profili**. Ogni riga permette di distribuire, avviare, arrestare, riavviare o vedere i log di quel servizio.
- **Distribuisci** (`docker compose up --detach`): tutti i servizi o quelli scelti, **profili** da attivare, compilare prima, criterio di download delle immagini, ricreare i container ed eliminare gli orfani. La finestra di dialogo mostra il comando esatto.
- **Avvia, Arresta, Riavvia**, **Scarica immagini** e **Compila**.
- **Elimina** (`docker compose down`): container e reti del progetto; facoltativamente i suoi volumi con nome (i loro dati vanno persi) e le sue immagini. Va confermato.
- **Output** dell’operazione in corso (si può **Interrompere**) e **Log** dei servizi in diretta.
- **File**: modifica i file; **Convalida** li verifica con Docker Compose senza salvarli, e **Salva** li convalida prima e non sovrascrive se un altro programma li ha modificati.
- **Configurazione**: la configurazione finale secondo Docker Compose. Può contenere segreti, quindi resta nascosta finché non la richiedi e non viene salvata nel pannello attività.

## 21. Log, statistiche e terminale di un container {#logs-stats-terminal}

Nel dettaglio di un container:

- **Log**: output in diretta (stderr in rosso). Puoi scegliere quante righe precedenti caricare, filtrare, mostrare l’ora, seguire la fine, copiare ciò che è visibile e smettere di seguire. Vengono conservate le ultime 10.000 righe.
- **Statistiche**: CPU, memoria, rete, disco e processi ogni secondo, calcolati come `docker stats`.
- **Terminale**: **Apri terminale** avvia una shell con `docker exec -it` (Automatica usa `bash` se esiste, altrimenti `sh`; puoi anche sceglierne una). **Chiudi sessione** termina la shell e i processi che ha lasciato nel container.

Sono operazioni reali che compaiono nel pannello attività finché sono aperte e si chiudono quando esci dal dettaglio. Per lavorare con più terminali contemporaneamente, usa la sezione **Terminali**.

Per vedere l’andamento dell’utilizzo nel tempo e ricevere avvisi, usa il **Monitoraggio** (sezione 25).

## 22. Terminali simultanei {#terminals}

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

## 23. Audit di sicurezza {#audit}

**Strumenti › Audit** esamina la sicurezza del server scelto (questo computer o uno remoto) e **non modifica nulla**: legge solo la configurazione.

1. Scegli **cosa sottoporre ad audit**: tutti i container (in esecuzione e fermati) o uno solo; anche le immagini e i progetti Compose (la loro configurazione viene letta con `docker compose config`, senza distribuire).
2. Fai clic su **Esegui audit**. Si vede l’avanzamento e si può annullare.
3. Esamina i **risultati**, per livello (critico, alto, medio, basso) e raggruppati per container, servizio o immagine. Ognuno spiega cosa succede, **perché è importante** e cosa si **consiglia**. Si possono filtrare per livello, tipo e testo.

Vengono controllati i rischi per il computer (socket Docker montato, cartelle sensibili, capability, modalità privilegiata, rete dell’host, PID e IPC condivisi, seccomp, AppArmor o SELinux disattivati, dispositivi e porte aperte su tutte le interfacce) e l’hardening (processo come root, file system modificabile, senza limiti di memoria, CPU o processi, senza `no-new-privileges` e immagini con più di un anno). Con Docker rootless o `userns-remap`, il root del container è segnato come basso.

- **Proponi correzioni** (container; il pulsante indica quante): apre **Modifica configurazione** con le modifiche già nella bozza. Rivedile e applicale come qualsiasi modifica (sezione 11); se le scarti, non cambia nulla.
- **Vulnerabilità**: se hai **Trivy** o **Grype** installati con il loro database scaricato, seleziona la casella e ogni immagine mostra quante vulnerabilità note ha, per gravità. L’applicazione non installa questi strumenti. Senza di essi, o su un server remoto, il rapporto indica «Non analizzato», mai «0».
- **Esporta** in JSON, Markdown o HTML: il file è leggibile solo dal tuo utente e i segreti sono nascosti.
- **Audit precedenti**: gli ultimi 200 di ogni server, per riaprirli.
- **Container negli altri strumenti**: per ogni container sottoposto ad audit, il risultato peggiore, se il monitoraggio lo controlla, il backup più recente e le attività di backup che lo includono, con accessi a **Vedi in Monitoraggio** e **Prepara un backup**.

## 24. Backup {#backup}

**Strumenti › Backup** esegue backup del server scelto (questo computer o uno remoto) e li ripristina.

1. **Cartella di destinazione**: **Scegli cartella** apre la finestra di dialogo del sistema. Al suo interno viene creata `container-manage-center-copias/<server>/<backup>/`, leggibile solo dal tuo utente. Non si possono usare cartelle di sistema; un disco esterno o una cartella di rete già montata sì.
2. **Nuovo backup**: seleziona cosa vuoi salvare.
3. **Esegui backup ora** mostra l’avanzamento e si può annullare. Un backup annullato o interrotto non viene mai considerato valido.

| Elemento | Come viene salvato |
|---|---|
| Volumi | Tramite l’API di Docker, con un container ausiliario che non viene mai avviato. Se un container in esecuzione lo usa, scegli **a caldo** (backup non garantito) o **fermando i container** (vengono fermati durante la copia e riavviati) |
| Database | MariaDB, MySQL, PostgreSQL, Redis o MongoDB in esecuzione: il dump lo esegue lo strumento del motore stesso dentro il container, con la password delle sue variabili. È coerente anche con scritture in corso |
| Configurazione dei container | Per ricrearli, senza le variabili con segreti |
| Progetti Compose | Solo su questo computer: i loro file. I `.env` solo se selezioni «Includi i file .env» |
| Configurazione dell’applicazione | Preferenze, server e attività (le password sono nel portachiavi e non vengono copiate) |

- **Backup**: quelli del server, con il loro stato (Completo, Incompleto), elementi, dimensione e avvisi. **Verifica** ricalcola la somma SHA-256 di ogni file e indica se qualcuno è alterato, manca o è in più.
- **Ripristina**: scegli quali elementi e dove, poi fai clic su **Anteprima**, che verifica l’integrità e indica cosa si farà con ciascuno. Un volume si ripristina in uno **nuovo** o vuoto, mai sopra uno con dati. Un container viene creato nuovo, con un altro nome e senza avviarlo. I file vengono scritti in una nuova cartella.
- **Ripristinare un database sostituisce i suoi dati attuali**: bisogna scrivere «ripristina» per confermarlo.
- **Attività pianificate**: **Salva come attività** salva la selezione con un nome, una frequenza (giornaliera, settimanale o mensile) e una conservazione (per impostazione predefinita 7 giornalieri, 4 settimanali e 3 mensili). La conservazione elimina solo i backup vecchi **di quell’attività**. **Pianifica** crea un timer utente di systemd (su Windows, un’attività dell’Utilità di pianificazione) che funziona anche ad applicazione chiusa; prima ti mostra cosa creerà e lo crea solo se lo autorizzi. Non servono diritti di amministratore. I timer utente di Linux vengono eseguiti solo con la sessione aperta, a meno che tu non attivi `loginctl enable-linger`.
- **Cronologia**: ogni backup, manuale o pianificato, con il suo risultato e i backup vecchi eliminati dalla conservazione.

I backup **non sono cifrati**: contengono i tuoi dati così come sono. Se li conservi fuori dal computer, usa un disco cifrato.

## 25. Monitoraggio e avvisi {#monitoring}

**Monitoraggio › Monitoraggio** mostra l’utilizzo dei container del server scelto nel tempo. Non installa nulla sul server: chiede a Docker le stesse informazioni di `docker stats`, anche via SSH.

- **Attivare**: seleziona «Monitora questo server». Si attiva per ciascun server e resta attivo alla riapertura dell’applicazione. L’**intervallo** (da 2 a 60 s) è comune a tutti; **Metti in pausa** ferma tutti i campionamenti.
- **Riepilogo**: CPU e memoria del computer secondo Docker, container in esecuzione, in pausa e fermati, e l’utilizzo totale in grafici. La tabella mostra, per container, CPU, memoria e il suo limite, rete, disco, processi, tempo di attività e riavvii. Fai clic su un nome per vederne i grafici. La CPU è in % di una CPU, come `docker stats`: un container che usa due CPU intere segna 200 %.
- **Periodo**: 5, 15 e 30 minuti provengono dalla memoria (da quando hai aperto l’applicazione); 1, 6 e 24 ore dal riepilogo al minuto salvato sul tuo computer.
- **Confronta**: fino a 4 container in esecuzione e una metrica nello stesso grafico.
- **Senza connessione**: se il server smette di rispondere, i grafici mostrano un tratto tratteggiato. Nessun dato viene inventato.

**Avvisi**: CPU alta e memoria alta (con una soglia di avviso e una di risoluzione, per non avvisare a ogni piccolo aumento), il container si arresta e Docker lo riavvia, memoria esaurita (OOM), termina con errore senza che nessuno lo fermi e si perde la connessione con il server. Fermare tu un container non è un errore. Dopo una notifica, lo stesso avviso non si ripete durante i minuti di silenzio (15 per impostazione predefinita). Le **notifiche desktop** non escono dal computer; **Prova notifica** ne invia una di prova. Tutti gli avvisi restano nella **Cronologia degli avvisi**.

Il monitoraggio non registra ogni campione nel pannello attività: solo quando inizia e quando termina su ciascun server.

## 26. Eventi, attività e cronologia delle operazioni {#activity}

Ogni operazione compare nel **pannello attività**: ora, distintivo **API** (richiesta a Docker Engine), **CLI** (processo, come `docker compose` o `apt-get`) o **APP** (operazione dell’applicazione stessa, come un cambio di lingua, o il riepilogo di una ricreazione i cui passi compaiono a parte), tipo, risorsa, server, durata e stato. La riga principale è il comando equivalente, che puoi copiare. Espandendo una voce vedi l’operazione realmente eseguita, il risultato, l’errore e il suo codice HTTP se è fallita, e l’output dei processi. I valori segreti sono nascosti con `********`.

La sezione **Eventi e registri** ha quattro schede:

- **In diretta**: gli eventi Docker di questa sessione (un container che si avvia, un’immagine scaricata…), che vengano da questa applicazione o da qualsiasi altro strumento. Si possono filtrare e **mettere in pausa**.
- **Salvati**: gli eventi ricevuti con l’applicazione aperta, anche di sessioni precedenti (i 20.000 più recenti).
- **Interroga Docker**: gli eventi di un periodo (`docker events --since … --until …`). Include ciò che è successo ad applicazione chiusa, ma Docker conserva solo gli ultimi 256.
- **Cronologia delle operazioni**: ciò che l’applicazione ha fatto, anche in sessioni precedenti (le 10.000 più recenti).

La cronologia registra cosa è stato fatto, quando e con quale risultato, ma mai il corpo delle richieste, i comandi né l’output, perché possono contenere password. Si trova in `~/.local/state/container-manage-center/`, leggibile solo dal tuo utente. Puoi cancellarla da ogni scheda o smettere di salvarla nelle Impostazioni.

## 27. Impostazioni dell’applicazione {#settings}

- **Aspetto**: tema (Sistema, Chiaro, Scuro) e lingua, in quest’ordine: **English**, **Español** (predefinita), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** e **Português**. Ogni lingua compare con il proprio nome. La lingua viene salvata e mantenuta alla riapertura dell’applicazione. Il colore principale è il **viola** e non si può cambiare.
- **Pannello attività**: aprirlo solo quando un’operazione fallisce, operazioni conservate (50–10.000) e **Conserva la cronologia tra le sessioni**.
- **Connessione a Docker** (sezioni 5 e 6) e verifica di **Docker Compose**.
- **Aggiornamenti**: dove vengono pubblicate le versioni e **Controlla ora**. L’applicazione non scarica né installa mai nulla.
- **Sistema**: versioni dell’applicazione, del pacchetto, di WebKitGTK e glibc, del sistema e di Docker; **Copia** genera un rapporto senza dati sensibili.
- **Disinstallazione e componenti** (sezione 29), **Guida e documentazione** (questo manuale) e **Note legali** (sezione 34).
- **Ripristina le preferenze**, dopo la conferma. Non cambia la connessione a Docker.

Tutto viene salvato subito in `~/.config/container-manage-center/config.toml`.

I messaggi generati da Docker stesso (per esempio, un errore di Docker Engine) e l’output dei programmi vengono mostrati così come arrivano, senza traduzione. I riepiloghi tecnici che il motore dell’applicazione scrive nel pannello attività e i messaggi di errore con dati specifici vengono generati in spagnolo in tutte le lingue.

## 28. Windows {#windows}

La versione per Windows è **in prova**: il programma di installazione viene generato, ma non è ancora stato convalidato su un Windows reale. È un programma di installazione `.exe` per il tuo utente, senza diritti di amministratore, e non è firmato, quindi Windows può mostrare un avviso di SmartScreen all’apertura.

Su Windows, Container Manage Center funziona come su Linux, con queste differenze:

- **Docker locale**: si connette a **Docker Desktop** tramite la sua named pipe (`\\.\pipe\docker_engine`). Docker Desktop deve essere in esecuzione e il tuo utente deve far parte del gruppo locale `docker-users` (lo aggiunge il programma di installazione di Docker Desktop).
- **Server remoti via SSH**: usano il client OpenSSH di Windows (`ssh.exe`, incluso in Windows 10 e 11). Poiché non supporta le connessioni multiplexate, ogni operazione apre la propria connessione: la prima volta l'identità del server viene verificata come su Linux, e la password o la passphrase salvata viene usata automaticamente. I **server intermedi** (ProxyJump) non sono ancora disponibili e, nel **terminale del server**, il ridimensionamento della finestra non arriva al server.
- **Password memorizzate**: nella **Gestione credenziali** di Windows (credenziali di Windows), con il nome `container-manage-center/…`.
- **Copie pianificate**: con l'**Utilità di pianificazione**, nella cartella «Container Manage Center», con il tuo utente e senza privilegi di amministratore. Vengono eseguite con la sessione aperta; se il computer era spento a quell'ora, la copia viene fatta appena possibile.
- **Notifiche**: quelle di sistema (centro notifiche).
- **Dati**: configurazione in `%APPDATA%\container-manage-center` e cronologia in `%LOCALAPPDATA%\container-manage-center`.
- **Disinstallare**: da Impostazioni di Windows › App › App installate. I tuoi dati e Docker Desktop non vengono toccati. La sezione «Disinstallazione e componenti» esiste solo su Debian e Ubuntu.

## 29. Disinstallazione e componenti {#uninstall}

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

## 30. Risoluzione dei problemi {#troubleshooting}

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

- **Un server remoto non si connette**: **Prova connessione** indica quale dei sette passi fallisce (sezione 7).
- **Un comando massivo fallisce in un container**: il container deve essere in esecuzione e avere il programma o l’interprete indicato; l’errore di Docker compare nel suo risultato.

## 31. Sicurezza e buone pratiche {#security}

- L’applicazione **non viene eseguita come root**, non chiede mai sudo per funzionare e non modifica i permessi del socket né i gruppi.
- I comandi esterni vengono eseguiti **senza shell** e con argomenti separati: ciò che scrivi non viene mai interpretato come comandi.
- **Non salva credenziali.** Quelle di un registro servono solo per un download e non compaiono nel pannello attività. Con «Quelle di Docker» si usano quelle di `docker login`.
- I valori di variabili come `PASSWORD`, `TOKEN` o `SECRET` sono nascosti sullo schermo e nel pannello (`********`), anche se Docker li riceve per intero.
- La cronologia persistente non salva mai il corpo delle richieste, i comandi né l’output.
- Le operazioni distruttive chiedono conferma.
- La finestra blocca le connessioni esterne. L’unica connessione propria dell’applicazione è il controllo delle versioni, disattivato finché non indichi una fonte. Il pulsante GitHub della Guida può aprire solo quell’indirizzo, nel tuo browser.
- Montare il socket di Docker in un container o appartenere al gruppo `docker` dà il controllo totale del computer: l’applicazione lo segnala.

**Buone pratiche**: usa Docker rootless quando puoi; consulta l’**Audit** di tanto in tanto; fai **backup** prima di eliminare volumi o aggiornare database; non pubblicare porte su tutte le interfacce se le usi solo su questo computer; non salvare password nelle immagini né nei comandi frequenti. Nella gestione massiva, l’eliminazione e i comandi a rischio amministrativo o distruttivo chiedono di scrivere il numero di container.

## 32. Domande frequenti {#faq}

**L’applicazione esegue comandi `docker`?** No: usa l’API di Docker Engine. Il comando che vedi nel pannello attività è l’equivalente, così puoi riprodurlo. Docker Compose invece viene eseguito con il suo programma ufficiale.

**Devo essere amministratore?** No. Ti basta l’accesso al socket di Docker. Solo la disinstallazione dei pacchetti chiede i permessi, e solo per quel comando.

**Funziona senza Internet?** Sì. Solo le operazioni di Docker verso registri remoti hanno bisogno della rete.

**Che cosa succede ai miei container se chiudo l’applicazione?** Continuano a funzionare: li gestisce Docker. La chiusura termina i terminali aperti e smette di seguire log e statistiche.

**Posso perdere dati modificando un container?** Non viene eliminato nessun volume, cartella o immagine, e l’originale viene conservato come copia. Solo i file scritti fuori dai volumi restano nella copia, e la revisione te lo segnala prima.

**Perdo i miei dati disinstallando?** No. La configurazione, la cronologia e tutto ciò che è in Docker vengono conservati.

**Dove vengono salvate la configurazione e la cronologia?** In `~/.config/container-manage-center/` e `~/.local/state/container-manage-center/`.

**Come cambio la lingua?** In Impostazioni › Aspetto › Lingua.

**I backup pianificati continuano a funzionare ad applicazione chiusa?** Sì: li esegue il sistema (systemd o l’Utilità di pianificazione di Windows), con la sessione aperta.

## 33. Glossario {#glossary}

| Termine | Significato |
|---|---|
| Container | Un programma in esecuzione isolato dal resto del computer, creato a partire da un’immagine |
| Immagine | Modello in sola lettura con il file system e la configurazione per creare container |
| Tag di un’immagine | Nome di una versione di un’immagine, come `nginx:1.27` |
| Etichetta di un container (*label*) | Dato `chiave=valore` salvato in un container, come `cmc.description` |
| Registro | Server da cui si scaricano le immagini, come Docker Hub |
| Volume | Archiviazione gestita da Docker che si conserva anche se il container viene eliminato |
| Directory collegata (*bind mount*) | Cartella o file del computer montato dentro un container |
| Rete | Rete virtuale che collega i container tra loro e con l’esterno |
| Porta pubblicata | Porta del computer che porta a una porta del container |
| Politica di riavvio | Cosa fa Docker quando un container termina o il computer si avvia |
| Docker Engine | Il servizio che esegue i container; CMC comunica con esso |
| API di Docker | Interfaccia con cui si controlla Docker Engine; la usano la CLI di Docker e CMC |
| Socket Unix | File speciale per comunicare con Docker sullo stesso computer (`/var/run/docker.sock`) |
| Rootless | Docker eseguito da un utente normale, senza privilegi di amministratore |
| Contesto Docker | Configurazione con nome che indica a quale Docker si connette la CLI (`docker context`) |
| `DOCKER_HOST` | Variabile d’ambiente che indica a quale Docker connettersi |
| Docker Compose | Strumento per definire ed eseguire applicazioni con più container tramite un file YAML |
| Servizio e profilo (Compose) | Un servizio è ogni parte di un progetto Compose; un profilo raggruppa servizi facoltativi |
| Docker Exec | Eseguire un programma dentro un container già in esecuzione |
| `stdout` e `stderr` | Output normale e output degli errori di un programma |
| Impronta (*fingerprint*) | Riassunto della chiave di un server SSH (`SHA256:…`) per verificarne l’identità |
| Archivio sicuro (portachiavi) | Luogo del sistema in cui le password sono conservate cifrate (GNOME Keyring, KWallet, Gestione credenziali) |
| Docker Desktop | Applicazione Docker per Windows e macOS |
| Named pipe | Meccanismo di Windows con cui ci si connette a Docker Desktop (`\\.\pipe\docker_engine`) |

## 34. Informazioni legali e licenza {#legal}

**Nome:** Container Manage Center (abbreviato **CMC**).

**Licenza:** GNU General Public License, versione 3 o successiva (identificatore SPDX `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

Questo programma è software libero: è possibile ridistribuirlo e/o modificarlo secondo i termini della GNU General Public License pubblicata dalla Free Software Foundation, nella versione 3 della Licenza o (a scelta) in qualsiasi versione successiva. È distribuito nella speranza che sia utile, ma SENZA ALCUNA GARANZIA; senza neppure la garanzia implicita di COMMERCIABILITÀ o IDONEITÀ PER UNO SCOPO PARTICOLARE. Per maggiori dettagli consultare la GNU General Public License.

Testo completo della licenza: https://www.gnu.org/licenses/gpl-3.0.html (anche in `/usr/share/doc/container-manage-center/LICENSE`).

**Componenti di terze parti.** L'applicazione include librerie e caratteri di terze parti che mantengono le proprie licenze (MIT, Apache-2.0, MPL-2.0, OFL-1.1 e altre) e le note di copyright dei rispettivi autori; non diventano GPL né di Sergio Alfaro Lorite. I testi completi si trovano in `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md`, e la nota di copyright del pacchetto in `/usr/share/doc/container-manage-center/copyright`.

**Codice sorgente.** Il codice sorgente corrispondente viene pubblicato con ogni versione del pacchetto (`container-manage-center-X.Y.Z-src.tar.gz`).

**Marchi.** Container Manage Center è un progetto indipendente: non è affiliato a Docker, Inc. né approvato da essa. «Docker» è un marchio di Docker, Inc. ed è usato qui solo per indicare la tecnologia con cui lavora l'applicazione (Docker Engine, la sua API, Docker Compose e la CLI di Docker).

**Windows.** Il programma di installazione per Windows include il caricatore WebView2 di Microsoft, coperto da un permesso aggiuntivo ai sensi della sezione 7 della GPL v3.

Gli stessi dati si trovano in **Impostazioni › Note legali**.
