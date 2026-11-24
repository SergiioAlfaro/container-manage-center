## 1. Qu’est-ce que Container Manage Center {#about}

Container Manage Center (**CMC**) est une application de bureau pour administrer **Docker Engine** sans dépendre du terminal, sur cet ordinateur et sur des serveurs distants par SSH. Tout ce qu’elle affiche est une donnée réelle demandée à Docker à ce moment-là, et chaque action est une opération réelle : rien n’est simulé.

- Elle dialogue avec Docker Engine par son **API officielle** : par le socket Unix sous Linux, par le canal nommé de Docker Desktop sous Windows et par SSH avec les serveurs distants.
- Elle exécute **Docker Compose** avec son programme officiel, avec des arguments séparés et sans shell.
- Chaque opération apparaît dans le **panneau d’activité** avec sa commande `docker` équivalente, pour que vous sachiez exactement ce qui a été fait.

Il n’est pas nécessaire de savoir utiliser Docker depuis le terminal : chaque rubrique explique ce qu’elle fait avant de le faire, et les opérations qui suppriment ou modifient quelque chose demandent une confirmation. Si un terme ne vous est pas familier, consultez le **Glossaire** (section 33).

## 2. Configuration requise {#requirements}

| Élément | Exigence |
|---|---|
| Système (Linux) | Debian 12 ou ultérieur, ou Ubuntu 22.04 ou ultérieur, 64 bits (x86_64) |
| Système (Windows, en test) | Windows 10 ou 11 64 bits avec Docker Desktop (section 28) |
| Docker | Docker Engine 20.10 ou ultérieur (API 1.41+), en mode normal ou rootless |
| Docker Compose | v2 ou ultérieur, uniquement pour la rubrique Docker Compose (`docker compose` ou `docker-compose` v2). Compose v1 n’est pas pris en charge |
| Bibliothèques (Linux) | WebKitGTK 4.1, GTK 3 et glibc. APT les installe avec le paquet |
| Serveurs distants | Le client SSH du système (`ssh`) et, sur le serveur, Docker Engine 20.10 ou ultérieur |

Docker et Docker Compose sont des **prérequis externes** : l’application ne les installe ni ne les configure, mais vous indique comment faire s’ils manquent.

L’application n’a pas besoin d’Internet. Seules les opérations que Docker Engine effectue auprès de registres distants (télécharger des images, chercher sur Docker Hub, construire avec des images de base absentes de l’ordinateur) nécessitent que Docker ait accès au réseau.

## 3. Installation et premier lancement {#install}

Sous Debian et Ubuntu, installez le paquet avec APT, qui résout les dépendances :

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

Pour Debian 12 et Ubuntu 22.04, utilisez le paquet compatible, dont le nom se termine par `-compat-debian12-ubuntu2204.deb`. Avant d’installer, vous pouvez vérifier le téléchargement avec le fichier `SHA256SUMS` publié avec chaque version (`sha256sum -c SHA256SUMS`).

L’application apparaît dans le menu sous le nom **Container Manage Center**. Elle s’exécute avec votre utilisateur normal : **ne la lancez pas avec sudo** (elle refuse de démarrer en root).

**Premier lancement.** À l’ouverture, elle cherche Docker Engine automatiquement. Si elle le trouve, le **Tableau de bord** affiche son état ; sinon, il affiche le diagnostic avec la cause et les étapes pour la résoudre. Il n’y a rien d’autre à configurer pour commencer.

**Mettre à jour.** Installez le nouveau paquet de la même façon : votre configuration et votre historique sont conservés.

L’installation sous Windows est expliquée à la section 28.

## 4. Fenêtre principale et navigation {#interface}

La fenêtre comporte quatre zones :

- **Barre latérale** (à gauche) : les rubriques, groupées. Elle se réduit avec **Ctrl+B** ou avec **Réduire la barre latérale**, en bas.
- **Barre supérieure** : le titre de la rubrique, le **sélecteur de serveur** (sur quel Docker vous travaillez : cet ordinateur ou un serveur distant), l’**indicateur de connexion** et le bouton **Activité**.
- **Zone de travail** : le contenu de la rubrique choisie.
- **Panneau d’activité** (à droite) : les opérations exécutées. Il s’ouvre et se ferme avec **Ctrl+Maj+A** ou avec le bouton **Activité**.

| Groupe | Rubriques |
|---|---|
| (en haut) | Tableau de bord, Serveurs |
| Ressources | Conteneurs, Gestion en masse, Images, Réseaux, Volumes, Répertoires liés |
| Orchestration | Docker Compose |
| Outils | Terminaux, Audit, Sauvegardes |
| Supervision | Surveillance, Événements et journaux |
| (en bas) | Paramètres |

Ce manuel se trouve dans **Paramètres › Aide et documentation**. L’index de gauche mène à chaque section, **Précédent** et **Suivant** parcourent le manuel dans l’ordre et la recherche n’affiche que les sections contenant le texte saisi.

| Raccourci | Action |
|---|---|
| Ctrl+B | Réduire ou développer la barre latérale |
| Ctrl+Maj+A | Ouvrir ou fermer le panneau d’activité |
| Échap | Fermer une boîte de dialogue ou un menu |
| Flèches, Début et Fin | Se déplacer dans les menus et déplacer les séparateurs de Terminaux |

Les couleurs d’état sont toujours les mêmes : **vert** pour ce qui fonctionne ou est en cours d’exécution, **ambre** pour les avertissements et **rouge** pour les erreurs.

## 5. Connexion à Docker Engine {#connection}

L’**indicateur de connexion** de la barre supérieure affiche l’état :

| Aspect | Signification |
|---|---|
| Point vert et `Docker 29.x` | Connecté. Au survol, le socket s’affiche |
| `Connexion à Docker…` | Détection ou connexion en cours |
| Rouge, `Pas de connexion à Docker` | Non connecté. Cliquez dessus pour voir le diagnostic |

En mode **automatique**, l’application essaie dans cet ordre : la variable `DOCKER_HOST` (uniquement `unix://`), le contexte Docker actif, votre socket rootless et le socket système `/var/run/docker.sock`. Elle surveille la connexion toutes les 10 secondes et se reconnecte seule lorsque Docker redevient disponible.

L’application ne demande jamais sudo, ne modifie ni permissions ni groupes et n’exécute pas à votre place les instructions du diagnostic. Votre utilisateur doit pouvoir ouvrir le socket : avec Docker rootless ou en appartenant au groupe `docker` (ce qui équivaut en pratique à des privilèges d’administrateur).

La configuration est enregistrée immédiatement dans `~/.config/container-manage-center/config.toml`. Si vous préférez indiquer un socket précis, consultez la section suivante.

## 6. Docker rootless et sockets personnalisés {#rootless}

Avec **Docker rootless**, le démon s’exécute avec votre utilisateur et le socket se trouve dans `$XDG_RUNTIME_DIR/docker.sock` (généralement `/run/user/1000/docker.sock`). L’application le détecte seule, y compris via le contexte `rootless`, et l’indique avec l’étiquette **rootless**. C’est l’option recommandée pour la sécurité.

Dans **Paramètres › Connexion à Docker**, vous pouvez :

- Choisir **Socket personnalisé** et saisir un chemin absolu.
- **Tester la connexion** : vérifie le chemin sans l’enregistrer.
- **Enregistrer et se connecter** : applique la configuration.
- **Se reconnecter** : relance la détection.

Les connexions `tcp://` ne sont pas prises en charge. Les serveurs distants s’administrent via SSH depuis **Serveurs** (section suivante).

