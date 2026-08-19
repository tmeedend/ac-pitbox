# Pit Box — Spécification de référence

> Gestionnaire de mods desktop pour Assetto Corsa. Remplace Mod Organizer 2, utilise Content Manager (CM) comme moteur de lancement.
> Application Tauri (frontend web + backend Rust), base SQLite. Thème Rosso Corsa.
> Ce document décrit l'application telle qu'elle fonctionne, par domaine. Il sert de référence de conception ; les consignes d'implémentation ponctuelles sont données séparément à Claude Code.

---

## 1. Principe directeur

Le cœur de l'app n'est pas « activer des mods » mais **résoudre l'identité d'un mod** (nouveau / mise à jour / doublon), indépendamment du nom de dossier. Tout le reste en découle.

L'app prend en charge tout le cycle de vie : importer (analyse, détection de type, rangement), identifier, organiser (tags harmonisés, recherche, filtres), activer/désactiver sans duplication (~300 Go de mods), lancer une session sans subir l'UI de CM, maintenir (mises à jour, historique, export, nettoyage).

---

## 2. Architecture

**Quatre couches** :
1. **Bibliothèque** — source de vérité des fichiers. Disque dédié (~300 Go). Tous les mods rangés proprement.
2. **Application** — import, identité, tags, activation, lancement.
3. **`content/` d'Assetto Corsa** — peuplé dynamiquement par des hardlinks par fichier vers la bibliothèque (`content/cars/<id>`, `content/tracks/<id>`).
4. **Content Manager** — moteur conservé (graphismes CSP/Sol/Pure, FFB, presets), invoqué par l'app pour lancer une session.

