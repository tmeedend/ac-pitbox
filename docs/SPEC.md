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

Avant tout cela, une question qui ne porte pas sur l'identité mais sur la nature : **ce dossier tient-il debout seul ?** Sans géométrie, il ne peut pas être un mod, quel que soit son nom — voir §4.3bis.

### 4.2 Sources d'import

Deux sources, même pipeline d'analyse/identité/tagging :
- **Archive** (`.zip`/`.rar`/`.7z`) — décompression puis analyse.
- **Dossier déjà décompressé** — analyse directe (cas clé : migrer un catalogue MO2 sans re-zipper).

**Le titre du rapport dit ce qui est entré, par nature** — « 1 voiture, 10 pilotes importés » et non « 1 élément importé ». Un décompte unique était juste au sens strict (un pilote est un « autre mod », pas un mod) mais laissait croire que les dix autres avaient été perdus ; les fondre dans un total unique effacerait au contraire la distinction qui dit **où** les retrouver. Un mod « autre » est nommé par la zone du jeu qu'il touche, seul nom de nature qu'il ait.

**Import en masse** : un dossier parent dont chaque sous-dossier direct est un mod. Scan sur un seul niveau. Flux en deux temps : phase d'analyse (scan sans rien écrire → récapitulatif : nouveaux / mises à jour / doublons / ambigus / ignorés), puis arbitrage groupé des exceptions, puis exécution. **« Ignoré » veut dire « rien à importer », pas « pas de voiture ni de circuit »** : un sous-dossier sans structure reconnue mais non vide (un mannequin de pilote nu, une notice, des images) part en « autre mod » à l'exécution, exactement comme s'il avait été importé seul (§7.3). Seul un dossier réellement vide est ignoré. Sans cette règle, l'import en masse écartait en silence tout ce que l'import d'un dossier isolé, lui, savait ranger.

**Copier / déplacer** : réglage par défaut mémorisable (pas deux boutons à chaque fois). Déplacement **adaptatif** : même disque → rename instantané ; disques différents → copie puis suppression après vérification.

**Interface d'import** : glisser-déposer disponible partout ; écran d'import dédié pour les options (chaque option expliquée). Un mod importé est **activé par défaut** (déploiement par hardlinks immédiat) — de même pour une app (junction) et un « autre mod » (junction/lien fichier, §7.3). Le glisser-déposer accepte **archives et dossiers** : le tri est fait côté backend (`split_dropped_paths`), le webview ne pouvant pas distinguer un dossier d'un fichier à partir du seul chemin. Un seul lot tourne à la fois.

### 4.2bis Progression, estimation et annulation d'un lot

Pensé pour un lot de plusieurs dizaines de mods.

**Une écriture de réglage qui échoue se voit.** La pile porte aussi une alerte jaune quand `ui_prefs.json` ne peut pas être écrit — elle ne se referme pas d'elle-même, parce que ce n'est pas une information de passage mais une perte de données en cours : ce qu'on règle reste actif jusqu'à la fermeture, puis disparaît. Ajoutée après un bug qui est resté invisible une journée entière — corps et tenues de pilote adoptés, rien sur le disque, et l'écran affichant sagement ce qu'on venait de choisir. Un `console.error` n'y suffisait pas : personne n'ouvre la console d'une app empaquetée.

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

**Une couche a une identité** : son parent et l'archive dont elle vient. Réimporter la même archive **remplace** la couche qu'elle avait posée — priorité reprise — au lieu d'en empiler une seconde, identique et inutile ; l'issue rapportée est alors une mise à jour, pas une extension. Remplacer plutôt qu'ignorer, parce qu'une archive au même nom peut avoir été corrigée, exactement comme un mod réimporté remplace sa version. Bug réel, `spa2022-release_V1-03.rar` (un layout posé sur le circuit Kunos) : deux imports donnaient deux couches, et le décompte affiché trahissait la mécanique — « 109 ajoutés · 0 écrasés » pour la première, comparée au circuit nu, puis « 0 ajouté · 109 écrasés » pour la seconde, comparée au circuit **déjà composé** avec la première. Une archive différente sur le même parent reste, elle, une seconde couche : c'est l'identité de l'archive qui compte.

**Règle absolue** : le **contenu de base** (Kunos, `is_stock`) ne reçoit **jamais** de mise à jour, **toujours** une couche. Garantit par construction qu'il ne peut pas être perdu. Même une « version améliorée complète » d'un circuit Kunos devient une couche posée sur la base intacte.

**Un mod installé hors Pit Box (`is_unmanaged`, §8) ne reçoit rien du tout** : ni mise à jour, ni couche. L'import s'arrête sur cet id, **rien n'est écrit**, et le rapport le dit (issue `UNMANAGED`, avec le décompte ajoutés/écrasés calculé en lecture seule, pour que l'utilisateur voie de quoi il s'agit). La raison n'est pas l'étiquette mais la conséquence : poser une couche entraîne la composition dans `content/`, donc la **sauvegarde puis l'effacement** du vrai dossier — un dossier que l'app n'a pas mis là. On n'y touche donc pas, jamais. Le garde-fou vit dans `layers::store_layer` plutôt que chez ses appelants : les quatre chemins qui posent une couche (import, dossier proposé par l'auteur §4.6ter, projection de sous-mod, et le prochain qui viendra) passent tous par lui.

**Modèle de couches recomposables** :
- La **base** (Kunos ou mod) reste une entité intacte, jamais fusionnée.
- Une **couche** (nouveau layout, améliorations, surcharge) est une entité séparée, intacte, rattachée à sa base.
- Ce que le jeu voit dans `content/` est un **résultat composé** : base + couches actives dans l'ordre de priorité.
- Désactiver/réordonner une couche = **recomposer** depuis les entités intactes (jamais de défaire chirurgical). Aucun état corrompu possible.

**Mécanisme, entièrement en hardlinks** (§2) :
- **Déploiement simple** pour les ~95 % de mods autonomes sans couche : hardlink direct de chaque fichier de la version active vers `content/<type>s/<id>`.
- **Composition** : même mécanisme, en superposant en plus chaque couche active (priorité croissante) sur la base — toujours un hardlink, jamais une copie de fusion. Retour au déploiement simple dès que la dernière couche est retirée.

**Contrôle** : ordre des couches modifiable + activation/désactivation par couche. Une couche peut se poser sur n'importe quel contenu (base ou mod).

**Ce que l'app affiche est le résultat composé, pas la version de base** (`library::entity_dirs`). Photos, tracés, layouts, `ui_*.json`, extensions CSP : tout se lit à travers une **pile** — couches actives par priorité décroissante, puis la version de base — et **fichier par fichier**, exactement comme `deploy::compose_tree` pose dans `content/`. Une couche qui ne remplace qu'un `preview.png` ne masque donc pas le `ui_track.json` de la base, et un layout que la couche ne touche pas garde sa photo d'origine. Les layouts sont l'**union** des deux : une couche peut en ajouter un. Les extensions CSP aussi — une couche ajoute ses fonctionnalités, elle ne remplace pas celles de la base.

*Pourquoi une pile et non une lecture de `content/`* : le résultat composé n'existe sur le disque que tant que le mod est **actif**. La fiche d'un mod désactivé doit dire la même chose que celle du même mod activé.

*Bug réel* : une couche remplaçait le `preview.png` d'un circuit ; le jeu affichait bien la nouvelle image, la fiche et la carte de bibliothèque continuaient d'afficher l'ancienne, **y compris après redémarrage** — parce que tout se lisait dans le dossier de la version de base, où une couche n'est par construction jamais écrite. Symétriquement, désactiver une couche doit faire réapparaître la base à l'écran comme en jeu : une couche inactive est exclue de la pile.

« Ouvrir le dossier » reste l'exception assumée : il désigne un vrai dossier de l'explorateur, donc la version de base (`entity_dir`), pas une pile.

### 4.3bis Fragments : une couche déguisée en mod

Un dossier peut avoir **la forme** d'un mod sans en être un. La détection de type ne regarde que `ui/` (`ui_car.json`, `ui/<layout>/ui_track.json`), et un dossier conçu pour être **posé sur** un mod existant porte exactement le même `ui/` — l'auteur le recopie pour livrer ses `preview.png`. Ce qui sépare les deux, c'est la **géométrie** : une voiture a son `.kn5` à la racine, un circuit en a un aussi ou un `models*.ini` qui nomme ceux qu'il charge. Un dossier qui n'a ni l'un ni l'autre **ne peut pas être chargé par le jeu**.

Mesuré sur tout le corpus de référence, **sans une seule exception** : 103 versions de circuit en bibliothèque, les 121 circuits de l'install AC et 123 versions de voiture portent leur géométrie. Le critère ne déclasse donc jamais un vrai mod — c'est ce qui permet d'**agir** dessus plutôt que de se contenter d'avertir.

**Bug réel qui a motivé la règle** : `Mike08_santamonica01`, une refonte visuelle de Santa Monica Mountains (`ui/` de deux layouts, `texture/`, `extension/ext_config.ini`, un `.vao-patch` — aucune géométrie). Nommée d'après son auteur, elle devenait un circuit de plus, **sans qu'aucune question ne soit posée** : l'identité d'un mod se réduisait au nom de son dossier, donc un fragment ainsi nommé ne rencontrait jamais l'arbitrage du §4.3. Résultat : une entrée que le jeu ne peut pas charger, et un circuit de base qui ne reçoit jamais ce qui lui était destiné.

**Trouver l'hôte** (`fragment.rs`), sources ordonnées de la plus sûre à la plus faible — même forme que `submods::resolve_sound_parent`, chacune chiffrée sur la bibliothèque de référence :

1. **le dossier porte déjà le nom d'un mod connu** — la règle d'identité historique, inchangée ;
2. **le `.vao-patch`** nomme le `.kn5` ou le `models*.ini` qu'il accompagne (**121/124** ; le nommer d'après l'id du circuit ne tient que pour 32/124, contrairement à l'intuition) ;
3. **les noms de layout** (circuit) ou de livrée (voiture) partagés avec un seul hôte — 185 des 224 paires (layout, circuit) portent un nom qui n'appartient qu'à un circuit, mais 13 noms génériques (`reverse` et `normal` chez 6 circuits, `short` chez 4) sont partagés : deux concordances sur le même hôte tranchent, une seule non ;
4. **un id d'hôte écrit dans le nom du dossier** ;
5. **le recouvrement de chemins** — quel hôte possède déjà les fichiers que le fragment apporte. Exige un vrai écart (au moins 2 concordances et le double du suivant) : `extension/ext_config.ini` existe seul chez des dizaines de circuits.

Deux candidats à égalité ne sont jamais départagés : poser le contenu sur un mod qu'il ne visait pas est pire que ne rien décider.

**Le dossier d'extraction ne devient jamais une identité** (`importer::incoming_name`). 7-Zip extrait à plat dans un dossier de travail temporaire : une archive dont le contenu est à la racine — la forme habituelle d'un fragment, un vrai mod devant porter son dossier d'id puisque AC le lit dans `content/<type>s/<id>` — fait donc de ce dossier de travail le dossier du mod. Son nom est alors repris de l'**archive**, privé de son extension. Un uuid ne désigne rien (cas réel : une couche rangée sous « pitbox-import-4df3c112-8c51-… ») et surtout il **change à chaque extraction**, ce qui cassait aussi la reprise après arbitrage : elle retrouve son mod par ce nom dans une seconde extraction, donc sous un autre uuid.

**Ce qui en découle** :

- **Hôte trouvé** → rangé en **couche**, automatiquement, sans rien demander : l'opération n'est pas destructive (§4.3) et la question n'a qu'une réponse. La décision est **visible** — la ligne du rapport nomme le dossier rangé *et* l'hôte, la couche apparaît dans « Couches & extensions » sous le nom de son dossier source, et l'historique de l'hôte porte sa ligne.
- **Un fragment n'est jamais une mise à jour**, quel que soit le décompte de fichiers et quelle que soit la décision demandée. Même règle absolue que le contenu de base (§4.3) : sans géométrie, remplacer la base par lui la rendrait injouable. C'est aussi le seul cas où le décompte ment — un fragment qui ne retouche que des `ui/` recouvre proportionnellement beaucoup d'un circuit qui en a peu.
- **Hôte nommé mais absent** → **rien n'est écrit**, on demande, et le défaut proposé est de **ne pas importer**. La troisième issue, « garder pour plus tard », range la couche sous l'id attendu sans rien poser dans le jeu : `compose::recompose` lit les couches par `parent_id`, que le mod existe ou non, donc l'hôte la reprend le jour où il arrive. Même parti que pour un son dont la voiture manque (§8.3).
- **Hôte introuvable** → on demande aussi, avec pour seules issues « ne pas importer » (défaut) et « importer quand même », qui produit l'ancien comportement, cette fois assumé et signalé dans le rapport.
- **En import de masse**, où l'on ne s'arrête jamais pour demander (§4.2bis), le défaut sûr est de **garder** : couche en attente si l'hôte est nommé, import tel quel sinon. Un mod de trop vaut mieux qu'un contenu perdu, et la ligne du rapport dit lequel.

**Un skin ou un son dont la voiture manque suit exactement la même règle** (§8.3) : rien n'est écrit, on demande, défaut « ne pas importer », et « garder pour plus tard » range sous l'id visé. Une livrée est du contenu posé **dans** une voiture — c'est la même chose qu'une couche, et deux comportements différents pour la même question ne se justifiaient pas.

Trois choses rendent cette reprise sûre, et elles existaient déjà :

- **Une clé stable** : `<parent_id>/<nom>`, qui ne dépend pas du dossier temporaire d'extraction. Elle contient un `/`, qu'un id de mod ne porte jamais, donc les deux espaces de clés cohabitent dans la même liste de décisions.
- **L'idempotence** : `sub_exists` empêche de réimporter un sous-élément déjà connu, donc rejouer l'archive entière pour trancher un seul élément ne duplique rien.
- **Un garde-fou de projection** : rien n'est posé dans le jeu pour un hôte absent. *Bug réel corrigé au passage* — `parent_content_dir` retombe sur `content/<type>s/<id>` pour un id inconnu (voulu pour le contenu Kunos, qui vit là), et la projection **créait** ce dossier : un `content/cars/<absente>/skins/` apparaissait dans l'install, et faisait ensuite échouer l'import de la vraie voiture, que `REAL_FOLDER_IN_CONTENT` refuse de recouvrir. Le même dossier fantôme que côté apps, en troisième variante.

Le fait est aussi rendu **localisable** : `SubImported` portait déjà un `warning`, mais en texte libre français, donc intraduisible sur six locales et de fait affiché nulle part. Deux booléens le remplacent (`parent_known`, `awaiting_decision`), que le rapport et la fenêtre d'arbitrage rendent correctement.

**Ce qui attend un hôte est listé sur l'écran Maintenance** (SESSION§3, `waiting_layers`). Ce n'est **pas une anomalie** — c'est le rangement voulu pour un contenu téléchargé avant sa base — et ça n'entre donc pas dans le « rien à signaler » de l'écran. Mais sans cette liste, une couche en attente est strictement invisible : la fiche qu'il faudrait ouvrir pour la voir est celle d'un mod qui n'existe pas encore. On peut y renoncer d'un bouton. Les apps comptent dans le test d'existence, sans quoi **toute** couche d'app passerait pour en attente — une app ne vit pas dans `mods`.

### 4.4 Packs multi-voitures

Chaque voiture d'un pack est une **entité de premier niveau** (activable/tagguable séparément), liée aux autres par une métadonnée `source_pack` (nom d'archive/dossier, connu dès l'import).

**Le pack possède aussi des ressources et des ajouts au jeu** (`extras::OwnerKind::Pack`, §4.5.3) : ce qu'une source livre autour de plusieurs mods lui appartient. Deux conséquences.

- Ses **ressources** apparaissent dans l'onglet Ressources de **chaque** membre, marquées « du pack » — comme un document resté dans le dossier du mod est marqué « dans le mod ». Une seule copie sur disque, lisible depuis toutes les fiches.
- Ses **ajouts au jeu** sont posés dès qu'au moins un membre est actif, et retirés quand aucun ne l'est (`extras::sync_pack`, appelé après chaque activation, désactivation ou suppression). Un pack n'est pas déployable en soi : c'est une métadonnée partagée, mais ce qu'il livre n'a de sens dans le jeu que tant qu'une de ses voitures y est. Le dernier membre supprimé emporte l'arbre du pack, ressources comprises. La fiche affiche un bloc « Source / origine » (pack cliquable, nom d'archive) et une section « autres voitures du même pack ». Actions : filtrer par pack, désinstaller le pack en lot. La rubrique « Provenance » de ce bloc s'adapte au type de contenu : nom d'archive pour un mod importé, **« Jeu de base »** ou nom du DLC (Dream Pack, Porsche Pack…) pour le contenu de base Kunos — résolu depuis `docs/kunos_content_dates.json` (`kunos_dates::pack_name`, même table que l'année/la date de publication estimées, §6.2).

**Le pack a sa fiche** (`PackDetail.svelte`, ouverte depuis le bloc « Source / origine » d'un de ses mods, posée **par-dessus** la fiche du mod — la fermer y ramène). Page pleine comme les autres fiches, pour la même raison : ce qu'elle montre, ce sont des listes de fichiers. Trois onglets — les **mods livrés** (vignettes cliquables vers leur fiche), les **ajouts au jeu** du pack, ses **ressources** — plus les tailles et la date d'entrée en bibliothèque, et la désinstallation en lot.

Elle existe parce que les ajouts au jeu d'un pack n'étaient affichés **nulle part** : `list_mod_extras` ne regarde que `extras/<type>/<id>`, jamais `extras/packs/<nom>`, et ces fichiers n'appartiennent par construction à aucun mod en particulier. Cas réel : un pack de 94 voitures livrant `content/{driver,fonts,texture}`, soit 82 fichiers correctement rangés, correctement posés dans le jeu, correctement retirés avec le dernier membre — et invisibles. Un pack **sans aucun membre** n'a pas de fiche vide : c'est une erreur (`errors.packNotFound`), puisque le dernier membre supprimé a emporté ses fichiers.

### 4.5 Ce qu'un mod pose, et où

> **`SPEC-import.md`** condense tout ce qui suit en un arbre de décision et une table de destinations, sur une page. C'est là qu'il faut regarder pour voir les règles *ensemble* ; ici, pour lire le raisonnement de chacune.


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
- **Informations seulement** (défaut) — extrait uniquement les fichiers légers d'information : `.txt`, `.pdf`, `.md`, `.doc`/`.docx`, `.rtf`, `.nfo`, `.html`, `.url`, `.lnk`, `.jsgme`. Le dernier est le descripteur de variante de JSGME (Generic Mod Enabler), dont beaucoup d'auteurs suivent encore la convention : nom et contenu standardisés — première ligne le nom de l'option, le reste son explication — et **jamais lu par AC**. C'est un document, au même titre qu'un readme ; sans lui dans cette liste, il partait en ajout au jeu et la fiche annonçait un fichier posé dans `MODS/`. Il est aussi prévisualisable comme du texte (§4.5.2, prévisualisation) : c'est souvent la seule chose qui dise à quoi sert un dossier optionnel.
- **Tout** — ajoute les fichiers lourds : templates d'édition (`.psd`, `.xcf`, `.ai`), archives jointes (`.zip`/`.7z`/`.rar`), sources 3D (`.fbx`, `.blend`, `.3dsmax`), vidéos de présentation.

**Les images ne sont jamais des annexes**, à aucune profondeur et même à côté du mod : rien ne distingue une capture de présentation d'un asset AC (`logo.png`, `body_shadow.png`, `map.png`, aperçu de skin) — donc on ne tranche pas, on laisse. Une capture de présentation qui reste dans le mod ne coûte rien ; un `body_shadow.png` retiré casse le rendu.

**Une annexe restée dans le mod est listée, jamais déplacée.** La règle d'or (§4.5.1) interdit de sortir quoi que ce soit du dossier du mod : le `..._readme.txt` que l'auteur a posé à la racine de son circuit y reste. Mais l'onglet Ressources le **liste** quand même, marqué « dans le mod » et résolu contre le dossier du mod au lieu du dossier ressources. Mêmes extensions que le classement à l'import (documents d'information), **racine du dossier du mod seulement** — plus profond, un `.txt` fait presque toujours partie du contenu — et `GUIDs.txt` exclu comme partout ailleurs. Sans ça, deux mods identiques donnaient deux comportements selon que l'auteur avait livré sa notice à côté du dossier ou dedans.

**Les images passent en galerie.** Un dossier de fonds d'écran livré par l'auteur (`Wallpapers/`, quatorze `01.jpg`…) ne se consulte pas en liste : les noms ne disent rien. Les ressources dont le format est une image sortent donc de la liste et s'affichent en **grille de vignettes**, la **même visionneuse plein écran** qu'ailleurs s'ouvrant au clic (`Lightbox`, §6.1) — navigation, diaporama, clavier et manette compris. Rien de neuf n'a été écrit pour ça : seule la source des images change, la visionneuse et le générateur de miniatures servaient déjà les captures et les backgrounds. Le reste des ressources garde sa liste, sous la grille.

**Prévisualisation dans l'onglet Ressources.** Un clic sur une annexe d'un format lisible l'ouvre **sous la liste**, dans la fiche : texte brut (`.txt`, `.nfo`, `.log`, `.ini`, `.cfg`, `.csv`, `.json`, `.yml`, `.lua`), markdown (`.md`) et **PDF** — les images, elles, passent par la galerie ci-dessus plutôt que par l'aperçu en ligne. Tout autre format garde le comportement d'origine — ouverture par l'application par défaut de Windows, également accessible d'un bouton dédié (`↗`) sur les formats prévisualisables. Au-delà de 32 Mio, la prévisualisation refuse et renvoie sur cette ouverture externe plutôt que de faire transiter le fichier par l'IPC.

Le document n'a **ni hauteur imposée ni défilement propre** — et son bandeau (nom du fichier, fermeture) n'est pas épinglé non plus : il s'étend dans le flux et c'est la page de la fiche qui défile. C'est ce qui écarte l'`<iframe>` pour le PDF — la WebView y répondrait par la visionneuse d'Edge, application autonome avec sa barre d'outils et son défilement interne dans une boîte à hauteur fixe. Le PDF est donc rendu par **pdf.js**, page par page en `<canvas>` empilés dans le flux.

**Le PDF a son niveau de zoom.** Une largeur de colonne convient à une notice, pas à un schéma de faisceau ni à un tableau de réglages, et elle gâche une affiche d'une page. La barre du lecteur offre donc deux ajustements — **largeur de page** (le défaut, comportement d'origine) et **page entière**, calculée sur la hauteur de la zone qui défile, une page valant alors un écran — et un **zoom libre** : boutons − / +, pourcentage cliquable qui ramène à la taille réelle, et Ctrl+molette continu. Le pourcentage veut dire ce qu'il veut dire partout ailleurs (100 % = taille réelle), et les deux ajustements se recalculent au redimensionnement de la fenêtre. Le réglage est **durable et global** (`ui_prefs.json`, §6.2) : c'est une façon de lire, pas une propriété d'un mod. Un changement d'échelle, quel qu'il soit, **ne touche pas au défilement** : la barre reste exactement où elle était. Corriger `scrollTop` dans le rapport des deux échelles — pour rester sur la même ligne de la même page, la hauteur du document étant proportionnelle à l'échelle — a été essayé puis retiré : la vue glissait sous le curseur à chaque clic, ce qu'on ne veut précisément pas d'un bouton qu'on s'apprête à presser encore. L'ancrage de défilement du navigateur (`overflow-anchor`) fait la même correction bien intentionnée de son côté ; il est désactivé sur la pile de pages. Au-delà de la largeur de la colonne, le document défile **horizontalement** ; verticalement rien ne change, c'est toujours la fiche qui défile.

Ces deux ajouts imposent le **rendu paresseux**, qui n'est pas un raffinement mais le garde-fou mémoire qui va avec : une page A4 à 400 % est un bitmap de 14 MPx, et la première version redessinait *tout* le document à chaque changement de largeur. Une page n'est donc dessinée que lorsqu'elle approche de la zone visible, et relâchée quand les bitmaps vivants dépassent un budget — le coût suit ce qui est à l'écran, plus la longueur du document. Deux conséquences visibles : la densité de rendu baisse d'elle-même au-delà de ~200 % de zoom (plutôt qu'une page de 228 Mo), et un changement d'échelle **ne blanchit pas** le document — le canvas déjà dessiné est étiré en CSS jusqu'à ce que le rendu net le remplace.

Deux points de sûreté, tous deux côté backend, hérités de l'ouverture externe : le chemin relatif est **résolu et validé** (garde-fou anti-traversée) avant toute lecture, et le contenu remonte par une commande Tauri plutôt que par `asset://` — seules les images, servies dans un `<img>`, passent par le protocole. Le markdown est **échappé avant** production du moindre tag (rendu maison, pas de dépendance de parsing), et les liens d'un readme partent dans le navigateur du système : suivis dans la WebView, ils remplaceraient l'application par la page distante.

#### 4.5.3 Ajouts au jeu → `extras/` en bibliothèque, posés dans AC

Ce qu'AC lit ailleurs que dans `content/<type>/<id>` : configs CSP (`extension/config/cars/rss/<id>/…`), shaders (`system/shaders/…`), textures d'équipe (`content/texture/…`), modèle de pilote (`content/driver/…`).

**Stockés bruts, avec leur chemin relatif à la racine d'AC**, dans `<lib>/extras/<type>/<id>/…` — jamais dans la version, qui est déployée telle quelle dans `content/`. Au **niveau du mod** comme `resources/` : une mise à jour remplace ses propres fichiers, les couches partagent le même arbre.

**Le chemin d'archive n'est pas toujours un chemin de jeu.** Le balayage (§7.3) pose que le chemin d'un reste relatif à la racine de l'archive est son chemin relatif à la racine d'AC. C'est vrai la plupart du temps, et faux de deux façons — `acpath.rs` porte les deux règles :

