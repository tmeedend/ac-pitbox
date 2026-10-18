# Pit Box — exporter et importer une bibliothèque

## Spécification d'implémentation

*Refaire une installation de Pit Box à l'identique sur une autre machine, ou après une réinstallation : les mods (en vitrine), le classement, les sessions, les profils et les préférences, dans un fichier de quelques Mo.*

> **Étiquette de renvoi : `EXPORT§`.** Repose entièrement sur la vitrine (`ESPACE§`) : **un export est une bibliothèque dont tous les mods sont en vitrine**, apps et autres mods compris (ESPACE§5.6).
>
> **Construit, derrière un interrupteur** (`FEATURE_LIBRARY_TRANSFER`, `src/lib/features.ts`) : visible sous `tauri dev`, absent d'un installateur tant qu'il n'a pas été essayé sur de vraies bibliothèques (`CHANTIERS.md`). Code : `src-tauri/src/transfer/`, `ExportDialog`, `ImportDialog`, l'étape de l'assistant.

---

# 1. Objet

Une installation neuve de Pit Box, un fichier `.pitbox` importé, et l'utilisateur retrouve **sa** bibliothèque : les mêmes cartes, les mêmes noms repris, les mêmes tags, familles, marques et pays, les mêmes règles et correctifs, ses notes, ses favoris, ses sessions et grilles enregistrées, ses profils, ses préférences d'affichage.

Ce qui n'y est pas : **le contenu des mods**. Chaque mod arrive en vitrine, et se récupère par les moyens de `ESPACE§7` : registre de Content Manager, site d'origine, archive glissée. Une bibliothèque de 300 Go devient un fichier de quelques Mo, et l'utilisateur ne retélécharge que ce qu'il veut vraiment conduire.

Deux usages, un seul format :

