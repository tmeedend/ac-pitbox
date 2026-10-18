# Pit Box — supprimer sans tout perdre : les mods en vitrine

## Spécification d'implémentation

*Supprimer un mod libère son espace disque, et le garde par défaut dans la bibliothèque, « en vitrine », avec tout ce qu'on sait de lui. Premier lot du chantier import / export de la bibliothèque (`EXPORT§`), qui repose entièrement sur cet état.*

> **Étiquette de renvoi : `ESPACE§`.** Livrée (`skeleton.rs`, `showcase/`, `library/showcase.svelte.ts`, `recovery.svelte.ts`), étendue aux apps et aux autres mods pour l'export (§5.6). Le fichier garde son nom d'origine (`sans-fichiers`) ; le mot de l'interface est « vitrine » (§4.4).

---

# 1. Objet

Une bibliothèque de 300 Go est faite à plus de 99 % de modèles 3D, de textures et de banques de son. Ce qui fait qu'on la **parcourt** (le nom, la marque, une image, la fiche technique, les tags, les notes, le favori) tient en quelques dizaines de Ko par mod.

« Supprimer » met d'abord le mod **en vitrine** : ses fichiers lourds sont supprimés, mais il reste dans les listes avec son image, sa fiche technique et tout ce qu'on lui a ajouté — nom repris, description, note, tags manuels, favori. Il ne peut plus aller dans le jeu tant qu'on n'a pas **récupéré ses fichiers**. La suppression complète reste possible, en choix explicite.

C'est aussi la brique de l'export de bibliothèque (`EXPORT§`) : une installation neuve qui importe un export ne contient que des mods en vitrine.

---

# 2. Règles fondatrices

## R1 — Supprimer, c'est d'abord mettre en vitrine

Une seule porte, **« Supprimer… »**, et deux issues dans sa confirmation : **« Garder en vitrine »**, choisie par défaut, et **« Supprimer complètement »** (§5). Quelqu'un qui cherche de la place va vers « Supprimer » ; c'est là, au moment où il en a besoin, qu'il découvre qu'il n'est pas obligé de tout perdre. Une entrée « Libérer l'espace » à part serait un geste de plus à apprendre, que presque personne ne trouverait.

## R2 — Rien de ce qui ne coûte rien ne se perd

La vitrine **ne supprime aucune ligne en base**. Seuls des fichiers partent. Tout ce que l'utilisateur a saisi et tout ce que l'app a trouvé ou calculé reste (§3.4). Ce qui n'était lisible que dans un fichier qui va partir est **copié en base avant** la suppression.

## R3 — Un squelette minimal reste en place

La version garde son `library_path`, son dossier existe toujours, et on y laisse le strict nécessaire aux **listes** (§3.1). Tout ce qui résout le dossier d'un mod (`library::entity_dir`, `overlay::get_version_path`) continue de répondre, et ce qui lit `ui/` continue de lire.

Le squelette vit dans la **bibliothèque de Pit Box**, jamais dans `content/` : le mod est désactivé avant (§5.5), et rien ne le repose (R5). Content Manager et Assetto Corsa ne lisent que `content/` : ils ne voient pas un mod en vitrine, ni cassé ni autrement. Garder ces quelques dizaines de Ko sur disque plutôt que tout copier en base laisse fonctionner tout ce qui lit `ui/`, et l'export (`EXPORT§`) réutilise le squelette tel quel.

## R4 — La fiche d'un mod en vitrine est dégradée, et le dit

Galerie de livrées, aperçu 3D, écoute du moteur, cartes de tracé : ce qui n'est pas dans le squelette disparaît de la fiche, **remplacé par une phrase qui dit pourquoi**, jamais par un vide ou une erreur. Ce qui est en base reste affiché.

## R5 — Un mod en vitrine n'entre jamais dans le jeu

Un squelette déployé dans `content/` serait une voiture sans modèle : Assetto Corsa et Content Manager planteraient au chargement. **Toute pose passe par un garde-fou unique** (§9.1), dans le moteur et pas seulement à l'écran.

## R6 — Récupérer, c'est importer

On récupère les fichiers en important l'archive, par le chemin ordinaire. L'import reconnaît la version en vitrine et la **réhydrate** au lieu de la classer en doublon (§7.3).

## R7 — Le disque dit ce qui manque

Chaque version en vitrine porte un **manifeste** à sa racine, `.pitbox-vitrine.json` (§4.2). Perdre la base ne rend pas un squelette indéchiffrable, et la réhydratation vérifie qu'elle a bien tout remis.

## R8 — Supprimer se dit avant et après