## 7. Serveurs distants via SSH {#remote-servers}

Depuis **Serveurs**, vous pouvez administrer le Docker d'autres machines Linux accessibles en **SSH**, en même temps que celui de cet ordinateur. La connexion utilise le client SSH du système (`ssh`) et exécute `docker system dial-stdio` sur le serveur : Docker n'est **jamais** exposé sur le réseau (pas de TCP 2375) et aucun port n'est ouvert.

**Prérequis du serveur** : un serveur SSH en marche, Docker Engine 20.10 ou ultérieur (API 1.41) et un utilisateur pouvant utiliser Docker. Pour Compose, `docker compose` ou `docker-compose` sur le serveur. L'application **n'installe pas Docker, ne modifie pas les permissions ni la configuration SSH et n'ouvre aucun port** : s'il manque quelque chose, elle vous dit quoi et comment le corriger.

**Ajouter un serveur** (assistant) : informations générales (nom, hôte, utilisateur, port 22), authentification, Docker (par défaut, un socket précis ou un contexte) et **Tester la connexion**, qui vérifie sept étapes : DNS, connexion SSH, identité du serveur, authentification, Docker disponible, permissions et compatibilité de l'API.

- **Authentification** : agent SSH (recommandé), clé privée, clé avec phrase secrète ou mot de passe (déconseillé). La clé privée n'est **jamais copiée** : seul son chemin est enregistré.
- **Identifiants** : la phrase secrète ou le mot de passe sont demandés à la connexion. Si vous choisissez de **mémoriser**, ils sont enregistrés dans le **stockage sécurisé du système** (GNOME Keyring, KWallet…) ; sans stockage sécurisé, ils sont demandés à chaque fois et ne sont **jamais** enregistrés en clair. Vous pouvez les supprimer en modifiant le serveur.
- **Identité du serveur** : la première fois, l'**empreinte** (`SHA256:…`) est affichée pour que vous la vérifiiez et la confirmiez. Si elle change, la connexion s'arrête avec un avertissement d'**attaque de l'homme du milieu** possible ; elle n'est remplacée qu'après vérification. Si l'ancienne clé se trouve dans votre `~/.ssh/known_hosts`, l'application ne modifie pas ce fichier et vous donne la commande pour le faire vous-même.

**Plusieurs serveurs** : le **sélecteur** de la barre supérieure indique sur quel serveur vous travaillez, avec son état, et distingue les serveurs distants. Quand vous changez de serveur, tout l'écran passe à ce serveur. **Vue d'ensemble** les montre tous à la fois (état, version, conteneurs et ressources). Chaque serveur est indépendant : un serveur indisponible n'affecte pas les autres, et chaque opération va au serveur où vous l'avez lancée. Les confirmations indiquent **le serveur et l'adresse**, et le panneau d'activité montre le serveur de chaque opération et permet de filtrer par serveur.

**Sur un serveur distant**, conteneurs, images, réseaux, volumes, terminaux et Compose fonctionnent comme en local. Les chemins (fichiers Compose, dossiers des bind mounts) sont **ceux du serveur** : vous les saisissez et ils sont vérifiés sur le serveur ; avant d'écrire un fichier sur le serveur, la destination est confirmée. Dans **Terminaux**, vous pouvez aussi ouvrir le **shell SSH du serveur**, un type de session à part.

**Se déconnecter** ferme la session SSH et ses terminaux ; **supprimer le profil** n'efface que les données de l'application (profil, identifiants et identité enregistrée). Aucune des deux actions ne touche aux conteneurs, images, réseaux, volumes ou données du serveur.

Appartenir au groupe `docker` du serveur équivaut à un accès **root** sur ce serveur : décidez-en avec son administrateur ; Docker rootless est l'alternative la plus sûre.

Sur un serveur distant, l’**Audit**, les **Sauvegardes** et la **Surveillance** fonctionnent aussi (sections 23 à 25).

## 8. Tableau de bord et diagnostic {#dashboard}

Une fois connecté, le **Tableau de bord** affiche des données obtenues à cet instant : versions de Docker Engine et de l’API, socket et origine de la connexion, conteneurs (en cours d’exécution, en pause, arrêtés), images, données de l’ordinateur (système, noyau, CPU, mémoire), stockage et exécution (pilote, répertoire de données, cgroups, journaux) et les avertissements du démon. **Actualiser** les redemande.

Sans connexion, il affiche le **diagnostic** : la cause précise, les étapes pour la résoudre (avec les commandes à copier dans un terminal), les sockets vérifiés avec leur résultat et ce qui a été détecté comme installé. Les boutons **Se reconnecter** et **Configurer la connexion** se trouvent dans l’en-tête.

Les causes les plus fréquentes d’un échec de connexion et leur solution se trouvent à la section 30.

## 9. Gestion des conteneurs {#containers}

La rubrique **Conteneurs** liste tous les conteneurs, comme `docker ps -a` : état, nom et ID court, projet Compose, image, état Docker, ports publiés et ancienneté. Vous pouvez filtrer par **Tous / En cours d’exécution / Arrêtés** et chercher par nom, image, ID ou projet. Le menu **⋯** de chaque ligne (ou le clic droit) propose **Modifier la configuration** et **Créer une image à partir de ce conteneur**.

| État | Actions disponibles |
|---|---|
| En cours d’exécution | Arrêter, Redémarrer, Mettre en pause, Supprimer |
| En pause | Reprendre, Arrêter, Supprimer |
| Créé ou arrêté | Démarrer, Supprimer |

Arrêter et redémarrer attendent 10 secondes que le processus se termine avant de le forcer, comme Docker. Après chaque action, l’état réel du conteneur est consulté. Si Docker refuse une action, son message apparaît au-dessus du tableau et dans le panneau d’activité. Les listes se mettent à jour seules lorsque quelque chose change dans Docker, y compris depuis le terminal.

Pour appliquer une même action à de nombreux conteneurs à la fois, utilisez la **Gestion en masse** (section 13).

## 10. Créer, dupliquer, inspecter et supprimer des conteneurs {#container-lifecycle}

**Créer un conteneur** ouvre un assistant à six onglets (seule l’image est obligatoire) : Général, Environnement, Réseau, Stockage, Ressources et Avancé.

- **Commande** et **Point d’entrée** s’écrivent comme dans un terminal et sont découpés en arguments, affichés en dessous. Aucun shell n’est exécuté.
- Le panneau **Équivalent** affiche le `docker run` équivalent et le corps exact envoyé à l’API.
- **Vérification** signale les erreurs (elles empêchent la création) et les avertissements (ports privilégiés, dossiers sensibles montés en écriture, mode privilégié…).
- **Créer** crée le conteneur sans le démarrer ; **Créer et démarrer** le démarre aussi.

Un clic sur le nom d’un conteneur ouvre son **détail** : Résumé, Journaux, Terminal, Statistiques et Inspection JSON (la réponse complète de `docker inspect`, avec un bouton Copier).

**Dupliquer** ouvre l’assistant avec la configuration du conteneur et le nom `<nom>-copie`, et signale les réglages qui ne sont pas repris.

**Supprimer** demande une confirmation. Un conteneur en cours d’exécution ou en pause n’est supprimé que si vous cochez **L’arrêter de force et le supprimer**. Ses volumes anonymes peuvent aussi être supprimés ; les volumes nommés et les dossiers liés ne sont pas touchés. Action irréversible.

Les journaux, les statistiques et le terminal de la vue détaillée sont expliqués à la section 21.

## 11. Modifier la configuration d’un conteneur {#container-edit}