- **Migrer** vers une nouvelle machine, ou repartir d'une installation propre.
- **Sauvegarder** ce qui est irremplaçable dans une bibliothèque (les saisies et décisions de l'utilisateur), là où la sauvegarde de démarrage (`backup.rs`) ne protège que contre une base abîmée sur la même machine.

---

# 2. Règles fondatrices

## R1 — Un export ne contient que des squelettes

Même pour un mod complet, l'export n'emporte que ce que `skeleton::is_kept` garde (`ESPACE§3.1`) : `ui/`, l'image de vitrine réduite, le manifeste. **Jamais un `.kn5`, un `.acd`, une banque de son, une texture.** Ni fichier chiffré ni fichier jouable : un export ne peut pas servir à redistribuer un mod, et il reste petit.

## R2 — Rien de la machine d'origine

Un export décrit une **bibliothèque**, pas une installation. Il ne contient :

- **aucun chemin absolu** : ni dossier du jeu, ni bibliothèque, ni Content Manager, ni 7-Zip, ni fichier de musique locale ;
- **rien qui décrit le dossier du jeu** de la machine d'origine : fichiers posés, réclamations, sauvegardes d'originaux, mods actifs ;
- **rien de ce qui est en cours** : imports en attente d'arbitrage, état de la session en préparation.

**Aucun chemin absolu ne part, à aucune profondeur** : chaque fichier JSON qui part (classement, grilles, presets, `ui_prefs.json`, colonnes, musique, préférences) perd toute chaîne qui est un chemin absolu, y compris dans un JSON stocké en chaîne, et elle devient `null` ; un fichier sans chemin part octet pour octet, un fichier illisible ne part pas. La base est compactée (`VACUUM`) avant d'être emballée : sans quoi ce qu'on en efface resterait lisible dans ses pages libres.

Sur la machine d'arrivée, ces informations se reconstituent d'elles-mêmes : le contenu d'origine est réindexé, et un mod est posé dans le jeu quand l'utilisateur récupère ses fichiers et l'active.

## R3 — Importer sur une installation vide

La v1 n'importe **que dans une installation sans bibliothèque** — base sans mod géré, dossier de bibliothèque vide — **et dont les chemins du jeu et de la bibliothèque sont réglés**. Fusionner deux bibliothèques veut dire arbitrer des conflits d'ids, de règles et de décisions, pour un cas rare ; c'est un autre chantier (§9). Cette règle rend l'import simple et sûr : il n'écrase jamais rien.

## R4 — Le format se lit sans Pit Box, et se migre avec lui

Un zip ordinaire, un manifeste JSON lisible, une base SQLite ouvrable par n'importe quel outil. La base est exportée **dans son schéma**. Pit Box n'a pas de numéro de schéma : ses migrations sont des ajouts de colonnes idempotents (`overlay::migrate`), rejoués à chaque démarrage. C'est ce qui permet d'importer un export plus ancien sans rien de particulier : les migrations le mettent à niveau comme elles le feraient d'une vieille base locale. Dans l'autre sens, rien ne dit ce qu'une colonne inconnue signifie : **un export fait par une version de Pit Box plus récente que celle qui l'importe est refusé**, sur la version de l'application écrite dans le manifeste, avec un message qui dit de mettre Pit Box à jour.

## R5 — Ce que l'utilisateur coche est ce qui part

L'export est découpé en **parties** (§3), cochables. Aucune partie ne se glisse dans une autre : décocher « Préférences » n'emporte aucune préférence, même indirectement.

---

# 3. Les parties

| Partie | Contenu | Défaut |
|---|---|---|
| **Bibliothèque** | les mods et tout ce qu'on sait d'eux : la base (§4), les squelettes (§5) | cochée, **obligatoire** pour Profils |
| **Classement** | règles de tags et surcouche de l'utilisateur, taxonomies (familles, marques, pays, alias, fusions), choix de logos et logos à soi | cochée |
| **Sessions et grilles** | sessions enregistrées (presets `.cmpreset` du dossier Pit Box de Content Manager), grilles d'adversaires enregistrées | cochée |
| **Profils** | ensembles nommés de mods | cochée |
| **Préférences** | préférences de l'application et de l'interface, colonnes de la bibliothèque, réglages musique sans les chemins de fichiers | cochée |

**Profils demande Bibliothèque** : un profil n'est qu'une liste d'ids de mods. La case se grise, avec la raison, si Bibliothèque est décochée.

**Sessions et grilles ne demandent rien** : une session qui cite une voiture absente se montre déjà comme telle, et c'est le comportement voulu à l'arrivée d'un export sans bibliothèque.

---

# 4. La base

## 4.1 Table par table

L'export produit une copie de `overlay.sqlite` (`VACUUM INTO`, lecture seule, comme la sauvegarde de démarrage), puis **vide ou filtre** ce qui ne doit pas partir. La table de référence, qui doit bouger avec chaque table nouvelle (§8.3) :

| Table | Export | Traitement |
|---|---|---|
| `mods` | oui | toutes les lignes, contenu d'origine compris (§4.2) ; `active_version_id` gardé |
| `versions` | oui | `content_state` forcé à `skeleton`, `freed_at` à la date d'export ; `library_path` relatif gardé ; signature, archive et origine gardées |
| `layers`, `sub_mods` | oui | `content_state` forcé à `skeleton`. **Une couche garde son interrupteur** (`is_active` dit si elle entre dans la composition, un choix de l'utilisateur) ; un son ou un habillage de circuit passe à 0 (son `is_active` dit qu'il est posé). Une couche de voiture ou de circuit part avec le squelette de son hôte (ESPACE§5.4). Les livrées fournies avec un mod restent `full` : elles reviennent avec lui |
| `apps`, `other_mods` | oui | `content_state` forcé à `skeleton` (ESPACE§5.6), `is_active` à 0 et `junctions` vidé pour un autre mod ; leur dossier ne part qu'avec son manifeste, écrit par l'export. Nom repris, note, priorité et rattachement corrigé suivent |
| `usage` | oui | « déjà essayé », nombre de lancements |
| `tech_facts`, `tech_user` | oui | la fiche technique (`FICHE§`) : c'est elle qui garde, sans aucun fichier, les chiffres, la mécanique lue dans la physique et l'électronique ; et les corrections de l'utilisateur |
| `history` | oui | plus une ligne « Transfert — exporté depuis une autre installation » à l'import |
| `wiki_link`, `wiki_no_match` | oui | décisions et résultats de rapprochement |
| `wiki_cache` | oui | repris seulement si l'export porte la même version de contenu que l'application qui importe (`wiki_content_version`) ; sinon il se refait au réseau |
| `import_decisions` | oui | le journal des décisions d'import (ce que l'app a décidé seule, et pourquoi) : de l'histoire, comme `history` |
| `forced_extras` | oui | autorisations de remplacer un fichier du jeu : une décision, qui doit survivre comme elle survit à une désactivation |
| `online_track_categories` | oui | catégories de circuit choisies sur la page Online, par id de circuit : une décision, que l'export emporte et qui l'emporte à l'import |
| `profiles`, `profile_entries`, `profile_extra_entries` | partie Profils | vidées sinon |
| `media_links` | **non** | rattachements de captures et replays à des fichiers de la machine d'origine, par chemin ; ces fichiers ne voyagent pas |
| `extra_links`, `game_backups` | **non** | état du dossier du jeu de la machine d'origine (R2) |
| `pending_folders`, `pending_answers` | **non** | imports en cours |
| `meta` | **non** | les versions de moteur (harmonisation, règles) se recalculent à l'arrivée : en leur absence, la réharmonisation se déclenche d'elle-même au premier démarrage |

