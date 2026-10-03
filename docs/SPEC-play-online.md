# PitBox — Page Online (serveurs multijoueur)

Sep 30, 2026 · @Théo

## Parti pris

La page Online de PitBox répond à une seule question : **« rejoindre vite le bon serveur, avec tout ce qu'il faut »**. Tout le reste (config fine, assists, grip) passe au second plan ou dans le détail.

Trois règles de lecture guident la maquette :

- **Gros et visuel pour ce qui décide** : circuit (image), remplissage, session en cours, voitures. Pas de petits mots en gris.
- **Ce que PitBox sait et que CM ne met pas en avant** : ton contenu installé. Chaque serveur est jugé contre ta bibliothèque (prêt / contenu manquant).
- **Filtres en tokens**, dans la même grammaire que la bibliothèque PitBox, au lieu d'une barre de cases à cocher.

Rappel design system : UI en anglais, mono pour les données, deux niveaux de gris, en-têtes de section rouges mono majuscules.

## Cas d'usage

Deux usages dominent, et ce ne sont pas des recherches : **rejoindre un serveur qu'on connaît déjà**, et **réussir à y entrer**. La recherche par critères existe, mais elle porte sur une ambiance (freeroam, drift, course) plus que sur des attributs fins de voiture. Tu avais raison : les filtres de bibliothèque ne répondent à aucun des deux premiers.

