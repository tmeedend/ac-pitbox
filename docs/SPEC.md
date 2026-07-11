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
  - **Composition par couches (§4.3)** : peut elle aussi utiliser des hardlinks au lieu de copies réelles pour composer base + couches, sauf pour les fichiers qui diffèrent effectivement entre couches (ceux-là doivent être de vrais fichiers distincts). Encore plus économe.
- **Content Manager conservé** comme moteur + launcher : reproduire son moteur de config serait énorme et fragile. On contourne son UI, pas son moteur.
- **Stack Tauri** : binaire léger, Rust à l'aise avec les opérations filesystem/hardlinks/process, frontend web pour la richesse visuelle.
- **SQLite** : placée dans `app_data_dir` (pas en chemin relatif, pour survivre aux rebuilds).

---

## 3. Modèle de données : overlay non destructif

**Le fichier `ui_car.json` / `ui_track.json` d'un mod n'est JAMAIS modifié.** Règle absolue. Réécrire le travail d'un moddeur casse les signatures d'intégrité et rend les modifications indissociables du mod.

**Deux sources de vérité séparées** :
- La **bibliothèque** = source de vérité des *fichiers* (contenu des mods, lecture seule).
- La **base d'overlay** (SQLite) = source de vérité des *métadonnées produites par l'app* : tags ajoutés/déduits, catégorie, année, specs complémentaires, favori, historique, profils, presets. Indexée sur l'empreinte du mod.

Le fichier du mod est une **entrée** du pipeline (lu), jamais une **sortie** (jamais écrit). Conséquence : désinstaller l'app laisse les mods intacts ; un badge « fichier du mod jamais modifié » rassure l'utilisateur.

**Entités** : Mod (identité stable, indépendante du nom de dossier), Tag (issu de l'ontologie), Profile (ensemble nommé de mods activés), HistoryEntry (événement horodaté), plus les sous-éléments et couches décrits plus bas.

**Historique d'un mod** : trace les événements avec le nom de l'archive/fichier source — « import initial », « mise à jour », « extension ajoutée ». **Ne trace PAS** les activations/désactivations (bruit sans valeur). Pas de compteur de nombre de mises à jour.

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

**Interface d'import** : glisser-déposer disponible partout ; écran d'import dédié pour les options (chaque option expliquée). Un mod importé est **activé par défaut** (déploiement par hardlinks immédiat).

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

**Mécanisme hybride hardlinks / copie** :
- **Junction par défaut** (instantané) pour les ~95 % de mods autonomes sans couche.
- **Composition par copie uniquement** pour les fichiers qui diffèrent réellement entre couches ; hardlinks pour le reste. Retour au hardlink simple dès que la dernière couche est retirée.

**Contrôle** : ordre des couches modifiable + activation/désactivation par couche. Une couche peut se poser sur n'importe quel contenu (base ou mod).

### 4.4 Packs multi-voitures

Chaque voiture d'un pack est une **entité de premier niveau** (activable/tagguable séparément), liée aux autres par une métadonnée `source_pack` (nom d'archive/dossier, connu dès l'import). La fiche affiche un bloc « Source / origine » (pack cliquable, nom d'archive, URL d'origine si présente) et une section « autres voitures du même pack ». Actions : filtrer par pack, désinstaller le pack en lot.

### 4.5 Ressources partagées (fonts, drivers)

Installées **globalement**, non gérées en activation (les désactiver casserait d'autres mods). Nettoyage optionnel des orphelins. Collision par contenu : identique → silencieux ; différent → warning (défaut = écraser). Le checksum anti-triche d'AC porte sur `data.acd` et `surfaces.ini`, pas sur les fonts/drivers.

### 4.6 Fichiers annexes du mod (docs, templates)

Beaucoup de mods embarquent des fichiers **hors contenu de jeu** : PDF de présentation, templates de skin (`.psd`), changelog/readme (`.txt`), images de présentation, archives de templates. AC ne les lit pas — ils ne doivent **jamais** aller dans `content/`.

- **Extraction** : à l'import, les fichiers qui ne sont pas du contenu AC (hors des dossiers voiture/circuit reconnus, extensions non-jeu) sont rangés dans un sous-dossier **ressources** du mod **dans la bibliothèque** (jamais déployés dans `content/`). Le déploiement vers `content/` ne porte que sur le vrai contenu de jeu. Le dossier Assetto reste propre, les annexes ne sont pas perdues.

- **Réglage global** (préférence persistante, §11 — pas de question à chaque import) : **« Extraction des fichiers annexes »**, trois positions :
  - **Aucun** — rien n'est extrait, les annexes restent dans l'archive/source, non copiées en bibliothèque.
  - **Informations seulement** (défaut) — extrait uniquement les fichiers légers d'information : `.txt`, `.pdf`, `.md`, `.doc`/`.docx`, `.rtf`, `.nfo`, `.html`, `.url`, `.lnk`, ainsi que les **images** (`.jpg`/`.png`) à la racine (ambiguës entre capture de présentation et aperçu de skin, mais légères — rangées ici par défaut ; au pire une capture en trop, sans conséquence).
  - **Tout** — ajoute les fichiers lourds : templates d'édition (`.psd`, `.xcf`, `.ai`), archives jointes (`.zip`/`.7z`/`.rar`), sources 3D (`.fbx`, `.blend`, `.3dsmax`), vidéos de présentation.

- **Bloc « Ressources » sur la fiche** : liste le contenu du dossier ressources **lu en direct** (pas une liste mémorisée en base). Conséquences : un fichier déposé **manuellement** dans le dossier ressources apparaît automatiquement ; les **mods déjà installés** n'ont rien à réimporter — le bloc se remplit dès qu'un dossier ressources existe. Un clic sur un fichier l'ouvre avec **l'application par défaut de l'OS** (PDF → lecteur, PSD → éditeur d'image).
- **Bouton « ouvrir le dossier du mod »** sur la fiche : ouvre le dossier du mod dans l'explorateur (distinct du bloc Ressources).

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

---

## 7. Bibliothèque et navigation

### 7.1 Deux bibliothèques distinctes

Voitures et circuits sont **deux bibliothèques séparées**, jamais mélangées : chacune a ses colonnes propres, persistées par type. Trois colonnes de dates : date d'ajout, date de mise à jour, date de publication (= date de modification des fichiers à l'import pour les mods ; pour le contenu de base, champ `release` de `kunos_content_dates.json`).