**Une ligne dont le dossier est hors de la bibliothèque ne part pas** (les livrées que le jeu livre avec son contenu, que l'autre machine retrouve), ni les versions synthétiques du contenu d'origine.

**Tout mod exporté est inactif à l'arrivée, et rien ne se réactive de lui-même** une fois récupéré. Les mods actifs au moment de l'export sont notés dans le manifeste (§6), sans autre usage.

## 4.2 Le contenu d'origine et les mods non gérés

**Contenu d'origine (`is_stock`).** Ses lignes partent, parce qu'elles portent des saisies (une note sur la Ferrari F2004, un favori) et des décisions. À l'import, le contenu d'origine de la nouvelle machine est indexé d'abord, par le chemin normal ; puis les lignes de l'export **ne rapportent que les champs de l'utilisateur** sur les ids qui existent. Pour les lignes où l'utilisateur a écrit quelque chose, la catégorie, la classe et le pays suivent aussi. Un id absent (DLC non possédé sur la nouvelle machine) est laissé de côté — sa ligne ne part pas dans la base d'arrivée — et compté dans le rapport : « 12 voitures d'origine absentes de cette installation (DLC ?) : leurs notes n'ont pas été reprises. » Les ids sont listés.

**Mods non gérés (`is_unmanaged`).** Ils n'ont pas de dossier en bibliothèque : ils vivent dans le `content/` de la machine d'origine. L'export leur **fabrique une version en vitrine** : squelette tiré de leur dossier dans le jeu, signature de contenu calculée sur leurs vrais fichiers au moment de l'export, `is_unmanaged` remis à 0. À l'arrivée, ce sont des mods en vitrine comme les autres, et **ils deviennent gérés** au premier import de leur archive. C'est le seul chemin, dans Pit Box, par lequel un mod non géré passe sous gestion sans que l'utilisateur ait à déplacer son dossier. **Un mod installé à la main des deux côtés** (aussi dans le `content/` de la machine d'arrivée) garde sa ligne locale et ne reçoit que ce que l'utilisateur avait écrit dessus ; le rapport le nomme.

## 4.3 Les images de vitrine

