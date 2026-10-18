# Pit Box — Spécification de référence

> Gestionnaire de mods desktop pour Assetto Corsa. Remplace Mod Organizer 2, utilise Content Manager (CM) comme moteur de lancement.
> Application Tauri (frontend web + backend Rust), base SQLite. Thème Rosso Corsa.
> Ce document décrit l'application telle qu'elle fonctionne, par domaine : le comportement et les règles qu'il faut connaître pour la modifier sans la casser. Les cas précis vivent dans les tests, le comment dans les commentaires du code, l'historique et le reste à faire dans `CHANTIERS.md`.

---

## 1. Principe directeur

Le cœur de l'app n'est pas « activer des mods » mais **résoudre l'identité d'un mod** (nouveau / mise à jour / doublon), indépendamment du nom de dossier. Tout le reste en découle.

L'app prend en charge tout le cycle de vie : importer (analyse, détection de type, rangement), identifier, organiser (tags harmonisés, recherche, filtres), activer/désactiver sans duplication (~300 Go de mods), lancer une session sans subir l'UI de CM, maintenir (mises à jour, historique, export, nettoyage).

---

## 2. Architecture

**Quatre couches** :
1. **Bibliothèque** — source de vérité des fichiers, sur un disque dédié. Tous les mods rangés proprement.
2. **Application** — import, identité, tags, activation, lancement.
3. **`content/` d'Assetto Corsa** — une **projection** de la bibliothèque, jamais l'original (`content/cars/<id>`, `content/tracks/<id>`).
4. **Content Manager** — moteur conservé (graphismes CSP/Sol/Pure, FFB, presets), invoqué par l'app pour lancer une session. On contourne son UI, pas son moteur.

**Décisions structurantes** :
- **Activation par hardlinks, un par fichier**, dans une vraie arborescence de dossiers (`deploy.rs`) : zéro duplication, instantané, sans droits admin. Les junctions de dossier ont été écartées parce qu'AC/CSP les traverse mal sur un circuit, et les symlinks exigent le mode développeur. Un hardlink n'est pas un point d'analyse : le jeu ne le distingue pas d'un fichier ordinaire.
  - **Même volume obligatoire** : si bibliothèque et jeu sont sur deux disques, un fichier dont le hardlink échoue est copié. C'est un prérequis vérifié par l'écran de réglages (`config::validate`), traité comme bloquant : la copie double l'espace et se refait à chaque activation.
  - **Un marqueur caché (`.pitbox-deployed.json`)** à la racine de chaque dossier déployé est la seule preuve qu'il a été créé par l'app : une arborescence de hardlinks est indiscernable d'un dossier Kunos par ses attributs.
  - **Composition par couches (§4.3)**, en hardlinks elle aussi : à chaque chemin, un lien vers le fichier gagnant (base ou couche de plus haute priorité). `content/<type>s/<id>` **est** le résultat composé, sans dossier intermédiaire.
  - **Mode de déploiement au choix** (`deploy_mode`, §11) : hardlinks (défaut) ou symlink (`mklink /D`, qui exige le mode développeur de Windows — un lien ouvre `ms-settings:developers`). **Un mod à couche(s) active(s) reste toujours en hardlinks** : un lien ne pointe que vers une cible, il ne sait pas fusionner (`activation::deploy_base`). Un mod déployé par l'ancien mécanisme `mklink /D` reste valable, et passe aux hardlinks à sa prochaine activation — jamais de migration forcée.
- **Stack Tauri**, **SQLite** dans `app_data_dir`.
- **Chemins de bibliothèque stockés relatifs à la racine de la bibliothèque** (`libpath.rs`), jamais en absolu : déplacer la bibliothèque sur un autre disque ou une autre machine demande seulement de changer `library_path` dans les Réglages. Une ligne écrite en absolu par une version antérieure reste lue telle quelle (`libpath::resolve`).

---

## 3. Modèle de données : overlay non destructif

**Le fichier `ui_car.json` / `ui_track.json` d'un mod n'est JAMAIS modifié.** Règle absolue : réécrire le travail d'un moddeur casse les signatures d'intégrité et rend les modifications indissociables du mod.

**Deux sources de vérité séparées** :
- La **bibliothèque** = les *fichiers* (contenu des mods, lecture seule).
- La **base d'overlay** (SQLite) = les *métadonnées produites par l'app* : tags ajoutés/déduits, catégorie, année, specs complémentaires, favori, historique, profils, presets. Indexée sur l'empreinte du mod.

Le fichier du mod est une **entrée** du pipeline, jamais une **sortie**. Désinstaller l'app laisse les mods intacts.

**Entités** : Mod (identité stable, indépendante du nom de dossier), Tag (issu de l'ontologie), Profile (ensemble nommé de mods activés), HistoryEntry (événement horodaté), plus les sous-éléments et couches décrits plus bas.

**Historique d'un mod** : les événements avec le nom de l'archive source — « import initial », « mise à jour », « extension ajoutée ». **Pas les activations/désactivations** (du bruit). Le contenu de base Kunos (`is_stock`) n'a pas de frise : la fiche affiche « Contenu de base ».

---

## 4. Identité et import

### 4.1 Empreinte et résolution

Chaque mod a une **empreinte composite** stable. À l'import, l'app la compare à la bibliothèque :
- **Même identité** → mise à jour (voir §4.3 pour la distinction mise à jour / couche).
- **Match flou** (marque et nom proches, dossier différent) → demande explicite, trois réponses : **garder les deux**, **remplacer l'ancien**, ou **l'ajouter en couche à l'ancien** — le filet du §4.3bis quand la détection a manqué un contenu conçu pour être posé dans le dossier de l'autre. Le mod tout juste importé sort alors du jeu, ses fichiers deviennent une couche de l'ancien (dont la base n'est pas touchée), ses ressources rejoignent celles de l'hôte, et son entrée disparaît. Seul un mod sans passé s'y prête — une seule version, aucune couche, aucun sous-élément rattaché, aucun ajout hors de son dossier — ; un autre est refusé, avec la raison. L'import en masse garde ses deux réponses.
- **Aucun match** → nouvel import.

Avant cela, une question qui porte sur la nature : **ce dossier tient-il debout seul ?** Sans géométrie, il ne peut pas être un mod, quel que soit son nom (§4.3bis).

### 4.2 Sources d'import

Deux sources, même pipeline :
- **Archive** (`.zip`/`.rar`/`.7z`) — décompression puis analyse.
- **Dossier déjà décompressé** — analyse directe (migrer un catalogue MO2 sans re-zipper).

**Le titre du rapport dit ce qui est entré, par nature** — « 1 voiture, 10 pilotes importés », pas « 1 élément importé ». Un mod « autre » est nommé par la zone du jeu qu'il touche.

**Import en masse** : un dossier parent dont chaque sous-dossier direct est un mod, scanné sur un seul niveau. Une phase d'analyse sans rien écrire (récapitulatif : nouveaux / mises à jour / doublons / ambigus / compléments / autres mods / ignorés), puis l'arbitrage groupé des exceptions, puis l'exécution.
- **« Ignoré » veut dire « rien à importer »** : un sous-dossier sans structure reconnue mais non vide (un mannequin nu, une notice, des images) part en « autre mod », exactement comme importé seul (§7.3), et le récapitulatif le compte (badge « Autre mod », compris dans le nombre du bouton). Seul un dossier vide est ignoré.
- **Les sons, packs de skins et apps** d'un sous-dossier sont rangés comme par un import isolé (§8.3, §8.4), sans question, et annoncés dès l'analyse comme « compléments » avec leur cible.
- **Lancée, l'exécution quitte le dialogue** et se suit dans la notification de progression, avec son bouton d'arrêt (§4.2bis).

**Copier / déplacer** : un réglage par défaut mémorisé. Déplacement **adaptatif** : même disque → renommage instantané ; disques différents → copie, puis suppression après vérification.

**Interface** : glisser-déposer partout, archives **et** dossiers (le tri est fait côté backend, `split_dropped_paths`) ; un écran d'import dédié pour les options. **Un mod importé est activé par défaut** — de même une app et un « autre mod » (§7.3). **Un seul lot tourne à la fois.**

### 4.2bis Progression, estimation et annulation d'un lot

Pensé pour un lot de plusieurs dizaines de mods.

- **Deux barres, cohérentes par construction** : celle du haut suit l'item, celle du bas le lot (masquée pour un lot d'un item). La barre du lot se compte en **secondes estimées**, recalculée à partir des avancements par item, donc elle ne peut ni contredire ni doubler celle de l'item. Un item en erreur ou ignoré est consommé quand même.
- **Estimation du temps restant** : un benchmark persistant (`import_bench.json`) mesure le débit de la machine par seau (extraction, rangement d'archive, copie, déplacement de dossier), amorti à chaque mesure. Il ne fixe que les **poids relatifs** ; l'échelle est recalibrée sur le temps écoulé du lot. Le temps restant s'affiche en unités grossières, lissé.
- **La progression est réelle** : pendant l'extraction (sortie de 7-Zip lue au fil de l'eau ; un binaire trop ancien extrait sans progression, jamais en échec), pendant le rangement d'un mod (octet par octet ; un renommage sur le même volume se referme d'un coup), et pendant la fin d'un item — skins et sons rattachés signalés un par un, puis apps, puis balayage des restes. Une archive imbriquée annonce son nom, sans pourcentage inventé.
- **Contrôle d'espace disque** : un lot plus gros que l'espace libre du volume de la bibliothèque est refusé **avant** d'écrire quoi que ce soit — jamais bloquant sur une information absente.
- **Annulation** constatée **entre deux items** (7-Zip est tué s'il décompresse), jamais au milieu du rangement d'un mod. Le rapport affiche ce qui a été importé avant l'arrêt.
- **Le rapport de fin est cliquable** : chaque contenu ouvre sa fiche, une couche ouvre son contenu de base, skins et sons sont regroupés par parent, apps et « autres mods » ouvrent leur écran ; un mod resté ambigu n'est pas cliquable. Ouvrir une fiche **replie** le rapport, qui se redéploie d'un clic sur son bandeau ; seul le `✕` le ferme. Il reste consultable sur l'écran Import jusqu'au redémarrage (en mémoire seulement).

**La pile de notifications** (`ToastStack.svelte`) : progression, rapports d'import et « nouveau périphérique » (§7.4) partagent une seule colonne en bas à droite. Les rapports **s'empilent**, le plus récent déplié près du coin, les précédents réduits à leur bandeau ; au-delà de trois, les plus anciens sortent. Les arbitrages qui attendent une réponse (§4.3, §4.4) restent des modales. **Une écriture de réglage qui échoue s'y voit** : une alerte jaune quand `ui_prefs.json` ne peut pas être écrit, qui ne se referme pas d'elle-même — ce n'est pas une information de passage mais une perte de données en cours.

**Un lot se pipeline et garde la base libre** : l'archive suivante se décompresse pendant que la précédente se range (au plus deux dossiers temporaires vivants), et extraction comme copie de l'archive source se font hors du verrou de la base ; seul le rangement d'un mod le garde, parce qu'il entrelace décisions et écritures.

### 4.3 Mise à jour vs couche (recomposition)

**Pas d'historique de versions conservé** (choix assumé pour la place disque) : une mise à jour remplace. Le filet de sécurité est le **modèle de couches** — la base reste toujours intacte.

**Détection à l'import sur un contenu existant**, en comparant les fichiers :
- fort chevauchement des fichiers existants → **mise à jour** (remplace) ;
- majorité de chemins nouveaux, peu de fichiers écrasés → **couche / extension** (ajoute) ;
- détection **automatique**, question **seulement si ambigu**, avec récapitulatif chiffré (« ajoute 84 fichiers, en écrase 6 sur 412 »).

**Reprise après arbitrage** : trancher un cas ambigu rejoue l'import de **ce mod-là seulement**, depuis sa seule source, re-décompressée — aucun dossier temporaire ne survit à l'import en attendant une réponse. Ce qui suit les mods (skins/sons, apps, restes) n'est pas rejoué.

**Une couche a une identité** : son parent et l'archive dont elle vient. **Réimporter la même archive remplace la couche qu'elle avait posée** (priorité reprise), au lieu d'en empiler une seconde ; l'issue rapportée est une mise à jour. Une archive différente sur le même parent fait une seconde couche.

**Règle absolue** : le **contenu de base** (Kunos, `is_stock`) ne reçoit **jamais** de mise à jour, **toujours** une couche — même une « version améliorée complète » d'un circuit Kunos.

**Un mod installé hors Pit Box (`is_unmanaged`, §8.2) ne reçoit rien du tout**, ni mise à jour ni couche : l'import s'arrête sur cet id, **rien n'est écrit**, et le rapport le dit (issue `UNMANAGED`, avec le décompte calculé en lecture seule). Poser une couche sauvegarderait puis effacerait un vrai dossier que l'app n'a pas mis là. Le garde-fou vit dans `layers::store_layer`, par où passent tous les chemins qui posent une couche.

**Modèle de couches recomposables** :
- La **base** (Kunos ou mod) reste une entité intacte, jamais fusionnée.
- Une **couche** (nouveau layout, améliorations, surcharge) est une entité séparée, intacte, rattachée à sa base. Elle peut se poser sur n'importe quel contenu.
- Ce que le jeu voit dans `content/` est un **résultat composé** : base + couches actives dans l'ordre de priorité, en hardlinks (§2). Sans couche, c'est un déploiement simple.
- Désactiver ou réordonner une couche = **recomposer** depuis les entités intactes, jamais défaire chirurgicalement.
- L'ordre des couches se change, et chaque couche s'active ou se désactive.

**Ce que l'app affiche est le résultat composé, pas la version de base** (`library::entity_dirs`) : photos, tracés, layouts, `ui_*.json`, extensions CSP se lisent à travers une **pile** — couches actives par priorité décroissante, puis la base — **fichier par fichier**, comme `deploy::compose_tree` pose dans `content/`. Les layouts et les extensions CSP sont l'**union** des deux. La pile vaut que le mod soit actif ou non : la fiche d'un mod désactivé dit la même chose que celle du même mod activé. Une couche inactive est exclue de la pile. « Ouvrir le dossier » reste l'exception : il ouvre la version de base.

### 4.3bis Fragments : une couche déguisée en mod

Un dossier peut avoir **la forme** d'un mod sans en être un : un dossier conçu pour être posé sur un mod existant porte le même `ui/`. Ce qui les sépare, c'est la **géométrie** — une voiture a son `.kn5` à la racine, un circuit un `.kn5` ou un `models*.ini`. **Un dossier sans géométrie ne peut pas être chargé par le jeu.** Mesuré sur tout le corpus de référence sans une exception, le critère ne déclasse jamais un vrai mod : c'est ce qui permet d'**agir** dessus.

**La géométrie est nécessaire, pas suffisante.** Un circuit qui livre ses `.kn5` mais en charge l'essentiel chez un autre (ses `models*.ini` nomment des `.kn5` qu'il n'a pas) est une couche de **l'hôte qui possède tous les `.kn5` manquants**, s'il est seul à les posséder **et** qu'il en manque **au moins trois** — un ou deux `.kn5` nommés mais absents sont courants dans des circuits complets. Un dossier qui porte déjà le nom d'un mod connu reste à l'arbitrage du §4.3.

**Trouver l'hôte** (`fragment.rs`), de la source la plus sûre à la plus faible :

1. le dossier porte déjà le nom d'un mod connu ;
2. le `.vao-patch` nomme le `.kn5` ou le `models*.ini` qu'il accompagne ;
3. les noms de layout (circuit) ou de livrée (voiture) partagés avec un seul hôte — un nom générique (`reverse`, `short`…) partagé ne compte pas : deux concordances sur le même hôte tranchent, une seule non ;
4. un id d'hôte écrit dans le nom du dossier ;
5. le recouvrement de chemins, avec un vrai écart (au moins 2 concordances et le double du suivant).

**Deux candidats à égalité ne sont jamais départagés.**

**Le dossier d'extraction ne devient jamais une identité** (`importer::incoming_name`) : une archive dont le contenu est à la racine prend le nom de l'**archive**, sans extension — jamais celui du dossier temporaire, qui change à chaque extraction.

**Ce qui en découle** :

- **Hôte trouvé** → rangé en **couche**, automatiquement, sans rien demander. La décision est **visible** : la ligne du rapport nomme le dossier et l'hôte, la couche apparaît dans « Couches & extensions » sous le nom de l'archive qui l'a apportée (celui de son dossier seulement sans archive), et l'historique de l'hôte porte sa ligne.
- **Un fragment n'est jamais une mise à jour**, quel que soit le décompte de fichiers.
- **Hôte nommé mais absent** → **rien n'est écrit**, on demande, défaut **ne pas importer**. « Garder pour plus tard » range la couche sous l'id attendu sans rien poser dans le jeu ; l'hôte la reprend le jour où il arrive.
- **Hôte introuvable** → on demande, avec « ne pas importer » (défaut) et « importer quand même », signalé dans le rapport.
- **En import de masse**, où l'on ne demande jamais (§4.2bis), le défaut sûr est de **garder** : couche en attente si l'hôte est nommé, import tel quel sinon.

**Un skin ou un son dont la voiture manque suit la même règle** (§8.3). La reprise est sûre : la clé `<parent_id>/<nom>` ne dépend pas de l'extraction, `sub_exists` empêche de dupliquer un sous-élément connu, et **rien n'est posé dans le jeu pour un hôte absent** — aucun dossier n'y est créé. Le rapport et la fenêtre d'arbitrage le disent dans la langue de l'utilisateur (`parent_known`, `awaiting_decision`).

**Ce qui attend un hôte est listé sur l'écran Maintenance** (§10, `waiting_layers`) : ce n'est pas une anomalie, donc pas dans le « rien à signaler », mais sans cette liste une couche en attente serait invisible. On peut y renoncer d'un bouton. Les apps comptent dans le test d'existence.

### 4.4 Packs multi-voitures

Chaque voiture d'un pack est une **entité de premier niveau** (activable, tagguable séparément), liée aux autres par `source_pack` (le nom d'archive ou de dossier).