**Modifier la configuration** se trouve dans le menu **⋯** de chaque conteneur (également par clic droit sur la ligne) et dans l’en-tête du détail. La configuration est organisée en onglets : **Général**, **Réseau**, **Stockage**, **Variables**, **Ports**, **Ressources** et **Avancé**. Chaque modification indique comment elle s’applique :

| Modification | Mode d’application |
|---|---|
| Nom | Directe (`docker rename`) |
| Politique de redémarrage, CPU, mémoire et mémoire réservée | Directe (`docker update`), sans redémarrer le conteneur |
| Connecter ou déconnecter des réseaux | Directe et immédiate (`docker network connect` / `disconnect`) |
| Variables d’environnement, ports, volumes et dossiers, description | Nécessite de **recréer** le conteneur |
| Supprimer une limite de CPU ou de mémoire | Nécessite une recréation (`docker update` ne sait pas la supprimer) |
| Image, commande, utilisateur, répertoire de travail et autres options **Avancé** | Lecture seule : Docker ne permet pas de les modifier. Utilisez **Dupliquer** |

- **Réseau** : affiche les réseaux, l’IP, la MAC et les alias. Docker ne permet pas de modifier les alias ni l’IP d’une connexion existante : il faut déconnecter puis reconnecter. S’il s’agit du seul réseau du conteneur, un avertissement s’affiche avant la déconnexion.
- **Variables** : les valeurs qui semblent secrètes sont masquées à l’écran et ne sont jamais enregistrées dans le panneau d’activité.
- **Description** : enregistrée dans l’étiquette `cmc.description` du conteneur.

**Vérifier les modifications** compare la configuration actuelle et la nouvelle : ce qui change (avant et après, élément par élément), ce qui est conservé et ce qui nécessite une recréation. Rien n’est appliqué tant que vous n’avez pas confirmé.

**Recréation sécurisée.** Lorsqu’une modification nécessite une recréation, l’assistant :

1. Vérifie l’image, les conflits de nom et de ports avec d’autres conteneurs et les raisons qui empêchent une recréation sûre : conteneurs Compose ou Swarm, créés avec `--rm`, en pause ou en redémarrage. Dans ces cas, il ne permet pas de continuer.
2. Affiche les risques à accepter un par un : fichiers écrits en dehors des volumes (ils ne passent pas dans le nouveau conteneur), montages qui ne sont plus montés, conteneurs qui utilisent le réseau de celui-ci…
3. Après confirmation : arrête le conteneur s’il est en cours d’exécution, le renomme `<nom>-cmc-backup-<date>` (copie de sauvegarde), crée le nouveau avec **toute** la configuration d’origine plus vos modifications, connecte ses réseaux et, s’il était en cours d’exécution, le démarre et vérifie qu’il continue de fonctionner quelques secondes.
4. En cas d’échec, il supprime le nouveau conteneur (sans toucher à ses volumes), rend son nom à l’original et le redémarre.

**Aucun volume, dossier ni image n’est jamais supprimé.** Les volumes anonymes sont remontés avec leur contenu. La copie reste arrêtée dans **Général › Copies de sauvegarde**, d’où vous pouvez la **Restaurer** (le conteneur actuel est mis de côté, pas supprimé) ou la **Supprimer** quand vous n’en avez plus besoin. Toutes les opérations sont enregistrées dans le panneau d’activité, sans valeurs sensibles.

## 12. Créer une image à partir d’un conteneur {#container-commit}

**Créer une image à partir de ce conteneur** (menu **⋯** de la liste et en-tête du détail) équivaut à `docker commit` : l’état actuel du système de fichiers du conteneur est enregistré dans une nouvelle image.

- Indiquez le **dépôt** et l’**étiquette** (vide = `latest`) ; facultativement, l’**auteur** et un **message**. Le nom complet est affiché et validé avant la création.
- **Les volumes et les dossiers montés ne sont pas inclus** : seule la couche en écriture du conteneur l’est. La boîte de dialogue liste les montages laissés de côté.
- L’image **enregistre bien la configuration du conteneur**, y compris ses variables d’environnement (même celles qui contiennent des mots de passe ou des jetons) : ne la publiez pas dans un registre si elle contient des secrets.
- Si le conteneur est en cours d’exécution, il est par défaut **mis en pause** quelques secondes pendant la création de l’image, pour qu’elle soit cohérente ; vous pouvez décocher cette option. Il n’est jamais arrêté.
- Si une image porte déjà ce nom, un avertissement s’affiche et vous devez l’accepter explicitement : l’ancienne n’est pas supprimée, elle perd simplement son étiquette.
- Le conteneur d’origine n’est pas modifié. À la fin, **Voir l’image** ouvre son détail dans Images.

## 13. Gestion en masse {#bulk}

La **Gestion en masse** (dans « Ressources ») applique une même opération ou une même commande à de nombreux conteneurs à la fois, y compris sur plusieurs serveurs. Elle réunit les conteneurs de cet ordinateur et des serveurs distants **connectés** ; les autres sont signalés dans un avis. Elle est divisée en onglets : **Résumé**, **Conteneurs**, **Opérations**, **Configuration en masse**, **Commandes en masse** et **Historique**.

- **Résumé** : total, en cours d’exécution, arrêtés, en pause et en erreur. Cliquer sur un compteur filtre selon cet état. **Actualiser** et l’actualisation automatique (de 5 s à 5 min, désactivable) redemandent les données.
- **Conteneurs** : recherche par nom ou ID ; filtres par état, image, serveur et étiquette (`clé` ou `clé=valeur`) ; tri en cliquant sur le titre d’une colonne. La sélection se fait avec la case de chaque ligne, celle de la page ou **tout ce qui est filtré**.
- **Opérations** : démarrer, arrêter, redémarrer, mettre en pause, reprendre et supprimer. Ce qui ne peut pas s’appliquer dans l’état actuel est **ignoré** et expliqué.
- **Configuration en masse** : affiche la politique de redémarrage de la sélection et permet de la changer pour tous (à chaud, avec `docker update` ; pas pour ceux créés avec `--rm`). Les **étiquettes** sont seulement affichées : Docker ne permet pas de les modifier sur un conteneur existant (il faudrait le recréer). Pour un conteneur précis, utilisez **Dupliquer**.
- **Historique** : les opérations et commandes en masse précédentes, avec ce qui a été fait dans chaque conteneur.

**Avant d’appliquer**, une confirmation présente l’opération, les serveurs, les conteneurs concernés et ignorés, les conséquences et le nombre d’opérations simultanées (de 1 à 16). **Supprimer** demande de saisir le nombre de conteneurs et **ne supprime jamais les volumes ni les images**.

**Résultats** : chaque conteneur a le sien : réussi (Docker l’a confirmé), échoué (avec l’erreur), ignoré, annulé ou **inconnu** (la connexion a été coupée avant la confirmation ; actualisez pour voir son état). Si l’un échoue, les autres continuent ; rien n’est annulé ni réessayé automatiquement. **Annuler ce qui reste** arrête l’envoi des ordres. Chaque ordre est enregistré dans le panneau d’activité avec le badge « Masse », et **Voir dans l’activité** n’affiche que cette opération.

Les **Commandes en masse** exécutent une même commande dans plusieurs conteneurs en cours d’exécution avec Docker Exec, sans passer par aucun shell de cet ordinateur :

