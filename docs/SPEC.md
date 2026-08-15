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
- **Chemins de bibliothèque stockés relatifs, pas absolus** (`src-tauri/src/libpath.rs`) : `library_path` (versions, couches, sous-éléments, apps, autres mods) et `kept_archive_path` sont enregistrés **relatifs à la racine de bibliothèque**, jamais en chemin absolu figé sur la machine d'import. Sans ça, migrer la bibliothèque vers un autre disque ou un autre PC (robocopy + copie du dossier de config) laisse chaque ligne pointer vers un chemin qui n'existe plus, même quand tous les fichiers sont bien arrivés — la copie de fichiers ne suffit pas si les métadonnées restent figées. Un seul changement de `library_path` dans les Réglages suffit alors à tout refaire résoudre. **Compat ascendante** : une ligne écrite avant ce format reste en absolu, reconnue et utilisée telle quelle (`libpath::resolve`) — jamais cassée sans action explicite. Un outil de maintenance ponctuel (`maintenance::relativize_library_paths`, écran Maintenance) convertit les bases existantes : il retrouve la partie portable de chaque chemin via la structure interne connue de sa table (`<type>/<id>`, `layers/<parent>`…), sans avoir besoin de connaître l'ancienne racine.

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

**Extraction et rangement en pipeline.** L'archive N+1 se décompresse pendant que la N se range — les deux saturent des ressources différentes. Le canal est un rendez-vous, ce qui borne l'avance à une archive et donc à deux dossiers temporaires vivants au plus.

**Verrou base réduit au rangement.** Extraction et copie de l'archive source, qui ne touchent pas la base, se font hors verrou — un écran qui lit l'overlay n'attend plus la décompression d'un gros circuit. Le rangement d'un mod, lui, garde le verrou : il entrelace décisions et écritures overlay, et le relâcher au milieu ouvrirait une fenêtre où l'UI pourrait modifier ce qu'on est en train d'écrire.

**Contrôle d'espace disque.** Un lot dont la taille dépasse l'espace libre du volume de la bibliothèque est refusé **avant** d'écrire quoi que ce soit. Jamais bloquant sur une information absente (bibliothèque non configurée, volume non interrogeable).

**Annulation.** Constatée **entre deux items** — et 7-Zip est tué s'il décompresse. Jamais au milieu du rangement d'un mod, qui laisserait une bibliothèque à moitié écrite. Le rapport affiche ce qui a été importé avant l'arrêt.

**Rapport de fin cliquable.** Chaque contenu importé ouvre sa fiche. Une **couche** ouvre le contenu de base auquel elle se rattache (§4.4). Skins et sons sont regroupés par contenu parent — une ligne par parent, pas par livrée. Apps et « autres mods » ouvrent leur écran. Un mod resté **ambigu** n'est pas cliquable : rien n'a encore été écrit.

### 4.3 Mise à jour vs couche (recomposition)

**Pas d'historique de versions conservé** (choix assumé pour la place disque). Une mise à jour remplace. Le filet de sécurité contre les pertes n'est pas le rollback mais le **modèle de couches** (la base reste toujours une entité intacte).

**Détection à l'import sur un contenu existant** : comparer les fichiers.
- Fort chevauchement des fichiers existants → **mise à jour** (remplace).
- Majorité de chemins nouveaux, peu de fichiers écrasés → **couche/extension** (ajoute).
- Détection **auto**, question à l'utilisateur **seulement si ambigu**, avec récapitulatif chiffré (« ajoute 84 fichiers, en écrase 6 sur 412 »).

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

#### 4.5.3 Ajouts au jeu → `extras/` en bibliothèque, posés dans AC

Ce qu'AC lit ailleurs que dans `content/<type>/<id>` : configs CSP (`extension/config/cars/rss/<id>/…`), shaders (`system/shaders/…`), textures d'équipe (`content/texture/…`), modèle de pilote (`content/driver/…`).

**Stockés bruts, avec leur chemin relatif à la racine d'AC**, dans `<lib>/extras/<type>/<id>/…` — jamais dans la version, qui est déployée telle quelle dans `content/`. Au **niveau du mod** comme `resources/` : une mise à jour remplace ses propres fichiers, les couches partagent le même arbre.