**Le pack possède les ressources et les ajouts au jeu** livrés autour de plusieurs mods (`extras::OwnerKind::Pack`, §4.5.3) :

- ses **ressources** apparaissent dans la carte Ressources de **chaque** membre, marquées « du pack » — une seule copie sur disque ;
- ses **ajouts au jeu** sont posés dès qu'au moins un membre est actif, retirés quand aucun ne l'est (`extras::sync_pack`, après chaque activation, désactivation ou suppression). Le dernier membre supprimé emporte l'arbre du pack, ressources comprises.

La fiche d'un membre porte un bloc « Source / origine » (pack cliquable, nom d'archive) et les autres voitures du même pack ; on peut filtrer par pack et désinstaller le pack en lot. Pour le contenu de base, la provenance dit **« Jeu de base »** ou le nom du DLC (`kunos_dates::pack_name`).

**Le pack a sa fiche** (`PackDetail.svelte`), ouverte depuis le bloc « Source / origine » et posée par-dessus la fiche du mod : les **mods livrés**, les **ajouts au jeu** du pack, ses **ressources**, les tailles, la date d'entrée, la désinstallation en lot. Un pack sans aucun membre n'a pas de fiche (`errors.packNotFound`).

### 4.5 Ce qu'un mod pose, et où

> **`SPEC-import.md`** condense ce qui suit en un arbre de décision et une table de destinations. C'est là qu'on voit les règles *ensemble* ; ici, chacune avec son pourquoi.

Une archive contient **le dossier du mod** — celui que l'auteur a conçu pour être posé dans `content/` (`rss_gtm_lanzo_v8/`, `ks_nordschleife/`) — et, autour de lui, le reste : notices, templates, configs CSP, shaders, textures d'équipe, modèle de pilote.

Trois destinations, une seule question : **le fichier appartient-il au dossier du mod ?**

| Où il est | Ce que c'est | Où il va |
| --- | --- | --- |
| **Dans** le dossier du mod | contenu du mod, à quelque profondeur que ce soit | bibliothèque, intégralement — jamais trié (§4.5.1) |
| **À côté**, non lu par AC | annexe : notice, template, changelog | `resources/` du mod, en bibliothèque (§4.5.2) |
| **À côté**, lu par AC ailleurs que dans `content/<type>/<id>` | ajout au jeu | `extras/` en bibliothèque, posé dans AC à l'activation (§4.5.3) |

#### 4.5.1 Le dossier du mod est intouchable

**Rien n'est jamais retiré de l'intérieur du dossier du mod.** Le critère est l'**appartenance au mod**, jamais l'extension ni la profondeur : `body_shadow.png`, `tyre_*_shadow.png` et `logo.png` sont de vrais assets AC à la racine d'un dossier voiture. `scripts/audit-resources.ps1` audite et répare l'existant.

Le tri ne porte que sur ce qui **entoure** le dossier du mod. Une annexe repérée *dedans* est **signalée, jamais déplacée**.

#### 4.5.2 Annexes → `resources/` en bibliothèque

Les fichiers hors contenu de jeu **à côté** du dossier du mod (PDF, templates `.psd`, readme) vont dans `resources/` du mod, en bibliothèque — jamais dans `content/`.

**Réglage « Extraction des fichiers annexes »** (§11), trois positions :

- **Aucun** — rien n'est extrait ni copié.
- **Informations seulement** (défaut) — les fichiers légers d'information : `.txt`, `.pdf`, `.md`, `.doc`/`.docx`, `.rtf`, `.nfo`, `.html`, `.url`, `.lnk`, `.jsgme` (le descripteur de variante de JSGME, jamais lu par AC — un document, et souvent la seule explication d'un dossier optionnel).
- **Tout** — plus les fichiers lourds : templates (`.psd`, `.xcf`, `.ai`), archives jointes, sources 3D (`.fbx`, `.blend`, `.3dsmax`), vidéos.

**Les images ne sont jamais des annexes**, à aucune profondeur : rien ne distingue une capture de présentation d'un asset AC. Dans le doute, on laisse.

**Une annexe restée dans le mod est listée, jamais déplacée** : la carte Ressources montre aussi les documents d'information de la **racine** du dossier du mod (mêmes extensions, `GUIDs.txt` exclu), marqués « dans le mod ».

**Les images des ressources passent en galerie** (vignettes, et la visionneuse plein écran de l'app, §6.1) ; le reste garde sa liste dessous.

**Prévisualisation dans la carte Ressources** : un clic sur une annexe lisible l'ouvre **sous la liste**, dans la fiche — texte brut (`.txt`, `.nfo`, `.log`, `.ini`, `.cfg`, `.csv`, `.json`, `.yml`, `.lua`), markdown, **PDF**. Les autres formats s'ouvrent dans l'application par défaut de Windows, aussi offerte d'un bouton `↗`. Au-delà de 32 Mio, la prévisualisation renvoie à cette ouverture externe.
- Le document n'a **ni hauteur imposée ni défilement propre** : c'est la page de la fiche qui défile. Le PDF est donc rendu par **pdf.js**, page par page, jamais par la visionneuse d'Edge dans un cadre.
- **Le PDF a son zoom** : **largeur de page** (défaut), **page entière**, et un zoom libre (− / +, pourcentage cliquable qui ramène à 100 % = taille réelle, Ctrl+molette). Le réglage est **durable et global** (`ui_prefs.json`). **Un changement d'échelle ne touche pas au défilement.** Au-delà de la largeur de la colonne, le document défile horizontalement.
- Une page n'est dessinée qu'en approchant de la zone visible, et relâchée au-delà d'un budget : la mémoire suit ce qui est à l'écran.
- **Sûreté** : le chemin est résolu et validé côté backend (anti-traversée) avant toute lecture, le contenu remonte par une commande Tauri, le markdown est **échappé avant** tout rendu, et les liens d'un readme s'ouvrent dans le navigateur du système.

#### 4.5.3 Ajouts au jeu → `extras/` en bibliothèque, posés dans AC

Ce qu'AC lit ailleurs que dans `content/<type>/<id>` : configs CSP, shaders, textures d'équipe, modèle de pilote. **Stockés bruts, avec leur chemin relatif à la racine d'AC**, dans `<lib>/extras/<type>/<id>/…`, au niveau du mod comme `resources/` : une mise à jour remplace ses propres fichiers, les couches partagent l'arbre.

**Le chemin d'archive n'est pas toujours un chemin de jeu** (`acpath.rs`) :

- **Dossier de jeu livré à nu** : un `driver/` contenant un `.kn5` est le `content/driver/` d'AC, préfixé avant tout usage. **Une seule règle de ce type**, volontairement : `weather/` et `sfx/` existent sous `content/` et sous `extension/`, on ne devine pas.
- **Emballage unique à la racine** : un dossier qui enveloppe toute la livraison (`NFS_TOURNAMENT…/content/…`) est traversé (`acpath::effective_root`), avec trois garde-fous — jamais un dossier accompagné d'autres entrées (c'est un choix de l'auteur), jamais un dossier de jeu (`content/` seul *est* la racine), jamais un mod reconnu (règle d'or n°3).
- **Emballage accompagné : la racine de jeu se déduit des mods trouvés.** Un mod à `<X>/content/cars/<id>` établit que `<X>` est la racine (`acpath::game_root`) ; à défaut d'une déduction sûre (aucun mod sous un `content/`, ou plusieurs qui ne s'accordent pas), on retombe sur `effective_root`. **Le balayage tient deux racines** : celle qu'on parcourt pour ne rien laisser derrière, et celle à laquelle les chemins sont relatifs pour AC. Un reste hors de la racine de jeu n'est pas posable. Le test d'annexe vaut aux deux.
- **Dossier d'emballage de l'auteur** (`Ferrari F2002 V1.4/`, `Optional - No ambient sounds/`) : un reste dont le premier segment n'est pas un dossier lu par AC — `content`, `system`, `extension`, `apps`, `cfg`, `launcher`, `sdk`, `server`, `plugins` — **n'est jamais posé**. `mods/` n'y est pas : c'est le dossier de stockage de JSGME, qu'AC ne lit pas. Un fichier isolé à la racine est refusé aussi (le `dwrite.dll` de CSP n'est pas posé par un gestionnaire de mods).

**Refuser ne jette rien** : le fichier reste en bibliothèque, **dans les ressources du mod** avec son chemin d'archive, et le journal d'import le dit (`pathRefused`, §4.6). Les entrées importées avant cette règle gardent leur marqueur « hors chemin de jeu ».

**Certains chemins de jeu appartiennent à Content Manager** : `extension/config/tracks/loaded/`, `extension/config/cars/loaded/` et `extension/vao-patches*/`, que son téléchargeur de configs synchronise. L'app **pose quand même** (arbitrage par date, §4.5.4), mais le **dit** (§4.5.5). La liste est dans `acpath.rs`.

Deux propriétés :

- **L'import ne jette rien que l'utilisateur n'ait pas écarté.** Ce qui n'est pas classé est conservé tel quel, donc l'interprétation reste recalculable depuis la bibliothèque à tout moment. Seule exception : un dossier proposé que l'utilisateur a écarté (§4.6ter), supprimé et journalisé (`userDiscarded`).
- **L'ajout vit et meurt avec son mod** : posé à l'activation, retiré à la désactivation, supprimé avec lui.

**Les archives imbriquées passent avant leurs voisins** : ce qui en sort entre dans les propriétaires possibles avant que les fichiers autour soient arbitrés (une livraison `readme.txt` + `Car.zip` range la notice chez la voiture). **Rien de reconnu à la racine ne veut pas dire rien du tout** : tant qu'une archive imbriquée traîne à n'importe quelle profondeur, on descend dedans avant de conclure.

**Rattachement** d'un reste (§7.3), dans cet ordre : le chemin contient l'id d'exactement un mod reconnu de l'archive ; sinon l'archive ne livre qu'un mod, et tout ce qui l'entoure lui appartient ; **sinon, c'est le pack** (§4.4), propriétaire de dernier recours. **Les apps sont des propriétaires** au même titre que voitures et circuits (`extras::OwnerKind`) ; les packs de skins et de sons ne le sont pas (leur parent n'est pas forcément dans l'archive). Un **document isolé** à la racine reste une annexe (§4.5.2), jamais un ajout au jeu.

**Pose fichier par fichier** (hardlink), jamais par jonction de dossier : plusieurs mods visent les mêmes arbres. **`content/fonts` et `content/driver` sont des ajouts au jeu comme les autres.**

#### 4.5.4 Poser sans écraser : réclamation, date, sauvegarde

Poser un fichier dans AC pose deux questions : **plusieurs mods peuvent viser le même chemin**, et **ce chemin peut déjà être occupé** (Kunos, mod installé à la main, autre mod de la bibliothèque). Ces règles valent pour les **deux** mécanismes de pose : les ajouts au jeu (§4.5.3) et les mods « autres » (§7.3).

**1. Compteur de références.** Chaque mod *réclame* les chemins d'AC dont il a besoin (`extra_links`). **Un fichier n'est retiré d'AC que lorsque plus aucun mod ne le réclame.**

**2. Arbitrage par date.** L'exemplaire **le plus récent** gagne ; à égalité, le **dernier mod installé**. La date traverse la chaîne intacte (7-Zip, copie, hardlink). L'arbitrage se rejoue quand le fournisseur s'en va. **Un exemplaire plus ancien, ou de même date, ne déloge jamais ce qui tourne déjà.**