- **Dossier de jeu livré à nu.** Un dossier `driver/` contenant un `.kn5` (à n'importe quelle profondeur) est le `content/driver/` d'AC : il est préfixé avant tout usage. Cas réel, la Ferrari 599 GTO livre `driver/driver_501.kn5` à côté du dossier de la voiture ; sans le préfixe le pilote atterrit dans `<AC>\driver\`, que le jeu ne lit pas. Une **seule** règle de ce type, volontairement : `weather/` et `sfx/` existent sous `content/` **et** sous `extension/`, on ne peut pas trancher sans regarder le contenu, et deviner mal pose des fichiers au mauvais endroit dans le jeu.
- **Emballage unique à la racine.** Un packageur enveloppe très souvent toute sa livraison dans un dossier à son nom (`NFS_TOURNAMENT_CLASS_A_2026-02-15/content/…`). `modscan` sait descendre cet emballage pour trouver les mods, et le balayage des restes doit s'accorder avec lui sur ce qu'est « la racine de l'archive » : sinon les voitures d'un pack s'installent pendant que ce qui les accompagne reste en bibliothèque, refusé comme chemin hors jeu. Bug réel : trois packs dont les `content/texture` et `content/fonts` n'ont jamais atteint AC alors que leurs voitures roulaient. `acpath::effective_root` traverse, avec **trois garde-fous** — jamais un dossier accompagné d'autres entrées (c'est un choix de l'auteur, `Optional - No ambient sounds/` à côté de son alternative, et en traverser un installerait une variante non choisie) ; jamais un dossier de jeu (`content/` seul *est* la racine, le traverser enverrait le contenu à `<AC>\cars\`) ; jamais un mod reconnu (une archive ne livrant qu'une voiture a elle aussi un dossier unique à sa racine, mais c'en est le contenu — descendre dedans ferait passer ses fichiers pour des restes et l'extraction des annexes le viderait, très exactement la règle d'or n°3).
- **Emballage accompagné : la racine de jeu se déduit, elle ne se devine pas.** Deviner à la forme cesse de marcher dès que l'auteur pose une notice à côté de son emballage — il n'est plus « seul à son niveau ». Or les mods déjà trouvés le disent sans ambiguïté : un mod à `<X>/content/cars/<id>` établit que `<X>` est la racine relative à laquelle AC lit cette livraison. `acpath::game_root` prend donc cette déduction, et ne retombe sur `effective_root` que si elle ne dit rien de sûr — aucun mod ne porte de `content/` au-dessus de lui (une voiture livrée à nu), ou plusieurs en portent et ne s'accordent pas (deux variantes côte à côte : en préférer une installerait un choix que l'auteur n'a pas fait). Bug réel, l'archive VRC Pageau 9T8 : sept entrées à la racine, donc `AC Files/content/fonts` refusé comme non-chemin de jeu et la font du mod jamais posée, alors que sa voiture roulait.

  **Le balayage tient donc deux racines**, et il faut les deux : la racine de *balayage* (ce qu'on parcourt pour ne rien laisser derrière — elle ne bouge pas, sinon le `MANUAL.pdf` posé à côté de l'emballage ne serait jamais ramassé) et la racine de *jeu* (celle à laquelle les chemins sont relatifs pour AC). Un reste hors racine de jeu voit son chemin compté depuis la racine de balayage, ce qui le rend d'office non posable — exactement ce qu'on veut dire de lui. « À la racine », pour le test d'annexe (§4.5.2), vaut pour **les deux** : une notice est une notice qu'elle soit dedans ou dehors.
- **Dossier d'emballage de l'auteur.** `Ferrari F2002 V1.4/`, `Track Installation/`, `Optional - No ambient sounds/` ne sont pas des chemins de jeu, et les poser revient à déverser un dossier d'archive à la racine de l'install. Un reste dont le premier segment n'est pas un dossier lu par AC (`content`, `system`, `extension`, `apps`, `cfg`, `launcher`, `sdk`, `server`, `plugins`) n'est **jamais posé** — ni en ajout au jeu, ni en « autre mod ». `mods/` a figuré dans cette liste et n'aurait jamais dû : c'est le dossier de *stockage* de JSGME, où chaque sous-dossier attend, inerte, que JSGME le recopie dans le jeu — AC n'y regarde jamais. Bug réel, LA Canyons : ses trois `MODS/LA Canyons 1.2 - …/content/…` étaient posés tels quels dans `<AC>\MODS\`, où le patch « Hide Pit Crew » ne fait strictement rien. La liste reste permissive par ailleurs — son travail est d'écarter les dossiers d'emballage, pas d'arbitrer ce qu'un mod a le droit de viser — mais un dossier qu'AC ne lit pas n'y a pas sa place : accepter, là, c'est installer un mod à moitié avec l'apparence du succès. Un fichier isolé à la racine est refusé pour la même raison : AC n'en lit pas, et l'exception qui vient à l'esprit (le `dwrite.dll` d'une install CSP) est précisément ce qu'un gestionnaire de mods ne pose pas tout seul.

Refuser ne jette rien : le fichier reste en bibliothèque — mais **dans les ressources du mod**, avec son chemin d'archive, pas dans « Ajouts au jeu ». Il y a longtemps été listé, marqué « hors chemin de jeu », au motif qu'un fichier absent sans explication serait plus déroutant qu'un fichier listé. C'était vrai tant que rien d'autre ne l'expliquait ; le journal d'import le dit désormais mieux (`pathRefused`, §4.6), et l'onglet répond à « qu'est-ce que ce mod met chez moi ? » (§4.5.5) sans y mêler ce qu'il n'y met pas. Cas réel, LA Canyons : le dossier des livrées CHP **contient** des skins reconnus, donc le balayage y descend au lieu de le ramasser en bloc, et son `description.jsgme` en ressortait seul — annoncé posé dans `MODS/` alors qu'il ne l'était jamais. Le marqueur « hors chemin de jeu » reste affiché pour les entrées importées avant ce correctif.

**Certains chemins de jeu appartiennent à un autre outil.** `extension/config/tracks/loaded/`, `extension/config/cars/loaded/` et les `extension/vao-patches*/` sont la cible de synchronisation du téléchargeur de configs de Content Manager, alimenté par le dépôt `acc-extension-config`. Des archives de circuit y déposent pourtant leur config CSP — et ce n'est **pas la bonne pratique** : `loaded/` est le dernier des trois emplacements que CSP consulte, après `content/tracks/<id>/extension/ext_config.ini` (la place prévue pour un auteur, prioritaire et qui voyage avec le mod) puis `extension/config/tracks/<id>.ini`, et c'est précisément celui que la synchro écrase. L'hypothèse la plus charitable est que l'auteur vise un repli pour les utilisateurs sans mise à jour automatique ; la plus probable est un packaging distrait.

L'app **pose quand même** : arbitrer les choix de l'auteur n'est pas son rôle, et le mécanisme normal (arbitrage par date, §4.5.4) s'applique tel quel. Mais elle le **dit** — un ajout que Content Manager peut remplacer sans prévenir ne doit pas avoir l'air stable (§4.5.5). `acpath.rs` porte la liste des zones concernées.

Deux propriétés en découlent :

- **L'import ne jette rien que l'utilisateur n'ait pas explicitement écarté.** Ce qui n'est pas classé est conservé tel quel, donc l'*interprétation* — où poser, qui arbitre un fichier partagé — reste recalculable depuis la bibliothèque à tout moment. Aucune règle des versions précédentes à mémoriser, aucune archive à conserver : c'est l'**entrée** qui est préservée, pas la décision. C'est ce qui rend un futur changement de règles rattrapable sans rien versionner. La seule exception est un dossier proposé que l'utilisateur a écarté lui-même (§4.6ter) : il n'a plus de décision à recalculer, puisqu'elle est prise. Il est supprimé, et il laisse une ligne au journal d'import (`userDiscarded`) — la règle protégeait la recalculabilité, pas les octets.
- **L'ajout vit et meurt avec son mod.** Posé à l'activation, retiré à la désactivation, supprimé avec lui. Le passage par « autre mod » (§7.3) ne donnait pas ça : les fichiers d'une voiture supprimée restaient dans AC, rattachés à une entrée anonyme que plus rien ne reliait au mod.

**Les archives imbriquées passent avant leurs voisins.** Une archive imbriquée est extraite et reclassée (§7.3), et ce qui en sort entre dans la liste des propriétaires possibles **avant** que les fichiers qui l'entouraient ne soient arbitrés. C'est ce qui permet à une livraison `readme.txt` + `Car.zip` de ranger la notice dans les ressources de la voiture sortie du zip. Corollaire : **rien de reconnu à la racine ne signifie pas rien du tout** — tant qu'une archive imbriquée traîne quelque part dans la source, à n'importe quelle profondeur, on descend dedans avant de conclure. Sans cette descente, la source entière partait en « autre mod » : la voiture n'entrait jamais en bibliothèque et le `.zip` brut se retrouvait lié à la racine du dossier du jeu.

**Rattachement** d'un reste (§7.3), dans cet ordre : le chemin contient l'id d'exactement un mod reconnu de l'archive ; sinon l'archive ne livre qu'un seul mod, et tout ce qui l'entoure lui appartient. **Les apps comptent parmi les propriétaires possibles** au même titre que les voitures et les circuits (`extras::OwnerKind`) : une app a autant qu'une voiture des fichiers livrés à côté de son dossier. Sans elles, l'archive `_RSS_Settings` — une app et son mode d'emploi — n'avait aucun propriétaire à proposer pour le PDF : il devenait un « autre mod » nommé d'après lui, inerte, et dont « ouvrir le dossier » échouait (tout son contenu étant parti en ressources, son dossier de contenu n'existait pas). Les packs de skins et de sons en sont absents à dessein : leur parent est une voiture qui n'est pas forcément dans cette archive, donc rien ne dit qu'un fichier voisin lui appartient. **Sinon, c'est le pack.** Une source qui livre plusieurs mods forme un pack (§4.4), et le pack est le propriétaire de dernier recours de tout ce qui les entoure. Cette règle en remplace une limite qui était écrite comme un compromis — *« un reste que rien ne rattache reste un autre mod »* — et qui était en fait un trou : une voiture livrée avec sa variante CSP est la forme la plus banale qui soit, et **plus rien** n'y était rattaché. Cas réel, l'archive VRC Pageau 9T8 : ses deux notices devenaient des « autres mods » inertes, et son `content/fonts` une entrée anonyme qui aurait survécu à la suppression des deux voitures. Rattacher à *tous* les mods aurait dupliqué des arbres parfois lourds ; rattacher au pack ne duplique rien, et c'est de toute façon plus juste — le manuel d'un pack de vingt voitures n'appartient à aucune d'elles. Un **document isolé** à la racine reste une annexe (§4.5.2) et va dans les ressources du mod, jamais dans AC : sans ce test, un `Read Me.pdf` deviendrait un ajout au jeu posé à la racine d'Assetto Corsa.

**Pose fichier par fichier** (hardlink), jamais par jonction de dossier : plusieurs mods visent les mêmes arbres (`extension/textures/common/rss/…` est livré à l'identique par chaque voiture RSS), et une jonction en donnerait la propriété exclusive au premier arrivé.

**`content/fonts` et `content/driver` ne sont pas un cas particulier** : ce sont des ajouts au jeu comme les autres. Ils ont eu leur propre mécanisme — copie globale dans l'install AC, jamais désactivée, écrasement par défaut en cas de collision — retiré pour trois raisons : il était déjà court-circuité (le balayage des restes, §7.3, les ramassait avant lui) ; il faisait cohabiter deux politiques contradictoires (« jamais désactivé » ici, « vit et meurt avec son mod » là) ; et son écrasement par défaut contredisait la règle d'or n°5. Le checksum anti-triche d'AC porte sur `data.acd` et `surfaces.ini`, pas sur les fonts/drivers.

#### 4.5.4 Poser sans écraser : réclamation, date, sauvegarde

Poser un fichier dans AC pose deux questions que `content/<type>/<id>` ne pose jamais : **plusieurs mods peuvent viser le même chemin**, et **ce chemin peut déjà être occupé** — par du contenu Kunos, par un mod installé hors de l'app, ou par un autre mod de la bibliothèque. Trois règles y répondent, et elles valent pour **les deux** mécanismes de pose : les ajouts au jeu (§4.5.3) et les mods « autres » (§7.3).

**1. Compteur de références.** Chaque mod *réclame* les chemins d'AC dont il a besoin (`extra_links`). Un fichier n'est retiré d'AC que lorsque plus aucun mod ne le réclame. Désactiver une voiture RSS n'emporte pas `extension/textures/common/rss/…` dont onze autres dépendent, et il n'y a plus de course à la propriété : le premier arrivé ne gagne rien.

**2. Arbitrage par date.** *(Sauf autorisation explicite — voir en fin de règle.)* L'exemplaire à la **date de modification la plus récente** gagne, un mod plus récent corrigeant en général des bugs de celui d'avant. La date traverse la chaîne intacte : 7-Zip restitue celle stockée dans l'archive, `std::fs::copy` la conserve sous Windows, un hardlink partage l'entrée MFT. À égalité (archives repackées par un tiers, qui perdent les dates), c'est le **dernier mod installé**. L'arbitrage se rejoue dans les deux sens : quand le fournisseur s'en va, le fichier repasse à l'exemplaire du meilleur réclamant restant. **Un exemplaire plus ancien, ou de même date, ne déloge jamais ce qui tourne déjà** — sans cette comparaison, le dernier mod installé écraserait une font déjà mise à jour par un autre outil.

**L'arbitrage par date protège les poses automatiques, pas les décisions.** Il n'a aucune autorité contre un « ajouter au jeu » que l'utilisateur vient de donner sur un dossier proposé (§4.6ter), devant l'avertissement qui lui disait combien de fichiers du jeu de base seraient remplacés. Les chemins ainsi autorisés sont mémorisés (`forced_extras`) et la comparaison de dates est levée pour eux — la **sauvegarde de l'original reste obligatoire**, c'est elle qui rend l'opération sûre, pas la date. Table séparée d'`extra_links`, effacée et réécrite à chaque déploiement : l'autorisation, elle, survit à une désactivation suivie d'une réactivation, sinon la question se reposerait à chaque fois. Elle ne disparaît qu'avec le mod. Bug réel, le patch « Hide Pit Crew » de LA Canyons : ses `pitcrew.kn5` datent de 2020, ceux de l'install Kunos portent la date du téléchargement Steam — donc plus récents. Seul le fichier neuf du patch arrivait ; les deux modèles qu'il devait remplacer restaient marqués « en attente » sur la fiche, alors que l'utilisateur venait de répondre oui.

**« En attente » n'est plus une impasse.** Un ajout au jeu qu'un fichier étranger occupe (`held_by_foreign_file`, §4.5.5) porte sur sa ligne un bouton **« Poser quand même »** (`force_mod_extra` → `extras::force_one`) : c'est la même autorisation explicite que celle donnée à l'import, prise cette fois devant le fichier concerné. Signalé par un utilisateur, et c'était le vrai défaut : l'app disait « en attente » sans dire quoi faire, et il n'y avait *rien* à faire — dire à quelqu'un qu'il est bloqué sans lui donner de sortie est pire que de se taire. Le geste reste réversible par construction : ce qui occupait le chemin est sauvegardé avant d'être remplacé et revient quand plus aucun mod ne le réclame (§4.5.4).

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

#### 4.6ter Dossiers proposés par l'auteur

**Le fourre-tout du milieu.** Le balayage (§7.3) sait classer deux choses : un chemin de jeu (ajout au jeu, §4.5.3) et un document isolé (annexe, §4.5.2). Entre les deux, ce qui restait était arbitré par le *chemin*, alors que ce qui s'y trouve n'est presque jamais un chemin :

| Ce que l'auteur livre | Ce que c'est |
| --- | --- |
| `2K Skins/`, `No Dust Skins/` (Ferrari F2002) | des livrées de meilleure qualité qui **recouvrent** celles de la voiture |
| `Optional Textures/` (VRC Pageau 9T8) | deux `.dds` à poser dans le dossier de la voiture |
| `MODS/<variante>/` (LA Canyons) | la convention JSGME, un sous-dossier par option — dont un patch qui masque le personnel des stands **sur toutes les pistes** |
| `Wallpapers/`, `Templates/`, `CM Previews Template/` | de la matière qui n'a rien à faire dans le jeu |

Aucune règle ne les sépare depuis le disque, et c'est le fond du problème : l'information est dans la notice, en prose, ou dans l'intention de l'utilisateur. Le §4.6 tranche ce cas depuis toujours — **information de préférence → demander**.

**Le déclencheur est structurel** : un *dossier* resté après le balayage dont le chemin ne mène nulle part dans le jeu. Un dossier qui mène dans le jeu reste un ajout, décidé sans rien demander ; un fichier isolé reste une annexe ou un ajout. Rien de nouveau ne s'ajoute au chemin nominal.

**Un cas de plus, et il est ailleurs** : un pack de skins dont la cible est inconnue **et** dont les livrées recouvrent celles d'un contenu de la même source n'est pas un pack, c'est une variante — il n'est donc pas consommé comme skin (`pending::offered_liveries_target`). Le test est volontairement étroit : sans ce recoupement de noms, un pack pour une voiture pas encore installée reste un pack, dort en bibliothèque, et `repair_projections` le branchera quand la voiture arrivera. Sans cette règle, les `2K Skins/` de la F2002 s'importaient comme skins d'une voiture nommée « 2K Skins », qui n'existe pas : jamais projetés, jamais utilisables.

**Rangé, jamais posé, jamais perdu.** Le dossier attend dans `<lib>/pending/<id>/`, sa ligne (`pending_folders`) survit à un redémarrage, et rien de lui n'entre dans le jeu. La question est posée **en fin de lot**, dans le rapport d'import, jamais pendant — un lot de cinquante mods ne s'interrompt pas (§4.6). Ne rien décider reste une réponse valable : ce qui attend est toujours là au prochain import.

**Cinq formes reconnues**, qui ne décident de rien : elles choisissent ce qu'on montre et ce qu'on pré-remplit.

- **variante JSGME** — un `description.jsgme` est présent. Convention vieille de vingt ans : première ligne le nom, le reste l'explication. C'est le seul cas où l'auteur fournit un libellé exploitable tel quel, et il est affiché tel quel.
- **contenu de jeu** — un enfant direct est un dossier lu par AC. Enfants directs seulement : plus profond, on retrouverait le `content/` d'un dossier de mod.
- **livrées de remplacement** — voir ci-dessus.
- **documents** — tous les fichiers sont d'un format qu'AC ne lit jamais. **Les images n'en font pas partie** : rien ne distingue une capture de présentation d'un asset AC (§4.5.2), donc un dossier de fonds d'écran retombe en *indéterminé*, ce qui est honnête — c'est l'utilisateur qui sait.
- **indéterminé** — aucune des quatre.

**Cinq sorts**, dont ceux qui ont un sens pour le dossier en question. Aucun n'est définitif — c'est ce que leurs libellés doivent dire, et ce que « Installer » ne disait pas :

| Sort | Ce qu'il fait | Comment on revient dessus |
| --- | --- | --- |
| **Ajouter au jeu** | c'est la réponse de l'utilisateur qui autorise à chercher la racine de jeu **à l'intérieur** du dossier. Sans elle, l'app n'a que le chemin d'archive et doit refuser (§4.5.3). Rangé en ajouts au jeu du propriétaire — sauf ce qui ne mène nulle part dans le jeu (le `description.jsgme` qui accompagne la variante sans en faire partie), qui va en ressources : dire « ajoute au jeu » n'a jamais voulu dire « pose tout ». | retiré quand le mod est désactivé, supprimé avec lui ; les fichiers du jeu remplacés sont restaurés (§4.5.4) |
| **Ajouter au dossier du mod** | composé par-dessus la version du mod (§4.4), qui n'est jamais touchée. La seule réponse non destructive à « copiez ces fichiers dans le dossier de la voiture ». **Proposé uniquement pour une variante de livrées** — voir ci-dessous. | se désactive ou se retire depuis « Couches & extensions » sur la fiche |
| **Garder dans l'onglet Ressources** | rangé sous son propre nom dans les ressources du propriétaire, consultable depuis sa fiche. Rien n'entre dans le jeu. Le libellé nomme l'endroit où le dossier réapparaîtra — « garder sans installer » disait ce que l'action ne fait pas, pas où elle range. | rien à défaire |
| **Garder à part** | entrée « autre mod » indépendante. Proposé seulement quand rien ne rattache le dossier à un mod. | s'active et se désactive depuis « Autres mods » |
| **Ne pas importer** | supprimé, et journalisé (`userDiscarded`). Le seul endroit de l'app où de la matière importée disparaît. | l'archive source, si elle est conservée (§11) |

**« Ajouter au dossier du mod » n'est proposé que quand on sait *où* les fichiers vont dedans.** On ne le sait que pour une variante de livrées, où la structure `skins/<nom>` et le recoupement des noms le prouvent. Partout ailleurs — deux `.dds` à nu, un dossier quelconque — la destination n'est écrite nulle part ailleurs que dans la notice de l'auteur : vérifié sur les `Optional Textures` de VRC, rien dans l'arborescence ne la donne. Proposer de composer à l'aveugle, c'est offrir un bouton qui a une chance sur trois de poser les fichiers au mauvais endroit, en silence. La notice, elle, est affichée au moment du choix : l'utilisateur peut le faire lui-même, en connaissance de cause. Un dossier appartenant à un pack ne le reçoit jamais non plus — une couche s'applique à un mod, pas à un pack.

**« Ajouter au jeu » et « Ajouter au dossier du mod » s'excluent** par ailleurs, et c'est l'arbre du dossier qui tranche, pas un goût. S'il porte un **arbre de jeu** (`content/`, `extension/`…), ces chemins sont relatifs à la racine d'AC : les composer dans un mod donnerait `content/tracks/<id>/content/objects3D/`, que rien ne lit. S'il n'en porte pas (deux `.dds`, un dossier `skins/`), ce sont des fichiers destinés au dossier du mod, et « ajouter au jeu » les refuserait un par un. Offrir les deux partout revenait à faire choisir entre une bonne réponse et une qui ne pouvait pas marcher. *Limite connue* : un arbre de jeu qui vise le dossier du mod lui-même (`content/cars/<id>/…` livré dans un dossier optionnel) reçoit « Ajouter au jeu » alors que la couche serait plus juste ; aucun cas réel rencontré à ce jour.

**Aucune proposition quand le dossier remplace des fichiers du jeu de base.** C'est le §4.6bis appliqué à la lettre : là, aucun des deux défauts n'est sûr — installer change le jeu pour **toutes** les sessions, ne pas installer peut priver d'un correctif que l'auteur juge nécessaire. Pré-cocher donnerait l'illusion que l'app sait. Les réponses sont alors à égalité, et c'est le décompte des fichiers remplacés qui parle.

**Aucune couleur sur les réponses**, pour la même raison. Ce sont quatre réponses à une question, pas une bonne et trois mauvaises : « Ne pas importer » est souvent la plus sensée devant une option qui écraserait du contenu de base. Le seul repère est la mention « proposé », et elle disparaît dès que l'app n'a pas d'avis. L'avertissement, lui, porte sur le **fait** (jaune, « remplace N fichiers du jeu de base — s'applique à toutes les sessions »), jamais sur un bouton.

**Une carte répondue reste à sa place, à sa taille exacte.** Elle quittait la liste au clic — le tour suivant était rechargé depuis la base — et tout ce qui la suivait remontait sous le pointeur : sur une série de dix dossiers, on répondait à une question qui venait de se déplacer. Rien ne se rétracte donc, pas même en une ligne de résumé : même texte, mêmes réponses, mêmes pixels, seul le contraste tombe et la réponse donnée s'allume en vert pendant que les autres s'éteignent. La question tranchée reste lisible, ce qui est la seule façon de vérifier qu'on a répondu ce qu'on croit. **Et rien ne défile** : amener l'écran sur la question suivante a été essayé puis retiré — la liste ne bougeant plus, une vue qui se déplace d'elle-même juste après un clic se lit comme une conséquence de ce clic, et rend la désorientation que la carte immobile venait d'enlever. La modale ne se ferme plus d'elle-même à la dernière réponse : son bouton dit « Terminé » au lieu de « Plus tard ».

**Ce qui est montré**, parce que c'est ce sur quoi la décision se prend : le titre écrit par l'auteur (qui passe devant le chemin d'archive — c'est la seule ligne écrite pour être lue), le chemin, la forme reconnue, le poids, le mod de rattachement, le décompte des fichiers du jeu remplacés, et la **notice avec son nom de fichier**, rendue sur place pour les formats texte. §4.6bis posait déjà que pour les dossiers qu'on ne sait pas interpréter, la réponse honnête est de rendre la notice lisible ; ici elle se lit sans quitter l'écran où la décision se prend, le va-et-vient vers l'explorateur étant précisément ce qui fait cliquer au hasard. Chaque réponse porte son explication en toutes lettres sous son libellé, et non en infobulle : une infobulle est invisible à la manette.

**Une modale, centrée, ouverte automatiquement en fin de lot** (`PendingDialog`) — et **non bloquante**, contrairement aux arbitrages de `ImportOverlay` : la fermer ne décide rien, les dossiers restent en attente, et le rapport d'import garde un bandeau pour y revenir. Le rapport lui-même n'en garde qu'une ligne : la pile de notifications fait 380 px de large, et une question illisible se répond au hasard.

**Ce qui reste automatique.** Un dossier de jeu livré à nu (`driver/` avec un `.kn5`) est toujours corrigé sans rien demander : c'est déterminable, et le §4.6 dit qu'une question à laquelle l'utilisateur ne peut pas mieux répondre que l'app est pire qu'un défaut. La décision est journalisée (`pathNormalized`) et relisible sur la fiche.

---

## 5. Tags et harmonisation

**Vocabulaire fermé (liste blanche)** : l'univers fini des tags autorisés (catégories `#gt3`/`#gte`/`#lmp1`, familles `prototype`/`endurance`/`vintage`, styles `#drift`/`#rally`/`#jdm`, propriétés CSP `rainfx`…). Tout tag entrant est mappé vers ce vocabulaire ou rejeté. C'est le vocabulaire borné qui harmonise, pas une meilleure détection.

**Ce vocabulaire, c'est exactement ce que les règles savent produire** : les sorties des fusions et des déductions, plus la liste blanche des catégories de circuit. Rien à tenir à jour en double — écrire une règle, c'est déclarer son vocabulaire. Un tag entrant qui n'y figure pas **n'est pas promu** : il reste le tag brut du mod, affiché comme tel et masqué avec les autres (voir plus bas). Il n'est donc jamais perdu, seulement privé d'un badge de règle qu'aucune règle ne lui a donné — et, s'il commence par `#`, privé de devenir une catégorie que personne n'a déclarée. C'est ce qui garde le filtre « catégorie » de la bibliothèque propre : sans ça, un `#` inventé par un seul auteur de mod y créait sa propre entrée.

**Trois origines de tags, tracées séparément** (permis par l'overlay non destructif) :
1. **Tags bruts du mod** — lus dans `ui_*.json`, lecture seule, masquables. S'y ajoute tout ce qu'aucune règle n'a reconnu : c'est la même chose du point de vue de l'utilisateur (« le fichier dit ça, Pit Box n'en fait rien »), donc c'est la même couleur et la même bascule.
2. **Tags déduits par règle** — calculés par l'ontologie, dans l'overlay.
3. **Tags manuels** — saisis, dans l'overlay, seuls directement supprimables.

Distinction par **code couleur** (rouge = catégorie, vert = règle, gris = manuel, bleu = brut du mod), légende discrète unique. Ordre : catégorie `#` → règle → manuel → brut du mod (en dernier, masquable). La bascule porte un **libellé stable** (« Tags bruts du mod ») avec une case à cocher, pas un verbe qui s'inverse à chaque clic : un état se lit plus vite qu'une action. Elle existe **dans les deux présentations de la fiche** (panneau latéral et page pleine) et partage la même préférence — c'est un seul réglage, pas deux.

**Ontologie de règles** (données, pas code — fichier `default-tag-rules-enriched.json`, éditable, versionnable). Familles :
- **Fusion** (synonymes → tag canonique), **Déduction** (tags implicites : `lmp1` → `#lmp1` + `prototype` + `endurance`), **Extraction** (tag → champ technique structuré), **brand_fix** (correction de marque depuis le nom).
- Il n'y a **pas de famille « suppression »**. Elle a existé — 314 tags de bruit listés à la main — parce que le moteur laissait passer les tags inconnus et qu'il fallait les rattraper un par un ; c'était une liste noire compensant l'absence de liste blanche. La reconnaissance étant devenue la règle, elle ne changeait plus rien et a été retirée. Un tag de bruit est simplement un tag qu'aucune règle ne produit.
- **Compteur d'effet** : chaque règle affiche le nombre de mods sur lesquels elle agit réellement (REGLES§8.3) — l'ancien « aperçu d'impact » global a disparu avec la barre d'enregistrement.
- Écran graphique de gestion des règles (`Atelier › Règles`, décrit plus bas avec le catalogue).

**Le résultat de l'harmonisation est stocké**, pas recalculé à l'affichage : faire évoluer le moteur ne change donc rien à ce qui est déjà en base. Un numéro de version (`rules::ENGINE_VERSION`) est incrémenté à chaque évolution qui produirait un résultat différent à règles égales, et une passe au démarrage recalcule toute la bibliothèque **une fois par version** — sans ça, il faudrait savoir qu'on doit rouvrir l'écran Règles et réenregistrer pour voir le changement. Le marqueur (table `meta` de l'overlay) n'est posé qu'en cas de succès, et rien n'est tenté bibliothèque inaccessible : un disque externe non monté ferait sinon passer un balayage à vide pour un rattrapage fait.

**Comportement** : harmonisation automatique à l'import, édition manuelle à tout moment, détection CSP (lecture des `ext_config.ini` → `rainfx`, `grassfx`, `weatherfx`, `lightingfx`, `has-skins`), recherche/filtres par tag/marque/type/catégorie/année/auteur.

**Catégorie = tag `#` déclaré par les règles** (convention CM) : le tag préfixé `#` identifie la catégorie de la voiture — à condition qu'une règle sache le produire. Sinon la voiture n'a pas de catégorie, et le `#` en question reste visible parmi ses tags bruts ; l'ajouter au vocabulaire (écran Règles) est le geste qui le promeut. Sert à la **composition de plateau** (§7), combiné à la fenêtre d'années.

### 5bis.3 Nom et description repris à la main

**Le nom et la description d'un mod s'éditent** depuis la fiche pleine page : un crayon à droite du nom, un autre dans l'en-tête de la carte « Description ». Même mécanisme que les tags manuels, et pour la même raison — les `ui_*.json` d'un mod sont en lecture seule (règle d'or n°1), donc la saisie va dans l'overlay et **survit à une mise à jour du mod**. Sans ça, corriger le nom d'un mod mal nommé serait un travail à refaire à chaque version publiée par l'auteur.

Deux colonnes SQLite distinctes des champs dérivés du fichier (`display_name_user`, `description_user`) : les champs dérivés, eux, sont réécrits à chaque réimport et à chaque réindex — une saisie qui y vivrait disparaîtrait à la première mise à jour. Le **nom effectif** est arbitré en SQL (`COALESCE(display_name_user, display_name)`), donc tout ce qui affiche un mod en profite d'un coup : liste, tri, fiche, sélecteur de session, adversaires, export. La description, elle, n'est pas en base — elle se relit dans le fichier à chaque affichage — donc son arbitrage se fait dans `library.rs`. Le nom du fichier reste exposé à part (`display_name_file`) : il faut bien montrer à quoi on reviendrait.

**La carte « Description » s'affiche même vide** : un mod qui n'en a aucune est justement celui à qui on veut en écrire une. Vider le champ vaut « reviens au fichier du mod » — le geste naturel pour annuler, et il évite de distinguer « vide » de « pas de surcharge », qui n'ont aucune différence utile.

**Où le dire à l'utilisateur** : la phrase (« Enregistré dans Pit Box, pas dans les fichiers du mod : conservé si le mod est mis à jour ») s'affiche **sous le champ, pendant la saisie**, pas en infobulle sur le crayon — une infobulle disparaît à l'instant précis où l'information devient utile, c'est-à-dire quand on tape. Le crayon garde une infobulle courte (« Renommer »), qui répond à une autre question : à quoi sert ce bouton.

**Une réindexation ne détruit jamais ces saisies** (SESSION§3.1). L'indexation du contenu de base repartait d'un `DELETE` global de ses lignes, donc chaque réindex effaçait tout l'overlay posé dessus — nom repris à la main, description, **tags manuels et favoris compris**. Bug réel signalé : « Mugello » renommé « Autodromo del Mugello » redevenait « Mugello ». Désormais seules les **versions synthétiques** repartent (elles se refabriquent avec un nouvel UUID et s'accumuleraient sinon) ; les lignes `mods` survivent, et ce qui a disparu de `content/` est retiré par comparaison avec ce qui a été trouvé sur disque — ce que le `DELETE` global faisait gratuitement et qu'il fallait donc remplacer explicitement. Une case à cocher **décochée par défaut** rétablit l'effacement complet, et elle seule fait apparaître une confirmation qui énumère ce qui va disparaître : la case se coche d'un clic, ce qu'elle efface ne se récupère pas.

**Nom d'un circuit multi-layouts** : un circuit n'a pas de nom à lui, chaque layout porte le sien. On prenait celui du **premier layout trouvé** — « Highlands Drift » pour un circuit qui s'appelle « Highlands », et le nom changeait selon l'ordre alphabétique des dossiers. Le nom est désormais la **racine commune** des noms de layouts, découpée sur des **mots entiers** : une comparaison caractère par caractère rendrait « Monza 19 » pour « Monza 1966 »/« Monza 1971 », qui n'est le nom de rien. La ponctuation de liaison laissée en bout est retirée (« Spa - GP »/« Spa - National » → « Spa »), la casse est ignorée pour comparer mais celle du premier layout est conservée à l'affichage, et sans aucun mot commun on retombe sur le comportement d'avant plutôt que sur un nom vide. Appliqué à l'import **et** au réindex ; l'utilisateur reste libre de renommer par-dessus.

---

## 6. Fiche technique et champs dédiés

Les caractéristiques mécaniques ne sont **pas** des tags (un tag filtre/groupe, il ne décrit pas une fiche technique).

**Champs lus directement de `ui_car.json`** : `specs` (objet structuré : bhp, torque, weight, topspeed, acceleration, pwratio — pas de parsing), courbes `powerCurve`/`torqueCurve` (présentes ~100 %, à tracer), `description` (à la demande), `country`, `author`, `version`.

**Le bandeau de specs en surimpression de l'aperçu a disparu** : puissance, couple, poids et vitesse max se lisaient à la fois sur la photo et dans la fiche technique juste à côté. Une donnée affichée deux fois au même endroit n'est pas une redondance utile, c'est une hésitation sur qui la porte — c'est la fiche.

**Pas de ligne « URL d'origine » sur la fiche.** Le champ existe en base (`source_url`, §4.4) et attend l'extension de navigateur qui le remplira ; en attendant, la ligne n'affichait « inconnue » sur toutes les fiches de tout le monde. Une rubrique permanente qui ne peut rien dire n'est pas une promesse, c'est du bruit : elle réapparaîtra avec ce qui la remplit.

**Une seule fiche technique, un seul composant** (`components/detail/TechSheet.svelte`), rendu par le panneau latéral **et** par la page pleine. Les deux la construisaient chacune de leur côté, et elles avaient divergé : le panneau montrait toute la fiche native (puissance, couple, poids, vitesse max, 0-100, rapport poids/puissance, autonomie, pays) plus les cinq champs harmonisés, la page en montrait six et laissait de côté tout ce que le moteur dit de lui-même — même titre, même écran, moitié moins de contenu (signalé par l'utilisateur). Le composant rend les cellules ; **le cadre reste à l'appelant** (le panneau dessine le sien, la page a sa carte), et le nombre de colonnes se déduit de la largeur reçue (`auto-fit`) plutôt que d'être dicté par l'un ou l'autre — deux colonnes dans le panneau, trois sur la page, sans que ni l'un ni l'autre ait à le dire. Une ligne vide est omise : une fiche de tirets ne dit rien de plus que son absence. Les champs déduits par les règles (§5) gardent leur couleur verte et leur infobulle « déduit par règle ».

**L'odomètre est une ligne de la fiche, pas une carte** : la distance parcourue se lit là où on la cherche, au milieu des autres chiffres de la voiture, et la carte « Distance » de la page a disparu avec elle. C'est la seule ligne toujours présente — un odomètre vide est lui-même une réponse, et il donne alors le marqueur « jamais essayée » plutôt qu'un tiret. **Les circuits suivent la même règle** depuis qu'ils ont, eux aussi, une carte de données (§6.3bis) : l'exception qu'ils formaient n'était pas un choix, seulement l'absence d'un endroit où mettre le chiffre. La règle vit dans `odometer.ts`, partagée par les deux fiches — c'est la seule ligne dont l'absence de valeur est une valeur, et deux copies de cette subtilité auraient divergé.

**Badge de marque** : `content/cars/<voiture>/ui/badge.png` (présent quasi partout, mod comme Kunos). Affiché sur fiches et vignettes. Source locale, pas de dépendance externe. Fallback (monogramme/générique) pour les rares voitures sans badge. **Pas d'icône d'auteur** (elle vient d'un pack externe communautaire, pas des fichiers du mod) : afficher le nom en texte.

**Champ `year` — résolution à trois niveaux** : (1) lire `year` de `ui_car.json` s'il est présent ; (2) sinon, pour le contenu de base, chercher dans la table statique `kunos_content_dates.json` ; (3) sinon « — ». L'app ne dépend pas de la base en ligne d'AcTools/CM.

**Champs structurés complémentaires** (overlay), remplis par la famille de règles Extraction, uniquement pour ce que `specs` ne couvre pas : `drivetrain` (RWD/FWD/AWD), `engine_pos` (FRONT/MID/REAR), `aspiration` (NA/TURBO/SUPERCHARGED), `engine_config` (V6/V8/…/ELECTRIC), `gearbox` (MANUAL/SEQUENTIAL/…). Une fois extraits, ces tags techniques sont retirés du vocabulaire. Même principe pour le **pays** (tag pays → champ `country` si vide, puis retrait du tag).

**Les alias de pays sont une seconde table, et ne se confondent pas avec la première.** L'extraction **devine** un pays absent à partir d'un tag et ne joue que si le champ natif est vide ; les alias **normalisent** un pays déjà déclaré (« ce mod dit `U.S.A.`, c'est-à-dire United States ») et jouent toujours. Les fusionner laisserait un tag réécrire ce que l'auteur a pris la peine de déclarer. **Les deux s'éditent dans l'onglet Pays de l'Atelier**, pas dans l'écran Règles (TAXO§8, REGLES§11 : une table de correspondance exacte n'est pas une heuristique). Chaque pays déplié y montre ses orthographes (alias) et les tags qui le désignent (extraction). Ils vivent **hors des deux familles** : un circuit déclare un pays comme une voiture, et l'écrit tout aussi librement.

**Après les alias, deux fusions qui ne sont pas des décisions** (TAXO§7.1, moteur v5) : une valeur égale à un nom de la table du jeu une fois la casse, les espaces et les accents repliés prend l'orthographe du jeu (`JAPAN`, `japán` → `Japan`), et un code ISO valide prend le nom du pays (`JP`, `JPN` → `Japan`). Rien d'autre : rapprocher `Swizerland` de `Switzerland` est une **proposition** de l'onglet Pays, jamais une décision de l'harmonisation. La table du jeu est chargée une fois au démarrage (`nationalities::set_known`) ; absente, seuls les alias jouent. Mesuré sur 579 `ui_*.json` : aucune variante de casse ni aucun code ISO dans le corpus — la règle est là pour les bibliothèques qui en ont.

**La normalisation a lieu à un seul endroit** (`harmonize::store`), et c'est ce qui la rend fiable : les deux chemins s'y rejoignent, le champ natif et le tag extrait. Normaliser plus haut, à la lecture du `ui_json`, aurait laissé passer le second — une voiture taguée `usa` se serait rangée à part d'une voiture déclarant `U.S.A.`, pour le même pays. Le `ui_*.json` du mod n'est jamais touché (règle d'or n°1) : c'est l'overlay qui porte la valeur normalisée.

**Le jeu d'alias par défaut est mesuré, pas deviné.** Relevé sur les 575 `ui_*.json` d'une bibliothèque réelle : 17 orthographes de pays, dont cinq introuvables telles quelles dans la table du jeu, et à elles seules 82 des 395 mods qui en portent un — `U.S.A.`, `USA` et `United States of America` pour les États-Unis, `Great Britain` pour le Royaume-Uni, plus une valeur cassée. **Les nations britanniques n'y sont pas** : AC donne son propre drapeau à l'Écosse, à l'Angleterre, au pays de Galles et à l'Irlande du Nord, et les replier sur le Royaume-Uni remplacerait un drapeau juste par un autre drapeau juste.

**Une valeur cassée se lit, elle ne se répare pas.** Le `ui_track.json` de `le_lancone` porte `"country": "France\", \"Corsica"` — son auteur a voulu écrire deux entrées et en a produit une seule, guillemets compris. Le nom retenu est ce qui précède le premier guillemet, qu'un nom de pays ne contient jamais. **Jamais à la virgule**, qui paraîtrait faire la même chose : la table du jeu en contient (`Tanzania, {United Republic of}`), et les couper les rendrait introuvables.

**Un circuit déclare son pays comme une voiture, et le garde comme elle.** Ce n'était pas le cas : la réharmonisation relisait le pays natif par le lecteur de `ui_car.json` **quel que soit le type**. Un circuit n'en a pas, son pays revenait donc vide — et comme rien n'extrait de pays d'un tag de circuit, la valeur posée à l'import était **effacée**, sans un mot, dès qu'on touchait aux règles. Le défaut était dans la duplication : deux lecteurs, dont un seul branchait sur le type. Il n'y en a plus qu'un, qui rend le pays natif en même temps que l'harmonisation — ils viennent du même fichier.

L'arrivée des alias fait passer le moteur en **version 3**, et cette correction en **version 4** : une bibliothèque indexée avant eux se réharmonise **toute seule au démarrage suivant**, sans rien demander — c'est le même rattrapage que la fermeture du vocabulaire en version 2, et c'est lui qui rend leur pays aux circuits qui l'avaient perdu.

**Favori** : état personnel (cœur), ni tag ni caractéristique.

**Onglets de premier niveau** en haut de la fiche (`DetailPage.svelte`, la
page pleine — désormais la seule fiche : le panneau latéral compact qui la
doublait dans la liste a été retiré) : **Fiche · Screenshots · Replays · Resources ·
Ajouts au jeu · Backgrounds** (ce dernier affiché seulement pour un circuit).
Ressources et Ajouts au jeu vivent dans leur propre onglet plutôt que dans la
colonne de la fiche (§4.5.5). Les actions secondaires de l'en-tête (Activer/Désactiver, Exporter,
Réinstaller, Supprimer, Aperçu 3D, Ouvrir le dossier) sont regroupées dans un
menu **⋮** — seuls le cœur favori et le badge « Contenu de base » restent
visibles en permanence, hors du menu.

**Un seul curseur pour toute l'app** (`src/lib/components/ui/Slider.svelte`) : réglages de session (dégâts, carburant, usure des pneus, heure), volume et fondu de Musique, cinq réglages de cadrage de l'aperçu 3D. Il y en avait quatre, tous faits main — les réglages de session dessinaient une poignée carrée rouge sur une piste de 3 px, Musique et Aperçu laissaient la poignée ronde du navigateur avec `accent-color`. Même contrôle, deux apparences. Le remplissage de la piste se calcule **dans** le composant, à partir des bornes : chaque appelant le recopiait à la main (`fuel_rate / 2` pour une échelle 0-200), donc une borne qui bouge laissait un remplissage faux. Le comportement manette « entrer dans le champ » (§7.4bis) vient avec, sans rien à déclarer : il porte sur le type `range` lui-même.

**Un seul composant d'onglets pour toute l'app** (`src/lib/components/ui/Tabs.svelte`) : fiche détail, Réglages, Apps et Règles de tags. Ils avaient chacun leur `.tabs` local — trois tailles de police, trois façons de marquer l'onglet actif, trois fonds. Le CSS Svelte étant scopé par composant, chaque copie dérivait de son côté sans que personne ne le voie : le mécanisme même qui a produit 53 signatures visuelles pour 68 libellés (§chantier libellés). Une variante `flush` (bande pleine largeur sur fond de carte) pour la fiche, qui occupe tout le cadre ; partout ailleurs la bande est transparente et porte elle-même son écart au contenu — une valeur de plus qui divergeait d'un écran à l'autre. Le composant s'inscrit tout seul auprès de `screenActions` (§7.4bis), ce qui rend tout écran à onglets parcourable à la manette sans une ligne de code de sa part — **sauf** une bande imbriquée dans un onglet (les sous-onglets du bloc textuel, §6.3bis), qui s'en retire : le registre est une pile dont la dernière inscription gagne, et sans ce retrait « onglet suivant » ferait défiler Description/Notes en rendant les onglets de la fiche injoignables.

**Chiffre entre parenthèses** sur Screenshots/Replays/Resources/Ajouts au jeu/Backgrounds
(ex. « Replays (3) ») dès qu'il est connu, pour savoir s'il y a quelque chose
avant de cliquer. Récupéré en tâche de fond à l'ouverture de la fiche (mêmes
appels que ceux faits à l'ouverture de chaque onglet) : aucun chiffre tant que
la réponse n'est pas là, jamais de blocage de l'affichage de la fiche pour
l'attendre. Backgrounds se recalcule aussi à chaque changement de layout
sélectionné (même filtrage que la sous-vue elle-même, §6.1).

### 6.1 Onglet Médias et documents (fiche voiture/circuit)

Quatre blocs réunis dans un seul onglet (REFONTE§7.8), en deux groupes : ce
que **tu as produit** — **Screenshots**, **Replays** — puis ce qui est **livré
avec le mod** — **Ressources**, **Backgrounds** (cette dernière réservée aux
circuits). Le décompte de l'onglet est la somme des quatre. Aucun bloc n'est
masqué quand il est vide : ses actions « Ouvrir le dossier » et « Lier un
fichier… » sont la seule voie pour y ajouter quelque chose.

**Les documents livrés avec le mod mais rangés à part y figurent aussi** : une
notice, un manuel, des notes de version que l'import a stockés comme des mods à
eux (§4.5.2) parce qu'ils étaient hors du dossier du mod. Leurs fichiers ne sont
donc pas dans les ressources de la voiture — mais c'est bien là qu'on les
cherche. Ils rejoignent **la liste du bloc Ressources**, chacun marqué du nom de
sa livraison, et non une carte par document : trois cartes au-dessus d'un bloc
« Ressources » annonçant « aucun fichier annexe » disaient le contraire de la
vérité. Ils restent par ailleurs gérables depuis « Posé sur ce mod » (§4.3) et
depuis l'inventaire — lire et gérer sont deux gestes.

Rattachement par simple **`nom_de_fichier.contains(id)`**
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
  `<type>` une lettre de session (`R` course, `Q` qualification, le reste
  « autre »), suffixe final de longueur variable ou absent.
  `replay\temp\` (fichiers de travail) n'est jamais scanné. **Ce nom-là est
  celui d'un autosave du jeu** ; Content Manager renomme ceux qu'on conserve,
  motif par défaut `<car_id>_<track_id>_<ddmmyy>-<hhmmss>`. La date se lit donc
  en tête ou en fin de nom selon l'écrivain, et à défaut dans le mtime : un
  replay a **toujours** un horodatage, sans quoi le tri par date renvoyait en
  fin de liste, sans date affichée, le replay qu'on venait d'enregistrer.
- `<ac_install>\extension\backgrounds\<track_id>[__<layout_id>]_<variant>.jpg`
  (CSP) — convention propre, match par préfixe (double underscore avant le
  layout).
**Ce qu'une ligne de replay affiche** : le combo réellement piloté (voiture,
circuit et layout), l'horodatage, la durée, le nombre de voitures en piste, le
nom du pilote, la taille du fichier, et le nom du fichier lui-même en dessous.
Les cinq premières viennent de l'**en-tête du `.acreplay`** (`acreplay.rs`),
pas du nom : le nom ne distingue pas deux courses du même combo, et il ne dit
rien du pilote ni de la durée. Le bloc de frames ayant un pas fixe
(`frames × (4 + objets_de_piste × 12)` octets), les champs qui suivent sont
atteints par un seek — lire l'en-tête d'un replay de 635 Mo coûte le même temps
que celui d'un replay de 867 Ko, ce qui rend l'opération tenable pour chaque
ligne de la liste. En-tête illisible (replay d'une version antérieure, fichier
tronqué, replay en cours d'écriture) : la ligne s'affiche avec ce que le nom et
le disque donnent, jamais d'erreur.

**Autosave ou gardé** (badge sur chaque ligne). Il n'existe **aucun nettoyage
par ancienneté** : ni Pit Box ni Content Manager ne suppriment un replay au bout
de X jours. C'est Assetto Corsa qui fait tourner ses autosaves, et il les
**compte** — `cfg/replay.ini`, section `[AUTOSAVE]`, `RACE`/`QUALIFY`/`OTHERS`
(les réglages « Replays autosave » de CM écrivent ce fichier, ils n'ajoutent
rien de leur côté). Une ligne porte donc soit **« Autosave rang/limite »** — son
rang parmi les autosaves du même type de session, toutes voitures confondues, le
plus récent en 1 —, soit **« Gardé »** quand le fichier a été renommé, ce qui le
sort de la rotation. Le détail (quel type, combien AC en garde) est dans
l'infobulle du badge, jamais en libellé.

**La liste se recharge à la fin d'une session**, sans qu'on ait à rouvrir la
fiche : `onSessionEnd` (`detail/media.ts`) écoute la retombée de `ac://running`,
le sondage du process du jeu qui sert déjà à couper la musique de Big Picture et
à suspendre les vignettes de la grille (GRILLE§5.4) — pas un second sondage.
Deux passages, à la fermeture puis six secondes après, parce qu'il y a deux
écrivains : AC pose son autosave avant de rendre la main, Content Manager
renomme ou recopie le fichier une fois le jeu parti. Le même signal rafraîchit
les Screenshots et les décomptes des onglets.

- **Replays — bouton « Lire dans CM »** (`launch.rs::launch_replay`) : passe le
  chemin du `.acreplay` en argument à l'exécutable Content Manager — même
  mécanisme que l'association de fichier Windows au double-clic, et cohérent
  avec la façon dont `launch()`/`open_content_manager` invoquent déjà CM (un
  argument passé directement à `Command::new`, jamais via le gestionnaire de
  protocole système). Pas de vérification empirique poussée (pas d'install AC
  sur le poste de développement) — à confirmer à l'usage.

### 6.2 Fond photo sur l'écran de réglages de session

Sur l'écran de réglages (SESSION§3), image de fond assombrie/floutée derrière
l'interface, avec ordre de repli :
1. Screenshot personnel du **combo exact** (même voiture + même circuit sélectionnés).
2. Screenshot personnel du **même circuit**, autre voiture (ambiance du lieu conservée).
3. **Background officiel** du circuit (§6.1).
4. Fond neutre actuel (aucun média disponible).


### 6.3 Coquille de fiche unique (REFONTE§6.1, §12)

**Les cinq fiches — voiture/circuit, app, mod « autre », son, pack — partagent
un seul en-tête** (`FicheHeader.svelte`) : retour, tuile, nom, sous-titre, état,
favori et menu ⋮. Avant lui, quatre d'entre elles affichaient un **nom de
fichier en monospace** suivi d'une rangée de boutons ; le titre de la fiche
était donc l'identifiant technique.

- **La tuile montre la chose réelle** quand elle existe — badge de marque d'une
  voiture, vignette — et un **pictogramme de type** sinon. Jamais deux lettres
  tirées du nom : « PO » pour `policeman` n'apprend rien et se lit comme un
  badge de marque inexistant.
- **Le nom d'affichage se reprend à la main sur tous les types** (§5bis.3
  étendu), la saisie vivant dans l'overlay à côté du nom dérivé et jamais à sa
  place. Quand un nom est repris, l'identifiant technique passe en sous-titre.
- **L'auteur est dans le sous-titre** : c'est une propriété du mod.
- **Les actions vivent dans le ⋮**, sauf un contrôle par fiche quand il en est
  la raison d'être (la clé de contact d'un mod de son).
- Variante `flush` pour la fiche qui déborde les marges de l'écran, comme le
  `flush` de `Tabs.svelte`.

**Un seul vocabulaire d'état** (`StateBadge.svelte`), rendu dans l'en-tête et
dans la colonne « État » du tableau : **Actif** (vert), **Inactif** (orange),
**En attente** (jaune), **De base** (bleu), **Non géré** (gris). « En attente »
ne se fond pas dans « inactif » : un mod inactif a été désactivé, un mod en
attente est actif mais perd l'arbitrage sur un emplacement disputé et reprendra
sa place dès que l'autre partira.

### 6.3bis Les trois onglets de la fiche voiture/circuit (REFONTE§7)

| Onglet | Contenu |
|---|---|
| **La voiture** / **Le circuit** | Aperçu, sélecteur, fiche technique, courbe, son, bloc textuel |
| **Médias et documents** | Screenshots, Replays, Ressources, Backgrounds (§6.1) |
| **Installation** | Ajouts au jeu, décisions d'import, origine, historique, couches, étiquettes, extensions CSP |

Six onglets exposaient auparavant la mécanique : Ressources et Ajouts au jeu
étaient vides la plupart du temps et il fallait cliquer pour le découvrir.

**Les étiquettes vivent dans Installation** : elles sont la matière
première d'où la catégorie est dérivée, et on les ouvre au moment où cette
dérivation s'est trompée — donc en même temps que l'origine et les décisions
d'import. Ce que l'utilisateur consulte en premier onglet, c'est le résultat :
la **catégorie**, remontée en puce près du titre. Les **extensions CSP**
quittent les étiquettes pour Installation : elles décrivent l'installation, pas
le contenu.

**Le sélecteur de livrée/tracé est une carte de la colonne de droite**
(`PickerBar`) : il y voisine avec la fiche technique, le son et les habillages,
et en reprend le cadre, l'en-tête rouge et le compteur (« SKINS 28 »,
« LAYOUTS 2 »). Il a d'abord été une barre nue collée sous l'aperçu ; ce qui
comptait dans ce placement — **un contrôle ne doit jamais exiger de faire
défiler** — est conservé, il reste au-dessus de la ligne de flottaison. Mais une
barre sans cadre au milieu de cartes qui en ont détonnait sans rien dire de
plus : ce qui désigne un contrôle, c'est qu'il agisse, pas qu'il se distingue de
ses voisins. L'intitulé quitte le champ, l'en-tête le porte. Trois gestes pour trois façons de chercher : les **flèches** (défiler
en gardant l'œil sur l'aperçu, geste le plus fréquent), le **nom** (liste
déroulante, quand on le connaît) et **« Voir les N »**, qui déplie la grille de
vignettes en place — beaucoup de livrées de mods s'appellent `skin_01`, et une
liste de noms n'en dit alors rien. La grille est repliée à chaque ouverture de
fiche, sans persistance.

**Le premier onglet tient en deux zones empilées.** La première est une rangée
à deux colonnes qui **se referme sur la hauteur de l'aperçu** : l'aperçu seul à
gauche, toutes les cartes à droite (voiture : fiche technique et courbe côte à
côte, son, livrées ; circuit : Circuit, habillages, tracés). Une colonne de
grille s'étire par défaut à la hauteur de la plus haute, et le panneau de
données — qui porte un fond — courait donc jusqu'en bas de la page alors qu'il
n'avait de contenu que sur un tiers ; le vide se voyait surtout sur un circuit,
dont la colonne est la plus maigre. Le cas inverse est réglé par la même règle :
une colonne de droite plus haute que l'aperçu s'allonge, l'aperçu reste aligné
en haut, la rangée ne casse pas. Sous ~1 100 px de largeur **disponible**, la
rangée passe à une colonne.

La seconde zone est le **bloc de lecture**, sous un filet horizontal : pleine
largeur, mais contenu centré sur des **paliers de largeur** (ceux du
`.container` de Bootstrap — 100 % tant que la place manque, puis 540 / 720 /
960 / 1140 / 1320 px). Un pourcentage donnerait une largeur différente à chaque
résolution, donc un rendu qu'on ne peut régler pour personne ; une largeur fixe
déborderait en fenêtre étroite. Les sous-onglets vivent dans le même conteneur
centré, et le bloc n'a **plus de hauteur minimale** : il était dans la colonne
de gauche, où une description de trois mots devait ne pas faire sauter la
colonne voisine — il n'a plus de voisine, donc une ligne de texte occupe une
ligne. L'article Wikipédia à venir se lira là, dans cette colonne.

**Tous les seuils de cette mise en page sont des requêtes de conteneur, jamais
de média** : ils doivent se mesurer sur la largeur qui reste à la fiche, rail et
colonne de session déduits — et le zoom d'interface (§13) déplace un seuil de
média sans déplacer cette largeur-là.

**La colonne de droite n'a que deux cartes de données sur un circuit** —
« Circuit » (longueur, nombre de tracés et combien sont apportés par une couche,
pays, odomètre) puis « Habillages ». **La longueur porte son unité** (« 7 004 m »),
et celle-ci se déduit : mesuré sur les 70 `ui_track.json` de l'installation de
référence, le champ s'écrit en entier de mètres (61 fois), avec son unité déjà
dedans (`165km`, `4456 m` — 7 fois) ou en décimal de **kilomètres** (`3.602`,
Laguna Seca, une fois). Une valeur sous 100 est donc lue en kilomètres — le plus
court tracé réel est une piste de drag de 200 m — et une valeur portant déjà une
lettre est rendue telle quelle, l'auteur ayant dit ce qu'il voulait dire
(`trackLength.ts`). C'est tout l'argument du *cas maigre* de la
maquette : **la grille n'est plus dictée par le type le plus riche**. Un circuit
n'a ni fiche technique ni courbe, donc la place existe, et une rangée basse
d'une carte et demie sous l'aperçu rouvrait précisément le trou qu'on venait de
fermer. Le **nom** du tracé n'entre pas dans la carte — le sélecteur le dit à
quelques pixels au-dessus, et c'est lui qui le change ; la carte porte les
chiffres, lui porte le choix. Le sous-titre de l'en-tête suit la même logique :
une marque et une année pour une voiture, une longueur et un nombre de tracés
pour un circuit, dont l'identité tient dans ces deux chiffres.

**Le bloc textuel est à sous-onglets** (§7.4) : Description | Notes, c'est le
contenu qui change, pas la mise en page — même boîte, même corps de texte. Notes n'est jamais l'onglet par défaut
et porte une pastille quand une note existe.

**Un troisième onglet porte l'article Wikipédia** — « Le modèle réel » sur une
voiture, « Le circuit » sur un circuit
(`docs/SPEC-wikipedia-fiche-detail.md`). Il est **toujours présent**, et c'est une
révision née de l'usage : un onglet absent ne se distingue ni d'une recherche en
cours, ni d'une fonctionnalité qui n'existe pas, et il prive l'utilisateur de
tout recours au moment où il en aurait le plus besoin. L'article est cherché à
l'ouverture de la fiche, jamais en masse, et la fiche s'affiche complète sans
l'attendre.

Quand il n'y a pas d'article, l'onglet **dit lequel des cinq cas s'applique** —
recherche en cours, rien trouvé, ambiguïté, entité sans article lisible,
enrichissement désactivé — et propose d'en associer un à la main : recherche
libre pré-remplie avec ce qui a été cherché, candidats affichés avec leur
description Wikidata, collage d'URL accepté. Aucun de ces états n'est une
erreur : pas d'icône d'alerte, pas d'encart rouge. Un choix fait à la main est
enregistré en `manual` et plus rien d'automatique ne l'écrase.

L'article est affiché **en entier et rendu** — sections, table des matières,
tableaux, infobox et images : on lit dans Pit Box, on n'y trouve pas un teaser.
Le HTML de Wikipédia n'est jamais injecté tel quel : un arbre neuf est
reconstruit à partir d'une liste blanche, la webview ayant accès à `invoke` et
le wiki étant éditable par n'importe qui. Les images affichées sont **celles de
Commons dont l'auteur et la licence sont connus**, chacune portant sa ligne de
crédit — c'est ce que leur licence impose, et c'est ce qui permet de les
afficher. Un clic sur l'une d'elles l'ouvre en grand **dans
l'application**, par **la visionneuse de l'app** (§6.1) — celle des captures et
des backgrounds, avec ses gestes déjà connus : souris, clavier, manette,
précédent/suivant, Échap. Le **crédit et le lien vers Commons** l'accompagnent,
et c'est ce qui autorise l'affichage, pas une mention de politesse. Les icônes
(une étoile de notation, un drapeau) ne s'ouvrent pas : ce n'est pas du
contenu. Deux choses ne sont pas négociables
et viennent du droit d'auteur, pas du goût : le texte n'est **jamais fondu**
dans la description du mod — ce sont deux sous-onglets, donc deux blocs
distincts — et il est affiché **tel que l'API le rend**, sans reformulation,
résumé ni traduction. L'attribution en pied (titre, lien, licence CC BY-SA) est
obligatoire. Le lien « Voir sur Wikipédia » ouvre le **navigateur système**,
jamais une webview interne : elle casserait le mode hors ligne, imposerait la
CSP du site et ferait perdre l'identité visuelle de l'app. Ce qu'il apporte
n'est plus l'infobox ni les images — la fiche les montre — mais ce que la
reconstruction laisse volontairement de côté : les références et leurs notes,
la navigation vers les autres articles, et la version vivante de la page.

L'enrichissement est **désactivable** (`wiki_online`, §11) : l'app interroge
Wikipédia à l'ouverture d'une fiche, ce qui révèle indirectement le contenu de
la bibliothèque, et une partie du public joue délibérément hors ligne.
Désactivé, aucune requête ne sort et le cache déjà constitué reste
consultable.

### 6.3bis (suite) Tracés apportés par une couche (REFONTE§7.7)

La carte des tracés montre l'**état composé** — celui que l'utilisateur aura au
lancement, couches comprises. Elle ne disait pas d'où venait chaque tracé : sur
un circuit dont une extension ajoute une variante, « 2 tracés » est exact et
trompeur à la fois.

Un tracé porte donc une marque d'origine quand une couche **active** l'apporte,
et le compteur du sélecteur ajoute « dont 1 ajouté ». Deux conditions, et la
seconde compte autant que la première (`layers::layout_origins`) : la couche
doit poser des fichiers sous ce tracé **et** la base ne doit pas déjà le
connaître. Une couche qui remplace la texture d'un tracé existant l'habille,
elle ne l'apporte pas — l'étiqueter reviendrait à présenter le contenu propre
du circuit comme un add-on.

### 6.3ter Fiche d'une couche (REFONTE§8)

Une couche a sa **fiche**, ouverte depuis la liste de l'hôte et posée
par-dessus elle (la fermer y ramène). La liste de l'hôte ne fait plus que
poser, activer et ordonner : la liste complète des fichiers — jusqu'à 392
lignes en monospace — ne s'y déplie plus.

Ordre des blocs, et il n'est pas neutre :

1. **Trois chiffres** : ajoutés, remplacés (en rouge dès qu'il y en a), poids.
2. **« Ce qui écrase la base »**, en clair et non en compteur — c'est le seul
   endroit où une couche inquiète. La promesse du §4.4 (l'original est
   sauvegardé avant d'être remplacé et revient dès qu'aucune couche ne le
   réclame) y est écrite une fois, au lieu d'être répétée en bandeau partout.
3. **« Ce qui s'ajoute »**, replié par dossier, avec compteur et poids **au
   niveau du dossier** : le poids d'un fichier ne décide de rien.
4. **Ordre**, seulement à partir de deux couches sur le même hôte.
5. **Notes**.

Le **nom affiché est dérivé** (`layerName.ts`) : extension d'archive retirée,
séparateurs rendus à l'espace, préfixe de l'hôte retiré quand l'archive le
répète. Dérivé donc faillible, donc repris à la main quand il se trompe. La
dérivation réemploie `withoutBrand` (§7.4), qui porte déjà la règle délicate —
comparaison insensible aux séparateurs, mais coupe seulement sur une espace,
pour qu'un hôte `ks_nords` n'ampute pas `ks_nordschleife` en plein mot.

### 6.3quater Fiche d'un mod greffé simple (REFONTE§11)

Une police, un fragment de config, un mannequin : leur fiche n'affichait qu'un
titre, trois métadonnées et « aucun fichier annexe ». Deux blocs la comblent,
sans rien inventer — le parcours de fichiers qui détecte les conflits produisait
déjà tout ce qu'il fallait :

- **Où il atterrit** : les chemins réellement posés dans le jeu, groupés par
  dossier de destination et **relatifs à la racine d'AC** — le chemin
  d'installation de l'utilisateur ne dit rien et prend toute la largeur. Un
  mod peut n'avoir rien posé sans être en panne : désactivé, ou porteur d'une
  copie plus ancienne que ce qui tourne déjà (règle d'or n°5). Le bloc le dit
  plutôt que d'afficher une liste vide.
- **Conflits** : quels autres mods visent les mêmes fichiers, combien, et qui
  gagne — la priorité marquée à la main, ou à défaut la date.

Ces fiches restent en **page pleine posée sur la liste** ; le panneau latéral
du §6.2 de la refonte est écarté. Il avait déjà été retiré du projet pour cause
de redondance, et la fiche posée par-dessus — dont le retour ramène à la liste
d'où l'on vient — rend le même service sans ajouter un second contenant.

### 6.4 Notes (REFONTE§9)

**Tout mod peut porter une note libre**, quel que soit son type — exclure un
type créerait une règle à apprendre pour une économie nulle. Colonne
`notes_user` de l'overlay, sur les cinq tables d'entités, écrite par la
commande commune `set_entity_note` (`usermeta.rs`).

**Une note n'est pas une description**, et la différence porte sur un seul
geste : effacer. La description surcharge ce que dit le fichier du mod, donc la
vider veut dire « reviens au fichier » ; une note n'a pas de valeur d'origine,
donc la vider veut dire vide. D'où deux champs, et deux colonnes.

Comportement (`NoteBlock.svelte`) : **texte brut**, pas de markdown — un rendu à
moitié interprété est pire que pas de rendu ; **sauvegarde à la perte du
focus**, sans bouton ; phrase « enregistré dans Pit Box, pas dans les fichiers
du mod » affichée **pendant** la saisie, comme pour le renommage.

**Une note se retrouve**, sans quoi elle serait en écriture seule. Trois
mécanismes, livrés ensemble : elle entre dans la **recherche plein texte** des
écrans concernés ; la bibliothèque offre un filtre **« Note contient… »** et un
filtre booléen **« A une note »**, plus une colonne Note optionnelle ; et un
**marqueur ✎** apparaît sur la carte de grille et sur la ligne de liste, avec
la note en infobulle.

---

## 7. Bibliothèque et navigation

### 7.1 Deux bibliothèques distinctes

Voitures et circuits sont **deux bibliothèques séparées**, jamais mélangées : chacune a ses colonnes propres, persistées par type. Trois colonnes de dates : date d'ajout, date de mise à jour, date de publication (= date de modification des fichiers à l'import pour les mods ; pour le contenu de base, champ `release` de `kunos_content_dates.json`). **Date d'ajout et date de mise à jour absentes (`—`) pour le contenu de base** (`is_stock`, `MOD_SELECT` dans `overlay.rs`) : la date stockée en base pour ce contenu est l'instant du réindex, pas une vraie date d'ajout/MAJ — aucune source fiable n'existe pour ces deux dates côté contenu de base (le mtime du filesystem reflèterait seulement la date d'installation du jeu), donc mieux vaut l'absence explicite qu'une date affichée comme fiable alors qu'elle ne l'est pas. Seule la date de publication (`kunos_content_dates.json`, curatée) reste renseignée pour ce contenu. Distinction facile à manquer (deux dates propres à l'installation locale, une au mod lui-même) : une icône ⓘ sur chacun des trois en-têtes de colonne ouvre une info-bulle l'explicitant (`ColumnDef.tooltipKey` dans `columns.ts`, rendu dans `Library.svelte`).

**Les filtres sont des puces, pas une barre de contrôles** (`src/lib/library/filters.ts` pour le modèle, `src/lib/components/filters/` pour l'écran). L'écran affichait onze contrôles en permanence, sur deux rangées et environ 200 px de hauteur, pour quelqu'un qui en emploie un ou deux. Il reste **une seule ligne**, qui ne passe à la suivante que si les puces débordent vraiment. L'éditeur d'un filtre vit dans un popover accroché à sa puce : la complexité d'un filtre n'a donc plus d'effet sur la hauteur de la barre, un champ à jetons multiples avec opérateur occupe les mêmes 26 px qu'une case à cocher.

Trois règles portent le système :

- **La puce est une ancre, pas un contrôle.** Rien ne se règle dans la barre elle-même.
- **Le clic modifie, la croix retire**, sans exception ni type de puce particulier. Une puce booléenne s'inverse indéfiniment au clic et ne s'évapore jamais au troisième — c'est la conséquence directe de la règle, et c'est ce qui distingue « inverser » de « retirer ». L'état « filtre indifférent » n'est pas une position d'un contrôle : c'est **l'absence de la puce**, et donc l'absence de la clé dans l'état. Jamais une valeur vide conservée en mémoire pour dire « peu importe » — un éditeur ouvert puis vidé rend sa clé à la fermeture.
- **La polarité appartient à la valeur, pas au filtre.** `marque = Ferrari` et `marque ≠ Abarth` coexistent dans une seule puce ; il n'y a pas de bascule globale inclure/exclure.

**Quatre types de filtre.** `val` (liste de valeurs signées : marque, pays, auteur, tag, catégorie, classe, état), `range` (année), `text` (description, « contient ») et `bool` (favoris, déjà essayé, pilote choisi). Pour un `val` : *(inclusions combinées par l'opérateur)* **ET** *(aucune des exclusions)*. L'opérateur ET/OU **ne gouverne que les inclusions** — les exclusions sont toujours conjonctives, « sauf A ou sauf B » n'a pas de sens, on veut A et B écartés. Exclure l'emporte donc toujours sur inclure : un mod qui porte une valeur refusée sort, même s'il en porte une demandée. Entre filtres différents : toujours ET.

**L'opérateur n'est offert que là où un ET peut remonter quelque chose** : les tags (plusieurs par mod), les familles des voitures (§7.1bis) et les catégories **des circuits** (multi-valuées, §7.1). Sur une voiture, la catégorie est unique et un ET sur deux valeurs ne remonterait jamais rien. La pastille ET/OU ne s'affiche sur la puce qu'à partir de **deux** inclusions : avec une seule, ET et OU disent la même chose et la pastille serait du bruit.

**Le catalogue n'est pas le même sur les deux écrans.** Marque, année, classe et pilote choisi n'existent que pour les voitures — d'où des **épingles rangées par type** (`pitbox.pinned.cars` / `.tracks`), et non une liste partagée qui aurait posé des fantômes sans objet sur l'écran des circuits.

**Épinglage.** Un filtre épinglé reste visible en **fantôme** (bordure pointillée, glyphe `◈`, libellé seul, pas de croix) même sans valeur ; un clic l'ouvre, ou pose directement sa polarité par défaut s'il est booléen. La croix d'une puce épinglée la **ramène à l'état fantôme** au lieu de la faire disparaître ; celle d'une puce ajoutée à la volée la supprime. Même geste, deux résultats, mais les deux répondent à « annule ce que je viens de faire ». L'épingle se règle **dans le menu d'ajout, pas dans les réglages** : l'intention d'épingler se forme au moment où l'on ajoute Marque pour la quatrième fois de la journée, pas dans un écran de configuration où il faudrait se rappeler la liste des filtres hors contexte. Épinglés d'usine : **Catégorie, Favoris, État** — les trois existent des deux côtés, donc la rangée fantôme se lit pareil pour les voitures et pour les circuits, ce qui est précisément pourquoi Marque et Année n'y sont pas. État y est pour ce que faisait la case « contenu de base » retirée : l'utilisateur connaît par cœur les voitures livrées avec le jeu et cherche presque toujours autre chose, ce qui reste le filtre négatif le plus fréquent de l'écran.

**Aucun bouton de validation nulle part** : chaque modification s'applique immédiatement et le décompte de résultats bouge à la frappe. Un « Appliquer » ferait douter que le compteur parle bien de ce qui est affiché. Le popover se ferme au clic ailleurs ou à Échap, sans rien annuler.

**Les valeurs `val` sont fermées, et c'est une propriété du domaine.** Elles sont **dérivées de la bibliothèque** (les marques qu'elle contient, les auteurs, les tags…), donc une valeur saisie librement ne remonterait aucun mod par construction. Le champ de l'éditeur ne sert qu'à retrouver une valeur dans une longue liste — un `-` en tête (ou Alt+Entrée) bascule la sélection en exclusion, le bouton `−` au survol d'une suggestion la pose directement, et un clic sur un jeton déjà posé inverse son signe. Trois chemins vers le même résultat, tous nécessaires selon d'où l'on part.

**Poser une valeur ne dérange pas la liste.** La valeur choisie y reste, à sa ligne, avec son signe allumé : un second clic sur le même signe la reprend, un clic sur l'autre la bascule. Elle en sortait, avant, et la ligne suivante remontait sous le curseur — sur une liste de tags défilée, on perdait l'endroit qu'on lisait. Pour la même raison, la recherche tapée n'est **pas** effacée au clic (elle l'est sur Entrée, où taper est justement le geste en cours), et le récapitulatif des jetons posés se lit **sous** la liste : au-dessus, le premier jeton poussait le champ et toutes les suggestions d'une rangée. Toutes origines de tag confondues — fichier mod, règle, manuel — équivalentes pour filtrer, seule la fiche détail les distingue par origine.

**Le pays porte le drapeau du jeu** partout où il s'affiche — un pays se reconnaît à son drapeau avant de se lire : dans le filtre (liste de l'éditeur, jetons posés, puce de la barre), sur la fiche technique d'une voiture et dans la fiche d'un circuit. Sur la fiche, **c'est le pays rangé qui s'affiche et non celui du fichier**, contrairement aux autres lignes de la fiche technique : c'est le seul champ que l'app normalise, et y lire « U.S.A. » quand le filtre et la colonne disent « United States » ferait douter qu'il s'agisse du même pays. La table est celle d'Assetto Corsa (`nationalities.rs`, la même que la colonne Nationalité du plateau) et la correspondance se fait **par le nom, insensible à la casse** : les valeurs viennent des `ui_*.json` des mods, où l'auteur écrit ce qu'il veut.

**Le pays se lit dans la langue de l'utilisateur, traduit depuis son code ISO** (TAXO§12) : dans la puce, l'éditeur du filtre, les tuiles de l'index, la colonne Pays du tableau et la fiche. Le code vient de la table du jeu (`nationalities.rs` : alpha-3 → alpha-2, table mesurée — voir le commentaire de `ISO2`), jamais du nom : 33 des 221 noms du jeu n'ont pas de correspondance exacte dans les noms de régions du moteur (`Czech Republic`, `Russian Federation`, `Turkey`…). Les quatre nations britanniques, sans code ISO, se traduisent par clé (`countryNames.*`) ; une valeur inconnue du jeu garde son orthographe. **La valeur stockée, filtrée et persistée reste le nom anglais du jeu** — seul l'affichage change, et le tri du tableau se fait toujours sur elle. Le drapeau se cherche donc par la valeur, jamais par le libellé.

**La correspondance est exacte, parce que la valeur a déjà été normalisée à l'écriture** (§5, alias de pays). Le filtre ne rattrape donc aucune orthographe : il lit ce qui est rangé. Un pays absent de la table du jeu garde son texte et n'a pas de drapeau — une correspondance approchée en poserait un faux, ce qui est pire. Dans la liste, une case vide tient alors la place, sans quoi les noms cesseraient de s'aligner.

**Les décomptes par valeur sont calculés sur le type courant, pas sur les résultats filtrés** : un chiffre qui bouge à chaque jeton posé ne sert à rien pour décider du jeton suivant. Une passe sur les cartes par champ, refaite seulement au chargement de la liste (mesuré : 423 voitures pour 43 auteurs distincts, bien sous la milliseconde), puis un simple accès de `Map` par ligne affichée. Même raisonnement pour l'évaluation : tout ce qui se résout par filtre (séparation des signes, mise en minuscules, choix de l'accesseur) est compilé **une fois** dans un prédicat par `buildPredicate`, jamais par carte — la liste entière est reparcourue à chaque frappe de n'importe quel champ.

**« État » dit maintenant la même chose que la pastille d'état.** C'était un `<select>` à trois positions (indifférent / actif / inactif) ; c'est une liste fermée à cinq valeurs signées, alignée sur `StateBadge` : **Actif, Inactif, De base, Non géré** — mutuellement exclusives, mêmes arbitrages que la pastille — plus **Cassé**, qui les traverse (un mod cassé est par ailleurs actif ou inactif). Au passage, un défaut est corrigé : `c.active` vaut aussi vrai pour le contenu de base, si bien que l'ancien « Actif » remontait tout le catalogue Kunos avec les mods déployés. « Actif » veut désormais dire ce qu'il dit. **Classe** suit le même modèle plutôt que de rester un `<select>` : ses valeurs sont lues sur la bibliothèque (`car_class` est une chaîne libre du `ui_car.json`, pas une énumération), donc une classe exotique s'y trouve au lieu d'être invisible.

**Il n'y a plus de case « contenu de base » à part.** Elle disait exactement ce que dit `État = De base`, et deux commandes pour une même question, c'est par là qu'une barre repasse à onze contrôles. Ce qu'elle achetait — un clic vers « tout sauf les voitures Kunos » — est repris par l'épingle sur État, dont l'éditeur propose cette valeur avec un `−` à côté. Le test reste le même : `is_stock && !is_unmanaged`, jamais `is_stock` seul — le drapeau couvre tout ce qui vit dans `content/`, mods installés hors Pit Box compris (§8), et ceux-là sont la valeur « Non géré ». Une préférence qui portait l'ancienne case est **rejouée** comme valeur d'État (`foldBaseIntoState`) plutôt que perdue : `sanitize` écarte les clés que le catalogue ne connaît plus, donc sans ce repli un « hors contenu de base » enregistré serait revenu sans aucun filtre, en silence.

**La recherche libre est le seul filtre sans puce**, et c'est voulu : elle traverse plusieurs champs à la fois (nom, marque, id de dossier, catégorie, pack §4.4, tags — mais **pas** la description), ce qu'aucune puce ne saurait résumer. Elle est donc en tête de la coulée, toujours visible, avec sa propre croix, et **« Tout effacer » l'efface aussi** — sans quoi un texte tapé seul resterait actif sans qu'aucun bouton ne propose d'en sortir. Elle est aussi le point d'entrée des recherches imposées de l'extérieur (« filtrer par pack », §4.4). Un terme par mot séparé par un espace, ET entre eux, chacun en simple « contient » : bug réel signalé, « GT-M Evo » ne remontait pas « GT-M Adonis Evo », recherché comme une seule sous-chaîne collée.

**L'exclusion n'a pas de couleur à elle.** Elle se dit par le mot (« sauf », « Hors contenu de base ») et par la rature sur les jetons de l'éditeur. Le jaune de l'app est l'alerte et le rouge le destructif : les charger d'un second sens les viderait du premier.

**Persistance** : les valeurs des filtres et les épingles vivent dans `ui_prefs.json`, par type (SESSION§1). Les valeurs survivent au redémarrage — les puces disent d'un coup d'œil ce qui est appliqué, ce que l'ancienne barre à onze contrôles ne faisait pas, et c'est ce qui rend le maintien du filtre sans danger. La relecture convertit les **deux générations précédentes** d'instantané (le texte à virgules et les `<select>` mono-valués d'abord, les jetons inclure/exclure et les tri-états ensuite) : une bibliothèque laissée filtrée avant une mise à jour se rouvre filtrée après.

**Catégories pour les circuits** : les circuits ont aussi des catégories (comme les voitures), pour filtrer et composer.

### 7.1bis L'index : l'état par défaut des deux bibliothèques

Spec détaillée : `SPEC-index-bibliotheque.md` (`INDEX§`). Sans puce active **et** sans texte dans la recherche, l'écran ne montre plus le mur de trois cents vignettes : il montre un **index**, une grille de tuiles qui donne la vue d'ensemble et sert de porte d'entrée vers le filtre. Une puce fantôme (épinglée, sans valeur) ne compte pas : elle ne filtre rien.

**La tuile pose une puce, et rien d'autre** (INDEX§2). Elle écrit dans le même état de filtre que l'éditeur de puce (`poseValue`, `filters.ts`) : croiser avec une année, exclure un tag, « Tout effacer », le décompte, l'épinglage marchent donc sans une ligne de plus. Retirer la dernière puce ramène à l'index ; il n'y a pas de bouton « retour à l'index ». **L'index n'est pas mémorisé** — mais les filtres le sont (§7.1) : une bibliothèque laissée filtrée se rouvre sur sa liste, pas sur l'index.

- **Circuits : un index, par pays.** Drapeau du jeu, nom **traduit depuis le code ISO** (le nom anglais rangé est retrouvé dans la table de régions du moteur ; un nom sans correspondance exacte — l'Écosse, qu'AC drapeaute à part — garde sa forme anglaise). Le lien **« Voir tous les circuits »** mène à la liste complète sans poser de puce.
- **Voitures : deux index, familles puis marques.** Pas d'index par pays pour les voitures (INDEX§6.3) : trois grilles empilées redeviennent un mur, et « Japon » redit moins bien ce que dit `#jdm`.
- **La tuile de valeur absente** (« Non renseigné », « Non classé ») ferme la grille, en pointillés : elle rend le trou de données visible. Elle pose une valeur comme les autres (`UNSET_VALUE`), que la puce montre, exclut et persiste de la même façon ; l'éditeur du filtre la propose aussi, en dernier.
- **Les marques significatives seulement** : celles qui, prises par nombre décroissant, couvrent 80 % de la bibliothèque, bornées à [8, 24] — jamais un « top N » figé. « Toutes les marques » déplie le reste par ordre alphabétique, sans mémoriser le dépli. L'emblème est provisoire : le badge de la voiture au plus petit id qui en a un, faute de logo canonique (`SPEC-taxonomies.md`, TAXO§4, non implémenté) ; des initiales sinon.
- **Poser une seconde valeur croise ou remplace selon le filtre**, jamais selon la taxonomie : un filtre à opérateur (plusieurs valeurs par mod) ajoute sous ET, un filtre mono-valué remplace.

**Les familles sont un filtre à part, pas la catégorie.** `Famille` regroupe des tags (`Prototype ← lmp1, group c, lmh…`, TAXO§7.3) ; une voiture en porte autant que ses tags en atteignent, d'où des décomptes qui ne s'additionnent pas — le sous-titre de la section le dit. `Catégorie` reste le premier tag `#` (convention CM) : la puce « même catégorie que ma voiture » du bloc Adversaires la pose, et la transformer en famille élargirait une grille GT3 à toutes les voitures de course. La table des familles vit dans les règles de tags (`car.category_families`, remplie depuis le seed si absente) : c'est un **index, pas une règle** — le moteur ne la lit pas, rien n'est écrit dans l'overlay, et les tags sont comparés sans casse ni `#` de tête. Un tag n'appartient qu'à une famille ; le seed livré en compte huit (Route, Sportive, Course, Classique, Monoplace, Prototype, Drift, Rallye). Mesuré sur la bibliothèque de référence : 359 voitures, 601 appartenances, 34 non classées (du trafic, pour l'essentiel).

**L'onglet Catégories de l'Atelier édite la table** (TAXO§6) : une ligne par famille (silhouette, nom, nombre de voitures, nombre de tags, fanion ⚑ quand elle diffère de la version livrée), triée par nombre de voitures, qui se déplie sur son nom, son icône choisie dans le jeu embarqué, et ses tags. Le champ d'ajout propose les tags que la bibliothèque porte, avec leur nombre de voitures et la famille qui les tient déjà : **rattacher un tag le déplace**, il ne se copie jamais. Une famille créée reçoit un id tiré de son nom **une fois pour toutes** — c'est ce que stocke une puce posée, qui doit survivre à un renommage. « Rétablir » remet une famille livrée dans son état d'origine et **renvoie chez eux** les tags qu'elle avait gagnés (un `gt3` passé dans Classique retourne dans Course, il ne disparaît pas) ; « Tout rétablir » et « Supprimer » demandent un second clic. Chaque modification s'écrit aussitôt, par une commande qui n'enregistre **que** la table (`save_category_families`) : pas de réharmonisation de la bibliothèque, une famille n'étant pas une règle. Le fichier écrit est normalisé côté Rust (ids uniques, tags sous leur forme comparée, un tag dans une seule famille), puisqu'il reste éditable à la main. Une table vidée entièrement revient à la version livrée au démarrage suivant (même repli que les autres tables des règles).

**Catalogue et surcouche** (`SPEC-regles.md`, REGLES§2 ; crate `pitbox-catalog`, `taxonomy.rs`). Les trois tables de taxonomie — familles, alias de pays, tags de pays — ne sont **plus recopiées** dans `tag-rules.json` : le **catalogue** est un fichier à part, `src-tauri/rules/taxonomy-catalog.json`, embarqué et relu à chaque chargement, et les **décisions** de l'utilisateur vivent à part, dans `taxonomy.json`, indexées par la clé naturelle de chaque entrée (le tag, l'orthographe, l'id de famille). Une mise à jour de Pit Box remplace donc le catalogue sans rien toucher à ce que l'utilisateur a décidé, et une amélioration du catalogue atteint une table qu'il a modifiée : la surcouche l'emporte entrée par entrée, en silence (REGLES§3). Une suppression est une **pierre tombale**, pas une absence — sans quoi le catalogue suivant ramènerait l'entrée. « Rétablir » n'est plus un calcul : c'est retirer les décisions qui touchent l'entrée. La surcouche des familles est **par tag, pas par famille** : dériver toute une famille pour y déplacer un tag la figerait, et le catalogue suivant ne pourrait plus y ajouter. Une entrée qui vise une famille disparue est gardée, sans effet ni erreur (REGLES§4). La table effective n'est calculée qu'en Rust : les onglets éditent la surcouche et affichent ce que Rust renvoie après chaque enregistrement.

**Ce qui est livré se distingue de ce que l'utilisateur a fait** (REGLES§8.1). Une famille livrée porte le badge `PIT BOX` ; dans une ligne dépliée (famille ou pays), un tag ou une orthographe **ajouté** par l'utilisateur est en pointillés et marqué `✎`, et un élément **livré qu'il a retiré** — ou déplacé ailleurs — reste affiché barré, avec `↺` pour le remettre. Sans ça, une suppression ne laissait aucune trace, et rien ne disait ce qu'une mise à jour ferait évoluer.

**Côté développeur, le catalogue se cure dans l'app** (`rules-tool`, crate jamais livrée) : on organise dans les onglets Catégories et Pays comme un utilisateur, puis `npm run rules:diff` liste ce que ces décisions changeraient au catalogue, et `npm run rules:promote` les y écrit et vide la surcouche (l'ancienne gardée en `.bak`). La fusion est celle de l'app — la même crate — donc ce qui est promu est exactement ce que l'app montrait, et le fichier réécrit ne bouge que sur les lignes concernées (testé : une promotion à vide le rend à l'octet près).

**Les règles de l'écran Règles suivent le même modèle** (`rule_overlay.rs`, `pitbox_catalog::rules`). Chaque règle livrée porte un **identifiant écrit à la main**, stable, jamais tiré de son contenu (`pitbox.brand.bayro`, `pitbox.car-merge.gt1`… — REGLES§4) : une règle corrigée dans une version suivante garde son identifiant, et un utilisateur qui l'avait désactivée la garde désactivée. Les décisions vivent dans `rules-overlay.json`, par section : règles livrées **désactivées** (un drapeau — elles continuent de recevoir les améliorations, REGLES§5), **dérivées** (une copie figée, avec l'empreinte de la version d'origine, pour dire plus tard qu'une nouvelle version existe), et **les siennes**. Les règles de l'utilisateur passent **devant** celles du catalogue : pour les règles qui s'arrêtent à la première correspondance (marque, classe, fusion de tags, specs), c'est ce que veut dire « la règle utilisateur gagne » (REGLES§3), et c'est déjà là que l'écran Règles insère une règle nouvelle. La liste ordonnée des catégories de circuit garde l'ordre de l'utilisateur quand il l'a changé ; une catégorie ajoutée par une version suivante se range après. Les règles de l'utilisateur ont aussi un identifiant, `own-N`, attribué par Rust à la normalisation — déterministe, donc le même à chaque chargement d'un fichier écrit avant qu'il existe. **`tag-rules.json` n'est plus lu** : migré une fois, il est mis de côté sous `tag-rules.pre-overlay.json` — gardé, jamais supprimé —, et seulement quand les deux fichiers de décisions sont écrits.

**L'écran `Atelier › Règles` édite la surcouche, geste par geste** (REGLES§8, `RulesEditor.svelte`, `rulesEdit.ts`). Une seule liste par section, **dans l'ordre d'exécution** — les règles de l'utilisateur, puis celles du catalogue, une dérivée à la place de la règle qu'elle dérive —, parce que c'est le seul ordre qui explique un résultat. Chaque ligne : une **bascule** (immédiate, sans confirmation : désactiver n'est pas dériver, REGLES§5), le badge `PIT BOX` sur une règle livrée et `✎` sur une dérivée, ce que la règle cherche → ce qu'elle donne, `⚑ nouvelle version` quand la règle livrée a changé depuis la dérivation, le **compteur d'effet**, et un menu `⋮` (modifier, dupliquer, rétablir le défaut pour une dérivée, supprimer pour une règle à soi — une règle livrée ne se supprime pas, elle s'éteint). **Modifier une règle livrée demande d'abord** (REGLES§8.2) : « Modifier quand même », « Désactiver plutôt », « Annuler ». En tête, l'**interrupteur global** « Utiliser le catalogue de règles Pit Box » avec la version et le nombre de règles livrées (REGLES§7) : éteint, le catalogue entier cesse de s'appliquer, les règles de l'utilisateur et ses dérivées continuent — seules les règles en liste sont concernées, les tables de taxonomie ne sont pas des règles (REGLES§11). Un filtre « Mes règles seulement ». Plus de barre d'enregistrement : **chaque geste est écrit aussitôt et réappliqué à toute la bibliothèque**, et les compteurs reviennent mesurés par cette même passe (`harmonize::harmonize_all_counting`, une transaction pour toute la passe). Le moteur note, pour chaque mod, les identifiants des règles qui ont agi (`Harmonized::fired`, jamais stocké ni comparé) : une règle à première correspondance qui a correspondu, chaque « nom → tag » qui a ajouté, chaque catégorie de circuit retenue. Mesuré sur l'install de dev (350 mods, build de debug) : 0,2 s pour les compteurs à l'ouverture, 0,2 s pour une réapplication. La liste blanche des catégories de circuit n'a pas d'identifiants — c'est une liste ordonnée de noms : une catégorie livrée s'éteint (retrait), une à soi se supprime, et monter ou descendre rend l'ordre à l'utilisateur.

**Un nouveau catalogue s'applique tout seul, et le dit après** (REGLES§6, `catalog_update.rs`). Le catalogue étant embarqué, il change avec une mise à jour de l'app. Au démarrage, l'empreinte du catalogue embarqué est comparée à celle vue la dernière fois (`catalog-state.json`) : si elle diffère, la bibliothèque est **reclassée** sous le nouveau catalogue — sans quoi une amélioration n'atteignait que les imports suivants, puisque la classification est stockée et que seul un changement de `ENGINE_VERSION` la recalculait — et un **rapport** est gardé : règles ajoutées, corrigées, retirées (par identifiant pour les règles en liste, par clé naturelle pour les tables), et les mods effectivement reclassés, mesurés en classant la bibliothèque sous l'ancien et le nouveau catalogue (`harmonize::snapshot`). Jamais d'assistant qui demande avant (REGLES§6.2) : une **notification** qui reste jusqu'à ce qu'on la ferme, dans la pile en bas à droite, et un **bandeau** en tête des onglets Règles, Catégories et Pays, dépliable sur le détail. **« Revenir à »** réinstalle le catalogue précédent, conservé en entier dans le fichier d'état, jusqu'à la mise à jour suivante ; « Réappliquer » en sort. Une seule version précédente est gardée, et les décisions de l'utilisateur ne sont touchées ni dans un sens ni dans l'autre. Une mise à jour vue sans bibliothèque accessible est remise au démarrage suivant plutôt que mesurée sur rien. Le premier lancement d'une version portant ce mécanisme n'a rien à comparer : il note le catalogue et ne rapporte rien.

**La migration est faite une fois, et à diff nul** (REGLES§13.5, testé) : les tables qu'un ancien `tag-rules.json` contenait en entier deviennent la surcouche qui les reproduit exactement. Elle compare au **manifeste figé** des tables telles qu'elles étaient recopiées (`rules/manifests/pre-layer-tables.json`), jamais au catalogue courant — un utilisateur qui sauterait des versions verrait sinon des améliorations qu'il n'a jamais reçues enregistrées comme ses propres décisions, et figées. Mesuré sur l'historique : les tags de pays sont identiques dans toutes les versions publiées (v0.1.0 à v0.7.0), alias et familles n'ont jamais été publiés, et **les règles en liste n'ont jamais changé du premier commit à v0.7.0** — un seul manifeste figé les représente (`rules/manifests/pre-layer-rules.json`), porteur des identifiants du catalogue, et c'est par lui qu'une règle recopiée est reliée à la règle livrée qu'elle était. Une règle livrée que l'utilisateur avait modifiée se lit comme un retrait plus une règle à lui (REGLES§13.3) : rien ne se perd, sa version gagne. Rejoué sur l'install de dev : aucune décision, 350 mods classés à l'identique. Un `taxonomy.json` illisible n'est jamais réécrit : la surcouche est ignorée pour la session, et l'échec journalisé.

**La bascule de vue passe à deux positions tant que rien n'est filtré** — index / liste complète — puisqu'une grille de tuiles n'a pas de densité ; la liste complète s'affiche dans la densité choisie, que le menu voisin continue de régler. Trois positions dès qu'un filtre fait une liste. En Big Picture, l'index s'efface devant la vue imposée.

### 7.2 Trois territoires

L'application compte une douzaine de destinations. Elles se répartissent en trois zones dont la frontière doit rester **étanche** — toute entrée d'interface nouvelle se rattache à l'une des trois :

| Territoire | Question | Contenu |
| --- | --- | --- |
| **Rail** (à gauche, `NavRail.svelte`) | *où je vais* | les destinations — des lieux qu'on parcourt, plus **Ouvrir CM** en pied |
| **Barre de titre** (en haut, `TitleBar.svelte`) | *quelle forme a la fenêtre* | réduire, agrandir, fermer, Big Picture — **et l'identité de l'app** : logo, nom, sous-titre |
| **Colonne de session** (`AppShell.svelte`) | *ce que je lance* | circuit, voiture, livrée, pilote, performance, type de session, lancement |

**La marque est dans la barre de titre, pas dans la colonne.** Elle a occupé un
bandeau en tête de la colonne de session — tuile, nom, sous-titre empilés — soit
une cinquantaine de pixels pris sur la seule ressource rare de cette colonne,
sa hauteur, pour une information qui ne change jamais. La barre de titre est
déjà l'endroit où une application se nomme, et elle portait le nom sans le logo
ni le sous-titre : les trois s'y rangent sur une ligne, à coût de hauteur nul.
Conséquence assumée : en mode Big Picture, où la barre de titre est masquée, la
marque n'apparaît nulle part — c'est un mode immersif, il n'a pas à se nommer.

Avant ce découpage, la colonne de session faisait office de navigation en plus de son travail propre, et les deux grilles de boutons `ADD-ONS` et `ATELIER` qu'elle portait en pied étaient orphelines : elles n'étaient pas mal dessinées, elles étaient mal placées. Deux conséquences qui ne se devinent pas : **« À propos » quitte la barre de titre** (c'est du contenu — version, liens, dépôt — pas un état de fenêtre) et descend au pied du rail ; et **« Ouvrir CM » quitte la navigation pour la colonne de session** (ce n'est pas une destination mais un chemin de lancement alternatif — le critère de rangement est l'intention, pas le fait que la cible soit externe).

**Ouvrir CM est depuis revenu dans le rail**, en pied, entre Réglages et À propos — décidé avec l'utilisateur. L'argument ci-dessus reste exact et ne suffisait plus : la colonne de session n'a pas de hauteur à donner à ce qui n'est pas la session, et le pied du rail ne porte déjà plus des lieux qu'on parcourt mais **ce qu'on ouvre à part**. L'entrée n'est donc pas une destination pour autant : elle n'est jamais active, ne porte pas d'`aria-current`, et **disparaît entièrement** quand Content Manager n'est pas détecté au chemin configuré — ni entrée grisée, ni message d'erreur au clic. Son icône est la flèche qui sort du cadre, qui porte le « ouvrir » ; son libellé se réduit donc à `Content Manager`, le rail faisant 74 px et « Ouvrir Content Manager » y tenant sur trois lignes quand toutes les autres entrées en font deux.

**Règle d'architecture : le rail porte les lieux, les onglets vivent à l'intérieur d'un lieu, aucun lieu n'a deux niveaux d'onglets.** C'est elle qui décide de tout le reste. Les deux inventaires restants (compléments, apps) portent déjà leurs propres facettes ou onglets : les ranger sous un onglet supplémentaire produirait deux rangées horizontales de forme identique, sans que rien n'indique laquelle commande l'autre. Ils sont donc des entrées de rail à part entière. Les quatre outils de l'Atelier, à l'inverse, n'ont **aucune** sous-rubrique — c'est la seule raison pour laquelle ce regroupement-là est légitime et l'autre non (§7.2quater).

**Sept entrées, deux rangs nommés** : *La session* — Circuits · Voitures ·
Pilote — puis *Le jeu* — Apps · Compléments — puis Atelier, et en pied
Réglages · Ouvrir CM · À propos. **Le circuit avant la voiture**, comme dans la
colonne de session : c'est l'ordre de la décision (SESSION§1), et deux listes qui
portent les mêmes entités dans deux ordres différents se paient à chaque coup
d'œil. Les rangs ne classent pas par type de contenu mais par
**durée de validité** de ce qu'on y règle : ce qui se décide à chaque session,
et ce qui reste vrai jusqu'à nouvel ordre. Les deux écrans d'add-ons ont
disparu — ils classaient par mécanique d'installation, c'est-à-dire par la
complexité que l'app existe pour absorber — et leur contenu vit dans
l'inventaire (§7bis). Apps devient une entrée : une app a un nom, une identité,
on l'installe volontairement, elle n'est la dépendance de rien. Chaque entrée porte une **icône et un libellé** : le rail n'est pas iconographique seul, « Add-ons voiture » contre « Compléments » n'étant pas une distinction qu'une icône peut porter, et un rail muet se paie en infobulles pour un gain de largeur sans valeur ici.

L'entrée active — celle dont l'écran est affiché — se marque par un **filet gauche rouge de 2 px** (`box-shadow: inset`, jamais une bordure : 2 px de bordure décaleraient le contenu d'un pixel à chaque changement d'écran), plus une icône pleine et un libellé en pleine lumière. C'est la seule apparition du rouge dans le rail (§7.2ter). **La colonne de session porte le même filet, et ce n'est pas une brèche dans la frontière** : le rail marque le *lieu* où l'on est, la colonne marque le *type de session* qu'on regarde (SESSION§1.1) — deux échelles, jamais deux réponses à la même question. Et la colonne distingue deux marques là où le rail n'en a qu'une : le libellé en pleine lumière dit *le type qui partira*, quel que soit l'écran affiché (c'est une valeur, comme la voiture et le circuit au-dessus), le filet rouge dit *on est en train de le regarder* et ne s'allume donc que sur l'écran de session. Au clavier, flèches haut/bas pour circuler dans le rail (bouclé aux deux extrémités), `Entrée` pour activer, `aria-current="page"` sur l'entrée active.

**Pastille d'alerte** : un point de 6 px en haut à droite de l'icône, bordé de la couleur du rail, sur une rubrique qui contient un problème. Pas d'agrégat sur une entrée parente et **pas de compteur** — il faut voir *laquelle* aller regarder, et le nombre exact ne change pas cette décision. Une seule source aujourd'hui : les conflits de fichiers entre compléments, que `list_others` calcule déjà. Les autres inventaires n'ont pas de notion de « problème » à remonter.

### 7.2quater Atelier

Règles, Catégories, Pays, Importer, Profils et Maintenance sont les six **onglets d'un même écran** (`Workshop.svelte`), titré `Atelier`. Catégories et Pays sont deux des trois onglets de la spec taxonomies (TAXO§6) ; avec Marques, l'Atelier atteindra sept onglets, la limite fixée par cette spec avant de passer à une liste latérale.

**L'onglet Pays** (TAXO§6) liste les pays rangés — drapeau, nom traduit (le nom rangé à côté quand il diffère), nombre de voitures et de circuits, nombre d'orthographes, fanion ⚑ quand elles diffèrent de la version livrée — triés par nombre de mods. Une ligne dépliée montre son code ISO (ou « inconnu du jeu »), ses orthographes rattachées (ajout, retrait), « Rattacher à… » (fusionner ce pays dans un autre, ce qui est aussi le moyen de le renommer : toutes les orthographes qui y menaient suivent) et « Rétablir les orthographes d'origine ». **Les valeurs inconnues du jeu** (sans drapeau, TAXO§3.1) sont proposées en bandeau jaune, le pays du jeu le plus proche présélectionné quand il y en a un (faute de frappe, nom tronqué), avec « Rattacher » ou « Ignorer » — mémorisé (`country_aliases.ignored`), réversible en pied d'écran. Un pays sans mod dont des orthographes restent curées n'est pas perdu : masqué, il réapparaît avec « Afficher les pays sans mod ». **Chaque modification réapplique les alias à toute la bibliothèque** (`save_country_aliases`), puisque le pays est décidé à l'écriture — contrairement aux familles. Chacun est un écran à une tâche sans sous-rubrique : c'est ce qui rend le regroupement possible sans créer le double niveau d'onglets que §7.2 interdit. Quatre entrées de rail économisées, et les quatre outils gagnent une maison visible au lieu d'être quatre boutons dans une grille. Chaque onglet garde son propre **sous-titre** — il décrit l'outil, là où le titre décrit le lieu.

**L'onglet reste porté par `nav.section`**, pas par un état local, et c'est ce qui compte à l'usage : la douzaine d'endroits qui appellent déjà `requestSection("import")` (le glisser-déposer global, un rapport d'import, un renvoi depuis la bibliothèque) atterrissent sur le bon onglet sans rien savoir de cet écran, la garde de navigation (§11) et l'historique (§7.2bis) restent en place, et l'entrée du rail — qui vise `rules` — repart forcément du premier onglet. **L'onglet actif n'est pas mémorisé.**

Les **préférences d'import** ont suivi l'opération : elles ont quitté les Réglages pour une **section repliable en pied de l'onglet Importer**, repliée par défaut. Deux noms quasi identiques dans deux endroits différents — l'un l'action, l'autre ses préférences — produisaient des allers-retours. Une section et non un onglet : cet écran est déjà un onglet de l'Atelier. Écriture immédiate, sans bouton Enregistrer (deux réglages, aucun aperçu live à valider ou annuler), un échec s'affichant plutôt que de se perdre.

### 7.2ter Barème de l'accent rouge

**Ce que le rouge signifie, en une phrase : *ce qui est retenu pour la session, et l'action qui la lance*.** Tout ce qui relève de la structure, de la navigation ou d'une action secondaire n'y a pas droit. Le rouge servait à huit choses (bordure de fenêtre, logo, filet sous chaque titre de section, encadré du bloc voiture, boutons « Changer », bouton de livrée, carte en session, bouton de lancement) : quand tout est accent, rien ne l'est, et « Démarrer la session » — la seule action que l'écran doit pousser — se retrouvait en concurrence avec quatre titres et deux boutons secondaires.

Quatre niveaux. Tout élément rouge doit pouvoir se rattacher à l'un d'eux ; sinon il ne prend pas de rouge.

| Niveau | Traitement | Sens | Quota |
| --- | --- | --- | --- |
| **1 — plein** | fond `--rosso`, texte blanc | l'action qui engage la session | **un seul élément à l'écran** (aujourd'hui : « ▶ Démarrer la session ») |
| **2 — trait** | bordure ou filet `--rosso`, 1 à 2 px | ce qui est retenu pour la session, ou ce qui a le focus | un par famille d'objets |
| **3 — éteint** | bordure `--rosso-border`, fond `--rosso-dim` | actif mais secondaire | libre |
| **0 — marque** | `--rosso` | identité, hors zone de contenu | deux occurrences fixes : le filet supérieur de la fenêtre et le carré du logo |

Niveau 2 : entrée active du rail, carte du duo en session dans la grille (+ badge `SESSION`), onglet actif, piste active de l'écran Pilote, case retenue dans une galerie, **option retenue d'un groupe segmenté** — c'est le ton `accent` de `Seg.svelte`, fond éteint plus filet de 2 px. Ce ton a longtemps rendu un fond rouge **plein**, et comme il est le défaut du composant, l'écran de session en comptait cinq : quatre segmentés plus le bouton de lancement, dont un seul a droit au niveau 1. Les quatre appels qui s'étaient posé la question avaient tous répondu `neutral` — le rouge n'était donc choisi nulle part, seulement hérité. Le **focus** est le seul emploi qui peut apparaître n'importe où — c'est cohérent, il désigne « où je suis » — et il reste **jaune** dans Pit Box (`:focus-visible`, `global.css`) : la carte en session portant elle-même une bordure rouge, un focus rouge s'y fondrait, ce qui était déjà la raison du jaune avant ce barème.

**Le survol n'introduit jamais de rouge sur un élément qui n'y a pas droit au repos.** Sinon le rouge acquiert un quatrième sens — « sous le curseur » — qui annule le barème. Un contrôle neutre (bouton secondaire, menu, champ, « + Filtre ») éclaircit son gris (`--line` → `--faint2`/`--faint`) ; seul un élément *sélectionnable* (carte, case de galerie, puce, option de liste) peut aller jusqu'au niveau 3, et l'action principale éclaircit son rouge plein.

**Test de conformité** : effondrer tous les tons neutres vers le fond et ne garder que `--rosso` et `--rosso-border`. Ce qui reste visible doit être exactement le filet de fenêtre, le logo, le filet de l'entrée active du rail, la carte en session, le bouton de lancement, et les puces actives en rouge éteint. Tout point rouge de plus est une régression.

### 7.2bis Historique de navigation

Les deux **boutons latéraux de la souris** font ce qu'ils font dans un navigateur : revenir à l'écran précédent, y retourner. Un « écran » est la section affichée **plus les fiches empilées dessus** (fiche pleine page, fiche de pack) : c'est l'adresse visible de l'app. Le reste de `nav` n'en fait pas partie — c'est soit une demande transitoire consommée par l'écran destinataire (`openMod`, `search`, `autoLaunch`, `settingsTab`), soit l'état interne d'un écran.

L'historique **observe** cette adresse plutôt que d'être alimenté par ceux qui la changent. Une douzaine d'endroits ouvrent une fiche ou changent de section — double-clic sur une carte, menu contextuel, manette, vue transversale, barre latérale, bloc Session. Leur demander à chacun de pousser aussi une entrée, c'est un historique qui devient faux le jour où un treizième apparaît ; il regarde donc le triplet changer et note ce qu'il voit (`navHistory.ts`, effet dans `AppShell`).

**Le « ← » d'une fiche est ce même retour**, et le bouton B de la manette
aussi : l'historique d'abord, la fermeture de la fiche en repli quand il n'y a
rien derrière (`goBackOr`). Une flèche qui ramène toujours à la liste de l'écran
courant — le travers classique des applications Android — rendait la
bibliothèque après un aller depuis l'inventaire, au lieu de l'inventaire.

**Changer de section ET ouvrir une fiche passe par `openInSection`.**
`requestSection` remet les fiches à zéro, et l'`await` qui la suit garantit que
l'observateur voit passer la liste d'arrivée comme un écran à part entière :
« précédent » y ramenait. Les deux écritures d'affilée n'exposent qu'un seul
état observable.

Trois points de comportement :
- **Naviguer après un retour efface l'avance**, comme dans un navigateur.
- **La garde de §11 s'applique** : reculer depuis les Réglages avec des changements non enregistrés propose de les enregistrer, exactement comme un clic dans la barre latérale. Un refus laisse l'historique où il était.
- **Rien n'est persisté** : une app rouverte le lendemain n'a pas d'écran précédent, pas plus qu'un onglet neuf n'a de bouton retour actif.

WebView2 mappe ces deux boutons sur **son** historique de navigation ; l'app à route unique (SPA `adapter-static`) n'a rien où reculer, et la webview quitterait la page pour une fenêtre blanche. Ils sont donc interceptés (`preventDefault` sur `mousedown` et sur `auxclick`) avant d'être traduits en navigation Pit Box.

### 7.3 Type « Autres mods »

« Autres mods » désigne un **type** de mod, jamais un écran. Celui qui les
montre est l'inventaire des compléments (**§7bis**), où ils voisinent avec les
livrées, les sons, les habillages et les couches ; les apps, elles, ont leur
propre écran (REFONTE§3.2). Tout ce qui suit décrit le type : comment il
s'importe, se pose et se classe.

**L'onglet Pilotes montre le mannequin.** Un mannequin ne se reconnaît qu'à sa géométrie — forme de casque, HANS ou pas, carrure, visage — et sa fiche ne portait que son id de fichier, ce qui ne dit rien de ce qu'on vient d'importer. Elle rend donc le corps en 3D, avec la galerie de l'écran Pilote (SESSION§5) telle quelle, PNG gardé sur disque compris. Deux dépendances qui se disent à l'écran plutôt que d'afficher un cadre vide : le mod doit être **actif** — les mannequins se lisent dans les jonctions posées, AC ne connaissant un corps que par le nom de fichier trouvé dans `content/driver/` — et une **voiture de session** doit être choisie, puisque c'est elle qui pose le mannequin (`prepare_body_preview`).

Mods de type non reconnu (shaders, configs CSP, mods d'UI, weather patterns…) : listés dans « Autres mods », activables/désactivables (hardlinks) comme les autres. Priorité notée + conflits signalés (pas de moteur de superposition type MO2). Chaque entrée a un bouton **« ouvrir le dossier »** vers son emplacement en bibliothèque — le chemin est résolu côté Rust depuis l'overlay, jamais reçu du front, ce qui permet de garder fermé le scope ACL du plugin `opener` (même rationale que `open_mod_folder`).

**Chaque entrée a sa fiche**, ouverte en cliquant son nom — comme un mod de son (§8). Elle porte les mêmes actions que la ligne, et surtout le bloc **Ressources** (§4.5.2), qui n'avait nulle part où vivre dans une liste plate : c'est là que se lisent la notice et les images qu'un auteur livre à côté de son mod. Le bloc est celui de la fiche voiture, pas une copie.

**La provenance d'un reste se lit sur deux lignes.** Un reste (voir plus bas) est nommé d'après l'archive **et** l'endroit d'où il en a été tiré — `<archive>__<chemin dedans>` —, parce que c'est cette chaîne qui forme aussi son identifiant. Les deux moitiés sont utiles et ne répondent pas à la même question : la fiche affiche donc « Provenance » (l'archive seule, le même mot que sur une fiche voiture) puis « Emplacement dans l'archive », et cette seconde ligne disparaît quand la livraison était l'archive entière. C'est l'archive seule, également, qui sert de clé au regroupement « par archive » de l'inventaire — sans quoi chaque reste forme un groupe d'une ligne au lieu de rejoindre les contenus arrivés avec lui.

**Un mannequin de pilote s'importe même livré nu.** Beaucoup se téléchargent comme un `.kn5` seul, sans archive ni dossier autour. Un tel fichier — déposé sur l'app ou choisi dans le sélecteur — est reconnu par son **contenu** (maillage skinné *et* nœuds préfixés `DRIVER:`, aucun faux positif sur voiture ou collider), mis en boîte sous `content/driver/` et rangé comme « autre mod », onglet Pilotes. Un `.kn5` qui n'est **pas** un mannequin est refusé avec un message : une carrosserie arrachée à son dossier n'a pas de destination qui ait un sens, et lui en inventer une poserait un fichier inerte dans le jeu. Le fichier d'origine n'est jamais déplacé, même en mode « déplacer » — seule différence assumée avec l'import d'un dossier, parce qu'un fichier isolé posé sur l'app n'a souvent pas d'autre exemplaire. Un mannequin trouvé **dans** une archive ou un dossier suit le même chemin sans être détecté par le contenu : son chemin le dit déjà.

**Classés par zone du jeu.** « Autres mods » est un fourre-tout par construction — un shader, une police et un pack de drapeaux n'ont rien à faire dans la même liste. Chaque entrée est donc classée d'après **les chemins de ses fichiers**, en dix zones. Ce classement ne dessine plus des onglets : il alimente la **nature** de l'inventaire (§7bis) — comportement, apparence, dépendance — et c'est la même donnée qui sert aux deux. Les zones : Extensions (`extension/`), Météo (`content/weather/`), Interface (`content/gui/`), Pilotes (`content/driver/`), Textures (`content/texture/` et `extension/textures/`), Objets 3D (`content/objects3D/`), Polices (`content/fonts/`), Filtres PP (`system/cfg/ppfilters/`), Showrooms (`content/showroom/`), le `extension/` **d'une voiture ou d'un circuit** (`content/cars/<id>/extension/`, une config CSP qui ne vaut que pour ce contenu — typiquement celle qui rattrape le volume d'un mod de son) et Autres — ce dernier ramassant ce qu'aucune règle ne reconnaît, y compris une entrée sans aucun fichier stocké (livraison partie entièrement en ressources, §4.5.2). Quatre points :

- **Rien n'est stocké.** La classification est recalculée à chaque lecture, à partir du parcours de fichiers que `list_others` fait déjà pour détecter les conflits. Une entrée mal rangée par une version antérieure se répare donc toute seule, ce qui est la seule voie possible : un mod « autre » ne connaît pas la mise à jour (voir plus bas).
- **La règle la plus précise gagne**, fichier par fichier : `extension/textures/` est une texture, pas une config CSP, alors que les deux préfixes correspondent.
- **Un mod est listé sous chacune des zones qu'il touche**, pas sous une zone dominante. Un pack qui livre des polices *et* un habillage d'interface se cherche des deux côtés, et le classer sous sa moitié la plus grosse perdrait l'autre. Chaque ligne porte donc la liste de ses zones : c'est ce qui permet de le reconnaître d'une facette à l'autre.

**Le signalement « zone Content Manager » vaut ici aussi** (§4.5.3) : le décompte des fichiers de l'entrée qui visent un dossier auto-synchronisé est affiché sur sa ligne, avec la même explication au survol que dans « Ajouts au jeu ». Ce n'est pas un doublon décoratif — c'est précisément ici qu'atterrissent les configs CSP d'un **pack multi-mods**, puisque rien ne les rattache à une voiture en particulier (voir le rattachement ci-dessous). N'avertir que dans « Ajouts au jeu » aurait laissé muet le cas le plus probable.

**Pas de notion de mise à jour.** Réimporter une archive dont l'id existe déjà en bibliothèque ne fait rien — ni remplacement, ni erreur, silencieusement ignoré. Pour reprendre un mod « autre » modifié, il faut d'abord le supprimer. *Conséquence à garder en tête* : une entrée mal rangée par une version antérieure de l'app ne se répare pas en réimportant, et l'utilisateur qui tente ce réflexe ne voit rien changer. Les corrections de rangement doivent donc s'appliquer **à la lecture** (activation, décompte des chemins) et pas seulement à l'import — c'est pourquoi la traversée de l'emballage (§4.5.3) est faite des deux côtés.

**Fichier isolé dans un dossier déjà réel côté AC** : posé par lien fichier (`mklink`, même mécanisme que les junctions de dossier) — ex. une nouvelle image dans `content/gui/flags/`, dossier qui existe déjà dans une install AC standard. **Un fichier déjà présent à cet emplacement est remplacé, plus sauté en silence** : l'original part en sauvegarde et revient à la désactivation (§4.5.4), et seul un exemplaire plus récent prend la place de ce qui tourne déjà. C'est ce qui manquait aux mods qui remplacent réellement du contenu — un mod façon CMRT visant `content/gui/` s'installait à moitié sans que rien ne l'indique.

**Rien n'est perdu même à côté d'un mod reconnu.** Un import n'est plus tout-ou-rien : si une archive contient une app (ou une voiture/circuit/skin/son) ET, à côté, du contenu non reconnu — cas des mods type CMRT qui livrent un dossier `apps/` et un zip séparé visant `content/gui/...` — ce reste est repéré et importé comme son propre « autre mod », plutôt que jeté au nettoyage du dossier temporaire. Un zip/7z/rar trouvé dans ce reste est extrait et reclassé récursivement (profondeur 2) avant de retomber, lui aussi, sur voiture/circuit/skin/son/app/autre mod si rien n'est reconnu dedans.

**Un reste = un id, dérivé de son chemin relatif à la racine balayée.** Deux invariants, chacun à l'origine d'une perte de données réelle (archive RSS GT-M Lanzo, qui livre une voiture plus `extension/`, `system/`, `content/texture` et `content/driver`) :

- **Le chemin relatif est conservé jusqu'au déploiement.** Un reste est stocké sous `others/<id>/<chemin relatif>`, et l'activation rejoue ce chemin depuis la racine d'AC. `content/driver` va donc bien à `AC\content\driver` — réduit à son seul nom de dossier, il atterrissait à `AC\driver`, hors de portée du jeu.
- **L'id porte ce chemin, et seules les extensions d'archive (`.zip`/`.7z`/`.rar`) en sont retirées.** Sinon tous les restes d'une même archive partagent un id : le premier est importé, les suivants rejetés comme déjà connus (§7.3, pas de mise à jour d'un mod « autre ») et leurs fichiers disparaissent au nettoyage du dossier temporaire. Un reste écarté pour id déjà connu est journalisé (`log::warn!`) — sans cette trace, la collision ne laissait aucun indice.

### 7.4 Vues et interactions

**La carte de la grille se détache de la page.** Fond au-dessus de celui de la grille, bordure enfin visible, élévation (liseré clair en haut, ombre portée), et l'image posée **en retrait sur un mat neutre** plutôt qu'à ras du bord. C'est la réponse au « deux voitures sombres sont indiscernables » — mais seulement à sa moitié *contenant* : les `preview.png` sont opaques et portent leur propre fond, cuit dans l'image, donc **aucun réglage de la carte ne peut agir derrière la voiture**. Ce traitement n'attend rien pour autant : certaines voitures sont chiffrées et ne seront jamais régénérables, la grille restera donc mixte indéfiniment, et le mat est ce qui **confine la disparité à l'intérieur de l'image** au lieu de la laisser contaminer la carte entière. Il absorbe aussi la preview absente ou de format inattendu : l'image est en `contain`, ce qui manque montre le mat plutôt qu'un trou noir.

**Les images ne sont jamais retouchées** — ni filtre, ni correction de luminosité, ni masque. Une preview soignée par son auteur ne doit pas être dégradée pour compenser celles qui ne le sont pas : le gain vient du contenant, ou de la source.

**La source, justement : les vignettes régénérées** (`SPEC-grille.md` §5). ⚠️ **Livrées derrière un interrupteur de fonctionnalité, aujourd'hui à `false`** (`FEATURE_GRID_THUMBS`, `src/lib/features.ts`) : rien de ce qui suit n'est visible ni actif dans une version construite en l'état — ni l'onglet de réglages, ni la question de l'assistant, ni la génération elle-même, et les cartes gardent la `preview.png` du mod. Le mécanisme est complet ; ce qui manque est le réglage des valeurs par défaut des trois presets. La description ci-dessous est donc celle du jour où l'interrupteur repasse à `true`. L'app convertit et rend déjà les voitures en 3D pour la fiche ; le même moteur produit les vignettes de la grille — **même cadrage, même éclairage, même fond, pour toute la bibliothèque**. Le gain ne se limite pas au confort : une fois les fonds homogènes, l'œil compare les *formes*, ce qu'il ne peut pas faire quand il se réadapte à chaque carte. Cinq propriétés en font le tour, et chacune est une décision prise contre une solution plus évidente :

- **Au fil de l'eau, jamais en masse.** Une carte demande sa vignette quand elle entre dans le champ de vision (`IntersectionObserver` dans `Library.svelte`), pas au chargement de la liste : la bibliothèque se normalise pendant qu'on l'utilise, sans attente initiale. Ce qui devient visible passe devant le reste de la file, si bien que changer de filtre la réordonne tout seul.
- **Le pipeline ne passe pas par le cache LRU des aperçus.** Le réutiliser le remplirait de 312 voitures que personne n'ouvrira, en évinçant celles qu'on consulte vraiment : le cache se mettrait à travailler contre son utilisateur. La conversion écrit donc dans un **brouillon** (`previews/scratch/`, `preview::prepare_scratch`) vidé avant chaque conversion et jeté aussitôt l'image rendue — pic disque d'une voiture à la fois. Ni l'éviction ni la mesure d'occupation ne le regardent, et le protocole `carpreview` le sert sous exactement la même validation de nom.
- **Le PNG est transparent, et c'est la carte qui fournit le fond.** Une vignette détourée ne résout rien à elle seule : posée sur le fond de carte, une carrosserie noire devient *moins* lisible qu'avant. C'est le mat du §7.4 ci-dessus qui la porte — en CSS, donc suivant le thème, la densité et les états de carte sans jamais régénérer une image, et identique pour les 312 par construction. Le rendu ajoute une ombre de contact, sans quoi la voiture flotte.
- **Rig fixe, exposition fixe, aucune auto-exposition par voiture.** Elle ramènerait une voiture noire et une voiture blanche au même gris moyen — c'est-à-dire qu'elle effacerait exactement la différence qu'on cherche à montrer. Le contre-jour est le paramètre le plus utile de tous : c'est lui qui détache la silhouette d'une carrosserie sombre, plus que n'importe quel réglage de fond.
- **L'identité d'une vignette est le nom d'entrée de cache de la voiture plus l'empreinte du gabarit** (`gridthumbs.rs`). Le premier porte déjà le `.kn5` et sa date, la livrée, les `ext_config.ini` et la version du convertisseur : un mod mis à jour se régénère donc tout seul, sans invalidation à écrire. Le magasin vit hors du plafond du cache (`app_cache_dir()/gridthumbs/`) — une image de deux cents kilo-octets n'a rien à faire dans une compétition d'éviction entre modèles de vingt mégaoctets.

**La génération est un travail, donc elle a une tâche de fond** (§8, `GridThumbToast`). Trois cents voitures à environ une seconde, c'est cinq minutes : un toast est éphémère par définition, celui-ci ne se ferme pas seul, survit à la navigation, se réduit au lieu de disparaître et porte une annulation. Trois règles s'y appliquent, chacune contre un réflexe : le temps restant n'apparaît **qu'après une dizaine de voitures** — avant, l'estimation est fantaisiste et détruit la confiance dans toutes les suivantes ; annuler **conserve ce qui est fait et le dit**, sans quoi on se retrouve avec une grille mixte sans savoir qu'on peut reprendre ; et les cartes se remplissent **une par une**, sans recharger la grille — un rafraîchissement complet qui perd la position de défilement est une agression. *Écart assumé au §8.2* : réduite, la tâche est une barre d'une ligne et non une pastille circulaire à anneau — la pile bas-droite n'a qu'une forme (§4.2bis), et le nom de la voiture, lui, disparaît bien de l'état réduit comme la spec le demande.

**Il n'y a pas un gabarit mais des presets** (`Réglages › Vignettes`). Ce n'est pas une commodité : la preview d'origine d'Assetto Corsa est plus *jolie* que notre rendu, plus vitrine, alors que le nôtre est plus *lisible*. Ce ne sont pas deux qualités d'exécution du même objectif, ce sont **deux objectifs** — identifier vite, ou avoir envie de regarder — et un compromis unique les aurait mal servis tous les deux. Rien n'est perdu de la propriété non négociable du GRILLE§5.6 (« toute valeur identique pour les 312 ») : elle porte sur un jeu d'images, pas sur le nombre de jeux possibles, et chaque preset reste uniforme chez lui. Quatre décisions le structurent :

- **Trois embarqués, en lecture seule, qu'on duplique.** *Catalogue*, *Vitrine* et *Officiel*. Un preset vide serait une douzaine de curseurs de rien ; une copie de Vitrine est à un réglage d'être la sienne. Ça remplace aussi le bouton « rétablir le gabarit d'origine » — l'original est toujours là, juste à côté, intact — et ça **supprime tout le versionnage du GRILLE§5.7** : plus besoin de deviner « l'utilisateur a-t-il personnalisé ? » pour savoir s'il hérite du nouveau défaut, puisque les embarqués évoluent avec l'app et que les copies ne bougent jamais.
- **Le mat fait partie du preset.** La moitié de ce qui rend une preview d'Assetto Corsa belle est son **fond**, cuit dans l'image ; le nôtre est du CSS, donc c'est là qu'il se règle. Un Vitrine à l'éclairage dramatique posé sur le mat clair du Catalogue ne donnerait que la moitié de l'effet. Effet de bord heureux : un mat sombre ressemble davantage aux `preview.png` d'origine, donc le preset Vitrine rend la grille mixte **plus** cohérente, pas moins.
- **Le mat n'entre pas dans l'empreinte** d'une vignette, parce qu'il ne change aucun pixel du PNG. Changer une couleur de carte est instantané et ne régénère rien ; deux presets aux gabarits identiques et aux mats différents partagent leurs images.
- **Le sol appartient au preset, et il décide de la transparence de l'image.** Un preset de catalogue n'a pas de sol : sa vignette ne porte que la voiture, détourée, et la carte fournit tout le fond. Un preset de vitrine allume une **flaque de lumière** et un **reflet** — et un reflet est une modulation de la luminosité d'un sol, donc il n'a rien à moduler sur du transparent : cette vignette-là cuit une part de son fond dans l'image. La transparence était un moyen (que la carte possède le fond), pas une fin ; un preset qui poursuit un autre but a le droit d'un autre moyen, à la condition que son mat corresponde, sans quoi la flaque se lit comme une soucoupe posée sur la carte. Le GRILLE§5.6 écartait le reflet miroir (« il consommerait la moitié du cadre ») : l'argument valait pour un objectif de lisibilité et pour un reflet pleine hauteur — coupé court, il en prend un quart, et c'est un échange légitime quand l'objectif est le plaisir des yeux. La flaque et le miroir sont ceux de l'aperçu de la fiche (`studioFloor.ts`, `floorMirror.ts`), extraits et non recopiés.
- **Officiel retourne le problème de la grille mixte au lieu de le contenir.** Tout le traitement du mat (§7.4) existe parce que les voitures chiffrées et le contenu de base gardent leur `preview.png` pour toujours : la disparité est confinée dans l'image. Ce preset-là l'efface — il *imite* ce rendu, si bien qu'une voiture régénérée ne se distingue plus de celle d'à côté. Deux conséquences, et elles sont le preset : son **fond est cuit dans l'image** (indiscernable exige que le fond soit dans le fichier, comme il l'est chez Kunos — la transparence était un moyen, pas une fin), et il **ne régénère pas le contenu de base** (les 178 voitures du jeu ont déjà exactement ce rendu, les refaire coûterait trois minutes pour un résultat identique). C'est donc le moins cher des trois alors qu'il est le plus ambitieux. Son critère n'est pas « est-ce beau » mais « est-ce indiscernable » — un critère **mesurable**, d'où la bascule *comparer à l'image d'origine* de l'atelier, qui montre la `preview.png` réelle à côté du rendu, sur le même mat. Piège à ne pas rouvrir : ce fichier est une référence de **cadrage**, jamais de luminosité — il est plus sombre que le rendu du jeu. Le cadrage, justement, a été **mesuré sur les 178 voitures officielles** plutôt que deviné : la voiture n'occupe que **66 % de la largeur** du cadre (contre plus de 90 % au catalogue), elle y est **basse** (33 % de marge en haut, 17 % en bas), et — le résultat le moins attendu — **le fond est plat et quasi noir** (luminance 4 dans les coins comme au-dessus de la voiture) : ce qu'on prend pour un halo derrière elle est en réalité la **flaque au sol devant elle**, seule zone claire du cadre (luminance 15 en bas). Un dégradé radial derrière la voiture aurait donc été une erreur, et c'est exactement ce que l'œil suggérait. **Les deux côtés se mesurent avec le même instrument**, sans quoi la comparaison ne veut rien dire : mesurer le cadrage du jeu et *estimer* le nôtre a donné une marge trois fois trop grande, parce que la marge de Pit Box n'est pas « le pourcentage de cadre laissé vide » — à marge nulle, la voiture n'occupe déjà que ~75 % de la largeur, le cadrage ajustant la boîte englobante **en 3D** projetée, plus grande que la silhouette visible.
- **Le mat n'entre dans l'empreinte que lorsqu'il est cuit.** C'est la seule exception à la règle précédente, et elle en est la lecture honnête : une couleur de carte ne décide de rien tant qu'elle reste en CSS, mais un preset qui la peint dans le PNG en fait une valeur de rendu. Toujours la hacher ferait régénérer 312 images pour un changement de thème ; ne jamais la hacher laisserait un fond cuit qui ne correspond plus à sa carte, exactement le défaut que ce preset existe pour effacer.
- **Un preset par densité de grille** (dense → Catalogue, confortable → Vitrine par défaut). La densité est déjà une déclaration d'intention — dense, c'est « je cherche » ; confortable, c'est « je regarde » — donc y accrocher le style n'ajoute pas un réglage, ça donne un second sens à un contrôle qui le portait déjà. Les deux jeux d'images **coexistent** sur disque (~45 Mo chacun), ce qui rend le rebasculement instantané une fois les deux produits : c'est le balayage, qui garde ce que réclame *tout* preset vivant et n'efface que le reste, qui rend ça vrai. La file étant paresseuse, une densité qu'on n'ouvre jamais ne coûte rien.

Un preset porte enfin **« laisser le contenu de base tel quel »**, propriété du preset et non réglage global : ça n'a de sens que pour un preset qui *imite* les previews d'origine — les 178 voitures Kunos ont déjà ce rendu, les régénérer coûterait trois minutes pour un résultat identique — alors qu'avec un preset qui cherche l'homogénéité, les sauter ruine précisément ce qu'on cherche.

**Cet onglet est séparé de `Réglages › Aperçu` (§6), et ce n'est pas un rangement : les deux écrans règlent une caméra et des lumières, mais tourner l'aperçu d'une fiche n'engage rien alors que toucher à un preset périme les images de toute la grille. C'est ce qui rend acceptable qu'un écran affiche une facture (« Appliquer régénérera 298 vignettes · environ 6 min ») et que l'autre n'avertisse jamais. **Rien ne s'applique avant Appliquer**, donc annuler ne coûte rien, ce qui rend l'expérimentation gratuite. Et **l'aperçu de réglage est une grille de six voitures, pas une voiture** : régler l'angle sur une seule conduit à l'optimiser pour elle et à massacrer les autres — on édite un catalogue, l'aperçu doit être un catalogue. La **voiture de la session** occupe la première case, parce que c'est une voiture que l'utilisateur a choisie et dont il sait donc à quoi elle doit ressembler ; les cinq autres sont prises une par catégorie distincte (le seul axe que la bibliothèque connaisse de la silhouette d'une voiture). Toutes sont chargées une fois et gardées en mémoire : bouger un curseur redessine, ne reconvertit jamais.

**Le choix se pose à l'installation, chiffré sur sa propre bibliothèque** (GRILLE§5.5) : *Léger* (aucun aperçu), *Normal* (aperçu 3D, vignettes d'origine — présélectionné), *Soigné* (vignettes régénérées). Nommés par leur résultat et non par leur moyen, et l'écran dit qu'on peut en changer — cette phrase transforme une décision en préférence. Il vient en **deuxième page** de l'assistant, après l'enregistrement des chemins : c'est seulement là que le contenu de base est indexé, donc que « 312 voitures détectées » veut dire quelque chose.

**La génération rend la machine pendant une session.** Elle se suspend **dès le clic** sur « Démarrer » — et non à l'apparition du process, contrairement à la musique de Big Picture qui, elle, accompagne l'écran de chargement jusqu'à ce que la voiture soit pilotable : trois cents conversions pendant ce chargement rallongeraient précisément ce qu'on attend. Elle reprend à la **fermeture du jeu**, que l'app sait voir sans rien ajouter — le fil qui coupe et reprend la musique surveille déjà `acs.exe` (`music/watch.rs`), et rend désormais compte à un second client. Hors session, la conversion tourne sur un pool borné à la moitié des cœurs : elle sature sinon toutes les unités de calcul par le transcodage parallèle des textures, et l'interface saccade — d'autant plus visible sur une grosse machine.

**Une voiture chiffrée garde sa `preview.png`, et l'échec est mémorisé** à côté de l'image qu'on n'a pas pu produire (§7 de la spec grille). Pas de badge sur la carte : l'utilisateur n'y peut rien, aucune action n'est proposable, et un marqueur d'erreur sur une carte parfaitement utilisable ne ferait que salir la grille. La tentative ne se refait que si le mod change — son empreinte est dans le nom du marqueur. C'est ce cas qui rend le traitement du mat obligatoire pour toujours : la grille restera mixte indéfiniment.

**Les marqueurs d'état ont quitté la grille.** Les cartes portaient trois marquages concurrents — badge `BASE`, badge `NON GÉRÉ`, pastille verte/grise, badge `CASSÉ` — dont un seul était lisible sans apprentissage. **On joue dans la grille, on gère dans le tableau** : un état d'installation ne change rien à la décision « je prends celle-là ce soir », et les filtres le couvrent déjà pour qui veut s'en servir comme critère. Ils vivent désormais dans la vue tableau, la fiche et les filtres. Il ne reste qu'un marqueur, le badge `SESSION` — qui n'est pas un état de mod mais la sélection courante, donc une autre catégorie (§7.2ter, niveau 2).

**L'exception : ce qui empêche de jouer.** Un mod cassé ou désactivé change la décision — sans marquage on choisit la voiture, on lance, et on découvre le problème dans le jeu. La carte entière est donc **éteinte** (`opacity: .42`, nom en gris) : ça se comprend au premier regard, sans légende et sans traduction, là où un badge demanderait un vocabulaire à apprendre dans six langues. La **raison** est portée par l'infobulle de la carte et par la fiche, jamais par la grille. Un mod installé hors Pit Box n'est pas concerné : il n'est pas « actif » au sens du déploiement géré, mais il est bien dans le jeu et se lance parfaitement. Et ces cartes ne sont **jamais masquées** — une voiture qui disparaît produit le « où est passée ma Skyline », bien pire qu'une carte éteinte ; elles restent sélectionnables et consultables, seul le lancement est bloqué, avec l'explication au moment du blocage (SESSION§1).

**Le nom tient sur deux lignes.** C'est la correction principale du §3 de la spec grille : sur une seule ligne, trois Skyline s'affichaient à l'identique — `Nissan Skyline GT-R R3…` — la troncature tombant exactement avant ce qui les distingue (R32, R33, R34). Deux lignes, `Nissan Skyline GT-R R34 V-Spec II Nür` tient en entier. La hauteur des deux lignes est **réservée** (`min-height`) même quand le nom est court : sans ça, cartes à nom court et à nom long n'ont pas la même hauteur et la grille se déchire.

**Trois niveaux de gris sous le nom, pas deux.** Nom, puis marque, puis séparateur et année : les deux lignes ne se distinguaient que par la taille, ce qui obligeait à *lire* pour balayer. Creuser le contraste sépare l'identité de la métadonnée.

**Retrait du préfixe de marque — option, désactivée par défaut.** Quand le nom commence par sa marque, ce préfixe peut être retiré **à l'affichage de la grille seulement**. Le défaut est « conservé » parce que la troncature est déjà réglée par les deux lignes : le retrait n'est plus une réparation mais un gain de densité, et quatorze ans d'habitude désignent ces voitures par leur nom complet. La comparaison est insensible à la casse et aux tirets, avec une courte table d'alias pour les formes qui ne s'en déduisent pas (`Alfa`, `Chevy`, `VW`…) — table tenue courte exprès, un alias trop gourmand amputant une identité au lieu d'une redondance (`AMG` seul ferait de « AMG GT » un « GT »).

**Trois interdits, et c'est ce qui rend le retrait purement cosmétique** (`displayName.ts`) : le **tri** porte toujours sur le nom complet — c'est lui qui garde les Nissan groupées sous N, le tri par nom complet *est* le tri par marque ; l'**index de recherche** contient le nom complet, taper « nissan skyline » doit marcher quand la carte affiche « Skyline GT-R R34 » ; et le **nom stocké n'est jamais modifié** — retrait au rendu, aucune écriture, réversible instantanément. Ils valent identiquement pour la vue tableau, qui affiche de toute façon le nom entier.

Deux vues commutables par bibliothèque… **trois positions** en réalité : grille dense, grille confortable, liste. Les deux grilles ne diffèrent que par la largeur minimale d'une carte. Le choix est persisté **par type** — voitures et circuits ne se regardent pas de la même façon, et c'est déjà la règle du tri et des colonnes ; la spec le voulait global, c'est un écart assumé.

**Un menu d'affichage** s'ouvre par un chevron accolé à la bascule et porte les préférences de présentation de la grille : la densité (redondante avec les icônes, mais nommée) et le retrait de la marque. Elles vivent **là et non dans les réglages globaux** : il faut en voir l'effet pour les juger, et un écran de réglages les rend invisibles.

En vue tableau, colonnes choisies, **réordonnables par glisser-déposer d'en-tête** (colonne
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
et les presets (§8.4/SESSION§1, voir plus bas) — fichier dédié écrit côté Rust,
pas `localStorage`, migration silencieuse depuis l'ancienne clé (visibilité
seule ; ordre et largeurs, fonctionnalités nouvelles, repartent toujours des
défauts lors de cette migration).

**Tous les autres petits réglages d'interface** (filtres et filtres épinglés, tri, vue
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

**Ce qui doit rester en place ne vit pas dans ce qui défile.** La barre de filtres et la liste sont deux frères (`.head` et `.scroll` de `Library.svelte`) : seul le second a un `overflow`. L'inverse a été essayé — tout dans une seule boîte, l'en-tête tenu par `position: sticky` — et il a fini par un bug d'affichage signalé en capture : `sticky` n'épingle que sur l'axe auquel on donne une position. Verticalement il tenait ; **horizontalement l'en-tête restait un bloc ordinaire, large comme la partie VISIBLE et non comme le contenu défilable**, si bien qu'un tableau assez large pour défiler de côté (§6.2, `width: max-content`) le faisait glisser et découvrait une bande de lignes au-dessus de lui. Mesuré sur la structure d'alors : défilé de 150 px vers la droite dans une zone de 411 px, l'en-tête s'arrêtait 150 px trop tôt. Sorti du conteneur, il ne peut plus glisser — et les trois compensations que l'ancien montage exigeait disparaissent avec lui : marges négatives, `top: -18px`, et une hauteur d'en-tête mesurée en JavaScript pour décaler d'autant les en-têtes de tableau collants (qui se contentent désormais de `top: 0`). À retenir pour tout autre écran : un `overflow-x` et un enfant `sticky` dans la même boîte ne cohabitent pas.

**La barre est une coulée unique, plus un bloc calé à droite.** Recherche, bouton `+ Filtre`, puces et « Tout effacer » vivent dans **un seul conteneur qui passe à la ligne** ; le décompte de résultats et la bascule de vue sont dans un bloc à part, aligné en haut (`align-items: flex-start`). Ce dernier détail n'est pas cosmétique : sans lui, le bloc de droite se centrerait sur toute la hauteur de la coulée et **descendrait avec les puces** dès qu'elles passent à la ligne, alors que le décompte doit rester à hauteur de la recherche.

Trois choix de mise en forme portent le reste :

- **Un filet vertical de 1 px entre le bouton et la première puce**, et seulement s'il y a des puces. Il sépare l'outil de son résultat sans boîte ni fond — c'est lui qui empêche la lecture « le bouton fait partie des puces ».
- **La recherche a une largeur fixe (300 px), pas un `flex: 1`.** À s'étirer, elle mangeait toute la ligne et refoulait la première puce au rang suivant : elle produisait donc exactement la deuxième rangée qu'on cherche à éviter.
- **Contrôles 32 px, puces 26 px.** L'écart n'est pas une négligence : un contrôle est une chose sur laquelle on agit, une puce est l'état qui en résulte. Deux natures, deux tailles, sans avoir à l'écrire nulle part.

Le **décompte de résultats** ne porte plus de badge de nombre de filtres actifs : les puces le montrent déjà. Le bouton « Réinitialiser N » a disparu avec ce badge ; c'est **« Tout effacer »**, en fin de coulée, visible seulement quand il y a quelque chose à effacer.

**Filtre sur la description** : champ texte libre « contient », un terme par mot, **ET** entre eux, chacun en simple sous-chaîne — même règle que la recherche du haut. Il porte sur la description **effective** (§5bis.3) : celle saisie par l'utilisateur dès qu'il en a saisi une, celle du `ui_*.json` sinon. L'arbitrage est fait côté Rust (`library::description_for`) et voyage sur la carte, comme le nom effectif — l'écran n'a rien à départager. **Rien n'est copié en base** : la description native reste dans le fichier du mod, relue à la construction de la liste. Pour une voiture, c'est gratuit — `ui_car.json` est déjà ouvert et parcouru pour le poids (§7.4, colonne « Poids »), la même lecture rend les deux champs (`car_specs_for`) ; pour un circuit, c'est une lecture de plus, par le lecteur léger `uijson::read_track_description` (pas le scan d'images de `read_track_detail`). Un mod **sans** description ne remonte jamais quand le champ est rempli. Côté écran, les descriptions sont préparées **une fois par chargement de liste** (index minuscules, balises retirées) et non à chaque frappe : mesuré sur une bibliothèque réelle, 116 descriptions sur 124 contiennent du HTML (`<br>`, `<b>`, `<font color=…>`), au point que filtrer le texte brut faisait matcher « b », « br » ou « color » sur presque tout.

**Suivi d'usage** : distance parcourue par voiture/circuit ; filtre « jamais essayé » (0 km CM **et** jamais lancé via l'app, l'app tenant son propre marqueur fiable).

**Les mods non gérés ont retrouvé un filtre**, mais comme **valeur d'« État »** et non comme contrôle à eux (§7.1). Ils avaient eu leur propre tri-état, retiré parce qu'une case de plus dans une barre déjà dense supposait qu'on les triât régulièrement, alors qu'ils sont un état de transition — ce qu'on en fait, c'est les reprendre en main, pas les consulter. Une valeur dans une liste fermée ne coûte rien de tel : elle ne se voit que quand on ouvre l'éditeur d'État. Une préférence enregistrée par la toute première version, qui posait un tri-état « non géré » à part, reste **ignorée** à la relecture : un filtre actif qu'aucune commande à l'écran ne montre ni ne défait est pire que pas de filtre du tout.

**Fourchette d'année (voitures) : vide par défaut, et vide veut dire « aucune borne »** — c'est aussi l'état où « Tout effacer » les ramène. Une borne absente ne filtre rien et ne fait pas exister la puce ; seule une valeur saisie filtre. L'éditeur propose en plus des **raccourcis de décennie déduits de la bibliothèque** plutôt que d'une liste en dur : une collection qui commence en 1930 se voit proposer 1930, une qui s'arrête en 1999 n'a que faire d'un bouton « 2010 ». Le défaut était auparavant `1950`/année courante, ce qui affichait deux bornes que l'utilisateur n'avait pas demandées et ne pouvait pas effacer : vider le champ le ramenait aussitôt à sa borne. Pire, les deux champs se bornant l'un l'autre, vider « année max » l'écrasait à `1950` et ne laissait plus **rien** remonter. `NumberStepper` porte donc une prop `emptyValue` : la valeur-sentinelle qui s'affiche comme un champ vide, échappe volontairement aux bornes (sinon `min` la ramènerait dans la plage — c'est le bug lui-même), et que « vider le champ » rétablit. Une sentinelle plutôt qu'un `null` : `value` reste un `number` pour tous les autres appelants, qui n'ont aucune raison de devenir nullables. Le champ **resynchronise aussi le DOM après coup** : Svelte ne réécrit l'attribut que si la valeur liée a changé, si bien qu'une saisie hors bornes — ou un deuxième vidage — restait affichée en contradiction avec l'état réel (symptôme rapporté : vider une première fois écrivait `1950`, vider une seconde fois laissait le champ vide alors que le filtre valait toujours `1950`). Un filtre enregistré avant ce changement portait les bornes de la plage comme sentinelle : elles sont relues comme « vide », ce qu'elles ont toujours voulu dire.

**Ni ▲ ni ▼ ne se désactive au prétexte que le champ est vide, et les deux flèches partent du même repère** (`emptyStart` de `NumberStepper`) — le même repère quel que soit le sens, comme taper directement cette valeur. Sans lui, ▲ retombait sur `min` même pour un champ dont le point de départ naturel n'est pas sa borne minimale (« année max » : l'année courante), et ▼ n'avait tout simplement aucune destination définie depuis « vide », d'où sa désactivation forcée. « Année min » part de `1950`, « année max » de l'année courante — dans les deux sens : un appui sur ▲ depuis « année max » vide affiche l'année courante, un second l'année suivante, exactement comme un appui sur ▼ affiche l'année courante puis l'année précédente. **Aucune des deux bornes n'a de plancher ni de plafond réel** — `1950`/l'année courante ne sont que des points de départ (`emptyStart`), jamais des `min`/`max` : des voitures existent bien avant 1950 (retour utilisateur direct — une première version bornait `min` à `1950`, empêchant d'aller plus bas une fois qu'on y était arrivé), et un mod peut légitimement porter une année future (voiture concept, DLC annoncé). Le plafond de « année min » suit seulement, dynamiquement, « année max » quand elle est renseignée (une borne ne doit pas dépasser l'autre), sans repli sur une constante quand elle ne l'est pas. Même correctif pour la fourchette d'année du vivier d'adversaires (`OpponentsBlock.svelte`, SESSION§1) : son plafond à l'année courante grisait ▲ dès qu'on l'atteignait, pour la même raison.

**Colonne « État » et pastille d'état** (`StateBadge.svelte`, partagé entre le tableau de bibliothèque et la fiche détail — c'est la même information, elle doit se lire pareil aux deux endroits). Quatre états, la couleur portant la distinction et le libellé l'état : **vert = actif**, **orange = inactif**, **gris = mod installé hors Pit Box** (§8 — présent dans le jeu, donc chargé, mais l'app ne le gère pas ; gris et non jaune parce que sur une install déjà moddée il y en a des centaines et qu'elles fonctionnent), **bleu = contenu de base Kunos** (toujours présent dans le jeu, il ne s'active ni ne se désactive — d'où une couleur à lui plutôt que le vert des mods qu'on a soi-même déployés, avec la même infobulle que le badge des vignettes), libellé **« De base »** plutôt que « Actif » — c'est vrai techniquement (`c.active` vaut aussi vrai pour lui, d'ailleurs le filtre « Actif » remonte le contenu de base sans qu'on y touche ici) mais ce n'est pas l'information que la pastille doit donner. Le tableau affichait auparavant un tiret pour « inactif » — une absence, là où l'utilisateur cherche un état — et rien n'y distinguait le contenu de base d'un mod actif. **Le tri et le filtre d'état ne changent pas** : ils restent sur `c.active`/`c.is_stock` directement, indépendants de l'affichage. Sur la **fiche détail**, cette pastille est posée à droite de la bande d'onglets (emplacement `trailing` de `Tabs.svelte`, donc alignée sur les onglets par construction) : c'est la première chose qu'on vient y vérifier, et elle n'était lisible qu'en ouvrant le menu ⋮, dont le libellé Activer/Désactiver était le seul indice.

**Persistance du duo de session** (`src-tauri/src/session_state.rs`, `app_config_dir/session.json`) : fichier écrit côté Rust, pas `localStorage` du webview. `localStorage` n'est pas garanti synchrone sur disque côté WebView2 — bug réel constaté : le circuit, typiquement choisi juste avant de fermer l'app, ne survivait presque jamais à un redémarrage, contrairement à la voiture (choisie plus tôt, le temps d'être vidangée sur disque). `std::fs::write` est synchrone : la commande `save_session_picks` ne rend la main qu'une fois réellement écrit. Migration silencieuse au premier démarrage après la mise à jour : si le nouveau fichier n'a rien pour une entité, `nav.svelte.ts` relit une dernière fois l'ancienne clé `localStorage` et la re-persiste aussitôt au nouvel endroit.

**Garde d'activation** (`AppShell.svelte`) : lancer une session avec un mod non activé (jamais junctionné dans `content/`) fait planter Content Manager/AC, qui ne trouve pas le contenu — bug réel signalé. C'est un trou **propre à Pit Box** : la bibliothèque montre les mods désactivés, Content Manager ne montre que ce qui est installé.

L'état d'activation n'est jamais déduit de `SessionPick` (juste id/nom/preview pour l'affichage, persisté tel quel — une donnée d'activation qui y serait figée resterait fausse dès que l'état change ailleurs, ex. désactivé depuis la fiche détail) mais interrogé via `get_mod_detail` à chaque changement de sélection, comme `trackDetail` pour le sélecteur de layout. Icône ⚠ (jaune, `title` natif) sur le nom du slot concerné dans la barre latérale.

**Une ligne au-dessus du bouton, pas un dialogue au clic.** C'était une confirmation posée au moment de cliquer « Démarrer » : trop tard, on est déjà parti mentalement, et elle ne couvrait que le duo. La ligne (`.warnbox`, jaune — une condition réparable d'un clic, pas une erreur, et le rouge de cette colonne appartient au lancement, §7.2ter) couvre **voiture, circuit et adversaires**, se voit avant de cliquer, et porte son remède. Le bouton de lancement reste **verrouillé** tant qu'elle est là : aucune session ne peut partir en échec. `Activer` active les mods concernés, relit leur état et rend la main.

**Les doublons comptent pour un** : trois adversaires sur la même voiture inactive font une seule activation, et la ligne annonce un mod, pas trois. L'activation reste **explicite** — lancer une course ne modifie jamais la bibliothèque dans le dos de l'utilisateur, même si les liens durs rendent l'opération réversible et sans coût disque.

**Le plateau est lu depuis un état global** (`gridMods.svelte.ts`), écrit par l'écran de réglages quand il est monté et semé au démarrage depuis `launch_state.json`. Sans lui, la garde ne connaîtrait le plateau que pendant que cet écran est ouvert — donc laisserait partir, depuis n'importe quel autre écran, exactement la session qu'elle existe pour empêcher.

**Support manette — choix du périphérique** (`src/lib/gamepadDevices.svelte.ts`, `ControllerSetup.svelte`) : **un périphérique ne pilote l'interface que si l'utilisateur l'a désigné, une fois, explicitement ; sans réponse, il ne pilote rien.** `mapping === "standard"` est *déclaré* par le périphérique, pas vérifié : un volant en « mode Xbox » ou derrière un adaptateur XInput s'annonce standard, et le layout Xbox place « haut/bas » sur l'axe 1 — sur un volant, c'est une pédale (bug réel : des éléments d'interface se déplaçaient seuls, volant branché ; effleurer le frein faisait défiler le focus, sans rien à l'écran pour l'expliquer). Défaut fermé, donc : un périphérique muet se diagnostique, un focus qui dérive n'a aucun recours évident. Démarrage, branchement à chaud et première installation sont **le même événement** — un périphérique visible sans décision enregistrée — donc un seul chemin de code, et pas d'étape dans le `SetupWizard` (à la première installation personne n'a encore touché son volant : la liste serait vide, l'étape ressemblerait à un écran cassé). La décision est **par périphérique**, jamais globale.

**Bandeau puis panneau** : rien ne s'ouvre tout seul — un modal ne se justifie que si l'app ne peut pas continuer sans réponse, et ici elle le peut ; on branche d'ailleurs un volant *juste avant* de lancer une session, où une popup arriverait au pire moment. Une notification de la pile bas-droite (`ControllerToast.svelte`, bleu = information) annonce le décompte ; elle est **persistante** — elle ne s'évanouit pas toute seule, qui n'a pas eu le temps de lire garde son chemin vers le panneau — son `✕` vaut « plus tard » et jamais refus, et il attend ~1 s que la rafale de branchements se calme pour que le décompte soit juste (un rig complet énumère six entrées en quelques centaines de ms). Le panneau `ControllerSetup.svelte` s'ouvre **au clic** (notification ou Réglages), pose `nav.inputCapture = "controller"` — même mécanisme que la visionneuse, pas un drapeau parallèle — et reste intégralement opérable souris/clavier : c'est un panneau au sujet d'un périphérique qui ne marche peut-être pas. Une ligne par périphérique (nom, `VID:PID · n axes · n boutons`, badge Reconnu / Manette standard / ⚠ Non reconnu), **sélection unique** — la question est « lequel utiliser », et les lignes non retenues sont marquées répondues, donc le rig complet se règle en un geste. **Aucune sélection par défaut** (personne ne valide par réflexe un panneau qui contient déjà une réponse), et « Fermer » (ne répond rien, la notification revient) n'est pas « Aucun pour l'instant » (clôt le sujet).

**Identité et persistance des décisions** : `Gamepad.index` est un slot réattribué au débranchement — jamais persisté, jamais utilisé comme clé. La clé est `VID:PID` (`deviceKey`), sinon l'`id` brut normalisé ; deux manettes XInput identiques la partagent donc, sans conséquence — la décision porte sur le modèle. Un volant se présentant sur plusieurs entrées `Gamepad` (base + boîtier de boutons), adopter l'une marque ses sœurs — même préfixe constructeur/modèle, `deviceFamily` — répondues et adoptées. Stockage dans `ui_prefs.json` (règle d'or n°6), clé `pitbox.gamepad.devices`, lue dans la boucle `requestAnimationFrame` par `peekUiPref` (jamais l'API asynchrone) ; le coupe-circuit global est séparé (`pitbox.gamepad.enabled`) pour que le couper n'efface pas les décisions. **Migration** de `pitbox.gamepadNav.mode`, relu une dernière fois puis retiré : `off` → coupe-circuit à `false`, un `id` forcé → ce périphérique adopté sans rien demander, `auto` → aucune décision (la notification apparaîtra, un clic pour les utilisateurs de manette existants).

**Résolution du profil** (`resolveProfile` dans `gamepadNav.ts`), dans l'ordre : profil calibré sur cette machine (gagne toujours) → profil livré (`DEVICE_OVERRIDES`) → layout standard si le périphérique se déclare `mapping === "standard"` → rien, périphérique inerte. Le layout standard reste lu tel quel plutôt que traduit en `NavProfile` : une direction y a deux sources (croix **et** stick gauche), qu'un `Binding` unique par direction ne sait pas représenter — le traduire ferait perdre le stick sur toutes les manettes normales.

**Retour au neutre exigé** (`armDevice` dans `gamepadNav.ts`) : un périphérique adopté ne produit son premier événement qu'après avoir été **vu au repos**, à l'adoption comme à chaque reconnexion (un slot libéré ne lègue ni son armement ni son dernier front). C'est le correctif du bug ci-dessus — le consentement explicite répond à une autre question, les deux sont complémentaires. Le repos se **mesure**, jamais ne se suppose : un hat DirectInput normalisé par Chromium repose *hors* de [-1, 1] (~3,2 constaté), les pédales à -1, un volant là où on l'a laissé. On attend donc 500 ms sans changement, on prend cet instantané comme référence (sauf profil calibré, qui porte le sien), et on n'arme que si rien de ce que le profil écoute n'est actif — une pédale maintenue est parfaitement stable, mais le profil sait la reconnaître, et le périphérique reste inerte tant qu'elle l'est plutôt que de faire dériver le focus.

**Profils et overrides** (`gamepadProfile.ts`, `DEVICE_OVERRIDES` dans `gamepadNav.ts`) : une liaison est un bouton ou une position d'axe — hat, stick et boutons se réduisent au même modèle `Binding` (`{kind:"button", index}` ou `{kind:"axis", hint, mode:"equals"|"beyond", value}`). L'index d'un axe n'est pas stable : `hint` n'est qu'un point de départ, une liaison `equals` se reconnaît par valeur sur tous les axes — en écartant tout axe qui *repose* sur la valeur cherchée, sans quoi une pédale au repos à -1 répond à la place d'un hat dont « haut » vaut aussi -1. Les profils livrés adoptent **le format exact que produit la calibration**, sinon chaque contribution reçue demanderait une traduction manuelle, donc une occasion de se tromper. Modèle couvert à ce jour : base Fanatec ClubSport Wheel Base V2.5 (croix rapportée comme un axe à 4 positions discrètes, confirmé fonctionnel).

#### 7.4bis Raccourcis manette

**Raccourcis manette** ( `Action` dans `gamepadProfile.ts`) : cinq boutons au-delà du déplacement du curseur, **tous optionnels** — un profil sans eux reste parfaitement utilisable, un raccourci absent ne fait rien et ne bloque rien. Sur le layout standard ils sont placés là où les interfaces de console les mettent : **gâchettes hautes** LB/RB (boutons 4/5) = onglet précédent/suivant, **gâchettes basses** LT/RT (6/7) = mod précédent/suivant, **Start** (9) = amener le curseur sur « Démarrer la session », **Y** (3) = ouvrir le menu contextuel de l'élément ciblé. Les deux paires sont voisines et ne font pas la même chose : onglets au-dessus, contenu en dessous, c'est cet ordre qui rend le couple mémorisable. Front montant uniquement, jamais de répétition au maintien — changer de mod recharge une fiche entière et reconvertit un modèle 3D, une rafale n'a rien d'un service. Les gâchettes hautes ont un **repli** : quand l'écran affiché n'a pas d'onglets (`cycleTab` répond `false`), elles changent de **zone** — barre latérale, liste, fiche de droite, marquées par `data-gp-region` et prises au niveau le plus interne (la zone de contenu d'`AppShell` en est une pour un écran d'un seul tenant ; la bibliothèque la redécoupe en deux). C'est ce qui manquait à la bibliothèque, seul écran sans onglets : rejoindre les filtres depuis le menu latéral, ou la fiche depuis la liste, demandait de traverser des centaines de cartes à la croix. Le curseur revient dans chaque zone **là où on l'avait laissé**, sinon l'aller-retour coûte le défilement. Le bouton menu synthétise un vrai événement `contextmenu` sur l'élément ciblé plutôt que de passer par un registre par écran : tout ce qui répond déjà à la souris (cartes, lignes, panneau de détail) répond du même coup, sans une ligne de code de sa part. Il est devenu nécessaire le jour où les actions groupées sont passées au clic droit — sans lui, elles auraient été inatteignables au volant. Start **amène le curseur**, il ne lance pas : lancer d'une pression depuis n'importe quel écran, sans avoir vu ce qu'on lance, serait le contraire d'un raccourci utile (la barre latérale étant toujours montée, la cible existe quel que soit l'écran ; elle se repère par l'attribut `data-gp-launch`, pas par sa classe — un nom de classe est du style, il se renomme sans qu'on pense à ce fichier).

**Les panneaux flottants prennent la navigation à eux** (`data-gp-overlay`) : le popover d'un filtre, le menu d'ajout, le menu contextuel. Quand l'un s'ouvre, le curseur y entre ; tant qu'il est ouvert, la croix ne circule **que** dedans (les gâchettes de zone ne répondent plus, le bouton menu non plus) ; Annuler le referme et rend le curseur à l'élément d'où il venait — la puce, la carte. C'est la seule exception au « plus proche voisin géométrique », et elle est nécessaire pour deux raisons distinctes : un panneau flottant n'appartient à aucune zone de mise en page, donc la géométrie y entre et en sort au hasard ; et surtout il est en `position: fixed`, or **`offsetParent` vaut `null` pour tout élément en `fixed`** (spec HTML) — le test de visibilité de la navigation les écartait donc purement et simplement. Le menu contextuel était dans ce cas depuis le début : le bouton menu de la manette l'ouvrait, et rien ne permettait ensuite d'en choisir une ligne, ce qui vidait de son sens le raccourci décrit juste au-dessus. Le repli sur les rectangles de rendu est **réservé à l'intérieur d'un panneau** : l'étendre à toute l'app y ferait entrer d'un coup la barre de titre, les notifications et les modales, avec le bouton « fermer la fenêtre » au passage. La fermeture passe par un **Échap synthétisé** plutôt que par un registre : chaque panneau porte déjà sa propre fermeture sur cette touche, exactement comme le bouton menu synthétise un `contextmenu`. La manette n'entre d'autorité que si elle pilotait déjà (repère visible) — sinon elle volerait le curseur d'une souris qui vient d'ouvrir le panneau — et elle ne le déplace pas quand le panneau a posé le sien (le menu d'ajout démarre dans son champ de recherche, qui est le bon point de départ). Les **modales** (BulkImport, sélection d'adversaire) restent hors périmètre : elles ne piègent toujours pas le focus.


**Tout champ porteur d'une valeur se laisse « entrer »** (`needsEntry` dans `gamepadNav.ts`) : liste déroulante, champ numérique **et curseur**. Tant qu'il n'est pas entré, gauche/droite déplace le curseur comme partout ailleurs ; validé une fois, gauche/droite règle sa valeur, et **annuler** (ou valider à nouveau) en ressort — haut/bas restant la sortie de secours. Les curseurs y ont rejoint les deux autres après un signalement : trois curseurs alignés sur une ligne (dégâts/carburant/pneus) sont un cul-de-sac si gauche/droite règle au lieu de déplacer, on n'atteint ni le voisin ni rien à droite du dernier. L'appui « annuler » qui sort d'un champ **ne fait que ça** cette image-là : sinon il refermerait aussi la fiche pleine page derrière. Le champ entré porte `.gp-editing` (anneau rouge) là où le simple ciblage porte `.gp-focus` (anneau jaune) — même geste, deux effets, donc l'état doit se voir.

**Défilement analogique** (`scrollAmount` dans `gamepadProfile.ts`) : un axe dédié fait défiler le conteneur sous le curseur **sans déplacer le curseur**, à la vitesse de la poussée (réponse quadratique, 1800 px/s à fond, zone morte de 0,25 renormalisée pour que le premier cran utile ne parte pas déjà au quart de la vitesse). Sur le layout standard c'est l'axe 3, la verticale du **stick droit** ; un profil calibré porte le sien, capturé à l'étape « Défilement rapide ». La croix parcourt les éléments un par un et emmène le défilement avec elle : c'est ce qu'il faut pour choisir, beaucoup trop lent pour traverser une bibliothèque de plusieurs centaines de mods. Le conteneur qui défile est trouvé en **remontant depuis l'élément ciblé**, jamais déclaré par écran. Un axe maintenu au branchement retarde l'armement, au même titre qu'un bouton (§ retour au neutre).
**Ce que les raccourcis déclenchent appartient à l'écran, pas à la manette** (`src/lib/shell/screenActions.ts`) : « onglet suivant » n'a de sens que pour l'écran qui possède ses onglets, « mod suivant » que pour la bibliothèque, seule à connaître son tri et ses filtres courants. D'où un petit registre — l'écran s'inscrit à son montage, se retire à son démontage, et le scrutin manette n'a jamais à savoir lequel est ouvert. Une **pile**, pas une variable unique : la fiche pleine page se monte par-dessus la bibliothèque, c'est la plus récente qui répond. `Tabs.svelte` s'y inscrit tout seul, donc tout écran à onglets devient parcourable sans une ligne de code de sa part.

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

## 7bis. Écran Compléments — l'inventaire (REFONTE§4)

**Tout ce qui n'est pas un contenu autonome**, dans une seule liste : livrées,
sons, habillages de circuit, couches, mods « autres », et tout ce que Pit Box
n'a pas su reconnaître. Les quatre contenus autonomes — voitures, circuits,
modèles de pilote, apps — ont leur écran et n'y figurent pas.

Trois écrans le précédaient (Add-ons voiture, Add-ons circuit, Compléments) :
ils classaient par **mécanique d'installation**, c'est-à-dire par la complexité
que l'app existe pour absorber, et un même mod pouvait y figurer deux fois sans
que rien ne le dise.

**Deux axes, indépendants du type** (`attach.rs`) :

- **le rattachement** — une voiture, un circuit, une app, le jeu, autonome —
  déduit par ordre de force décroissante, le **signal voyageant avec la
  réponse** : chemin posé (quasi certain), hôte écrit dans la ligne (certain),
  nom de config formé sur une entité (fort), même archive (conjecture).
  Corrigeable à la main, et seule la correction est stockée ;
- **la nature** — apparence, comportement, dépendance, non reconnu — déduite
  des zones du jeu touchées, la plus conséquente l'emportant.

**Facettes tri-état**, chaque valeur avec son compteur : un clic inclut, un
deuxième exclut. Les décomptes se calculent sur la recherche et non sur le
résultat filtré — un chiffre qui bouge à chaque facette posée ne sert à rien
pour décider de la suivante.

**Le contexte de recherche survit à l'écran.** Champ libre, facettes,
regroupement et tri sont enregistrés (`ui_prefs.json`, règle §6.2) et
restaurés au montage, comme les filtres de bibliothèque. Sans cela, cliquer une
ligne — le geste même auquel la recherche sert — démontait l'inventaire, et
« précédent » le ramenait vierge : on venait de poser trois facettes pour
trouver la ligne qu'on est allé voir, et il fallait les reposer. Le grief ne
visait pas seulement le retour : changer d'écran et revenir faisait la même
chose. Une facette enregistrée qui n'existe plus dans le code est **écartée à
la relecture** — laissée en place, elle filtrerait sur une clé que plus aucune
ligne ne porte, c'est-à-dire un écran vide que rien n'explique.

**Une livrée a sa fiche** (`SkinDetail.svelte`), voiture ou circuit, comme un
son et un mod « autre ». Le titre de la ligne y mène ; le lien de rattachement,
à droite, continue de mener à l'hôte. Les deux mènaient au même endroit, si
bien qu'un des deux gestes était perdu et que rien nulle part ne parlait de la
livrée elle-même. Elle porte ce qu'une ligne ne peut pas porter : ses fichiers,
son poids, sa provenance, son nom déclaré par `ui_skin.json`, sa note — et
**si le jeu la voit**, seul fait de la fiche qui demande une action. Une livrée
stockée mais non projetée (§8.3) est parfaitement normale partout ailleurs
dans l'app et n'existe pas pour le jeu ; la réparation générale la rebranche.
Une **couche** n'a toujours pas de fiche à elle : elle vit sur celle de son
hôte, où le lien mène.

**Le type d'une ligne se dit par ce qu'elle touche.** Livrée, Son, Habillage,
Couche, Mannequin et Document se nomment d'eux-mêmes ; il restait « Mod », qui
est le mot qu'on écrit quand on n'a rien de plus précis à dire. Une ligne issue
d'un mod « autre » affiche donc **les zones du jeu qu'elle touche** (§7.3) —
« Polices », « Interface », « Extensions », plusieurs séparées par un point
médian — et ne retombe sur « Mod » que lorsque aucune n'est reconnue, ce qui
est exactement ce que ce mot veut dire. Ce n'est pas une classification de
plus : c'est la donnée dont la **nature** est déjà tirée, remontée d'un cran, et
c'est le vocabulaire de la fiche, pas un second.

**La ligne est à deux niveaux** (nom lisible, identifiant technique en
dessous), ce qui est la condition pour renommer sans rien perdre, et **aucun
bouton n'y est exposé** : ouvrir, prioriser, désactiver, supprimer passent par
le ⋮. Reste visible ce qui se *lit* — le rattachement (lien vers l'hôte), la
nature, l'étoile de priorité quand elle est posée, le marqueur de note, l'état.

**Les mannequins de pilote y figurent**, comme les livrées — et pour la même
raison : la galerie de l'écran Pilote (SESSION§5) et le sélecteur d'une fiche
voiture servent à **choisir**, l'inventaire à **gérer** (désactiver, supprimer,
ouvrir le dossier, annoter). Deux gestes, deux écrans. Les retirer d'ici, essayé
puis annulé, supprimait le seul endroit d'où on pouvait agir sur eux.

L'écran Pilote annonce de son côté le nombre de `.kn5` qu'il a **écartés**
(illisibles ou sans squelette) : ceux-là n'apparaissent pas dans sa galerie, et
sans ce décompte rien n'expliquait leur absence.

**Groupement par archive ou par hôte.** Le second remplace le regroupement par
voiture des anciennes vues transversales : sans lui, « voir toutes les livrées
de cette voiture » se perdait.

---

## 8. Skins, sons, apps

### 8.1 Indexation du contenu de base

**Base Kunos indexée** en lecture seule (`is_stock`), non désactivable, pour que skins/sons puissent s'attacher à une voiture/circuit de base comme à un mod.

**Le nom d'un circuit multi-layouts s'y calcule comme partout ailleurs** : la racine commune des noms de ses layouts (§5bis.3), pas celui du premier trouvé. Le correctif n'existait que pour la relecture des mods ; l'indexation du contenu de base rendait encore « Highlands Drift » pour un circuit qui s'appelle « Highlands », au gré de l'ordre alphabétique des dossiers. Deux chemins qui nomment la même chose doivent la nommer pareil.

**Quand l'index est (re)construit** — deux déclencheurs, et il a fallu les deux : à l'**enregistrement de la configuration**, dès que le dossier du jeu est désigné ou qu'il change (`stock::needs_reindex`) ; et au **démarrage**, si rien n'est indexé alors qu'un dossier est connu. Le second seul ne suffisait pas : au tout premier lancement, la config n'existe pas encore quand l'app démarre, l'assistant l'écrit après — la bibliothèque restait donc vide jusqu'au lancement **suivant**. Même angle mort en changeant de dossier de jeu depuis les Réglages, où l'index continuait de décrire l'ancienne install. Un bouton « Indexer le contenu de base » reste disponible en Maintenance pour forcer la reconstruction.

### 8.2 Mods installés hors Pit Box — « non géré »

**Deux populations vivent dans `content/`, et les confondre était dangereux.** Pit Box s'installe souvent sur une install **déjà moddée** : les vrais dossiers de `content/cars` et `content/tracks` sont alors, en majorité, des mods posés à la main avant lui. L'indexation les prenait tous pour du contenu de jeu — auteur « Kunos » d'office, libellé « Jeu de base » dans la fiche, et surtout **le chemin de couche ouvert dessus**, qui sauvegarde puis efface le vrai dossier pour le remplacer par un composé (§4.3). L'utilisateur voyait son mod déplacé sans l'avoir demandé.

**Le critère est la table du contenu officiel**, `docs/kunos_content_dates.json` (`kunos_dates::is_official`) — les 178 voitures et 21 circuits du jeu de base et de ses DLC, la même table qui sert déjà aux années et dates de publication. Un id qu'elle ne connaît pas est un mod : drapeau `is_unmanaged`, affiché « Non géré ». Le champ `author` du `ui_*.json` ne pouvait pas jouer ce rôle — il est facultatif, et un mod dérivé d'une voiture Kunos garde l'auteur d'origine — alors qu'aucun moddeur n'appelle son dossier `ks_porsche_911_gt3_rs`. Le contenu d'AC1 ne bouge plus depuis 2019, la table est donc stable ; et une entrée qui y manquerait ne ferait que **déclasser** du contenu officiel en mod non géré, jamais l'inverse — le sens sûr, puisqu'un mod non géré est protégé de toute écriture. Angle mort assumé : un mod qui **écrase un dossier Kunos** garde l'id officiel et reste classé contenu de base.

**`is_unmanaged` affine `is_stock`, il ne s'y substitue pas.** Les deux sont vrais ensemble : un mod non géré vit dans `content/` exactement comme le contenu de jeu, et tout ce que `is_stock` protège ou résout doit continuer de s'appliquer à lui (vignettes et skins lus depuis `content/`, refus d'activation, absence de date d'ajout/MAJ). Ce qui change tient en quatre points : pas d'auteur « Kunos » inventé, pas de libellé « Jeu de base » en provenance, aucune couche possible (§4.3), et un message de refus d'activation qui lui est propre — dire « contenu de base Kunos » à quelqu'un qui regarde son propre mod ne lui apprend rien.

**Pit Box n'adopte pas un mod en place, et c'est délibéré.** La seule façon de faire passer un mod non géré sous gestion est que **l'utilisateur retire lui-même son dossier du jeu, puis importe le mod** — ce que les libellés lui disent (infobulle de la pastille, note du bloc Source de la fiche, messages de refus, note du rapport d'import). Une « prise en charge » automatique était possible sans rien déplacer (hardlinks de `content/` vers la bibliothèque, puis marqueur de déploiement), mais elle faisait écrire l'app dans un dossier qu'elle n'a pas posé, pour un résultat que le réimport donne sans aucun risque ni surface de test. Décision prise avec l'utilisateur : l'app ne touche pas, elle explique.

**La reclassification se fait sur place, à la réindexation.** Les bases écrites avant cette distinction ont ces mods en `is_stock` : le premier réindex bascule le seul drapeau `is_unmanaged`, sans toucher à ce que l'utilisateur a saisi dessus (nom repris à la main, description, tags manuels, favori).

### 8.3 Livrées et sons — des sous-éléments rattachés à une entité

**Skins — sélection, pas activation filesystem.** Un skin est un sous-dossier dans `skins/` ; AC les charge tous. Aucune activation/désactivation. Seules actions : prévisualiser, et désigner le **skin piloté** (étoile) pour le lancement. Import via l'import général (rattachement automatique via le dossier `skins/<voiture>/`). **Miniature `livery.png`** (couleurs/motif du skin seul, convention AC reprise par CM) affichée quand présente : dans la liste déroulante compacte de sélection du skin de session (barre latérale, SESSION§1 — bien plus lisible que la photo de la voiture entière écrasée à 20px) et en médaillon dans le coin supérieur droit de chaque vignette de la grille de skins (fiche détail, §6.3) ; jamais sur la grande photo du skin sélectionné.
- **Vue Skins** : sélection multiple (Ctrl/Alt) pour supprimer plusieurs skins d'un coup. **Regroupement par archive d'origine** (pour supprimer d'un coup tous les skins d'une même archive) ou, au choix, **par voiture**.

**Un skin stocké à part fait deux sauts pour arriver dans le jeu, et le second se perdait.** Le premier est la **projection** : une junction dans le `skins/` de l'entité cible (§8.3). Pour du contenu de base, ce dossier *est* celui du jeu et tout est dit. Pour un **mod géré**, c'est le dossier de **bibliothèque** — et `content/<type>s/<id>` n'en est qu'une copie en hardlinks figée au dernier déploiement (§2). D'où le second saut, le **redéploiement de l'hôte**, désormais fait à la fin de l'import d'un pack, au retrait d'une livrée, et après chaque recomposition de `skins/default/` d'un circuit. Il n'a lieu que pour un mod géré : recomposer du contenu de base reconstruirait l'arbre depuis `stock_base/` et effacerait la junction qu'on vient de poser. Best-effort — ce qui est stocké sans être déployé est rattrapé par la réparation générale (SESSION§3).

Deux bugs réels tenaient là, et le second rendait le premier invisible. Les 20 livrées d'un pack F1 étaient stockées, projetées, listées dans la fiche et rendues dans l'aperçu 3D — tout cela lit la bibliothèque — et n'existaient **nulle part dans le jeu**, sans un mot dans le rapport puisque, du point de vue de la bibliothèque, rien n'avait échoué. Réactiver le mod à la main n'y changeait rien non plus : **une junction n'est ni un fichier ni un dossier** pour `symlink_metadata` (`is_dir()` et `is_file()` tous deux faux), donc le parcours de déploiement la rejetait des deux côtés et la livrée disparaissait du déploiement en silence. Le déploiement **suit** désormais les junctions rencontrées dans la bibliothèque — les seules qui s'y trouvent sont nos propres projections, une archive n'en porte jamais — et `content/` continue de ne contenir aucun point d'analyse.

**Sons** — exclusifs (un seul actif par voiture), vrai remplacement de fichiers (`.bank` + `GUIDs.txt`), original toujours restaurable.

**Sons — écoute sans lancer le jeu.** Chaque ligne du cadre « Son du moteur » de la fiche voiture porte une **clé de contact** qui fait entendre le moteur : comparer deux mods sonores demandait jusqu'ici d'en activer un, démarrer une session, écouter, revenir et recommencer. Exclusif (un seul moteur à la fois), et **l'écoute ne déploie rien** — c'est le bouton radio de la ligne qui active, la clé n'est qu'un lecteur. Les deux gestes sont deux boutons distincts, faute de quoi on installerait un mod en croyant l'auditionner. « Origine » lit la sauvegarde `__original__` dès qu'elle existe, jamais le `sfx/` du jeu, qui contient le mod actif.

**Ce qui joue, c'est le moteur audio du jeu lui-même** (`docs/SPEC-engine-sound-fmod.md`). Pit Box charge les DLL FMOD livrées avec Assetto Corsa — celles que l'utilisateur possède déjà, rien n'est redistribué — et joue l'événement `engine_ext` que le jeu jouerait, avec ses couches mélangées comme en course. La clé s'accompagne alors d'un **curseur de régime**, du ralenti au rupteur : entendre un mod monter en régime sans lancer une session est exactement ce qu'on veut comparer entre deux mods. La plage du curseur vient de la **courbe de puissance de la voiture**, lue en clair dans son `ui_car.json`, et non de l'événement : une F1 monte à 19 500 tr/min là où un utilitaire diesel plafonne à 5 000.

**Le son suit l'aperçu 3D.** Tourner autour de la voiture change ce qu'on entend : l'événement moteur d'AC est spatialisé et calcule lui-même l'angle sous lequel on l'écoute, si bien que le bank fournit déjà la différence de timbre entre le capot et l'échappement — l'app ne fait que dire où se trouve l'oreille. Le plateau qui tourne compte autant que la caméra qu'on fait glisser : c'est l'angle **dans le repère de la voiture** qui est envoyé. Ce qui change est le timbre, pas la direction — la source reste devant, comme pour quelqu'un qui garde les yeux sur la voiture.

**Un bouton « Coups d'accélérateur »** fait jouer le moteur tout seul : quelques secondes de ralenti, puis une rafale de brefs coups de gaz, quelques-uns jusque près du rupteur, en boucle et sans régularité — quelqu'un qui fait écouter sa voiture. Les sommets se tirent au sort par rapport au régime maximal de **cette** voiture, jamais en valeur absolue. Le curseur et ce bouton pilotent le même paramètre : toucher le curseur arrête la démonstration, et pendant qu'elle tourne le curseur s'efface.

Quand ce chemin n'est pas disponible — pas d'installation configurée, DLL absentes, bank refusé — l'écoute retombe **silencieusement** sur le décodeur maison, qui joue une boucle de ralenti lue directement dans le `.bank`. Aucune erreur à l'écran : c'est le seul chemin qui fonctionne sans jeu installé, et il reste par ailleurs ce qui alimente la fiche d'un mod de son. Le curseur, lui, n'apparaît pas : un échantillon figé n'a rien à régler.

**Un mod de son a sa fiche**, ouverte d'un clic sur son nom dans la vue Sons — même raison que pour une app : les listes de fichiers y vivent, pas dans un dépliant au milieu d'une liste. Elle porte la voiture visée, la taille sur disque, l'archive d'origine, la date d'import, la clé d'écoute, les **ressources** (la notice livrée avec le mod), et **ce que le bank contient réellement** : encodage, nombre d'échantillons, fréquence, durée totale, présence ou non de la table des noms. Cette dernière ligne n'est affichable que parce que l'app sait décoder le conteneur. L'**auteur** est un champ **saisi à la main** (colonne `author` de `sub_mods`) : aucun fichier de mod ne le porte, et le lire dans une notice serait une devinette sur du texte libre.

Le format des banks FMOD (conteneur FSB5, codecs PCM16 et FADPCM) est consigné dans `docs/fsb5-format.md`, avec la méthode qui a établi chaque fait et les hypothèses écartées. Deux points structurent le reste : la moitié du corpus n'a **pas de table de noms** (les mods la suppriment), donc le ralenti se trouve par le nom quand il existe et **par la mesure** sinon — autocorrélation sur une fraction de seconde, ce qui écarte portes, klaxon et bruit de vent, puis la fondamentale la plus basse. Cette mesure ne tombe juste que 40 fois sur 91, et **ce n'est pas réparable** : rien dans le signal ne distingue un ralenti extérieur d'un bas régime en lâcher de gaz. C'est la raison d'être du chemin FMOD ci-dessus, qui aboutit lui sur 299 voitures sur 299 — l'heuristique ne sert plus que de repli, et **ne doit pas être retouchée**. Et **Vorbis n'est pas décodé** (4 voitures sur 297), le codec étant nommé dans l'erreur plutôt que de rendre du silence.

### 8.4 Apps

**Apps** — type autonome, vue propre, activables (par défaut dès l'import, comme les mods voiture/circuit et les « autres mods »). Détection Python (`<id>/<id>.py`) et Lua/CSP (`<id>/<id>.lua`) ; activation par junction vers `apps/python/<id>` ou `apps/lua/<id>` selon le langage constaté. Ressources annexes (§4.5.2, ex. manuel PDF fourni avec l'app) listées et ouvrables depuis la vue, comme sur une fiche voiture/circuit.

**Une app a sa fiche**, ouverte d'un clic sur son nom dans la vue. Page pleine, comme celle d'une voiture et pour la même raison (§4.5.5) : les listes de fichiers y vivent, pas dans un dépliant au milieu d'une liste — une app qui pose trente configs CSP ferait déborder la vue. Sans tags ni fiche technique, qu'une app n'a pas : ce qui la décrit tient sur une ligne (nom, convention Python ou Lua/CSP, archive d'origine, date d'import), et le reste de la page est **ce qu'elle met sur le disque** — deux onglets **Ressources** et **Ajouts au jeu**, les mêmes composants que la fiche d'un mod, pas des copies. La liste, elle, ne garde que ce qui se décide sans ouvrir : état, activation, dossier, suppression.

**Une app possède aussi des ajouts au jeu** (§4.5.3). Elle en a pour la même raison qu'une voiture : ce qu'une archive livre **à côté** de son dossier lui appartient — config CSP, textures, fichiers de `cfg/`. Elle est donc un propriétaire de restes au même titre (`extras::OwnerKind::App`), avec toutes les propriétés qui vont avec : arbre `extras/apps/<id>/`, pose fichier par fichier à l'activation, retrait à la désactivation, suppression avec l'app. Sans ça, ces fichiers devenaient des « autres mods » anonymes que plus rien ne reliait à l'app — ils lui survivaient donc, très exactement le défaut que les ajouts au jeu existent pour éviter.

**Pas de notion de mise à jour.** Contrairement aux voitures/circuits (§4.3), réimporter une app dont l'id existe déjà **remplace intégralement** ses fichiers, sans comparaison ni choix — pas de diff, pas d'historique de versions. Une app est un script autonome, elle n'a pas les enjeux de versions d'un mod de contenu.

**Mais une app reçoit des couches** (§4.3), et le même attirail : ordre de priorité, activation par couche, `LayersBlock` repris tel quel en troisième onglet de sa fiche. Parce que des mods de contenu **ajoutent des fichiers dans le dossier d'une app** — cas réels : une voiture RSS livrant son `.lua` de réglages à `RSS_Settings`, un circuit livrant ses caméras à `CamTool_2`. C'est très exactement une couche, et la traiter comme un « ajout au jeu » (§4.5.3) produisait deux défauts : le dossier de l'app était **créé en vrai dossier** quand elle n'était pas installée (bloquant définitivement son installation ultérieure, §8.2), et une fois l'app active, les fichiers étaient écrits **à travers la junction**, donc dans son dossier de bibliothèque, qu'un réimport de l'app effaçait.

**Une couche est rangée sous le type de son hôte** : `<lib>/layers/<type>/<hôte>/<nom>`, aligné sur `extras/` et `resources/`. Les couches rangées **avant** ce segment gardent leur chemin — il est lu en base, jamais recalculé — et continuent donc de fonctionner là où elles sont : pas de migration, pas de risque, au prix d'un arbre mixte le temps que les anciennes disparaissent.

**Une couche s'identifie par son hôte *et* l'espace de noms de celui-ci** (`overlay::LAYER_HOST`). `parent_id` seul suffisait tant que seuls des mods recevaient des couches : voitures et circuits vivent dans la **même** table, dont `id_interne` est la clé primaire, donc deux d'entre eux ne peuvent pas porter le même id. Une app vit dans `apps`, avec sa propre clé — rien n'empêche un circuit et une app de s'appeler pareil. Sans ce filtre, les couches de l'un remonteraient sur l'autre, et `recompose` composerait celles de l'app dans le dossier du circuit. Seule la distinction app / pas-app est nécessaire, et c'est tout ce que le filtre fait.

**Déploiement : junction tant que l'app est nue, composition par hardlinks dès qu'une couche est active.** Exactement la règle du §2 pour les mods, et pour la même raison physique — une junction ne pointe que vers *une* cible, elle ne sait rien fusionner. `compose::recompose` est donc **un seul point d'entrée pour les trois types d'hôte** : l'id est cherché parmi les mods puis parmi les apps, ce qui fait que les actions de couche (activer, réordonner, supprimer) n'ont pas eu à connaître la différence. Corollaire à ne pas oublier : « active » veut dire *junction **ou** arbre composé marqué* (`apps::is_app_active`) — ne tester que la junction faisait passer pour inactive toute app à couche.

**Le langage (`apps/python/` ou `apps/lua/`) se lit toujours sur la base**, jamais sur le composé : il décide de la cible, et une couche apportant un `<id>.lua` à une app Python ferait autrement bouger le dossier de destination alors que la junction est déjà posée ailleurs.

**Une couche peut remplacer le script principal de l'app** (`<id>.py` / `<id>.lua`) — c'est autorisé, mais **signalé**, parce que cela change ce que l'app *est*. Pas de sauvegarde à prévoir pour autant, contrairement à un fichier du jeu de base (§4.5.4) : la base reste intacte dessous, retirer la couche la restitue.

**À l'import, c'est le chemin qui décide** (`apps::app_layer_target`) : un reste dont le chemin de jeu est `apps/<lang>/<AppId>/…` vise l'intérieur d'une app, donc c'est une couche de cette app et non un ajout au jeu du mod qui le livre. Le test passe **avant** la recherche de propriétaire — la cible est écrite dans le chemin, elle ne dépend pas de qui livre le fichier ; le propriétaire ne sert ensuite qu'à **nommer** la couche, ce qui rend « les caméras de tel circuit » identifiable et supprimable à la main.

Deux précautions dans ce classement :

- **Une couche par app, pas une par fichier.** Un circuit qui livre neuf fichiers de caméras à CamTool doit produire une seule couche. Les restes concernés sont donc accumulés puis vidés en fin de balayage, groupés par app, chacun reconstitué dans un dossier temporaire qui rejoue les chemins **relatifs au dossier de l'app** — `data/x.json`, jamais `apps/python/<id>/data/x.json`, qui donnerait `apps/python/<id>/apps/python/<id>/data/x.json` à la composition.
- **Le balayage ramasse les restes en bloc.** Un `apps/` livré à côté d'un circuit arrive comme un seul dossier nommé `apps`, pas comme ses fichiers : tester le seul chemin du reste ne voyait rien (`apps` n'a pas assez de composants pour désigner une app). C'est donc chaque **fichier** sous le reste qui est testé. Et un reste **mixte** — des chemins d'app mêlés à d'autres — n'est pas touché du tout : en consommer la moitié dupliquerait les fichiers en import « copie » et ferait ranger un dossier amputé en « déplacement ».