Pour un mod déjà en vitrine, l'image de vitrine existe : elle part telle quelle. Pour un mod complet, l'export la fabrique par la même fonction (`ESPACE§3.3`) : l'image que montre la carte, c'est-à-dire l'aperçu de la livrée ou du layout préféré, lu dans `ui_prefs.json` avant le nettoyage des chemins.

---

# 5. Les fichiers

## 5.1 Le contenu du `.pitbox`

```
bibliotheque.pitbox
├── pitbox-export.json              manifeste (§6)
├── bibliotheque/
│   ├── overlay.sqlite              §4
│   └── fichiers/                   les squelettes, aux chemins relatifs de la bibliothèque
│       ├── cars/rss_gtm_lanzo_v10/v4/ui/ui_car.json
│       ├── cars/rss_gtm_lanzo_v10/v4/ui/badge.png
│       ├── cars/rss_gtm_lanzo_v10/v4/.pitbox-vitrine.png
│       ├── cars/rss_gtm_lanzo_v10/v4/.pitbox-vitrine.json
│       └── …
├── classement/
│   ├── taxonomy.json
│   ├── rules-overlay.json
│   ├── tag-rules.json              tant qu'il n'est pas migré
│   ├── brand_logos.json
│   └── logos/                      les logos à soi
├── sessions/
│   ├── saved_grids.json
│   └── presets/*.cmpreset
└── preferences/
    ├── prefs.json                  les préférences de config.json, sans les chemins (§5.2)
    ├── ui_prefs.json
    ├── library_columns.json
    └── music.json                  sans les chemins de fichiers (§5.2)
```

**Le manifeste de vitrine est écrit par l'export** pour chaque version, y compris d'un mod complet : la liste `removed` est calculée sur les vrais fichiers au moment de l'export. À l'arrivée, chaque mod sait exactement ce qui lui manque et combien ça pèse, et la réhydratation vérifie comme pour un mod mis en vitrine sur place (`ESPACE§7.4`).

## 5.2 Les fichiers de préférences, un par un

Repris de la liste de la sauvegarde de démarrage (`backup.rs`), fichier par fichier :