Deux propriétés en découlent :

- **L'import ne jette rien.** Ce qui n'est pas classé est conservé tel quel, donc l'*interprétation* — où poser, qui arbitre un fichier partagé — reste recalculable depuis la bibliothèque à tout moment. Aucune règle des versions précédentes à mémoriser, aucune archive à conserver : c'est l'**entrée** qui est préservée, pas la décision. C'est ce qui rend un futur changement de règles rattrapable sans rien versionner.
- **L'ajout vit et meurt avec son mod.** Posé à l'activation, retiré à la désactivation, supprimé avec lui. Le passage par « autre mod » (§7.3) ne donnait pas ça : les fichiers d'une voiture supprimée restaient dans AC, rattachés à une entrée anonyme que plus rien ne reliait au mod.

**Rattachement** d'un reste (§7.3), dans cet ordre : le chemin contient l'id d'exactement un mod reconnu de l'archive ; sinon l'archive ne livre qu'un seul mod, et tout ce qui l'entoure lui appartient. *Limite assumée* : dans un pack multi-mods, un reste que rien ne rattache reste un « autre mod » — le rattacher à tous dupliquerait des arbres parfois lourds, et « autre mod » ne perd rien. Un **document isolé** à la racine reste une annexe (§4.5.2) et va dans les ressources du mod, jamais dans AC : sans ce test, un `Read Me.pdf` deviendrait un ajout au jeu posé à la racine d'Assetto Corsa.