1. Choisissez le mode : **Exécutable avec arguments** (un argument par ligne, envoyé tel quel) ou **Script avec un interpréteur** (plusieurs lignes pour `/bin/sh`, `bash`, `python3`…). Vous pouvez aussi charger un exemple ou une commande fréquente enregistrée.
2. Si besoin, indiquez l’utilisateur, le répertoire de travail, le délai maximal (de 1 à 3600 s) et le nombre d’exécutions simultanées.
3. Vérifiez l’**Aperçu** : le `docker exec` équivalent, les arguments exacts que reçoit Docker et le niveau de risque (Informatif, Modification, Administratif ou Potentiellement destructeur). La classification est indicative : elle ne remplace pas la relecture de la commande.
4. Confirmez. Les commandes à risque administratif ou destructeur demandent de saisir le nombre de conteneurs.

Chaque conteneur indique son état, son code de sortie, sa durée, et `stdout` et `stderr` séparément (copiables et exportables en Markdown ou JSON avec les secrets masqués). **Annuler les exécutions en attente** ne lance pas la commande dans les conteneurs qui n’ont pas encore commencé ; **Interrompre les actives** cesse en plus d’attendre celles en cours. Docker ne permet pas d’arrêter un processus déjà lancé : quand le délai expire ou que vous interrompez, le processus peut continuer dans le conteneur, et CMC **n’arrête ni ne redémarre jamais** le conteneur pour le stopper. Les commandes fréquentes peuvent être enregistrées, **jamais avec des secrets**.

## 14. Images {#images}

La rubrique **Images** liste les images locales, comme `docker image ls` : étiquettes, ID court, taille, ancienneté et conteneurs qui les utilisent. Filtres **Toutes / Utilisées / Inutilisées / Sans étiquette** et recherche par étiquette ou ID.

- **Détail** : Résumé (avec les valeurs sensibles des variables masquées), Couches (comme `docker image history`), Conteneurs qui l’utilisent et Inspection JSON.
- **Étiqueter** ajoute un nom `dépôt:étiquette`.
- **Supprimer** : si l’image a plusieurs étiquettes, vous pouvez n’en retirer qu’une ; l’image entière exige de forcer. Docker ne supprime pas les images de conteneurs en cours d’exécution.
- **Nettoyer** supprime les images sans étiquette qu’aucun conteneur n’utilise ou, après une confirmation supplémentaire, toutes celles qui ne sont pas utilisées.

## 15. Télécharger et construire des images {#pull-build}

**Télécharger une image** demande la référence (`nginx:1.27`, `utilisateur/app`, `registre.exemple.com:5000/app:v2` ; sans étiquette, `latest` est utilisé). Identifiants :

| Option | Effet |
|---|---|
| Ceux de Docker | Utilise ce qui a été enregistré avec `docker login` (ou son *credential helper*) |
| Aucun | Téléchargement anonyme |
| Nom d’utilisateur et mot de passe | Uniquement pour ce téléchargement : ils ne sont pas enregistrés et n’apparaissent pas dans le panneau d’activité |

Le panneau **Téléchargements** affiche la progression totale et par couche ; vous pouvez changer de rubrique entre-temps et **Annuler**. **Chercher sur Docker Hub** cherche comme `docker search` (c’est Docker Engine qui le fait, et il a besoin d’Internet).

**Construire une image** ouvre un assistant : contexte (le dossier envoyé, en respectant `.dockerignore`) et Dockerfile, noms (`--tag`), arguments (`--build-arg`), métadonnées (`--label`), étape finale, sans cache, rechercher des images de base plus récentes et constructeur (**BuildKit** ou **classique**). Les étapes s’affichent avec leur état et la sortie comme `docker build --progress=plain` ; la construction peut être annulée.

## 16. Réseaux {#networks}

La rubrique **Réseaux** liste les réseaux, comme `docker network ls`, avec pilote, sous-réseaux et conteneurs. Les réseaux **bridge**, **host** et **none** sont prédéfinis et ne peuvent pas être supprimés.

- **Créer un réseau** : nom, pilote (`bridge`, `macvlan`, `ipvlan`, `overlay` ou un autre), réseau interne, adresses IPv4 (sous-réseau, passerelle et plage) et IPv6, options du pilote et métadonnées. La commande équivalente et la requête exacte s’affichent, et les erreurs sont signalées avant tout envoi à Docker.
- **Connecter** un conteneur (en cours d’exécution ou arrêté), avec alias et IP fixe facultatifs. **Déconnecter** demande une confirmation.
- **Supprimer** et **Nettoyer** avertissent si des conteneurs arrêtés ont le réseau configuré et ne pourront plus démarrer ensuite.

## 17. Volumes {#volumes}

La rubrique **Volumes** liste les volumes, comme `docker volume ls`, avec leur pilote et les conteneurs qui les montent. Les volumes anonymes sont signalés comme tels. **Calculer les tailles** demande à Docker l’espace de chacun (comme `docker system df -v`) ; ce n’est pas fait automatiquement car cela peut être long.

- **Créer un volume** : nom (facultatif) et type : local, tmpfs, NFS, un dossier de l’ordinateur ou un autre pilote.
- **Détail** : répertoire, options, métadonnées, taille, conteneurs avec leur chemin et leur mode, et inspection JSON. **Créer un conteneur avec ce volume** le monte dans `/data`.
- **Supprimer** : Docker ne supprime pas un volume monté par un conteneur, même arrêté.
- **Nettoyer** : supprime les volumes anonymes inutilisés ou, après confirmation, tous les volumes inutilisés.

**Les données d’un volume supprimé ne peuvent pas être récupérées.** Si vous en avez besoin, faites d’abord une sauvegarde (section 24).

## 18. Répertoires liés {#binds}

Cette rubrique affiche les dossiers et fichiers de votre ordinateur montés dans des conteneurs, regroupés par chemin : type, permissions et propriétaire, existence, conteneurs qui les montent, où et avec quel accès. Ceux qui demandent de l’attention sont signalés : le **socket Docker** (il donne au conteneur le contrôle de Docker), la racine ou les répertoires système et les chemins qui n’existent plus.

Docker ne permet pas d’ajouter un dossier à un conteneur existant. **Lier un dossier…** ouvre le sélecteur de dossiers puis l’assistant de conteneurs avec ce dossier monté dans `/mnt/<nom>`.

## 19. Docker Compose : projets {#compose}

La rubrique **Docker Compose** nécessite Compose v2 ou ultérieur ; s’il n’est pas installé, elle explique comment l’installer. Toutes les opérations exécutent le Docker Compose officiel et apparaissent dans le panneau d’activité avec la commande exacte.

La liste affiche les projets ayant des conteneurs dans Docker (y compris ceux déployés depuis le terminal) et ceux que vous ajoutez, avec leur état (en cours d’exécution, partiel, arrêté ou non déployé), dossier et services.

- **Ouvrir un projet…** : choisissez son `compose.yaml` (et les fichiers supplémentaires dans l’ordre où ils sont combinés).
- **Nouveau projet** : choisissez un dossier, écrivez le contenu (à partir d’un modèle) puis **Créer le projet** : il est validé avec Docker Compose avant la création de `compose.yaml`.
- **Retirer de la liste** le retire seulement de la liste ; Docker et les fichiers ne sont pas touchés.

## 20. Docker Compose : services, déploiement et profils {#compose-projects}

Dans le détail d’un projet :