**Corbeille Windows d'abord**, suppression définitive en repli ; la confirmation le dit **avant**, et l'issue réelle (recyclé ou effacé) est affichée **après**. Même discipline que la suppression d'une version de mod dans `SPEC.md`. À plusieurs Go par mod, le quota de la corbeille est dépassé dans le cas normal.

---

# 3. Le squelette

## 3.1 La liste blanche

Ce qui **reste** dans le dossier d'une version en vitrine. Tout le reste part. **La même liste sert à l'export** (`EXPORT§`) : il n'y a qu'une définition de ce qu'est un squelette.

| Type | Chemin, relatif au dossier de la version | Pourquoi |
|---|---|---|
| Voiture | `ui/ui_car.json`, `ui/badge.png`, et les autres `ui/*.json` | nom, marque, specs, courbes de puissance et de couple, tags, badge : la fiche technique et le rangement |
| Circuit | `ui/**/ui_track.json`, `ui/**/outline.png`, `ui/**/logo.png`, et `ui/**/preview.png` **réduit** (§3.3) ; aussi `outline.jpg`, `logo.jpg`, `preview.jpg` et un `map.png` **rangé dans `ui/`**, que la carte lit en repli | chaque tracé garde son nom, son contour et son image |
| Voiture, circuit | `.pitbox-vitrine.png` ou `.pitbox-vitrine.jpg` : **l'image de vitrine** (§3.3), l'extension disant le format réel | l'image de la carte |
| Voiture, circuit | `.pitbox-vitrine.json` : le manifeste (§4.2) | ce qui manque |

**Ce qui part, et pourquoi :**

- **`data.acd` et `data/`.** Le `data.acd` est chiffré par Kunos ; le garder, et surtout l'exporter, reviendrait à redistribuer une partie protégée du mod. Ce qu'on en tire est copié en base avant (§3.4).
- **Tous les aperçus de livrées, les `livery.png`, les `.dds` de `ui/`.** La galerie de livrées se réduit aux **noms** (colonne `versions.skins`, déjà en base) autour de l'image de vitrine (R4).
- **`extension/ext_config.ini`.** Les fonctionnalités CSP relevées à l'import sont déjà en base (`versions.csp_features`).
- **Les cartes de tracé (`<tracé>/map.png`)**, les ressources (notices, templates), les ajouts au jeu, les couches.

La liste vit dans le code en **un seul endroit** (`skeleton.rs`, §9.1), avec ses tests.

## 3.2 Ce que ça pèse

Un squelette pèse quelques dizaines de Ko pour une voiture, moins d'un Mo pour un circuit à plusieurs tracés (mesuré : 75 Ko pour une voiture de 551 Mo). Mille mods en vitrine tiennent dans 100 à 200 Mo : c'est ce qui rend acceptable que la vitrine soit le choix par défaut, et qu'elles s'accumulent.

Les logos et contours de tracé ne sont pas réduits : ce sont déjà de petits PNG, et un contour recompressé perdrait sa netteté pour quelques Ko.

## 3.3 L'image de vitrine

**Une image par mod, figée au moment de la mise en vitrine**, et c'est celle que la carte affiche ensuite.