**Pose fichier par fichier** (hardlink), jamais par jonction de dossier : plusieurs mods visent les mêmes arbres (`extension/textures/common/rss/…` est livré à l'identique par chaque voiture RSS), et une jonction en donnerait la propriété exclusive au premier arrivé.

**`content/fonts` et `content/driver` ne sont pas un cas particulier** : ce sont des ajouts au jeu comme les autres. Ils ont eu leur propre mécanisme — copie globale dans l'install AC, jamais désactivée, écrasement par défaut en cas de collision — retiré pour trois raisons : il était déjà court-circuité (le balayage des restes, §7.3, les ramassait avant lui) ; il faisait cohabiter deux politiques contradictoires (« jamais désactivé » ici, « vit et meurt avec son mod » là) ; et son écrasement par défaut contredisait la règle d'or n°5. Le checksum anti-triche d'AC porte sur `data.acd` et `surfaces.ini`, pas sur les fonts/drivers.

#### 4.5.4 Poser sans écraser : réclamation, date, sauvegarde

Poser un fichier dans AC pose deux questions que `content/<type>/<id>` ne pose jamais : **plusieurs mods peuvent viser le même chemin**, et **ce chemin peut déjà être occupé** — par du contenu Kunos, par un mod installé hors de l'app, ou par un autre mod de la bibliothèque. Trois règles y répondent, et elles valent pour **les deux** mécanismes de pose : les ajouts au jeu (§4.5.3) et les mods « autres » (§7.3).

**1. Compteur de références.** Chaque mod *réclame* les chemins d'AC dont il a besoin (`extra_links`). Un fichier n'est retiré d'AC que lorsque plus aucun mod ne le réclame. Désactiver une voiture RSS n'emporte pas `extension/textures/common/rss/…` dont onze autres dépendent, et il n'y a plus de course à la propriété : le premier arrivé ne gagne rien.

**2. Arbitrage par date.** L'exemplaire à la **date de modification la plus récente** gagne, un mod plus récent corrigeant en général des bugs de celui d'avant. La date traverse la chaîne intacte : 7-Zip restitue celle stockée dans l'archive, `std::fs::copy` la conserve sous Windows, un hardlink partage l'entrée MFT. À égalité (archives repackées par un tiers, qui perdent les dates), c'est le **dernier mod installé**. L'arbitrage se rejoue dans les deux sens : quand le fournisseur s'en va, le fichier repasse à l'exemplaire du meilleur réclamant restant. **Un exemplaire plus ancien, ou de même date, ne déloge jamais ce qui tourne déjà** — sans cette comparaison, le dernier mod installé écraserait une font déjà mise à jour par un autre outil.

**3. Sauvegarde avant écriture.** Un fichier que **personne ne réclame** — contenu Kunos, mod posé à la main, reste d'une version antérieure de l'app ou de Content Manager — relève du même arbitrage, mais il n'est remplacé qu'après mise à l'abri de l'original, et il revient dès que plus aucun mod ne réclame le chemin. Il n'est en revanche **jamais supprimé** : personne ne l'ayant réclamé, rien ne dit qu'il est de trop. Un nettoyage éclairé des orphelins reste possible plus tard, une fois qu'on peut distinguer réclamé et non réclamé.

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

Les deux onglets sont **absents du panneau latéral `ModDetail`** (§6) : les listes de fichiers vivent dans la page pleine.

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
manette. Pose `nav.lightboxOpen` tant qu'elle est ouverte : la navigation
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

Mods de type non reconnu (shaders, configs CSP, mods d'UI, weather patterns…) : listés dans « Autres mods », activables/désactivables (hardlinks) comme les autres. Priorité notée + conflits signalés (pas de moteur de superposition type MO2).

**Pas de notion de mise à jour.** Réimporter une archive dont l'id existe déjà en bibliothèque ne fait rien — ni remplacement, ni erreur, silencieusement ignoré. Pour reprendre un mod « autre » modifié, il faut d'abord le supprimer.

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
naturelle). Visibilité, ordre et largeurs persistés ensemble. Le tableau
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

**Sélection multiple (Ctrl/Alt)** : ouvre un **panneau en bas, en surimpression** par-dessus la grille (le panneau de droite continue d'afficher le dernier mod cliqué ; le layout de la grille ne bouge pas en largeur). Champs d'édition en masse : tags (ajout/retrait), activation, suppression, favori, catégorie, export. Les champs propres à une voiture (specs, skin piloté) ne sont pas proposés en masse.
- Quand plusieurs **véhicules** sont sélectionnés, deux actions supplémentaires dans ce panneau : **« Définir en tant qu'adversaires »** (vide la liste d'adversaires puis ajoute la sélection) et **« Ajouter en tant qu'adversaires »** (ajoute à la liste existante). Les deux basculent le mode adversaires de la session Course sur **« Libre »** ; si on était sur « même voiture » ou « même catégorie », les adversaires de ces modes sont récupérés dans « Libre » en plus de la sélection.
- Même paire d'actions pour une **seule** voiture, sans passer par la sélection multiple : dans le menu clic droit d'une carte/ligne (« Définir comme adversaire » / « Ajouter comme adversaire »), comportement identique.

**Suivi d'usage** : distance parcourue par voiture/circuit ; filtre « jamais essayé » (0 km CM **et** jamais lancé via l'app, l'app tenant son propre marqueur fiable).

**Filtre « Cacher le contenu de base »** : exclut le contenu Kunos (`is_stock`) de la liste, cases favoris/jamais essayé/contenu de base regroupées et alignées verticalement dans la barre de filtres.

**Persistance du duo de session** (`src-tauri/src/session_state.rs`, `app_config_dir/session.json`) : fichier écrit côté Rust, pas `localStorage` du webview. `localStorage` n'est pas garanti synchrone sur disque côté WebView2 — bug réel constaté : le circuit, typiquement choisi juste avant de fermer l'app, ne survivait presque jamais à un redémarrage, contrairement à la voiture (choisie plus tôt, le temps d'être vidangée sur disque). `std::fs::write` est synchrone : la commande `save_session_picks` ne rend la main qu'une fois réellement écrit. Migration silencieuse au premier démarrage après la mise à jour : si le nouveau fichier n'a rien pour une entité, `nav.svelte.ts` relit une dernière fois l'ancienne clé `localStorage` et la re-persiste aussitôt au nouvel endroit.

**Garde-fou activation au lancement** (`AppShell.svelte`) : lancer une session avec une voiture ou un circuit sélectionné mais non activé (jamais junctionné dans `content/`) fait planter Content Manager/AC, qui ne trouve pas le contenu — bug réel signalé. L'état d'activation du duo n'est jamais déduit de `SessionPick` (juste id/nom/preview pour l'affichage, persisté tel quel — une donnée d'activation qui y serait figée resterait fausse dès que l'état change ailleurs, ex. désactivé depuis la fiche détail) mais interrogé à chaque changement de sélection via `get_mod_detail`, comme `trackDetail` pour le sélecteur de layout. Icône ⚠ (jaune, `title` natif) sur le nom du slot concerné dans la barre latérale tant qu'il n'est pas activé. Cliquer « Démarrer la session » avec un duo non activé bloque le lancement, demande confirmation, active le(s) mod(s) concerné(s) puis ne poursuit vers l'écran de réglages (`nav.autoLaunch`) qu'une fois l'activation réussie — jamais d'activation silencieuse sans accord explicite, jamais de lancement si elle échoue.

**Support manette** (`src/lib/gamepadNav.ts`) : navigation dans l'application à la manette. En mode automatique (défaut), seules les manettes que le navigateur reconnaît avec `mapping === "standard"` pilotent la navigation — un volant (Fanatec…) n'a jamais ce mapping (ses axes/boutons ne suivent pas le layout Xbox supposé par ce module : pédales/rotation au lieu du stick, boutons à d'autres indices), donc il est ignoré par défaut plutôt que de faire dériver le focus. Réglages > Général propose un filet de sécurité : forcer une manette précise par son `id`, ou désactiver complètement la navigation manette (`pitbox.gamepadNav.mode` dans `ui_prefs.json`) — accompagné d'un tableau de diagnostic en direct (mapping/axes/boutons de chaque périphérique détecté), lui-même l'outil servant à relever les valeurs d'un nouveau volant.

**Overrides par périphérique** (`DEVICE_OVERRIDES` dans `gamepadNav.ts`) : un volant n'a aucune norme fiable pour sa croix directionnelle — souvent un hat switch matériel rapporté comme un seul axe à valeurs discrètes (pas un vrai stick 2D), avec des valeurs propres au pilote/modèle. Un override par modèle constaté (identifié par une sous-chaîne de `Gamepad.id`, en support des cas où un même volant s'annonce sur deux entrées `Gamepad` distinctes au même `id` — base + interface boutons séparée) associe boutons de confirmation/annulation par index et positions du hat par valeur, localisées à l'exécution parmi tous les axes du périphérique (pas d'index fixe, non garanti stable). Ne se généralise pas d'un modèle à l'autre : chaque nouveau volant remonté nécessite sa propre entrée, relevée via le tableau de diagnostic ci-dessus. Modèle couvert à ce jour : base Fanatec ClubSport Wheel Base V2.5 (croix rapportée comme un axe à 4 positions discrètes, confirmé fonctionnel).

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