- **Services** : chaque service avec son image, son état, ses conteneurs, ses ports et ses **profils**. Chaque ligne permet de déployer, démarrer, arrêter, redémarrer ou consulter les journaux de ce service.
- **Déployer** (`docker compose up --detach`) : tous les services ou ceux choisis, **profils** à activer, construire avant, politique de téléchargement des images, recréer les conteneurs et supprimer les orphelins. La boîte de dialogue affiche la commande exacte.
- **Démarrer, Arrêter, Redémarrer**, **Télécharger les images** et **Construire**.
- **Supprimer** (`docker compose down`) : conteneurs et réseaux du projet ; éventuellement ses volumes nommés (leurs données sont perdues) et ses images. Une confirmation est nécessaire.
- **Sortie** de l’opération en cours (elle peut être **interrompue**) et **Journaux** des services en direct.
- **Fichier** : modifiez les fichiers ; **Valider** les vérifie avec Docker Compose sans enregistrer, et **Enregistrer** valide d’abord et n’écrase pas si un autre programme les a modifiés.
- **Configuration** : la configuration finale selon Docker Compose. Elle peut contenir des secrets, elle reste donc masquée jusqu’à ce que vous la demandiez et n’est pas enregistrée dans le panneau d’activité.

## 21. Journaux, statistiques et terminal d’un conteneur {#logs-stats-terminal}

Dans le détail d’un conteneur :

- **Journaux** : sortie en direct (stderr en rouge). Vous pouvez choisir combien de lignes précédentes charger, filtrer, afficher l’horodatage, suivre la fin, copier ce qui est visible et arrêter le suivi. Les 10 000 dernières lignes sont conservées.
- **Statistiques** : CPU, mémoire, réseau, disque et processus chaque seconde, calculés comme `docker stats`.
- **Terminal** : **Ouvrir un terminal** lance un shell avec `docker exec -it` (Automatique utilise `bash` s’il existe, sinon `sh` ; vous pouvez aussi en choisir un). **Fermer la session** termine le shell et les processus qu’il a laissés dans le conteneur.

Ce sont des opérations réelles qui apparaissent dans le panneau d’activité tant qu’elles sont ouvertes et qui se ferment en quittant le détail. Pour travailler avec plusieurs terminaux à la fois, utilisez la rubrique **Terminaux**.

Pour voir l’évolution de l’utilisation dans le temps et recevoir des alertes, utilisez la **Surveillance** (section 25).

## 22. Terminaux simultanés {#terminals}

La rubrique **Terminaux** permet d’ouvrir et d’afficher en même temps plusieurs terminaux réels (`docker exec -it`) dans des conteneurs en cours d’exécution, dans le même conteneur ou dans d’autres.

- **Nouveau terminal** : choisissez un conteneur en cours d’exécution, le shell (Automatique, bash ou sh) et, si vous le souhaitez, un nom pour la session.
- Chaque panneau affiche le **conteneur**, le **nom de la session** et un **indicateur de connexion** (Connexion…, Connecté, Terminé ou Erreur), avec des boutons pour **renommer**, **agrandir** et **fermer**. Un double-clic sur le nom permet aussi de le renommer.
- **Disposition** : un terminal occupe tout l’espace ; deux se placent sur deux colonnes ; trois, un à gauche et deux à droite ; quatre, en grille 2×2 ; au-delà de quatre, en rangées de deux avec défilement vertical.
- Les **séparateurs** se déplacent à la souris. Lorsqu’un séparateur a le focus, les flèches le déplacent, Début et Fin l’amènent aux extrémités et un double-clic répartit l’espace à parts égales.
- Chaque session est indépendante : son propre shell, son entrée et sa sortie, sa taille et son état. Ce que vous faites dans l’une n’affecte pas les autres.
- Les sessions **restent ouvertes lorsque vous changez de rubrique**. **Tout fermer** demande une confirmation. Jusqu’à 12 sessions simultanées sont possibles.
- **Fermer** une session termine son shell et les processus qu’il a laissés en arrière-plan dans le conteneur. Si le conteneur est **arrêté** ou **supprimé**, le panneau l’indique ; une session terminée peut être **rouverte** si le conteneur existe encore.
- **Fermer l’application** termine toutes les sessions.

Le panneau d’activité enregistre l’ouverture, la fermeture et les erreurs de chaque session, mais pas ce qui est saisi ni la sortie du terminal.

## 23. Audit de sécurité {#audit}

**Outils › Audit** examine la sécurité du serveur choisi (cet ordinateur ou un serveur distant) et **ne modifie rien** : il lit seulement la configuration.

1. Choisissez **quoi auditer** : tous les conteneurs (en cours d’exécution et arrêtés) ou un seul ; aussi les images et les projets Compose (leur configuration est lue avec `docker compose config`, sans déploiement).
2. Cliquez sur **Auditer**. La progression s’affiche et l’audit peut être annulé.
3. Examinez les **constats**, par niveau (critique, élevé, moyen, faible) et groupés par conteneur, service ou image. Chacun explique ce qui se passe, **pourquoi c’est important** et ce qui est **recommandé**. Ils peuvent être filtrés par niveau, par type et par texte.

Sont vérifiés les risques pour l’ordinateur (socket Docker monté, dossiers sensibles, capacités, mode privilégié, réseau de l’hôte, PID et IPC partagés, seccomp, AppArmor ou SELinux désactivés, périphériques et ports ouverts sur toutes les interfaces) et le durcissement (processus en root, système de fichiers modifiable, sans limites de mémoire, de CPU ou de processus, sans `no-new-privileges` et images de plus d’un an). Avec Docker rootless ou `userns-remap`, le root du conteneur est marqué comme faible.

- **Proposer des corrections** (conteneurs ; le bouton indique combien) : ouvre **Modifier la configuration** avec les changements déjà dans le brouillon. Vérifiez-les et appliquez-les comme toute modification (section 11) ; si vous les abandonnez, rien ne change.
- **Vulnérabilités** : si **Trivy** ou **Grype** est installé avec sa base téléchargée, cochez la case et chaque image affiche le nombre de vulnérabilités connues, par gravité. L’application n’installe pas ces outils. Sans eux, ou sur un serveur distant, le rapport indique « Non analysé », jamais « 0 ».
- **Exporter** en JSON, Markdown ou HTML : le fichier n’est lisible que par votre utilisateur et les secrets sont masqués.
- **Audits précédents** : les 200 derniers de chaque serveur, pour les rouvrir.
- **Conteneurs dans les autres outils** : pour chaque conteneur audité, son pire constat, si la surveillance le suit, sa sauvegarde la plus récente et les tâches de sauvegarde qui l’incluent, avec des accès **Voir dans Surveillance** et **Préparer une sauvegarde**.

## 24. Sauvegardes {#backup}

**Outils › Sauvegardes** sauvegarde le serveur choisi (cet ordinateur ou un serveur distant) et restaure les sauvegardes.

1. **Dossier de destination** : **Choisir un dossier** ouvre la boîte de dialogue du système. À l’intérieur est créé `container-manage-center-copias/<serveur>/<sauvegarde>/`, lisible seulement par votre utilisateur. Les dossiers système ne peuvent pas être utilisés ; un disque externe ou un dossier réseau déjà monté, oui.
2. **Nouvelle sauvegarde** : cochez ce que vous voulez sauvegarder.
3. **Sauvegarder maintenant** affiche la progression et peut être annulé. Une sauvegarde annulée ou interrompue n’est jamais considérée comme valide.