**Reprise des bibliothèques existantes** (`extras::migrate_app_extras_to_layers`, au démarrage). Les ajouts au jeu rangés avant que les couches d'app n'existent sont convertis. **Idempotente par construction** : plus aucun chemin `apps/<lang>/…` ne subsiste dans l'arbre des ajouts après coup, donc un second démarrage ne trouve rien — pas de drapeau à mémoriser. **Sans risque de perte** : l'arbre des ajouts est la source et n'est touché qu'après un `undeploy` réussi ; un exemplaire posé dans le jeu étant un *hardlink* de celui du magasin, le retirer ne fait que décrémenter le compteur de liens, y compris dans le cas tordu où le chemin de jeu traversait la junction d'une app et pointait donc dans la bibliothèque. Ce qui restait d'ajouts légitimes est reposé ensuite, et seulement si le mod est actif.

### 8.5 Accès transversal : l'inventaire des compléments

**Accès transversal : l'inventaire des compléments** (§7bis). Les écrans
« Add-ons voiture » et « Add-ons circuit » (`Transversal.svelte`) **ont
disparu** : ils classaient par mécanique d'installation, et leurs trois
rubriques — skins, sons, couches — sont devenues trois valeurs de facette dans
une liste unique, qui porte en plus les mods « autres ». Le regroupement **par
hôte** y remplace leur regroupement par voiture : sans lui, « voir toutes les
livrées de cette voiture » se perdait. L'accès par la fiche du mod, lui, n'a
pas bougé.