**Catégories pour les circuits** : les circuits ont aussi des catégories (comme les voitures), pour filtrer et composer.

### 7.2 Barre latérale unifiée

Une **colonne latérale unique** (maquette de référence `pitbox-biblio-session2.html`) :
- **Bloc SESSION en haut** : previews du duo sélectionné (voiture + circuit), chacune cliquable pour ouvrir la bibliothèque correspondante — le bloc Session est le point d'accès aux bibliothèques (pas d'entrées « Voitures »/« Circuits » séparées). Bouton **« Démarrer une session »** qui ouvre l'écran de réglages à droite.
- **ADD-ONS** (titre rouge/mono) en deux colonnes : Skins | Sons, Apps, Autres mods.
- **ATELIER** (même style) : Règles | Importer, Réglages.

### 7.3 Type « Autres mods »

Mods de type non reconnu (shaders, configs CSP, mods d'UI, weather patterns…) : listés dans « Autres mods », activables/désactivables (hardlinks) comme les autres. Priorité notée + conflits signalés (pas de moteur de superposition type MO2).

### 7.4 Vues et interactions

Deux vues commutables par bibliothèque (galerie / tableau), colonnes choisies persistées.

**Sélection** :
- **1 clic** = sélectionne (affiche dans le panneau de droite ET définit comme voiture/circuit de session).
- **Double-clic** = ouvre la fiche détaillée (où l'on choisit le skin piloté).
- **Skin piloté persistant** : mémorisé pour la voiture, affiché sur la vignette, rappelé dans le bloc Session.

**Sélection multiple (Ctrl/Alt)** : ouvre un **panneau en bas, en surimpression** par-dessus la grille (le panneau de droite continue d'afficher le dernier mod cliqué ; le layout de la grille ne bouge pas en largeur). Champs d'édition en masse : tags (ajout/retrait), activation, suppression, favori, catégorie, export. Les champs propres à une voiture (specs, skin piloté) ne sont pas proposés en masse.
- Quand plusieurs **véhicules** sont sélectionnés, deux actions supplémentaires dans ce panneau : **« Définir en tant qu'adversaires »** (vide la liste d'adversaires puis ajoute la sélection) et **« Ajouter en tant qu'adversaires »** (ajoute à la liste existante). Les deux basculent le mode adversaires de la session Course sur **« Libre »** ; si on était sur « même voiture » ou « même catégorie », les adversaires de ces modes sont récupérés dans « Libre » en plus de la sélection.

**Suivi d'usage** : distance parcourue par voiture/circuit ; filtre « jamais essayé » (0 km CM **et** jamais lancé via l'app, l'app tenant son propre marqueur fiable).

**Support manette** : navigation dans l'application à la manette.

---

## 8. Skins, sons, apps

**Base Kunos indexée** en lecture seule (`is_stock`), non désactivable, pour que skins/sons puissent s'attacher à une voiture/circuit de base comme à un mod.

**Skins — sélection, pas activation filesystem.** Un skin est un sous-dossier dans `skins/` ; AC les charge tous. Aucune activation/désactivation. Seules actions : prévisualiser, et désigner le **skin piloté** (étoile) pour le lancement. Import via l'import général (rattachement automatique via le dossier `skins/<voiture>/`).
- **Vue Skins** : sélection multiple (Ctrl/Alt) pour supprimer plusieurs skins d'un coup. **Regroupement par archive d'origine** (pour supprimer d'un coup tous les skins d'une même archive) ou, au choix, **par voiture**.

**Sons** — exclusifs (un seul actif par voiture), vrai remplacement de fichiers (`.bank` + `GUIDs.txt`), original toujours restaurable.

**Apps** — type autonome, vue propre, activables.

**Accès transversal** : vues Skins / Sons / Apps dans la barre latérale, en plus de l'accès par la fiche.

**Analyse des extensions CSP** : poussée plus loin (détection fine des fonctionnalités CSP d'un mod).

---

## 9. Lancement de session

### 9.1 La bibliothèque est le sélecteur

Pas d'écran séparé de sélection : la voiture/le circuit sélectionnés dans la bibliothèque sont ceux de la session. Le **bloc Session** de la barre latérale montre en permanence le duo courant. La page « Démarrer une session » ne contient aucune sélection de voiture/circuit — seulement les réglages + Lancer.

### 9.2 Pilotage par presets CM

L'app ne fixe pas les réglages par écriture de fichiers : elle **pilote des presets CM** (CM est maître de `race.ini`). CM est démarré en service + Steam ouvert, puis la commande est émise.
> Point à vérifier avant de figer le module de lancement : la commande exacte pour activer un preset CM par programmation (protocole `acmanager://` / `Values.data` / CLI).

**Bouton « Ouvrir dans CM »** : lance CM sans argument de session, sélection active, pour les réglages fins (échappatoire power-user).

### 9.3 Écran de réglages

Maquette de référence `pitbox-reglages-session.html`. Pas de rappel du duo en haut (déjà dans la barre latérale) — titre + Lancer. Toutes les options visibles (pas de bloc replié).

**Communs à tous les types de session** :
- **Simulation** : dégâts, conso carburant, usure pneus (actifs quel que soit le type, pas seulement en Course).
- **Météo** et **heure**.

**Course uniquement** :
- **Adversaires — type de plateau** : 4 modes (Même voiture / Même catégorie / Même ère ±5 ans via `year` / Libre). Remplissage auto selon le mode, **liste du plateau visible et ajustable** (chaque IA avec sa force, retirer/ajouter).
- Nombre d'adversaires, tours ou durée, départ (arrêté/lancé), position de départ, cases qualifications / pénalités / évolution du grip, aides (ABS/antipatinage/ligne).

**Hotlap** : ghost car.

**Niveau IA** : fourchette min-max (deux curseurs), le plateau réparti dans la plage.

**Météo** : conditions en **icônes SVG stylisées** (thème, libre de droits) — Beau, Quelques nuages, Couvert, Brouillard, Pluie légère, Pluie, Orage. **Température et vent implicites**, déduits de la condition + heure (+ stack SOL/CSP), affichés non réglés en v1.

**Presets de session par type** : chaque type (Practice/Hotlap/Course) a un preset mémorisé ; toute modif est persistée pour les prochaines sessions du même type.

**Sauvegarde/chargement de session** (surtout pour la liste d'adversaires) : bouton **Charger** ouvre la liste des sessions sauvegardées ; bouton **Sauvegarder** propose de nommer une nouvelle sauvegarde ou d'écraser une session existante (après confirmation).

**Choix layout/skin de circuit** : sur la fiche/bibliothèque circuit, image d'aperçu (`preview.png`) avec le tracé du layout (`outline.png`/`map.png`) par-dessus, infos (longueur, virages, CSP).

### 9.4 Aperçu 3D des voitures

Bouton d'aperçu 3D sur la fiche (lance `acshowroom` pour un rendu du modèle). **Option de réglage** : activer la 3D par défaut sans afficher l'image d'abord — si activée, le bouton « aperçu 3D » disparaît de la fiche (la 3D s'affiche d'emblée). Pendant le démarrage d'`acshowroom`, afficher une **animation de chargement en haut à droite** de l'image en attendant le rendu.

---

## 10. Maintenance, export, nettoyage

**Export d'archive autonome** : repackager un mod complet avec ses dépendances éparpillées (pilotes 3D, polices). Seule fonction qui justifie de lire le `data.acd` chiffré (extraction acd.bms, isolée dans le module d'export, jamais sur le chemin d'import/activation).

**Nettoyage** : détection assistée des mods cassés (voitures sans `ui/`, circuits sans contenu valide, hardlinks orphelins pointant vers un mod supprimé).

**Activation / désactivation vs désinstallation — deux axes distincts.**
- **Activer / désactiver** répond à « ce mod est-il actuellement déployé dans le jeu ? ». Active un mod sans couche = créer les hardlinks du mod vers `content/` ; désactiver = les supprimer (contenu **intact en bibliothèque**). Contenu à couches = composer/recomposer (§4.3). Quasi instantané, réversible, ne libère pas d'espace. Utile pour alléger le roster que CM scanne, éviter des conflits ponctuels, composer une sélection courante.
- **Supprimer de la bibliothèque** répond à « ce mod doit-il encore occuper de la place sur le disque ? ». Action **distincte**, avec sa propre confirmation — efface les fichiers de la bibliothèque (et désactive au passage s'il était actif). Non réversible sans réimport (sauf si l'archive source a été conservée, voir ci-dessous).
- **Profils** : ensembles nommés activables/désactivables en masse.
- **Garde-fou** : vérifier hardlink/junction vs fichier ou dossier réel avant toute suppression dans `content/`.

**Conservation de l'archive source** (réglage optionnel, défaut désactivé — cohérent avec l'absence d'historique de versions/couches, §4.3) : si activé, l'archive/dossier source d'un mod est conservée en bibliothèque en plus du contenu extrait. Rend disponible une action **« Réinstaller depuis l'archive source »** sur la fiche du mod (visible seulement si l'archive est conservée) : réextrait l'archive et remplace le contenu de bibliothèque pour ce mod. Utile en cas de corruption, de modification accidentelle, ou pour repartir propre sans retélécharger.

---

## 11. Configuration et préférences

**Chemins requis** (assistant de première configuration, détection auto si possible) : dossier d'install AC, bibliothèque, exécutable CM, 7-Zip, QuickBMS + script acd.bms (optionnels, export seulement).

**Trois bases/fichiers distincts** : bibliothèque (fichiers), base d'overlay SQLite (métadonnées), fichier de règles (ontologie), plus le fichier de config (chemins + préférences).

**Préférences persistantes** : affichage des tags du fichier mod (masquables), état du panneau de suivi (global), vue bibliothèque + colonnes (par type), presets de session (par type), preset CM graphique/FFB par défaut, aperçu 3D par défaut (§9.4), regroupement des skins (archive/voiture), extraction des fichiers annexes (Aucun / Informations seulement / Tout — §4.6), **conservation de l'archive source** (défaut désactivé — §10).

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

- **Langue de travail** : français.
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

---

## 15. Points à vérifier

- **Bascule junctions → hardlinks (§2)** : tester d'abord sur un cas de circuit réel (ex. Spa) avant de généraliser à tout le déploiement — confirmer que le chargement fonctionne bien sans droits admin avant de retirer le code symlink.

- **Pilotage des presets CM + `acmanager://`** : commande exacte pour activer un preset par programmation (§9.2).
- **Détection de la stack météo** (Pure/SOL/CSP/vanilla) et correspondance preset → backend.
- **Table Kunos** : valider les noms de dossiers / années contre l'installation réelle (correction triviale ligne par ligne).