| Élément | Comment il est sauvegardé |
|---|---|
| Volumes | Par l’API Docker, avec un conteneur auxiliaire qui n’est jamais démarré. Si un conteneur en cours d’exécution l’utilise, choisissez **à chaud** (sauvegarde non garantie) ou **en arrêtant les conteneurs** (ils sont arrêtés pendant la copie puis redémarrés) |
| Bases de données | MariaDB, MySQL, PostgreSQL, Redis ou MongoDB en cours d’exécution : le vidage est fait par l’outil du moteur lui-même dans le conteneur, avec le mot de passe de ses variables. Il est cohérent même en cas d’écritures |
| Configuration des conteneurs | Pour les recréer, sans les variables contenant des secrets |
| Projets Compose | Seulement sur cet ordinateur : leurs fichiers. Les `.env` seulement si vous cochez « Inclure les fichiers .env » |
| Configuration de l’application | Préférences, serveurs et tâches (les mots de passe sont dans le trousseau et ne sont pas copiés) |

- **Sauvegardes** : celles du serveur, avec leur état (Complète, Incomplète), éléments, taille et avertissements. **Vérifier** recalcule la somme SHA-256 de chaque fichier et indique si l’un est altéré, manquant ou en trop.
- **Restaurer** : choisissez quels éléments et où, puis cliquez sur **Aperçu**, qui vérifie l’intégrité et indique ce qui sera fait pour chacun. Un volume est restauré dans un volume **nouveau** ou vide, jamais par-dessus un volume contenant des données. Un conteneur est créé neuf, sous un autre nom et sans démarrer. Les fichiers sont écrits dans un nouveau dossier.
- **Restaurer une base de données remplace ses données actuelles** : il faut saisir « restaurer » pour confirmer.
- **Tâches planifiées** : **Enregistrer comme tâche** enregistre la sélection avec un nom, une fréquence (quotidienne, hebdomadaire ou mensuelle) et une rétention (par défaut, 7 quotidiennes, 4 hebdomadaires et 3 mensuelles). La rétention ne supprime que les anciennes sauvegardes **de cette tâche**. **Planifier** crée un minuteur utilisateur systemd (sous Windows, une tâche du Planificateur de tâches) qui fonctionne même application fermée ; il vous montre d’abord ce qu’il va créer et ne le crée que si vous l’autorisez. Les droits d’administrateur ne sont pas nécessaires. Les minuteurs utilisateur de Linux ne s’exécutent que lorsque votre session est ouverte, sauf si vous activez vous-même `loginctl enable-linger`.
- **Historique** : chaque sauvegarde, manuelle ou planifiée, avec son résultat et les anciennes sauvegardes supprimées par la rétention.

Les sauvegardes **ne sont pas chiffrées** : elles contiennent vos données telles quelles. Si vous les stockez hors de l’ordinateur, utilisez un disque chiffré.

## 25. Surveillance et alertes {#monitoring}

**Supervision › Surveillance** affiche l’utilisation des conteneurs du serveur choisi au fil du temps. Rien n’est installé sur le serveur : elle demande à Docker la même chose que `docker stats`, y compris par SSH.

- **Activer** : cochez « Surveiller ce serveur ». L’activation se fait par serveur et reste active à la réouverture de l’application. L’**intervalle** (de 2 à 60 s) est commun à tous ; **Mettre en pause** arrête tous les échantillons.
- **Résumé** : CPU et mémoire de l’ordinateur selon Docker, conteneurs en cours d’exécution, en pause et arrêtés, et utilisation totale en graphiques. Le tableau indique, par conteneur, CPU, mémoire et sa limite, réseau, disque, processus, durée de fonctionnement et redémarrages. Cliquez sur un nom pour voir ses graphiques. Le CPU est en % d’un CPU, comme `docker stats` : un conteneur qui utilise deux CPU entiers affiche 200 %.
- **Période** : 5, 15 et 30 minutes proviennent de la mémoire (depuis l’ouverture de l’application) ; 1, 6 et 24 heures, du résumé par minute stocké sur votre ordinateur.
- **Comparer** : jusqu’à 4 conteneurs en cours d’exécution et une métrique sur le même graphique.
- **Sans connexion** : si le serveur ne répond plus, les graphiques affichent une zone hachurée. Aucune donnée n’est inventée.

**Alertes** : CPU élevé et mémoire élevée (avec un seuil d’alerte et un seuil de résolution, pour ne pas alerter à chaque petite hausse), le conteneur plante et Docker le redémarre, manque de mémoire (OOM), il se termine en erreur sans que personne ne l’arrête et la connexion au serveur est perdue. Arrêter vous-même un conteneur n’est pas une erreur. Après une notification, la même alerte ne se répète pas pendant les minutes de silence (15 par défaut). Les **notifications de bureau** ne quittent pas l’ordinateur ; **Tester la notification** en envoie une de test. Toutes les alertes restent dans l’**Historique des alertes**.

La surveillance n’enregistre pas chaque échantillon dans le panneau d’activité : seulement son début et sa fin sur chaque serveur.

## 26. Événements, activité et historique des opérations {#activity}

Chaque opération apparaît dans le **panneau d’activité** : heure, badge **API** (requête à Docker Engine), **CLI** (processus, comme `docker compose` ou `apt-get`) ou **APP** (opération de l’application elle-même, comme un changement de langue, ou le résumé d’une recréation dont les étapes apparaissent à part), type, ressource, serveur, durée et état. La ligne principale est la commande équivalente, que vous pouvez copier. En dépliant une entrée, vous voyez l’opération réellement exécutée, le résultat, l’erreur et son code HTTP en cas d’échec, et les sorties des processus. Les valeurs secrètes sont masquées par `********`.

La rubrique **Événements et journaux** comporte quatre onglets :

- **En direct** : les événements Docker de cette session (un conteneur qui démarre, une image téléchargée…), qu’ils viennent de cette application ou de tout autre outil. Ils peuvent être filtrés et mis en **Pause**.
- **Enregistrés** : les événements reçus pendant que l’application était ouverte, y compris lors de sessions précédentes (les 20 000 plus récents).
- **Interroger Docker** : les événements d’une période (`docker events --since … --until …`). Cela inclut ce qui s’est passé application fermée, mais Docker ne conserve que les 256 derniers.
- **Historique des opérations** : ce que l’application a fait, y compris lors de sessions précédentes (les 10 000 plus récentes).

L’historique conserve ce qui a été fait, quand et avec quel résultat, mais jamais le corps des requêtes, les commandes ni les sorties, car ils peuvent contenir des mots de passe. Il se trouve dans `~/.local/state/container-manage-center/`, lisible seulement par votre utilisateur. Vous pouvez l’effacer depuis chaque onglet ou cesser de l’enregistrer dans les Paramètres.

## 27. Paramètres de l’application {#settings}

- **Apparence** : thème (Système, Clair, Sombre) et langue, dans cet ordre : **English**, **Español** (par défaut), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** et **Português**. Chaque langue apparaît sous son propre nom. La langue est enregistrée et conservée à la réouverture de l’application. La couleur principale est le **violet** et ne peut pas être changée.
- **Panneau d’activité** : l’ouvrir seulement quand une opération échoue, opérations conservées (50–10 000) et **Conserver l’historique entre les sessions**.
- **Connexion à Docker** (sections 5 et 6) et vérification de **Docker Compose**.
- **Mises à jour** : où sont publiées les versions et **Vérifier maintenant**. L’application ne télécharge ni n’installe jamais rien.
- **Système** : versions de l’application, du paquet, de WebKitGTK et glibc, du système et de Docker ; **Copier** génère un rapport sans données sensibles.
- **Désinstallation et composants** (section 29), **Aide et documentation** (ce manuel) et **Mentions légales** (section 34).
- **Réinitialiser les préférences**, après confirmation. Cela ne modifie pas la connexion à Docker.