| Fichier | Partie | Traitement |
|---|---|---|
| `config.json` | Préférences | **seul l'objet `prefs`** part, dans `prefs.json` ; les six chemins (jeu, bibliothèque, Content Manager, 7-Zip, QuickBMS, script) jamais |
| `ui_prefs.json` | Préférences | tel quel, chemins retirés : les livrées et layouts préférés gardent leur choix sans leur aperçu |
| `library_columns.json` | Préférences | tel quel |
| `music.json` | Préférences | ses dossiers d'ambiance à `null` : l'autre machine joue son pack embarqué |
| `taxonomy.json`, `rules-overlay.json`, `brand_logos.json`, `tag-rules.json` | Classement | tels quels ; les logos à soi (`logos/`) avec eux |
| `saved_grids.json` | Sessions et grilles | tel quel |
| presets `.cmpreset` du dossier `Pit Box\` de Content Manager | Sessions et grilles | tels quels, sans les chemins de leurs presets voisins (ils gardent la copie de leur contenu) |
| `session.json`, `launch_state.json` | **non** | état de la session en préparation (R2) |
| `saved_sessions.json` | **non** | ancien format, migré en presets (`SESSION§3.6`) |

---

# 6. Le manifeste

`pitbox-export.json`, à la racine :

```json
{
  "format": 1,
  "app_version": "0.42.0",
  "exported_at": "2026-09-27T09:41:00Z",
  "parts": ["library", "classification", "sessions", "profiles", "preferences"],
  "counts": { "cars": 146, "tracks": 6, "layers": 11, "skins": 240, "sounds": 3,
              "stock_with_user_data": 17, "unmanaged": 4 },
  "active_at_export": ["rss_gtm_lanzo_v10", "shannonville"],
  "library_bytes_at_export": 312400000000
}
```

- `format` est celui du conteneur ; `app_version` décide si l'export est importable (R4). Deux numéros, parce que le conteneur changera bien moins souvent que l'application.
- `active_at_export` n'a pas d'usage à l'arrivée.
- `library_bytes_at_export` sert une seule phrase, à l'import : « Cette bibliothèque pesait 312 Go ; ce fichier en fait 4 Mo. »

---

# 7. Les écrans

## 7.1 Où

**Fichiers › Maintenance**, une rubrique **« Transférer la bibliothèque »**, avec « Exporter… » et « Importer… ». À côté du relevé anonyme, qui est lui aussi un fichier que la bibliothèque produit.

**L'assistant de première configuration** propose l'import à sa dernière étape, une fois les chemins réglés : « Vous avez un export Pit Box ? Importez-le pour retrouver votre bibliothèque. » C'est le moment où la règle R3 est satisfaite sans effort, et celui où l'utilisateur se pose la question.

## 7.2 Exporter

1. **Les parties**, cochables, avec pour chacune ce qu'elle contient en une ligne et un décompte (« 152 mods, 11 couches, 17 voitures d'origine annotées »).
2. **La taille estimée**, calculée avant d'écrire, sans préparer l'export : les décomptes viennent de la base, le poids des seuls fichiers d'un squelette, avec des taux de compression mesurés.
3. **Ce qui ne partira pas**, en clair : « Le contenu des mods ne part pas : ils arriveront en vitrine, apps et compléments compris. »
4. **Le fichier** : boîte d'enregistrement, nom proposé `Pit Box - <date>.pitbox`.
5. **En fond**, progression dans la pile de notifications, comme le relevé anonyme. Rapport à la fin, avec « Ouvrir le dossier ».

**Une phrase sur le contenu du fichier**, sous le bouton, pour qui voudrait le partager : « Ce fichier contient vos notes et les images des mods (celles de leurs auteurs), mais aucun fichier jouable. » Pas d'avertissement plus appuyé : c'est le fichier de l'utilisateur.

## 7.3 Importer

1. **Choisir le fichier.** Lecture du manifeste seul, sans rien écrire.
2. **Vérifier** : format connu et version de l'application pas plus récente (R4), installation vide (R3). Un refus dit **pourquoi** et **quoi faire** : « Cette installation contient déjà 146 mods. L'import d'une bibliothèque ne se fait que dans une installation vide. » ; « Cet export vient de Pit Box 0.45. Mettez Pit Box à jour pour l'importer. »
3. **Montrer ce qui arrive** : parties présentes (cochables, pour n'en reprendre qu'une partie), décomptes, date d'export, « pesait 312 Go ».
4. **Importer**, en fond, dans cet ordre :
   1. la sauvegarde de démarrage, même sur une installation vide ;
   2. l'indexation du contenu d'origine de cette machine ;
   3. la base de l'export est attachée à la base vivante et recopiée **dans une transaction**, puis `overlay::migrate` la met à niveau (R4) ;
   4. le report des champs de l'utilisateur sur le contenu d'origine (§4.2) ;
   5. les squelettes, aux chemins relatifs de la bibliothèque ;
   6. les fichiers de classement, sessions et préférences — **seuls les fichiers que l'import connaît** reviennent dans `app_config_dir`, et aucune entrée du zip ne peut sortir du dossier où elle atterrit : un `.pitbox` se transmet, un fichier fabriqué ne doit rien pouvoir remplacer. Les presets rejoignent le dossier `Pit Box\` de Content Manager **sans écraser** un preset de même nom (`Nom (2).cmpreset`) ;
   7. `prefs.json` fusionné dans `config.json` : les préférences sont reprises, **les chemins de cette machine ne bougent pas** ;
   8. la réharmonisation, déclenchée d'elle-même par l'absence des versions de moteur (§4.1).
5. **Le rapport** : combien de mods sont en vitrine et combien étaient actifs, et « Ouvrir la bibliothèque », qui **recharge l'application** — préférences, colonnes et musique viennent d'être réécrites. **Rien n'est filtré ni sélectionné, rien ne se réactive** : sur une autre machine, la plupart des mods restent en vitrine, et on récupère ceux dont on a l'archive quand on veut (`ESPACE§7`).

**Un import qui échoue en route** se défait : la transaction est annulée, chaque fichier écrit est retiré, chaque fichier remplacé retrouve ses octets. L'installation revient vide, et le rapport dit à quelle étape ça s'est arrêté.

---

# 8. Implémentation

## 8.1 Backend

Le module `transfer/` **réutilise sans recopier** la liste blanche et l'image figée de la vitrine (`ESPACE§9.1`), la copie de base de la sauvegarde de démarrage (`VACUUM INTO` depuis une connexion en lecture seule), l'indexation du contenu d'origine et les migrations d'`overlay::migrate`. **Le zip s'écrit en flux**, jamais en mémoire, et le manifeste se lit sans rien extraire.

## 8.2 Frontend

La rubrique « Transférer la bibliothèque » de Fichiers › Maintenance, l'étape de l'assistant, et les deux parcours de §7.2 et §7.3 avec leurs refus en toutes lettres.

## 8.3 La table de §4.1 est une porte

Un test liste les tables de la base (`sqlite_master`) et **échoue** si l'une d'elles n'est pas classée dans `transfer.rs` (exportée, filtrée ou exclue). Une table ajoutée plus tard sans y penser serait sinon exportée par défaut avec tout son contenu, ou perdue en silence. Même logique que le contrôle des renvois : c'est la seule façon qu'un oubli se voie.

## 8.4 Tests

Les tests de `transfer` portent les cas : l'aller-retour d'une bibliothèque de fixture (saisies et décisions identiques, tout en vitrine, aucun `.kn5` ni `.acd` dans le zip), la fiche technique rendue à l'identique sans lire de fichier (`FICHE§9.4`), **aucun chemin absolu dans le zip, à aucune profondeur d'échappement**, les refus, l'export ancien mis à niveau, le contenu d'origine absent compté, le mod non géré réhydraté et géré, le retour arrière, et la porte des tables (§8.3).

---

# 9. Ordre de livraison et hors périmètre

Livré après la vitrine (ESPACE lot 1) et l'état sans fichiers des apps et des autres mods (ESPACE§5.6), qui existe pour lui.

**Hors périmètre, à rediscuter** :

- **Fusion** d'un export dans une bibliothèque existante (R3).
- **Export partiel de mods** (une sélection, un profil) pour partager une collection avec quelqu'un. Le format le permet ; l'écran et la question des doublons à l'arrivée restent à penser.
- **Une table d'auteurs connus** (préfixe d'id ou nom d'archive vers la page du magasin : RSS, VRC, URD…) comme source de plus pour « Récupérer les fichiers ». Les adresses sont à fournir par le propriétaire du projet, pas à deviner ; ce sont surtout des mods payants, donc toujours une page à ouvrir.
- **Les captures et replays** (`media_links`) : ils vivent dans les dossiers de Content Manager et du jeu, pas dans la bibliothèque.

---

# 10. Ce que l'implémentation a appris

## 10.1 Écarts assumés

Les écarts au plan d'origine sont dans le texte ci-dessus (§2, §4, §5.2, §7.3), qui décrit ce que l'app fait.

## 10.2 Pièges payés

- **Une base copiée garde ce qu'on en efface** dans ses pages libres : un `VACUUM` final avant d'emballer (R2).
- **Deux tables ont une clé étrangère vers `mods`** (`versions`, `tech_facts`) : une ligne sur un contenu d'origine absent de l'autre machine faisait échouer tout l'import ; elle ne part pas (§4.2).
- **Des chemins encodés deux fois passaient inaperçus** : `ui_prefs.json` stocke ses valeurs en JSON sérialisé dans une chaîne, où un chemin a ses `\` quadruplés. Le test « rien de la machine » cherche donc n'importe quel chemin absolu, à toute profondeur d'échappement.