| # | Cas d'usage | Ce qui le montre | Ce que la page optimise |
| --- | --- | --- | --- |
| 1 | **Rejoindre un serveur connu** : celui d'un ami, d'une communauté Discord, celui d'hier | CM permet de marquer des joueurs comme amis puis de n'afficher que les serveurs où ils roulent ([Steam](https://steamcommunity.com/app/244210/discussions/0/1458455461499678231/)). Les communautés partagent des liens de connexion directe ([OverTake](https://www.overtake.gg/threads/assetto-corsa-server-list.114178/)). Le launcher No Hesi met en avant l'invitation d'amis et un état « prêt » partagé par le groupe ([nohesi.gg](https://nohesi.gg/get-started)) ; Rorzone vend un code de connexion à partager ([ror.zone](https://ror.zone/)) | Favoris, Récents, Amis, coller un lien |
| 2 | **Pouvoir entrer** : contenu manquant, mauvaise version, CSP, DLC | Sans le contenu, on ne rejoint pas ; les voitures manquantes s'affichent en rouge ([Steam](https://steamcommunity.com/app/244210/discussions/0/1742230617609308664)). Une voiture de version différente fait échouer le contrôle d'intégrité sur `data.acd` et le joueur est expulsé ([AssettoHosting](https://assettohosting.com/en/article/tutorial/fix-checksum-assetto-corsa)). Les serveurs Shutoko exigent le DLC japonais ([Steam](https://steamcommunity.com/app/244210/discussions/0/5312592706218870422)) ; No Hesi exige une version de CSP précise | Un statut de préparation lisible, et un bouton qui prépare tout |
| 3 | **Choisir une ambiance** : freeroam avec trafic, drift/touge, course, trackday | Les listes publiques sont dominées par Shutoko, LA Canyons et les serveurs drift ([gs4u](https://www.gs4u.net/en/assettocorsa/)). Une seule communauté drift revendique plus de 400 serveurs ([ACS Drift](https://acsdrift.com/)) | Filtre par catégorie de circuit |
| 4 | **Trouver du monde maintenant**, ou une course qui démarre | Courses publiques et ligues coexistent ; en ligue, on repère son serveur à son nom dans la liste de réservation ([ACRL](https://steamcommunity.com/groups/acrl/)) | Tri par joueurs, session en cours, temps restant |
| 5 | **Rouler sur ses combos habituels** | Tu l'as dit : les circuits joués sont souvent les mêmes | Autocomplétion circuit / voiture, suggestions tirées de ton historique |

La page s'organise donc autour des cas 1 et 2. Les cas 3 à 5 sont des filtres, pas la structure.

## Ce qu'un serveur AC expose

Un serveur AC donne trois niveaux d'information, du plus universel au plus riche. La liste doit tenir avec le niveau 1 seul ; le détail exploite les niveaux 2 et 3 quand ils existent.

| Niveau | Source | Disponible sur | Données utiles |
| --- | --- | --- | --- |
| 1 · Lobby / INFO | Lobby Kunos + endpoint HTTP `/INFO` du serveur | Tous les serveurs | Nom, IP/ports, pays, `clients`/`maxclients`, `track` (id + layout), `cars[]` (ids), mot de passe, session en cours, types et durées de sessions, temps restant, course au temps ou aux tours, arrêt au stand obligatoire, grille inversée, mode booking/pickup |
| 2 · Entry list | Endpoint `/JSON` | Tous les serveurs | Un slot par voiture : modèle, skin, pilote, équipe, nation, **connecté ou libre**. Permet de calculer les slots libres **par voiture** |
| 3 · Détails étendus | `/api/details` du wrapper CM, ou `EnableServerDetails` d'AssettoServer | Serveurs qui l'activent (fréquent sur les serveurs communautaires) | Description, météo actuelle, températures air/piste, vent, grip, assists autorisés (ABS, TC, ESC, embrayage auto, couvertures), usure pneus / carburant / dégâts, contacts max par km, **liens de téléchargement du contenu manquant**, checksum du mot de passe |

À noter :

- Les types de session sont codés : booking, practice, qualify, race. Le serveur indique la session active et le temps restant.
- Le **ping** n'est pas fourni par le serveur : PitBox doit le mesurer lui-même.
- AssettoServer (serveurs freeroam, trafic IA) exige une version minimale de **CSP** : c'est une info bloquante à afficher.
- Le checksum du mot de passe permet de **vérifier le mot de passe avant de lancer le jeu** : on évite le chargement de 40 s pour se faire refuser.

Sources : [champs /INFO (assetto-server-manager)](https://pkg.go.dev/github.com/cj123/assetto-server-manager), [format du wrapper CM](https://github.com/JustaPenguin/assetto-server-manager/blob/v1.7.8/content_manager_wrapper.go), [ac-server-wrapper](https://github.com/gro-ove/ac-server-wrapper), [AssettoServer — Server Details](https://assettoserver.org/docs/misc/server-details/).

## Liste des serveurs

Recommandation : une ligne large par serveur (pas une grille de cartes), avec **quatre blocs lisibles d'un coup d'œil** — circuit, session, remplissage, ton état de préparation. Le reste passe en icônes.

**Quatre onglets au-dessus de la liste : `ALL` · `FAVOURITES` · `RECENT` · `FRIENDS`.** Les trois derniers répondent au cas 1 sans aucun filtre, et ce sont eux qu'on ouvre le plus souvent. `RECENT` garde la voiture utilisée : rejoindre à l'identique est un clic. **Coller un lien de connexion** (lien de partage CM ou `acmanager://`) n'importe où sur la page ouvre directement le détail du serveur.

| Priorité | Info | Affichage recommandé |
| --- | --- | --- |
| 1 | **Circuit** | Vignette `preview.png` du circuit (ou son tracé `outline.png`) à gauche, nom + layout en gros. Le visuel reconnaissable remplace la lecture |
| 1 | **Joueurs** | `14 / 24` en mono, gros, + barre de remplissage. Couleur : vide = gris, actif = normal, plein = rouge |
| 1 | **Amis présents** | Pastille `★ Léo` (ou `★ 2`) sur la ligne dès qu'un joueur marqué ami est connecté |
| 1 | **Session en cours** | Badge `RACE` / `QUALI` / `PRACTICE` + temps restant (`12 min` ou `8 laps`). Une course en cours = tu attendras la prochaine session : c'est décisif |
| 1 | **État de préparation** (propre à PitBox) | `READY` · `1 CLICK` · `DOWNLOAD` · `BLOCKED`, détaillé plus bas (Contenu manquant) |
| 2 | Voitures | 3 noms courts max + `+12`. Les voitures que tu possèdes en premier |
| 2 | Nom du serveur | Titre de la ligne, tronqué proprement. Souvent bruité (« 24/7 \| discord.gg/... ») : on ne le laisse pas écraser le reste. Le lien Discord qu'il contient est extrait et rendu cliquable dans le détail |
| 2 | Ping | Mono, coloré par seuil (vert < 60 ms, orange < 120 ms, rouge au-delà) |
| 3 | Pays, mot de passe, CSP requis | Icônes seules : drapeau, cadenas, badge `CSP` |

À **ne pas** montrer dans la liste : IP, ports, durées des sessions, assists, météo. Ils vont dans le détail.

Tri par défaut : serveurs avec amis, puis joignables (`READY`, puis `1 CLICK`), puis joueurs connectés décroissants. Un serveur vide mais prêt reste utile pour s'entraîner seul, il ne doit pas disparaître.

## Détail d'un serveur

Le détail est un **panneau à droite** de la liste (la liste reste visible), organisé dans l'ordre où l'on décide : où, quand, avec quoi, puis rejoindre.

1. **En-tête circuit** : bannière du circuit en grand, nom + layout, drapeau, ping, `14 / 24`. Si le circuit manque : bannière grisée + action de téléchargement.
2. **Frise des sessions** : `PRACTICE 10 min → QUALI 10 min → RACE 15 laps`, la session active surlignée avec le temps restant. Pastilles pour `MANDATORY PIT` et `REVERSED GRID` si actifs.
3. **Choix de la voiture** (le cœur de l'écran) : une carte par modèle avec vignette, nom, et **slots libres** `2 FREE / 4`. Trois états visuels : disponible et installée · installée mais complète (grisée) · non installée (badge `MISSING` + action de préparation, voir « Contenu manquant »). Clic = sélection ; skin du slot affichée sous la carte.
4. **Pilotes connectés** : liste compacte nom + voiture + drapeau. Utile pour repérer des amis.
5. **Conditions** (si détails étendus) : une bande d'icônes — météo, air `22°C`, piste `31°C`, grip `98%`, vent.
6. **Règles** (si détails étendus) : assists autorisés, dégâts, usure pneus, conso, contacts max. Affichées comme **écarts** par rapport à la norme (« ABS forced off », « Damage 100% »), pas comme une liste de 12 valeurs.
7. **Description** du serveur, repliée par défaut (souvent longue, liens Discord).

En bas, collé : le bouton **JOIN** unique. Désactivé, il dit pourquoi : `PICK A CAR`, `TRACK MISSING`, `SERVER FULL`, `CSP REQUIRED`. Si mot de passe : champ inline, vérifié via checksum avant le lancement quand le serveur le permet.

Ce bouton suit le niveau de préparation du serveur pour la voiture choisie : `JOIN`, `PREPARE & JOIN` ou `GET CONTENT` (section suivante). Dans la liste des pilotes, un clic sur un nom le marque comme ami.

## Contenu manquant : prêt à rejoindre

C'est le vrai différenciateur. CM ne voit que `content/` ; PitBox sait, pour chaque voiture et circuit du serveur, si tu l'as, sous quelle forme, et comment l'obtenir. Il peut donc transformer la plupart des « contenu manquant » en un clic.

| Ce que PitBox trouve | Ce qu'il fait | Niveau |
| --- | --- | --- |
| Installé, actif | Rien | `READY` |
| Installé mais **désactivé** | L'active (hardlinks, instantané). CM le verrait comme manquant | `1 CLICK` |
| **En vitrine** (sans fichiers) | Le réhydrate : archive conservée, puis lien fourni par le serveur, puis registre CUP | `1 CLICK` si la source est directe, sinon `DOWNLOAD` |
| Absent, **lien direct** fourni par le serveur | Télécharge, puis passe par l'import PitBox (mod géré, activé) | `1 CLICK` |
| Absent, lien non direct (Mega, Drive, Discord) | Ouvre le navigateur ; l'archive glissée dans PitBox est reconnue et l'état se met à jour | `DOWNLOAD` |
| Absent, aucune source | Affiche le lien Discord extrait du nom ou de la description | `DOWNLOAD` |
| **Version différente** de celle du serveur (quand il la déclare) | Propose la mise à jour. Sinon, le contrôle d'intégrité échoue et le joueur est expulsé | `1 CLICK` ou `DOWNLOAD` |
| **Couche PitBox** qui remplace `data.acd` d'une voiture ou `surfaces.ini` du circuit | Rejoint sans elle et la réactive à la fermeture du jeu, annoncé avant le lancement (voir « Couches et versions ») | `1 CLICK` |
| Contenu d'un DLC Kunos non possédé | Nomme le DLC (table `kunos_content_dates`) | `BLOCKED` |
| CSP trop ancien pour le serveur | Le dit, renvoie vers CM | `BLOCKED` |

Le niveau d'un serveur est le pire niveau de ce qui est nécessaire. **Le bouton suit ce niveau** : `JOIN` quand tout est prêt ; `PREPARE & JOIN` quand tout se règle en un clic (actions enchaînées dans la pile de notifications, même progression que l'import, puis lancement via CM) ; `GET CONTENT` quand il faut passer par le navigateur, avec une ligne par téléchargement qui se coche à chaque archive importée.

**Le bouton « Install missing content » de CM est à contourner, pas à reproduire.** Il n'apparaît que si le serveur fournit des liens directs ([Emperor Servers](https://wiki.emperorservers.com/assetto-corsa-server-manager/share-content-with-content-manager-wrapper)), et il installe directement dans `content/` : le contenu arriverait en « non géré ». PitBox lit les mêmes liens et les fait passer par son propre import.

### Couches et versions

PitBox est le seul outil qui sait qu'une voiture ou un circuit n'est pas « tel que livré ». Le serveur contrôle `data.acd` et les `surfaces.ini` ([JustaPenguin, FAQ](https://github-wiki-see.page/m/JustaPenguin/assetto-server-manager/wiki/2%29-Troubleshooting-FAQ)) ; le reste peut passer ou non. D'où deux niveaux, selon que l'échec est certain ou seulement possible.

| Cas | Ce que fait PitBox | Ce que voit l'utilisateur |
| --- | --- | --- |
| **Échec certain** : une couche active remplace `data.acd` (ou `data/`) de la voiture choisie, ou un `surfaces.ini` du circuit | Désactive la couche pour la session, la réactive à la fermeture du jeu | Une ligne au-dessus du bouton : « La couche *X* sera désactivée pendant la session, puis réactivée ». Le bouton devient `PREPARE & JOIN`. Une case permet de refuser |
| **Échec possible** : une couche active touche autre chose sur la voiture ou le circuit (son, textures, extension CSP) | Rien par défaut | Encadré jaune « Couche active sur cette voiture » avec son nom, et l'option « Rejoindre sans les couches » |
| **Aucune couche** | Rien | Rien |

**La réactivation ne dépend pas d'une fermeture propre.** Le choix « désactivée pour la session » est écrit sur disque avant le lancement. La fermeture du jeu (signal de fin de session déjà utilisé par les replays) réactive la couche et le dit dans une notification. Si PitBox a été fermé entre-temps, la réactivation se fait au démarrage suivant, comme le filet de `gamebackup`.

**Versions à côté du choix.** Sous la voiture choisie et sous le circuit : la version installée et l'archive d'origine (`v1.4 · rss_911_v14.7z`). Quand le serveur déclare sa version (détails étendus), elle s'affiche à côté ; un écart passe en orange. Une mauvaise version installée est l'autre cause fréquente d'échec, et c'est ce qu'on vérifie en premier.

À vérifier : un mod son (`.bank` remplacé) fait-il échouer la connexion de façon reproductible sur certains serveurs ? Si oui, il passe dans « échec certain ».

## Filtres de base

Une barre unique, sur une seule ligne comme celle de la bibliothèque : un champ de recherche, quatre **puces oui/non épinglées**, le reste en tokens. Pas de panneau de 20 cases.

| Filtre | Forme | Défaut |
| --- | --- | --- |
| Nom du serveur | Champ texte, recherche instantanée | vide |
| Not full | Puce oui/non, épinglée | posée |
| No password | Puce oui/non, épinglée | fantôme |
| Not empty | Puce oui/non, épinglée | fantôme |
| Joinable (`READY` ou `1 CLICK`) | Puce oui/non, épinglée | fantôme |
| Circuit | Token avec autocomplétion sur les circuits en ligne, nombre de serveurs affiché (`Spa · 23 servers`) | aucun |
| Voiture | Même principe que le circuit | aucun |
| Catégorie de circuit | Token `track category: Freeroam`, tiré des catégories de circuit de ta bibliothèque | aucun |
| Contenu | Token `content: kunos` (rejoignable sans rien télécharger) ou `content: mods` | aucun |
| Ping max | Token `ping < 100` | aucun |
| Pays / région | Token avec drapeaux | aucun |
| Session en cours | Token `session: race` | aucun |

La catégorie de circuit répond au cas 3 (freeroam, drift, touge, circuit) avec un vocabulaire que PitBox tient déjà. Un circuit absent de ta bibliothèque n'a pas de catégorie : il compte comme **inconnu**, ni inclus ni exclu, sinon on masque en silence les serveurs qu'on cherche justement à découvrir.

Une puce oui/non a trois états, comme à la bibliothèque : absente (indifférent), positive, négative. Le négatif sert : « Avec mot de passe » retrouve le serveur privé de sa ligue, « Pas joignable » montre ce qu'il faudrait télécharger.

Les puces et le tri sont **mémorisés** entre deux ouvertures de la page.

## Au-delà de Content Manager

Ce que PitBox peut faire mieux que CM tient aux cas 1 et 2 : **préparer l'entrée** avec la bibliothèque (section suivante) et **retrouver ses serveurs et ses amis** sans chercher. Les filtres de bibliothèque (tags, marque, famille) sont retirés : aucun cas d'usage ne les demande.

| Idée | Cas | Lot |
| --- | --- | --- |
| Favoris et Récents (avec la voiture utilisée) | 1 | v1 |
| Amis : marquer un joueur depuis la liste des pilotes connectés, onglet `FRIENDS` | 1 | v1 — CM le fait déjà, c'est un minimum |
| Coller un lien de connexion | 1 | v1 |
| Préparer et rejoindre en un clic | 2 | v1 |
| Filtre catégorie de circuit | 3 | v1 |
| Regrouper par circuit (30 serveurs Shutoko = une ligne dépliable) | 3 | v2 |
| Lien bibliothèque → online : `Online: 4 servers now` sur la fiche d'un circuit | 5 | v2 |
| Prévenir quand un ami se connecte ou qu'un slot se libère | 1, 4 | v2 |
| Filtres de bibliothèque (tags, marque, famille, motorisation) | aucun | écarté |

## Maquette fil de fer

&#91;embedded content: Page Online · barre de filtres, liste, panneau de détail\]

La liste reste lisible sans ouvrir le détail : circuit, session, remplissage et état `READY` sont dans des colonnes fixes. Le panneau de droite ne sert qu'à choisir la voiture et rejoindre. Données fictives.

## Décisions et questions ouvertes

La v1 se construit autour des onglets Favoris / Récents / Amis, du statut de préparation et du bouton qui prépare tout. Les filtres restent légers.

| Sujet | Décision |
| --- | --- |
| Lancement | Via CM, par `acmanager://race/online?ip=…&port=…&httpPort=…&car=…&track=…` (lancement direct avec la voiture choisie), plus la réinjection de `[REMOTE] __FEATURES` dans `race.ini` sur AssettoServer. `race/online/join` ne sert qu'en repli (booking) : il rouvre la fiche de CM où la voiture se rechoisit. Voir `online-join-research.md` |
| Source de la liste | Lobby Kunos en direct, `/INFO` et `/JSON` interrogés à la demande. Miroir de CM en repli (voir plus bas) |
| Ping | Mesuré sur les lignes visibles, mis en cache quelques minutes |
| Freeroam, drift, etc. | Filtre par catégorie de circuit, depuis la bibliothèque. Un libellé « type de serveur » plus tard, seulement si une donnée fiable le permet |
| Rail | Entrée `Online` dédiée, juste sous `Session`, au-dessus du filet. La colonne de session est masquée sur Online : le panneau de détail joue son rôle (choix de la voiture, lancement) |

**Sur la source de la liste**, il y a deux choses distinctes, et aucune n'est une base de contenu. La liste officielle est le **lobby Kunos** : chaque serveur public s'y enregistre, et c'est lui que le jeu interroge. Content Manager tient en plus **son propre miroir en cache** de cette liste, hébergé par son auteur : plus rapide, et il a maintenu la liste visible quand le lobby Kunos est tombé ([OverTake](https://www.overtake.gg/threads/lobby-server-unavailable.272823/)). PitBox dépend déjà d'acstuff pour les mises à jour (CUP), donc s'en servir en repli ne change pas de principe. Son format d'accès reste à vérifier dans le code de CM.

- [ ] **Voitures nécessaires** : faut-il avoir toutes les voitures du serveur, ou seulement la sienne plus celles des joueurs connectés ? Ça change le calcul du niveau de préparation. À vérifier en jeu.
- [x] **Voiture dans le lien CM** : oui pour la voiture (`race/online&car=`, vérifié en jeu). Non pour la livrée : le serveur impose celle du slot libre, quelle que soit celle demandée. Voir `online-join-research.md`.
- [ ] **Mods son** : échec reproductible ou aléatoire ? Décide s'ils passent en « échec certain ».
- [x] **SPEC §7.2** : tableau des trois territoires et ordre du rail mis à jour (`Session`, `Online`, filet, Apps…).
- [x] **Rejoindre sans une couche** : décidé, voir « Couches et versions ».
- [x] **Amis** : comme CM, par nom affiché.

## Lots

**Lot 1 — lister et rejoindre (livré, à vérifier à l'écran).** L'entrée
`Online` du rail ; la liste du lobby Kunos (recherche, les quatre bascules,
mémorisées) triée joignables d'abord puis par joueurs ; un panneau de détail
interrogé sur le serveur lui-même (`/INFO`, `/JSON`) : voitures avec leurs
slots libres et le skin imposé, pilotes connectés ; le bouton Rejoindre, qui
active voiture et circuit au besoin puis passe par `race/online` et la
réinjection de `__FEATURES` (`online-join-research.md`). Un serveur en booking
ouvre la fiche de CM, qui seule sait réserver. « Joignable » y veut dire :
circuit et au moins une voiture jouables ici (installés, ou dans la
bibliothèque avec leurs fichiers) — les quatre niveaux de préparation viennent
plus tard.

**Lot 2 — noms et vignettes (livré).** La liste montre la photo du layout, son
nom et celui des voitures tels que la bibliothèque les connaît (correction de
l'utilisateur comprise), les voitures possédées en premier ; la recherche lit
aussi ces noms. Le panneau ouvre sur la bannière du circuit avec son tracé, et
chaque voiture porte la photo du skin que le serveur imposera. Ce que la
bibliothèque n'a pas garde son identifiant. Les voitures sans aucun slot
joueur (le trafic IA d'un AssettoServer) ne sont pas proposées.

**Lot 3 — Favoris et Récents (livré).** Trois onglets au-dessus de la liste :
`Tous`, `Favoris`, `Récents`, avec leur décompte ; l'onglet ouvert est
mémorisé. L'étoile du panneau met un serveur en favori ; chaque Rejoindre
réussi l'ajoute aux récents avec la voiture utilisée (vingt gardés, un par
serveur, le plus récent en tête), et rouvrir ce serveur présélectionne cette
voiture si elle est encore prenable — rejoindre à l'identique est un clic.
Les deux listes gardent une **copie** du serveur tel qu'il était : elles
s'affichent tout de suite, sans attendre le lobby, et survivent à sa panne ou
à un serveur qu'il ne liste plus ; l'entrée fraîche du lobby remplace la copie
dès qu'elle existe. Les quatre bascules ne s'appliquent qu'à `Tous` : un
serveur à soi reste listé plein, verrouillé ou vide, puisque c'est justement
là qu'on le cherche ; la recherche s'applique partout. Stockage :
`online.json` d'`app_config_dir`, écrit côté Rust et compris dans la
sauvegarde de démarrage.

**Lot 4 — Amis (livré).** Dans le panneau, un clic sur un pilote connecté le
marque comme ami (☆ → ★, en vert), un second clic le retire ; les noms se
comparent sans casse ni espaces autour, comme les écrit chaque jeu. Le lobby
ne nomme personne : après chaque chargement de la liste, et seulement si au
moins un ami est marqué, Pit Box demande leurs pilotes aux serveurs qui ont
des joueurs (32 requêtes à la fois — mesuré le 2026-10-03 : 341 serveurs
occupés sur 9 000, 4,5 s). Un serveur où roule un ami porte une pastille
`★ Léo` (ou `★ 3`), passe en tête de `Tous`, et forme l'onglet `Amis`. Les
amis vivent dans `online.json` avec les favoris.

**Lot 5 — niveaux de préparation, sans les couches ni les téléchargements
(livré).** Chaque circuit et chaque voiture est connu avec l'endroit où il est :
dans le jeu (`Prêt`), seulement en bibliothèque (`1 clic` — Rejoindre le pose
d'abord), en vitrine ou absent (`À télécharger`), ou contenu Kunos absent
(`Bloqué`, avec le nom du DLC) ; un serveur qui exige un CSP plus récent que
celui installé (ou CSP quand il n'y en a pas) est `Bloqué` lui aussi. La liste
montre le niveau de chaque serveur — le pire de son circuit, de sa meilleure
voiture et du CSP, puisque le lobby ne dit pas quelle voiture a une place —,
la cause au survol, et trie `Prêt` avant `1 clic` ; la bascule « Joignable »
garde ces deux-là. Dans le panneau, chaque voiture porte son propre niveau, on
peut en choisir une manquante pour savoir pourquoi on ne peut pas entrer, et le
bouton suit : `Rejoindre`, `Préparer et rejoindre`, ou la raison
(« Circuit manquant », « DLC requis : Red Pack », « CSP 3465 ou plus récent
requis »). Ce que le lot ne fait pas encore : télécharger, et écarter une
couche le temps d'une session — un serveur qui en aurait besoin se lit
`À télécharger` ou `Prêt`.

**Lot 6 — le détail du panneau (livré).** La **frise des sessions** sous les
faits du serveur : chaque session et sa durée lues comme CM les lit (secondes,
sauf une course aux tours ; « +1 tour » pour une course au temps qui en
ajoute un ; heures au-delà de deux), la session en cours encadrée avec le temps
qui reste, et `Arrêt obligatoire` / `Grille inversée` en pastilles. Sur les
serveurs qui publient `/api/details` (AssettoServer, le wrapper de CM — pas un
serveur Kunos nu) : une bande de **conditions** (météo telle que CSP la nomme,
air, piste, grip, vent), les **règles en écarts** à ce qu'un serveur AC fait par
défaut (ABS ou antipatinage interdit ou imposé, ESC autorisé, embrayage auto
interdit, couvertures, rétro virtuel, dégâts, consommation et usure s'ils ne
sont pas à 100 %, roues hors piste tolérées), la **description** repliée,
débarrassée de son BBCode. Les **liens** du nom et de la description (Discord
le plus souvent, `discord.gg/…` écrit sans `https` compris, images exclues)
s'ouvrent dans le navigateur. **Coller un lien de connexion** n'importe où sur
la page (lien de partage CM `acstuff.club/s/q:race/online/join?…`, forme
`acmanager://`) ouvre son serveur, même absent du lobby ; un mot de passe en
clair dans le lien remplit le champ — le mot de passe chiffré de CM ne se lit
pas ici. Le **ping** est mesuré par Pit Box (une connexion TCP au port HTTP du
serveur, soit un aller-retour) sur les lignes visibles une fois le défilement
arrêté, gardé cinq minutes : vert sous 60 ms, orange sous 120, gris au-delà —
pas rouge, réservé à la session (SPEC §7.2ter).

**Lot 7a — écarter les couches le temps d'une session (livré).** Le panneau
lit les couches actives du circuit et de la voiture choisie, et leurs fichiers :
une couche qui remplace `data.acd` ou un fichier de `data/` d'une voiture, ou
un `surfaces.ini` d'un circuit (le sien ou celui d'un layout), est un **échec
certain** ; une couche qui touche autre chose, un **échec possible**. Au-dessus
du bouton, une ligne annonce celles qui seront désactivées pendant la session
puis réactivées, avec « Garder ces couches » pour refuser ; un encadré jaune
nomme les autres, avec « Rejoindre sans ces couches ». Le bouton devient
`Préparer et rejoindre` dès qu'une couche est écartée. La liste est écrite dans
`online_layers.json` **avant** la moindre désactivation ; la fin du jeu (le
sondage du process qui sert déjà la musique) réactive les couches et le dit
dans une notification, et l'annonce de départ de ce même sondage fait le
rattrapage au démarrage suivant si Pit Box a été fermé entre-temps. Les couches
passent par l'interrupteur ordinaire (`compose::set_layer_active`) ; seules
celles de la voiture et du circuit rejoints sont acceptées.

**Lot 7b — récupérer le contenu manquant (livré).** Pour le circuit et chaque
voiture d'un serveur, le panneau cherche une source, dans l'ordre de la table
ci-dessus : l'**archive gardée à l'import** d'un mod en vitrine ; le **lien du
serveur** — le bloc `content` de `/api/details`, au format du wrapper de CM :
une `url` par voiture et pour le circuit, ou, sans clé `url`, le fichier servi
par le serveur lui-même (`/content/car/<id>`, `/content/track`) ; une `url`
vide ne vaut rien (mesuré : 404), `direct: false` non plus, et `cup: true`
renvoie au registre — ; enfin le **registre CUP**, quand il liste l'id sans le
réserver aux humains (`limited`). Une source change « À télécharger » en
`1 clic` ; une version installée plus ancienne que celle que le serveur déclare
aussi, quand une source permet la mise à jour — les deux versions s'affichent
côte à côte, en orange quand elles diffèrent. `Préparer et rejoindre` récupère
d'abord, une source après l'autre, par les chemins existants : l'archive
gardée comme la récupération d'un mod en vitrine, un lien ou le registre comme
une mise à jour de mod (même progression, même annulation, puis l'import
ordinaire : le contenu arrive **géré**, jamais posé tel quel dans `content/`).
Une page au lieu d'une archive s'ouvre dans le navigateur, le panneau le dit,
et l'archive glissée ensuite sur Pit Box fait relire le serveur. Mesuré le
2026-10-03 : 151 serveurs sur 254 interrogés publient des liens, souvent
directs (`.rar` d'une communauté, publications GitHub, Google Drive).
Le contenu servi par un serveur protégé par mot de passe n'est pas proposé :
il exige un hachage du mot de passe que Pit Box ne calcule pas.

**Lot 8 — les tokens de filtre (livré).** La barre est celle de la
bibliothèque, puces et popovers compris (`FilterBar`) : la recherche, `+ Filtre`
et les puces, avec `Circuit`, `Voiture` et `Catégorie de circuit` épinglés en
fantômes. Sept tokens : **circuit** (le layout, sous son nom ici, et le nombre
de serveurs qui le font tourner), **voiture** (`ET` / `OU` entre deux),
**catégorie de circuit** (celles de la bibliothèque — un circuit qu'elle n'a
pas est inconnu, ni inclus ni exclu ; un circuit qu'elle a sans catégorie, lui,
n'en a pas), **contenu** (`Kunos` : circuit et toutes les voitures officiels,
DLC compris, possédés ou non ; `Mods` : au moins un mod), **ping** (moins de
60, 120 ou 200 ms), **pays** (drapeau et nom dans la langue de l'utilisateur)
et **session en cours**. Chaque valeur s'inclut ou s'exclut comme à la
bibliothèque. Les tokens s'appliquent à tous les onglets, comme la recherche :
ce sont des critères posés, et la puce les montre partout. Un serveur dont le
ping n'est pas mesuré n'est pas « proche » : poser un token de ping fait
mesurer les serveurs qui passent tous les autres filtres, le haut de la liste
d'abord, par lots de 64, et ils apparaissent à mesure qu'ils répondent (« Mesure
du ping · N serveurs restants ») ; un serveur qui ne répond pas reste écarté.
Les tokens et les épingles sont mémorisés ; la recherche ne l'est toujours
pas. Le pays d'un serveur est un **drapeau** : seul sur la ligne,
son nom en infobulle ; suivi du nom dans le panneau. Un code que le jeu ne
connaît pas reste écrit tel quel.

**Lot 9 — les puces oui/non et le tableau (livré).** Les quatre cases
« Pas plein », « Sans mot de passe », « Pas vide » et « Joignable » sont des
puces de la barre, épinglées, à trois états ; « Pas plein » est posée d'usine,
« Sans mot de passe » ne l'est plus. Elles ne s'offrent que sur `Tous` : un
serveur à soi reste listé plein, verrouillé ou vide. Un état enregistré par les
anciennes cases se relit comme les puces équivalentes, et les quatre rejoignent
les épingles.

La liste est un **tableau**, celui de la bibliothèque (SPEC §7.4,
`components/ui/DataTable.svelte`) : colonnes choisies, déplacées par
glisser-déposer d'en-tête, redimensionnées, mémorisées côté Rust
(`library_columns.json`, clé `online`). Colonnes : **Circuit** (fixe, ni
masquable ni déplaçable), Serveur, Session, Joueurs, Ping, Voitures, CSP, Pays,
État ; IP:port et Mot de passe en option. Un clic d'en-tête trie croissant, un
deuxième décroissant, un troisième rend l'ordre par défaut (amis, joignables,
joueurs) ; ce qui n'a pas de valeur (ping non mesuré, pas de CSP) va toujours en
dernier. Le tableau est virtuel : seules les lignes visibles sont dessinées.

Sous le nom du circuit, le **nom lisible du layout** (« Main Layout », la
répétition du nom du circuit ôtée), l'identifiant en infobulle. L'**état** reste
discret quand tout est prêt ; la couleur va à `1 clic` (bleu), `À télécharger`
(orange) et `Bloqué` (jaune). La **session** dit le temps restant, et
« Ouvert » pour des essais de plus de 3 h. « Réservation » ne s'écrit qu'une
fois. Les voitures tronquées gardent leur `+N` en vue, la liste entière au
survol.

**Regrouper par circuit** est dans le menu d'affichage (le chevron à côté des
colonnes), sur `Tous`, mémorisé, éteint d'usine. La ligne d'un groupe est une
ligne du tableau, une cellule par colonne visible : Circuit (vignette et nom),
Serveur (le nombre de serveurs, et les amis présents), Joueurs (total / places) ;
les autres cellules restent vides. Les groupes viennent par total de joueurs,
le plus fréquenté en tête — le nombre de serveurs ne dit pas ce qui est joué.
Une colonne peut déclarer un **agrégat** (Circuit, Serveur, Joueurs) : trier
sur elle ordonne les groupes par l'agrégat et les serveurs de chaque groupe par
leur valeur ; trier sur une autre n'ordonne que les serveurs. Un clic déplie un
groupe, ses serveurs dessous, un filet à gauche et la cellule Circuit réduite au
layout. Un layout tenu par un seul serveur garde sa ligne ordinaire, rangée
comme un groupe d'un.

**v2 — « Online : N serveurs » sur la fiche d'un circuit (livré).** La
liste du lobby est **une seule**, en cache partagé entre la page Online et les
fiches, gardée trois minutes (`online/lobby.svelte.ts`) — moins si la
bibliothèque change entre-temps (un import, une activation, du contenu récupéré
pour rejoindre) : les niveaux de préparation de la liste ne valent que pour la
bibliothèque contre laquelle ils ont été jugés. Deux demandes en même temps n'en
font qu'une, et « Actualiser » passe outre le cache. La fiche d'un
circuit s'affiche sans l'attendre et, quand la liste est là, montre sous ses
données `Online · N serveurs · M joueurs` — compté en local, sur les seuls
serveurs qui ont au moins un joueur, tous layouts du circuit confondus, sans une
requête aux serveurs eux-mêmes. Un clic ouvre Online sur `Tous` avec le jeton
Circuit posé sur ces layouts. Lobby injoignable ou personne en piste : rien ne
s'affiche, pas d'erreur.

**v2 — prévenir : ami connecté, place libérée (livré).** Une surveillance
démarrée avec la coquille (`online/watch.svelte.ts`), qui tourne quel que soit
l'écran ouvert. Elle **ne regarde que ce que l'utilisateur a désigné**, jamais
les 9 000 serveurs — ce serait abusif pour eux, et certains routeurs bloquent
une machine qui contacte des milliers d'adresses :

- **amis** : le `/JSON` des seuls serveurs en favoris et en récents, une requête
  chacun toutes les 2 minutes. Le premier tour, 15 s après le démarrage, ne fait
  que relever qui est déjà là : un ami en ligne au lancement de Pit Box ne vient
  pas de se connecter. Un serveur qui ne répond pas à un tour garde ce qu'il
  disait au précédent : rien n'est annoncé sur lui tant qu'il n'a pas répondu
  deux fois, sans quoi chaque raté de réseau ramènerait ses amis comme
  « en ligne ». Un ami sur un serveur inconnu n'est pas vu : limite assumée ;
- **place libre** : seulement sur le serveur où l'utilisateur a cliqué « Me
  prévenir » — proposé dans le panneau quand le serveur est plein ou que la
  voiture choisie n'a plus de place —, toutes les 30 s, par le seul `/JSON`
  (`online_slot_counts`). La place attendue est celle de la voiture choisie
  quand c'est elle qui manque de place, sinon n'importe laquelle. La
  surveillance s'arrête quand la place se libère, au bout d'une heure, ou au
  lancement du jeu ; un serveur à la fois.

Elle se met en pause pendant une session (`ac://running`) et repart de zéro
après : ce qu'elle savait avant ne dit rien de qui est arrivé pendant. Une
alerte entre dans la pile de notifications de l'app, avec « Ouvrir », qui mène
au panneau du serveur sur la page Online ; quand la fenêtre n'a pas le focus,
**Windows la montre aussi** (`tauri-plugin-notification`), et la pile la garde
pour le retour. Réglages › Général la coupe entière, « Me prévenir » compris.

**Restent** : les niveaux de préparation de la liste elle-même, qui ne
voient ni les liens ni les versions des serveurs (il faudrait `/api/details`
serveur par serveur).