Tout est enregistré immédiatement dans `~/.config/container-manage-center/config.toml`.

Les messages produits par Docker lui-même (par exemple, une erreur de Docker Engine) et la sortie des programmes s’affichent tels qu’ils arrivent, sans traduction. Les résumés techniques que le moteur de l’application écrit dans le panneau d’activité et les messages d’erreur contenant des données précises sont générés en espagnol dans toutes les langues.

## 28. Windows {#windows}

La version Windows est **en test** : l’installateur est généré, mais il n’a pas encore été validé sur un vrai Windows. C’est un installateur `.exe` pour votre utilisateur, sans droits d’administrateur, et il n’est pas signé : Windows peut donc afficher un avertissement SmartScreen à son ouverture.

Sous Windows, Container Manage Center fonctionne comme sous Linux, avec ces différences :

- **Docker local** : il se connecte à **Docker Desktop** par son canal nommé (`\\.\pipe\docker_engine`). Docker Desktop doit être démarré et votre utilisateur doit faire partie du groupe local `docker-users` (l'installateur de Docker Desktop l'y ajoute).
- **Serveurs distants par SSH** : ils utilisent le client OpenSSH de Windows (`ssh.exe`, inclus dans Windows 10 et 11). Comme il ne prend pas en charge les connexions multiplexées, chaque opération ouvre sa propre connexion : la première fois, l'identité du serveur est vérifiée comme sous Linux, et le mot de passe ou la phrase secrète enregistrés sont utilisés automatiquement. Les **serveurs de rebond** (ProxyJump) ne sont pas encore disponibles et, dans le **terminal du serveur**, le redimensionnement de la fenêtre n'est pas transmis au serveur.
- **Mots de passe mémorisés** : dans le **Gestionnaire d'identification** de Windows (informations d'identification Windows), sous le nom `container-manage-center/…`.
- **Copies planifiées** : avec le **Planificateur de tâches**, dans le dossier « Container Manage Center », avec votre utilisateur et sans privilèges d'administrateur. Elles s'exécutent quand votre session est ouverte ; si l'ordinateur était éteint à cette heure-là, la copie est faite dès que possible.
- **Notifications** : celles du système (centre de notifications).
- **Données** : configuration dans `%APPDATA%\container-manage-center` et historique dans `%LOCALAPPDATA%\container-manage-center`.
- **Désinstaller** : depuis Paramètres Windows › Applications › Applications installées. Vos données et Docker Desktop ne sont pas modifiés. La page « Désinstallation et composants » n'existe que sous Debian et Ubuntu.

## 29. Désinstallation et composants {#uninstall}

**Paramètres › Désinstallation et composants** montre ce que l’application a installé et permet de le retirer sans utiliser le terminal. **Analyser** interroge dpkg et APT sans rien modifier :

- **Paquet de l’application** (`container-manage-center`), avec sa taille.
- **Dépendances pouvant être retirées** : uniquement celles qu’APT a installées automatiquement **en même temps que l’application** (d’après son historique) et dont aucun autre paquet n’a besoin.
- **Conservées**, avec la raison : dépendances partagées (avec les paquets qui les utilisent), installées explicitement, déjà présentes auparavant, d’origine incertaine (si l’historique d’APT ne montre pas comment l’application a été installée) et composants du système.
- **Outils externes** : Docker Engine, Docker CLI et Docker Compose. **Toujours conservés.**
- **Données qui ne sont pas supprimées** : votre configuration, votre historique et les données de la fenêtre. Rien n’est touché dans Docker non plus : images, conteneurs, réseaux, volumes et projets Compose.
- La **simulation APT** complète de la désinstallation.

Vous pouvez choisir **Container Manage Center uniquement** ou **avec les dépendances pouvant être retirées**, ou cocher les paquets un par un ; le résumé indique l’espace récupéré. **Vérifier et désinstaller…** contrôle à nouveau la sélection et simule exactement la commande : si APT devait retirer quelque chose que vous n’avez pas choisi, vous ne pouvez pas continuer. La boîte de dialogue affiche la commande qui sera exécutée :

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove container-manage-center …
```

Lorsque vous confirmez, le système demande votre mot de passe d’administrateur (pkexec) **uniquement pour cette commande** ; l’application continue de s’exécuter sans privilèges. Si vous annulez l’authentification, rien n’est retiré. `purge` n’est pas utilisé. Tout est enregistré dans le panneau d’activité. Si l’application elle-même est retirée, fermez-la à la fin.

Si `pkexec` n’est pas installé, l’application ne peut pas demander d’autorisation : la boîte de dialogue affiche la commande équivalente avec `sudo` pour que vous l’exécutiez vous-même dans un terminal.

## 30. Résolution des problèmes {#troubleshooting}

| Cause affichée | Que faire |
|---|---|
| Docker ne semble pas installé | `sudo apt install docker.io` ou le guide officiel de Docker |
| Docker Engine introuvable / arrêté | `sudo systemctl enable --now docker` (rootless : `systemctl --user enable --now docker`) |
| Pas de permission pour utiliser le socket | Docker rootless, ou le groupe `docker` puis se reconnecter à la session |
| Vous appartenez déjà au groupe, mais cela ne fonctionne pas | Fermez la session et reconnectez-vous |
| Connexion Docker non prise en charge | `tcp://` ou `ssh://` dans la connexion locale : utilisez un socket local (`docker context use default`) ; pour un serveur distant, ajoutez-le dans **Serveurs** |
| Docker ne répond pas à temps | `sudo systemctl restart docker` |
| Version de Docker trop ancienne | Mettez à jour vers Docker 20.10 ou ultérieur |

- **Docker Compose n’apparaît pas** : installez-le (Debian 13 : `sudo apt install docker-compose` ; Ubuntu : `sudo apt install docker-compose-v2` ; dépôt Docker : `docker-compose-plugin`) puis cliquez sur **Vérifier** dans Paramètres.
- **Un téléchargement ou une recherche échoue** : Docker Engine a besoin d’Internet ; le message d’erreur est celui de Docker.
- **Un terminal ne s’ouvre pas** : le conteneur doit être en cours d’exécution et disposer d’un shell (`sh` ou `bash`).
- **La recréation d’un conteneur n’est pas autorisée** : la raison apparaît dans la vérification (conteneur Compose, créé avec `--rm`, en pause, image qui n’existe plus…).
- **La désinstallation ne propose pas une dépendance** : un autre programme l’utilise, elle était déjà installée ou cela ne peut pas être déterminé avec certitude ; la raison s’affiche à côté.
- **Pour signaler un problème** : Paramètres › Système › **Copier** génère un rapport sans données sensibles.

- **Un serveur distant ne se connecte pas** : **Tester la connexion** indique laquelle des sept étapes échoue (section 7).
- **Une commande en masse échoue dans un conteneur** : le conteneur doit être en cours d’exécution et disposer du programme ou de l’interpréteur indiqué ; l’erreur de Docker apparaît dans son résultat.

## 31. Sécurité et bonnes pratiques {#security}