**Décisions structurantes** :
- **Activation par hardlinks par fichier** (comme Vortex) : zéro duplication (critique à 300 Go), instantané, pas de droits admin. Le `content/` d'AC est une projection, jamais l'original.
  - ⚠️ **Changement par rapport aux junctions de dossier** (`mklink /J`) : testées, elles échouent pour les circuits (AC/CSP semble filtrer ou mal traverser ce type de reparse point sur l'arborescence complexe d'un circuit — plusieurs layouts, `ai/`, `data/`, `extension/`). Les **symlinks** (`mklink /D`) fonctionnent mais exigent les droits admin/mode développeur. Solution retenue : **un hardlink par fichier**, à l'intérieur d'une vraie arborescence de dossiers dans `content/`. Un hardlink n'est **pas** un reparse point — le fichier lié est indiscernable d'un fichier normal pour AC, aucune indirection à mal interpréter. Ni droits admin, ni duplication.
  - **Contrainte** : les hardlinks exigent le **même volume** (contrairement aux junctions, qui peuvent traverser les disques). Chez l'utilisateur, bibliothèque et jeu sont sur le même disque. **Repli en copie physique** si bibliothèque et jeu se retrouvent sur des disques différents (détection automatique, comme pour le déplacement adaptatif à l'import, §4.2).
  - **Composition par couches (§4.3)** : entièrement en hardlinks elle aussi, y compris pour les fichiers qu'une couche écrase — pas de copie de fusion nécessaire, juste un hardlink vers le fichier réellement gagnant (base ou couche de plus haute priorité) à chaque chemin. Pas de dossier de composition intermédiaire : `content/<type>s/<id>` **est** directement le résultat composé.
  - **Implémenté et testé** (`src-tauri/src/deploy.rs`) : moteur générique de déploiement/composition par hardlinks, avec repli en copie physique par fichier si le hardlink échoue (disques différents). Garde-fou : un marqueur caché (`.pitbox-deployed.json`) à la racine de chaque dossier déployé, seule preuve qu'il a été créé par l'app (une arborescence de hardlinks est un vrai dossier, indiscernable d'un dossier Kunos par ses seuls attributs — contrairement à une junction/symlink, détectable par son type de reparse point). **Compat ascendante** : les mods déjà actifs sous l'ancien mécanisme (`mklink /D`) restent inoffensifs indéfiniment, migrés vers les hardlinks seulement à leur prochaine (ré)activation — jamais de migration forcée.
  - **Mode de déploiement au choix** (réglage `deploy_mode`, §11) : hardlinks (défaut, décrit ci-dessus) ou symlink (`mklink /D`, l'ancien mécanisme, redevenu un choix explicite plutôt qu'un vestige de compatibilité). Chaque mode a son prérequis, vérifié dans l'écran de réglages (`config::validate`) : hardlinks exige que le dossier Assetto Corsa et la bibliothèque soient sur le **même disque** (sinon repli silencieux en copie physique — techniquement fonctionnel mais double l'espace disque et recopie à chaque activation, donc traité comme un prérequis bloquant plutôt qu'un simple avertissement) ; symlink exige le **mode développeur Windows** activé, ou l'app lancée en administrateur (déconseillé — un lien dédié ouvre directement `ms-settings:developers`). Un mod à couche(s) active(s) (§4.3) reste **toujours** déployé par hardlinks quel que soit ce réglage : une junction/symlink ne peut pointer que vers une seule cible, elle ne peut pas fusionner base + couches — seule une base sans couche suit le mode choisi (`activation::deploy_base`).
- **Content Manager conservé** comme moteur + launcher : reproduire son moteur de config serait énorme et fragile. On contourne son UI, pas son moteur.
- **Stack Tauri** : binaire léger, Rust à l'aise avec les opérations filesystem/hardlinks/process, frontend web pour la richesse visuelle.
- **SQLite** : placée dans `app_data_dir` (pas en chemin relatif, pour survivre aux rebuilds).
- **Chemins de bibliothèque stockés relatifs, pas absolus** (`src-tauri/src/libpath.rs`) : `library_path` (versions, couches, sous-éléments, apps, autres mods) et `kept_archive_path` sont enregistrés **relatifs à la racine de bibliothèque**, jamais en chemin absolu figé sur la machine d'import. Sans ça, migrer la bibliothèque vers un autre disque ou un autre PC (robocopy + copie du dossier de config) laisse chaque ligne pointer vers un chemin qui n'existe plus, même quand tous les fichiers sont bien arrivés — la copie de fichiers ne suffit pas si les métadonnées restent figées. Un seul changement de `library_path` dans les Réglages suffit alors à tout refaire résoudre. **Compat ascendante** : une ligne écrite avant ce format reste en absolu, reconnue et utilisée telle quelle (`libpath::resolve`) — jamais cassée.

---

## 3. Modèle de données : overlay non destructif

**Le fichier `ui_car.json` / `ui_track.json` d'un mod n'est JAMAIS modifié.** Règle absolue. Réécrire le travail d'un moddeur casse les signatures d'intégrité et rend les modifications indissociables du mod.

**Deux sources de vérité séparées** :
- La **bibliothèque** = source de vérité des *fichiers* (contenu des mods, lecture seule).
- La **base d'overlay** (SQLite) = source de vérité des *métadonnées produites par l'app* : tags ajoutés/déduits, catégorie, année, specs complémentaires, favori, historique, profils, presets. Indexée sur l'empreinte du mod.

Le fichier du mod est une **entrée** du pipeline (lu), jamais une **sortie** (jamais écrit). Conséquence : désinstaller l'app laisse les mods intacts ; un badge « fichier du mod jamais modifié » rassure l'utilisateur.

**Entités** : Mod (identité stable, indépendante du nom de dossier), Tag (issu de l'ontologie), Profile (ensemble nommé de mods activés), HistoryEntry (événement horodaté), plus les sous-éléments et couches décrits plus bas.

**Historique d'un mod** : trace les événements avec le nom de l'archive/fichier source — « import initial », « mise à jour », « extension ajoutée ». **Ne trace PAS** les activations/désactivations (bruit sans valeur). Pas de compteur de nombre de mises à jour. Contenu de base Kunos exclu de cette frise (`is_stock`) : pas de vraie notion de version ni d'import à raconter, la fiche affiche une simple ligne « Contenu de base ».

---

## 4. Identité et import

### 4.1 Empreinte et résolution

Chaque mod a une **empreinte composite** stable. À l'import, l'app la compare à la bibliothèque :
- **Même identité** → mise à jour (voir §4.3 pour la distinction mise à jour / couche).
- **Match flou** (marque+nom proches, dossier différent) → demande explicite à l'utilisateur.
- **Aucun match** → nouvel import.

### 4.2 Sources d'import

Deux sources, même pipeline d'analyse/identité/tagging :
- **Archive** (`.zip`/`.rar`/`.7z`) — décompression puis analyse.
- **Dossier déjà décompressé** — analyse directe (cas clé : migrer un catalogue MO2 sans re-zipper).

**Import en masse** : un dossier parent dont chaque sous-dossier direct est un mod. Scan sur un seul niveau. Flux en deux temps : phase d'analyse (scan sans rien écrire → récapitulatif : nouveaux / mises à jour / doublons / ambigus / ignorés), puis arbitrage groupé des exceptions, puis exécution.

**Copier / déplacer** : réglage par défaut mémorisable (pas deux boutons à chaque fois). Déplacement **adaptatif** : même disque → rename instantané ; disques différents → copie puis suppression après vérification.

**Interface d'import** : glisser-déposer disponible partout ; écran d'import dédié pour les options (chaque option expliquée). Un mod importé est **activé par défaut** (déploiement par hardlinks immédiat) — de même pour une app (junction) et un « autre mod » (junction/lien fichier, §7.3). Le glisser-déposer accepte **archives et dossiers** : le tri est fait côté backend (`split_dropped_paths`), le webview ne pouvant pas distinguer un dossier d'un fichier à partir du seul chemin. Un seul lot tourne à la fois.

### 4.2bis Progression, estimation et annulation d'un lot

Pensé pour un lot de plusieurs dizaines de mods.

**Deux barres, cohérentes par construction.** La barre du haut suit l'item en cours, celle du bas le lot entier — cette dernière masquée quand le lot n'a qu'un item, où elle répéterait la première. La barre globale n'est **pas** comptée en nombre d'items (un skin de 3 Mo et un circuit de 2 Go pèsent alors pareil, et la barre avance par à-coups) mais en **secondes estimées** ; elle est recalculée à partir des avancements par item à chaque émission, donc elle contient la barre de l'item et ne peut ni la contredire ni la doubler. Un item en erreur ou ignoré est quand même consommé, sans quoi la barre n'atteindrait jamais sa fin.

**Estimation du temps restant.** Un benchmark persistant (`import_bench.json`, écriture synchrone côté Rust — §6.2) mesure le débit réel de la machine, par seau : extraction d'archive, rangement d'archive, copie de dossier, déplacement de dossier. Chaque seau amortit ses mesures (facteur 0,85), ce qui pondère naturellement un gros mod plus qu'un petit et fait oublier l'ancien disque après un déménagement de bibliothèque. Le benchmark ne fixe que les **poids relatifs** des items : l'échelle absolue est recalibrée en direct sur le temps réellement écoulé dans le lot en cours, donc une estimation fausse d'un facteur 2 converge après le premier item. L'ETA est affichée en unités grossières et lissée — un ordre de grandeur, pas une prédiction.

**Progression réelle pendant l'extraction.** 7-Zip est lancé avec `-bsp1` et sa sortie lue au fil de l'eau (mises à jour séparées par des retours chariot, pas des sauts de ligne). Un binaire trop ancien pour ce commutateur retombe sur une extraction sans progression, jamais sur un échec. Les événements sont plafonnés à 10/s : sans ça, quarante archives noieraient l'IPC.

**La fin d'un item n'est pas muette.** Après le dernier mod rangé, il reste les skins/sons rattachés, les apps et le balayage des restes (§7.3). Ces trois étapes se partagent la queue de la barre, chacune dans sa bande : les skins/sons en prennent la moitié et sont signalés **un par un** — un pack de deux cents livrées tient dans une seule entrée détectée, et c'est précisément là que la barre semblait bloquée. Une archive imbriquée annonce son nom pendant sa décompression, sans pourcentage : leur nombre n'étant pas connu d'avance, leur avancement ne se projette sur aucune part fiable de la barre, et une précision inventée serait pire que pas de précision.

**Le rangement d'un mod n'est pas opaque non plus.** Chaque mod occupe une part de la barre de son item, et sa progression interne est projetée dedans, octet par octet. Sans cela, un import de dossier ne contenant qu'un seul mod de plusieurs Go faisait passer la barre de son début à sa fin d'un bloc. Un `rename` sur le même volume ne signale rien : il est instantané, il n'y a pas de progression à montrer — la part se referme directement.

**Extraction et rangement en pipeline.** L'archive N+1 se décompresse pendant que la N se range — les deux saturent des ressources différentes. Le canal est un rendez-vous, ce qui borne l'avance à une archive et donc à deux dossiers temporaires vivants au plus.

**Verrou base réduit au rangement.** Extraction et copie de l'archive source, qui ne touchent pas la base, se font hors verrou — un écran qui lit l'overlay n'attend plus la décompression d'un gros circuit. Le rangement d'un mod, lui, garde le verrou : il entrelace décisions et écritures overlay, et le relâcher au milieu ouvrirait une fenêtre où l'UI pourrait modifier ce qu'on est en train d'écrire.

**Contrôle d'espace disque.** Un lot dont la taille dépasse l'espace libre du volume de la bibliothèque est refusé **avant** d'écrire quoi que ce soit. Jamais bloquant sur une information absente (bibliothèque non configurée, volume non interrogeable).

**Annulation.** Constatée **entre deux items** — et 7-Zip est tué s'il décompresse. Jamais au milieu du rangement d'un mod, qui laisserait une bibliothèque à moitié écrite. Le rapport affiche ce qui a été importé avant l'arrêt.

**Rapport de fin cliquable.** Chaque contenu importé ouvre sa fiche. Une **couche** ouvre le contenu de base auquel elle se rattache (§4.4). Skins et sons sont regroupés par contenu parent — une ligne par parent, pas par livrée. Apps et « autres mods » ouvrent leur écran. Un mod resté **ambigu** n'est pas cliquable : rien n'a encore été écrit. Ouvrir une fiche **replie** le rapport au lieu de le fermer — il recouvrirait la fiche qu'on vient d'ouvrir, mais on enchaîne souvent plusieurs mods d'un même lot, et le rapport fermé ne revenait par aucun chemin ; un clic sur son bandeau le redéploie, seul le `✕` le ferme. Le rapport survit à cette fermeture et reste consultable sur l'écran Import — un lot de plusieurs dizaines de mods ne doit pas disparaître sur un clic réflexe. En mémoire seulement : c'est le compte rendu d'une action, pas un réglage, et il n'a pas à survivre à un redémarrage.


**Pile de notifications** (`ToastStack.svelte`). Progression, rapports d'import et « nouveau périphérique » (§7.4) partagent une seule colonne en bas à droite, cadre commun (`Toast.svelte`) et une position définie à un seul endroit : deux cartes `position: fixed` épinglées au même coin ne s'empilent pas, elles se recouvrent — un second import cachait purement et simplement le rapport du premier. Les rapports s'**empilent** donc, le plus récent près du coin et seul déplié, les précédents réduits à leur bandeau-titre. Au-delà de trois, les plus anciens sortent : le dernier reste de toute façon sur l'écran Import. Les arbitrages qui attendent une réponse (§4.3, §4.4) restent des modales, pas des éléments de la pile.

### 4.3 Mise à jour vs couche (recomposition)

**Pas d'historique de versions conservé** (choix assumé pour la place disque). Une mise à jour remplace. Le filet de sécurité contre les pertes n'est pas le rollback mais le **modèle de couches** (la base reste toujours une entité intacte).

**Détection à l'import sur un contenu existant** : comparer les fichiers.
- Fort chevauchement des fichiers existants → **mise à jour** (remplace).
- Majorité de chemins nouveaux, peu de fichiers écrasés → **couche/extension** (ajoute).
- Détection **auto**, question à l'utilisateur **seulement si ambigu**, avec récapitulatif chiffré (« ajoute 84 fichiers, en écrase 6 sur 412 »).

**Reprise après arbitrage** : trancher un cas ambigu rejoue l'import de **ce mod-là uniquement**, depuis la seule source d'où il venait — pas le lot entier. Ses voisins ne sont pas retouchés, et ce qui suit les mods (skins/sons, apps, restes) n'est pas rejoué : tout cela a déjà été rangé au premier passage. La source est re-décompressée plutôt que gardée au chaud, pour qu'aucun dossier temporaire ne survive à l'import en attendant une réponse.

**Règle absolue** : le **contenu de base** (Kunos, `is_stock`) ne reçoit **jamais** de mise à jour, **toujours** une couche. Garantit par construction qu'il ne peut pas être perdu. Même une « version améliorée complète » d'un circuit Kunos devient une couche posée sur la base intacte.

**Modèle de couches recomposables** :
- La **base** (Kunos ou mod) reste une entité intacte, jamais fusionnée.
- Une **couche** (nouveau layout, améliorations, surcharge) est une entité séparée, intacte, rattachée à sa base.
- Ce que le jeu voit dans `content/` est un **résultat composé** : base + couches actives dans l'ordre de priorité.
- Désactiver/réordonner une couche = **recomposer** depuis les entités intactes (jamais de défaire chirurgical). Aucun état corrompu possible.

**Mécanisme, entièrement en hardlinks** (§2) :
- **Déploiement simple** pour les ~95 % de mods autonomes sans couche : hardlink direct de chaque fichier de la version active vers `content/<type>s/<id>`.
- **Composition** : même mécanisme, en superposant en plus chaque couche active (priorité croissante) sur la base — toujours un hardlink, jamais une copie de fusion. Retour au déploiement simple dès que la dernière couche est retirée.

**Contrôle** : ordre des couches modifiable + activation/désactivation par couche. Une couche peut se poser sur n'importe quel contenu (base ou mod).

### 4.4 Packs multi-voitures

Chaque voiture d'un pack est une **entité de premier niveau** (activable/tagguable séparément), liée aux autres par une métadonnée `source_pack` (nom d'archive/dossier, connu dès l'import). La fiche affiche un bloc « Source / origine » (pack cliquable, nom d'archive, URL d'origine si présente) et une section « autres voitures du même pack ». Actions : filtrer par pack, désinstaller le pack en lot. La rubrique « Provenance » de ce bloc s'adapte au type de contenu : nom d'archive pour un mod importé, **« Jeu de base »** ou nom du DLC (Dream Pack, Porsche Pack…) pour le contenu de base Kunos — résolu depuis `docs/kunos_content_dates.json` (`kunos_dates::pack_name`, même table que l'année/la date de publication estimées, §6.2).

### 4.5 Ce qu'un mod pose, et où

Une archive de mod contient **le dossier du mod** — celui que l'auteur a conçu pour être posé dans `content/` (`rss_gtm_lanzo_v8/`, `ks_nordschleife/`) — et, autour de lui, tout le reste : notices, templates, configs CSP, shaders, textures d'équipe, modèle de pilote. L'archive RSS GT-M Lanzo en compte 69 rien qu'en fichiers de jeu hors `content/cars/`.

Trois destinations, et une seule question pour choisir : **le fichier appartient-il au dossier du mod ?**

| Où il est | Ce que c'est | Où il va |
| --- | --- | --- |
| **Dans** le dossier du mod | Contenu du mod, à quelque profondeur que ce soit | Bibliothèque, intégralement — jamais trié (§4.5.1) |
| **À côté**, non lu par AC | Annexe : notice, template, changelog | `resources/` du mod, en bibliothèque (§4.5.2) |
| **À côté**, lu par AC ailleurs que dans `content/<type>/<id>` | Ajout au jeu | `extras/` en bibliothèque, posé dans AC à l'activation (§4.5.3) |

#### 4.5.1 Le dossier du mod est intouchable

**Rien n'est jamais retiré de l'intérieur du dossier du mod.** Tout ce qui est dedans est du contenu du mod : copié en bibliothèque intégralement, jamais trié. Le critère est l'**appartenance au mod**, jamais l'extension ni la profondeur — c'est en se fondant sur l'extension que `body_shadow.png`, `tyre_*_shadow.png` et `logo.png`, de vrais assets AC vivant à la racine du dossier voiture, ont été sortis de 23 mods. `scripts/audit-resources.ps1` audite et répare l'existant.

Le tri ne porte donc que sur ce qui **entoure** le dossier du mod : racine de l'archive, dossiers frères. Une annexe repérée *dedans* (PDF de notice livré au milieu de la voiture) est **signalée sur la fiche, jamais déplacée** : dans le doute, le fichier reste où l'auteur l'a mis.

#### 4.5.2 Annexes → `resources/` en bibliothèque

Beaucoup de mods embarquent des fichiers **hors contenu de jeu** : PDF de présentation, templates de skin (`.psd`), changelog/readme (`.txt`), archives de templates. AC ne les lit pas — ils ne doivent **jamais** aller dans `content/`. À l'import, ceux qui sont **à côté** du dossier du mod sont rangés dans un sous-dossier `resources/` du mod **dans la bibliothèque**. Le dossier Assetto reste propre, les annexes ne sont pas perdues.

**Réglage global** (préférence persistante, §11 — pas de question à chaque import) : **« Extraction des fichiers annexes »**, trois positions :

- **Aucun** — rien n'est extrait, les annexes restent dans l'archive/source, non copiées en bibliothèque.
- **Informations seulement** (défaut) — extrait uniquement les fichiers légers d'information : `.txt`, `.pdf`, `.md`, `.doc`/`.docx`, `.rtf`, `.nfo`, `.html`, `.url`, `.lnk`.
- **Tout** — ajoute les fichiers lourds : templates d'édition (`.psd`, `.xcf`, `.ai`), archives jointes (`.zip`/`.7z`/`.rar`), sources 3D (`.fbx`, `.blend`, `.3dsmax`), vidéos de présentation.

**Les images ne sont jamais des annexes**, à aucune profondeur et même à côté du mod : rien ne distingue une capture de présentation d'un asset AC (`logo.png`, `body_shadow.png`, `map.png`, aperçu de skin) — donc on ne tranche pas, on laisse. Une capture de présentation qui reste dans le mod ne coûte rien ; un `body_shadow.png` retiré casse le rendu.

**Une annexe restée dans le mod est listée, jamais déplacée.** La règle d'or (§4.5.1) interdit de sortir quoi que ce soit du dossier du mod : le `..._readme.txt` que l'auteur a posé à la racine de son circuit y reste. Mais l'onglet Ressources le **liste** quand même, marqué « dans le mod » et résolu contre le dossier du mod au lieu du dossier ressources. Mêmes extensions que le classement à l'import (documents d'information), **racine du dossier du mod seulement** — plus profond, un `.txt` fait presque toujours partie du contenu — et `GUIDs.txt` exclu comme partout ailleurs. Sans ça, deux mods identiques donnaient deux comportements selon que l'auteur avait livré sa notice à côté du dossier ou dedans.

**Prévisualisation dans l'onglet Ressources.** Un clic sur une annexe d'un format lisible l'ouvre **sous la liste**, dans la fiche : texte brut (`.txt`, `.nfo`, `.log`, `.ini`, `.cfg`, `.csv`, `.json`, `.yml`, `.lua`), markdown (`.md`), images (`.png`, `.jpg`, `.gif`, `.webp`, `.bmp`, `.avif`) et **PDF**. Tout autre format garde le comportement d'origine — ouverture par l'application par défaut de Windows, également accessible d'un bouton dédié (`↗`) sur les formats prévisualisables. Au-delà de 32 Mio, la prévisualisation refuse et renvoie sur cette ouverture externe plutôt que de faire transiter le fichier par l'IPC.

Le document n'a **ni hauteur imposée ni défilement propre** — et son bandeau (nom du fichier, fermeture) n'est pas épinglé non plus : il s'étend dans le flux et c'est la page de la fiche qui défile. C'est ce qui écarte l'`<iframe>` pour le PDF — la WebView y répondrait par la visionneuse d'Edge, application autonome avec sa barre d'outils et son défilement interne dans une boîte à hauteur fixe. Le PDF est donc rendu par **pdf.js**, page par page en `<canvas>` empilés sur toute la largeur disponible, re-rendus au redimensionnement.

Deux points de sûreté, tous deux côté backend, hérités de l'ouverture externe : le chemin relatif est **résolu et validé** (garde-fou anti-traversée) avant toute lecture, et le contenu remonte par une commande Tauri plutôt que par `asset://` — seules les images, servies dans un `<img>`, passent par le protocole. Le markdown est **échappé avant** production du moindre tag (rendu maison, pas de dépendance de parsing), et les liens d'un readme partent dans le navigateur du système : suivis dans la WebView, ils remplaceraient l'application par la page distante.

#### 4.5.3 Ajouts au jeu → `extras/` en bibliothèque, posés dans AC

Ce qu'AC lit ailleurs que dans `content/<type>/<id>` : configs CSP (`extension/config/cars/rss/<id>/…`), shaders (`system/shaders/…`), textures d'équipe (`content/texture/…`), modèle de pilote (`content/driver/…`).

**Stockés bruts, avec leur chemin relatif à la racine d'AC**, dans `<lib>/extras/<type>/<id>/…` — jamais dans la version, qui est déployée telle quelle dans `content/`. Au **niveau du mod** comme `resources/` : une mise à jour remplace ses propres fichiers, les couches partagent le même arbre.

**Le chemin d'archive n'est pas toujours un chemin de jeu.** Le balayage (§7.3) pose que le chemin d'un reste relatif à la racine de l'archive est son chemin relatif à la racine d'AC. C'est vrai la plupart du temps, et faux de deux façons — `acpath.rs` porte les deux règles :

- **Dossier de jeu livré à nu.** Un dossier `driver/` contenant un `.kn5` (à n'importe quelle profondeur) est le `content/driver/` d'AC : il est préfixé avant tout usage. Cas réel, la Ferrari 599 GTO livre `driver/driver_501.kn5` à côté du dossier de la voiture ; sans le préfixe le pilote atterrit dans `<AC>\driver\`, que le jeu ne lit pas. Une **seule** règle de ce type, volontairement : `weather/` et `sfx/` existent sous `content/` **et** sous `extension/`, on ne peut pas trancher sans regarder le contenu, et deviner mal pose des fichiers au mauvais endroit dans le jeu.
- **Emballage unique à la racine.** Un packageur enveloppe très souvent toute sa livraison dans un dossier à son nom (`NFS_TOURNAMENT_CLASS_A_2026-02-15/content/…`). `modscan` sait descendre cet emballage pour trouver les mods, et le balayage des restes doit s'accorder avec lui sur ce qu'est « la racine de l'archive » : sinon les voitures d'un pack s'installent pendant que ce qui les accompagne reste en bibliothèque, refusé comme chemin hors jeu. Bug réel : trois packs dont les `content/texture` et `content/fonts` n'ont jamais atteint AC alors que leurs voitures roulaient. `acpath::effective_root` traverse, avec **trois garde-fous** — jamais un dossier accompagné d'autres entrées (c'est un choix de l'auteur, `Optional - No ambient sounds/` à côté de son alternative, et en traverser un installerait une variante non choisie) ; jamais un dossier de jeu (`content/` seul *est* la racine, le traverser enverrait le contenu à `<AC>\cars\`) ; jamais un mod reconnu (une archive ne livrant qu'une voiture a elle aussi un dossier unique à sa racine, mais c'en est le contenu — descendre dedans ferait passer ses fichiers pour des restes et l'extraction des annexes le viderait, très exactement la règle d'or n°3).
- **Dossier d'emballage de l'auteur.** `Ferrari F2002 V1.4/`, `Track Installation/`, `Optional - No ambient sounds/` ne sont pas des chemins de jeu, et les poser revient à déverser un dossier d'archive à la racine de l'install. Un reste dont le premier segment n'est pas un dossier lu par AC (`content`, `system`, `extension`, `apps`, `cfg`, `launcher`, `sdk`, `server`, `plugins`, `mods`) n'est **jamais posé** — ni en ajout au jeu, ni en « autre mod ». Un fichier isolé à la racine est refusé pour la même raison : AC n'en lit pas, et l'exception qui vient à l'esprit (le `dwrite.dll` d'une install CSP) est précisément ce qu'un gestionnaire de mods ne pose pas tout seul.

Refuser ne jette rien : le fichier reste en bibliothèque et **reste listé** dans « Ajouts au jeu », marqué « hors chemin de jeu ». Un fichier listé qui n'arrive jamais dans le jeu sans qu'on dise pourquoi serait plus déroutant qu'un fichier absent.

**Certains chemins de jeu appartiennent à un autre outil.** `extension/config/tracks/loaded/`, `extension/config/cars/loaded/` et les `extension/vao-patches*/` sont la cible de synchronisation du téléchargeur de configs de Content Manager, alimenté par le dépôt `acc-extension-config`. Des archives de circuit y déposent pourtant leur config CSP — et ce n'est **pas la bonne pratique** : `loaded/` est le dernier des trois emplacements que CSP consulte, après `content/tracks/<id>/extension/ext_config.ini` (la place prévue pour un auteur, prioritaire et qui voyage avec le mod) puis `extension/config/tracks/<id>.ini`, et c'est précisément celui que la synchro écrase. L'hypothèse la plus charitable est que l'auteur vise un repli pour les utilisateurs sans mise à jour automatique ; la plus probable est un packaging distrait.

L'app **pose quand même** : arbitrer les choix de l'auteur n'est pas son rôle, et le mécanisme normal (arbitrage par date, §4.5.4) s'applique tel quel. Mais elle le **dit** — un ajout que Content Manager peut remplacer sans prévenir ne doit pas avoir l'air stable (§4.5.5). `acpath.rs` porte la liste des zones concernées.

Deux propriétés en découlent :

- **L'import ne jette rien.** Ce qui n'est pas classé est conservé tel quel, donc l'*interprétation* — où poser, qui arbitre un fichier partagé — reste recalculable depuis la bibliothèque à tout moment. Aucune règle des versions précédentes à mémoriser, aucune archive à conserver : c'est l'**entrée** qui est préservée, pas la décision. C'est ce qui rend un futur changement de règles rattrapable sans rien versionner.
- **L'ajout vit et meurt avec son mod.** Posé à l'activation, retiré à la désactivation, supprimé avec lui. Le passage par « autre mod » (§7.3) ne donnait pas ça : les fichiers d'une voiture supprimée restaient dans AC, rattachés à une entrée anonyme que plus rien ne reliait au mod.

**Les archives imbriquées passent avant leurs voisins.** Une archive imbriquée est extraite et reclassée (§7.3), et ce qui en sort entre dans la liste des propriétaires possibles **avant** que les fichiers qui l'entouraient ne soient arbitrés. C'est ce qui permet à une livraison `readme.txt` + `Car.zip` de ranger la notice dans les ressources de la voiture sortie du zip. Corollaire : **rien de reconnu à la racine ne signifie pas rien du tout** — tant qu'une archive imbriquée traîne quelque part dans la source, à n'importe quelle profondeur, on descend dedans avant de conclure. Sans cette descente, la source entière partait en « autre mod » : la voiture n'entrait jamais en bibliothèque et le `.zip` brut se retrouvait lié à la racine du dossier du jeu.

**Rattachement** d'un reste (§7.3), dans cet ordre : le chemin contient l'id d'exactement un mod reconnu de l'archive ; sinon l'archive ne livre qu'un seul mod, et tout ce qui l'entoure lui appartient. *Limite assumée* : dans un pack multi-mods, un reste que rien ne rattache reste un « autre mod » — le rattacher à tous dupliquerait des arbres parfois lourds, et « autre mod » ne perd rien. Un **document isolé** à la racine reste une annexe (§4.5.2) et va dans les ressources du mod, jamais dans AC : sans ce test, un `Read Me.pdf` deviendrait un ajout au jeu posé à la racine d'Assetto Corsa.

**Pose fichier par fichier** (hardlink), jamais par jonction de dossier : plusieurs mods visent les mêmes arbres (`extension/textures/common/rss/…` est livré à l'identique par chaque voiture RSS), et une jonction en donnerait la propriété exclusive au premier arrivé.

**`content/fonts` et `content/driver` ne sont pas un cas particulier** : ce sont des ajouts au jeu comme les autres. Ils ont eu leur propre mécanisme — copie globale dans l'install AC, jamais désactivée, écrasement par défaut en cas de collision — retiré pour trois raisons : il était déjà court-circuité (le balayage des restes, §7.3, les ramassait avant lui) ; il faisait cohabiter deux politiques contradictoires (« jamais désactivé » ici, « vit et meurt avec son mod » là) ; et son écrasement par défaut contredisait la règle d'or n°5. Le checksum anti-triche d'AC porte sur `data.acd` et `surfaces.ini`, pas sur les fonts/drivers.

#### 4.5.4 Poser sans écraser : réclamation, date, sauvegarde

Poser un fichier dans AC pose deux questions que `content/<type>/<id>` ne pose jamais : **plusieurs mods peuvent viser le même chemin**, et **ce chemin peut déjà être occupé** — par du contenu Kunos, par un mod installé hors de l'app, ou par un autre mod de la bibliothèque. Trois règles y répondent, et elles valent pour **les deux** mécanismes de pose : les ajouts au jeu (§4.5.3) et les mods « autres » (§7.3).

**1. Compteur de références.** Chaque mod *réclame* les chemins d'AC dont il a besoin (`extra_links`). Un fichier n'est retiré d'AC que lorsque plus aucun mod ne le réclame. Désactiver une voiture RSS n'emporte pas `extension/textures/common/rss/…` dont onze autres dépendent, et il n'y a plus de course à la propriété : le premier arrivé ne gagne rien.

**2. Arbitrage par date.** L'exemplaire à la **date de modification la plus récente** gagne, un mod plus récent corrigeant en général des bugs de celui d'avant. La date traverse la chaîne intacte : 7-Zip restitue celle stockée dans l'archive, `std::fs::copy` la conserve sous Windows, un hardlink partage l'entrée MFT. À égalité (archives repackées par un tiers, qui perdent les dates), c'est le **dernier mod installé**. L'arbitrage se rejoue dans les deux sens : quand le fournisseur s'en va, le fichier repasse à l'exemplaire du meilleur réclamant restant. **Un exemplaire plus ancien, ou de même date, ne déloge jamais ce qui tourne déjà** — sans cette comparaison, le dernier mod installé écraserait une font déjà mise à jour par un autre outil.

**3. Sauvegarde avant écriture.** Un fichier que **personne ne réclame** — contenu Kunos, mod posé à la main, reste d'une version antérieure de l'app ou de Content Manager — relève du même arbitrage, mais il n'est remplacé qu'après mise à l'abri de l'original, et il revient dès que plus aucun mod ne réclame le chemin. Il n'est en revanche **jamais supprimé** : personne ne l'ayant réclamé, rien ne dit qu'il est de trop. Un nettoyage éclairé des orphelins reste possible plus tard, une fois qu'on peut distinguer réclamé et non réclamé.

**4. On ne retire que ce qu'on a posé, et seulement si c'est encore là.** Deux vérifications distinctes, parce que deux choses peuvent mal tourner.

D'abord, seule l'**absence de réclamation en base** autorise une suppression. Ne pas savoir *résoudre* une réclamation — bibliothèque déplacée, exemplaire disparu, type illisible — n'est jamais une raison d'effacer : c'est la réclamation qui décide, pas notre capacité à la suivre. Bug réel corrigé par cette règle : le type était écrit `"tracks"` et relu comme `"Track"`, donc tout circuit était cherché dans l'arbre des voitures ; aucun réclamant trouvé, et les ajouts au jeu d'un circuit étaient posés puis **immédiatement effacés**. Les voitures passaient par hasard.

Ensuite, avant de retirer, on vérifie que le fichier posé est **encore celui qu'on a mis** — comparaison taille + date avec l'exemplaire de bibliothèque, ce qui couvre le hardlink (entrée MFT partagée, donc identiques par construction) comme le repli en copie. Si un outil externe a recréé le fichier depuis — Content Manager resynchronisant une config dans `loaded/` (§4.5.3) — on n'y touche pas. La règle d'or n°5 vaut dans les deux sens : supprimer un fichier qu'on n'a pas posé casse l'install de l'utilisateur, et aucun avertissement ne couvre ça.

Le fournisseur courant est mémorisé (`provided`), jamais déduit de la taille et de la date du fichier posé : c'est précisément dans le cas qu'on veut arbitrer — deux exemplaires de même date — que cette déduction se trompe. Pour la même raison, `kind` et `claimed_at` sont dupliqués depuis `mods` : une ligne doit se suffire à elle-même, une jointure vers une ligne manquante ferait disparaître la réclamation et l'arbitrage effacerait d'AC un fichier encore utile. Ce qui a été posé est mémorisé **fichiers et dossiers créés pour l'occasion séparément** : c'est la seule façon d'élaguer les dossiers vides sans risquer d'emporter un dossier d'AC préexistant devenu vide.

**Le remplacement, en détail.** Certains mods ne se contentent pas d'**ajouter** des fichiers : ils en **remplacent** — shader `system/shaders/…` modifié, config CSP qui écrase la stock, HUD façon CMRT qui remplace des images de `content/gui/`. Jusqu'ici l'app refusait, **et en silence** : la pose sautait le fichier sans laisser de trace, le mod s'installait à moitié et rien n'en informait l'utilisateur.

La règle d'or n°5 n'interdit pas de toucher un fichier du jeu : elle exige qu'il soit **sauvegardé et restauré**, et qu'un filet de sécurité rattrape les fermetures anormales. C'est ce que fait `gamebackup.rs`, en généralisant au fichier isolé la discipline déjà éprouvée sur les dossiers par `compose::recompose_stock` (§4.3) :

1. sauvegarde **avant** toute écriture, jamais l'inverse ;
2. vérification que la sauvegarde est lisible avant de toucher au jeu — sinon on ne remplace pas ;
3. la **première** sauvegarde fait foi : un second mod visant le même chemin ne sauvegarde pas la version du premier par-dessus l'originale, sinon la restauration rendrait un fichier de mod et l'original serait perdu ;
4. restauration dès que plus aucun mod ne réclame le chemin ;
5. au démarrage, restauration de toute sauvegarde que plus rien ne réclame — le filet pour une app tuée entre la sauvegarde et la pose, ou entre le retrait et la restauration. Ce filet compte les réclamations des **deux** mécanismes de pose : n'en regarder qu'un restaurerait un fichier qu'un mod actif utilise encore.

L'original vit dans `<lib>/game_backup/<chemin relatif à AC>`, la table `game_backups` fait le lien. Perdre la base ne perd donc pas l'original : le chemin de la sauvegarde dit à lui seul où le fichier doit revenir.

Le remplacement est fait **par défaut**, pas sur autorisation : c'est la réversibilité qui rend l'opération sûre, et un mod cassé en silence est pire qu'un mod installé et annoncé. L'annonce, elle, est obligatoire (§4.5.5).

#### 4.5.5 Ce que la fiche montre

Deux onglets frères sur la fiche pleine (`DetailPage`), **Ressources** et **Ajouts au jeu**, avec le même décompte et la même mécanique : liste **lue en direct sur disque**, jamais mémorisée en base. Deux conséquences valables pour les deux : un fichier déposé **manuellement** dans le dossier apparaît automatiquement ; et les **mods déjà installés** n'ont rien à réimporter — l'onglet se remplit dès que le dossier existe, y compris pour un mod importé avant que l'app ne suive ces fichiers. Seul l'**état de pose** des ajouts au jeu vient de la base.

- **Ressources** — un clic sur un fichier l'ouvre avec **l'application par défaut de l'OS** (PDF → lecteur, PSD → éditeur d'image). Un bouton **« ouvrir le dossier du mod »**, distinct, ouvre le dossier du mod dans l'explorateur.
- **Ajouts au jeu** — répond à « qu'est-ce que ce mod met chez moi en plus de son dossier ? », que rien ne montrait auparavant : un mod pouvait en poser 69 en silence. **Regroupé par dossier de destination** : quatre destinations disent tout de suite ce que le mod touche, là où 69 lignes plates sont illisibles. Chaque groupe est dépliable ; les fichiers **partagés** qu'un autre mod fournit sont signalés avec son nom ; les fichiers qui **remplacent** un fichier du jeu sont marqués en **rouge**, au niveau du fichier et du dossier — c'est l'annonce obligatoire du §4.5.4.

  Deux signalements de plus, au fichier comme au dossier, parce que le silence y était trompeur. **Zone Content Manager** (bleu, information) : le chemin est dans un dossier qu'un outil externe synchronise (§4.5.3) ; le survol dit que CM peut y remplacer la version du mod par la sienne et que ce n'est pas l'emplacement recommandé. Un bandeau reprend l'avertissement en tête de bloc dès qu'un fichier est concerné. **En attente** (jaune, alerte) : un fichier étranger — ni le nôtre, ni celui d'un autre mod, ni un fichier du jeu qu'on a remplacé — occupe déjà le chemin ; l'exemplaire du mod reste en bibliothèque et sera posé si l'autre disparaît. Ce dernier cas est le plus fréquent en zone Content Manager et il était totalement muet : les configs du dépôt CSP sont remises à jour en continu quand une archive porte la date de son packaging, donc **CM gagne presque toujours l'arbitrage par date**, et rien à l'écran ne disait pourquoi le fichier du mod n'arrivait pas.

Les deux onglets sont **absents du panneau latéral `ModDetail`** (§6) : les listes de fichiers vivent dans la page pleine.

### 4.6 Décider ou demander, et rendre compte

**Le critère : demander seulement quand l'information nécessaire est dans la tête de l'utilisateur, pas sur le disque.**

- Information déterminable en regardant le contenu → l'app **décide**, et rend compte. Jamais de question.
- Information de préférence ou d'intention (quelle variante d'un mod installer) → **demander** (§4.3 en est le précédent : détection auto, question seulement si ambigu, avec récapitulatif chiffré).
- Information nulle part → défaut **réversible**, et signalement visible.

Ce n'est pas de la frilosité : une question à laquelle l'utilisateur ne peut pas répondre correctement est **pire** qu'un défaut, parce qu'elle donne l'illusion du consentement sans en avoir la substance. Demander « ce mod livre un dossier `driver/`, dois-je le poser dans `content/driver` ? » revient à rendre à l'utilisateur le travail que l'app existe pour faire — et il y a une bonne réponse, déterminable. À l'inverse, ce qui a réellement coûté cher n'est jamais un défaut mal choisi : c'est un défaut **appliqué en silence**.

Deux contraintes encadrent toute nouvelle question : **jamais de blocage en import de masse** (un lot de cinquante mods avec douze questions devient un impôt qu'on paie en cliquant « défaut » douze fois), et **arbitrage groupé** en fin de lot. Les cas identifiés comme relevant vraiment de l'utilisateur : les **variantes offertes par l'auteur** (`Optional - No ambient sounds`, `Track Installation`, dossiers frères de même forme) et **deux versions du même mod dans une archive**.

**Journal des décisions.** Chaque arbitrage non trivial pris pendant un import est enregistré (`import_decisions` dans l'overlay) et relisible sur la fiche, bloc « Décisions d'import » — sous Provenance, effacé quand il n'y a rien à dire. Ne sont journalisées que les décisions **surprenantes** : chemin deviné (`pathNormalized`), chemin refusé (`pathRefused`), annexe non extraite (`ancillaryDropped`), reste qu'aucun mod ne réclame (`leftoverUnattached`). Rattacher `extension/` à la seule voiture d'une archive est la routine ; le noter à chaque fois noierait ce qu'on veut voir.

Le journal décrit le **dernier** import : il est effacé et réécrit à chaque passage, par mod **et** par archive — une archive réimportée à l'identique classe ses mods en doublons, ce qui saute leur écriture overlay, alors que le balayage des restes tourne quand même et réenregistre tout. Écriture best-effort : le journal explique l'import, il ne le conditionne pas.

#### 4.6bis Composants optionnels

**Le discriminant est le rayon d'action, pas le contenu.** Ce qui atterrit *chez le mod* (`apps/lua/<name>/`, `content/cars/<id>/`) s'installe sans rien demander : c'est la définition d'installer le mod. Ce qui atterrit *chez les autres* — un fichier du jeu de base, vu dans **toutes** les sessions et sur **toutes** les voitures — n'est plus le mod qui s'installe, c'est le jeu qu'on modifie.

**Détection : deux signaux, exigés ensemble.**

1. **Archive imbriquée** livrée à côté du mod. Personne ne zippe un sous-dossier de son propre mod par accident : un dossier sert à « une partie de la chose », une archive à « une chose qu'on peut vouloir ou non ». Signal structurel, pas sémantique — le nom du fichier n'est jamais lu.
2. **Elle remplace des fichiers du jeu de base** : le chemin existe dans AC, aucun mod ne le réclame, et ce n'est pas un exemplaire qu'on a soi-même posé (`others::game_files_replaced`).

Aucun des deux ne suffit, et c'est le cœur de la règle. Une archive imbriquée porte très souvent le **mod principal** — beaucoup d'auteurs livrent une racine réduite à un `readme.txt` et un zip. Et remplacer des fichiers du jeu est le quotidien de mods parfaitement obligatoires (shaders, fonts).

**Traitement** : importé et rangé en bibliothèque (l'import ne jette rien), mais **laissé inactif**, et la question posée en **fin de lot** dans le rapport d'import — jamais pendant, un lot de cinquante mods ne s'interrompt pas. Ne rien décider est une réponse valable : le composant reste activable depuis « Autres mods ».

C'est le seul cas rencontré jusqu'ici où **aucun des deux défauts n'est sûr**, et c'est ce qui justifie la question. Cas réel, `CMRT_Complete_hud` : l'archive jointe neutralise drapeaux et jauge de carburant du jeu (fichiers de ~1,7 Ko remplaçant des originaux de 10 à 60 Ko), parce que le HUD les redessine lui-même. L'installer en silence fait disparaître les drapeaux partout, y compris sur des voitures sans rapport ; ne pas l'installer peut faire doublonner l'affichage. L'auteur livre d'ailleurs les originaux en `*_backup.png` — son propre « annulez à la main », que `gamebackup` (§4.5.4) rend inutile. Ces `_backup` sont posés comme le reste : aucune heuristique de nom, la règle d'or n°3 a déjà coûté assez cher.

**Limite assumée** : cette règle ne couvre pas l'espace. Beaucoup de mods livrent des dossiers quelconques accompagnés d'instructions en prose (« copiez X dans Y si vous voulez Z »), et rien dans l'arborescence ne permet de les interpréter. Pour ceux-là, la réponse honnête n'est pas de deviner : c'est de rendre la notice **lisible** (§4.5.2, prévisualisation dans l'onglet Ressources) et de dire ce qu'on a fait (§4.6).

---

## 5. Tags et harmonisation

**Vocabulaire fermé (liste blanche)** : l'univers fini des tags autorisés (catégories `#gt3`/`#gte`/`#lmp1`, familles `prototype`/`endurance`/`vintage`, styles `#drift`/`#rally`/`#jdm`, propriétés CSP `rainfx`…). Tout tag entrant est mappé vers ce vocabulaire ou rejeté. C'est le vocabulaire borné qui harmonise, pas une meilleure détection.

**Trois origines de tags, tracées séparément** (permis par l'overlay non destructif) :
1. **Tags du mod** — lus dans `ui_*.json`, lecture seule, masquables.
2. **Tags déduits par règle** — calculés par l'ontologie, dans l'overlay.
3. **Tags manuels** — saisis, dans l'overlay, seuls directement supprimables.

Distinction par **code couleur** (rouge = catégorie, vert = règle, gris = manuel, bleu = fichier mod), légende discrète unique. Ordre : catégorie `#` → règle → manuel → fichier mod (en dernier, masquable).

**Ontologie de règles** (données, pas code — fichier `default-tag-rules-enriched.json`, éditable, versionnable). Familles :
- **Fusion** (synonymes → tag canonique), **Suppression** (bruit → rien), **Déduction** (tags implicites : `lmp1` → `#lmp1` + `prototype` + `endurance`), **Extraction** (tag → champ technique structuré), **brand_fix** (correction de marque depuis le nom).
- **Aperçu d'impact** : avant validation d'une règle, afficher le nombre de mods affectés.
- Écran graphique de gestion des règles.

**Comportement** : harmonisation automatique à l'import, édition manuelle à tout moment, détection CSP (lecture des `ext_config.ini` → `rainfx`, `grassfx`, `weatherfx`, `lightingfx`, `has-skins`), recherche/filtres par tag/marque/type/catégorie/année/auteur.

**Catégorie = tag `#`** (convention CM) : le tag préfixé `#` identifie la catégorie de la voiture. Sert à la **composition de plateau** (§7), combiné à la fenêtre d'années.

---

## 6. Fiche technique et champs dédiés

Les caractéristiques mécaniques ne sont **pas** des tags (un tag filtre/groupe, il ne décrit pas une fiche technique).

**Champs lus directement de `ui_car.json`** : `specs` (objet structuré : bhp, torque, weight, topspeed, acceleration, pwratio — pas de parsing), courbes `powerCurve`/`torqueCurve` (présentes ~100 %, à tracer), `description` (à la demande), `country`, `author`, `version`.

**Badge de marque** : `content/cars/<voiture>/ui/badge.png` (présent quasi partout, mod comme Kunos). Affiché sur fiches et vignettes. Source locale, pas de dépendance externe. Fallback (monogramme/générique) pour les rares voitures sans badge. **Pas d'icône d'auteur** (elle vient d'un pack externe communautaire, pas des fichiers du mod) : afficher le nom en texte.

**Champ `year` — résolution à trois niveaux** : (1) lire `year` de `ui_car.json` s'il est présent ; (2) sinon, pour le contenu de base, chercher dans la table statique `kunos_content_dates.json` ; (3) sinon « — ». L'app ne dépend pas de la base en ligne d'AcTools/CM.

**Champs structurés complémentaires** (overlay), remplis par la famille de règles Extraction, uniquement pour ce que `specs` ne couvre pas : `drivetrain` (RWD/FWD/AWD), `engine_pos` (FRONT/MID/REAR), `aspiration` (NA/TURBO/SUPERCHARGED), `engine_config` (V6/V8/…/ELECTRIC), `gearbox` (MANUAL/SEQUENTIAL/…). Une fois extraits, ces tags techniques sont retirés du vocabulaire. Même principe pour le **pays** (tag pays → champ `country` si vide, puis retrait du tag).

**Favori** : état personnel (cœur), ni tag ni caractéristique.

**Onglets de premier niveau** en haut de la fiche (`DetailPage.svelte` — la
page pleine, pas `ModDetail.svelte` qui reste un panneau compact avec son
propre menu clic-droit) : **Fiche · Screenshots · Replays · Resources ·
Ajouts au jeu · Backgrounds** (ce dernier affiché seulement pour un circuit).
Ressources et Ajouts au jeu vivent dans leur propre onglet plutôt que dans la
colonne de la fiche (§4.5.5). Les actions secondaires de l'en-tête (Activer/Désactiver, Exporter,
Réinstaller, Supprimer, Aperçu 3D, Ouvrir le dossier) sont regroupées dans un
menu **⋮** — seuls le cœur favori et le badge « Contenu de base » restent
visibles en permanence, hors du menu.

**Un seul curseur pour toute l'app** (`src/lib/components/Slider.svelte`) : réglages de session (dégâts, carburant, usure des pneus, heure), volume et fondu de Musique, cinq réglages de cadrage de l'aperçu 3D. Il y en avait quatre, tous faits main — les réglages de session dessinaient une poignée carrée rouge sur une piste de 3 px, Musique et Aperçu laissaient la poignée ronde du navigateur avec `accent-color`. Même contrôle, deux apparences. Le remplissage de la piste se calcule **dans** le composant, à partir des bornes : chaque appelant le recopiait à la main (`fuel_rate / 2` pour une échelle 0-200), donc une borne qui bouge laissait un remplissage faux. Le comportement manette « entrer dans le champ » (§7.4bis) vient avec, sans rien à déclarer : il porte sur le type `range` lui-même.

**Un seul composant d'onglets pour toute l'app** (`src/lib/components/Tabs.svelte`) : fiche détail, Réglages, Add-ons voiture/circuit et Règles de tags. Ils avaient chacun leur `.tabs` local — trois tailles de police, trois façons de marquer l'onglet actif, trois fonds. Le CSS Svelte étant scopé par composant, chaque copie dérivait de son côté sans que personne ne le voie : le mécanisme même qui a produit 53 signatures visuelles pour 68 libellés (§chantier libellés). Une variante `flush` (bande pleine largeur sur fond de carte) pour la fiche, qui occupe tout le cadre ; partout ailleurs la bande est transparente et porte elle-même son écart au contenu — une valeur de plus qui divergeait d'un écran à l'autre. Le composant s'inscrit tout seul auprès de `screenActions` (§7.4bis), ce qui rend tout écran à onglets parcourable à la manette sans une ligne de code de sa part.

**Chiffre entre parenthèses** sur Screenshots/Replays/Resources/Ajouts au jeu/Backgrounds
(ex. « Replays (3) ») dès qu'il est connu, pour savoir s'il y a quelque chose
avant de cliquer. Récupéré en tâche de fond à l'ouverture de la fiche (mêmes
appels que ceux faits à l'ouverture de chaque onglet) : aucun chiffre tant que
la réponse n'est pas là, jamais de blocage de l'affichage de la fiche pour
l'attendre. Backgrounds se recalcule aussi à chaque changement de layout
sélectionné (même filtrage que la sous-vue elle-même, §6.1).

### 6.1 Onglet Médias (fiche voiture/circuit)

Sous-vues **Screenshots**, **Replays** et **Backgrounds** (cette dernière
réservée aux circuits). Rattachement par simple **`nom_de_fichier.contains(id)`**
sur l'ensemble des id de la bibliothèque (voitures ∪ circuits, stock inclus) —
pas de découpage voiture/circuit dans le nom : les deux espaces de noms ne se
recoupent jamais (`content/cars/<id>` vs `content/tracks/<id>`), donc un id
trouvé dans le nom désigne sans ambiguïté la bonne entité. Un faux positif
occasionnel (id imbriqués, ex. « imola » contenu dans un mod
« rt_imola_historic ») est accepté : ces médias sont un agrément (§6.1), pas
une fonctionnalité critique — mieux vaut un média de trop qu'un rattachement
manqué. Repli : **association manuelle** (bouton « Associer un fichier… »,
dialogue de sélection natif) quand le rattachement automatique ne trouve rien
— stocké dans `overlay.sqlite` (table `media_links`), jamais écrit par le
matching automatique lui-même.

**Mise à la corbeille** (Screenshots et Replays uniquement — les Backgrounds
sont des fichiers posés par CSP, pas des médias de l'utilisateur) : bouton 🗑
sur chaque vignette/ligne et dans la visionneuse, plus la touche **Suppr** sur
l'élément focalisé (vignette, ligne de replay) et sur l'image affichée en
visionneuse. Le fichier part dans la **corbeille Windows**, jamais en
suppression définitive : `media::trash_file` s'appuie sur `IFileOperation` +
`FOFX_RECYCLEONDELETE`, qui échoue quand le recyclage est impossible (partage
réseau, fichier plus gros que le quota) au lieu de basculer silencieusement en
effacement définitif. Récupérable, donc **aucune confirmation** — une boîte de
dialogue par image rendrait le tri d'une galerie insupportable. Tout
rattachement manuel (`media_links`) pointant sur le fichier est retiré dans la
foulée, sinon la ligne survivrait au fichier et referait apparaître le média
disparu. Supprimer l'image affichée en visionneuse enchaîne sur la suivante
(sur la précédente pour la dernière de la liste), et ferme la visionneuse
quand il ne reste plus rien.

**Visionneuse plein écran** (`Lightbox.svelte`, générique aux deux galeries
Screenshots/Backgrounds) : clic sur une vignette pour l'ouvrir en grand,
précédent/suivant (boutons, flèches clavier, croix/stick manette), diaporama
(bouton lecture/pause, avance automatique toutes les 4 s), fermeture par le
bouton ✕ en haut à gauche, un clic sur le fond, Echap, ou le bouton B/annuler
manette. Pose `nav.inputCapture = "lightbox"` tant qu'elle est ouverte (même
drapeau que le panneau de périphérique, §7.4) : la navigation
manette globale et le précédent/suivant de mod de la fiche pleine page
(`Library.svelte::navigateFull`) cèdent gauche/droite/B pendant ce temps, pour
qu'une même pression n'agisse jamais à la fois sur la visionneuse et sur ce
qu'il y a en dessous.

**Miniatures mises en cache** (`src-tauri/src/thumbnails.rs`) : les captures AC sont en pleine résolution jeu — les grilles Screenshots/Backgrounds n'affichent jamais l'original, seulement une miniature JPEG générée au premier affichage puis persistée sur disque (`app_cache_dir()/thumbnails/`, clé = hash du chemin + date de modification + taille cible), réutilisée telle quelle même après redémarrage de l'app. Seule la visionneuse plein écran (Lightbox) charge l'image d'origine. Pas de politique d'éviction pour l'instant — le cache grossit avec les captures vues, jamais purgé automatiquement.

Formats réels vérifiés sur le poste avant implémentation (remplace la
convention supposée) :
- `Documents\Assetto Corsa\screens\Screenshot_<car_id>_<track_id>_<d>-<m>-<y>-<h>-<m>-<s>.jpg`
  (capture en session) et `Showroom_<car_id>_<d>-<m>-<yyyy>-<h>-<m>-<s>.jpg`
  (aperçu showroom, **pas de circuit** — le showroom n'a pas de piste). Le
  format de l'année dans le nom n'est pas uniforme selon le mode de capture
  (bug `tm_year` en session) : jamais parsé, seul le mtime du fichier est lu.
- `Documents\Assetto Corsa\replay\AC_<ddmmyy>-<hhmmss>_<type>_<car_id>_<track_id[_layout]>_<suffixe?>.acreplay`,
  `<type>` une lettre de session, suffixe final de longueur variable ou absent.
  `replay\temp\` (fichiers de travail) n'est jamais scanné.
- `<ac_install>\extension\backgrounds\<track_id>[__<layout_id>]_<variant>.jpg`
  (CSP) — convention propre, match par préfixe (double underscore avant le
  layout).
- **Replays — bouton « Lire dans CM »** (`launch.rs::launch_replay`) : passe le
  chemin du `.acreplay` en argument à l'exécutable Content Manager — même
  mécanisme que l'association de fichier Windows au double-clic, et cohérent
  avec la façon dont `launch()`/`open_content_manager` invoquent déjà CM (un
  argument passé directement à `Command::new`, jamais via le gestionnaire de
  protocole système). Pas de vérification empirique poussée (pas d'install AC
  sur le poste de développement) — à confirmer à l'usage.

### 6.2 Fond photo sur l'écran de réglages de session

Sur l'écran de réglages (§9.3), image de fond assombrie/floutée derrière
l'interface, avec ordre de repli :
1. Screenshot personnel du **combo exact** (même voiture + même circuit sélectionnés).
2. Screenshot personnel du **même circuit**, autre voiture (ambiance du lieu conservée).
3. **Background officiel** du circuit (§6.1).
4. Fond neutre actuel (aucun média disponible).


---

## 7. Bibliothèque et navigation

### 7.1 Deux bibliothèques distinctes

Voitures et circuits sont **deux bibliothèques séparées**, jamais mélangées : chacune a ses colonnes propres, persistées par type. Trois colonnes de dates : date d'ajout, date de mise à jour, date de publication (= date de modification des fichiers à l'import pour les mods ; pour le contenu de base, champ `release` de `kunos_content_dates.json`). **Date d'ajout et date de mise à jour absentes (`—`) pour le contenu de base** (`is_stock`, `MOD_SELECT` dans `overlay.rs`) : la date stockée en base pour ce contenu est l'instant du réindex, pas une vraie date d'ajout/MAJ — aucune source fiable n'existe pour ces deux dates côté contenu de base (le mtime du filesystem reflèterait seulement la date d'installation du jeu), donc mieux vaut l'absence explicite qu'une date affichée comme fiable alors qu'elle ne l'est pas. Seule la date de publication (`kunos_content_dates.json`, curatée) reste renseignée pour ce contenu. Distinction facile à manquer (deux dates propres à l'installation locale, une au mod lui-même) : une icône ⓘ sur chacun des trois en-têtes de colonne ouvre une info-bulle l'explicitant (`ColumnDef.tooltipKey` dans `columns.ts`, rendu dans `Library.svelte`).

**Filtre par tag** : champ texte libre (avec suggestions natives `<datalist>` des tags vus dans la bibliothèque), plusieurs tags séparés par des virgules — **ET** entre eux (ne remonte que les mods qui ont tous les tags saisis, pas au moins un). Toutes origines de tag confondues — fichier mod, règle, manuel — équivalentes pour filtrer, seule la fiche détail les distingue par origine.

**Catégories pour les circuits** : les circuits ont aussi des catégories (comme les voitures), pour filtrer et composer.

### 7.2 Barre latérale unifiée

Une **colonne latérale unique** (maquette de référence `pitbox-biblio-session2.html`) :
- **Bloc SESSION en haut** : previews du duo sélectionné (voiture + circuit), chacune cliquable pour ouvrir la bibliothèque correspondante — le bloc Session est le point d'accès aux bibliothèques (pas d'entrées « Voitures »/« Circuits » séparées). Bouton **« Démarrer une session »** qui ouvre l'écran de réglages à droite.
- **ADD-ONS** (titre rouge/mono) en deux colonnes : Skins | Sons, Apps, Autres mods.
- **ATELIER** (même style) : Règles | Importer, Réglages.

### 7.3 Type « Autres mods »

Mods de type non reconnu (shaders, configs CSP, mods d'UI, weather patterns…) : listés dans « Autres mods », activables/désactivables (hardlinks) comme les autres. Priorité notée + conflits signalés (pas de moteur de superposition type MO2). Chaque entrée a un bouton **« ouvrir le dossier »** vers son emplacement en bibliothèque — le chemin est résolu côté Rust depuis l'overlay, jamais reçu du front, ce qui permet de garder fermé le scope ACL du plugin `opener` (même rationale que `open_mod_folder`).

**Le signalement « zone Content Manager » vaut ici aussi** (§4.5.3) : le décompte des fichiers de l'entrée qui visent un dossier auto-synchronisé est affiché sur sa ligne, avec la même explication au survol que dans « Ajouts au jeu ». Ce n'est pas un doublon décoratif — c'est précisément ici qu'atterrissent les configs CSP d'un **pack multi-mods**, puisque rien ne les rattache à une voiture en particulier (voir le rattachement ci-dessous). N'avertir que dans « Ajouts au jeu » aurait laissé muet le cas le plus probable.

**Pas de notion de mise à jour.** Réimporter une archive dont l'id existe déjà en bibliothèque ne fait rien — ni remplacement, ni erreur, silencieusement ignoré. Pour reprendre un mod « autre » modifié, il faut d'abord le supprimer. *Conséquence à garder en tête* : une entrée mal rangée par une version antérieure de l'app ne se répare pas en réimportant, et l'utilisateur qui tente ce réflexe ne voit rien changer. Les corrections de rangement doivent donc s'appliquer **à la lecture** (activation, décompte des chemins) et pas seulement à l'import — c'est pourquoi la traversée de l'emballage (§4.5.3) est faite des deux côtés.

**Fichier isolé dans un dossier déjà réel côté AC** : posé par lien fichier (`mklink`, même mécanisme que les junctions de dossier) — ex. une nouvelle image dans `content/gui/flags/`, dossier qui existe déjà dans une install AC standard. **Un fichier déjà présent à cet emplacement est remplacé, plus sauté en silence** : l'original part en sauvegarde et revient à la désactivation (§4.5.4), et seul un exemplaire plus récent prend la place de ce qui tourne déjà. C'est ce qui manquait aux mods qui remplacent réellement du contenu — un mod façon CMRT visant `content/gui/` s'installait à moitié sans que rien ne l'indique.

**Rien n'est perdu même à côté d'un mod reconnu.** Un import n'est plus tout-ou-rien : si une archive contient une app (ou une voiture/circuit/skin/son) ET, à côté, du contenu non reconnu — cas des mods type CMRT qui livrent un dossier `apps/` et un zip séparé visant `content/gui/...` — ce reste est repéré et importé comme son propre « autre mod », plutôt que jeté au nettoyage du dossier temporaire. Un zip/7z/rar trouvé dans ce reste est extrait et reclassé récursivement (profondeur 2) avant de retomber, lui aussi, sur voiture/circuit/skin/son/app/autre mod si rien n'est reconnu dedans.

**Un reste = un id, dérivé de son chemin relatif à la racine balayée.** Deux invariants, chacun à l'origine d'une perte de données réelle (archive RSS GT-M Lanzo, qui livre une voiture plus `extension/`, `system/`, `content/texture` et `content/driver`) :

- **Le chemin relatif est conservé jusqu'au déploiement.** Un reste est stocké sous `others/<id>/<chemin relatif>`, et l'activation rejoue ce chemin depuis la racine d'AC. `content/driver` va donc bien à `AC\content\driver` — réduit à son seul nom de dossier, il atterrissait à `AC\driver`, hors de portée du jeu.
- **L'id porte ce chemin, et seules les extensions d'archive (`.zip`/`.7z`/`.rar`) en sont retirées.** Sinon tous les restes d'une même archive partagent un id : le premier est importé, les suivants rejetés comme déjà connus (§7.3, pas de mise à jour d'un mod « autre ») et leurs fichiers disparaissent au nettoyage du dossier temporaire. Un reste écarté pour id déjà connu est journalisé (`log::warn!`) — sans cette trace, la collision ne laissait aucun indice.

### 7.4 Vues et interactions

Deux vues commutables par bibliothèque (galerie / tableau). En vue tableau,
colonnes choisies, **réordonnables par glisser-déposer d'en-tête** (colonne
« Nom » fixe, jamais déplaçable) et **redimensionnables** par une poignée à la
jonction de deux en-têtes (glissé souris, ou flèches gauche/droite au clavier
une fois la poignée focus — double-clic/Entrée pour revenir à la largeur
naturelle). Le **libellé d'une colonne triable est un vrai bouton** : sans lui,
la seule chose focusable d'un en-tête était cette poignée de redimensionnement,
donc trier restait hors de portée à la manette et au clavier, et le repère de
sélection jaune posé sur un trait de quelques pixels ressemblait à une bordure
cassée. Le bouton ne porte aucun gestionnaire — le clic remonte au `<th>`, qui
trie déjà — et la poignée est retirée du parcours manette (`data-gp-skip`),
tout en restant focusable au clavier comme l'exige son motif WAI-ARIA.
Visibilité, ordre et largeurs persistés ensemble. Le tableau
lui-même s'élargit au besoin plutôt que de comprimer ses colonnes (`width:
max-content` sur la balise `<table>`, défilement horizontal du conteneur) —
sans ça, ajouter une colonne dans un tableau déjà chargé pouvait la rendre
quasi invisible au lieu de déclencher le défilement (bug réel constaté).
**Persistance** (`src-tauri/src/library_columns.rs`,
`app_config_dir/library_columns.json`) : même mécanisme que le duo de session
et les presets (§8.4/§8.6, voir plus bas) — fichier dédié écrit côté Rust,
pas `localStorage`, migration silencieuse depuis l'ancienne clé (visibilité
seule ; ordre et largeurs, fonctionnalités nouvelles, repartent toujours des
défauts lors de cette migration).

**Tous les autres petits réglages d'interface** (filtres, tri, vue
galerie/tableau, regroupement/tri de la vue transversale, mode copier/déplacer
à l'import, tags de fichier affichés, skin/layout préféré par mod) suivent la
même règle (§ CLAUDE.md, règle d'or n°5) : `app_config_dir/ui_prefs.json` via
`src/lib/uiPrefs.svelte.ts`, jamais `localStorage`. Migration en bloc (toutes
les clés `pitbox.*` encore en `localStorage`, hors celles qui vivent déjà dans
un fichier Rust dédié) au premier démarrage après la mise à jour, pas une clé
à la fois.

**Sélection** :
- **1 clic** = sélectionne (affiche dans le panneau de droite ET définit comme voiture/circuit de session).
- **Double-clic** = ouvre la fiche détaillée (où l'on choisit le skin piloté).
- **Skin piloté persistant** : mémorisé pour la voiture, affiché sur la vignette, rappelé dans le bloc Session.

**Sélection multiple** : Ctrl-clic (bascule un mod), Maj-clic (plage dans l'ordre affiché), **Ctrl+A** (tout ce qui est **affiché** — filtres et recherche courants, jamais la bibliothèque entière : c'est le geste attendu après un filtre précis, et c'est aussi ce qui borne la casse). Ctrl+A dans le champ de recherche garde son sens habituel, sélectionner le texte.

**Le menu contextuel agit sur la sélection** : clic droit sur un mod qui en fait partie, et l'action porte sur tout le lot, décompte écrit dans le libellé (« Supprimer les 12 mods ») — sans ce décompte, la même phrase désignerait deux gestes dont l'un est irréversible. Clic droit **hors** sélection : convention des gestionnaires de fichiers, la sélection revient au seul mod visé. Le contenu de base est écarté des actions qui ne le concernent pas (activer, exporter, supprimer) plutôt que de faire échouer le lot ligne par ligne ; « ouvrir la fiche » et « ouvrir le dossier » restent réservés au mod unique (douze explorateurs d'un clic est hostile).

**Panneau en bas, en surimpression** par-dessus la grille dès deux mods sélectionnés (le panneau de droite continue d'afficher le dernier mod cliqué ; le layout de la grille ne bouge pas en largeur). Il ne garde que **ce qu'un menu ne peut pas porter** : un champ de saisie (catégorie, tag) et une paire de boutons sans argument (favori). Activation, suppression, export et adversaires sont partis au clic droit — deux endroits pour la même action, c'était un endroit de trop pour la chercher. Les champs propres à une voiture (specs, skin piloté) ne sont pas proposés en masse.

**Les lots qui touchent au disque sont asynchrones et rendent des comptes** (§6.3bis). Activer, désactiver, supprimer, exporter passent par `async` + `spawn_blocking` comme l'import et pour la même raison (§4.2) : une commande Tauri synchrone s'exécute sur le thread principal, donc supprimer quarante circuits y gelait la boucle d'événements — plus aucun `invoke` ne répondait, et les événements de progression ne seraient de toute façon partis qu'à la fin. Progression et rapport s'affichent dans la **pile de notifications** (§4.2bis), pas dans le panneau : un rapport enfermé dans le panneau partait avec lui, alors que c'est le seul endroit où est écrit ce qui n'a pas marché. Un bouton **Arrêter** interrompt le lot **entre deux mods** — jamais au milieu de l'un d'eux, qui laisserait une junction à moitié posée — et le rapport le dit (`cancelled`), sans quoi il se lirait comme un lot complet dont la moitié aurait disparu. Favori, catégorie et tags restent synchrones : quelques écritures SQLite, où une barre ne serait qu'un clignotement.

- Quand plusieurs **véhicules** sont sélectionnés, deux actions du menu : **« Définir en tant qu'adversaires »** (vide la liste d'adversaires puis ajoute la sélection) et **« Ajouter en tant qu'adversaires »** (ajoute à la liste existante). Les deux basculent le mode adversaires de la session Course sur **« Libre »** ; si on était sur « même voiture » ou « même catégorie », les adversaires de ces modes sont récupérés dans « Libre » en plus de la sélection.
- Même paire d'actions pour une **seule** voiture (« Définir comme adversaire » / « Ajouter comme adversaire »), comportement identique.

**Suivi d'usage** : distance parcourue par voiture/circuit ; filtre « jamais essayé » (0 km CM **et** jamais lancé via l'app, l'app tenant son propre marqueur fiable).

**Filtre « Cacher le contenu de base »** : exclut le contenu Kunos (`is_stock`) de la liste, cases favoris/jamais essayé/contenu de base regroupées et alignées verticalement dans la barre de filtres.

**Fourchette d'année (voitures) : vide par défaut, et vide veut dire « aucune borne »** — c'est aussi l'état où « Réinitialiser » les ramène. Une borne absente ne filtre rien et ne compte pas dans le badge de filtres actifs ; seule une valeur saisie filtre. Le défaut était auparavant `1950`/année courante, ce qui affichait deux bornes que l'utilisateur n'avait pas demandées et ne pouvait pas effacer : vider le champ le ramenait aussitôt à sa borne. Pire, les deux champs se bornant l'un l'autre, vider « année max » l'écrasait à `1950` et ne laissait plus **rien** remonter. `NumberStepper` porte donc une prop `emptyValue` : la valeur-sentinelle qui s'affiche comme un champ vide, échappe volontairement aux bornes (sinon `min` la ramènerait dans la plage — c'est le bug lui-même), et que « vider le champ » rétablit. Une sentinelle plutôt qu'un `null` : `value` reste un `number` pour tous les autres appelants, qui n'ont aucune raison de devenir nullables. Le champ **resynchronise aussi le DOM après coup** : Svelte ne réécrit l'attribut que si la valeur liée a changé, si bien qu'une saisie hors bornes — ou un deuxième vidage — restait affichée en contradiction avec l'état réel (symptôme rapporté : vider une première fois écrivait `1950`, vider une seconde fois laissait le champ vide alors que le filtre valait toujours `1950`). Un filtre enregistré avant ce changement portait les bornes de la plage comme sentinelle : elles sont relues comme « vide », ce qu'elles ont toujours voulu dire.

**Ni ▲ ni ▼ ne se désactive au prétexte que le champ est vide, et les deux flèches partent du même repère** (`emptyStart` de `NumberStepper`) — le même repère quel que soit le sens, comme taper directement cette valeur. Sans lui, ▲ retombait sur `min` même pour un champ dont le point de départ naturel n'est pas sa borne minimale (« année max » : l'année courante), et ▼ n'avait tout simplement aucune destination définie depuis « vide », d'où sa désactivation forcée. « Année min » part de `1950`, « année max » de l'année courante — dans les deux sens : un appui sur ▲ depuis « année max » vide affiche l'année courante, un second l'année suivante, exactement comme un appui sur ▼ affiche l'année courante puis l'année précédente. **Aucune des deux bornes n'a de plancher ni de plafond réel** — `1950`/l'année courante ne sont que des points de départ (`emptyStart`), jamais des `min`/`max` : des voitures existent bien avant 1950 (retour utilisateur direct — une première version bornait `min` à `1950`, empêchant d'aller plus bas une fois qu'on y était arrivé), et un mod peut légitimement porter une année future (voiture concept, DLC annoncé). Le plafond de « année min » suit seulement, dynamiquement, « année max » quand elle est renseignée (une borne ne doit pas dépasser l'autre), sans repli sur une constante quand elle ne l'est pas. Même correctif pour la fourchette d'année du vivier d'adversaires (`OpponentsBlock.svelte`, §8.6) : son plafond à l'année courante grisait ▲ dès qu'on l'atteignait, pour la même raison.

**Colonne « État » et pastille d'état** (`StateBadge.svelte`, partagé entre le tableau de bibliothèque et la fiche détail — c'est la même information, elle doit se lire pareil aux deux endroits). Trois états, la couleur portant la distinction et le libellé l'état : **vert = actif**, **orange = inactif**, **bleu = contenu de base Kunos** (toujours présent dans le jeu, il ne s'active ni ne se désactive — d'où une couleur à lui plutôt que le vert des mods qu'on a soi-même déployés, avec la même infobulle que le badge des vignettes), libellé **« De base »** plutôt que « Actif » — c'est vrai techniquement (`c.active` vaut aussi vrai pour lui, d'ailleurs le filtre « Actif » remonte le contenu de base sans qu'on y touche ici) mais ce n'est pas l'information que la pastille doit donner. Le tableau affichait auparavant un tiret pour « inactif » — une absence, là où l'utilisateur cherche un état — et rien n'y distinguait le contenu de base d'un mod actif. **Le tri et le filtre d'état ne changent pas** : ils restent sur `c.active`/`c.is_stock` directement, indépendants de l'affichage. Sur la **fiche détail**, cette pastille est posée à droite de la bande d'onglets (emplacement `trailing` de `Tabs.svelte`, donc alignée sur les onglets par construction) : c'est la première chose qu'on vient y vérifier, et elle n'était lisible qu'en ouvrant le menu ⋮, dont le libellé Activer/Désactiver était le seul indice.

**Persistance du duo de session** (`src-tauri/src/session_state.rs`, `app_config_dir/session.json`) : fichier écrit côté Rust, pas `localStorage` du webview. `localStorage` n'est pas garanti synchrone sur disque côté WebView2 — bug réel constaté : le circuit, typiquement choisi juste avant de fermer l'app, ne survivait presque jamais à un redémarrage, contrairement à la voiture (choisie plus tôt, le temps d'être vidangée sur disque). `std::fs::write` est synchrone : la commande `save_session_picks` ne rend la main qu'une fois réellement écrit. Migration silencieuse au premier démarrage après la mise à jour : si le nouveau fichier n'a rien pour une entité, `nav.svelte.ts` relit une dernière fois l'ancienne clé `localStorage` et la re-persiste aussitôt au nouvel endroit.

**Garde-fou activation au lancement** (`AppShell.svelte`) : lancer une session avec une voiture ou un circuit sélectionné mais non activé (jamais junctionné dans `content/`) fait planter Content Manager/AC, qui ne trouve pas le contenu — bug réel signalé. L'état d'activation du duo n'est jamais déduit de `SessionPick` (juste id/nom/preview pour l'affichage, persisté tel quel — une donnée d'activation qui y serait figée resterait fausse dès que l'état change ailleurs, ex. désactivé depuis la fiche détail) mais interrogé à chaque changement de sélection via `get_mod_detail`, comme `trackDetail` pour le sélecteur de layout. Icône ⚠ (jaune, `title` natif) sur le nom du slot concerné dans la barre latérale tant qu'il n'est pas activé. Cliquer « Démarrer la session » avec un duo non activé bloque le lancement, demande confirmation, active le(s) mod(s) concerné(s) puis ne poursuit vers l'écran de réglages (`nav.autoLaunch`) qu'une fois l'activation réussie — jamais d'activation silencieuse sans accord explicite, jamais de lancement si elle échoue.

**Support manette — choix du périphérique** (`src/lib/gamepadDevices.svelte.ts`, `ControllerSetup.svelte`) : **un périphérique ne pilote l'interface que si l'utilisateur l'a désigné, une fois, explicitement ; sans réponse, il ne pilote rien.** `mapping === "standard"` est *déclaré* par le périphérique, pas vérifié : un volant en « mode Xbox » ou derrière un adaptateur XInput s'annonce standard, et le layout Xbox place « haut/bas » sur l'axe 1 — sur un volant, c'est une pédale (bug réel : des éléments d'interface se déplaçaient seuls, volant branché ; effleurer le frein faisait défiler le focus, sans rien à l'écran pour l'expliquer). Défaut fermé, donc : un périphérique muet se diagnostique, un focus qui dérive n'a aucun recours évident. Démarrage, branchement à chaud et première installation sont **le même événement** — un périphérique visible sans décision enregistrée — donc un seul chemin de code, et pas d'étape dans le `SetupWizard` (à la première installation personne n'a encore touché son volant : la liste serait vide, l'étape ressemblerait à un écran cassé). La décision est **par périphérique**, jamais globale.

**Bandeau puis panneau** : rien ne s'ouvre tout seul — un modal ne se justifie que si l'app ne peut pas continuer sans réponse, et ici elle le peut ; on branche d'ailleurs un volant *juste avant* de lancer une session, où une popup arriverait au pire moment. Une notification de la pile bas-droite (`ControllerToast.svelte`, bleu = information) annonce le décompte ; elle est **persistante** — elle ne s'évanouit pas toute seule, qui n'a pas eu le temps de lire garde son chemin vers le panneau — son `✕` vaut « plus tard » et jamais refus, et il attend ~1 s que la rafale de branchements se calme pour que le décompte soit juste (un rig complet énumère six entrées en quelques centaines de ms). Le panneau `ControllerSetup.svelte` s'ouvre **au clic** (notification ou Réglages), pose `nav.inputCapture = "controller"` — même mécanisme que la visionneuse, pas un drapeau parallèle — et reste intégralement opérable souris/clavier : c'est un panneau au sujet d'un périphérique qui ne marche peut-être pas. Une ligne par périphérique (nom, `VID:PID · n axes · n boutons`, badge Reconnu / Manette standard / ⚠ Non reconnu), **sélection unique** — la question est « lequel utiliser », et les lignes non retenues sont marquées répondues, donc le rig complet se règle en un geste. **Aucune sélection par défaut** (personne ne valide par réflexe un panneau qui contient déjà une réponse), et « Fermer » (ne répond rien, la notification revient) n'est pas « Aucun pour l'instant » (clôt le sujet).

**Identité et persistance des décisions** : `Gamepad.index` est un slot réattribué au débranchement — jamais persisté, jamais utilisé comme clé. La clé est `VID:PID` (`deviceKey`), sinon l'`id` brut normalisé ; deux manettes XInput identiques la partagent donc, sans conséquence — la décision porte sur le modèle. Un volant se présentant sur plusieurs entrées `Gamepad` (base + boîtier de boutons), adopter l'une marque ses sœurs — même préfixe constructeur/modèle, `deviceFamily` — répondues et adoptées. Stockage dans `ui_prefs.json` (règle d'or n°6), clé `pitbox.gamepad.devices`, lue dans la boucle `requestAnimationFrame` par `peekUiPref` (jamais l'API asynchrone) ; le coupe-circuit global est séparé (`pitbox.gamepad.enabled`) pour que le couper n'efface pas les décisions. **Migration** de `pitbox.gamepadNav.mode`, relu une dernière fois puis retiré : `off` → coupe-circuit à `false`, un `id` forcé → ce périphérique adopté sans rien demander, `auto` → aucune décision (la notification apparaîtra, un clic pour les utilisateurs de manette existants).

**Résolution du profil** (`resolveProfile` dans `gamepadNav.ts`), dans l'ordre : profil calibré sur cette machine (gagne toujours) → profil livré (`DEVICE_OVERRIDES`) → layout standard si le périphérique se déclare `mapping === "standard"` → rien, périphérique inerte. Le layout standard reste lu tel quel plutôt que traduit en `NavProfile` : une direction y a deux sources (croix **et** stick gauche), qu'un `Binding` unique par direction ne sait pas représenter — le traduire ferait perdre le stick sur toutes les manettes normales.

**Retour au neutre exigé** (`armDevice` dans `gamepadNav.ts`) : un périphérique adopté ne produit son premier événement qu'après avoir été **vu au repos**, à l'adoption comme à chaque reconnexion (un slot libéré ne lègue ni son armement ni son dernier front). C'est le correctif du bug ci-dessus — le consentement explicite répond à une autre question, les deux sont complémentaires. Le repos se **mesure**, jamais ne se suppose : un hat DirectInput normalisé par Chromium repose *hors* de [-1, 1] (~3,2 constaté), les pédales à -1, un volant là où on l'a laissé. On attend donc 500 ms sans changement, on prend cet instantané comme référence (sauf profil calibré, qui porte le sien), et on n'arme que si rien de ce que le profil écoute n'est actif — une pédale maintenue est parfaitement stable, mais le profil sait la reconnaître, et le périphérique reste inerte tant qu'elle l'est plutôt que de faire dériver le focus.

**Profils et overrides** (`gamepadProfile.ts`, `DEVICE_OVERRIDES` dans `gamepadNav.ts`) : une liaison est un bouton ou une position d'axe — hat, stick et boutons se réduisent au même modèle `Binding` (`{kind:"button", index}` ou `{kind:"axis", hint, mode:"equals"|"beyond", value}`). L'index d'un axe n'est pas stable : `hint` n'est qu'un point de départ, une liaison `equals` se reconnaît par valeur sur tous les axes — en écartant tout axe qui *repose* sur la valeur cherchée, sans quoi une pédale au repos à -1 répond à la place d'un hat dont « haut » vaut aussi -1. Les profils livrés adoptent **le format exact que produit la calibration**, sinon chaque contribution reçue demanderait une traduction manuelle, donc une occasion de se tromper. Modèle couvert à ce jour : base Fanatec ClubSport Wheel Base V2.5 (croix rapportée comme un axe à 4 positions discrètes, confirmé fonctionnel).

**Raccourcis manette** (§7.4bis, `Action` dans `gamepadProfile.ts`) : cinq boutons au-delà du déplacement du curseur, **tous optionnels** — un profil sans eux reste parfaitement utilisable, un raccourci absent ne fait rien et ne bloque rien. Sur le layout standard ils sont placés là où les interfaces de console les mettent : **gâchettes hautes** LB/RB (boutons 4/5) = onglet précédent/suivant, **gâchettes basses** LT/RT (6/7) = mod précédent/suivant, **Start** (9) = amener le curseur sur « Démarrer la session », **View/Select** (8) = ouvrir le menu contextuel de l'élément ciblé. Les deux paires sont voisines et ne font pas la même chose : onglets au-dessus, contenu en dessous, c'est cet ordre qui rend le couple mémorisable. Front montant uniquement, jamais de répétition au maintien — changer de mod recharge une fiche entière et reconvertit un modèle 3D, une rafale n'a rien d'un service. Les gâchettes hautes ont un **repli** : quand l'écran affiché n'a pas d'onglets (`cycleTab` répond `false`), elles changent de **zone** — barre latérale, liste, fiche de droite, marquées par `data-gp-region` et prises au niveau le plus interne (la zone de contenu d'`AppShell` en est une pour un écran d'un seul tenant ; la bibliothèque la redécoupe en deux). C'est ce qui manquait à la bibliothèque, seul écran sans onglets : rejoindre les filtres depuis le menu latéral, ou la fiche depuis la liste, demandait de traverser des centaines de cartes à la croix. Le curseur revient dans chaque zone **là où on l'avait laissé**, sinon l'aller-retour coûte le défilement. Le bouton menu synthétise un vrai événement `contextmenu` sur l'élément ciblé plutôt que de passer par un registre par écran : tout ce qui répond déjà à la souris (cartes, lignes, panneau de détail) répond du même coup, sans une ligne de code de sa part. Il est devenu nécessaire le jour où les actions groupées sont passées au clic droit — sans lui, elles auraient été inatteignables au volant. Start **amène le curseur**, il ne lance pas : lancer d'une pression depuis n'importe quel écran, sans avoir vu ce qu'on lance, serait le contraire d'un raccourci utile (la barre latérale étant toujours montée, la cible existe quel que soit l'écran ; elle se repère par l'attribut `data-gp-launch`, pas par sa classe — un nom de classe est du style, il se renomme sans qu'on pense à ce fichier).


**Tout champ porteur d'une valeur se laisse « entrer »** (`needsEntry` dans `gamepadNav.ts`) : liste déroulante, champ numérique **et curseur**. Tant qu'il n'est pas entré, gauche/droite déplace le curseur comme partout ailleurs ; validé une fois, gauche/droite règle sa valeur, et **annuler** (ou valider à nouveau) en ressort — haut/bas restant la sortie de secours. Les curseurs y ont rejoint les deux autres après un signalement : trois curseurs alignés sur une ligne (dégâts/carburant/pneus) sont un cul-de-sac si gauche/droite règle au lieu de déplacer, on n'atteint ni le voisin ni rien à droite du dernier. L'appui « annuler » qui sort d'un champ **ne fait que ça** cette image-là : sinon il refermerait aussi la fiche pleine page derrière. Le champ entré porte `.gp-editing` (anneau rouge) là où le simple ciblage porte `.gp-focus` (anneau jaune) — même geste, deux effets, donc l'état doit se voir.

**Défilement analogique** (`scrollAmount` dans `gamepadProfile.ts`) : un axe dédié fait défiler le conteneur sous le curseur **sans déplacer le curseur**, à la vitesse de la poussée (réponse quadratique, 1800 px/s à fond, zone morte de 0,25 renormalisée pour que le premier cran utile ne parte pas déjà au quart de la vitesse). Sur le layout standard c'est l'axe 3, la verticale du **stick droit** ; un profil calibré porte le sien, capturé à l'étape « Défilement rapide ». La croix parcourt les éléments un par un et emmène le défilement avec elle : c'est ce qu'il faut pour choisir, beaucoup trop lent pour traverser une bibliothèque de plusieurs centaines de mods. Le conteneur qui défile est trouvé en **remontant depuis l'élément ciblé**, jamais déclaré par écran. Un axe maintenu au branchement retarde l'armement, au même titre qu'un bouton (§ retour au neutre).
**Ce que les raccourcis déclenchent appartient à l'écran, pas à la manette** (`src/lib/screenActions.ts`) : « onglet suivant » n'a de sens que pour l'écran qui possède ses onglets, « mod suivant » que pour la bibliothèque, seule à connaître son tri et ses filtres courants. D'où un petit registre — l'écran s'inscrit à son montage, se retire à son démontage, et le scrutin manette n'a jamais à savoir lequel est ouvert. Une **pile**, pas une variable unique : la fiche pleine page se monte par-dessus la bibliothèque, c'est la plus récente qui répond. `Tabs.svelte` s'y inscrit tout seul, donc tout écran à onglets devient parcourable sans une ligne de code de sa part.

**La fiche pleine page se navigue comme n'importe quel écran.** La croix directionnelle y déplaçait le curseur *entre les mods* — si bien que rien de la fiche elle-même (grille de skins, onglets, boutons) n'était atteignable à la manette. Elle déplace désormais le curseur **dans** la fiche, et le changement de mod a ses deux boutons dédiés. À l'ouverture, le curseur est posé sur la **vignette de skin sélectionnée** (la première par défaut, celle mémorisée sinon) plutôt que sur le premier élément focusable de la page — le bouton « retour », d'où rejoindre les skins demandait une dizaine d'appuis. Une seule fois par mod ouvert : le curseur appartient à l'utilisateur dès qu'il l'a bougé. Ce placement d'autorité n'a lieu que si un périphérique adopté pilote réellement l'interface (`isGamepadDriving`) : sans manette, voler le focus ferait sauter le défilement.

**Une deuxième validation d'affilée sur le même élément vaut double-clic** (`activate` dans `gamepadNav.ts`) : c'est exactement la convention de la souris — cliquer sélectionne, double-cliquer ouvre — donc valider deux fois une carte de bibliothèque ouvre sa fiche pleine page, sans avoir à traverser l'écran jusqu'au bouton « Agrandir ». Gratuit partout où un `ondblclick` existe déjà (cartes et lignes de bibliothèque, slots de session de la barre latérale). Le `click` part **dans tous les cas**, y compris à la deuxième pression : sans lui, un bouton qui n'écoute que `click` (une flèche d'ordre de couche, un « + » d'adversaire) ne répondrait qu'un appui sur deux — le double-clic s'ajoute, il ne remplace pas. Le compteur suit le curseur et se remet à zéro dès qu'il bouge, sinon revenir plus tard sur une carte déjà validée l'ouvrirait au premier appui. L'événement doit **remonter** (`bubbles: true`) : Svelte 5 délègue `dblclick` à la racine du document. Les champs de saisie ne passent pas par là — ils ont leur propre sémantique de validation (`entered`), où un double-clic ne voudrait rien dire.

**Clavier dans la fiche pleine page** : **Page préc./suiv.** = mod précédent/suivant, **flèches** = déplacement du curseur, exactement ce que fait la croix directionnelle (même fonction `moveFocus`, pas deux implémentations qui divergent). Les flèches tenaient le rôle du changement de mod, et c'était le mauvais choix pour la même raison que ci-dessus. Un champ de saisie garde ses flèches, et un curseur `range` aussi (c'en est un) : les réglages de l'aperçu 3D posés sur la fiche restent réglables au clavier.

**Le panneau dit s'il faut calibrer, pas seulement ce qu'il a reconnu.** Une manette Xbox fonctionne telle quelle (elle annonce l'agencement standard du navigateur), un modèle couvert par un profil livré aussi — et tous deux apparaissaient pourtant dans la même liste, avec le même bouton « calibrer » à côté qu'un volant inconnu. Une ligne par périphérique répond donc à la question dans ces termes : « fonctionne sans calibration » (et pourquoi : agencement standard, profil livré, ou calibration déjà faite) contre « à calibrer : sans profil, ce périphérique ne pilote rien » (jaune, et c'est le seul cas où le bouton devient l'action principale).

**Calibration guidée** (`ControllerSetup.svelte`) : `repos (2 s) → haut → bas → gauche → droite → valider → retour`, puis les cinq raccourcis ci-dessus et enfin le **défilement rapide** (un axe poussé vers le bas, pas un bouton — dernière étape pour ne pas casser le rythme des dix appuis qui se ressemblent), une étape par écran, « Passer » et « Recommencer » toujours visibles. **Échap passe l'étape** sans rien assigner : on garde les mains sur le volant pendant la calibration, et « Passer » demandait d'aller chercher la souris (hors calibration, Échap ferme le panneau). Chaque capture retient le changement le plus marqué par rapport au repos (bouton passé à `pressed`, ou axe écarté de plus de 0,3), exige **150 ms de stabilité** (sinon un rebond de contact ou une valeur intermédiaire d'axe analogique est enregistré à la place du geste), exige le **retour au repos** avant l'étape suivante (sinon le même maintien est capté deux fois) et **refuse un doublon** (deux directions sur la même liaison est pire qu'un profil incomplet). Au bout de ~10 s, réessayer ou passer — beaucoup de volants n'ont pas de croix, « Passer » est un chemin normal, et les cinq étapes de raccourcis le disent explicitement à l'écran (« Facultatif : passez si votre périphérique n'a pas de bouton libre »), sans quoi on attend devant un bouton qu'on n'a pas. **Hat ou stick** se lit *pendant* la capture (`axisMode`) : valeurs intermédiaires observées **et** extrême atteint (|v| ≥ 0,9) → `mode: "beyond"` (seuil, deadzone 0,5 contre les diagonales) ; saut direct d'une valeur discrète à une autre → `mode: "equals"` (±0,1). Ce n'est pas cosmétique : un seuil appliqué à un hat dont « haut » vaut -0,71 ne déclenche jamais rien. L'écran final montre le récapitulatif **et une zone d'essai** — quatre cases où le repère bouge réellement avec le profil construit, parce que lire « Haut → axe 9 = -1,00 » ne prouve rien à un utilisateur — puis propose `[Copier le profil]` et `[Ouvrir un ticket pré-rempli]` (le profil contient le modèle, la forme du périphérique et les index ; rien d'identifiant).

**Réglages > Général** garde ce qui est rattrapable : le coupe-circuit global, la **liste des périphériques connus** (débranchés compris — label mémorisé, grisé) avec la source de leur profil (calibré / livré / standard / aucun), la bascule utilisé/non utilisé, `[Calibrer]` qui rouvre le panneau, et `[Oublier]` qui repasse en « jamais demandé » — le bouton « je me suis trompé », sans lequel une réponse erronée est définitive. Le tableau de diagnostic en direct (mapping/axes/boutons de chaque périphérique) a quitté Réglages : il est replié sous « Détails techniques » dans le panneau, pour le cas où la calibration échoue.

**Pièges du Gamepad API sous WebView2**, chacun coûtant une soirée s'il est ignoré : un périphérique **n'existe pas tant qu'on ne l'a pas touché** (Chromium ne l'expose qu'après une première entrée, anti-fingerprinting) — donc `getGamepads()` peut être vide au démarrage volant branché et allumé, `gamepadconnected` se déclenche à la première pression et non au branchement, et **toute liste vide dit « appuyez sur un bouton pour qu'il apparaisse ici »**, jamais « aucun périphérique détecté » ; `getGamepads()` renvoie un **instantané troué**, relu à chaque image et jamais conservé d'une frame à l'autre ; un `Gamepad` lu hors d'une boucle `requestAnimationFrame` reste figé (d'où le scrutin rAF, jamais `setInterval`) ; `Gamepad.timestamp` ne bouge qu'au changement d'état ; hors focus fenêtre, `requestAnimationFrame` est suspendu, donc la navigation gèle — attendu, mais à savoir avant de chasser un fantôme.

**Listes déroulantes et champs numériques à la manette** (`needsEntry` dans `gamepadNav.ts`) : gauche/droite au simple survol déplace le focus vers le champ suivant, comme n'importe quel autre élément — ne change jamais la valeur en passant dessus (bugs réels signalés : la croix modifiait les filtres `<select>` de bibliothèque juste en naviguant à travers, et le champ année — `type="number"`, `NumberStepper.svelte` — restait piégé, gauche/droite ne faisant plus que grimper/descendre sa valeur sans jamais en sortir). Confirm « entre » dans le champ (état `entered`) : gauche/droite change alors sa valeur, et un nouvel appui sur confirm en ressort. Les curseurs (`type="range"`, `isAdjustable`) restent en dehors de cette logique et continuent de répondre à gauche/droite dès le focus, sans geste d'entrée — pas de popup native à éviter pour eux, gauche/droite y est déjà l'équivalent naturel d'un clic-glisse.

**Navigation manette par région** (`regionOf`/`bestCandidate` dans `gamepadNav.ts`) : le plus proche voisin géométrique seul peut préférer un bouton du menu latéral (`.side`) à une carte de la grille plus bas dans le contenu (`.content`), quand celui-ci est horizontalement plus proche du bord gauche du contenu — bug réel signalé, "bas" depuis les filtres de bibliothèque retombait sur le menu latéral au lieu d'entrer dans la grille. `moveFocus` cherche donc d'abord un candidat dans la même région (`.side` ou `.content`) que l'élément courant, et ne se rabat sur toutes les régions que si la région courante n'a rien dans cette direction — c'est ce repli qui préserve le passage intentionnel grille → menu en allant à gauche depuis la première colonne de la grille.

**Vue tableau de la bibliothèque** : les lignes (`<tr>` dans `Library.svelte`) portent `tabindex="0"` spécifiquement pour la navigation manette — un `<tr>` avec juste `onclick` n'entre dans aucun sélecteur `FOCUSABLE` de `gamepadNav.ts` (ni bouton, ni lien, ni champ), donc restait invisible à la croix/au stick même une fois la région "bas" corrigée ci-dessus (repli sur le menu latéral faute de candidat dans le tableau). `.click()` déclenche le même `onclick` que la souris (sélection simple, sans Ctrl/Shift) — aucun geste séparé à coder côté manette.

**Répétition en rester appuyé** (`shouldFire` dans `gamepadNav.ts`) : haut/bas/gauche/droite maintenus enchaînent les déplacements sans relâcher — sinon parcourir une longue liste (tableau bibliothèque…) demandait un appui par ligne, bug réel signalé. Décollage après 380ms (laisse un appui ponctuel se comporter comme avant, sans répétition parasite), puis rythme constant (130ms), qui accélère (60ms) après 1,5s de maintien continu. Un état par (manette, direction), comme `lastByGamepad` — jamais une variable partagée entre manettes. Confirm/back restent à appui unique, jamais répétés (un clic en boucle n'a pas de sens).

---

## 8. Skins, sons, apps

**Base Kunos indexée** en lecture seule (`is_stock`), non désactivable, pour que skins/sons puissent s'attacher à une voiture/circuit de base comme à un mod.

**Quand l'index est (re)construit** — deux déclencheurs, et il a fallu les deux : à l'**enregistrement de la configuration**, dès que le dossier du jeu est désigné ou qu'il change (`stock::needs_reindex`) ; et au **démarrage**, si rien n'est indexé alors qu'un dossier est connu. Le second seul ne suffisait pas : au tout premier lancement, la config n'existe pas encore quand l'app démarre, l'assistant l'écrit après — la bibliothèque restait donc vide jusqu'au lancement **suivant**. Même angle mort en changeant de dossier de jeu depuis les Réglages, où l'index continuait de décrire l'ancienne install. Un bouton « Indexer le contenu de base » reste disponible en Maintenance pour forcer la reconstruction.

**Skins — sélection, pas activation filesystem.** Un skin est un sous-dossier dans `skins/` ; AC les charge tous. Aucune activation/désactivation. Seules actions : prévisualiser, et désigner le **skin piloté** (étoile) pour le lancement. Import via l'import général (rattachement automatique via le dossier `skins/<voiture>/`). **Miniature `livery.png`** (couleurs/motif du skin seul, convention AC reprise par CM) affichée quand présente : dans la liste déroulante compacte de sélection du skin de session (barre latérale, §9.1 — bien plus lisible que la photo de la voiture entière écrasée à 20px) et en médaillon dans le coin supérieur droit de chaque vignette de la grille de skins (fiche détail, §6.3) ; jamais sur la grande photo du skin sélectionné.
- **Vue Skins** : sélection multiple (Ctrl/Alt) pour supprimer plusieurs skins d'un coup. **Regroupement par archive d'origine** (pour supprimer d'un coup tous les skins d'une même archive) ou, au choix, **par voiture**.

**Sons** — exclusifs (un seul actif par voiture), vrai remplacement de fichiers (`.bank` + `GUIDs.txt`), original toujours restaurable.

**Apps** — type autonome, vue propre, activables (par défaut dès l'import, comme les mods voiture/circuit et les « autres mods »). Détection Python (`<id>/<id>.py`) et Lua/CSP (`<id>/<id>.lua`) ; activation par junction vers `apps/python/<id>` ou `apps/lua/<id>` selon le langage constaté. Ressources annexes (§4.5.2, ex. manuel PDF fourni avec l'app) listées et ouvrables depuis la vue, comme sur une fiche voiture/circuit.

**Pas de notion de mise à jour ni de couche.** Contrairement aux voitures/circuits (§4.3), réimporter une app dont l'id existe déjà **remplace intégralement** ses fichiers, sans comparaison ni choix — pas de diff, pas d'historique de versions. Comportement délibérément simple : une app est un script autonome, sans les enjeux de couches/composition d'un mod de contenu.

**Accès transversal** : vues Skins / Sons / Apps dans la barre latérale, en plus de l'accès par la fiche.

**Écrans Add-ons voiture / circuit en onglets** (`Transversal.svelte`) : Skins | Sons | Couches & extensions pour les voitures, Skins | Couches & extensions pour les circuits (les sons ne concernent que les voitures). Les trois rubriques s'empilaient sur une page interminable alors qu'elles ne se consultent jamais ensemble. La recherche est descendue de l'en-tête dans la barre d'outils de la liste : elle y accompagne ce qu'elle filtre, et couvre du même coup les sons, qui n'avaient aucun champ de recherche une fois imbriqués.

**Les skins de circuit fournis avec le mod ne sont plus listés dans la vue transversale.** Reconnus sur disque dans `cm_skins/` (§8 ci-dessus), jamais importés séparément, donc sans archive d'origine : ils remplissaient à eux seuls la rubrique « Origine inconnue ». Et rien dans cette vue ne s'applique à eux — ni sélection, ni suppression (seul le mod entier les emporte), ni activation (elle se fait depuis la barre latérale ou la fiche du circuit). Les lister n'apprenait donc rien et noyait ce qui se gère vraiment. Conséquence : la rubrique « Origine inconnue » n'apparaît plus que si un skin réellement importé n'a pas d'archive connue. Les skins **de voiture** fournis avec le mod restent listés : là, parcourir l'ensemble des skins d'une voiture est un usage légitime de la vue.

**Analyse des extensions CSP** : poussée plus loin (détection fine des fonctionnalités CSP d'un mod).

---

## 9. Lancement de session

### 9.1 La bibliothèque est le sélecteur

Pas d'écran séparé de sélection : la voiture/le circuit sélectionnés dans la bibliothèque sont ceux de la session. Le **bloc Session** de la barre latérale montre en permanence le duo courant. La page « Démarrer une session » ne contient aucune sélection de voiture/circuit — seulement les réglages + Lancer.

**`ImageSelectDropdown.svelte` (sélecteur de skin/layout du bloc Session) échappe au clip de la barre latérale.** La barre latérale (`.side`) défile verticalement (`overflow-y: auto`) — et une règle CSS fait qu'un seul axe posé à `auto` calcule l'autre à `auto` aussi, donc `.side` rogne également tout ce qui déborde en largeur. Un `position: absolute` classique en aurait fait les frais dès qu'un libellé de layout dépassait la largeur de la barre latérale : la liste ouverte restait aussi étroite que le déclencheur, ellipsée à mi-mot, sans le moindre moyen de lire le nom en entier. Corrigé sur deux fronts, indépendants l'un de l'autre :
- **La liste ouverte passe en `position: fixed`**, positionnée en JS depuis le rectangle du déclencheur (`getBoundingClientRect`), avec `width: max-content` (elle grandit jusqu'à son plus long libellé, plafonnée à `min(420px, 100vw - 16px)`) plutôt que calée sur la largeur du déclencheur. `fixed` échappe au clip de n'importe quel ancêtre à `overflow` — aucun n'y pose de `transform`/`filter`/`will-change`, ce qui aurait recréé un cadre de référence local et annulé l'échappée — et se repositionne au plus près du bord droit de la fenêtre si son plus long libellé la ferait déborder, une fois sa largeur réelle connue après rendu. Se ferme sur un défilement de n'importe quel ancêtre (sans ça, une liste `fixed` resterait figée pendant qu'un `.side` défilerait sous elle) — sauf le sien propre, sans quoi parcourir une longue liste la refermerait avant qu'on ait pu cliquer. **Piège vérifié plutôt que supposé** : un élément `position: fixed` reste un `offsetParent` valide pour ses propres enfants (seul lui-même, interrogé directement, renvoie `offsetParent === null`) — la navigation manette (`gamepadNav.ts`, filtre `offsetParent !== null`) continue donc de trouver les boutons de la liste sans adaptation.
- **Le déclencheur montre le nom complet au survol/focus**, à la place de l'ancienne infobulle native figée sur le texte du placeholder (« Choisir un layout ») : une bulle maison (même mécanique CSS que `Tooltip.svelte` — `:hover`/`:focus`, pas de JS — mais pas le composant lui-même, qui enveloppe son déclencheur dans un `inline-flex` incompatible avec le `width: 100%` du bouton ici) affiche le libellé complet, alignée à gauche et plafonnée en largeur (200px, texte qui s'enroule) pour la même raison de clip que ci-dessus. `:focus`, pas `:focus-within` : ne réagit qu'au déclencheur lui-même, jamais à un bouton de la liste ouverte (qui montre déjà les noms en entier) — et la bulle disparaît entièrement tant que la liste est ouverte, pour ne pas s'y superposer. Fonctionne aussi bien au focus posé par la manette (`gamepadNav.ts` appelle un vrai `.focus()` DOM) qu'au survol souris, contrairement à l'attribut `title` natif, qui ne réagit qu'au survol réel.

### 9.2 Pilotage par preset Quick Drive CM

L'app pilote CM via son protocole `acmanager://race/quick?presetFile=…` : un **preset Quick Drive** (JSON, format `SaveableData` de CM) est généré à chaque lancement dans un fichier temporaire jetable, puis passé à `Content Manager.exe`. C'est le même chemin (`QuickDrive.ViewModel.Go()`) que le bouton « DRIVE » de l'UI Quick Drive native de CM — condition nécessaire pour que le téléchargement CSP automatique (VAO/config manquants) se déclenche.

> L'ancien mécanisme `race/config?configFile=` (race.ini brut via `PreparedConfig`) a été abandonné : il ne peuple pas `StartProperties.BasicProperties`, dont dépend le check CSP auto-load côté CM — bug confirmé empiriquement, détail en `docs/L4-cm-launch-research.md`.

Limites connues du preset Quick Drive (pas de champ correspondant trouvé dans le schéma) : évolution du grip non mappée (toujours « Optimum »/sec), durée de session Practice non appliquée (sessions à durée libre par design Quick Drive).

### 9.2ter Une course, deux modes CM selon la qualification

Le mode `QuickDrive_Weekend.xaml` n'a pas d'état « pas de qualification » : son curseur est borné à `[5, 90]` min et son `Save()` n'écrit jamais de durée nulle. Une course **sans** qualification passe donc par l'autre mode course de CM, `QuickDrive_Race.xaml`, dont le `ModeData` ne porte aucune durée — c'est le même contenu que Weekend moins `PracticeLength`/`QualificationLength` (schéma confirmé sur un preset réel sauvegardé depuis l'UI de CM). Côté Pit Box, un seul type de session « Course » : c'est la case Qualification qui décide du mode envoyé.

Les essais libres n'existent que dans Weekend, ils suivent donc la qualification.

**`PracticeLength: 0`, jamais `null`** : le `Load()` de `QuickDrive_Weekend.xaml.cs` fait `r.PracticeLength ?? 15`, donc `null` ne saute pas la phase — il rend 15 minutes d'essais par défaut. Seul `0` la saute (curseur `[0, 90]`, libellé « Skip session » côté CM). Bug réel, constaté en jeu avant d'être retrouvé dans la source.

L'URI de lancement porte `&loadAssists=true`, qui correspond au flag `forceAssistsLoading` lu par `ArgumentsHandler.Race.cs::ProcessRaceQuick` (code source de CM) et force le chargement de l'`AssistsData` du preset (dégâts/carburant/pneus/aides/chauffe-pneus), **indépendamment** du réglage global de CM « Charger assistances avec préréglage de course rapide » (désactivé par défaut chez CM). Sans ce flag, CM ignore silencieusement (pas d'exception, pas de log) l'`AssistsData` de n'importe quel preset Quick Drive — y compris ceux sauvegardés par l'utilisateur lui-même dans CM — et garde les assistances actuellement actives dans son UI. Confirmé en lisant `QuickDrive.xaml.cs`/`ArgumentsHandler.Race.cs` (`gro-ove/actools`). `TrackPropertiesData` (grip) n'a pas de garde équivalente côté CM — toujours chargé, indépendamment de ce flag.

**Skin joueur — réinjecté dans le `race.ini` juste après CM.** Le protocole `race/quick` ne transporte aucun skin joueur : `ArgumentsHandler.Race.cs::ProcessRaceQuick` ne lit qu'un preset + des assists et n'en transmet aucun à `QuickDrive.RunAsync()` (qui a pourtant un paramètre `carSkinId`, jamais alimenté depuis l'URI), et le format de preset lui-même n'a pas de champ skin — mesuré, pas déduit : deux `.cmpreset` sauvegardés par CM avec deux skins différents sont identiques octet pour octet. CM retombe donc sur `CarObject.SelectedSkin`, sa mémoire par voiture.

Pit Box écrit donc **après** lui. CM réécrit `Documents\Assetto Corsa\cfg\race.ini` à l'instant où il lance `acs.exe`, mais le jeu ne lit ce fichier que quelques centaines de ms plus tard, pendant son chargement. Un fil de surveillance (`raceini.rs`) guette cette réécriture, remplace `SKIN=` dans `[RACE]` et `[CAR_0]` — les deux sections du joueur, les adversaires (`[CAR_1]`…) gardant les skins que CM vient d'écrire depuis notre grille — puis substitue le fichier par `fs::rename` (atomique : le jeu voit l'ancienne version ou la nouvelle, jamais une moitié). Mesuré sur un lancement réel : écriture de CM à +1694 ms après l'URI, réinjection à +1702 ms, skin confirmé chargé par le jeu dans son propre `logs\log.txt`.

Best-effort par construction, comme tout le reste du lancement : arriver trop tard laisse simplement le skin de CM, l'état d'avant ce mécanisme. Un garde vérifie que `[RACE] MODEL=` correspond bien à la voiture demandée avant de toucher au fichier — on ne tamponne pas un skin sur la session de quelqu'un d'autre — et chaque abandon est journalisé (`log::warn!`). Écrire directement dans le cache interne de CM (`Values.data`) a été écarté : fichier compressé au format propriétaire, réécrit par CM à sa fermeture, et skin mis en cache mémoire dès la première lecture.

> Écrire `race.ini` **avant** de lancer CM ne sert à rien : `Game.StartAsync` charge le fichier existant, le nettoie, puis `BasicProperties.Set()` réécrit `[RACE] SKIN` et reconstruit `[CAR_0]` intégralement. Vérifié : un skin sentinelle écrit avant lancement est effacé 0,26 s après l'envoi de l'URI.

### 9.2quater Track day

Quatrième type de session, à droite de Course. Passe par son propre mode CM, `QuickDrive_Trackday.xaml` — schéma confirmé sur un preset réel sauvegardé depuis l'UI de CM (`pitbox-trackday.cmpreset`) : même grille manuelle d'adversaires que Course (§9.3), tours, faux départ et pénalités, plus `SpeedLimit` (pas de champ correspondant côté Pit Box pour l'instant — toujours 0, comme le seul preset de référence vu). Contrairement à Course, aucun mode Weekend équivalent : jamais de qualification ni d'essais libres, quel que soit le réglage.

### 9.2bis Steam doit tourner avant le lancement

Assetto Corsa est un jeu Steam : c'est Steam qui le démarre, quel que soit le `Starter` retenu par CM. Steam éteint, l'échec se produit **après** que Pit Box a rendu la main — aucune erreur ne remonte à l'app, l'utilisateur voit seulement une session qui ne démarre pas.

Le lancement vérifie donc la présence du process `steam.exe` (`launch::steam_running`, scan ponctuel `sysinfo`, même mécanique que la surveillance du jeu en §16.4) **avant** de construire le preset. Absent : un dialogue demande de démarrer Steam et de valider, la validation revérifie, et tant que Steam manque le dialogue reste ouvert en le signalant. Une erreur de la vérification elle-même laisse passer le lancement — le pire cas redevient simplement l'échec côté CM.

**Bouton « Ouvrir dans CM »** : lance CM sans argument de session, sélection active, pour les réglages fins (échappatoire power-user).

### 9.3 Écran de réglages

Maquette de référence `pitbox-reglages-session.html`. Pas de rappel du duo en haut (déjà dans la barre latérale) — titre + Lancer. Toutes les options visibles (pas de bloc replié). **Fond photo** derrière l'interface : voir §6.2 pour l'ordre de repli (screenshot du combo → screenshot du circuit → background officiel → fond neutre).

**Communs à tous les types de session** — regroupés dans « Simulation » (dégâts,
conso carburant, usure pneus, chauffe-pneus, puis en sous-rubrique aides à la
conduite ABS/antipatinage/ligne) et dans « Options de session » (pénalités,
évolution du grip) : ces réglages sont envoyés au preset Quick Drive quel que
soit le type (`Penalties` figure dans les trois `ModeData` ; `TrackPropertiesData`
et `AssistsData` sont au niveau racine du preset, pas dans `ModeData`), rien ne
justifie de les cantonner à Course. Météo et heure, également communes.

**Course et Track day** (absents des schémas Quick Drive Practice/Hotlap : pas de
grille, pas de phase weekend) :
- **Adversaires** : 4 modes (Même voiture / Même catégorie / Même ère via année min/max / Libre). Remplissage auto selon le mode, **liste du plateau visible et ajustable** (chaque IA avec sa force, retirer/ajouter, cliquer une ligne pour changer sa voiture/son skin — vignette de la ligne : livery du skin choisi si connue, même convention que le sélecteur de skin de la barre latérale, repli sur la preview du skin puis sur celle du mod). Un bouton « + » par ligne duplique cette voiture avec un skin différent (pas encore pris par un autre adversaire du même mod dans le plateau ; reboucle sur les skins déjà pris une fois tous épuisés). Nombre d'adversaires, **Difficulté** (fourchette min-max, deux curseurs, le plateau réparti dans la plage — sous-rubrique du bloc Adversaires, pas une rubrique séparée) et année min/max sur une même ligne. Année min/max sont deux champs numériques indépendants (pas une double glissière) : 0 ou vide = pas de borne de ce côté, filtrage fait côté front (non transmis au preset Quick Drive). Mode **Même catégorie** : la catégorie effectivement utilisée (même liste que le filtre catégorie de la bibliothèque voitures) est un menu déroulant intégré dans l'onglet du mode lui-même, sous son libellé, visible dès que ce mode est actif — toujours réinitialisé sur la catégorie de la voiture pilotée dès qu'elle change, mais modifiable ensuite pour piocher dans une autre catégorie que la sienne.
- Faux départ, pénalités — communs à Course et Track day. **Tours** : Course uniquement — envoyé dans le `ModeData` de Track day aussi (schéma confirmé sur un preset CM réel), mais sans effet en jeu : une session Track day ne se termine jamais sur un compte de tours, donc le réglage n'a pas sa place dans l'écran pour ce type de session. **Case qualification** (durée en min, mini 5 — borne de CM) et, sous elle, **case essais libres** (durée en min) : **Course uniquement**, absentes de Track day (§9.2quater, aucun mode Weekend équivalent côté CM). Décocher la qualification décoche les essais libres : les deux n'existent que dans le mode Weekend de CM, et sans qualification le preset bascule sur son mode course sèche, où aucune phase préparatoire n'existe (§9.2ter). Laisser les essais cochables y afficherait un réglage sans effet en jeu.

**Régénération du plateau** (Course et Track day) : le vivier dépend de la voiture pilotée, donc changer de voiture régénère les adversaires — sauf en mode **Libre**, dont le vivier n'en dépend pas et où le plateau est le plus souvent réglé à la main. Changer seulement de **skin** ne régénère jamais. La voiture pour laquelle le plateau a été construit est persistée avec lui (`grid_car_id`) : l'écran de lancement est démonté dès qu'on passe à la bibliothèque, c'est donc la seule façon, au remontage, de distinguer un plateau fait pour la voiture courante d'un plateau hérité de la précédente — sans ça, changer de voiture depuis la bibliothèque puis revenir laissait le plateau de l'ancienne (bug réel).

**Practice** : pas de champ durée (non applicable — session à durée libre par design Quick Drive, voir §9.2 ; pas de champ correspondant côté Pit Box), ni tours/faux départ (absents du schéma `QuickDrive_Practice.xaml`, réservés à Course/Track day). Départ (Stand/Piste/Position de chrono → `StartType` du `ModeData`, trois valeurs) : "Piste" non vérifiée sur un preset réel, voir commentaire `PracticeStart`.

**Hotlap** : ghost car.

**Météo** : conditions en **icônes SVG stylisées** (thème, libre de droits) — Beau, Quelques nuages, Couvert, Brouillard, Pluie légère, Pluie, Orage. **Température, vent et heure implicites** sur une même ligne (heure modifiable, température/vent recommandés par condition + heure + stack SOL/CSP, tous corrigeables à la main). **Saison** optionnelle : un champ date natif (en premier, avant les 4 cartes saison) qui affiche/permet de corriger précisément la date associée — sélectionner une saison y reporte automatiquement la date calculée (milieu de saison), la modifier à la main ne désélectionne pas la saison affichée.

**Presets de session par type** : chaque type (Practice/Hotlap/Course/Track day) a un preset mémorisé ; toute modif est persistée pour les prochaines sessions du même type. **Persistance** (`src-tauri/src/session_state.rs`, `app_config_dir/launch_state.json`) : fichier écrit côté Rust, même mécanisme et même raison que le duo de session (§7.4) — `localStorage` n'est pas garanti synchrone sur disque côté WebView2, ce qui pouvait perdre les presets et la dernière sélection (type de session, adversaires) à la fermeture de l'app. Fichier dédié, distinct de `session.json` (chaque commande réécrit tout son fichier ; les mélanger ferait que sauvegarder le duo de session écrase les presets, et inversement). Migration silencieuse au premier démarrage après la mise à jour, même schéma qu'en §7.4.

**Sessions enregistrées** (carte dédiée, à droite de « Type de session », surtout utile pour la liste d'adversaires) : liste inline, scrollable, **filtrée par le type de session courant** — change avec l'onglet Practice/Hotlap/Course/Track day. Cliquer une entrée la charge immédiatement. Bouton **Sauvegarder** au-dessus de la liste ouvre une popup de nommage (ou sélection d'une sauvegarde existante du même type, pour l'écraser). Clé par `<type>::<nom>` : deux types peuvent avoir une sauvegarde du même nom sans collision. **Persistance** (`src-tauri/src/saved_sessions.rs`, `app_config_dir/saved_sessions.json`) : même mécanisme et même raison que le duo de session et les presets ci-dessus — fichier dédié écrit côté Rust, migration silencieuse depuis `localStorage` au premier démarrage après la mise à jour.

**Contenu d'une session enregistrée** : les réglages (météo, adversaires, options) **et le duo de session** — voiture pilotée avec son skin, circuit avec son tracé et ses **skins de circuit actifs** (§8, seul élément hors `setup` : c'est un état de déploiement, d'où un champ `trackSkins` à part). Le chargement rétablit le tout en passant par le duo de session (§8.6), qui reste la source de vérité : voiture et circuit sont reposés via `pickSession`, pas écrits directement dans le setup. Les skins de circuit sont remis **à l'identique** — ceux qui manquent sont activés, ceux en trop désactivés, sinon un skin resté actif d'une session précédente changerait l'apparence du circuit sans que rien ne le signale. Une sauvegarde antérieure au champ `trackSkins` (`undefined`, distinct d'une liste vide) n'y touche pas du tout.

**Chargement partiel** : un mod supprimé depuis la sauvegarde n'interrompt jamais le chargement — la sélection courante est conservée pour ce qui manque (voiture, circuit), le tracé retombe sur celui par défaut, et un **bandeau d'avertissement jaune** en tête d'écran (même emplacement que le retour de lancement, jamais une popup : il n'y a rien à décider) énumère ce qui n'a pas pu être rétabli. Il s'efface au chargement suivant et au lancement de la session.

**Écran non prêt** : le corps de l'écran (colonnes de réglages) reste masqué derrière `LoadingState` (même indicateur que les listes de mods) tant que le chargement initial n'est pas terminé (bibliothèque, presets/sélection persistés, duo de session, météo par défaut) — évite que l'utilisateur voie les champs se réajuster au fur et à mesure que les valeurs mémorisées arrivent.

**Choix layout/skin de circuit** : sur la fiche/bibliothèque circuit, image d'aperçu (`preview.png`) avec le tracé du layout (`outline.png`/`map.png`) par-dessus, infos (longueur, virages, CSP).

### 9.4 Aperçu 3D des voitures

Deux aperçus 3D **coexistent**, parce qu'ils ne rendent pas le même service.

**Aperçu intégré à la fiche** (docs/SPEC-preview-3d-kn5.md). Le modèle `.kn5` de la voiture est lu, converti en glTF et affiché **dans la zone héros**, à la place de la photo. La voiture **tourne lentement sur elle-même**, comme sur un socle de salon (un tour en ~28 s) : c'est la présentation par défaut, l'orbite et le zoom à la souris restent disponibles par-dessus. Le plateau s'arrête dès qu'on attrape le modèle et repart après quelques secondes d'inactivité ; il s'arrête aussi quand la fiche quitte l'écran ou que la fenêtre passe en arrière-plan, et ne démarre pas du tout si le système demande de réduire les animations. Cadrage trois-quarts avant calculé sur les dimensions réelles du modèle. Actif par défaut sur les fiches voiture ; une bascule discrète en bas à droite du héros permet de revenir à la photo, et ce choix est mémorisé. Pendant la préparation, la photo reste affichée en fond flouté avec un badge d'étape en haut à droite. Si le modèle est introuvable, protégé (KN5 chiffré) ou si la machine n'a pas de WebGL, l'aperçu retombe **silencieusement** sur la photo — badge discret « aperçu 3D indisponible » seulement quand il y a une raison à donner. Ce n'est jamais une erreur bloquante : c'est un bonus visuel. Le skin sélectionné sur la fiche est appliqué au modèle. **Changer de skin ne repasse pas par la photo** : le modèle en place continue de tourner pendant que le nouveau se prépare, et celui-ci reprend le plateau et la caméra exactement où le précédent les avait laissés — la voiture change de peinture sans interrompre sa rotation. Si le nouveau skin ne peut pas être converti, l'ancien modèle reste à l'écran avec sa peinture précédente plutôt que de retomber sur la photo. La conversion est mise en cache sur disque : le deuxième affichage d'une même voiture est immédiat.

**Aperçu natif** (bouton de la fiche, lance `acshowroom`) — conservé pour le rendu fidèle du jeu, que l'aperçu intégré n'imite pas. Le showroom est un **process indépendant**, affiché par-dessus l'app avec les réglages vidéo du jeu : l'utilisateur le ferme lui-même pour revenir à Pit Box. L'intégration de sa fenêtre dans la page a été tentée puis abandonnée (voir `showroom-3d-preview-research.md`). **Option de réglage** : le décor (`content/showroom/<id>`) chargé par `acshowroom`, choisi parmi ceux installés — défaut `studio_white`, le seul instantané et sans musique. Pendant le démarrage d'`acshowroom`, afficher une **animation de chargement en haut à droite** de l'image.

---

## 10. Maintenance, export, nettoyage

**Export d'archive autonome** : repackager un mod complet avec ses dépendances éparpillées (pilotes 3D, polices). Seule fonction qui justifie de lire le `data.acd` chiffré (extraction acd.bms, isolée dans le module d'export, jamais sur le chemin d'import/activation).

**Nettoyage** : détection assistée des mods cassés (voitures sans `ui/`, circuits sans contenu valide, hardlinks orphelins pointant vers un mod supprimé).

**Activation / désactivation vs désinstallation — deux axes distincts.**
- **Activer / désactiver** répond à « ce mod est-il actuellement déployé dans le jeu ? ». Active un mod sans couche = créer les hardlinks du mod vers `content/` ; désactiver = les supprimer (contenu **intact en bibliothèque**). Contenu à couches = composer/recomposer (§4.3). Quasi instantané, réversible, ne libère pas d'espace. Utile pour alléger le roster que CM scanne, éviter des conflits ponctuels, composer une sélection courante.
- **Supprimer de la bibliothèque** répond à « ce mod doit-il encore occuper de la place sur le disque ? ». Action **distincte**, avec sa propre confirmation — efface les fichiers de la bibliothèque (et désactive au passage s'il était actif). Non réversible sans réimport (sauf si l'archive source a été conservée, voir ci-dessous).
- **Profils** : ensembles nommés activables/désactivables en masse — capture l'état actif des **trois** types activables (mods voiture/circuit avec leur version, Autres mods §7.3, Apps §8). Utile pour resynchroniser une bibliothèque copiée sur une autre machine (ex. réplication via robocopy) : les fichiers voyagent, mais aucune junction/hardlink ne survit à un copiage — capturer un profil avant de migrer, l'appliquer une fois la bibliothèque et `overlay.sqlite` en place sur la nouvelle machine réactive tout en une action. Autres mods et Apps n'ont pas de notion de version (simple actif/inactif), stockés à part côté overlay (`profile_extra_entries`).
- **Garde-fou** : vérifier hardlink/junction vs fichier ou dossier réel avant toute suppression dans `content/`.

**Sauvegarde automatique de démarrage** (`src-tauri/src/backup.rs`, best-effort, silencieuse) : à chaque lancement de l'app, avant toute ouverture de connexion à la base, copie `overlay.sqlite` et les petits fichiers de préférences (`config.json`, `ui_prefs.json`, `library_columns.json`, `session.json`, `launch_state.json`, `saved_sessions.json`, `music.json`, `tag-rules.json`) dans `app_config_dir/backups/<horodatage>/`. Rotation sur les 7 plus récentes. Filet de sécurité contre une base corrompue ou un fichier de préférences écrasé par erreur — pas un vrai système de restauration point-in-time (pas d'écran dédié pour l'instant) : en cas de pépin, fermer l'app et recopier à la main les fichiers voulus depuis le dossier de sauvegarde le plus récent.

**Conservation de l'archive source** (réglage optionnel, défaut désactivé — cohérent avec l'absence d'historique de versions/couches, §4.3) : si activé, l'archive/dossier source d'un mod est conservée en bibliothèque en plus du contenu extrait. Rend disponible une action **« Réinstaller depuis l'archive source »** sur la fiche du mod (visible seulement si l'archive est conservée) : réextrait l'archive et remplace le contenu de bibliothèque pour ce mod. Utile en cas de corruption, de modification accidentelle, ou pour repartir propre sans retélécharger.

**Ce qui survit volontairement à la suppression d'un mod.** Deux tables sont délibérément absentes du `DELETE` :

- **`usage`** (§6.5) — marqueur « déjà essayé » et nombre de lancements. Réimporter la même voiture retrouve son historique plutôt que de repartir de zéro. Le **kilométrage** n'a de toute façon jamais été chez nous : il vit dans le journal de sessions de Content Manager, indexé par `CarId`, donc rien de ce que fait Pit Box ne peut le perdre.
- **`sub_mods`** — skins et sons rattachés, dont les fichiers ne sont pas effacés non plus. Réimporter le parent sous le même id les retrouve tels quels, ce qui est précisément le geste d'une réinstallation.

Ce n'est un déchet que si le parent ne revient jamais. Ils sont donc **listés en maintenance** (« Skins et sons sans mod ») et nettoyés **sur décision**, jamais automatiquement. Le nettoyage contourne le garde-fou `removable` : il protège un skin fourni avec un mod vivant, ce qui n'a plus de sens quand le parent a disparu.

**Réparation générale** (écran Maintenance, à la manière du « purge & deploy » des autres gestionnaires de mods). Sa définition tient en une phrase : **recalculer tout ce qui dérive de la bibliothèque**. Rien de tout cela n'exige de connaître les règles des versions précédentes de l'app — `content/` est une fonction pure de la bibliothèque, recalculée à chaque activation, donc un changement de règles de déploiement se rattrape en redéployant, sans rien versionner ni comparer. Deux étapes sûres et rejouables à volonté : (1) recréer les projections (junctions) de skins voiture/circuit manquantes ou cassées — cas typique, une copie de bibliothèque (robocopy, migration) qui ne préserve pas les junctions, leur cible étant un chemin absolu propre à la machine source ; (2) **redéployer les mods actifs**, ce qui refait `content/` selon le mode et les règles du jour, ajouts au jeu compris (§4.5.3) — un mod importé avant leur existence les pose ainsi sans réimport. Un mod que l'utilisateur avait **désactivé n'est jamais réactivé** au passage : ce serait une surprise, pas une réparation. Une case à cocher optionnelle ajoute la seule étape qui touche la bibliothèque elle-même : réinstaller depuis l'archive source conservée tout mod détecté cassé qui en a une ; sans archive conservée il est laissé de côté, visible dans la liste des mods cassés. Les échecs individuels sont listés en détail sous le bouton, pas seulement comptés — chaque ligne identifie le skin/mod concerné et la raison technique brute.

**Journal fichier** (`tauri-plugin-log`, niveau Warn, `%APPDATA%\com.pitbox.app\logs\pitbox.log`) : seul moyen de diagnostiquer, sur une install packagée sans console, un échec d'opération best-effort qui ne bloque jamais l'UI (activation automatique à l'import, arbitrage de priorité entre « autres mods », etc.). N'enregistre que des échecs réels — jamais un flux d'activité normale.

---

## 11. Configuration et préférences

**Chemins requis** (assistant de première configuration, détection auto si possible) : dossier d'install AC, bibliothèque, exécutable CM, 7-Zip, QuickBMS + script acd.bms (optionnels, export seulement). Détection auto (`detect.rs`) : AC via les bibliothèques Steam, Content Manager dans le dossier AC ou `%LOCALAPPDATA%\AcTools Content Manager`, 7-Zip dans ses emplacements standard ou, à défaut, le `7z.exe` que Content Manager embarque pour son propre usage (`%LOCALAPPDATA%\AcTools Content Manager\Plugins\7Zip\7z.exe` — beaucoup d'utilisateurs CM n'ont jamais installé 7-Zip à part). La bibliothèque n'a pas de détection à proprement parler (rien n'y existe encore au premier lancement) mais une **suggestion** pré-remplie dans le dossier utilisateur (`<home>\PitBox Library`, jamais Documents/Bureau/Images — redirigés vers OneDrive par défaut sur Windows, ce qui tenterait de synchroniser une bibliothèque de plusieurs centaines de Go), éditable comme les autres champs détectés.

**Trois bases/fichiers distincts** : bibliothèque (fichiers), base d'overlay SQLite (métadonnées), fichier de règles (ontologie), plus le fichier de config (chemins + préférences).

**Préférences persistantes** : affichage des tags du fichier mod (masquables), état du panneau de suivi (global), vue bibliothèque + colonnes (par type), presets de session (par type), preset CM graphique/FFB par défaut, décor de l'aperçu 3D natif (§9.4), **aperçu 3D intégré affiché ou non sur la fiche voiture** (défaut affiché — §9.4), regroupement des skins (archive/voiture), extraction des fichiers annexes (Aucun / Informations seulement / Tout — §4.5.2), **conservation de l'archive source** (défaut désactivé — §10), **mode de déploiement** (hardlink/symlink, défaut hardlink — §2), **zoom du mode Big Picture** (§16, distinct du zoom normal — `None` reprend ce dernier).

**Écran Réglages en onglets** (Général / Chemins / Import / Aperçu / Musique) depuis le mode Big Picture (§16) — Général/Chemins/Import partagent `AppConfig` et sa garde de navigation (§10bis) ; Aperçu et Musique ont chacun leur propre stockage et **s'appliquent sans bouton Enregistrer** (`ui_prefs.json` pour l'un, `music.json` pour l'autre).

**Onglet Aperçu** (`components/settings/PreviewTab.svelte`) : réglages de l'aperçu 3D intégré (§9.4) — affiché ou non sur les fiches (même réglage que la bascule de la zone héros), zoom, orientation, angle de plongée, hauteur de caméra, vitesse du plateau tournant, et un bouton qui rétablit le cadrage d'origine. Les curseurs eux-mêmes sont dans `components/detail/Preview3dControls.svelte`, **partagé avec le panneau posé sur la fiche voiture** : on les règle là où on voit le résultat, on les retrouve ici avec leur mode d'emploi. Les valeurs par défaut sont celles mesurées sur les `preview.jpg` de Kunos, pour que la bascule photo/3D ne saute pas à l'œil (trois-quarts avant gauche, vue basse — détail dans `SPEC-preview-3d-kn5.md` §15), et un changement s'applique à une fiche déjà ouverte sans recharger son modèle.

---

## 12. Écran « À propos »

Maquette de référence `pitbox-a-propos.html`. Contenu :
- **Identité** : nom, version/build, courte phrase de philosophie (non-destructif).
- **Outils tiers** (Assetto Corsa, Content Manager, QuickBMS) : description, auteur/studio, lien externe, mention **non-affiliation** par outil (Kunos Simulazioni, gro-ove, Luigi Auriemma). Content Manager marqué **requis**, QuickBMS marqué **optionnel** (non embarqué — export seulement, §10).
- **Soutien & communauté** : lien **PayPal** (don libre, pas d'abonnement), profil OverTake, lien vers le **dépôt source** (code ouvert), lien « signaler un bug », journal des versions.
- **Licence** : Pit Box est **open source, sous licence GPL v3** — le code source est public ; toute version dérivée distribuée doit rester elle aussi sous GPL v3 (empêche un fork fermé/revendu sans partage). Bandeau légal à mettre à jour en conséquence (mention GPL v3 au lieu de « tous droits réservés »). Éligible à la signature de code **gratuite** via SignPath Foundation (programme pour projets open source qualifiants) plutôt qu'un certificat OV payant.
- **Bibliothèques open source** utilisées par Pit Box lui-même (Tauri, React, crates Rust, paquets npm) : liste repliable, avec licence de chacune. Nécessaire pour les licences MIT/Apache qui exigent l'attribution — liste générable automatiquement depuis `Cargo.toml`/`package.json`.
- **Bandeau légal** : non-affiliation générale, mention marque déposée (Assetto Corsa = Kunos Simulazioni), mention de la licence **GPL v3** du code de Pit Box.

## 13. Conventions

- **Langues** : le **code** (identifiants, commentaires, tests) est en anglais — l'app est destinée à être publique. Les échanges de travail et cette documentation restent en français. Les chaînes visibles par l'utilisateur ne sont **jamais** en dur : elles passent par l'i18n (`fr.json` + `en.json`).
- **Erreurs remontées à l'UI** : ce sont des **clés i18n** (`errors.*`, constantes de `src-tauri/src/errors.rs`), pas des phrases — une phrase codée en dur ne se traduit pas. Le frontend les résout via `errorText()`. Les erreurs purement techniques (E/S, SQLite) restent en texte brut, comme diagnostic.
- **Intégration continue** : `.github/workflows/ci.yml` (types, build, clippy `-D warnings`, tests, empaquetage) sur chaque push ; `release.yml` sur tag `v*`. Signature de l'installateur : voir `windows-code-signing.md`.
- **Thème Rosso Corsa** : #d40000 sur fonds sombres (#08080c/#0d0d12), coins carrés, police mono pour les données, esthétique « pit garage » industrielle, logo « PITBOX » italique.
- **Logos officiels** : dans les maquettes, monogrammes placeholder (on ne reproduit pas les logos de marque officiels) ; l'app réelle lit `ui/badge.png`.
- **Tokens de design** en variables CSS ; Claude Code extrait les tokens et reproduit le look en composants Tauri (ne pas copier le HTML des maquettes inline).

---

## 14. Références (fichiers du dossier docs/)

Voir `README.md` pour l'index complet. Fichiers de données et maquettes cités ci-dessus :
- `kunos_content_dates.json` — années + dates de publication du contenu officiel Kunos.
- `default-tag-rules-enriched.json` — ontologie de tags.
- `pitbox-biblio-session2.html` — barre latérale unifiée (référence écran principal).
- `pitbox-reglages-session.html` — écran de réglages de session.
- `pitbox-fiche-B-revisee.html` — fiche voiture (référence layout).
- `pitbox-vues-transversales.html` — vues Skins/Sons/Apps.
- `pitbox-source-pack.html` — affichage pack d'origine.
- `archives.py` — logique d'import/détection à porter (référence, jamais exécutée ; ne jamais réécrire les `ui_*.json`).
- `spec-module-musique_2.md` — spec de référence du module musique (§16), écrite pour une autre stack (C#/NAudio) : les écarts de transposition Rust/`rodio` sont documentés en tête de `src-tauri/src/music/engine.rs`, pas ici.

---

## 15. Points à vérifier

- **Bascule symlinks → hardlinks (§2)** : moteur implémenté et couvert par des tests automatisés (déploiement/composition/repli copie/nettoyage, y compris un scénario circuit type Spa) — confirme la mécanique et l'absence de besoin de droits admin (`CreateHardLinkW`, contrairement à `CreateSymbolicLink`). **Validé en conditions réelles par l'utilisateur** (juillet 2026) : déploiement + composition par couches fonctionnels sur sa bibliothèque réelle.
- **Détection de la stack météo** (Pure/SOL/CSP/vanilla) et correspondance preset → backend.
- **Table Kunos** : valider les noms de dossiers / années contre l'installation réelle (correction triviale ligne par ligne).
- **Module musique (§16)** : implémenté et testé sur les parties pures (courbes de fondu, mélange sans répétition, playlist, config, RMS/index §16.3), mais pas encore validé à l'oreille par un humain — l'app tourne sans dossiers musicaux pré-remplis (pas de pack CC0 embarqué, voir §16). Le premier scan d'un dossier (décodage complet de chaque piste pour le RMS) est bloquant côté thread moteur — "quelques secondes pour 30 pistes" par la spec §3.4 : à confirmer que ce n'est pas gênant à l'usage (silence de quelques secondes à la première entrée en Big Picture sur un nouveau dossier) avant d'investir dans un scan progressif avec barre de progression.
- **Détection AC_LIVE (§16.2)** et **filet de sécurité plein écran (§16.5)** : le champ `Status` et les offsets utilisés viennent d'une implémentation tierce open source, pas testés avec une vraie session AC en cours de développement (pas d'AC installé sur la machine de dev). À confirmer en conditions réelles : la musique GRID doit continuer pendant le chargement, se couper/baisser exactement quand la voiture devient pilotable, et le plein écran doit couvrir l'écran entier sans laisser la zone de l'ancienne barre des tâches visible.

---

## 16. Mode Big Picture et musique

Bouton dans la barre de titre (icône à côté de l'aide « ? », `TitleBar.svelte`) : bascule la fenêtre en plein écran (`Window.setFullscreen` + repli explicite sur les bornes du moniteur, voir §16.6 — pas de 10-foot UI dédiée, c'est l'interface habituelle, agrandie) et démarre l'ambiance musicale si activée. **La barre de titre custom est masquée en Big Picture** (gagne en hauteur, plus aucun sens une fois plein écran). Seule sortie visible : bouton collant en bas de la barre latérale (`position: sticky`, jamais par-dessus les boutons de navigation même si la fenêtre est basse), ou touche **Échap**. Un **zoom dédié** (`prefs.bigpicture_zoom`, §11) s'applique en plus du zoom normal, pensé pour une lecture à distance manette en main.

### 16.1 Musique — périmètre retenu

Transposition du document `spec-module-musique_2.md` (écrit pour une stack C#/.NET + NAudio) vers Rust/Tauri avec la crate `rodio`. Décidé avec l'utilisateur, périmètre **noyau du module** :

- Moteur audio à deux ambiances (MENU pendant la navigation, GRID sur l'écran de paramétrage de session `race`), crossfade à puissance constante, machine à états MENU/GRID/SESSION (`src-tauri/src/music/engine.rs`).
- Détection du lancement d'Assetto Corsa (`acs.exe`/`AssettoCorsa.exe`, polling 500 ms) **et** de la fin du chargement (mémoire partagée AC, §16.2) pour couper la musique seulement une fois la voiture réellement en piste — l'ambiance GRID continue de jouer pendant tout l'écran de chargement — puis fade-in au retour. **Toujours coupée pendant une session, jamais baissée en fond** (décidé avec l'utilisateur — l'option "duck" a existé puis a été retirée : en course comme en essais, plus de musique de préparation une fois la voiture en piste).
- Sélection de dossier par Parcourir (menu/grid), écoute au clic, fichier de config séparé (`music.json`, versionné, jamais fusionné dans `config.json`).
- Normalisation RMS entre pistes + cache d'index par dossier (§3.4, `src-tauri/src/music/index.rs`) — voir §16.3.

**Pack par défaut embarqué** (`src-tauri/assets/music/`, décision revue avec l'utilisateur — initialement hors périmètre) : deux pistes sous **Pixabay Content License** (usage libre, redistribution incluse ; crédits dans `assets/music/CREDITS.md` et l'onglet À propos), embarquées via `include_bytes!`. C'est le comportement **par défaut, sans configuration** : `MusicConfig.use_custom_folders` (case « Utiliser mes propres dossiers de musique » dans Réglages > Musique) vaut `false` par défaut, auquel cas les deux ambiances jouent le pack embarqué — déposé dans un dossier dédié entièrement piloté par l'app (`app_config_dir/Music/embedded/{menu,grid}`), **réécrit à chaque démarrage** pour rester synchronisé avec le binaire (une mise à jour de l'app peut changer les pistes). Cocher la case révèle les sélecteurs de dossier menu/grid (repli sur `app_config_dir/Music/{menu,grid}`, vides, tant qu'aucun n'est choisi) — ces dossiers-là restent la propriété de l'utilisateur, jamais réécrits par l'app.

**Écarté pour de bon** (pas seulement reporté) : la détection automatique des bandes-son Steam (liste déroulante « Bandes-son détectées », §3.2 de `spec-module-musique_2.md`) — décidé avec l'utilisateur, aucun intérêt pour son usage. Le sélecteur de dossier par Parcourir suffit.

### 16.2 Détection de fin de chargement

`acs.exe` reste le même process du début du chargement jusqu'au retour aux stands/résultats — sa seule présence ne dit donc pas si la voiture est pilotable. Plutôt que de scruter des logs (format instable d'une version à l'autre, coût d'I/O disque à chaque scrutation — sensible pendant la course, précisément quand on scrute le plus), `src-tauri/src/music/ac_status.rs` lit la **mémoire partagée officielle d'AC** (`Local\acpmf_graphics`, l'API utilisée par tous les tableaux de bord tiers — SimHub, CrewChief…) : une simple lecture mémoire, de l'ordre de la microseconde, jamais de disque. Le champ `Status` (`AC_STATUS`, un `int32` juste après `PacketId`) vaut `AC_LIVE` (2) uniquement quand la voiture est réellement en piste — `AC_OFF`/`AC_REPLAY`/`AC_PAUSE` le reste du temps, chargement compris.

`watch.rs` scrute la présence du process toutes les 500 ms (inchangé) et, seulement une fois le process détecté, le statut `AC_LIVE` toutes les 1000 ms. Trois signaux distincts envoyés au moteur : `AcProcessStarted`/`AcProcessStopped` (repère d'état pur, aucun effet sur la lecture — sert uniquement à `enter_big_picture` pour rester silencieux si Big Picture s'ouvre pendant qu'AC tourne déjà) et `EnterSession`/`ExitSession` (le fondu réel, déclenché par la transition `AC_LIVE`). `AcProcessStopped` reste aussi un filet de sécurité : si la mémoire partagée n'a pas signalé la sortie de `AC_LIVE` (fermeture brutale d'AC), la fermeture du process force quand même la reprise de la musique.

### 16.3 Normalisation RMS + cache d'index

`src-tauri/src/music/index.rs` : au premier scan d'un dossier (première entrée en Big Picture après avoir pointé vers ce dossier), chaque piste est décodée en entier via `rodio` — pas de bibliothèque audio de plus, le décodage complet est de toute façon nécessaire pour calculer le RMS — pour en tirer une correction de gain vers -18 dBFS (bornée à ±12 dB, §3.4) et sa durée exacte. Le résultat est mis en cache dans le dossier lui-même (`.pitbox-index.json`), invalidé si le nombre de fichiers ou la date de modification du dossier changent. **Toujours actif, pas de réglage pour le désactiver** (décidé avec l'utilisateur) ; le gain s'applique en plus du fondu/session courant, recalculé à chaque tick plutôt que figé au chargement.

Bénéfice secondaire : la durée exacte obtenue au passage comble l'écart documenté en §16.4 pour le préchargement du crossfade — `engine.rs` la préfère désormais à `Source::total_duration()` (souvent `None` pour un MP3 décodé en direct), donc le vrai recouvrement `crossfade_ms + 500ms` s'applique aussi aux MP3, pas seulement au WAV/FLAC.

Écart assumé vs la spec : le tag ReplayGain n'est pas lu ("si présent, le préférer au calcul", §3.4) — lecture de tags audio = une dépendance de plus (`lofty`/`id3`) pour une préférence secondaire ; le calcul RMS s'applique donc systématiquement.

### 16.4 Écarts assumés vs la spec d'origine

Documentés en tête de `engine.rs`, résumé ici :
- `rodio`/`cpal` mixent et rééchantillonnent déjà en interne (un `Sink` par piste dans le même `OutputStream`) — pas besoin de rejouer à la main la chaîne `MixingSampleProvider`/`WdlResamplingSampleProvider` de NAudio décrite par la spec.
- Sortie WASAPI **partagée** par défaut (jamais exclusive, qui couperait le son d'AC) — comportement natif de `cpal` sur Windows, rien à configurer.
- Préchargement (§5.3 de la spec musique) : la durée totale d'une piste n'est connue à l'avance que pour certains formats (WAV/FLAC typiquement). Quand elle l'est, le crossfade démarre bien `crossfade_ms + 500ms` avant la fin ; sinon (la plupart des MP3), il démarre quand `Sink::empty()` devient vrai — la piste précédente est alors déjà silencieuse, donc ce qui reste du crossfade se comporte comme un simple fondu d'entrée plutôt qu'un vrai recouvrement.
- Chemins stockés en absolu (`Option<PathBuf>`, cohérent avec `AppConfig`), pas en variables d'environnement non résolues — la portabilité multi-machine visée par la spec avait du sens pour un `%APPDATA%\<AppName>` C#, moins ici où `app_config_dir()` est déjà par-utilisateur.

### 16.5 Interface

Écran Réglages > onglet **Musique** (`components/settings/MusicTab.svelte`) : coupe-circuit, case « Utiliser mes propres dossiers de musique » (décochée par défaut, pack embarqué), sélecteurs de dossier menu/grid affichés seulement si cochée (Parcourir + écoute ▶, nombre de pistes détectées), lecture aléatoire, volume, durée de fondu. Sauvegarde indépendante des trois autres onglets (fichier séparé).

Écart assumé vs `spec-module-musique_2.md` (§2) : l'option « baisser le volume en fond pendant une session » (mode "duck") a existé puis a été retirée (décidé avec l'utilisateur) — la musique s'arrête désormais systématiquement au démarrage d'une session, aucun réglage pour changer ce comportement. Contrôle d'un lecteur média **externe** (Spotify, foobar2000…) envisagé séparément, pas encore implémenté : voir « Chantiers en cours » de `CLAUDE.md`.

### 16.6 Plein écran — filet de sécurité Windows

Sur une fenêtre sans décorations (`decorations: false`), `Window.setFullscreen(true)` peut ne couvrir que la **zone de travail** (écran moins la barre des tâches) plutôt que l'écran entier — bug constaté (zone en bas de l'écran, là où était la barre des tâches, restée hors fenêtre et visuellement cassée). `bigpicture.svelte.ts` force donc explicitement les bornes du moniteur courant (`currentMonitor()` + `setPosition`/`setSize`) après l'appel à `setFullscreen`, et restaure la taille/position d'avant (mémorisées, pas seulement celles que `setFullscreen(false)` sait annuler tout seul) à la sortie.