**L'arbitrage par date protège les poses automatiques, pas les décisions.** Un « ajouter au jeu » donné sur un dossier proposé (§4.6ter), devant l'avertissement qui disait combien de fichiers du jeu seraient remplacés, lève la comparaison de dates pour ces chemins (`forced_extras`, réécrite à chaque déploiement, qui survit à une désactivation et ne disparaît qu'avec le mod). **La sauvegarde de l'original reste obligatoire.**

**« En attente » n'est pas une impasse** : un ajout qu'un fichier étranger occupe (§4.5.5) porte sur sa ligne **« Poser quand même »** (`extras::force_one`), la même autorisation explicite, réversible par la sauvegarde.

**3. Sauvegarde avant écriture.** Un fichier que **personne ne réclame** (Kunos, mod posé à la main, reste d'une ancienne version) n'est remplacé qu'après mise à l'abri de l'original, et revient dès que plus aucun mod ne réclame le chemin. Il n'est **jamais supprimé**.

**4. On ne retire que ce qu'on a posé, et seulement si c'est encore là.**
- **Seule l'absence de réclamation en base autorise une suppression.** Ne pas savoir *résoudre* une réclamation (bibliothèque déplacée, exemplaire disparu, type illisible) n'est jamais une raison d'effacer.
- **Avant de retirer, on vérifie que le fichier posé est encore le nôtre** (taille + date avec l'exemplaire de bibliothèque). Un fichier recréé depuis par un outil externe n'est pas touché.
- Le fournisseur courant est **mémorisé** (`provided`), jamais déduit du fichier posé ; une réclamation se suffit à elle-même (`kind` et `claimed_at` y sont recopiés) ; les fichiers et les dossiers créés pour l'occasion sont mémorisés séparément, pour n'élaguer que des dossiers qu'on a créés.

**Le remplacement d'un fichier du jeu** — shader modifié, config CSP qui écrase la stock, images de `content/gui/` — **est fait par défaut, et annoncé** (§4.5.5). C'est la réversibilité qui le rend sûr, et elle passe par `gamebackup.rs` (règle d'or n°5) :

1. sauvegarde **avant** toute écriture ;
2. sauvegarde vérifiée lisible avant de toucher au jeu, sinon on ne remplace pas ;
3. **la première sauvegarde fait foi** : un second mod visant le même chemin ne sauvegarde pas le fichier du premier ;
4. restauration dès que plus aucun mod ne réclame le chemin ;
5. au démarrage, restauration de toute sauvegarde que plus rien ne réclame — en comptant les réclamations des **deux** mécanismes de pose.

L'original vit dans `<lib>/game_backup/<chemin relatif à AC>`, la table `game_backups` fait le lien : le chemin de la sauvegarde dit à lui seul où le fichier revient.

#### 4.5.5 Ce que la fiche montre

Deux cartes, **Ressources** (onglet Médias et documents, §6.1) et **Ajouts au jeu** (onglet Installation, §6.3bis), avec la même mécanique : liste **lue en direct sur disque**, jamais mémorisée en base — un fichier déposé à la main apparaît, et un mod importé avant ces règles n'a rien à réimporter. Seul l'**état de pose** des ajouts vient de la base.

- **Ressources** — un clic ouvre le fichier (prévisualisation, §4.5.2) ; un bouton ouvre le dossier du mod.
- **Ajouts au jeu** — répond à « qu'est-ce que ce mod met chez moi en plus de son dossier ? », **regroupé par dossier de destination**, chaque groupe dépliable. Signalés au fichier et au dossier : les fichiers **partagés** avec un autre mod (avec son nom) ; ceux qui **remplacent** un fichier du jeu, en **rouge** (l'annonce obligatoire du §4.5.4) ; la **zone Content Manager** (bleu, information, et un bandeau en tête de bloc) ; **en attente** (jaune) quand un fichier étranger occupe le chemin — cas fréquent en zone CM, qui gagne souvent l'arbitrage par date.

### 4.6 Décider ou demander, et rendre compte

**Le critère : demander seulement quand l'information nécessaire est dans la tête de l'utilisateur, pas sur le disque.**

- Déterminable en regardant le contenu → l'app **décide**, et rend compte. Jamais de question.
- Préférence ou intention (quelle variante installer) → **demander** (§4.3 : détection automatique, question seulement si ambigu, récapitulatif chiffré).
- Nulle part → défaut **réversible**, et signalement visible.

Une question à laquelle l'utilisateur ne peut pas répondre correctement est **pire** qu'un défaut : elle donne l'illusion du consentement. Ce qui coûte cher n'est pas un défaut mal choisi, c'est un défaut **appliqué en silence**.

Deux contraintes encadrent toute nouvelle question : **jamais de blocage en import de masse**, et **arbitrage groupé** en fin de lot. Relèvent vraiment de l'utilisateur : les **variantes offertes par l'auteur** et **deux versions du même mod dans une archive**.

**Journal des décisions** : chaque arbitrage **surprenant** d'un import est enregistré (`import_decisions`) et relisible sur la fiche, bloc « Décisions d'import » (masqué quand il n'y a rien) — chemin deviné (`pathNormalized`), chemin refusé (`pathRefused`), annexe non extraite (`ancillaryDropped`), reste qu'aucun mod ne réclame (`leftoverUnattached`). La routine n'y est pas. Il décrit le **dernier** import, réécrit par mod et par archive ; écriture best-effort.

#### 4.6bis Composants optionnels

**Le discriminant est le rayon d'action.** Ce qui atterrit *chez le mod* s'installe sans rien demander ; ce qui atterrit *chez les autres* — un fichier du jeu de base, vu dans toutes les sessions — n'est plus le mod qui s'installe, c'est le jeu qu'on modifie.

**Deux signaux, exigés ensemble** : une **archive imbriquée** livrée à côté du mod (signal structurel, le nom n'est jamais lu), **et** elle **remplace des fichiers du jeu de base** (le chemin existe dans AC, aucun mod ne le réclame, ce n'est pas un exemplaire qu'on a posé — `others::game_files_replaced`). Aucun des deux ne suffit seul.

**Traitement** : importé, rangé, **laissé inactif**, et la question posée **en fin de lot** dans le rapport. Ne rien décider est valable : le composant reste activable depuis l'inventaire. Les fichiers `_backup` d'un auteur sont posés comme le reste : aucune heuristique de nom.

**Limite assumée** : des dossiers quelconques accompagnés d'instructions en prose ne s'interprètent pas ; la réponse est de rendre la notice **lisible** (§4.5.2) et de dire ce qu'on a fait.

#### 4.6ter Dossiers proposés par l'auteur

**Le déclencheur est structurel** : un *dossier* resté après le balayage dont le chemin ne mène nulle part dans le jeu (`2K Skins/`, `Optional Textures/`, `MODS/<variante>/`, `Wallpapers/`). Un dossier qui mène dans le jeu reste un ajout ; un fichier isolé reste une annexe ou un ajout. L'information qui manque est dans la notice ou dans l'intention : **on demande** (§4.6).

Un pack de skins dont la cible est inconnue **et** dont les livrées recouvrent celles d'un contenu de la même source est une **variante** proposée, pas un pack (`pending::offered_liveries_target`). Sans ce recoupement, un pack pour une voiture absente reste un pack, et se branchera quand elle arrivera.

**Rangé, jamais posé, jamais perdu** : le dossier attend dans `<lib>/pending/<id>/` (`pending_folders`), rien n'entre dans le jeu, et la question est posée **en fin de lot**. Ne rien décider reste valable.

**Une question pour les dossiers identiques** : même nom (dernier segment, sans la casse), même forme, mêmes réponses → demandés une fois, la carte disant combien de mods elle couvre ; « Répondre mod par mod » la dédouble. Ce n'est pas un « appliquer à tout ». **Un dossier qui remplace des fichiers du jeu de base n'est jamais regroupé.** Un groupe dont une partie échoue se dédouble.

**Cinq formes reconnues**, qui choisissent ce qu'on montre et ce qu'on pré-remplit : **variante JSGME** (un `description.jsgme`, libellé repris tel quel), **contenu de jeu** (un enfant direct est un dossier lu par AC), **livrées de remplacement**, **documents** (que des formats qu'AC ne lit jamais — images exclues), **indéterminé**.

| Sort | Ce qu'il fait | Comment on revient dessus |
| --- | --- | --- |
| **Ajouter au jeu** | autorise à chercher la racine de jeu **dans** le dossier ; rangé en ajouts au jeu du propriétaire, sauf ce qui ne mène nulle part (le `description.jsgme`), qui va en ressources | retiré à la désactivation, supprimé avec le mod ; les fichiers du jeu remplacés sont restaurés |
| **Ajouter au dossier du mod** | composé par-dessus la version du mod, qui n'est jamais touchée — **proposé seulement pour une variante de livrées** | depuis « Couches & extensions » ; retirée, la réponse devient « ne pas importer » |
| **Garder dans les ressources du mod** | rangé sous son nom dans les ressources du propriétaire ; rien dans le jeu ; une seconde fois, il **remplace** la copie précédente | ✕ sur la ligne, dans la carte Ressources ; la réponse devient « ne pas importer » |
| **Garder à part** | entrée « autre mod » indépendante, proposée quand rien ne rattache le dossier à un mod | depuis l'inventaire |
| **Ne pas importer** | supprimé et journalisé (`userDiscarded`) — le seul endroit où de la matière importée disparaît | l'archive source, si elle est conservée (§10) |

**Une réponse se donne une fois** : mémorisée par propriétaire et par nom de dossier (`pending_answers`), elle s'applique **sans question** à la prochaine archive du même mod qui livre le même dossier, et le rapport le dit. « Garder » remplace la copie, « ajouter au dossier du mod » remplit la couche existante (même place, même état). **Changer d'avis se fait là où vit le résultat** : retirer le dossier gardé ou la couche devient la réponse. **Exception (§4.6bis)** : un dossier qui remplace des fichiers du jeu de base n'est jamais réajouté seul ; la question revient, la réponse précédente **marquée, jamais pré-cochée**.

**« Ajouter au jeu » et « Ajouter au dossier du mod » s'excluent**, et l'arbre du dossier tranche : un arbre de jeu (`content/`, `extension/`…) n'a de sens qu'à la racine d'AC, des fichiers nus (`.dds`, `skins/`) qu'au dossier du mod. Un dossier d'un pack ne reçoit jamais « Ajouter au dossier du mod ».

**Aucune proposition, aucune couleur sur les réponses, quand le dossier remplace des fichiers du jeu de base** : ce sont des réponses à égalité, et l'avertissement porte sur le **fait** (jaune, « remplace N fichiers du jeu de base — s'applique à toutes les sessions »). Le seul repère est la mention « proposé », quand l'app a un avis.

**Une carte répondue reste à sa place, à sa taille exacte** : même texte, mêmes réponses, le contraste baissé et la réponse donnée en vert. **Rien ne défile tout seul.** La modale ne se ferme pas d'elle-même : son bouton dit « Terminé ».

**Ce qui est montré** : le titre de l'auteur (devant le chemin d'archive), le chemin, la forme, le poids, le mod de rattachement, le décompte des fichiers du jeu remplacés, et **la notice avec son nom de fichier**, rendue sur place. Chaque réponse porte son explication en toutes lettres (une infobulle est invisible à la manette).

**Une modale centrée, ouverte en fin de lot** (`PendingDialog`), **non bloquante** : la fermer ne décide rien, et le rapport garde un bandeau pour y revenir.

**Ce qui reste automatique** : un dossier de jeu livré à nu (`driver/` avec un `.kn5`) est corrigé sans rien demander, et journalisé (`pathNormalized`).

### 4.7 Mises à jour des mods

Pit Box annonce qu'un mod a une nouvelle version et l'installe en un clic, **par la même source que Content Manager** : le registre CUP (`cup.rs`). Une requête à `https://acstuff.club/cup/` rend, par type, la dernière version de chaque id enregistré. **La comparaison se fait sur la machine** : le registre ne reçoit rien de la bibliothèque. `/cup/<type>/<id>` donne le changelog ; `/cup/<type>/<id>/get` redirige vers l'archive ou vers son hébergeur.

**Ce qui compte comme une mise à jour** : une voiture ou un circuit **géré par Pit Box** dont la version du registre est plus récente que le champ `version` de sa version active. Ni le contenu de base, ni un mod non géré (§8.2 — l'import refuserait ensuite de s'y poser), ni les apps (sans version connue). **Les versions se comparent exactement comme dans CM** (`CompareAsVersionTo`) : un `v` en tête retiré, découpage sur les `.`, chaque segment en ordre naturel — `1.5` est plus ancien que `1.46` ; un libellé absent est plus ancien que tout.

**Quand** : une minute après le démarrage, puis une fois par jour. Coupable dans Réglages › Général (`mod_updates_online`), qui offre aussi « Vérifier maintenant ». Une vérification ratée garde la liste précédente et affiche son erreur dans les réglages, sans toast. **Entre deux vérifications, la liste suit la bibliothèque** sans requête (`cup::still_pending`) : une mise à jour importée à la main disparaît dès l'import.

**Où ça se voit** :
- **un toast** des mises à jour pas encore annoncées, chacune avec « Mettre à jour » et « Ignorer » ; refermé, il ne revient pas pour les mêmes versions (`pitbox.modUpdates.announced`) ;
- **une pastille bleue** sur la carte (une flèche en vue tableau), jusqu'à la mise à jour ou l'ignorance ;
- **un bandeau** sur la fiche, avec « Nouveautés », « Ignorer », « Mettre à jour ».

**« Ignorer » porte sur une version** : une version plus récente est de nouveau annoncée.

**Mettre à jour = télécharger, puis l'import ordinaire** (§4.2) : même rapport, mêmes arbitrages. L'archive est nommée `<id>.<ext>`, supprimée après l'import sauf si un arbitrage en attente en a besoin ; ce qu'un arrêt brutal laisse est balayé au démarrage. **Un seul téléchargement à la fois, jamais pendant un import**, annulable depuis son toast, qui a l'anatomie de celui de l'import ; la barre se remplit quand le serveur annonce une taille, sinon un segment va et vient.

**Une archive ou le navigateur** : la réponse de `/get` n'est téléchargée que si elle est **une archive reconnue à ses premiers octets** (zip, 7z, rar) ; une page web, un mur de connexion ou un refus s'ouvrent dans le **navigateur**, et l'utilisateur glisse ensuite l'archive. Aucune liste d'hébergeurs. Les entrées `limited` du registre tombent dans ce cas.

**Le client HTTP** est celui de l'enrichissement Wikipédia (`http.rs`, WinHTTP) : pas de dépendance nouvelle.

---

## 5. Tags et harmonisation

**Vocabulaire fermé** : l'univers fini des tags autorisés (catégories `#gt3`/`#lmp1`, familles `prototype`/`vintage`, styles `#drift`/`#jdm`, propriétés CSP `rainfx`…). C'est le vocabulaire borné qui harmonise, pas une meilleure détection. **Ce vocabulaire, c'est exactement ce que les règles savent produire** — sorties des fusions et des déductions, plus la liste blanche des catégories de circuit : écrire une règle, c'est déclarer son vocabulaire. Un tag entrant qu'aucune règle ne produit **n'est pas promu** : il reste le tag brut du mod, affiché comme tel, et un `#` inventé par un seul auteur ne devient pas une catégorie.

**Trois origines de tags, tracées séparément** :
1. **Tags bruts du mod** — lus dans `ui_*.json`, lecture seule, masquables, avec tout ce qu'aucune règle n'a reconnu.
2. **Tags déduits par règle** — dans l'overlay.
3. **Tags manuels** — saisis, dans l'overlay, seuls directement supprimables.

Code couleur : rouge = catégorie, vert = règle, gris = manuel, bleu = brut du mod, dans cet ordre, avec une légende discrète. La bascule porte un **libellé stable** (« Tags bruts du mod ») et une case à cocher, jamais un verbe qui s'inverse.

**Les règles sont des données, pas du code** (le catalogue livré, REGLES§2, et les décisions de l'utilisateur). Familles : **Fusion** (synonymes → tag canonique), **Déduction** (`lmp1` → `#lmp1` + `prototype` + `endurance`), **Extraction** (tag → champ technique), **brand_fix** (marque corrigée depuis le nom). **Il n'y a pas de famille « suppression »** : un tag de bruit est simplement un tag qu'aucune règle ne produit. Chaque règle affiche le **nombre de mods sur lesquels elle agit** (REGLES§8.3). L'écran est `Classement › Règles` (§7.1bis).

**Le résultat de l'harmonisation est stocké**, pas recalculé à l'affichage. `rules::ENGINE_VERSION` s'incrémente à chaque évolution qui produirait un autre résultat à règles égales, et une passe au démarrage recalcule toute la bibliothèque **une fois par version** — marqueur posé seulement en cas de succès, rien tenté si la bibliothèque est inaccessible.

**Comportement** : harmonisation automatique à l'import, édition manuelle à tout moment, détection CSP (lecture des `ext_config.ini` → `rainfx`, `grassfx`, `weatherfx`, `lightingfx`, `has-skins`).

**Catégorie = tag `#` que les règles savent produire** (convention CM). Sans lui, la voiture n'a pas de catégorie, et le `#` reste parmi ses tags bruts ; l'ajouter au vocabulaire le promeut. Sert à la composition du plateau (SESSION§3.1).

### 5bis.3 Nom et description repris à la main

**Le nom et la description d'un mod s'éditent** depuis la fiche (un crayon à droite du nom, un autre dans la carte « Description »). La saisie va dans l'overlay (`display_name_user`, `description_user`), à côté des champs dérivés du fichier, donc **survit à une mise à jour du mod et à une réindexation**. Le **nom effectif** est arbitré en SQL (`COALESCE`) : liste, tri, fiche, sélecteur de session, adversaires, export en profitent. La description, relue dans le fichier, est arbitrée dans `library.rs`. Le nom du fichier reste exposé à part : c'est celui auquel on reviendrait.

- **La carte « Description » s'affiche même vide.** Vider le champ vaut « reviens au fichier du mod ».
- La phrase « Enregistré dans Pit Box, pas dans les fichiers du mod : conservé si le mod est mis à jour » s'affiche **sous le champ, pendant la saisie** ; le crayon garde une infobulle courte (« Renommer »).
- **Une réindexation ne détruit jamais ces saisies** — ni nom, ni description, ni tags manuels, ni favori. Seules les versions synthétiques du contenu de base sont refaites ; ce qui a disparu de `content/` est retiré. Une case **décochée par défaut** rétablit l'effacement complet, derrière une confirmation qui énumère ce qui va disparaître.
- **Le nom d'un circuit multi-layouts** est la **racine commune** des noms de ses layouts, en **mots entiers** (« Monza » pour « Monza 1966 » / « Monza 1971 », jamais « Monza 19 »), ponctuation de liaison retirée en bout, casse du premier layout ; sans mot commun, celui du premier layout. À l'import, à la réindexation et à l'indexation du contenu de base (§8.1).

---

## 6. Fiche technique et champs dédiés

Les caractéristiques mécaniques ne sont **pas** des tags : un tag filtre et groupe, il ne décrit pas une fiche technique.

**La fiche technique d'une voiture est en base** (`SPEC-fiche-technique.md`, FICHE§), une source par valeur, toutes gardées, et l'affichage prend la première qui répond, **toujours dans l'ordre** : modifié par vous › physique du mod (`data/` ou `data.acd`) › `ui_car.json` › déduit des tags. La physique est lue à l'import, à la réindexation, à l'indexation du contenu de base, **à travers les couches actives** (une couche qui apporte un `engine.ini` passe devant la base), et relue quand une couche ou la version active change. Une version qui introduit ou corrige une lecture rattrape en fond toute la bibliothèque, et **dit ce qui a bougé, en nombre**, par une notification « Fiches techniques relues » qui reste jusqu'à ce qu'on la ferme (« Admission : 287 renseignées · 1 modifiée ») ; une relecture qui ne change rien ne dit rien. **L'affichage ne lit plus aucun fichier**, ce qui garde la fiche entière pour un mod en vitrine (ESPACE§). Restent lus dans `ui_car.json` à l'affichage : description, auteur, version, et la puissance et le poids des cartes.

- **Les specs de l'auteur sont lues comme des nombres quand elles en sont** : `--`, une chaîne vide ou un zéro de remplissage sont une absence ; `500+Nm` garde son `+` ; les unités sont ramenées à une orthographe, jamais converties (`whp` reste `whp`) ; ce qui ne se lit pas est affiché tel quel. Le rapport poids/puissance absent est calculé quand puissance (bhp/hp) et poids (kg) le permettent.
- **La fiche se lit en trois temps** (`TechSheet.svelte`) : les **chiffres clés** en grand (puissance, couple, poids, rapport P/P, vitesse max, 0-100) ; la **Mécanique** en phrases courtes (moteur, transmission, carburant avec autonomie **en km**, origine) ; l'**Électronique** en puces (FICHE§5). Un champ absent disparaît, une ligne ou un groupe vide aussi : jamais de « — » dans une case. Pas de bandeau de specs sur l'aperçu, pas de ligne « URL d'origine » tant que rien ne la remplit.
- **La provenance se lit sans survol et sans couleur** : rien pour ce qui vient des fichiers du mod, « ≈ » en gris devant une valeur déduite des tags, « ✎ » en gris après une valeur modifiée, chaque signe expliqué au survol. Les colonnes Transmission, Boîte, Moteur, Position et Admission de la bibliothèque sont **le cache de la valeur affichée**, avec les mêmes mots et le même « ≈ » ; le tri se fait sur le mot.
- **Les puces d'électronique**, lues dans la physique seule : ABS, antipatinage, EDL, DRS, KERS, ERS, roues arrière directrices, anti-roulis actif, répartiteur électronique. Présente : pleine ; absente : pointillés, nom barré ; inconnue : pas de puce. **Seuls ABS et antipatinage s'affichent absents.** L'écran de session lit ces deux aides dans la même fiche (SESSION§3).
- **La fiche se corrige** (FICHE§8) : le crayon ouvre l'édition de toute la fiche — nombres à unité fixe, listes fermées (dont le pays, avec la liste du jeu et ses drapeaux), une puce à trois états par aide. Chaque champ dit sa source ; « ↺ revenir » rend la valeur du mod. Rien n'est écrit avant « Enregistrer », une valeur invalide n'enregistre rien. La correction survit à une mise à jour, une réindexation et une réharmonisation ; l'historique note « Fiche technique modifiée (boîte, rapports) », jamais les valeurs.
- **L'odomètre est dans l'en-tête de la fiche voiture**, avec « essayée » ou « jamais essayée » (dans la carte de données pour un circuit, §6.3bis) ; la règle vit dans `odometer.ts`.
- **Badge de marque** : `ui/badge.png`, sur fiches et vignettes, un repli quand il manque. Pas d'icône d'auteur : le nom en texte.
- **Année** : `year` de `ui_car.json`, sinon pour le contenu de base la table `kunos_content_dates.json`, sinon « — ». Aucune dépendance à la base en ligne de CM.
- **Champs structurés** remplis par l'Extraction, pour ce que les specs ne couvrent pas (transmission, position du moteur, admission, architecture, boîte) ; le tag technique extrait sort du vocabulaire. Même principe pour le **pays** : tag pays → champ `country` s'il est vide.

**Le pays est normalisé à l'écriture, à un seul endroit** (`harmonize::store`), et le `ui_*.json` n'est jamais touché :

- **Les alias** normalisent un pays déclaré (`U.S.A.` → United States) et jouent toujours ; **l'extraction** devine un pays absent d'après un tag et ne joue que si le champ est vide. Les deux s'éditent dans **Classement › Pays** (TAXO§8), pas dans l'écran Règles, et valent pour voitures et circuits.
- **Deux fusions qui ne sont pas des décisions** : un nom de la table du jeu, casse, espaces et accents repliés, prend l'orthographe du jeu ; un code ISO valide prend le nom du pays. Toute autre ressemblance (`Swizerland`) est une **proposition** de l'onglet Pays. La table du jeu est chargée au démarrage ; absente, seuls les alias jouent.
- Le jeu d'alias livré est **mesuré** sur une bibliothèque réelle. **Les nations britanniques n'y sont pas** : AC leur donne leur propre drapeau.
- **Une valeur cassée se lit, elle ne se répare pas** : le nom retenu est ce qui précède le premier guillemet ; **jamais une coupe à la virgule**, que des noms de la table contiennent.
- Un circuit déclare et garde son pays comme une voiture : la réharmonisation le relit par le même lecteur que le reste du fichier. Une évolution de ces règles incrémente `ENGINE_VERSION` (§5) et se rattrape seule au démarrage.

**Favori** : état personnel (cœur), ni tag ni caractéristique.

**La fiche pleine page (`DetailPage.svelte`) est la seule fiche.** Trois onglets (§6.3bis) : **La voiture / Le circuit**, **Médias et documents** — avec son décompte entre parenthèses dès qu'il est connu, récupéré en fond sans retarder l'affichage —, et **Installation**, sans décompte : c'est une section, pas une collection. Les actions secondaires de l'en-tête (activer/désactiver, exporter, réinstaller, supprimer, aperçu natif, ouvrir le dossier) sont dans un menu **⋮** ; le cœur et le badge « Contenu de base » restent visibles.

**Un seul curseur** (`ui/Slider.svelte`) et **un seul composant d'onglets** (`ui/Tabs.svelte`) pour toute l'app : le remplissage d'un curseur se calcule dans le composant, à partir de ses bornes ; un composant d'onglets s'inscrit tout seul auprès de la manette (§7.4bis), sauf une bande imbriquée dans un onglet (les sous-onglets du bloc textuel), qui s'en retire. Variante `flush` pour la fiche.

### 6.1 Onglet Médias et documents (fiche voiture/circuit)

Quatre blocs, en deux groupes : ce que **l'utilisateur a produit** — **Screenshots**, **Replays** — puis ce qui est **livré avec le mod** — **Ressources**, **Backgrounds** (circuits seulement). Le décompte de l'onglet est leur somme. **Aucun bloc n'est masqué quand il est vide** : « Ouvrir le dossier » et « Associer un fichier… » sont la seule voie pour y ajouter quelque chose.

**Les documents livrés à part rejoignent la liste Ressources** : une notice ou un manuel que l'import a rangés comme mods à eux (hors du dossier du mod, §4.5.2) y figurent, chacun marqué du nom de sa livraison — pas une carte par document. Ils restent gérables depuis « Posé sur ce mod » et l'inventaire.

**Rattachement des médias** par **`nom_de_fichier.contains(id)`** sur tous les ids de la bibliothèque (voitures ∪ circuits, contenu de base compris) : les deux espaces de noms ne se recoupent pas. Un faux positif occasionnel (id contenu dans un autre) est accepté — un média de trop vaut mieux qu'un rattachement manqué. Repli : **l'association manuelle** (`media_links`), jamais écrite par le rattachement automatique.

**Mise à la corbeille** (Screenshots et Replays — les Backgrounds sont des fichiers de CSP) : 🗑 sur chaque vignette, ligne et dans la visionneuse, et **Suppr** sur l'élément focalisé. **Corbeille Windows, jamais une suppression définitive** : quand le recyclage est impossible, l'opération échoue au lieu d'effacer (`media::trash_file`). Récupérable, donc **sans confirmation**. Un rattachement manuel vers le fichier est retiré avec lui. En visionneuse, on enchaîne sur l'image suivante (la précédente pour la dernière), et on ferme quand il ne reste rien.

**Visionneuse plein écran** (`Lightbox.svelte`) : précédent/suivant (boutons, flèches, manette), diaporama (4 s), fermeture par ✕, clic sur le fond, Échap ou B. Ouverte, elle prend la manette pour elle (`nav.inputCapture = "lightbox"`) : une pression n'agit jamais sur ce qu'il y a dessous.

**Miniatures en cache sur disque** (`thumbnails.rs`, `app_cache_dir()/thumbnails/`, clé = chemin + date + taille cible) : les grilles n'affichent jamais l'original ; seule la visionneuse charge l'image d'origine. Pas d'éviction.

**Les noms de fichiers lus** :
- `Documents\Assetto Corsa\screens\` : `Screenshot_<car>_<track>_<date>.jpg` en session, `Showroom_<car>_<date>.jpg` (sans circuit). La date du nom n'est jamais lue (format irrégulier) : seule la date du fichier compte.
- `Documents\Assetto Corsa\replay\` : l'autosave du jeu `AC_<ddmmyy>-<hhmmss>_<type>_<car>_<track[_layout]>_…`, ou le nom que CM donne à un replay gardé, `<car>_<track>_<ddmmyy>-<hhmmss>`. La date se lit dans le nom, à défaut dans le fichier : **un replay a toujours un horodatage**. `replay\temp\` n'est jamais lu.
- `<AC>\extension\backgrounds\<track>[__<layout>]_<variante>.jpg` (CSP), par préfixe.

**Une ligne de replay** affiche le combo réellement piloté, l'horodatage, la durée, le nombre de voitures, le pilote, la taille et le nom du fichier. Les cinq premiers viennent de **l'en-tête du `.acreplay`** (`acreplay.rs`), atteint par un saut de taille fixe : lire un replay de 635 Mo coûte autant qu'un de 867 Ko. En-tête illisible : la ligne garde ce que le nom et le disque donnent, jamais d'erreur.

**Autosave ou gardé** : AC fait tourner ses autosaves par **nombre** (`cfg/replay.ini`, `[AUTOSAVE]`), personne ne les supprime par ancienneté. Une ligne porte **« Autosave rang/limite »** (son rang parmi les autosaves du même type de session, le plus récent en 1) ou **« Gardé »** (renommé, sorti de la rotation) ; le détail est en infobulle.

**La liste se recharge à la fin d'une session**, sans rouvrir la fiche, sur le signal de fin du jeu (`ac://running`) : à la fermeture, puis six secondes après — AC pose l'autosave, CM le renomme ensuite. Screenshots et décomptes suivent.

**« Lire dans CM »** (`launch.rs::launch_replay`) passe le chemin du `.acreplay` en argument à Content Manager, comme le double-clic de Windows. Pas pendant qu'un jeu tourne (SESSION§2.4).

### 6.2 Fond photo sur l'écran de réglages de session

Sur l'écran de réglages (SESSION§3), une image assombrie et floutée derrière l'interface, dans cet ordre de repli :
1. un screenshot du **combo exact** (même voiture et même circuit) ;
2. un screenshot du **même circuit**, autre voiture ;
3. le **background officiel** du circuit (§6.1) ;
4. le fond neutre.

### 6.3 Coquille de fiche unique (REFONTE§6.1, §12)

**Les cinq fiches — voiture/circuit, app, mod « autre », son, pack — partagent un seul en-tête** (`FicheHeader.svelte`) : retour, tuile, nom, sous-titre, état, favori, menu ⋮.

- **La tuile montre la chose réelle** quand elle existe (badge de marque, vignette), un **pictogramme de type** sinon — jamais deux lettres tirées du nom.
- **Le nom d'affichage se reprend à la main sur tous les types** (§5bis.3), à côté du nom dérivé ; un nom repris fait passer l'identifiant technique en sous-titre.
- **L'auteur est dans le sous-titre.**
- **Les actions vivent dans le ⋮**, sauf un contrôle par fiche quand il en est la raison d'être (la clé de contact d'un mod de son).

**Un seul vocabulaire d'état** (`StateBadge.svelte`), dans l'en-tête et dans la colonne « État » du tableau : **Actif** (vert), **Inactif** (orange), **En attente** (jaune — actif mais perdant l'arbitrage sur un emplacement disputé), **De base** (bleu), **Non géré** (gris), et **En vitrine** (ESPACE§).

### 6.3bis Les trois onglets de la fiche voiture/circuit (REFONTE§7)

| Onglet | Contenu |
|---|---|
| **La voiture** / **Le circuit** | aperçu, sélecteur, fiche technique, courbe, son, bloc textuel |
| **Médias et documents** | Screenshots, Replays, Ressources, Backgrounds (§6.1) |
| **Installation** | ajouts au jeu, décisions d'import, origine, historique, couches, étiquettes, extensions CSP |

**Les étiquettes vivent dans Installation** : on les ouvre quand la dérivation de la catégorie s'est trompée ; le premier onglet montre le résultat, la **catégorie**, en puce près du titre. Les **extensions CSP** sont dans Installation : elles décrivent l'installation, pas le contenu.

**Le sélecteur de livrée ou de tracé est une carte de la colonne de droite** (`PickerBar`), avec en-tête et compteur (« SKINS 28 »), **au-dessus de la ligne de flottaison** — un contrôle n'exige jamais de défiler. Trois gestes : les **flèches** (défiler en gardant l'œil sur l'aperçu), le **nom** (liste déroulante) et **« Voir les N »** (grille de vignettes dépliée en place, repliée à chaque ouverture de fiche).

**Le premier onglet tient en deux zones.** Une rangée à deux colonnes **qui se referme sur la hauteur de l'aperçu** — l'aperçu à gauche, toutes les cartes à droite ; une colonne de droite plus haute que l'aperçu allonge la rangée, l'aperçu reste en haut ; sous ~1 100 px de largeur **disponible**, une colonne. Puis le **bloc de lecture**, pleine largeur, contenu centré sur des **paliers de largeur** (100 %, puis 540 / 720 / 960 / 1140 / 1320 px), sans hauteur minimale. **Tous les seuils de cette mise en page sont des requêtes de conteneur**, jamais de média.

**Un circuit n'a que deux cartes de données** — « Circuit » (longueur, nombre de tracés et combien une couche en apporte, pays, odomètre) puis « Habillages » —, et la grille n'est pas dictée par le type le plus riche. **La longueur porte son unité** : un entier est en mètres, une valeur sous 100 est lue en **kilomètres**, une valeur qui porte déjà une unité est rendue telle quelle (`trackLength.ts`). Le nom du tracé n'entre pas dans la carte : le sélecteur le dit. Le sous-titre de l'en-tête : marque et année pour une voiture, longueur et nombre de tracés pour un circuit.

**Le bloc textuel est à sous-onglets** : Description | Notes | l'article Wikipédia (« Le modèle réel » sur une voiture, « Le circuit » sur un circuit). Notes n'est jamais l'onglet par défaut et porte une pastille quand une note existe. **L'onglet Wikipédia est toujours présent** : sans article, il dit lequel des cinq cas s'applique (recherche en cours, rien trouvé, ambiguïté, entité sans article lisible, enrichissement désactivé), sans ton d'erreur, et propose d'en associer un à la main — choix enregistré en `manual`, que rien d'automatique n'écrase. L'article est cherché à l'ouverture de la fiche, jamais en masse, affiché **en entier, reconstruit depuis une liste blanche** (jamais le HTML de Wikipédia injecté tel quel), avec les seules images de Commons dont l'auteur et la licence sont connus, chacune créditée, ouvertes dans la visionneuse de l'app. **Le texte n'est jamais fondu dans la description ni reformulé ou traduit**, et l'attribution en pied (titre, lien, CC BY-SA) est obligatoire ; « Voir sur Wikipédia » ouvre le navigateur du système. Détail : `SPEC-wikipedia-fiche-detail.md`. L'enrichissement se coupe (`wiki_online`, §11) : coupé, aucune requête ne sort, le cache reste consultable.

### 6.3bis (suite) Tracés apportés par une couche (REFONTE§7.7)

La carte des tracés montre l'**état composé**, et marque l'origine d'un tracé qu'une couche **active** apporte ; le compteur ajoute « dont 1 ajouté ». Deux conditions (`layers::layout_origins`) : la couche pose des fichiers sous ce tracé **et** la base ne le connaît pas. Une couche qui habille un tracé existant ne l'apporte pas.

### 6.3ter Fiche d'une couche (REFONTE§8)

Une couche a sa **fiche**, ouverte depuis la liste de l'hôte et posée par-dessus. La liste de l'hôte ne fait que poser, activer et ordonner. Dans l'ordre :

1. **Trois chiffres** : ajoutés, remplacés (en rouge dès qu'il y en a), poids.
2. **« Ce qui écrase la base »**, en clair — avec, une fois, la promesse que l'original revient dès qu'aucune couche ne le réclame.
3. **« Ce qui s'ajoute »**, replié par dossier, compteur et poids au niveau du dossier.
4. **Ordre**, à partir de deux couches sur le même hôte.
5. **Notes**.

Le **nom affiché est dérivé** (`layerName.ts`) : extension d'archive retirée, séparateurs rendus à l'espace, préfixe de l'hôte retiré quand l'archive le répète — en coupant sur une espace seulement (`withoutBrand`). Il se reprend à la main.

### 6.3quater Fiche d'un mod greffé simple (REFONTE§11)

Une police, un fragment de config, un mannequin :

- **Où il atterrit** : les chemins réellement posés, groupés par dossier de destination, **relatifs à la racine d'AC**. Un mod qui n'a rien posé (désactivé, ou porteur d'une copie plus ancienne que ce qui tourne) le dit au lieu d'afficher une liste vide.
- **Conflits** : quels autres mods visent les mêmes fichiers, combien, et qui gagne — la priorité marquée à la main, ou à défaut la date.

Page pleine posée sur la liste, comme les autres fiches.

### 6.4 Notes (REFONTE§9)

**Tout mod peut porter une note libre**, quel que soit son type (`notes_user` sur les cinq tables d'entités, `set_entity_note`). **Une note n'est pas une description** : vider une description veut dire « reviens au fichier », vider une note veut dire vide. **Texte brut**, pas de markdown ; **sauvegarde à la perte du focus** ; la phrase « enregistré dans Pit Box » pendant la saisie.

**Une note se retrouve** : elle entre dans la **recherche plein texte**, la bibliothèque offre « Note contient… », « A une note » et une colonne Note, et un **marqueur ✎** est sur la carte et la ligne, la note en infobulle.

---

## 7. Bibliothèque et navigation

### 7.1 Deux bibliothèques distinctes

Voitures et circuits sont **deux bibliothèques séparées**, jamais mélangées, chacune avec ses colonnes, son tri et ses filtres, persistés par type. Trois colonnes de dates : **ajout**, **mise à jour**, **publication** (la date des fichiers à l'import ; pour le contenu de base, `release` de `kunos_content_dates.json`). **Ajout et mise à jour valent `—` pour le contenu de base** : aucune source fiable n'existe, et une date fausse affichée comme fiable est pire que l'absence. Une icône ⓘ sur les trois en-têtes explique la différence.

**Les filtres sont des puces, pas une barre de contrôles** (`library/filters.ts`, `components/filters/`) : **une seule ligne**, qui ne passe à la suivante que si les puces débordent. L'éditeur d'un filtre vit dans un popover accroché à sa puce.

- **La puce est une ancre, pas un contrôle** : rien ne se règle dans la barre.
- **Le clic modifie, la croix retire**, sans exception. « Filtre indifférent » n'est pas une position : c'est **l'absence de la puce**, donc de la clé dans l'état — un éditeur ouvert puis vidé rend sa clé à la fermeture.
- **La polarité appartient à la valeur** : `marque = Ferrari` et `marque ≠ Abarth` coexistent dans une puce.

**Quatre types de filtre** : `val` (valeurs signées : marque, pays, auteur, tag, catégorie, classe, état…), `range` (année), `text` (description) et `bool` (favoris, déjà essayé, pilote choisi…). Pour un `val` : *(inclusions combinées par l'opérateur)* **ET** *(aucune des exclusions)* — **l'exclusion l'emporte toujours**. Entre filtres différents : toujours ET. **L'opérateur ET/OU n'est offert que là où un ET peut remonter quelque chose** (tags, familles des voitures, catégories des circuits), et sa pastille n'apparaît qu'à partir de deux inclusions.

**Le catalogue diffère entre les deux écrans** (marque, année, classe, pilote choisi : voitures seulement), d'où des **épingles rangées par type**. **Un filtre épinglé reste visible en fantôme** (bordure pointillée, `◈`, sans croix) ; sa croix le ramène au fantôme au lieu de le faire disparaître. L'épingle se règle **dans le menu d'ajout**, pas dans les réglages. **Épinglés d'usine : Catégorie, Favoris, État** — les trois existent des deux côtés.

- **Aucun bouton de validation** : chaque modification s'applique tout de suite et le décompte bouge à la frappe. Le popover se ferme au clic ailleurs ou à Échap, sans rien annuler.
- **Les valeurs `val` sont dérivées de la bibliothèque**, donc fermées. Le champ de l'éditeur sert à retrouver une valeur ; un `-` en tête (ou Alt+Entrée), le `−` au survol d'une suggestion, ou un clic sur un jeton posé basculent en exclusion.
- **Poser une valeur ne dérange pas la liste** : la valeur reste à sa ligne, signe allumé ; la recherche tapée n'est pas effacée au clic (elle l'est sur Entrée) ; le récapitulatif des jetons se lit **sous** la liste. Toutes origines de tag sont équivalentes pour filtrer.
- **Les décomptes par valeur se calculent sur le type courant**, pas sur les résultats filtrés, une fois par chargement de la liste ; chaque filtre est compilé une fois en prédicat (`buildPredicate`), jamais par carte.
- **Le pays porte le drapeau du jeu** partout où il s'affiche, et **se lit dans la langue de l'utilisateur, traduit depuis son code ISO** (TAXO§12) — code tiré de la table du jeu, jamais du nom ; les nations britanniques par clé. **La valeur stockée, filtrée et triée reste le nom anglais du jeu** ; le drapeau se cherche par la valeur. Sur la fiche, c'est le pays rangé (normalisé, §6) qui s'affiche. La correspondance est exacte : un pays absent de la table du jeu n'a pas de drapeau, jamais un drapeau approché.
- **« État »** dit ce que dit la pastille : **Actif, Inactif, De base, Non géré** (exclusives) et **Cassé**, qui les traverse. « Actif » ne remonte pas le contenu de base. Le test « de base » est `is_stock && !is_unmanaged`. **Classe** est une liste lue sur la bibliothèque.
- **La recherche libre est le seul filtre sans puce** : elle porte sur le nom, la marque, l'id, la catégorie, le pack et les tags — **pas la description** —, un terme par mot, ET entre eux, chacun en « contient ». En tête de la coulée, avec sa croix ; « Tout effacer » l'efface aussi. Elle reçoit les recherches imposées de l'extérieur (« filtrer par pack »).
- **L'exclusion n'a pas de couleur à elle** : elle se dit par le mot (« sauf ») et la rature. Une puce oui/non à son état négatif s'écrit en texte plein, jamais grisée (elle se lirait comme un fantôme).
- **Persistance** : valeurs et épingles dans `ui_prefs.json`, par type. Les deux générations précédentes d'instantané sont relues et converties (une ancienne case « contenu de base » devient une valeur d'État ; un ancien tri-état « non géré » est ignoré) : une bibliothèque filtrée avant une mise à jour se rouvre filtrée.

**Les circuits ont aussi des catégories**, pour filtrer et composer.

### 7.1bis L'index : l'état par défaut des deux bibliothèques

Spec : `SPEC-index-bibliotheque.md` (`INDEX§`). **Sans puce active et sans texte dans la recherche**, l'écran montre un **index** de tuiles au lieu du mur de vignettes. Une puce fantôme ne compte pas.

**La tuile pose une puce, et rien d'autre** (INDEX§2), dans le même état de filtre que l'éditeur (`poseValue`) : croiser, exclure, « Tout effacer », l'épinglage marchent sans rien de plus. Retirer la dernière puce ramène à l'index. **L'index n'est pas mémorisé, les filtres le sont** : une bibliothèque laissée filtrée se rouvre sur sa liste.

- **Circuits : un index par pays**, par ordre alphabétique du nom affiché (`Non renseigné` en dernier), drapeau et nom traduit ; « Voir tous les circuits » mène à la liste sans puce.
- **Voitures : deux index, familles puis marques**, pas de pays (INDEX§6.3).
- **La tuile de valeur absente** (« Non renseigné », « Non classé ») ferme la grille, en pointillés, et pose une valeur comme les autres (`UNSET_VALUE`).
- **Les marques significatives seulement** : celles qui couvrent 80 % de la bibliothèque par nombre décroissant, bornées à [8, 24]. « Toutes les marques » déplie la liste entière **par ordre alphabétique**, sans mémoriser le dépli. L'emblème est le logo élu de la marque (§7.2quater).
- **Une seconde valeur croise ou remplace selon le filtre** : un filtre à opérateur ajoute sous ET, un filtre mono-valué remplace.

**Les familles sont un filtre à part, pas la catégorie** (TAXO§7.3). Une famille regroupe des tags ; une voiture en porte autant que ses tags **et sa classe** (`street`/`race`) en atteignent, d'où des décomptes qui ne s'additionnent pas. `Catégorie` reste le premier tag `#` : la puce « même catégorie » du bloc Adversaires la pose. La table des familles est **un index, pas une règle** : le moteur ne la lit pas, rien n'est écrit dans l'overlay, les tags sont comparés sans casse ni `#`. **Un tag n'appartient qu'à une famille.** Le catalogue en livre neuf (Route, Sportive, Course, Classique, Monoplace, Prototype, Drift, Rallye, Trafic).

**L'onglet Familles de Classement édite la table** (TAXO§6) : une ligne par famille, triée par nombre de voitures, avec un fanion ⚑ quand elle diffère de la version livrée, dépliable sur son nom, son icône et ses tags. **Rattacher un tag le déplace**, jamais ne le copie. Une famille créée reçoit un id tiré de son nom **une fois pour toutes**. « Rétablir » remet une famille livrée à l'origine et **renvoie chez eux** les tags qu'elle avait gagnés ; « Tout rétablir » et « Supprimer » demandent un second clic. Chaque modification s'écrit aussitôt (`save_category_families`), sans réharmoniser.

**Catalogue et surcouche** (`SPEC-regles.md`, REGLES§2 ; `pitbox-catalog`, `taxonomy.rs`, `rule_overlay.rs`) : les tables de taxonomie (familles, alias et tags de pays, fusions de marques, « pas une marque ») et les règles de l'écran Règles sont un **catalogue embarqué, relu à chaque chargement**, et les **décisions** de l'utilisateur vivent à part (`taxonomy.json`, `rules-overlay.json`), indexées par la clé naturelle de chaque entrée ou par l'**identifiant stable** de chaque règle livrée (`pitbox.brand.bayro`…, REGLES§4). Une mise à jour remplace le catalogue sans toucher aux décisions ; une décision l'emporte entrée par entrée (REGLES§3). **Une suppression est une pierre tombale.** La surcouche des familles est **par tag**. Une règle livrée se **désactive** (elle continue de recevoir les améliorations, REGLES§5) ou se **dérive** (une copie figée, avec l'empreinte de son origine) ; les règles de l'utilisateur passent **devant** celles du catalogue. **La table effective n'est calculée qu'en Rust.** `tag-rules.json` n'est plus lu : migré une fois **à diff nul**, contre le manifeste figé des tables publiées (REGLES§13.5), puis mis de côté sous `tag-rules.pre-overlay.json`. Un `taxonomy.json` illisible n'est jamais réécrit.

**Ce qui est livré se distingue de ce que l'utilisateur a fait** (REGLES§8.1) : badge `PIT BOX` sur une famille ou une règle livrée, `✎` et pointillés sur un ajout de l'utilisateur, l'élément livré retiré affiché barré avec `↺`.

**L'écran `Classement › Règles`** (REGLES§8) : une liste par section **dans l'ordre d'exécution** ; par ligne, une bascule immédiate, ce que la règle cherche → ce qu'elle donne, `⚑ nouvelle version` quand la règle livrée a changé depuis une dérivation, le **compteur d'effet**, un menu ⋮. **Modifier une règle livrée demande d'abord** (REGLES§8.2) : « Modifier quand même », « Désactiver plutôt », « Annuler ». En tête, l'**interrupteur global** du catalogue (REGLES§7), qui ne concerne que les règles en liste. **Chaque geste est écrit aussitôt et réappliqué à toute la bibliothèque**, et les compteurs reviennent de cette même passe.

**Exporter et importer ses règles** (REGLES§9) : l'export ne contient que les **décisions** ; **l'import fusionne, il ne remplace jamais** — en cas de conflit, la décision de l'utilisateur reste —, et rend compte. Il accepte aussi un **relevé** de bibliothèque (§10), dont seule la section `decisions` est lue.

**Un nouveau catalogue s'applique tout seul, et le dit après** (REGLES§6, `catalog_update.rs`) : au démarrage, si l'empreinte du catalogue embarqué a changé, la bibliothèque est **reclassée**, et un **rapport** reste — règles ajoutées, corrigées, retirées, et l'effet mesuré de chacune sur les mods reclassés —, dans une notification et un bandeau des onglets de Classement, jusqu'à ce qu'on les ferme. **Jamais d'assistant qui demande avant.** « Revenir à » réinstalle le catalogue précédent, gardé jusqu'à la mise à jour suivante. Une mise à jour vue sans bibliothèque accessible est remise au démarrage suivant.

**Côté développeur, le catalogue se cure dans l'app** : on organise dans Classement comme un utilisateur, puis `npm run rules:diff` et `npm run rules:promote` (`rules-tool`, jamais livré) ; une règle à soi promue reçoit un identifiant neuf écrit une fois pour toutes, une règle livrée éteinte sort du catalogue et son identifiant n'est jamais réutilisé.

**La bascule de vue** a deux positions tant que rien n'est filtré (index / liste complète), trois dès qu'un filtre fait une liste. En Big Picture, l'index s'efface devant la vue imposée.

### 7.2 Trois territoires

L'application compte une douzaine de destinations, en trois zones dont la frontière reste **étanche** — toute entrée nouvelle se rattache à l'une des trois :

| Territoire | Ce qu'il porte | Contenu |
| --- | --- | --- |
| **Rail** (`NavRail.svelte`) | *ce qui n'appartient pas à la session* | `Session`, `Online` (et `Story`, derrière son drapeau) ; sous le filet Apps, Compléments, Classement, Fichiers ; en pied Réglages, **Content Manager**, À propos |
| **Barre de titre** (`TitleBar.svelte`) | *la forme de la fenêtre* | réduire, agrandir, fermer, Big Picture — et l'identité de l'app : logo, nom, sous-titre |
| **Colonne de session** (`SessionColumn.svelte`) | *la session elle-même* | circuit, voiture, livrée, pilote, performance, type de session, lancement — **seulement dans la zone session** |

**Le rail porte ce qui n'appartient pas à la session ; la colonne est la session, et ses parties sont des destinations** (SESSION§1) : une partie de la session ne se double pas d'une entrée de rail. Le rail n'a **qu'une entrée pour ce territoire, `Session`**, qui ramène à la **dernière bibliothèque consultée** (Voitures au tout premier lancement ; l'écran Pilote n'est pas retenu), sans tableau de bord derrière. Le bouton Start de la manette passe par le même chemin hors de la zone.

- **La marque est dans la barre de titre** ; en Big Picture, où celle-ci est masquée, elle n'apparaît nulle part.
- **« À propos »** est au pied du rail : c'est du contenu.
- **Content Manager** est au pied du rail, entre Réglages et À propos : ce n'est pas une destination — jamais actif, sans `aria-current` — et l'entrée **disparaît entièrement** quand CM n'est pas détecté. Son libellé est `Content Manager`, son icône la flèche qui sort du cadre ; il ne dit pas « dans » : CM s'ouvre sur son propre état.
- **Le rail porte les lieux hors session, les onglets vivent à l'intérieur d'un lieu, et aucun lieu n'a deux niveaux d'onglets.** Apps et Compléments, qui ont leurs propres onglets ou facettes, sont donc des entrées à part ; Classement et Fichiers regroupent des outils sans sous-rubrique (§7.2quater).
- **Session et Online au-dessus du filet** ; puis Apps · Compléments · Classement · Fichiers ; en pied Réglages · Content Manager · À propos. Aucun intitulé de rang.
- **Online n'est pas dans la zone session** : la colonne y est masquée, c'est le panneau du serveur qui choisit la voiture et rejoint (`SPEC-play-online.md`).
- **Story**, sous Online, n'existe pas encore pour les utilisateurs : visible sous `tauri dev`, et dans un build livré seulement après un lancement avec `--enable-story-studio` (`--disable-story-studio` la retire). Elle masque la colonne comme Online, et l'app ne se rouvre jamais dessus.
- **La colonne de session est une zone, pas du mobilier permanent** : affichée seulement sur Voitures, Circuits, Pilote et les réglages de la session, **sans glissement**, masquée et non démontée.
- **« Vous êtes ici », sur deux niveaux** : l'entrée `Session` reste active sur les quatre écrans ; dans la colonne, la carte de l'écran affiché porte le même filet gauche rouge de 2 px qu'une entrée de rail active (la carte voiture sur Voitures, la ligne `PILOTE` sur Pilote, le type sur l'écran de réglages).
- **L'app se rouvre sur l'écran où on l'a quittée**, onglet de Classement ou de Fichiers compris (`ui_prefs.json`) — jamais sur une fiche, jamais en Big Picture ; un écran inconnu de la version courante ramène à Voitures.
- **Quitter Big Picture est au pied du rail** quand le mode est actif.
- Chaque entrée porte **une icône et un libellé**. L'entrée active : **filet gauche rouge de 2 px** (`box-shadow: inset`, jamais une bordure qui décalerait le contenu), icône pleine, libellé en pleine lumière — la seule apparition du rouge dans le rail (§7.2ter). Flèches haut/bas pour circuler (bouclé), `Entrée` pour activer, `aria-current="page"`.
- **Pastille d'alerte** : un point de 6 px sur l'icône d'une rubrique qui contient un problème, **sans compteur** et sans agrégat sur une entrée parente. Une seule source aujourd'hui : les conflits de fichiers entre compléments.

### 7.2quater Classement et Fichiers

Deux entrées de rail, deux écrans à onglets de même mécanique (`Workshop.svelte`) :

| Entrée | Onglets | Ce qu'elle gère |
| --- | --- | --- |
| **Classement** | Règles · Marques · Familles · Pays | comment la bibliothèque se range — aucune écriture sur le disque |
| **Fichiers** | Importer · Profils · Maintenance · Dossier du jeu | ce qui est sur le disque |

Ranger la bibliothèque et gérer les fichiers sont deux métiers ; chaque onglet garde son sous-titre, qui décrit l'outil.

**L'onglet Dossier du jeu** (`gamestate/GameFolder.svelte`, `SPEC-etat-dossier.md`) montre le dossier d'Assetto Corsa tel qu'il est sur le disque, et pour chaque chemin d'où il vient. **Lecture seule** : son seul bouton est « Rescanner » ; tout le reste navigue (fiche, Maintenance, Explorateur). Le détail est dans sa spec (DOSSIER§).

**La marque est un terme, comme le pays** (TAXO§7, `brands.rs`), décidée à l'écriture : celle d'une règle `brand_fix`, sinon celle du fichier ; puis **les fusions de l'utilisateur** ; puis **la casse, les espaces et les accents**, qui ne sont pas une décision (l'orthographe la plus employée par la bibliothèque l'emporte, élue de façon déterministe). **Aucune autre fusion n'est silencieuse** (TAXO§7.2) : l'onglet Marques, trié **par nom**, **propose** (un nom qui est le début d'un autre, une faute de frappe sur un nom d'au moins cinq lettres) ; `Fusionner` ou `Ignorer`, ignorer étant mémorisé et réversible. Renommer et fusionner sont un seul geste, dont le bouton dit ce qu'il va faire. Le détail d'une marque montre **les deux façons d'y arriver** : quand le fichier écrit la marque (le nom et les écritures fusionnées), et quand le **nom** de la voiture contient un mot (les règles `brand_fix`, qu'on crée depuis là : taper un mot montre toutes les voitures qu'il attrape, « Ranger ces N voitures ici » crée la règle). **Un pack, une série, un moddeur ne sont pas des marques** : la case « Ce n'est pas une marque » fait prendre à leurs voitures la marque que **leur nom** donne (`brands::from_name` — mot entier, la première marque du nom puis la plus longue) ; rien trouvé, la voiture reste sous la valeur d'origine. Une règle « le nom contient » l'emporte sur cette recherche.

**Logos de marque** (TAXO§3 à §5, §9, `logos.rs`) : une voiture garde **son** badge partout où la voiture est montrée ; la **marque** a un logo élu, là où c'est la marque qu'on montre (tuile d'index, onglet Marques, éditeur du filtre Marque, puce d'une seule marque). **Aucun logo n'est livré.** L'élection, sur les `badge.png` des voitures : **fond transparent** d'abord, puis **résolution** (plafonnée à 128 px de petit côté), puis **le plus de voitures**, puis le premier identifiant. Un fichier à **fond cuit** (anneau de bordure opaque et quasi uniforme) est dessiné sur une **pastille claire** partout où il apparaît (`ui/Emblem.svelte`). Dans le détail d'une marque : l'aperçu aux tailles réelles, le mode de rendu (seul le logo de la marque se force, jamais le badge d'une voiture), les variantes, et **un fichier à soi** (PNG d'au moins 64 px ou SVG), copié dans le dossier de Pit Box — jamais dans le jeu. Les choix vivent dans `brand_logos.json` ; « Revenir au choix automatique » les efface. Sur la fiche d'une voiture, le ⋮ propose « Éditer la marque … ».

**L'onglet Pays** (TAXO§6) : drapeau, nom traduit (le nom rangé à côté quand il diffère), décomptes, orthographes, fanion ⚑, trié par nombre de mods. Une ligne dépliée : code ISO, orthographes (ajout, retrait), « Rattacher à… » (fusionner — et donc renommer), « Rétablir les orthographes d'origine ». **Les valeurs inconnues du jeu** sont proposées en bandeau jaune, le pays le plus proche présélectionné, avec « Rattacher » ou « Ignorer » (mémorisé, réversible). **Chaque modification réapplique les alias à toute la bibliothèque** — le pays est décidé à l'écriture.

**L'onglet est `nav.section`, pas un état local** : tous les `requestSection("import")` de l'app atterrissent sur le bon onglet, la garde de navigation (§11) et l'historique (§7.2bis) restent valables. **L'onglet actif n'est pas mémorisé par l'entrée du rail**, qui vise son premier onglet ; seul le démarrage rouvre l'onglet quitté.

**Les préférences d'import** sont une **section repliable en pied de l'onglet Importer**, repliée par défaut, écrites immédiatement, un échec s'affichant.

### 7.2ter Barème de l'accent rouge

**Le rouge signifie *ce qui est retenu pour la session, et l'action qui la lance*.** Ni la structure, ni la navigation, ni une action secondaire n'y ont droit.

| Niveau | Traitement | Sens | Quota |
| --- | --- | --- | --- |
| **1 — plein** | fond `--rosso`, texte blanc | l'action qui engage la session | **un seul élément à l'écran** (« ▶ Démarrer la session ») |
| **2 — trait** | bordure ou filet `--rosso`, 1 à 2 px | ce qui est retenu pour la session, ou ce qui a le focus | un par famille d'objets |
| **3 — éteint** | bordure `--rosso-border`, fond `--rosso-dim` | actif mais secondaire | libre |
| **0 — marque** | `--rosso` | identité, hors zone de contenu | le filet supérieur de la fenêtre et le carré du logo |

Niveau 2 : entrée active du rail, carte du duo en session dans la grille (+ badge `SESSION`), onglet actif, piste active de l'écran Pilote, case retenue d'une galerie, **option retenue d'un groupe segmenté** (`Seg.svelte`, fond éteint plus filet). Le **focus** reste **jaune** (`:focus-visible`) : un focus rouge se fondrait dans la bordure de la carte en session.

**Le survol n'introduit jamais de rouge sur un élément qui n'y a pas droit au repos.** Un contrôle neutre éclaircit son gris ; seul un élément *sélectionnable* (carte, case, puce, option) va jusqu'au niveau 3 ; l'action principale éclaircit son rouge.

**Test de conformité** : effondrer tous les tons neutres vers le fond et ne garder que `--rosso` et `--rosso-border`. Il doit rester exactement le filet de fenêtre, le logo, le filet de l'entrée active du rail, la carte en session, le bouton de lancement, et les puces actives en rouge éteint.

### 7.2bis Historique de navigation

Les deux **boutons latéraux de la souris** reculent et avancent, comme dans un navigateur. Un « écran » est la section affichée **plus les fiches empilées dessus** ; le reste de `nav` (demandes transitoires, état interne d'un écran) n'en fait pas partie. **L'historique observe cette adresse** (`navHistory.ts`) au lieu d'être alimenté par ceux qui la changent.

- **Le « ← » d'une fiche est ce même retour**, comme le bouton B de la manette : l'historique d'abord, la fermeture de la fiche en repli (`goBackOr`).
- **Changer de section et ouvrir une fiche passe par `openInSection`**, sans quoi la liste d'arrivée passerait pour un écran à part entière.
- **Naviguer après un retour efface l'avance.**
- **La garde du §11 s'applique** : reculer depuis les Réglages avec des changements non enregistrés propose de les enregistrer ; un refus laisse l'historique où il était.
- **Rien n'est persisté.**

WebView2 donne ces boutons à **son** historique, qui quitterait la page : ils sont interceptés (`mousedown` et `auxclick`) avant d'être traduits.

### 7.3 Type « Autres mods »

« Autres mods » désigne un **type**, jamais un écran : ils sont listés dans l'inventaire des compléments (§7bis). Ce qui suit dit comment ils s'importent, se posent et se classent.

- **Ce qui n'a pas de type reconnu** (shaders, configs CSP, mods d'UI, météo, mannequins…) est un « autre mod », activable et désactivable comme les autres. Priorité notée et conflits signalés — pas de moteur de superposition à la MO2. « Ouvrir le dossier » résout le chemin côté Rust, jamais reçu du front.
- **Chaque entrée a sa fiche**, avec ses actions et ses **Ressources** (§4.5.2), le même bloc que la fiche voiture.
- **La provenance d'un reste se lit sur deux lignes** : « Provenance » (l'archive) et « Emplacement dans l'archive » (absent quand la livraison était l'archive entière). L'archive seule sert de clé au regroupement « par archive » de l'inventaire.
- **Un mannequin de pilote s'importe même livré nu** : un `.kn5` seul est reconnu par son **contenu** (maillage skinné et nœuds `DRIVER:`), mis sous `content/driver/` et rangé en autre mod. Un `.kn5` qui n'est pas un mannequin est refusé avec un message. **Le fichier d'origine n'est jamais déplacé**, même en mode « déplacer ». Un mannequin dans une archive suit son chemin, sans détection.
- **La fiche d'un mannequin le montre en 3D** (la galerie de l'écran Pilote), à condition que le mod soit **actif** — et elle le dit sinon.
- **Classés par zone du jeu**, d'après les chemins de leurs fichiers : Extensions, Météo, Interface, Pilotes, Textures, Objets 3D, Polices, Filtres PP, Showrooms, l'`extension/` d'une voiture ou d'un circuit, Autres (dont une entrée sans fichier stocké). La classification n'est **pas stockée** (recalculée à chaque lecture, donc une entrée mal rangée se répare seule) ; **la règle la plus précise gagne**, fichier par fichier ; **un mod est listé sous chacune des zones qu'il touche**. Elle nourrit la nature de l'inventaire.
- **Le signalement « zone Content Manager »** (§4.5.3) vaut ici aussi, sur la ligne.
- **Pas de notion de mise à jour** : réimporter une archive dont l'id existe ne fait rien. Les corrections de rangement s'appliquent donc **à la lecture** (activation, décompte), pas seulement à l'import.
- **Un fichier isolé dans un dossier qui existe déjà dans AC** est posé par lien fichier ; un fichier déjà présent est **remplacé avec sauvegarde** (§4.5.4), et seul un exemplaire plus récent prend la place.
- **Rien n'est perdu à côté d'un mod reconnu** : le contenu non reconnu d'une archive devient son propre « autre mod » ; une archive trouvée dans ce reste est extraite et reclassée récursivement (profondeur 2).
- **Un reste = un id, dérivé de son chemin relatif à la racine balayée** : le chemin est conservé jusqu'au déploiement (`others/<id>/<chemin>`, rejoué depuis la racine d'AC), et seules les extensions d'archive en sont retirées. Un reste écarté pour id déjà connu est journalisé.

### 7.4 Vues et interactions

**La carte de la grille se détache de la page** : fond au-dessus de celui de la grille, bordure visible, élévation, et l'image posée **en retrait sur un mat neutre**, en `contain`. Les `preview.png` portent leur propre fond, donc aucun réglage de la carte n'agit derrière la voiture : le mat **confine la disparité à l'intérieur de l'image**, et absorbe une preview absente ou de format inattendu. **Les images ne sont jamais retouchées** (ni filtre, ni correction, ni masque).

**Les vignettes régénérées** (`SPEC-grille.md`, GRILLE§5 à §8) — le même moteur que l'aperçu 3D, même cadrage, même éclairage, même fond pour toute la bibliothèque, en trois presets — sont **construites mais éteintes** (`FEATURE_GRID_THUMBS`, `src/lib/features.ts`) : ni l'onglet de réglages, ni la question de l'assistant, ni la génération ne sont visibles, et les cartes gardent la `preview.png` du mod. Ce qui manque est le réglage des valeurs par défaut des presets.

**Les marqueurs d'état ont quitté la grille : on joue dans la grille, on gère dans le tableau.** Un état d'installation vit dans la vue tableau, la fiche et les filtres ; il ne reste sur une carte que le badge `SESSION` (la sélection courante, §7.2ter). **L'exception est ce qui empêche de jouer** : une carte d'un mod cassé ou désactivé est **éteinte** (opacité réduite, nom en gris), la raison en infobulle et sur la fiche — **jamais masquée**, toujours sélectionnable ; seul le lancement est bloqué, avec l'explication à ce moment-là (§7.4, garde d'activation). Un mod non géré n'est pas éteint : il est dans le jeu.

**Le nom tient sur deux lignes**, hauteur réservée même pour un nom court. **Trois niveaux de gris sous le nom** : nom, marque, puis séparateur et année.

**Retrait du préfixe de marque — option, désactivée par défaut**, à l'affichage de la grille seulement. Comparaison insensible à la casse et aux tirets, avec une courte table d'alias (`Alfa`, `Chevy`, `VW`…), tenue courte exprès. **Trois interdits** (`displayName.ts`) : le **tri** porte sur le nom complet, l'**index de recherche** aussi, et le **nom stocké n'est jamais modifié**.

**Quatre positions sur la bascule de vue** : l'index (INDEX§3), grille dense, grille confortable, liste. Les deux grilles ne diffèrent que par la largeur minimale d'une carte. Le choix est persisté **par type**. **Un menu d'affichage** (chevron accolé à la bascule) porte la densité et le retrait de la marque : on en juge l'effet sous les yeux.

**En vue tableau** : colonnes choisies, **réordonnables par glisser-déposer d'en-tête** (toutes, « Nom » compris : il ne se masque pas mais se déplace), **redimensionnables** (poignée à la souris, ou flèches au clavier une fois la poignée focalisée ; double-clic ou Entrée pour revenir à la largeur naturelle). **Le libellé d'une colonne triable est un vrai bouton**, joignable au clavier et à la manette ; la poignée sort du parcours manette. Le tableau **s'élargit** plutôt que de comprimer ses colonnes, et défile horizontalement. Visibilité, ordre et largeurs sont persistés ensemble dans `library_columns.json` (fichier écrit côté Rust).

**Tous les autres petits réglages d'interface** (filtres, épingles, tri, vue, mode copier/déplacer, tags bruts affichés, skin et layout préférés par mod…) vivent dans `ui_prefs.json` via `uiPrefs.svelte.ts`, jamais `localStorage` (règle d'or n°6) ; les clés `pitbox.*` restées en `localStorage` sont migrées en bloc au premier démarrage d'une version qui les connaît.

**Ce qui doit rester en place ne vit pas dans ce qui défile** : la barre de filtres et la liste sont deux frères, et seul le second défile. Un `overflow-x` et un enfant `sticky` dans la même boîte ne cohabitent pas.

**La barre de filtres est une coulée unique**, plus un bloc calé à droite en haut (décompte et bascule de vue) : la recherche (largeur fixe, 300 px), `+ Filtre`, un filet vertical s'il y a des puces, les puces, et « Tout effacer » seulement quand il y a quelque chose à effacer. **Contrôles 32 px, puces 26 px** : un contrôle se manipule, une puce est l'état qui en résulte.

**Filtre sur la description** : texte libre, un terme par mot, ET entre eux, sur la **description effective** (§5bis.3), arbitrée côté Rust et préparée une fois par chargement (balises HTML retirées) ; rien n'est copié en base. Un mod sans description ne remonte pas.

**Suivi d'usage** : distance parcourue par voiture et circuit ; filtre « jamais essayé » (0 km chez CM **et** jamais lancé par l'app, qui tient son propre marqueur).

**Fourchette d'année** (voitures) : **vide par défaut, et vide veut dire « aucune borne »**. L'éditeur propose des **raccourcis de décennie déduits de la bibliothèque**. Aucune des deux bornes n'a de plancher ni de plafond réel — 1950 et l'année courante ne sont que les **points de départ** des flèches ▲▼ depuis un champ vide —, la borne basse ne dépasse simplement pas la borne haute. Un filtre enregistré avec les anciennes bornes comme sentinelles se relit vide. Même règle pour la fourchette d'année du vivier d'adversaires (SESSION§3.1).

**Sélection** :
- **1 clic** sélectionne la carte et en fait la voiture ou le circuit de la session.
- **Double-clic** ouvre la fiche.
- **Le skin piloté est mémorisé par voiture**, affiché sur la vignette, rappelé dans la colonne de session.

**Sélection multiple** : Ctrl-clic, Maj-clic (plage dans l'ordre affiché), **Ctrl+A** (tout ce qui est **affiché**, jamais la bibliothèque entière ; dans la recherche, il sélectionne le texte).

**Le menu contextuel agit sur la sélection** quand le clic droit tombe dedans, et son libellé porte le décompte (« Supprimer les 12 mods ») ; hors sélection, la sélection revient au seul mod visé. Le contenu de base est écarté des actions qui ne le concernent pas ; « ouvrir la fiche » et « ouvrir le dossier » sont réservés au mod unique. **« Définir comme adversaires » / « Ajouter comme adversaires »** (une voiture ou plusieurs) font passer la session en Course et remplacent le plateau ou le complètent, puis ouvrent les réglages de la session.

**Un panneau en bas, en surimpression**, dès deux mods sélectionnés, ne garde que **ce qu'un menu ne peut pas porter** : un champ de saisie (catégorie, tag) et le favori. Les champs propres à une voiture ne sont pas proposés en masse.

**Les lots qui touchent au disque tournent hors du fil de la fenêtre et rendent des comptes** (activer, désactiver, supprimer, exporter) : progression et rapport dans la pile de notifications (§4.2bis), un bouton **Arrêter** qui interrompt **entre deux mods**, et un rapport qui dit qu'il a été arrêté. Favori, catégorie et tags restent synchrones.

**Colonne « État » et pastille d'état** (`StateBadge.svelte`, partagée par le tableau et la fiche) : vert = actif, orange = inactif, gris = non géré (§8.2 — présent et chargé, mais pas géré), bleu = contenu de base, jaune = en attente (§6.3), « En vitrine » (ESPACE§). Le tri et le filtre ne dépendent pas de l'affichage. Sur la fiche, la pastille est posée à droite de la bande d'onglets.

**Persistance du duo de session** (`session_state.rs`, `session.json`) : écrit côté Rust, synchrone. **Une écriture ratée ne passe pas inaperçue** (règle d'or n°6, `durableWrite.svelte.ts`) : réessayée une fois avec la sélection la plus récente, journalisée, puis signalée par la notification « Vos réglages ne sont pas enregistrés ».

**Garde d'activation** (`SessionColumn.svelte`) : lancer une session avec un mod non activé ferait planter CM ou AC. L'état d'activation **n'est jamais déduit du duo mémorisé** mais interrogé à chaque changement de sélection. **Une ligne au-dessus du bouton, pas un dialogue au clic** (`.warnbox`, jaune) : elle couvre **voiture, circuit et adversaires**, se voit avant de cliquer, et porte son remède — `Activer` active les mods concernés et relit leur état. **Le bouton reste verrouillé tant qu'elle est là.** Les doublons comptent pour un. **Lancer une course ne modifie jamais la bibliothèque** dans le dos de l'utilisateur. Le plateau est lu depuis un état global (`gridMods.svelte.ts`), semé au démarrage depuis `launch_state.json`, pour que la garde le connaisse depuis n'importe quel écran.

**Support manette — choix du périphérique** (`gamepadDevices.svelte.ts`, `ControllerSetup.svelte`, `controller-onboarding-design.md`) : **un périphérique ne pilote l'interface que si l'utilisateur l'a désigné, une fois, explicitement ; sans réponse, il ne pilote rien.** `mapping === "standard"` est *déclaré*, pas vérifié : un volant en mode Xbox s'annonce standard, et ses pédales font dériver le focus. Démarrage, branchement à chaud et première installation sont le **même événement** — un périphérique visible sans décision —, et la décision est **par périphérique**.

- **Bandeau puis panneau**, rien ne s'ouvre seul : une notification persistante (`ControllerToast.svelte`, bleu) annonce le décompte après une courte attente qui laisse la rafale de branchements se calmer ; son `✕` vaut « plus tard ». Le panneau s'ouvre au clic (notification ou Réglages), prend la manette pour lui (`nav.inputCapture = "controller"`) et reste opérable à la souris et au clavier. Une ligne par périphérique, **sélection unique**, **aucune présélection** ; « Fermer » ne répond rien, « Aucun pour l'instant » clôt le sujet.
- **Identité** : `VID:PID` (`deviceKey`), jamais `Gamepad.index`. Adopter une entrée d'un volant multi-entrées marque ses sœurs (`deviceFamily`). Décisions dans `ui_prefs.json` (`pitbox.gamepad.devices`, lues par `peekUiPref`), coupe-circuit à part (`pitbox.gamepad.enabled`).
- **Résolution du profil** (`resolveProfile`), dans l'ordre : profil calibré sur la machine → profil livré (`DEVICE_OVERRIDES`) → layout standard si déclaré → rien, périphérique inerte.
- **Retour au neutre exigé** (`armDevice`) : un périphérique adopté ne produit rien avant d'avoir été **vu au repos**, à l'adoption et à chaque reconnexion. Le repos se **mesure** (un instantané stable), jamais ne se suppose ; une pédale maintenue garde le périphérique inerte.
- **Le panneau dit s'il faut calibrer** : « fonctionne sans calibration » (et pourquoi) ou « à calibrer : sans profil, ce périphérique ne pilote rien » (jaune, seul cas où le bouton devient l'action principale).
- **Calibration guidée** : repos, haut, bas, gauche, droite, valider, retour, puis les cinq raccourcis (§7.4bis) et le défilement rapide, une étape par écran, « Passer » et « Recommencer » toujours visibles, **Échap passe l'étape**. Chaque capture exige de la stabilité, le retour au repos avant l'étape suivante, et **refuse un doublon** ; hat ou stick se déduit pendant la capture. La fin montre un récapitulatif, **une zone d'essai**, puis « Copier le profil » et « Ouvrir un ticket pré-rempli » (rien d'identifiant).
- **Réglages › Général** : le coupe-circuit, la liste des périphériques connus (débranchés compris) avec la source de leur profil, utilisé ou non, `[Calibrer]` et `[Oublier]`.
- **Pièges du Gamepad API sous WebView2** : un périphérique **n'existe pas tant qu'on ne l'a pas touché**, d'où « appuyez sur un bouton pour qu'il apparaisse ici », jamais « aucun périphérique détecté » ; `getGamepads()` est relu à chaque image dans une boucle `requestAnimationFrame`, jamais conservé ; hors focus, la navigation gèle.

#### 7.4bis Raccourcis manette

**Cinq boutons au-delà du déplacement, tous optionnels** (`Action`, `gamepadProfile.ts`). Sur le layout standard : **LB/RB** = onglet précédent/suivant, **LT/RT** = mod précédent/suivant, **Start** = amener le curseur sur « Démarrer la session » (jamais lancer), **Y** = menu contextuel de l'élément ciblé (un vrai événement `contextmenu`). Front montant uniquement, jamais de répétition au maintien. **Sans onglets à l'écran, LB/RB changent de zone** (`data-gp-region`), et le curseur revient dans chaque zone là où on l'avait laissé.

**Ce que les raccourcis déclenchent appartient à l'écran** (`screenActions.ts`) : un registre en **pile**, où l'écran le plus récent répond ; `Tabs.svelte` s'y inscrit tout seul.

**Les panneaux flottants prennent la navigation à eux** (`data-gp-overlay` : popover d'un filtre, menu d'ajout, menu contextuel) : le curseur y entre, n'en sort pas tant qu'il est ouvert, et Annuler le referme (par un Échap synthétisé) en rendant le curseur à l'élément d'origine. La manette n'y entre d'autorité que si elle pilotait déjà. Les modales restent hors périmètre.

**Tout champ porteur d'une valeur se laisse « entrer »** (`needsEntry`) — liste déroulante, champ numérique et curseur : non entré, gauche/droite déplace le curseur ; entré, gauche/droite règle la valeur, et annuler (ou valider) en ressort, cette pression-là ne faisant rien d'autre. `.gp-editing` (anneau rouge) distingue l'entré du simple ciblage `.gp-focus` (jaune). **Le survol à la croix ne change jamais une valeur.**

**Défilement analogique** (`scrollAmount`) : le stick droit fait défiler le conteneur sous le curseur **sans déplacer le curseur**, à la vitesse de la poussée ; le conteneur se trouve en remontant depuis l'élément ciblé.

**Navigation par région** (`regionOf`/`bestCandidate`) : le déplacement cherche d'abord dans la même région (barre latérale ou contenu), et ne passe à l'autre que si la région courante n'a rien dans cette direction. Les lignes du tableau de bibliothèque sont focusables pour la manette.

**Répétition au maintien** des quatre directions (`shouldFire`), après un délai de décollage, puis plus vite après un long maintien ; un état par manette et par direction. Valider et annuler ne se répètent jamais.

**Une deuxième validation sur le même élément vaut double-clic** (`activate`) : valider deux fois une carte ouvre sa fiche ; le `click` part à chaque pression ; le compteur se remet à zéro dès que le curseur bouge.

**La fiche pleine page se navigue comme un écran** : la croix déplace le curseur **dans** la fiche, et le changement de mod a ses boutons dédiés. À l'ouverture, le curseur est posé sur la **vignette de skin sélectionnée**, une fois par fiche, et seulement si une manette pilote vraiment. **Au clavier** : Page préc./suiv. = mod précédent/suivant, flèches = déplacement du curseur (la même fonction que la croix) ; un champ et un curseur gardent leurs flèches.

---

## 7bis. Écran Compléments — l'inventaire (REFONTE§4)

**Tout ce qui n'est pas un contenu autonome, dans une seule liste** : livrées, sons, habillages de circuit, couches, mods « autres », et ce que Pit Box n'a pas su reconnaître. Voitures, circuits, mannequins et apps ont leur écran ; les mannequins figurent **aussi** ici, parce que l'écran Pilote sert à choisir et l'inventaire à gérer (désactiver, supprimer, ouvrir, annoter).

**Deux axes, indépendants du type** (`attach.rs`) :
- **le rattachement** — une voiture, un circuit, une app, le jeu, autonome —, déduit par force décroissante, **le signal voyageant avec la réponse** : chemin posé, hôte écrit dans la ligne, nom de config formé sur une entité, même archive. Corrigeable à la main ; seule la correction est stockée ;
- **la nature** — apparence, comportement, dépendance, non reconnu —, déduite des zones du jeu touchées (§7.3), la plus conséquente l'emportant.

**Facettes tri-état** (un clic inclut, un deuxième exclut), chaque valeur avec un compteur calculé sur la recherche, pas sur le résultat filtré. **Le contexte de recherche survit à l'écran** : champ, facettes, regroupement et tri sont dans `ui_prefs.json`, et une facette que le code ne connaît plus est écartée à la relecture.

**Une ligne à deux niveaux** (nom lisible, identifiant en dessous), **sans bouton exposé** : les actions sont dans le ⋮. Restent visibles le rattachement (un lien vers l'hôte), la nature, l'étoile de priorité, le marqueur de note, l'état. **Le type d'une ligne se dit par ce qu'elle touche** : Livrée, Son, Habillage, Couche, Mannequin, Document, et pour un mod « autre » ses zones du jeu (« Polices · Interface ») — « Mod » seulement quand aucune n'est reconnue.

**Une livrée a sa fiche** (`SkinDetail.svelte`) : ses fichiers, son poids, sa provenance, son nom déclaré, sa note, et **si le jeu la voit** — une livrée stockée mais non projetée (§8.3) se répare par la réparation générale. Le titre de la ligne y mène, le lien de rattachement mène à l'hôte. Une **couche** n'a pas de fiche à elle dans l'inventaire : le lien mène à son hôte.

**Groupement par archive ou par hôte.** L'écran Pilote annonce de son côté le nombre de `.kn5` qu'il a écartés.

---

## 8. Skins, sons, apps

### 8.1 Indexation du contenu de base

**La base Kunos est indexée en lecture seule** (`is_stock`), non désactivable, pour que skins et sons s'attachent à une voiture ou un circuit de base comme à un mod. Le nom d'un circuit multi-layouts s'y calcule comme partout (§5bis.3).

**L'index est (re)construit** à l'enregistrement de la configuration dès que le dossier du jeu est désigné ou change (`stock::needs_reindex`), et au démarrage si rien n'est indexé alors qu'un dossier est connu. « Indexer le contenu de base » en Maintenance force la reconstruction.

### 8.2 Mods installés hors Pit Box — « non géré »

Sur une install déjà moddée, les vrais dossiers de `content/` sont en majorité des mods posés à la main. **Le critère est la table du contenu officiel** (`kunos_content_dates.json`, `kunos_dates::is_official`) : un id qu'elle ne connaît pas est un mod — `is_unmanaged`, affiché « Non géré ». Une entrée qui manquerait à la table ne ferait que **déclasser** du contenu officiel en non géré, le sens sûr. Angle mort assumé : un mod qui écrase un dossier Kunos garde l'id officiel.

**`is_unmanaged` affine `is_stock`, il ne s'y substitue pas** : tout ce que `is_stock` protège ou résout s'applique encore (lecture depuis `content/`, refus d'activation, pas de date d'ajout). Ce qui change : pas d'auteur « Kunos » inventé, pas de « Jeu de base » en provenance, **aucune couche ni mise à jour** (§4.3), et un message de refus qui lui est propre.

**Pit Box n'adopte pas un mod en place.** Pour le faire passer sous gestion, l'utilisateur retire lui-même son dossier du jeu, puis importe le mod — ce que disent l'infobulle de la pastille, la fiche, les refus et le rapport d'import. L'app n'écrit pas dans un dossier qu'elle n'a pas posé.

**La reclassification se fait à la réindexation**, sur place : seul le drapeau bascule, sans toucher à ce que l'utilisateur a saisi.

### 8.3 Livrées et sons — des sous-éléments rattachés à une entité

**Skins — sélection, pas activation.** AC charge toutes les livrées d'un `skins/`. Seules actions : prévisualiser, et désigner le **skin piloté** pour le lancement. Import par l'import général, rattachement par le dossier `skins/<voiture>/`. **La miniature `livery.png`** est affichée quand elle existe : dans la liste déroulante de la colonne de session et en médaillon sur les vignettes de la grille de skins, jamais sur la grande photo. La vue Skins de l'inventaire permet la sélection multiple, et le regroupement par archive ou par voiture.

**Un skin stocké à part arrive dans le jeu en deux sauts** : la **projection** (une junction dans le `skins/` de l'entité cible), puis, pour un **mod géré**, le **redéploiement de l'hôte** (fin d'import d'un pack, retrait d'une livrée, recomposition d'un `skins/default/` de circuit) — `content/` n'étant qu'une copie en hardlinks de la bibliothèque. Pas de redéploiement pour le contenu de base, dont le dossier *est* celui du jeu. **Le déploiement suit les junctions de la bibliothèque** (les nôtres) : une junction n'est ni fichier ni dossier pour `symlink_metadata`, et serait sinon perdue en silence. Ce qui reste stocké sans être déployé est rattrapé par la réparation générale (§10).

**Sons — exclusifs** (un seul actif par voiture), vrai remplacement de `.bank` + `GUIDs.txt`, **original toujours restaurable** : copié une fois dans `<lib>/sounds/<voiture>/__original__` avant la première bascule. Pour un mod géré, c'est le `sfx/` de la **bibliothèque** qui change, et la voiture est redéployée si elle est dans le jeu ; pour une voiture Kunos, celui **du jeu**. **La bascule se fait d'un seul geste** : le nouveau son est préparé à côté (`sfx.pitbox-new`), puis échangé par deux renommages, la base écrite entre l'échange et le nettoyage ; un dossier de bascule retrouvé au démarrage fait remettre le son que dit la base. Ce n'est pas `gamebackup.rs` — choix assumé, `CHANTIERS.md`, « Son d'une voiture Kunos ».

**Écouter sans lancer le jeu** : chaque ligne du cadre « Son du moteur » porte une **clé de contact**. Exclusif, et **l'écoute ne déploie rien** : le bouton radio de la ligne active, la clé n'est qu'un lecteur. « Origine » lit la sauvegarde `__original__` dès qu'elle existe.

**Ce qui joue est le moteur audio du jeu** (`SPEC-engine-sound-fmod.md`) : les DLL FMOD de l'installation, l'événement `engine_ext` mélangé comme en course. La clé démarre le moteur quand le bank a un démarreur ; un **curseur de régime** va du ralenti au rupteur **de cette voiture**, lus dans sa physique (FMOD§4.4) ; un bouton **« Coups d'accélérateur »** fait jouer le moteur tout seul (FMOD§6bis) — toucher le curseur l'arrête, et pendant qu'il tourne le curseur s'efface. **Le son suit l'aperçu 3D** : l'angle d'écoute est celui de la caméra **dans le repère de la voiture**, plateau compris ; c'est le timbre qui change, pas la direction. Sans ce chemin (pas d'installation, DLL absentes, bank refusé), l'écoute retombe **silencieusement** sur le décodeur maison, une boucle de ralenti lue dans le `.bank`, sans curseur.

**Un mod de son a sa fiche** : la voiture visée, la taille, l'archive, la date, la clé d'écoute, les ressources, et **ce que le bank contient** (encodage, nombre d'échantillons, fréquence, durée, table des noms). L'**auteur** se saisit à la main (`sub_mods.author`) : aucun fichier ne le porte. Le format des banks est dans `fsb5-format.md` ; l'heuristique de ralenti du décodeur maison **ne doit pas être retouchée**, et Vorbis n'est pas décodé (le codec est nommé dans l'erreur).

### 8.4 Apps

**Type autonome, vue propre, activables** (par défaut dès l'import). Détection Python (`<id>/<id>.py`) et Lua/CSP (`<id>/<id>.lua`) ; activation par junction vers `apps/python/<id>` ou `apps/lua/<id>`. **Une app a sa fiche**, page pleine : nom, langage, archive, date, puis **ce qu'elle met sur le disque** — Ressources et Ajouts au jeu, les mêmes composants que pour un mod —, et ses couches. La liste ne garde que ce qui se décide sans ouvrir.

- **Une app possède des ajouts au jeu** (§4.5.3, `extras::OwnerKind::App`) : ce qu'une archive livre à côté de son dossier lui appartient, posé et retiré avec elle.
- **Pas de notion de mise à jour** : réimporter une app remplace intégralement ses fichiers.
- **Une app reçoit des couches** (§4.3) — des mods de contenu ajoutent des fichiers dans le dossier d'une app (le `.lua` de réglages d'une voiture RSS dans `RSS_Settings`, des caméras dans `CamTool_2`). **À l'import, c'est le chemin qui décide** (`apps::app_layer_target`) : un reste dont le chemin de jeu est `apps/<lang>/<AppId>/…` est une couche de cette app, testé **avant** la recherche de propriétaire, qui ne sert qu'à **nommer** la couche. **Une couche par app**, pas une par fichier, reconstituée relativement au dossier de l'app ; un reste **mixte** n'est pas touché.
- **Une couche est rangée sous le type de son hôte** (`<lib>/layers/<type>/<hôte>/<nom>`) ; une couche rangée avant ce segment garde son chemin, lu en base.
- **Une couche s'identifie par son hôte et l'espace de noms de celui-ci** (`overlay::LAYER_HOST`) : un circuit et une app peuvent porter le même id.
- **Déploiement : junction tant que l'app est nue, composition par hardlinks dès qu'une couche est active** (`compose::recompose`, point d'entrée unique pour les trois types d'hôte). « Active » veut dire junction **ou** arbre composé marqué (`apps::is_app_active`). **Le langage se lit toujours sur la base**, jamais sur le composé.
- **Une couche peut remplacer le script principal de l'app** : autorisé, mais **signalé**.
- **Reprise des bibliothèques existantes** (`extras::migrate_app_extras_to_layers`, au démarrage) : les ajouts au jeu rangés avant les couches d'app sont convertis, **de façon idempotente** et sans risque de perte (l'arbre des ajouts n'est touché qu'après un retrait réussi du jeu).

### 8.5 Accès transversal : l'inventaire des compléments

Skins, sons et couches sont trois valeurs de facette de l'inventaire (§7bis), regroupables **par hôte** ; l'accès par la fiche du mod n'a pas bougé. **Les skins de circuit fournis avec le mod** (reconnus dans `cm_skins/`, jamais importés à part) ne sont pas listés : rien de l'inventaire ne s'applique à eux, ils s'activent depuis la colonne de session ou la fiche du circuit. Les skins de voiture fournis avec le mod, eux, restent listés.

---

## 9. Lancement de session

**Dans `docs/SPEC-session.md`**, dont le code parle par l'étiquette `SESSION§` : la bibliothèque comme sélecteur (SESSION§1), le pilotage par preset Quick Drive (SESSION§2), l'écran de réglages avec le plateau et les conditions (SESSION§3), l'aperçu 3D (SESSION§4) et l'écran Pilote (SESSION§5).

## 10. Maintenance, export, nettoyage

**Relevé anonyme de la bibliothèque** (`survey.rs`, écran Maintenance) : le vrai moteur passé sur toute la bibliothèque, en lecture seule, dans un fichier JSON que l'utilisateur enregistre où il veut et **envoie lui-même** — rien ne part tout seul ; un bouton ouvre la page de ticket GitHub. Pour chaque mod : ce que son fichier dit et ce que les règles en font ; en résumé, les voitures qu'aucune famille ne prend, les tags qu'aucune règle ne connaît (hors ceux que le moteur écarte exprès et ceux qu'une famille prend), les pays sans drapeau, les marques et leurs orthographes, les variantes de logo et leur fond ; et les **décisions** de l'utilisateur sur les règles, que `rules-tool promote` sait intégrer. **Anonyme par construction** : jamais un chemin, un auteur, une note ni un tag saisi à la main. Il porte la version de l'app, celle du catalogue, sa date de génération (UTC) et sa source. **« Relever un dossier de mods… »** produit le même fichier depuis un dossier que Pit Box ne tient pas (le dossier `mods` de Mod Organizer), lu en place, voitures et circuits trouvés comme par l'import en masse (`modscan::scan`), une voiture livrée deux fois comptée une fois. Le relevé tourne **en arrière-plan**, suivi dans la pile de notifications.

**Transférer la bibliothèque** (`transfer/`, écran Maintenance et dernière étape de l'assistant ; **visible sous `tauri dev` seulement** tant que `FEATURE_LIBRARY_TRANSFER` n'est pas allumé) : un fichier `.pitbox` de quelques Mo, sans aucun fichier jouable ni chemin de la machine ; l'import le refait dans une installation vide, chaque mod en vitrine. Le détail est dans `SPEC-export-bibliotheque.md` (`EXPORT§`).

**Export d'archive autonome** : repackager un mod avec ses dépendances éparpillées (pilotes 3D, polices). Seule fonction qui lit le `data.acd` par QuickBMS, isolée dans le module d'export, jamais sur le chemin d'import ou d'activation.

**Nettoyage** : détection assistée des mods cassés (voitures sans `ui/`, circuits sans contenu valide, liens orphelins). Supprimer un mod cassé passe par la même confirmation qu'ailleurs, vitrine comprise. Un mod en vitrine n'est jamais listé comme cassé.

**Taille sur disque** : chaque version porte la somme réelle de ses fichiers, calculée à l'import. La colonne « Taille » cumule les versions d'un mod, l'historique montre celle de chaque version, la fiche d'un pack la somme de ses membres ; « — » tant qu'elle n'est pas connue. Un mod en vitrine garde la taille qu'il avait (ce que coûtera sa récupération). La réindexation la recalcule sur demande, case **décochée par défaut** (c'est lent).

**Activer / désactiver et supprimer sont deux axes distincts.**
- **Activer / désactiver** : « ce mod est-il déployé dans le jeu ? » — créer ou retirer ses hardlinks dans `content/`, ou recomposer (§4.3). Contenu **intact en bibliothèque**, réversible, ne libère rien.
- **Supprimer** : « ce mod doit-il encore occuper de la place ? ». **Une seule porte pour deux issues**, choisies dans la confirmation (ESPACE§5) :
  - **Garder en vitrine**, le choix par défaut pour une voiture ou un circuit : les fichiers lourds partent (corbeille d'abord), le mod reste avec son image, sa fiche technique, ses notes et son historique, dans un squelette de quelques dizaines de Ko en bibliothèque — jamais dans `content/`. Toutes ses versions passent en vitrine ensemble, avec ses couches, ses sous-éléments, ses ajouts au jeu et ses ressources (sauf les courtes notices) ; la confirmation les nomme. Un mod en vitrine porte la pastille « En vitrine », n'entre plus en session ni dans une grille, et sa fiche dit pourquoi l'aperçu 3D, l'écoute et l'activation manquent. **Ses fichiers se récupèrent** en réimportant son archive (l'import la reconnaît et la réhydrate), depuis l'archive conservée, ou depuis le registre de CM.
  - **Supprimer complètement** : fichiers de la bibliothèque et ligne du mod (désactivé au passage). Irréversible sans réimport, sauf archive source conservée.

  Les apps, « autres mods » et mannequins n'ont pas de vitrine.
- **Profils** : ensembles nommés activables en masse, qui capturent l'état actif des **trois** types activables (mods avec leur version, autres mods, apps — ces deux derniers dans `profile_extra_entries`). Ils servent aussi à resynchroniser une bibliothèque copiée sur une autre machine, aucun lien ne survivant à une copie.
- **Garde-fou** : vérifier junction ou hardlink contre vrai dossier avant toute suppression dans `content/`.

**Supprimer une version, pas le mod** : un bouton corbeille sur chaque version **autre que celle en place** dans la frise de la fiche.
- **La version installée n'est jamais supprimable** — ni bouton, ni backend (`errors.versionIsActive`).
- **Corbeille Windows d'abord** ; une version dépasse souvent le quota de la corbeille, donc la suppression définitive est le repli explicite, annoncé **avant** dans la confirmation, et l'issue réelle affichée **après**.
- **L'archive source part avec** sa version.
- **Les profils qui l'épinglaient sont repointés sur la version en place**, jamais vidés, et nommés dans la confirmation.

**Sauvegarde automatique de démarrage** (`backup.rs`, best-effort, silencieuse) : **une fois par semaine par défaut** (réglable : à chaque démarrage, une fois par jour, une fois par semaine), et **toujours au premier démarrage d'une nouvelle version**. Avant toute ouverture de la base, elle copie `overlay.sqlite` et les fichiers de préférences et de décisions de `app_config_dir` dans `app_config_dir/backups/<horodatage>/`, **plus les sessions enregistrées**, qui vivent chez Content Manager (SESSION§3.6). **La base n'est pas copiée, elle est lue** (`VACUUM INTO` depuis une connexion en lecture seule, après un `quick_check`) : la base est en WAL, et une copie du seul fichier perdrait le travail encore dans le journal. Une base illisible est sauvegardée brute avec son `-wal`, mais **ne déclenche pas la rotation**, pour ne jamais chasser les copies saines. Rotation sur les 7 plus récentes. Au démarrage, le `quick_check` et la recherche d'une **copie fantôme** du dossier de config sous une app empaquetée (MSIX, `shadowdir.rs`) écrivent chacun une ligne au journal. Pas d'écran de restauration : on recopie à la main depuis la sauvegarde la plus récente.

**Une version plus ancienne refuse une base qu'une plus récente a ouverte** (`version_guard.rs`) : elle perdrait en silence ce qu'elle ne connaît pas. La base retient le build le plus récent qui l'a ouverte, **daté par son commit** ; un build plus ancien s'arrête **avant tout** — ni sauvegarde, ni migration, ni écriture — sur une boîte de message dans la langue de l'utilisateur, qui nomme les deux versions. Un build qui ne peut pas se dater ne refuse rien et n'inscrit rien.

**Conservation de l'archive source** (réglage, désactivé par défaut) : l'archive d'un mod est gardée en bibliothèque, ce qui offre **« Réinstaller depuis l'archive source »** sur la fiche.

**Ce qui survit volontairement à la suppression complète d'un mod** : l'appariement Wikipédia (`wiki_link`), l'usage (`usage` — « déjà essayé », nombre de lancements ; le kilométrage, lui, vit chez CM), et les skins et sons rattachés (`sub_mods`, fichiers compris) — réimporter le parent les retrouve. Ils sont **listés en Maintenance** (« Skins et sons sans mod ») et nettoyés **sur décision**. Le reste (historique, journal d'import, médias rattachés à la main, fiche technique) part avec le mod.

**Réparation générale** (Maintenance) : **recalculer tout ce qui dérive de la bibliothèque** — `content/` en est une fonction pure. Deux étapes rejouables : (1) recréer les projections de skins manquantes ou cassées ; (2) **redéployer les mods actifs**, ajouts au jeu compris. **Un mod désactivé n'est jamais réactivé.** Une case optionnelle réinstalle depuis l'archive conservée tout mod cassé qui en a une. Les échecs sont listés un par un, avec leur raison brute. **Elle tourne en fond** : progression dans la pile de notifications, **comptée en octets**, avec une phase de pesée annoncée, un temps restant muet tant que la mesure ne vaut rien ; le compte rendu détaillé reste sur l'écran Maintenance.

**Journal fichier** (`tauri-plugin-log`, niveau Warn, `%APPDATA%\com.pitbox.app\logs\pitbox.log`) : le seul moyen de diagnostiquer, sur une install sans console, un échec best-effort qui ne bloque pas l'UI. Seulement des échecs réels.

---

## 11. Configuration et préférences

**Chemins requis** (assistant de première configuration, détection automatique quand c'est possible, `detect.rs`) : dossier d'install AC (bibliothèques Steam), bibliothèque, Content Manager (dans le dossier AC, dans `%LOCALAPPDATA%\AcTools Content Manager`, ou là où pointe le protocole `acmanager://`), 7-Zip (emplacements standard, sinon celui que CM embarque), QuickBMS + `acd.bms` (optionnels, export seulement). La bibliothèque a une **suggestion** pré-remplie, `<home>\PitBox Library` — jamais Documents, Bureau ou Images, que Windows synchronise souvent avec OneDrive.

**Quatre fichiers distincts** : la bibliothèque, la base d'overlay, les règles (catalogue embarqué et décisions, §7.1bis), et le fichier de config (chemins et préférences). `Prefs` est en `#[serde(default)]` : un champ ajouté prend son défaut, un champ retiré est ignoré.

**Préférences persistantes**, dont : tags bruts affichés, vue et colonnes (par type), presets de session (par type), décor de l'aperçu natif (SESSION§4), aperçu 3D affiché sur la fiche (défaut affiché), regroupement des skins, extraction des annexes (§4.5.2), conservation de l'archive source (défaut désactivé, §10), mode de déploiement (défaut hardlink, §2), zoom du mode Big Picture (§16 — vide, il reprend le zoom normal), enrichissement Wikipédia (`wiki_online`, défaut activé) et sa langue (automatique : celle de l'app, puis la chaîne de repli), vérification des mises à jour de mods (`mod_updates_online`, défaut activé, §4.7), surveillance Online en fond (défaut activée, `SPEC-play-online.md`).

**L'écran Réglages est à onglets** — Général / Chemins / Aperçu 3D / Vignettes (éteint, §7.4) / Musique / Wikipédia. Général et Chemins partagent la config et sa **garde de navigation** : quitter avec des changements non enregistrés demande quoi en faire. **Rien ne s'enregistre tout seul** dans Général, Chemins, Aperçu et Musique : chaque onglet a son bouton Enregistrer (Musique écrit son propre fichier). Seule exception, la bascule photo/3D de la fiche voiture, un interrupteur d'un clic. Les préférences d'import vivent au pied de `Fichiers › Importer` (§7.2quater).

**Onglet Wikipédia** : l'interrupteur et son motif, la langue de lecture, la purge du cache d'articles, et l'**export des corrections manuelles** — les seuls liens `manual`, jamais les `auto`, qu'un fichier livré figerait par-dessus tout moteur meilleur. Les seuils d'appariement ne sont pas à l'écran : ce sont des paramètres de `Prefs` qu'on règle par la mesure.

**Onglet Aperçu** : il porte son propre aperçu 3D (la voiture de la session, à défaut la première de la bibliothèque) et tous les réglages de l'aperçu intégré, en deux colonnes assignées — sous l'aperçu **Rendu**, **Éclairage**, **Sol** ; à sa droite **Cadrage**, puis **Cache**, seul bloc qui efface des fichiers, en dernier. Le détail des réglages, de leurs défauts (choisis par l'utilisateur pour le cadrage, calés sur le jeu pour la focale et l'angle) et de leur enregistrement est au PREVIEW§15.

---

## 12. Écran « À propos »

Atteint par la **dernière entrée du rail** (§7.2). Contenu :
- **Identité** : nom, version et date de build, une phrase de philosophie (non destructif).
- **Outils tiers** (Assetto Corsa, Content Manager, QuickBMS, FMOD) : description, auteur, lien, mention de **non-affiliation** par outil ; Content Manager marqué **requis**, QuickBMS **optionnel**. La mention FMOD est celle qu'exige sa licence (FMOD§3), en dur et non traduite.
- **Soutien et communauté** : lien PayPal (don libre), profil OverTake, dépôt source, « signaler un bug », journal des versions.
- **Licence** : Pit Box est **open source, sous GPL v3** — toute version dérivée distribuée doit rester sous GPL v3.
- **Bibliothèques open source** utilisées par Pit Box, liste repliable avec la licence de chacune, générée depuis `Cargo.toml` et `package.json`.
- **Bandeau légal** : non-affiliation, marque déposée (Assetto Corsa = Kunos Simulazioni), licence GPL v3.

## 13. Conventions

- **Langues** : le code est en anglais, la documentation et les échanges en français, et **aucune chaîne visible n'est en dur** : tout passe par l'i18n. Six langues livrées (français, anglais, italien, allemand, espagnol, portugais brésilien), choisies dans Réglages ou déduites du système ; `en` définit les clés et sert de repli, donc une clé absente d'une traduction s'affiche en anglais, jamais brute. `pt-BR` et `pt-PT` retombent sur `pt`.
- **Erreurs remontées à l'UI** : des **clés i18n** (`errors.*`, `errors.rs`), résolues par `errorText()` ; les erreurs techniques (E/S, SQLite) restent en texte brut.
- **Thème Rosso Corsa** : `#d40000` sur fonds sombres, coins carrés, police mono pour les données, logo « PITBOX » italique. Les jetons sont des variables CSS de `global.css` ; une maquette donne l'UX, jamais l'UI.
- **Logos officiels** : l'app lit `ui/badge.png` ; aucune maquette ne reproduit un logo de marque.
- **La coquille ne défile jamais, et c'est garanti** : `html` et `body` sont en `overflow: hidden`, mais le navigateur peut encore les faire défiler (focus, `scrollIntoView`) sans que la molette puisse revenir. `shellScroll.ts` remet d'aplomb après un geste risqué (`pinShell()`) et ramène à zéro tout défilement du document (`watchShellScroll()`). **On fait défiler le conteneur d'écran, jamais `scrollIntoView`** (`scrollIntoContainer`).
- **Un démarrage, un seul écran de chargement** : la fenêtre naît sombre, un écran de chargement en HTML pur couvre tout, et la page applique langue et zoom **avant** de monter la coquille, puis retire l'écran une fois la première image peinte (`shell/boot.ts`). L'écran prend l'inverse du zoom, et reste une zone de glisser de la fenêtre.

---

## 14. Références (fichiers du dossier docs/)

**L'index est `docs/README.md`**, et il est le seul. Un seul fichier de `docs/` est lu par l'application : **`kunos_content_dates.json`** (embarqué par `kunos_dates.rs`). **`docs/default-tag-rules-enriched.json` n'est pas ce que l'app charge** : c'est `src-tauri/rules/default-tag-rules.json`, le catalogue embarqué ; éditer celui de `docs/` ne change rien.

---

## 15. Points à vérifier

Les vérifications en conditions réelles qui restent sont dans `CHANTIERS.md`, « Points à vérifier en conditions réelles ».

---

## 16. Mode Big Picture et musique

Le bouton **Big Picture** de la barre de titre bascule la fenêtre en plein écran — l'interface habituelle, agrandie, pas une interface dédiée — et démarre l'ambiance musicale si elle est activée. **La barre de titre est masquée** ; la sortie est un bouton au pied du rail (§7.2) ou **Échap**. Un **zoom dédié** (`bigpicture_zoom`, §11) s'applique en plus du zoom normal.

### 16.1 Musique — périmètre retenu

Transposition de `spec-module-musique_2.md` (écrite pour C#/NAudio) vers Rust avec `rodio`, **noyau du module** :

- **Deux ambiances** : MENU pendant la navigation, GRID sur l'écran de réglages de la session ; fondu à puissance constante ; machine à états MENU/GRID/SESSION (`music/engine.rs`).
- **Coupée pendant une session, jamais baissée en fond** : la musique continue pendant l'écran de chargement et s'arrête **quand la voiture est réellement en piste** (§16.2), puis revient en fondu au retour.
- Choix d'un dossier par ambiance, écoute au clic, fichier de config séparé (`music.json`, jamais fusionné dans `config.json`).
- Normalisation RMS et cache d'index (§16.3).

**Un pack par défaut est embarqué** (`src-tauri/assets/music/`, deux pistes sous Pixabay Content License, crédits dans `CREDITS.md` et l'écran À propos) : c'est le comportement **sans configuration**. Le pack est déposé dans un dossier piloté par l'app (`app_config_dir/Music/embedded/{menu,grid}`), **réécrit à chaque démarrage**. La case « Utiliser mes propres dossiers de musique » (décochée par défaut) révèle les sélecteurs ; ces dossiers-là appartiennent à l'utilisateur et ne sont jamais réécrits.

**Écarté pour de bon** : la détection automatique des bandes-son Steam (MUSIQUE§3.2).

### 16.2 Détection de fin de chargement

`acs.exe` vit du début du chargement jusqu'au retour aux stands : sa présence ne dit pas si la voiture est pilotable. `music/ac_status.rs` lit la **mémoire partagée officielle d'AC** (`Local\acpmf_graphics`), sans disque : `Status` vaut `AC_LIVE` (2) seulement quand la voiture est en piste. `watch.rs` scrute la présence du process toutes les 500 ms, et, une fois le process vu, le statut toutes les 1000 ms. Le moteur reçoit deux paires de signaux : process lancé ou arrêté (un repère d'état, sans effet sur la lecture), session entrée ou quittée (le fondu réel, sur la transition `AC_LIVE`) — et **l'arrêt du process force la reprise de la musique** si la mémoire partagée n'a pas signalé la sortie.

### 16.3 Normalisation RMS + cache d'index

Au premier scan d'un dossier, chaque piste est décodée entière (`rodio`) pour une correction de gain vers −18 dBFS, bornée à ±12 dB, et sa durée exacte. Le résultat est mis en cache **dans le dossier** (`.pitbox-index.json`), invalidé si le nombre de fichiers ou la date du dossier change. **Toujours actif**, sans réglage ; le gain s'applique en plus du fondu. La durée exacte sert au préchargement du fondu enchaîné (§16.4).

### 16.4 Écarts assumés vs la spec d'origine

Documentés en tête de `engine.rs` :
- `rodio`/`cpal` mélangent et rééchantillonnent déjà : rien à rejouer de la chaîne NAudio.
- Sortie WASAPI **partagée**, jamais exclusive (elle couperait le son d'AC).
- **Préchargement** : le fondu enchaîné démarre `crossfade_ms + 500 ms` avant la fin quand la durée de la piste est connue (index RMS, ou format qui l'annonce), sinon quand la piste se termine.
- Chemins en absolu, pas en variables d'environnement.
- Le tag ReplayGain n'est pas lu : le calcul RMS s'applique toujours.

### 16.5 Interface

Réglages › **Musique** (`MusicTab.svelte`) : coupe-circuit, case « Utiliser mes propres dossiers », sélecteurs de dossier menu/grid (Parcourir, écoute ▶, nombre de pistes) quand elle est cochée, lecture aléatoire, volume, durée de fondu. Sauvegarde indépendante des autres onglets.

**Ce qui se règle depuis le mode s'applique dans le mode** : couper ou rallumer la musique depuis Big Picture l'arrête ou la relance sur place, et régler un zoom applique **celui des deux qui est à l'écran** (`applyZoomFor`).

**Écarts assumés** : l'option « baisser le volume en fond pendant une session » a été retirée — la musique s'arrête toujours ; le contrôle d'un lecteur média externe n'est pas implémenté.

### 16.6 Plein écran — filet de sécurité Windows

Sur une fenêtre sans décorations, `setFullscreen(true)` peut ne couvrir que la zone de travail : `bigpicture.svelte.ts` force les **bornes du moniteur** après l'appel, et restaure à la sortie la taille et la position d'avant. **Une fenêtre maximisée ne se restaure pas en lui rendant sa taille** : l'état maximisé est mémorisé à l'entrée et rétabli par `maximize()`, sans `setSize` ni `setPosition`.