- L’application **ne s’exécute pas en tant que root**, ne demande jamais sudo pour fonctionner et ne modifie ni les permissions du socket ni les groupes.
- Les commandes externes s’exécutent **sans shell** et avec des arguments séparés : ce que vous saisissez n’est jamais interprété comme des commandes.
- **Elle n’enregistre pas d’identifiants.** Ceux d’un registre ne servent que pour un téléchargement et n’apparaissent pas dans le panneau d’activité. Avec « Ceux de Docker », ceux de `docker login` sont utilisés.
- Les valeurs de variables comme `PASSWORD`, `TOKEN` ou `SECRET` sont masquées à l’écran et dans le panneau (`********`), même si Docker les reçoit en entier.
- L’historique persistant n’enregistre jamais le corps des requêtes, les commandes ni les sorties.
- Les opérations destructives demandent une confirmation.
- La fenêtre bloque les connexions externes. La seule connexion propre à l’application est la vérification des versions, désactivée tant que vous n’indiquez pas de source. Le bouton GitHub de l’Aide ne peut ouvrir que cette adresse, dans votre navigateur.
- Monter le socket Docker dans un conteneur ou appartenir au groupe `docker` donne le contrôle total de l’ordinateur : l’application le signale.

**Bonnes pratiques** : utilisez Docker rootless quand c’est possible ; consultez l’**Audit** de temps en temps ; faites des **sauvegardes** avant de supprimer des volumes ou de mettre à jour des bases de données ; ne publiez pas de ports sur toutes les interfaces si vous ne les utilisez que sur cet ordinateur ; ne stockez pas de mots de passe dans les images ni dans les commandes fréquentes. Dans la gestion en masse, la suppression et les commandes à risque administratif ou destructeur demandent de saisir le nombre de conteneurs.

## 32. Questions fréquentes {#faq}

**L’application exécute-t-elle des commandes `docker` ?** Non : elle utilise l’API de Docker Engine. La commande affichée dans le panneau d’activité est l’équivalent, pour que vous puissiez la reproduire. Docker Compose, lui, est exécuté avec son programme officiel.

**Puis-je perdre des données en modifiant un conteneur ?** Aucun volume, dossier ni image n’est supprimé, et l’original est conservé comme copie. Seuls les fichiers écrits en dehors des volumes restent dans la copie, et la vérification vous en avertit au préalable.

**Faut-il être administrateur ?** Non. Il suffit d’avoir accès au socket Docker. Seule la désinstallation de paquets demande des autorisations, et uniquement pour cette commande.

**Fonctionne-t-elle sans Internet ?** Oui. Seules les opérations de Docker auprès de registres distants nécessitent le réseau.

**Que deviennent mes conteneurs si je ferme l’application ?** Ils continuent de fonctionner : c’est Docker qui les gère. La fermeture termine les terminaux ouverts et arrête le suivi des journaux et des statistiques.

**Est-ce que je perds mes données en désinstallant ?** Non. Votre configuration, votre historique et tout ce qui se trouve dans Docker sont conservés.

**Où sont enregistrés la configuration et l’historique ?** Dans `~/.config/container-manage-center/` et `~/.local/state/container-manage-center/`.

**Comment changer de langue ?** Dans Paramètres › Apparence › Langue.

**Les sauvegardes planifiées continuent-elles application fermée ?** Oui : c’est le système qui les exécute (systemd ou le Planificateur de tâches de Windows), lorsque votre session est ouverte.

## 33. Glossaire {#glossary}

| Terme | Signification |
|---|---|
| Conteneur | Un programme en cours d’exécution isolé du reste de l’ordinateur, créé à partir d’une image |
| Image | Modèle en lecture seule avec le système de fichiers et la configuration pour créer des conteneurs |
| Étiquette d’image (*tag*) | Nom d’une version d’une image, comme `nginx:1.27` |
| Étiquette de conteneur (*label*) | Donnée `clé=valeur` stockée dans un conteneur, comme `cmc.description` |
| Registre | Serveur depuis lequel on télécharge des images, comme Docker Hub |
| Volume | Stockage géré par Docker qui est conservé même si le conteneur est supprimé |
| Répertoire lié (*bind mount*) | Dossier ou fichier de l’ordinateur monté dans un conteneur |
| Réseau | Réseau virtuel qui relie les conteneurs entre eux et avec l’extérieur |
| Port publié | Port de l’ordinateur qui mène à un port du conteneur |
| Politique de redémarrage | Ce que fait Docker quand un conteneur se termine ou que l’ordinateur démarre |
| Docker Engine | Le service qui exécute les conteneurs ; CMC dialogue avec lui |
| API Docker | Interface qui permet de piloter Docker Engine ; la CLI Docker et CMC l’utilisent |
| Socket Unix | Fichier spécial pour communiquer avec Docker sur le même ordinateur (`/var/run/docker.sock`) |
| Rootless | Docker exécuté par un utilisateur normal, sans privilèges d’administrateur |
| Contexte Docker | Configuration nommée indiquant à quel Docker la CLI se connecte (`docker context`) |
| `DOCKER_HOST` | Variable d’environnement qui indique à quel Docker se connecter |
| Docker Compose | Outil pour définir et exécuter des applications à plusieurs conteneurs avec un fichier YAML |
| Service et profil (Compose) | Un service est chaque partie d’un projet Compose ; un profil regroupe des services facultatifs |
| Docker Exec | Exécuter un programme dans un conteneur déjà en cours d’exécution |
| `stdout` et `stderr` | Sortie normale et sortie d’erreur d’un programme |
| Empreinte (*fingerprint*) | Résumé de la clé d’un serveur SSH (`SHA256:…`) pour vérifier son identité |
| Stockage sécurisé (trousseau) | Emplacement du système où les mots de passe sont stockés chiffrés (GNOME Keyring, KWallet, Gestionnaire d’identification) |
| Docker Desktop | Application Docker pour Windows et macOS |
| Canal nommé | Mécanisme de Windows pour se connecter à Docker Desktop (`\\.\pipe\docker_engine`) |

## 34. Informations légales et licence {#legal}

**Nom :** Container Manage Center (en abrégé **CMC**).

**Licence :** GNU General Public License, version 3 ou ultérieure (identifiant SPDX `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

Ce programme est un logiciel libre : vous pouvez le redistribuer et/ou le modifier selon les termes de la Licence publique générale GNU publiée par la Free Software Foundation, soit la version 3 de la Licence, soit (à votre choix) toute version ultérieure. Il est distribué dans l'espoir qu'il sera utile, mais SANS AUCUNE GARANTIE ; sans même la garantie implicite de QUALITÉ MARCHANDE ou d'ADÉQUATION À UN USAGE PARTICULIER. Consultez la Licence publique générale GNU pour plus de détails.

Texte complet de la licence : https://www.gnu.org/licenses/gpl-3.0.html (également dans `/usr/share/doc/container-manage-center/LICENSE`).

**Composants tiers.** L'application inclut des bibliothèques et des polices tierces qui conservent leurs propres licences (MIT, Apache-2.0, MPL-2.0, OFL-1.1 et autres) et les mentions de droit d'auteur de leurs auteurs ; elles ne deviennent ni GPL ni la propriété de Sergio Alfaro Lorite. Leurs textes complets se trouvent dans `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md`, et la mention de droit d'auteur du paquet dans `/usr/share/doc/container-manage-center/copyright`.

**Code source.** Le code source correspondant est publié avec chaque version du paquet (`container-manage-center-X.Y.Z-src.tar.gz`).

**Marques.** Container Manage Center est un projet indépendant : il n'est ni affilié à Docker, Inc. ni approuvé par elle. « Docker » est une marque de Docker, Inc. et n'est utilisée ici que pour nommer la technologie avec laquelle l'application fonctionne (Docker Engine, son API, Docker Compose et la CLI Docker).

**Windows.** L’installateur Windows inclut le chargeur WebView2 de Microsoft, couvert par une autorisation supplémentaire au titre de l’article 7 de la GPL v3.

Les mêmes informations figurent dans **Paramètres › Mentions légales**.