- **Laquelle** : exactement celle que la carte montre à cet instant. Si une **vignette de grille régénérée** existe (GRILLE§5, aujourd'hui éteinte), c'est elle ; sinon l'aperçu de la livrée que la carte prend. Le choix est fait par **la fonction de la carte**, exposée pour l'occasion, pas par une règle recopiée.
- **Pourquoi la figer** : l'identité d'une vignette régénérée est calculée sur le `.kn5` (nom, date, livrée, configuration CSP, `gridthumbs.rs`). Le `.kn5` parti, la carte ne retrouverait plus sa vignette. L'image doit donc être copiée dans le squelette avant la suppression, pas cherchée après.
- **Format** : 480 px de large. PNG si elle a de la transparence (vignette régénérée), JPEG qualité 80 sinon. Quelques dizaines de Ko.
- **Circuit** : l'image de vitrine est l'aperçu du tracé par défaut, réduit de la même façon ; les `preview.png` de chaque tracé sont réduits en place.

À la réhydratation, l'image de vitrine est supprimée : la carte revient à son chemin normal.

## 3.4 Ce qui reste, et ne coûte rien

**Tout ce qui est en base reste**, puisque aucune ligne n'est supprimée (R2) : les saisies de l'utilisateur (nom, description, note, tags, favori, classe, livrée pilotée), ses décisions (lien Wikipédia, médias rattachés à la main, autorisations de remplacer un fichier du jeu), ce que l'app a déduit ou relevé à l'import (marque, pays, année, catégories, version, auteur, fonctionnalités CSP, noms des livrées et des tracés, taille, archive d'origine), l'usage, l'historique et le journal d'import, l'article Wikipédia en cache, et les lignes des couches et sous-éléments.

**La fiche technique entière est en base** (`FICHE§`) : chiffres, courbes, mécanique lue dans la physique, électronique, et les saisies de l'utilisateur. Rien n'est donc à copier au moment de la mise en vitrine, pourvu que la fiche ait été remplie ; le déroulé le vérifie (§5.5) et la remplit si ce n'est pas le cas, **avant** de supprimer `data.acd`. **Règle générale** : toute donnée que la fiche tire d'un fichier hors de la liste blanche est écrite en base à l'import, jamais relue sur disque.

La **distance parcourue** n'est pas chez Pit Box : elle vit dans le journal de Content Manager (`SPEC.md`), qui ne dépend pas des fichiers du mod. Elle reste affichée.

---

# 4. Le statut

## 4.1 En base

Une version porte `content_state` (`full` par défaut, ou `skeleton`), la date et le volume libérés, et l'origine de l'archive (§8). **Sa taille n'est pas réécrite** : elle reste celle d'avant la mise en vitrine — ce que coûtera la récupération —, et c'est sur elle que trie la colonne Taille ; une réindexation saute les squelettes. Les colonnes s'ajoutent avec leur défaut : les versions existantes restent complètes sans rien réécrire. « Vitrine » est le mot de l'interface, `skeleton` celui du code.

**Le statut d'un mod se déduit, il ne se stocke pas.** Un mod est **en vitrine** quand sa version active est `skeleton`. Une ancienne version peut être en vitrine alors que l'active ne l'est pas.

`content_state` et `freed_at` s'ajoutent aussi à `layers` et à `sub_mods` (§5.4), et à `apps` et `other_mods` (§5.6).

## 4.2 Sur le disque : le manifeste

`.pitbox-vitrine.json`, à la racine du dossier de la version (et de chaque couche ou sous-élément libéré) :

```json
{
  "format": 1,
  "freed_at": "2026-09-27T10:14:03Z",
  "mod_id": "rss_gtm_lanzo_v10",
  "version_label": "1.4",
  "content_signature": "9f3a…",
  "source_archive": "RSS_GTM_Lanzo_V10_v1.4.7z",
  "source_site": "https://www.overtake.gg/",
  "source_file_name": "RSS_GTM_Lanzo_V10_v1.4.7z",
  "removed": [
    { "path": "rss_gtm_lanzo_v10.kn5", "size": 50635170 },
    { "path": "sfx/rss_gtm_lanzo_v10.bank", "size": 70384864 }
  ],
  "removed_bytes": 550900000
}
```

Trois usages : afficher ce qui manque sans lire la base ; vérifier qu'une réhydratation a tout remis (§7.4) ; retrouver l'archive et la signature si la base est perdue.

**La signature stockée en base ne se recalcule jamais sur un squelette.** `identity::content_signature` hache les `.kn5` et `.acd` présents : sur un squelette, elle rendrait une autre valeur, et c'est précisément la valeur d'origine qui permet de reconnaître l'archive au retour.

## 4.3 À l'écran

- **Carte de bibliothèque** : l'image de vitrine ; une pastille grise « En vitrine » en coin, avec une icône de cadre. Pas d'image grisée : la bibliothèque doit rester belle, c'est tout l'intérêt.
- **Vue tableau** : même pastille dans la colonne d'état.
- **Filtre** : une valeur « En vitrine » dans la puce État de la bibliothèque, et son contraire (exclure).
- **Fiche** : un bandeau sous les onglets, de la famille du bandeau de mise à jour (`UpdateBanner`), qui porte la phrase de §4.4 et « Récupérer les fichiers » (§7).
- **Colonne de session, sélecteur d'adversaires** : jamais proposé (§6).

## 4.4 Le vocabulaire, et comment il s'explique

| Quoi | Libellé |
|---|---|
| L'état | **En vitrine** |
| Le choix par défaut de la suppression | **Garder en vitrine** |
| Le choix qui efface tout | **Supprimer complètement** |
| L'action qui en sort | **Récupérer les fichiers** |
| Ce qu'on dit après | « … est en vitrine, 550 Mo libérés » |

« En vitrine » dit ce que le mod **est encore** : exposé, rangé, décrit, mais pas dans le garage. On regarde un mod en vitrine, on ne le conduit pas.

**Une phrase de référence** l'explique partout où il apparaît, sans variante d'un écran à l'autre (une seule clé de langue) :

> **En vitrine** : ce mod reste dans votre bibliothèque avec son image, sa fiche technique et vos notes, mais ses fichiers ne sont plus sur le disque. Récupérez-les pour pouvoir l'utiliser en session.

Elle apparaît à quatre endroits, et n'est jamais portée par le seul survol (règle manette) :

1. **Le bandeau de la fiche** (§4.3).
2. **La confirmation de suppression**, sous le choix « Garder en vitrine » (§5.2). La première fois, un encart « Comment ça marche » en trois temps : le mod passe en vitrine · il ne va plus en session · on le récupère en réimportant son archive ou en le téléchargeant. « Ne plus afficher » (`ui_prefs.json`).
3. **La liste vide** quand « En vitrine » est posée en filtre.
4. **L'infobulle de la pastille**, pour la souris ; à la manette, la même phrase est dans le bandeau de la fiche, où mène la carte.

---

# 5. Supprimer

## 5.1 Où

Les points d'entrée existants de la suppression, **inchangés de place** :

| Endroit | Libellé |
|---|---|
| Fiche d'un mod, menu ⋮ | « Supprimer… » |
| Bibliothèque, sélection multiple | « Supprimer… » |
| Mod déjà en vitrine, fiche ou sélection | « Supprimer complètement… » (plus rien d'autre à proposer) |

La corbeille d'une **ancienne version**, dans la frise de la fiche, reste ce qu'elle est : elle supprime une version, pas le mod. Elle gagne un voisin, « Mettre en vitrine », pour libérer une ancienne version sans perdre sa ligne dans la frise.

## 5.2 La confirmation

> **Supprimer RSS GTM Lanzo V10 ?**
>
> ◉ **Garder en vitrine** (recommandé)
> 550 Mo libérés. Le mod reste dans votre bibliothèque avec son image, sa fiche technique et vos notes. Vous pourrez récupérer ses fichiers plus tard.
>
> ○ **Supprimer complètement**
> 550 Mo libérés. Le mod et ce que vous y avez ajouté (nom, notes, tags, favori, historique) disparaissent de la bibliothèque.
>
> Il sera désactivé. Sa couche « Livrées HD Lanzo » perd aussi ses fichiers (212 Mo).
> Pour le récupérer : **RSS_GTM_Lanzo_V10_v1.4.7z**, téléchargé depuis overtake.gg, ou le registre de Content Manager.
>
> [Annuler] [Supprimer]

- **Deux choix côte à côte, pas une case « purger ».** Une case ressemble à une option avancée, et on la saute ; deux choix obligent à lire les deux conséquences, et c'est là que l'utilisateur apprend ce qu'est la vitrine.
- **Le choix par défaut est toujours la vitrine**, et il n'est pas mémorisé. Se tromper vers la vitrine se rattrape en un clic ; se tromper vers la suppression complète fait perdre des notes.
- **La deuxième ligne de chaque choix** dit ce qui reste et ce qui part, en toutes lettres.
- **Lignes conditionnelles** : « Il sera désactivé » si le mod est actif ; « Ses 3 versions perdent leurs fichiers » s'il en a plusieurs (§5.4) ; une ligne par couche, avec son poids et le rappel qu'elle a sa propre archive (réimporter la base ne ramène pas les couches, §7.5) ; la source, quand on la connaît (§8), sinon « Aucune source connue : gardez l'archive quelque part. » La ligne de source ne s'affiche que pour le choix vitrine.
- **Si l'archive source est conservée** : une case « Garder aussi l'archive conservée », cochée, qui permet de récupérer en un clic et hors ligne.

**En masse**, la confirmation résume : « 12 mods, 18,4 Go. 3 sont actifs et seront désactivés. 2 ont des couches. 4 n'ont aucune source connue. » Les 4 sont nommés sous la phrase.

**Après**, la notification dit ce qui s'est passé et laisse une sortie : « RSS GTM Lanzo V10 est en vitrine, 550 Mo libérés. » avec **« Supprimer complètement »**. C'est la réponse à la surprise la plus probable (« je l'ai supprimé et il est encore là »), avec la pastille sur la carte.

## 5.3 Supprimer complètement

La suppression complète (`overlay::delete_mod`) :

- **efface aussi** les rattachements manuels de médias, le journal des décisions d'import et la fiche technique du mod, faits et saisies (`FICHE§`) ;
- **garde trois exceptions volontaires**, qui existent pour qu'un réimport sous le même id retrouve ce qu'il avait : le marqueur « déjà essayé » (`usage`), les livrées et sons rattachés (`sub_mods`), et l'appariement Wikipédia (`wiki_link`, sans clé étrangère **délibérément**, selon le commentaire du schéma). La confirmation ne les cite pas : ce ne sont pas des saisies que l'utilisateur penserait perdre.

Sur un mod en vitrine, elle supprime le squelette et la ligne.

## 5.4 Ce qui passe en vitrine avec le mod

| Élément | Traitement | Pourquoi |
|---|---|---|
| Toutes ses versions | vitrine, **toutes ou aucune** | une ancienne version sans la nouvelle n'a pas de sens pour le jeu ; si une seule résiste (fichier verrouillé), aucune ne perd ses fichiers et la suppression échoue en le disant. La confirmation annonce le nombre de versions (§5.2) |
| Ses couches | vitrine, avec **le squelette de leur hôte** (§3.1) : ligne et manifeste gardés, et ce que la liste blanche garde pour une voiture ou un circuit | elles ne composent rien sans leur base ; mais un layout qu'une couche apporte doit rester sur la fiche du circuit (voir ci-dessous) |
| Ses ajouts au jeu (`extras/`) | supprimés, **sans manifeste** dans leur dossier (il serait posé dans le jeu) | retirés du jeu à la désactivation |
| Ses livrées et sons rattachés (`sub_mods`) | vitrine, sans squelette ; un son actif est d'abord remis à l'original ; les livrées fournies avec le mod partent avec sa version | inutilisables sans la voiture ; leurs noms restent |
| Ses ressources | supprimées, sauf les fichiers texte de moins de 64 Ko (`.txt`, `.md`, `.nfo`, `.url`) | une notice dit souvent **où télécharger** le mod, et pèse quelques Ko |

**Le squelette d'une couche.** Une couche de circuit apporte souvent un layout entier (`spa2022` sur le Spa de Kunos) : sans rien garder, ce layout disparaissait de la fiche du circuit en vitrine, nom, contour et aperçu compris. Une couche garde donc ce que garde son hôte (§3.1), aperçus réduits, et **la fiche d'un hôte en vitrine lit ses couches en vitrine** comme elle lit son propre squelette. Sur un hôte qui a ses fichiers, en revanche, une couche en vitrine reste hors de la lecture : un layout qu'on ne peut pas lancer n'a rien à faire dans la liste des layouts. Elle reste dans le bloc Couches de la fiche, « En vitrine », avec le nom de son archive et une recherche sur le web — c'est le seul endroit d'où récupérer une couche posée sur du contenu d'origine, qui n'a jamais de bandeau de vitrine. Une couche d'app n'a pas de squelette.

**Pas de geste « Garder en vitrine » pour les autres types** : une app, un « autre mod », un mannequin de pilote, une livrée d'une voiture d'origine se suppriment comme aujourd'hui. Les supprimer ne fait presque rien perdre. Les apps et les autres mods ont en revanche l'**état** sans fichiers, pour que l'export les emporte (§5.6).

## 5.5 Le déroulé

En fond (`spawn_blocking`, progression dans la pile de notifications, comme la réparation générale), dans cet ordre :

1. **Désactiver** si actif, par le chemin normal (`activation::deactivate`), qui retire aussi les ajouts au jeu et restaure les originaux remplacés. Si la désactivation échoue, **on s'arrête** : ne jamais supprimer la source d'un mod encore posé dans le jeu.
2. **Vérifier que la fiche technique est en base** (`FICHE§`), la remplir sinon, et **figer l'image de vitrine** (§3.3).
3. **Écrire le manifeste** de chaque version, couche et sous-élément, **avant** toute suppression. Une app tuée au milieu laisse un manifeste qui décrit plus que ce qui manque, jamais moins.
4. **Supprimer** tout ce qui n'est pas dans la liste blanche (le dossier envoyé à la corbeille garde une copie de `ui/` : un mod complet qu'on peut restaurer et réimporter), **réduire** les aperçus de tracé gardés : corbeille d'abord, définitif sinon (R8). Élaguer les dossiers devenus vides, **sauf** la racine de la version. En deux temps, pour que ce soit **toutes les versions ou aucune** (§5.4) : les fichiers de chaque version sont d'abord mis de côté à côté d'elle (un déplacement sur le même disque, qui se défait) ; la première version qui résiste remet tout en place ; alors seulement l'ensemble part à la corbeille.
5. **Marquer** en base (`content_state`, `freed_at`, `freed_bytes`), dans une transaction.
6. **Journaliser** : « Mis en vitrine, 550 Mo libérés ». C'est un événement de cycle de vie, il a sa place dans la frise.

**Au démarrage**, un manifeste présent sur une version encore marquée `full` veut dire « interrompu entre 3 et 5 » : la suppression est reprise et le marquage terminé — **seulement si le manifeste porte l'id du mod** et ce que la ligne sait de son contenu. **Un retour arrière ne supprime jamais un fichier** : il remet ce que contient le dossier d'attente, et s'il ne peut pas tout remettre, il garde manifeste et dossier d'attente pour que le démarrage suivant finisse.

## 5.6 Les apps et les autres mods

Pour l'export (`EXPORT§`) : une app et un « autre mod » (mannequins et tenues de pilote compris, §7.3 du SPEC) portent un nom repris, une note, une priorité, une place dans les profils : une bibliothèque exportée doit les emporter, et leur ligne arrive alors sur l'autre machine **sans dossier**.

**L'état, pas le geste.** Leur suppression ne propose pas « Garder en vitrine » (§5.4) : seul l'import d'un export produit une app ou un autre mod en vitrine.

| Point | Comportement |
|---|---|
| Base | `content_state` et `freed_at` sur `apps` et `other_mods`, défaut `full` (§4.1) |
| Disque | le dossier de bibliothèque ne garde que le manifeste (§4.2), sans squelette ; sa liste `removed` porte chaque chemin et sa taille |
| Ce qui se lit dans le manifeste | les zones du jeu d'un autre mod — c'est ce qui fait d'un mannequin un mannequin dans les Compléments —, son rattachement déduit, son nombre de fichiers et sa taille d'origine ; le langage d'une app |
| Activation, profils | refusées par le garde-fou (`CONTENT_FREED`) ; un profil compte l'élément parmi les ignorés |
| Conflits entre autres mods | aucun, ni dans un sens ni dans l'autre : un autre mod en vitrine ne pose rien |
| Récupérer | réimporter l'archive. L'id d'un autre mod et celui d'une app se déduisent de l'archive et du dossier : même archive, même ligne, nom, note et priorité gardés. L'élément revient **actif**, comme toute app et tout autre mod importés (§4.2 du SPEC) — c'est l'écart avec une voiture (§7.3), dont la réhydratation laisse le choix |
| Écrans | pastille « En vitrine » dans l'écran Apps, dans l'inventaire des Compléments (couches, livrées et sons en vitrine compris) et sur leur fiche, avec le nom de l'archive et une recherche sur le web ; valeur « En vitrine » dans la facette État des Compléments |

---

# 6. Ce que chaque écran fait d'un mod en vitrine

**Défaut pour tout ce qui n'est pas listé : le traiter comme un mod inactif.**

| Écran, geste | Comportement |
|---|---|
| Bibliothèque, index, filtres, tri | normal, avec l'image de vitrine, la pastille et le filtre (§4.3) |
| Fiche : identité, fiche technique, courbes, tags, notes, historique, Wikipédia, médias | normal |
| Fiche : image principale | l'image de vitrine |
| Fiche : galerie de livrées | **les noms seulement** (`versions.skins`) ; « Définir comme livrée pilotée » reste possible, c'est une préférence |
| Fiche : tracés d'un circuit | normaux, avec leur aperçu réduit ; la carte du tracé est remplacée par son contour |
| Fiche : fonctionnalités CSP | lues en base (`versions.csp_features`) |
| Fiche : aperçu 3D | remplacé par l'image, avec « Le modèle 3D reviendra avec les fichiers » |
| Fiche : écoute du moteur | clé de contact désactivée, même phrase |
| Fiche : Ressources et Ajouts au jeu | Ressources : la notice gardée, s'il y en a une ; Ajouts au jeu : vide |
| Fiche : activer, choix de version | désactivés, remplacés par « Récupérer les fichiers » |
| Fiche : showroom Content Manager, régénérer la vignette, exporter le mod | désactivés |
| Colonne de session, choix de voiture ou de circuit | **exclu**. Si le choix retenu passe en vitrine, la carte le dit et « Démarrer la session » est désactivé avec la raison |
| Sélecteur d'adversaires, grilles enregistrées | exclu du vivier ; une grille qui en contient lance sans lui, sans le signaler. La voiture ou le circuit du joueur, eux, refusent la session (`launch::launch`, en filet derrière l'écran) |
| Mise à jour disponible (CUP) | **affichée normalement** : « Mettre à jour » est le moyen le plus simple de récupérer les fichiers |
| Activation en masse, profils | le mod est sauté, compté à part : « 3 mods en vitrine ignorés » |
| Réparation générale | sautée, **jamais comptée comme un échec ni comme cassé** |
| Maintenance, mods cassés | un squelette n'est **pas** cassé ; `broken_reason` lit `content_state` avant tout |
| Maintenance, orphelins | le manifeste, l'image et la racine gardée ne sont pas des orphelins |
| Dossier du jeu (DOSSIER§) | rien dans l'arbre ; la recherche le donne comme « Bibliothèque · en vitrine » |
| Relevé anonyme | inclus : il ne lit que `ui/` |
| Mises à jour de règles, harmonisation | normales |

---

# 7. Récupérer les fichiers

## 7.1 Le panneau

« Récupérer les fichiers », depuis le bandeau de la fiche, liste **les sources connues, dans cet ordre** :

| Source | Condition | Geste |
|---|---|---|
| **Archive conservée** | l'archive source est encore en bibliothèque | « Réinstaller depuis l'archive », un clic, hors ligne |
| **Registre de Content Manager** | l'id figure dans le registre CUP | « Télécharger » : le téléchargement de la mise à jour, tel quel (`SPEC.md` §4.7), y compris la bascule vers le navigateur. Précise la version proposée si elle diffère |
| **Page enregistrée** | `mods.source_url` (adresse d'un pack) | « Ouvrir la page » |
| **Adresse de l'auteur** | champ `url` du `ui_*.json`, quand il est rempli | « Ouvrir la page » |
| **Recherche sur le site d'origine** | `source_site` connu (§8) | « Chercher sur overtake.gg » : le nom du fichier d'origine, restreint à ce site |
| **Recherche** | toujours | « Chercher sur le web » (DuckDuckGo, **sans guillemets** — la recherche exacte d'un nom de fichier ne trouvait rien) : le nom du fichier d'origine, ou le nom du mod |

En pied de panneau, toujours : le **nom exact du fichier d'origine**, sélectionnable, et « ou glissez l'archive dans Pit Box ». Les sources absentes ne sont pas affichées. Rien ne se télécharge sans clic.

## 7.2 En masse

Filtre « En vitrine », sélection multiple, « Récupérer les fichiers… » : Pit Box télécharge à la suite ce que le registre CUP sert en archive directe, et liste le reste avec leur geste. Un téléchargement à la fois, jamais pendant un import.

Pour chaque mod, la source qui ne demande rien à l'utilisateur d'abord : l'**archive conservée** (hors ligne), puis le **registre** quand il sert l'archive elle-même. Un registre qui ne propose qu'une page (hébergeur, Patreon) est **listé, jamais ouvert** : un lot de vingt ouvrirait vingt onglets. Les autres sources (page du pack, de l'auteur, recherches) sont sur la fiche du mod ; le compte rendu, dans la pile de notifications, donne pour chaque mod restant son geste — « Ouvrir » la page, ou « Ouvrir la fiche ». Un mod n'est compté récupéré que s'il est réellement sorti de la vitrine : un import arrêté sur une question n'a encore rien récupéré. Un mod seul n'a pas d'entrée de lot : sa fiche liste toutes ses sources.

## 7.3 La réhydratation à l'import

**Le point de code qui décide tout** : une version en vitrine garde sa signature d'origine (§4.2), donc réimporter l'archive exacte serait classé **doublon** sans la classe `rehydrate`, qui passe avant `duplicate` (`importer::classify`) :

| Situation | Classe | Ce qui se passe |
|---|---|---|
| Id connu, signature égale à celle d'une version **`skeleton`** | **`rehydrate`** | les fichiers sont copiés dans le dossier existant de **cette** version ; `content_state` repasse à `full` ; le manifeste est vérifié (§7.4), puis supprimé avec l'image de vitrine |
| Id connu, signature égale à une version `full` | `duplicate` | inchangé |
| Id connu, signature inconnue, version active `skeleton` | `update` | inchangé : une nouvelle version devient l'active, l'ancienne reste en vitrine dans la frise |
| Id inconnu | inchangé | |

La comparaison porte sur **toutes** les versions du mod, et **sur le manifeste, pas sur le squelette** (`skeleton::original_files`) : à côté du squelette, une couche paraîtrait entièrement neuve. La réhydratation **ne réactive pas** le mod : la ligne du rapport d'import (« Fichiers récupérés ») porte un bouton « Activer », sauf pour une couche, dont l'hôte peut être encore en vitrine. **Les dossiers sont échangés avant que la base passe à `full`**, jamais l'inverse.

Les saisies de l'utilisateur ne sont pas touchées : le chemin de mise à jour les respecte déjà (colonnes `*_user` à côté des champs dérivés), et la réhydratation ne réécrit que les champs relevés sur les fichiers.

## 7.4 Vérification

Après copie, chaque chemin du manifeste doit exister avec sa taille. Un écart est signalé dans le rapport (« 2 fichiers attendus manquent »), le mod passe quand même à `full`, et `broken_reason` prend le relais si l'essentiel manque.

## 7.5 Les couches, livrées et sons

Chacun a sa propre archive et se récupère **à part**, par le même mécanisme : l'import le reconnaît comme couche ou sous-élément du mod (classement inchangé) et, si une entrée en vitrine de même nom existe, la réhydrate au lieu d'en créer une seconde. La fiche les liste sous le bandeau, avec leur nom d'archive.

---

# 8. Noter l'origine à l'import

## 8.1 Ce que Windows garde, mesuré

Pour un fichier téléchargé par un navigateur, Windows écrit un flux NTFS **`Zone.Identifier`** (`<archive>:Zone.Identifier`), section `[ZoneTransfer]`. Relevé le 2026-09-26 sur une archive téléchargée depuis overtake.gg :

| Champ | Valeur relevée | Ce qu'elle vaut |
|---|---|---|
| `ReferrerUrl` | `https://www.overtake.gg/` | **le site**, pas la page du mod : le navigateur réduit le référent à l'origine dès que le fichier est servi par un autre domaine |
| `HostUrl` | une adresse `r2.cloudflarestorage.com/…` signée, `X-Amz-Expires=60` | **un lien jetable**, expiré **une minute** après le clic. Mais son paramètre `response-content-disposition` contient le **nom de fichier d'origine** (`camtool-v3.0.0-beta.3.zip`) |

On ne garde donc **jamais** `HostUrl` comme adresse de téléchargement.

## 8.2 Ce qu'on garde

| Colonne (`versions`) | Tirée de |
|---|---|
| `source_site` | `ReferrerUrl`, réduit à son origine |
| `source_file_name` | le `filename` de `response-content-disposition` ; à défaut, le dernier segment du chemin de `HostUrl` ; à défaut, le nom du fichier importé |

- `mods.source_url` garde son rôle (l'adresse d'un pack) et n'est pas écrasé.
- **Les paramètres de requête ne sont jamais stockés** : ce sont des jetons signés, sans valeur une minute plus tard.
- L'origine n'est lue que sur un fichier d'archive importé tel quel — ni sur un dossier, ni sur une archive imbriquée, ni sur un téléchargement du registre, que le panneau de récupération interroge par l'id du mod. Le flux peut manquer : ce n'est pas une erreur, et rien ne s'affiche.

Le nom de fichier d'origine est la meilleure clé de recherche qui existe ; avec le site, il donne une recherche restreinte qui tombe presque toujours sur la bonne page. La **page exacte**, seule l'extension de navigateur prévue au lot L7 pourra la fournir.

---

# 9. Implémentation

## 9.1 Backend

**La liste blanche** (`skeleton::is_kept`) est la seule définition d'un squelette, pour la vitrine comme pour l'export. **Un seul garde-fou** (`skeleton::guard`, `errors::CONTENT_FREED`) refuse de poser, composer, lancer, écouter, ouvrir dans le showroom, exporter en archive ou prévisualiser en 3D un mod en vitrine ; activation en masse, profils et réparation générale le comptent comme « ignoré ». `maintenance::broken_reason` teste `content_state` avant tout.

## 9.3 Frontend

La phrase de référence (§4.4) est **une seule clé de langue**, et un seul prédicat (`isPlayable`) exclut les mods en vitrine de la colonne de session et du vivier d'adversaires.

## 9.4 Tests

Les tests de `skeleton` et de `showcase` portent les cas : la liste blanche exacte, rien de perdu après la mise en vitrine (fiche technique comprise, `FICHE§9.4`), l'image de carte gardée, l'aller-retour (réimporter → `rehydrate`, même version, fichiers identiques), la version différente (`update`), le garde-fou, l'interruption reprise au démarrage, le mod actif retiré du jeu avant toute suppression, et l'origine lue dans un `Zone.Identifier` réel sans paramètre signé en base. Ils ne passent jamais par la corbeille.

---

# 10. Ce que l'implémentation a appris

## 10.1 Écarts assumés

Les écarts au plan d'origine sont dans le texte ci-dessus (§5.4, §5.5, §6, §7.3, §8.2), qui décrit ce que l'app fait.

## 10.2 Pièges payés

- **La réindexation et la relecture de la fiche technique relisent le disque et remplacent ce qui est en base** : elles sautent les squelettes (`maintenance::reindex_mod`, `techsheet::active_stack`), sans quoi livrées, fonctionnalités CSP et physique seraient effacées.
- **Les livrées projetées sont des junctions dans le dossier de la version**, qu'un parcours de fichiers ne voit pas : elles sont retirées à part, lien seul, **après** avoir figé l'image de la carte, qui passe souvent par l'une d'elles.
- **`active_layers` filtre les couches en vitrine** : c'est le seul endroit où une composition lit ses couches, et une couche réduite à son manifeste l'aurait posé dans `content/`.
- **Pas de manifeste dans les dossiers des ajouts au jeu ni des ressources** : il serait posé dans le jeu, ou montré sur la fiche.