**Analyse des extensions CSP** : poussée plus loin (détection fine des fonctionnalités CSP d'un mod).

---

## 9. Lancement de session

### 9.1 La bibliothèque est le sélecteur

Pas d'écran séparé de sélection : la voiture/le circuit sélectionnés dans la bibliothèque sont ceux de la session. Le **bloc Session** de la barre latérale montre en permanence le duo courant. La page « Démarrer une session » ne contient aucune sélection de voiture/circuit — seulement les réglages + Lancer.

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
- Tours ou durée, faux départ, pénalités — communs à Course et Track day. **Case qualification** (durée en min, mini 5 — borne de CM) et, sous elle, **case essais libres** (durée en min) : **Course uniquement**, absentes de Track day (§9.2quater, aucun mode Weekend équivalent côté CM). Décocher la qualification décoche les essais libres : les deux n'existent que dans le mode Weekend de CM, et sans qualification le preset bascule sur son mode course sèche, où aucune phase préparatoire n'existe (§9.2ter). Laisser les essais cochables y afficherait un réglage sans effet en jeu.

**Régénération du plateau** (Course et Track day) : le vivier dépend de la voiture pilotée, donc changer de voiture régénère les adversaires — sauf en mode **Libre**, dont le vivier n'en dépend pas et où le plateau est le plus souvent réglé à la main. Changer seulement de **skin** ne régénère jamais. La voiture pour laquelle le plateau a été construit est persistée avec lui (`grid_car_id`) : l'écran de lancement est démonté dès qu'on passe à la bibliothèque, c'est donc la seule façon, au remontage, de distinguer un plateau fait pour la voiture courante d'un plateau hérité de la précédente — sans ça, changer de voiture depuis la bibliothèque puis revenir laissait le plateau de l'ancienne (bug réel).

**Practice** : pas de champ durée (non applicable — session à durée libre par design Quick Drive, voir §9.2 ; pas de champ correspondant côté Pit Box), départ (Stand/Piste → `StartType` du `ModeData` ; "Piste" non vérifiée sur un preset réel, voir commentaire `mode_data_practice`).

**Hotlap** : ghost car.

**Météo** : conditions en **icônes SVG stylisées** (thème, libre de droits) — Beau, Quelques nuages, Couvert, Brouillard, Pluie légère, Pluie, Orage. **Température, vent et heure implicites** sur une même ligne (heure modifiable, température/vent recommandés par condition + heure + stack SOL/CSP, tous corrigeables à la main). **Saison** optionnelle : un champ date natif (en premier, avant les 4 cartes saison) qui affiche/permet de corriger précisément la date associée — sélectionner une saison y reporte automatiquement la date calculée (milieu de saison), la modifier à la main ne désélectionne pas la saison affichée.

**Presets de session par type** : chaque type (Practice/Hotlap/Course/Track day) a un preset mémorisé ; toute modif est persistée pour les prochaines sessions du même type. **Persistance** (`src-tauri/src/session_state.rs`, `app_config_dir/launch_state.json`) : fichier écrit côté Rust, même mécanisme et même raison que le duo de session (§7.4) — `localStorage` n'est pas garanti synchrone sur disque côté WebView2, ce qui pouvait perdre les presets et la dernière sélection (type de session, adversaires) à la fermeture de l'app. Fichier dédié, distinct de `session.json` (chaque commande réécrit tout son fichier ; les mélanger ferait que sauvegarder le duo de session écrase les presets, et inversement). Migration silencieuse au premier démarrage après la mise à jour, même schéma qu'en §7.4.

**Sessions enregistrées** (carte dédiée, à droite de « Type de session », surtout utile pour la liste d'adversaires) : liste inline, scrollable, **filtrée par le type de session courant** — change avec l'onglet Practice/Hotlap/Course/Track day. Cliquer une entrée la charge immédiatement. Bouton **Sauvegarder** au-dessus de la liste ouvre une popup de nommage (ou sélection d'une sauvegarde existante du même type, pour l'écraser). Clé par `<type>::<nom>` : deux types peuvent avoir une sauvegarde du même nom sans collision. **Persistance** (`src-tauri/src/saved_sessions.rs`, `app_config_dir/saved_sessions.json`) : même mécanisme et même raison que le duo de session et les presets ci-dessus — fichier dédié écrit côté Rust, migration silencieuse depuis `localStorage` au premier démarrage après la mise à jour.

**Contenu d'une session enregistrée** : les réglages (météo, adversaires, options) **et le duo de session** — voiture pilotée avec son skin, circuit avec son tracé et ses **skins de circuit actifs** (§8, seul élément hors `setup` : c'est un état de déploiement, d'où un champ `trackSkins` à part). Le chargement rétablit le tout en passant par le duo de session (§8.6), qui reste la source de vérité : voiture et circuit sont reposés via `pickSession`, pas écrits directement dans le setup. Les skins de circuit sont remis **à l'identique** — ceux qui manquent sont activés, ceux en trop désactivés, sinon un skin resté actif d'une session précédente changerait l'apparence du circuit sans que rien ne le signale. Une sauvegarde antérieure au champ `trackSkins` (`undefined`, distinct d'une liste vide) n'y touche pas du tout.

**Chargement partiel** : un mod supprimé depuis la sauvegarde n'interrompt jamais le chargement — la sélection courante est conservée pour ce qui manque (voiture, circuit), le tracé retombe sur celui par défaut, et un **bandeau d'avertissement jaune** en tête d'écran (même emplacement que le retour de lancement, jamais une popup : il n'y a rien à décider) énumère ce qui n'a pas pu être rétabli. Il s'efface au chargement suivant et au lancement de la session.

**Écran non prêt** : le corps de l'écran (colonnes de réglages) reste masqué derrière `LoadingState` (même indicateur que les listes de mods) tant que le chargement initial n'est pas terminé (bibliothèque, presets/sélection persistés, duo de session, météo par défaut) — évite que l'utilisateur voie les champs se réajuster au fur et à mesure que les valeurs mémorisées arrivent.

**Choix layout/skin de circuit** : sur la fiche/bibliothèque circuit, image d'aperçu (`preview.png`) avec le tracé du layout (`outline.png`/`map.png`) par-dessus, infos (longueur, virages, CSP).

### 9.4 Aperçu 3D des voitures

Bouton d'aperçu 3D sur la fiche (lance `acshowroom` pour un rendu du modèle). Le showroom est un **process indépendant**, affiché par-dessus l'app avec les réglages vidéo du jeu : l'utilisateur le ferme lui-même pour revenir à Pit Box. L'intégration de sa fenêtre dans la page a été tentée puis abandonnée (voir `showroom-3d-preview-research.md`), et avec elle l'option « 3D par défaut ». **Option de réglage** : le décor (`content/showroom/<id>`) chargé par `acshowroom`, choisi parmi ceux installés — défaut `studio_white`, le seul instantané et sans musique. Pendant le démarrage d'`acshowroom`, afficher une **animation de chargement en haut à droite** de l'image.

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

**Migration de bibliothèque — chemins relatifs** (écran Maintenance, `libpath.rs`) : les chemins enregistrés en overlay sont relatifs à la bibliothèque (§2), donc portables. Une base créée avant ce format (ou copiée avant une mise à jour de l'app) garde ses chemins en absolu jusqu'à passage explicite de l'action **« Convertir »** : retrouve la partie portable de chaque ligne via la structure interne connue de sa table, sans avoir besoin de connaître l'ancienne racine, puis réécrit `library_path`/`kept_archive_path` en base — aucun fichier déplacé ni copié. Sûr à rejouer (déjà relatif = ignoré) ; ce qui n'est pas reconnu est laissé de côté et listé, jamais deviné. Le contenu de base Kunos (`is_stock`) en est exclu : ses versions pointent vers `content/`, jamais la bibliothèque. Étape recommandée après une migration multi-machine, avant ou après « Réparer » ci-dessus (les deux sont indépendants et sans risque à combiner).

---

## 11. Configuration et préférences

**Chemins requis** (assistant de première configuration, détection auto si possible) : dossier d'install AC, bibliothèque, exécutable CM, 7-Zip, QuickBMS + script acd.bms (optionnels, export seulement). Détection auto (`detect.rs`) : AC via les bibliothèques Steam, Content Manager dans le dossier AC ou `%LOCALAPPDATA%\AcTools Content Manager`, 7-Zip dans ses emplacements standard ou, à défaut, le `7z.exe` que Content Manager embarque pour son propre usage (`%LOCALAPPDATA%\AcTools Content Manager\Plugins\7Zip\7z.exe` — beaucoup d'utilisateurs CM n'ont jamais installé 7-Zip à part). La bibliothèque n'a pas de détection à proprement parler (rien n'y existe encore au premier lancement) mais une **suggestion** pré-remplie dans le dossier utilisateur (`<home>\PitBox Library`, jamais Documents/Bureau/Images — redirigés vers OneDrive par défaut sur Windows, ce qui tenterait de synchroniser une bibliothèque de plusieurs centaines de Go), éditable comme les autres champs détectés.

**Trois bases/fichiers distincts** : bibliothèque (fichiers), base d'overlay SQLite (métadonnées), fichier de règles (ontologie), plus le fichier de config (chemins + préférences).

**Préférences persistantes** : affichage des tags du fichier mod (masquables), état du panneau de suivi (global), vue bibliothèque + colonnes (par type), presets de session (par type), preset CM graphique/FFB par défaut, décor de l'aperçu 3D (§9.4), regroupement des skins (archive/voiture), extraction des fichiers annexes (Aucun / Informations seulement / Tout — §4.5.2), **conservation de l'archive source** (défaut désactivé — §10), **mode de déploiement** (hardlink/symlink, défaut hardlink — §2), **zoom du mode Big Picture** (§16, distinct du zoom normal — `None` reprend ce dernier).

**Écran Réglages en onglets** (Général / Chemins / Import / Musique) depuis le mode Big Picture (§16) — Général/Chemins/Import partagent `AppConfig` et sa garde de navigation (§10bis), l'onglet Musique gère son propre fichier (`music.json`) et sa propre sauvegarde.

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