**Les skins de circuit fournis avec le mod ne sont pas listés dans l'inventaire.** Reconnus sur disque dans `cm_skins/` (§8 ci-dessus), jamais importés séparément, donc sans archive d'origine : ils remplissaient à eux seuls la rubrique « Origine inconnue ». Et rien dans cette vue ne s'applique à eux — ni sélection, ni suppression (seul le mod entier les emporte), ni activation (elle se fait depuis la barre latérale ou la fiche du circuit). Les lister n'apprenait donc rien et noyait ce qui se gère vraiment. Conséquence : la rubrique « Origine inconnue » n'apparaît plus que si un skin réellement importé n'a pas d'archive connue. Les skins **de voiture** fournis avec le mod restent listés : là, parcourir l'ensemble des livrées d'une voiture est un usage légitime.

**Analyse des extensions CSP** : poussée plus loin (détection fine des fonctionnalités CSP d'un mod).

---

## 9. Lancement de session

**Déplacé dans `docs/SPEC-session.md`**, dont le code parle par l'étiquette
`SESSION§`. La section pesait 924 lignes — 38 % de ce fichier — et couvrait
une chaîne entière : choisir la voiture et le circuit, régler le type de
session, composer le plateau, poser les conditions, puis lancer via Content
Manager. Elle méritait son fichier ; ce qui restait ici n'était plus consultable.

Y sont décrits : la bibliothèque comme sélecteur (`SESSION§1`), le pilotage par
preset Quick Drive (`SESSION§2`), l'écran de réglages avec le plateau et les
conditions (`SESSION§3`), l'aperçu 3D (`SESSION§4`) et l'écran Pilote
(`SESSION§5`).

## 10. Maintenance, export, nettoyage

**Export d'archive autonome** : repackager un mod complet avec ses dépendances éparpillées (pilotes 3D, polices). Seule fonction qui justifie de lire le `data.acd` chiffré (extraction acd.bms, isolée dans le module d'export, jamais sur le chemin d'import/activation).

**Nettoyage** : détection assistée des mods cassés (voitures sans `ui/`, circuits sans contenu valide, hardlinks orphelins pointant vers un mod supprimé).

**Activation / désactivation vs désinstallation — deux axes distincts.**
- **Activer / désactiver** répond à « ce mod est-il actuellement déployé dans le jeu ? ». Active un mod sans couche = créer les hardlinks du mod vers `content/` ; désactiver = les supprimer (contenu **intact en bibliothèque**). Contenu à couches = composer/recomposer (§4.3). Quasi instantané, réversible, ne libère pas d'espace. Utile pour alléger le roster que CM scanne, éviter des conflits ponctuels, composer une sélection courante.
- **Supprimer de la bibliothèque** répond à « ce mod doit-il encore occuper de la place sur le disque ? ». Action **distincte**, avec sa propre confirmation — efface les fichiers de la bibliothèque (et désactive au passage s'il était actif). Non réversible sans réimport (sauf si l'archive source a été conservée, voir ci-dessous).
- **Profils** : ensembles nommés activables/désactivables en masse — capture l'état actif des **trois** types activables (mods voiture/circuit avec leur version, Autres mods §7.3, Apps §8). Utile pour resynchroniser une bibliothèque copiée sur une autre machine (ex. réplication via robocopy) : les fichiers voyagent, mais aucune junction/hardlink ne survit à un copiage — capturer un profil avant de migrer, l'appliquer une fois la bibliothèque et `overlay.sqlite` en place sur la nouvelle machine réactive tout en une action. Autres mods et Apps n'ont pas de notion de version (simple actif/inactif), stockés à part côté overlay (`profile_extra_entries`).
- **Garde-fou** : vérifier hardlink/junction vs fichier ou dossier réel avant toute suppression dans `content/`.

**Supprimer une version, pas le mod.** Un mod qui a été mis à jour garde ses versions précédentes en bibliothèque, chacune dans son dossier. La frise de la fiche détail porte donc, sur chaque version **autre que celle en place**, un bouton corbeille. Quatre règles :

- **La version installée n'est jamais supprimable.** C'est elle que `content/` pointe par hardlinks : l'effacer viderait le mod sous les pieds du jeu. Le bouton n'est pas proposé, et le backend refuse quand même (`errors.versionIsActive`) — en activer une autre d'abord est une décision, pas un détail qu'on prend à la place de l'utilisateur.
- **Corbeille Windows d'abord.** Le `trash` de `media.rs` sert ici aussi : il passe par `IFileOperation`/`FOFX_RECYCLEONDELETE`, qui **échoue** au lieu d'effacer en douce quand le recyclage est impossible. Or une version de mod pèse couramment plusieurs Go, donc au-delà du quota de corbeille du volume : c'est le cas normal, pas le cas rare. La suppression définitive est le repli explicite, la confirmation le dit **avant**, et l'issue réelle (recyclée ou effacée) est affichée **après** — ce qui a eu lieu ne se devine pas.
- **L'archive source part avec.** Une version qui a fait conserver son archive (§ ci-dessus) l'emporte : elle appartient à cette version-là et n'a plus rien à réinstaller une fois la version partie.
- **Les profils qui l'épinglaient sont repointés sur la version en place**, jamais vidés. Un profil amputé de son entrée ne contient plus ce mod, donc l'appliquer le **désactiverait** — l'inverse de ce qu'il dit. Le profil perd l'épinglage d'une version précise, pas son intention. Les profils concernés sont nommés dans la confirmation, avant que ce soit irréversible.

**Sauvegarde automatique de démarrage** (`src-tauri/src/backup.rs`, best-effort, silencieuse) : à chaque lancement de l'app, avant toute ouverture de connexion à la base, copie `overlay.sqlite` et les petits fichiers de préférences (`config.json`, `ui_prefs.json`, `library_columns.json`, `session.json`, `launch_state.json`, `saved_sessions.json`, `music.json`, les décisions sur les règles `taxonomy.json` et `rules-overlay.json`, et `tag-rules.json` tant qu'il n'est pas migré) dans `app_config_dir/backups/<horodatage>/`, **plus les sessions enregistrées** — elles ne vivent plus dans `app_config_dir` mais chez Content Manager (SESSION§3.6), et le filet les suit là-bas : les `.cmpreset` du dossier `Pit Box\` sont copiés dans un sous-dossier `saved-sessions/`. Sans ça, la seule chose que l'utilisateur ait composée à la main serait la seule hors du filet. Rotation sur les 7 plus récentes. Filet de sécurité contre une base corrompue ou un fichier de préférences écrasé par erreur — pas un vrai système de restauration point-in-time (pas d'écran dédié pour l'instant) : en cas de pépin, fermer l'app et recopier à la main les fichiers voulus depuis le dossier de sauvegarde le plus récent.

**Conservation de l'archive source** (réglage optionnel, défaut désactivé — cohérent avec l'absence d'historique de versions/couches, §4.3) : si activé, l'archive/dossier source d'un mod est conservée en bibliothèque en plus du contenu extrait. Rend disponible une action **« Réinstaller depuis l'archive source »** sur la fiche du mod (visible seulement si l'archive est conservée) : réextrait l'archive et remplace le contenu de bibliothèque pour ce mod. Utile en cas de corruption, de modification accidentelle, ou pour repartir propre sans retélécharger.

**Ce qui survit volontairement à la suppression d'un mod.** Deux tables sont délibérément absentes du `DELETE` :

- **`usage`** (§6) — marqueur « déjà essayé » et nombre de lancements. Réimporter la même voiture retrouve son historique plutôt que de repartir de zéro. Le **kilométrage** n'a de toute façon jamais été chez nous : il vit dans le journal de sessions de Content Manager, indexé par `CarId`, donc rien de ce que fait Pit Box ne peut le perdre.
- **`sub_mods`** — skins et sons rattachés, dont les fichiers ne sont pas effacés non plus. Réimporter le parent sous le même id les retrouve tels quels, ce qui est précisément le geste d'une réinstallation.

Ce n'est un déchet que si le parent ne revient jamais. Ils sont donc **listés en maintenance** (« Skins et sons sans mod ») et nettoyés **sur décision**, jamais automatiquement. Le nettoyage contourne le garde-fou `removable` : il protège un skin fourni avec un mod vivant, ce qui n'a plus de sens quand le parent a disparu.

**Réparation générale** (écran Maintenance, à la manière du « purge & deploy » des autres gestionnaires de mods). Sa définition tient en une phrase : **recalculer tout ce qui dérive de la bibliothèque**. Rien de tout cela n'exige de connaître les règles des versions précédentes de l'app — `content/` est une fonction pure de la bibliothèque, recalculée à chaque activation, donc un changement de règles de déploiement se rattrape en redéployant, sans rien versionner ni comparer. Deux étapes sûres et rejouables à volonté : (1) recréer les projections (junctions) de skins voiture/circuit manquantes ou cassées — cas typique, une copie de bibliothèque (robocopy, migration) qui ne préserve pas les junctions, leur cible étant un chemin absolu propre à la machine source ; (2) **redéployer les mods actifs**, ce qui refait `content/` selon le mode et les règles du jour, ajouts au jeu compris (§4.5.3) — un mod importé avant leur existence les pose ainsi sans réimport. Un mod que l'utilisateur avait **désactivé n'est jamais réactivé** au passage : ce serait une surprise, pas une réparation. Une case à cocher optionnelle ajoute la seule étape qui touche la bibliothèque elle-même : réinstaller depuis l'archive source conservée tout mod détecté cassé qui en a une ; sans archive conservée il est laissé de côté, visible dans la liste des mods cassés. Les échecs individuels sont listés en détail sous le bouton, pas seulement comptés — chaque ligne identifie le skin/mod concerné et la raison technique brute.

**Elle tourne en fond, et elle se voit.** La commande était synchrone, c'est-à-dire exécutée sur le **thread principal** : sur une install réelle — trois cents mods à redéployer, plusieurs centaines de milliers de hardlinks — la fenêtre entière gelait pendant toute sa durée, plus aucun `invoke` ne répondait, et Windows finissait par la marquer comme ne répondant plus. Elle faisait exactement ce qu'on lui demandait, sans qu'on puisse le savoir. `async` + `spawn_blocking`, donc, comme l'import (§4.2) et les lots (§6.3bis) — et la progression part dans la **pile de notifications** (§4.2bis), pas dans l'écran : une réparation dure des minutes, rien n'oblige à rester devant l'Atelier pendant ce temps. Le compte rendu détaillé reste sur l'écran Maintenance, seul endroit qui sache retrouver le *nom* des mods en échec.

**La barre se compte en octets, pas en items**, même raison qu'à l'import : une livrée de 3 Mo et un circuit de 4 Go valent chacun un pas, et une barre en items sauterait. Tout est donc pesé avant de commencer — d'où une phase de *pesée* annoncée, sans laquelle le premier instant d'une grosse install ressemble déjà à un blocage. Une projection compte pour un poids nominal : une junction se crée en une milliseconde, mais *zéro* serait le mauvais chiffre — une barre immobile pendant toute la première phase se lit comme une réparation figée, ce que cette barre existe précisément pour démentir. Le temps restant s'extrapole du temps passé, lissé, et **muet tant que la mesure ne vaut rien** (sous 2 % du travail, le temps écoulé parle du démarrage, pas de la vitesse).

**Journal fichier** (`tauri-plugin-log`, niveau Warn, `%APPDATA%\com.pitbox.app\logs\pitbox.log`) : seul moyen de diagnostiquer, sur une install packagée sans console, un échec d'opération best-effort qui ne bloque jamais l'UI (activation automatique à l'import, arbitrage de priorité entre « autres mods », etc.). N'enregistre que des échecs réels — jamais un flux d'activité normale.

---

## 11. Configuration et préférences

**Chemins requis** (assistant de première configuration, détection auto si possible) : dossier d'install AC, bibliothèque, exécutable CM, 7-Zip, QuickBMS + script acd.bms (optionnels, export seulement). Détection auto (`detect.rs`) : AC via les bibliothèques Steam, Content Manager dans le dossier AC ou `%LOCALAPPDATA%\AcTools Content Manager`, 7-Zip dans ses emplacements standard ou, à défaut, le `7z.exe` que Content Manager embarque pour son propre usage (`%LOCALAPPDATA%\AcTools Content Manager\Plugins\7Zip\7z.exe` — beaucoup d'utilisateurs CM n'ont jamais installé 7-Zip à part). La bibliothèque n'a pas de détection à proprement parler (rien n'y existe encore au premier lancement) mais une **suggestion** pré-remplie dans le dossier utilisateur (`<home>\PitBox Library`, jamais Documents/Bureau/Images — redirigés vers OneDrive par défaut sur Windows, ce qui tenterait de synchroniser une bibliothèque de plusieurs centaines de Go), éditable comme les autres champs détectés.

**Trois bases/fichiers distincts** : bibliothèque (fichiers), base d'overlay SQLite (métadonnées), fichier de règles (ontologie), plus le fichier de config (chemins + préférences).

**Préférences persistantes** : affichage des tags du fichier mod (masquables), état du panneau de suivi (global), vue bibliothèque + colonnes (par type), presets de session (par type), preset CM graphique/FFB par défaut, décor de l'aperçu 3D natif (SESSION§4), **aperçu 3D intégré affiché ou non sur la fiche voiture** (défaut affiché — SESSION§4), regroupement des skins (archive/voiture), extraction des fichiers annexes (Aucun / Informations seulement / Tout — §4.5.2), **conservation de l'archive source** (défaut désactivé — §10), **mode de déploiement** (hardlink/symlink, défaut hardlink — §2), **zoom du mode Big Picture** (§16, distinct du zoom normal — `None` reprend ce dernier),
**enrichissement Wikipédia** (`wiki_online`, défaut activé — §6.3) et sa **langue de
lecture** (automatique par défaut : la langue de l'app, puis la chaîne de repli).

**Écran Réglages en onglets** (Général / Chemins / Aperçu 3D / Vignettes / Musique / Wikipédia) depuis le mode Big Picture (§16) — Général et Chemins partagent `AppConfig` et sa garde de navigation (§11) ; Aperçu 3D et Musique ont chacun leur propre stockage et **s'appliquent sans bouton Enregistrer** (`ui_prefs.json` pour l'un, `music.json` pour l'autre). L'onglet **Import** n'est plus ici : ses deux préférences vivent au pied de l'écran `Atelier › Importer` (§7.2quater).

**Onglet Wikipédia** (`components/settings/WikiTab.svelte`) : l'interrupteur de
l'enrichissement et son motif (§6.3), la langue de lecture, la **purge du cache**
d'articles, et l'**export des corrections manuelles**. Ce dernier n'exporte que
les liens `manual` — jamais ceux que l'appariement automatique a posés : un
appariement `auto` figé dans un fichier livré deviendrait un `import`, qui prime
sur `auto`, et gèlerait donc l'algorithme d'aujourd'hui par-dessus tout moteur
meilleur à venir. Les seuils d'appariement, eux, ne sont **pas** à l'écran : ce
sont des paramètres de `Prefs` (`wiki_match_*`, `wiki_track_*`) qu'on règle par
la mesure, pas au jugé.

**Onglet Aperçu** (`components/settings/PreviewTab.svelte`) : **il porte son propre aperçu 3D**, en haut, et c'est ce qui justifie que les treize curseurs y soient — on règle en voyant le résultat. La voiture montrée est celle de la session en cours, à défaut la première de la bibliothèque. La fiche voiture, elle, n'en garde qu'un raccourci : son panneau compact ne tenait que cinq curseurs sur treize. Réglages de l'aperçu 3D intégré (SESSION§4), en **deux colonnes assignées** : sous l'aperçu, ce qu'on regarde en même temps que lui — **Rendu**, **Éclairage**, **Sol** ; à sa droite, ce qu'on manipule le plus — **Cadrage**, puis **Cache**, seul bloc qui efface des fichiers pour de bon et donc placé en dernier. Les colonnes sont assignées et non laissées au flux du navigateur : celui-ci répartissait les cartes comme il voulait, et le bloc le plus utilisé tombait où il tombait. *Rendu* : affichage de l'aperçu, **pilote au volant** (Toujours / Au démarrage du moteur / Jamais), **braquage**, qualité (Standard / Élevée) et effet d'entrée du plateau (Aucun / Progressif / Lancé). *Cadrage* : affiché ou non sur les fiches (même réglage que la bascule de la zone héros), zoom, orientation, angle de plongée, hauteur de caméra, **focale** et vitesse du plateau tournant. La focale recalcule la distance pour que la voiture garde sa taille dans le cadre : elle ne change que la perspective, le zoom restant ce qui recadre. *Éclairage* : exposition et intensité des rampes du studio. *Sol* : **reflet de la voiture** (intensité, flou, portée), flaque de lumière et ombre portée. Chaque groupe porte son propre bouton de remise à zéro, qui ne touche qu'à lui. *Cache* : plafond du cache d'aperçus (0,5 à 20 Go, défaut 2 Go), taille réellement occupée, et un bouton qui vide le cache. **Comme l'onglet Général, rien ne s'enregistre tout seul** : les réglages s'appliquent à l'aperçu mais n'atteignent le disque qu'au clic sur Enregistrer, un bouton Annuler revient sur l'enregistré, et quitter l'écran avec des changements en attente demande quoi en faire. Seule exception, la bascule photo/3D de la fiche voiture, qui est un interrupteur d'un clic. Baisser le plafond évince tout de suite, sans attendre la prochaine conversion. La qualité ne touche **que** le rendu : en changer n'invalide aucune entrée de cache. Les curseurs eux-mêmes sont dans `components/detail/Preview3dControls.svelte`, **partagé avec le panneau posé sur la fiche voiture** : on les règle là où on voit le résultat, on les retrouve ici avec leur mode d'emploi. Les valeurs par défaut sont celles mesurées sur les `preview.jpg` de Kunos, pour que la bascule photo/3D ne saute pas à l'œil (trois-quarts avant gauche, vue basse — détail dans `SPEC-preview-3d-kn5.md` §15), et un changement s'applique à une fiche déjà ouverte sans recharger son modèle.

---

## 12. Écran « À propos »

Atteint par la **dernière entrée du rail** (§7.2) : c'est du contenu, pas un état de fenêtre, et il n'a donc rien à faire dans la barre de titre où il vivait sous forme d'icône « ? ». Maquette de référence `maquettes/pitbox-a-propos.html`. Contenu :
- **Identité** : nom, version/build, courte phrase de philosophie (non-destructif).
- **Outils tiers** (Assetto Corsa, Content Manager, QuickBMS) : description, auteur/studio, lien externe, mention **non-affiliation** par outil (Kunos Simulazioni, gro-ove, Luigi Auriemma). Content Manager marqué **requis**, QuickBMS marqué **optionnel** (non embarqué — export seulement, §10).
- **Soutien & communauté** : lien **PayPal** (don libre, pas d'abonnement), profil OverTake, lien vers le **dépôt source** (code ouvert), lien « signaler un bug », journal des versions.
- **Licence** : Pit Box est **open source, sous licence GPL v3** — le code source est public ; toute version dérivée distribuée doit rester elle aussi sous GPL v3 (empêche un fork fermé/revendu sans partage). Bandeau légal à mettre à jour en conséquence (mention GPL v3 au lieu de « tous droits réservés »). Éligible à la signature de code **gratuite** via SignPath Foundation (programme pour projets open source qualifiants) plutôt qu'un certificat OV payant.
- **Bibliothèques open source** utilisées par Pit Box lui-même (Tauri, React, crates Rust, paquets npm) : liste repliable, avec licence de chacune. Nécessaire pour les licences MIT/Apache qui exigent l'attribution — liste générable automatiquement depuis `Cargo.toml`/`package.json`.
- **Bandeau légal** : non-affiliation générale, mention marque déposée (Assetto Corsa = Kunos Simulazioni), mention de la licence **GPL v3** du code de Pit Box.

## 13. Conventions

- **Langues** : le **code** (identifiants, commentaires, tests) est en anglais — l'app est destinée à être publique. Les échanges de travail et cette documentation restent en français. Les chaînes visibles par l'utilisateur ne sont **jamais** en dur : elles passent par l'i18n. Six langues sont livrées — français, anglais, italien, allemand, espagnol et portugais (brésilien) — choisies dans Réglages ou déduites du système. `en` définit l'ensemble des clés et sert de repli : une clé absente d'une traduction s'affiche en anglais, jamais sous sa forme brute, ce qui rend une traduction partielle acceptable. `pt-BR` et `pt-PT` retombent tous deux sur `pt`, la détection ne gardant que le code à deux lettres.
- **Erreurs remontées à l'UI** : ce sont des **clés i18n** (`errors.*`, constantes de `src-tauri/src/errors.rs`), pas des phrases — une phrase codée en dur ne se traduit pas. Le frontend les résout via `errorText()`. Les erreurs purement techniques (E/S, SQLite) restent en texte brut, comme diagnostic.
- **Intégration continue** : `.github/workflows/ci.yml` (types, build, clippy `-D warnings`, tests, empaquetage) sur chaque push ; `release.yml` sur tag `v*`. Signature de l'installateur : voir `windows-code-signing.md`.
- **Thème Rosso Corsa** : #d40000 sur fonds sombres (#08080c/#0d0d12), coins carrés, police mono pour les données, esthétique « pit garage » industrielle, logo « PITBOX » italique.
- **Logos officiels** : dans les maquettes, monogrammes placeholder (on ne reproduit pas les logos de marque officiels) ; l'app réelle lit `ui/badge.png`.
- **Tokens de design** en variables CSS ; Claude Code extrait les tokens et reproduit le look en composants Tauri (ne pas copier le HTML des maquettes inline).
- **La coquille ne défile jamais, et c'est garanti, pas espéré.** `html` et `body` sont en `overflow: hidden` : le document ne défile pas, chaque écran gère son défilement interne. Mais `overflow: hidden` n'interdit que la **molette** — le navigateur, lui, fait défiler ces conteneurs tout seul pour amener dans la fenêtre un élément qui vient de prendre le focus, et un `scrollIntoView` posé n'importe où fait défiler *tous* les ancêtres scrollables, document compris. Comme la molette ne peut plus revenir dessus, le décalage est **définitif** : bande noire sous la fenêtre, barre de titre à moitié sortie, aucun geste pour rattraper, seul un redémarrage efface. Vu à l'usage, et arrivé par deux chemins différents — le sommaire de l'onglet Wikipédia, puis le défilement vers la carte sélectionnée de la bibliothèque. D'où `shellScroll.ts` : `pinShell()` remet d'aplomb après un geste qu'on sait risqué, et `watchShellScroll()` — monté une fois par `AppShell` — écoute le défilement du document et le ramène à zéro, pour les chemins qu'on n'a pas vus venir. Un `scrollTop` non nul sur un document en `overflow: hidden` est par définition un accident : il n'y a rien à préserver. **Corollaire à l'écriture** : on fait défiler le conteneur d'écran (`scrollTop`, en divisant toute mesure de pixels par `zoomFactor()`), jamais `scrollIntoView`.

---

## 14. Références (fichiers du dossier docs/)

**L'index est `docs/README.md`**, et il est le seul — une deuxième liste ici se
mettrait à diverger de la première, ce qu'elle a fait : elle a renvoyé
plusieurs mois durant à `archives.py`, supprimé du dépôt, et présentait comme
« référence de l'écran principal » une maquette antérieure à la refonte de la
navigation. Il dit, pour chaque fichier, ce qu'il contient, à quelle date il a
été écrit, et s'il fait encore autorité.

Un seul fichier de `docs/` est lu par l'application, et à ce titre il est une
donnée et non de la documentation : **`kunos_content_dates.json`** (années et
dates de publication du contenu officiel Kunos, §7.1), embarqué par
`include_str!` depuis `kunos_dates.rs`.

**`docs/default-tag-rules-enriched.json` n'est pas ce que l'app charge** —
c'est `src-tauri/rules/default-tag-rules.json`, le catalogue embarqué, sur
lequel se posent les décisions de l'utilisateur (§5, REGLES§2). Les deux ont divergé :
la copie de `docs/` porte un groupe de règles de plus. Ne pas confondre les
deux, et ne pas éditer celle de `docs/` en croyant changer le comportement.

---

## 15. Points à vérifier

- **Bascule symlinks → hardlinks (§2)** : moteur implémenté et couvert par des tests automatisés (déploiement/composition/repli copie/nettoyage, y compris un scénario circuit type Spa) — confirme la mécanique et l'absence de besoin de droits admin (`CreateHardLinkW`, contrairement à `CreateSymbolicLink`). **Validé en conditions réelles par l'utilisateur** (juillet 2026) : déploiement + composition par couches fonctionnels sur sa bibliothèque réelle.
- **Détection de la stack météo** (Pure/SOL/CSP/vanilla) et correspondance preset → backend.
- **Table Kunos** : valider les noms de dossiers / années contre l'installation réelle (correction triviale ligne par ligne).
- **Module musique (§16)** : implémenté et testé sur les parties pures (courbes de fondu, mélange sans répétition, playlist, config, RMS/index §16.3), mais pas encore validé à l'oreille par un humain — l'app tourne sans dossiers musicaux pré-remplis (pas de pack CC0 embarqué, voir §16). Le premier scan d'un dossier (décodage complet de chaque piste pour le RMS) est bloquant côté thread moteur — "quelques secondes pour 30 pistes" par la spec MUSIQUE§3.4 : à confirmer que ce n'est pas gênant à l'usage (silence de quelques secondes à la première entrée en Big Picture sur un nouveau dossier) avant d'investir dans un scan progressif avec barre de progression.
- **Détection AC_LIVE (§16.2)** et **filet de sécurité plein écran (§16.5)** : le champ `Status` et les offsets utilisés viennent d'une implémentation tierce open source, pas testés avec une vraie session AC en cours de développement (pas d'AC installé sur la machine de dev). À confirmer en conditions réelles : la musique GRID doit continuer pendant le chargement, se couper/baisser exactement quand la voiture devient pilotable, et le plein écran doit couvrir l'écran entier sans laisser la zone de l'ancienne barre des tâches visible.

---

## 16. Mode Big Picture et musique

Bouton dans la barre de titre (icône à côté de l'aide « ? », `TitleBar.svelte`) : bascule la fenêtre en plein écran (`Window.setFullscreen` + repli explicite sur les bornes du moniteur, voir §16.6 — pas de 10-foot UI dédiée, c'est l'interface habituelle, agrandie) et démarre l'ambiance musicale si activée. **La barre de titre custom est masquée en Big Picture** (gagne en hauteur, plus aucun sens une fois plein écran). Seule sortie visible : bouton collant en bas de la barre latérale (`position: sticky`, jamais par-dessus les boutons de navigation même si la fenêtre est basse), ou touche **Échap**. Un **zoom dédié** (`prefs.bigpicture_zoom`, §11) s'applique en plus du zoom normal, pensé pour une lecture à distance manette en main.

### 16.1 Musique — périmètre retenu

Transposition du document `spec-module-musique_2.md` (écrit pour une stack C#/.NET + NAudio) vers Rust/Tauri avec la crate `rodio`. Décidé avec l'utilisateur, périmètre **noyau du module** :

- Moteur audio à deux ambiances (MENU pendant la navigation, GRID sur l'écran de paramétrage de session `race`), crossfade à puissance constante, machine à états MENU/GRID/SESSION (`src-tauri/src/music/engine.rs`).
- Détection du lancement d'Assetto Corsa (`acs.exe`/`AssettoCorsa.exe`, polling 500 ms) **et** de la fin du chargement (mémoire partagée AC, §16.2) pour couper la musique seulement une fois la voiture réellement en piste — l'ambiance GRID continue de jouer pendant tout l'écran de chargement — puis fade-in au retour. **Toujours coupée pendant une session, jamais baissée en fond** (décidé avec l'utilisateur — l'option "duck" a existé puis a été retirée : en course comme en essais, plus de musique de préparation une fois la voiture en piste).
- Sélection de dossier par Parcourir (menu/grid), écoute au clic, fichier de config séparé (`music.json`, versionné, jamais fusionné dans `config.json`).
- Normalisation RMS entre pistes + cache d'index par dossier (MUSIQUE§3.4, `src-tauri/src/music/index.rs`) — voir §16.3.

**Pack par défaut embarqué** (`src-tauri/assets/music/`, décision revue avec l'utilisateur — initialement hors périmètre) : deux pistes sous **Pixabay Content License** (usage libre, redistribution incluse ; crédits dans `assets/music/CREDITS.md` et l'onglet À propos), embarquées via `include_bytes!`. C'est le comportement **par défaut, sans configuration** : `MusicConfig.use_custom_folders` (case « Utiliser mes propres dossiers de musique » dans Réglages > Musique) vaut `false` par défaut, auquel cas les deux ambiances jouent le pack embarqué — déposé dans un dossier dédié entièrement piloté par l'app (`app_config_dir/Music/embedded/{menu,grid}`), **réécrit à chaque démarrage** pour rester synchronisé avec le binaire (une mise à jour de l'app peut changer les pistes). Cocher la case révèle les sélecteurs de dossier menu/grid (repli sur `app_config_dir/Music/{menu,grid}`, vides, tant qu'aucun n'est choisi) — ces dossiers-là restent la propriété de l'utilisateur, jamais réécrits par l'app.

**Écarté pour de bon** (pas seulement reporté) : la détection automatique des bandes-son Steam (liste déroulante « Bandes-son détectées », MUSIQUE§3.2) — décidé avec l'utilisateur, aucun intérêt pour son usage. Le sélecteur de dossier par Parcourir suffit.

### 16.2 Détection de fin de chargement

`acs.exe` reste le même process du début du chargement jusqu'au retour aux stands/résultats — sa seule présence ne dit donc pas si la voiture est pilotable. Plutôt que de scruter des logs (format instable d'une version à l'autre, coût d'I/O disque à chaque scrutation — sensible pendant la course, précisément quand on scrute le plus), `src-tauri/src/music/ac_status.rs` lit la **mémoire partagée officielle d'AC** (`Local\acpmf_graphics`, l'API utilisée par tous les tableaux de bord tiers — SimHub, CrewChief…) : une simple lecture mémoire, de l'ordre de la microseconde, jamais de disque. Le champ `Status` (`AC_STATUS`, un `int32` juste après `PacketId`) vaut `AC_LIVE` (2) uniquement quand la voiture est réellement en piste — `AC_OFF`/`AC_REPLAY`/`AC_PAUSE` le reste du temps, chargement compris.

`watch.rs` scrute la présence du process toutes les 500 ms (inchangé) et, seulement une fois le process détecté, le statut `AC_LIVE` toutes les 1000 ms. Trois signaux distincts envoyés au moteur : `AcProcessStarted`/`AcProcessStopped` (repère d'état pur, aucun effet sur la lecture — sert uniquement à `enter_big_picture` pour rester silencieux si Big Picture s'ouvre pendant qu'AC tourne déjà) et `EnterSession`/`ExitSession` (le fondu réel, déclenché par la transition `AC_LIVE`). `AcProcessStopped` reste aussi un filet de sécurité : si la mémoire partagée n'a pas signalé la sortie de `AC_LIVE` (fermeture brutale d'AC), la fermeture du process force quand même la reprise de la musique.

### 16.3 Normalisation RMS + cache d'index

`src-tauri/src/music/index.rs` : au premier scan d'un dossier (première entrée en Big Picture après avoir pointé vers ce dossier), chaque piste est décodée en entier via `rodio` — pas de bibliothèque audio de plus, le décodage complet est de toute façon nécessaire pour calculer le RMS — pour en tirer une correction de gain vers -18 dBFS (bornée à ±12 dB, MUSIQUE§3.4) et sa durée exacte. Le résultat est mis en cache dans le dossier lui-même (`.pitbox-index.json`), invalidé si le nombre de fichiers ou la date de modification du dossier changent. **Toujours actif, pas de réglage pour le désactiver** (décidé avec l'utilisateur) ; le gain s'applique en plus du fondu/session courant, recalculé à chaque tick plutôt que figé au chargement.

Bénéfice secondaire : la durée exacte obtenue au passage comble l'écart documenté en §16.4 pour le préchargement du crossfade — `engine.rs` la préfère désormais à `Source::total_duration()` (souvent `None` pour un MP3 décodé en direct), donc le vrai recouvrement `crossfade_ms + 500ms` s'applique aussi aux MP3, pas seulement au WAV/FLAC.

Écart assumé vs la spec : le tag ReplayGain n'est pas lu ("si présent, le préférer au calcul", MUSIQUE§3.4) — lecture de tags audio = une dépendance de plus (`lofty`/`id3`) pour une préférence secondaire ; le calcul RMS s'applique donc systématiquement.

### 16.4 Écarts assumés vs la spec d'origine

Documentés en tête de `engine.rs`, résumé ici :
- `rodio`/`cpal` mixent et rééchantillonnent déjà en interne (un `Sink` par piste dans le même `OutputStream`) — pas besoin de rejouer à la main la chaîne `MixingSampleProvider`/`WdlResamplingSampleProvider` de NAudio décrite par la spec.
- Sortie WASAPI **partagée** par défaut (jamais exclusive, qui couperait le son d'AC) — comportement natif de `cpal` sur Windows, rien à configurer.
- Préchargement (MUSIQUE§5.3) : la durée totale d'une piste n'est connue à l'avance que pour certains formats (WAV/FLAC typiquement). Quand elle l'est, le crossfade démarre bien `crossfade_ms + 500ms` avant la fin ; sinon (la plupart des MP3), il démarre quand `Sink::empty()` devient vrai — la piste précédente est alors déjà silencieuse, donc ce qui reste du crossfade se comporte comme un simple fondu d'entrée plutôt qu'un vrai recouvrement.
- Chemins stockés en absolu (`Option<PathBuf>`, cohérent avec `AppConfig`), pas en variables d'environnement non résolues — la portabilité multi-machine visée par la spec avait du sens pour un `%APPDATA%\<AppName>` C#, moins ici où `app_config_dir()` est déjà par-utilisateur.

### 16.5 Interface

Écran Réglages > onglet **Musique** (`components/settings/MusicTab.svelte`) : coupe-circuit, case « Utiliser mes propres dossiers de musique » (décochée par défaut, pack embarqué), sélecteurs de dossier menu/grid affichés seulement si cochée (Parcourir + écoute ▶, nombre de pistes détectées), lecture aléatoire, volume, durée de fondu. Sauvegarde indépendante des trois autres onglets (fichier séparé).

**Ce qui se règle depuis le mode s'applique dans le mode**, sans en ressortir. Deux réglages étaient dans ce cas et ne faisaient rien tant qu'on y était : le **coupe-circuit** de la musique (`enabled`) ne décidait que du sort de la *prochaine* entrée en Big Picture — donc rien du tout, à l'oreille, pour qui le décoche depuis le mode lui-même (bug signalé) ; et le **zoom Big Picture** (§11) ne s'appliquait aussi qu'à l'entrée suivante, alors que c'est le seul endroit d'où on peut le juger. Désormais : `UpdateConfig` compare l'ancien `enabled` au nouveau et lance/coupe l'ambiance sur place (`enter_big_picture`/`exit_big_picture`, qui portent déjà leurs gardes — AC lancé, session en cours, rien qui joue), sans toucher au drapeau « Big Picture ouvert », ce qui permet de rallumer sans sortir ; et tout ce qui applique un zoom en direct passe par `applyZoomFor` (`bigpicture.svelte.ts`), qui choisit **celui des deux réglages qui est à l'écran** — sinon régler le zoom Big Picture depuis Big Picture ne montrerait rien, et régler le zoom normal depuis Big Picture se battrait avec celui du mode.

Écart assumé vs `spec-module-musique_2.md` (§2) : l'option « baisser le volume en fond pendant une session » (mode "duck") a existé puis a été retirée (décidé avec l'utilisateur) — la musique s'arrête désormais systématiquement au démarrage d'une session, aucun réglage pour changer ce comportement. Contrôle d'un lecteur média **externe** (Spotify, foobar2000…) envisagé séparément, pas encore implémenté : voir « Chantiers en cours » de `CLAUDE.md`.

### 16.6 Plein écran — filet de sécurité Windows

Sur une fenêtre sans décorations (`decorations: false`), `Window.setFullscreen(true)` peut ne couvrir que la **zone de travail** (écran moins la barre des tâches) plutôt que l'écran entier — bug constaté (zone en bas de l'écran, là où était la barre des tâches, restée hors fenêtre et visuellement cassée). `bigpicture.svelte.ts` force donc explicitement les bornes du moniteur courant (`currentMonitor()` + `setPosition`/`setSize`) après l'appel à `setFullscreen`, et restaure la taille/position d'avant (mémorisées, pas seulement celles que `setFullscreen(false)` sait annuler tout seul) à la sortie.

**Une fenêtre maximisée ne se restaure pas en lui rendant sa taille** : elle redeviendrait une fenêtre *normale* qui se trouve avoir la taille de l'écran — pas la même chose, et ça se voit tout de suite (bouton d'agrandissement inversé, la barre de titre ne la décolle plus). Bug signalé. L'état maximisé est donc mémorisé à part (`isMaximized()` à l'entrée) et rétabli par `maximize()` à la sortie, **sans** `setSize`/`setPosition` dans ce cas : les deux écraseraient au passage la géométrie que Windows garde pour le retour à l'état restauré.
