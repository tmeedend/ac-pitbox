# Pit Box — état du dossier du jeu

## Spécification d'implémentation

*Un écran qui montre le dossier Assetto Corsa tel qu'il est sur le disque et, pour chaque chemin, d'où il vient. Lecture seule. Ne remplace rien : il rend visible ce que `SPEC.md` §4.5.3 à §4.5.5 et IMPORT§6 à IMPORT§7 décrivent.*

> **Une maquette accompagne cette spec** : `maquettes/pitbox-etat-dossier.html`. Elle sert à juger la disposition, l'ordre de lecture et les gestes, **pas de référence graphique** : couleurs, polices et hauteurs viennent de `styles/global.css`. Ses chiffres et ses noms sont plausibles mais inventés. En cas de divergence, la spec gagne.
>
> **Étiquette de renvoi : `DOSSIER§`.** À ajouter à la table de `scripts/check-refs.mjs` en même temps que le premier renvoi du code.

---

# 1. Objet

Mod Organizer 2 montre un « disque virtuel » : ce que le jeu verrait, fichier par fichier, avec le mod qui fournit chacun. Pit Box n'a pas de disque virtuel. Il pose de **vrais fichiers** (hardlinks, junctions, remplacements sauvegardés) dans un dossier où d'autres écrivent aussi : Kunos, Content Manager, CSP, l'utilisateur à la main.

L'écran répond donc à la question de MO2, avec une différence de fond : il montre **l'état réel** du disque, y compris ce qui s'est écarté de ce que Pit Box croit avoir posé.

Quatre questions, dans cet ordre de fréquence :

1. Est-ce que j'ai tel mod, tel fichier ? *(la recherche, §7)*
2. Ce fichier-là, il vient de quel mod ? *(le panneau de détail, §8)*
3. Qu'est-ce qui a été modifié dans le jeu d'origine, et comment je reviens en arrière ? *(le filtre « remplace le jeu », §6)*
4. Qu'est-ce qui n'est à personne, ou qui a dérivé ? *(les filtres « à personne » et « dérive », §6)*

---

# 2. Quatre règles fondatrices

## R1 — Lecture seule

**Rien sur cet écran n'écrit dans le dossier du jeu, dans la bibliothèque, ni dans la base.** Pas de « désactiver », pas de « poser quand même », pas de « restaurer l'original ». Les seuls boutons sont des **navigations** : ouvrir la fiche d'un mod, aller à la Maintenance, afficher un chemin dans l'Explorateur. Le scan (§5) lit le disque et la base ; son résultat vit en mémoire.

Ce que l'écran constate et ne peut pas corriger, il le **dit** et il dit **où** le corriger. C'est la leçon du « en attente » de `SPEC.md` §4.5.4 : un état sans sortie est pire que le silence. Ici la sortie est toujours un lien vers l'écran qui sait agir.

## R2 — Le disque fait foi, la base explique

L'arbre est celui du **disque**, pas celui de la base. Un chemin que la base croit posé et qui n'existe pas apparaît quand même, comme un **manquant** (§4.3) ; un fichier que personne n'a réclamé apparaît comme **à personne**. C'est le croisement des deux sources qui produit l'information, et l'une sans l'autre ne dirait rien des dérives.

## R3 — Un seul arbre ; la provenance est un filtre

Il n'y a **qu'un arbre**, celui du dossier du jeu. La vue « par mod » n'est pas un second arbre : c'est la puce **Provenance** (§6.2), qui élague l'arbre à ce qu'un mod a posé, où que ce soit. Même logique que l'index de bibliothèque (`INDEX§2`) : un seul état de filtre, pas d'état parallèle.

La vue par mod d'un seul mod existe déjà : c'est l'onglet **Ajouts au jeu** de sa fiche (`SPEC.md` §4.5.5). Cet écran en est la version globale et en reprend le vocabulaire, les couleurs et le regroupement par dossier de destination.

## R4 — Le contenu d'origine est là, mais écrasé

Le contenu intact de Kunos et les fichiers que personne ne réclame **restent dans l'arbre**, sinon la recherche mentirait (« est-ce que j'ai la F2004 ? » doit répondre oui). Mais ils sont **regroupés en une ligne repliée par dossier** (§3.3), en gris. Ce qui compte, ce que Pit Box a posé ou ce qui a dérivé, garde seul la lumière.

---

# 3. Place dans l'app et disposition

## 3.1 Place dans le rail : l'Atelier se partage en deux

L'écran est le quatrième onglet d'une nouvelle entrée de rail, **Fichiers**. Il vient avec un découpage de l'Atelier décidé en même temps (2026-09-26) :

| Entrée de rail | Onglets | Ce qu'elle gère |
|---|---|---|
| **Classement** | Règles · Marques · Familles · Pays | comment la bibliothèque se range : règles de tags, termes, correctifs |
| **Fichiers** | Importer · Profils · Maintenance · **Dossier du jeu** | ce qui est sur le disque : entrée des mods, activation en masse, réparation, état du jeu |

Le rail devient : Session, puis Apps · Compléments · Classement · Fichiers, puis en pied Réglages · Content Manager · À propos. **L'Atelier disparaît.**

- **Pourquoi ce découpage.** L'Atelier réunissait deux métiers sans rapport : ranger la bibliothèque (des règles, aucune écriture sur le disque) et gérer les fichiers (import, profils, réparation). L'écran Dossier du jeu aurait été un huitième onglet, au-delà de la limite de sept (`SPEC.md` §7.2quater). Deux entrées de quatre onglets chacune règlent les deux problèmes.
- **Mécanique inchangée.** Chaque entrée est un écran à onglets comme l'était l'Atelier : l'onglet **est** la section (`rules`, `brands`, `categories`, `countries` d'un côté ; `import`, `profiles`, `maintenance`, `gamefolder` de l'autre), les `requestSection("import")` existants atterrissent toujours au bon endroit, l'entrée de rail repart du premier onglet, l'onglet actif n'est pas mémorisé.
- **Pas de double niveau d'onglets.** L'écran Dossier du jeu n'a pas de sous-onglets : ses filtres et sa recherche sont des contrôles, pas une navigation (`SPEC.md` §7.2quater).
- **À reporter ailleurs en même temps que le code** : `SPEC.md` §7.2 et §7.2quater (le rail, l'Atelier), la carte des écrans de `CLAUDE.md`, les libellés du rail. Les libellés « Classement » et « Fichiers » sont une proposition : à trancher avant de toucher aux fichiers de langue.
- **Hors zone session** : la colonne de session est masquée hors de la zone session, l'écran prend toute la largeur.
- **Pas de pastille d'alerte au lot 1.** Une dérive n'est connue qu'après un scan, et le scan ne tourne qu'à l'ouverture de l'écran. Une pastille qui s'allume seulement quand on est déjà dessus ne sert à rien. À reconsidérer si le scan devient périodique (§11).

**Icônes** : un dossier pour Fichiers, une étiquette pour Classement, pris dans le jeu d'icônes du rail. La clé de l'Atelier se retire avec lui.

## 3.2 Autres portes d'entrée

| D'où | Geste | Arrivée |
|---|---|---|
| Fiche d'un mod, onglet Ajouts au jeu | lien « Voir dans le dossier du jeu » en pied d'onglet | l'écran, puce Provenance posée sur ce mod |
| Fiche d'un mod, menu ⋮ | « Voir dans le dossier du jeu » | idem |
| Onglet Maintenance (voisin) | lien « Voir les dérives » sous la réparation générale | l'onglet Dossier du jeu, puce État = dérive |

Toutes passent par `openInSection` (`SPEC.md` §7.2bis), pour que « précédent » ramène à la fiche.

## 3.3 Disposition

```
┌─ Fichiers ───────────────────────────────────────────────────────────────┐
│ Importer · Profils · Maintenance · [Dossier du jeu]                      │
│ D:\Steam\…\assettocorsa        Scanné il y a 2 min · 412 318 entrées  ⟳ │
│ ┌ bilan ────────────────────────────────────────────────────────────────┐ │
│ │ Posé 38 204 · Remplace le jeu 23 · En attente 4 · Zone CM 61 ·        │ │
│ │ Dérive 7 · À personne 373 019                                         │ │
│ └───────────────────────────────────────────────────────────────────────┘ │
│ [⌕ Mod, dossier ou fichier…      ] [+ Filtre] (puces posées…)            │
├───────────────────────────────────────────────┬──────────────────────────┤
│ ▾ content                                     │  PANNEAU DE DÉTAIL        │
│   ▾ cars                                      │  (le nœud sélectionné)    │
│     ▸ rss_gtm_lanzo_v10   RSS GTM Lanzo  ●    │                           │
│     ▸ 42 voitures d'origine, intactes         │                           │
│   ▸ tracks                                    │                           │
│ ▸ extension                                   │                           │
│ ▸ apps                                        │                           │
└───────────────────────────────────────────────┴──────────────────────────┘
```

Trois bandes et deux colonnes :

1. **En-tête** : titre `.lbl-screen` de l'entrée (« Fichiers »), rangée d'onglets, puis le sous-titre propre à l'onglet (`.lbl-sub`), le chemin racine du jeu en `.mono` et l'état du scan à droite (§5.4).
2. **Bilan** : les décomptes par état (§4.2). Chaque décompte **est** le raccourci de la puce correspondante : un clic pose « État = … », un second la retire. Il répond à la première moitié de la question « qu'est-ce que Pit Box a mis dans mon jeu ».
3. **Barre de recherche et de filtres** : champ de recherche, bouton `+ Filtre`, puces posées. Même composant que la barre de filtres de la bibliothèque, mêmes gestes.
4. **Arbre** à gauche (colonne souple), **panneau de détail** à droite (360 px fixes). Le panneau suit la sélection ; il n'est jamais vide (sans sélection, il montre la racine).

Sous 1 100 px de largeur, le panneau passe **sous** l'arbre au lieu d'à côté.

---

# 4. Ce qu'un chemin peut dire

## 4.1 Population : ce qu'est le chemin

Déterminée au scan, dans cet ordre (le premier qui répond gagne) :

| # | Population | Signal | Exemple |
|---|---|---|---|
| 1 | **Livrée, app, autre mod** (junction) | reparse point de type junction, cible résolue en bibliothèque | `content/cars/x/skins/red`, `apps/lua/RSS_Settings` |
| 2 | **Dossier d'un mod** | ancêtre le plus proche portant `.pitbox-deployed.json` | `content/cars/rss_gtm_lanzo_v10/…` |
| 3 | **Son de voiture remplacé** | `content/cars/<id>/sfx/` d'une voiture dont un son est actif | `content/cars/ks_bmw_m3_e30/sfx/` |
| 4 | **Ajout au jeu** | chemin présent dans `extra_links` (ajouts et « autres mods ») | `extension/textures/common/rss/…` |
| 5 | **Contenu d'origine** | dossier de 1er niveau de `content/cars` ou `content/tracks` dont l'id est dans `kunos_content_dates.json` (`kunos_dates::is_official`) | `content/cars/ks_ferrari_f2004` |
| 6 | **Mod non géré** | même niveau, id inconnu de la table, entité `is_unmanaged` en base | `content/cars/vrc_formula_na_2021` |
| 7 | **Reste** | tout le reste | `extension/config/tracks/loaded/…`, `system/…`, `dwrite.dll` |

Les populations 1 à 4 sont **ce que Pit Box a posé**. Les populations 5 à 7 sont **à personne** du point de vue de Pit Box, sauf dérive détectée (§4.3).

**Les junctions ne sont jamais traversées.** Leur cible est lue (reparse point) et rapportée, pas parcourue : elle est en bibliothèque, et la parcourir compterait deux fois les fichiers et ferait apparaître la bibliothèque dans l'arbre du jeu.

## 4.2 État : ce que Pit Box en sait

Un **état principal** par fichier, parmi cinq, plus **deux marques** qui s'y ajoutent.

| État | Couleur (existante) | Sens | Condition |
|---|---|---|---|
| **Posé** | vert `--green` | fourni par un mod, conforme | population 1–4, et le fichier sur disque est celui du fournisseur (§4.3) |
| **Remplace le jeu** | rouge `--rosso-bright` | un fichier d'origine a été remplacé ; l'original est sauvegardé | ligne dans `game_backups` pour ce chemin, ou `sfx/` d'un son actif |
| **En attente** | jaune `--yellow` | un fichier étranger occupe la place ; l'exemplaire du mod attend en bibliothèque | réclamé, non fourni, `held_by_foreign_file` |
| **Dérive** | jaune `--yellow`, glyphe ≠ | le disque ne correspond plus à ce que Pit Box a posé | §4.3 |
| **À personne** | gris `--muted` | Pit Box n'y est pour rien et n'y touche pas | population 5–7 sans dérive |

| Marque | Couleur | Sens |
|---|---|---|
| **Partagé** | aucune, texte « +N » | plusieurs mods réclament le chemin ; un seul exemplaire gagne (arbitrage par date, `SPEC.md` §4.5.4) |
| **Zone Content Manager** | bleu `--blue` | le chemin est dans une zone que CM resynchronise (`acpath.rs`) ; CM peut remplacer l'exemplaire du mod |

**Pas de couleur nouvelle.** Ce sont exactement celles de l'onglet Ajouts au jeu : rouge = remplace le jeu, jaune = alerte, bleu = information, vert = acquis. Dérive et en attente partagent le jaune et se distinguent par leur libellé et leur glyphe. Le rouge employé ici est un **état**, pas un accent : il ne tombe pas sous le barème de `SPEC.md` §7.2ter, comme les marques rouges de l'onglet Ajouts au jeu n'y tombent pas.

**« Remplace le jeu » l'emporte sur « posé »** : un fichier fourni par un mod **et** qui a remplacé un original est « remplace le jeu ». C'est le seul état qui touche toutes les sessions, il doit se voir en premier.

## 4.3 Les dérives

Une dérive est un écart entre ce que la base dit avoir posé et ce que le disque contient. Quatre sortes, chacune avec son libellé dans le panneau :

| Sorte | Condition | Cause probable | Où corriger |
|---|---|---|---|
| **Modifié depuis la pose** | fichier fourni, mais taille ou date ≠ celles de l'exemplaire de bibliothèque | un outil l'a réécrit ; dans une zone CM, c'est presque toujours CM | Maintenance › Réparation générale |
| **Manquant** | fichier réclamé et fourni, ou attendu dans la composition d'un mod actif, absent du disque | suppression à la main, copie de la bibliothèque sans le jeu | Maintenance › Réparation générale |
| **Orphelin** | dossier marqué `.pitbox-deployed.json` d'un mod inactif ou supprimé ; junction vers une cible de bibliothèque qui n'est plus projetée ; fichier identique (chemin relatif, taille, date) à un fichier d'`extras/` que plus aucun mod ne réclame | app tuée entre deux étapes, base restaurée | Maintenance › Nettoyage |
| **Junction cassée** | junction dont la cible n'existe plus | bibliothèque déplacée, copiée sans les junctions | Maintenance › Réparation générale |

**La comparaison est celle de `SPEC.md` §4.5.4 règle 4** : taille + date contre l'exemplaire de bibliothèque. Elle couvre le hardlink (entrée MFT partagée, identiques par construction) comme le repli en copie. Pas de hachage : sur des centaines de milliers de fichiers, il ferait du scan une opération de plusieurs minutes pour un gain nul dans les cas réels.

**Un fichier présent dans un dossier de mod et absent de sa composition** (ajouté après la pose, par exemple une vignette régénérée par CM) n'est **pas** une dérive au lot 1 : il est « à personne » dans un dossier posé. En faire une dérive signalerait chaque aperçu de showroom.

## 4.4 État d'un dossier

Un dossier n'a pas d'état propre : il porte le **décompte des états de ses fichiers**. La ligne de l'arbre en montre au plus deux, par ordre de gravité, sous forme de pastilles chiffrées :

**Dérive > Remplace le jeu > En attente > Zone CM > Posé > À personne**

Un dossier qui ne contient que du « posé » montre une seule pastille verte sans chiffre. Un dossier qui ne contient que de l'« à personne » n'en montre aucune. Le panneau de détail donne la décomposition complète (§8.2).

## 4.5 Le regroupement « à personne »

Dans chaque dossier, les enfants **entièrement à personne** (le nœud et tout son sous-arbre, sans dérive) sont remplacés par **une seule ligne repliée**, placée après les autres enfants :

| Cas | Libellé |
|---|---|
| dossiers d'origine sous `content/cars` | `42 voitures d'origine, intactes` |
| dossiers d'origine sous `content/tracks` | `21 circuits d'origine, intacts` |
| autres | `1 240 éléments à personne` |

Déplier la ligne montre ses membres en place, en gris, triés par nom. Le repli est **par dossier et non mémorisé** : c'est un état d'affichage, pas une préférence.

**Les mods non gérés ne sont pas regroupés.** Ils sont « à personne » pour Pit Box, mais ce sont des mods de l'utilisateur, et « est-ce que j'ai tel mod » doit les montrer. Ils gardent leur ligne, avec la mention « non géré ».

**Une puce État posée désactive le regroupement** : si l'utilisateur demande l'« à personne », il veut le voir.

---

# 5. Le scan

## 5.1 Ce qu'il lit

| Source | Ce qu'on en tire |
|---|---|
| **Disque, dossier du jeu** | arborescence complète ; pour chaque entrée : type (fichier, dossier, junction), taille, date de modification ; cible des junctions ; présence de `.pitbox-deployed.json` |
| **Base** | `extra_links` (réclamations, fournisseur `provided`), `game_backups`, `forced_extras`, mods actifs et leur version déployée, couches actives, `sub_mods` (livrées, sons actifs), apps, autres mods, `is_stock` / `is_unmanaged` |
| **Bibliothèque** | taille et date des exemplaires attendus (pour la comparaison §4.3), arbre `extras/` pour la détection des orphelins, plan de composition des mods actifs (`deploy.rs` sait le calculer sans écrire) |
| **Code** | `kunos_content_dates.json`, liste des zones CM (`acpath.rs`) |

## 5.2 Comment

- **En fond** : `async` + `spawn_blocking`, comme l'import et la réparation générale. Jamais sur le thread principal : c'est exactement ce qui a gelé la réparation générale.
- **Sans suivre les junctions** (`symlink_metadata`, jamais `metadata`), et en repérant les junctions comme le fait déjà `activation.rs` : ni fichier ni dossier pour la bibliothèque standard.
- **Progression dans l'écran**, pas dans la pile de notifications : on est devant. Compteur d'entrées lues, pas de pourcentage ni de temps restant (le total n'est pas connu d'avance). Quitter l'écran n'annule pas le scan ; revenir retrouve le résultat.
- **Affichage progressif** : l'arbre est utilisable dès la fin du parcours du disque ; les états se remplissent ensuite. Pendant cette seconde phase, les pastilles sont en gris neutre et le bilan affiche « classement… ».

## 5.3 Ce qu'il garde

Un **index en mémoire** côté Rust, dans l'état de l'app : un tableau de nœuds (parent, nom, type, taille, date, population, état, marques, fournisseur, sorte de dérive), les noms internés. Ordre de grandeur : 500 000 nœuds, quelques dizaines de Mo. Les décomptes par état de chaque dossier sont **calculés au scan** et stockés sur le nœud : l'arbre les affiche sans reparcourir.

**Rien n'est persisté au lot 1.** Pas de table, pas de fichier de cache. L'index vit le temps de la session de l'app.

**Les tailles sont collectées dès le lot 1**, puisqu'elles sont lues de toute façon (§5.1) : elles sont dans l'index, pas encore à l'écran. Le lot 2 n'aura qu'à les montrer (§10).

## 5.4 Quand il tourne

| Moment | Comportement |
|---|---|
| Première ouverture de l'écran dans la session | scan automatique |
| Réouverture, index frais | index réutilisé, instantané |
| Réouverture, index **périmé** | scan automatique |
| Bouton **Rescanner** | scan, à tout moment. Il ne fait que lire, il a donc sa place sur un écran en lecture seule |

**Périmé** veut dire : une opération de Pit Box qui écrit dans le jeu a eu lieu depuis le scan (activation, désactivation, import, suppression, réparation, application d'un profil, bascule de son). Un compteur de génération incrémenté par ces opérations suffit ; l'écran compare. Une écriture **extérieure** (CM, l'utilisateur) n'est pas détectée : c'est le rôle du bouton, et de l'heure affichée.

L'en-tête dit toujours de quand date ce qu'on regarde : `Scanné il y a 2 min · 412 318 entrées`. Si l'index est périmé pendant qu'on est sur l'écran (une activation lancée depuis la pile de notifications), un bandeau `.warnbox` le dit et propose **Rescanner**, sans rescanner tout seul sous les yeux de l'utilisateur.

---

# 6. Filtres

Même barre que la bibliothèque : `+ Filtre` ouvre l'éditeur, une puce posée se retire d'une croix, « Tout effacer ». Les filtres s'appliquent **à l'arbre et aux résultats de recherche**.

## 6.1 État

Valeurs : Posé · Remplace le jeu · En attente · Dérive · À personne · Zone CM · Partagé. Plusieurs valeurs = **OU**. Les décomptes du bilan (§3.3) posent et retirent ces valeurs.

Un filtre d'état **élague** l'arbre : ne restent que les fichiers dans l'état demandé et leurs dossiers ancêtres. Les ancêtres gardés pour le chemin sont affichés normalement ; leurs pastilles ne comptent que ce qui passe le filtre.

## 6.2 Provenance

Un mod, une app, une livrée, un son, un autre mod. Valeur unique. Élague l'arbre à ce que l'élément **réclame ou fournit** : son dossier déployé, ses ajouts au jeu, ses junctions, son `sfx/`. Un fichier partagé qu'il réclame sans le fournir **reste affiché**, marqué « fourni par <autre mod> » : c'est la réponse à « pourquoi mon exemplaire n'est pas celui qui est posé ».

La puce se pose depuis la recherche (« Filtrer sur ce mod »), le panneau de détail, ou une porte d'entrée (§3.2). L'éditeur de la puce propose les éléments par nom, avec la même recherche que §7.

## 6.3 Ce qui n'est pas un filtre

**Population** n'est pas proposée comme filtre au lot 1. Elle se lit sur chaque ligne et dans le panneau, et les états couvrent les quatre questions. À ajouter si l'usage le demande.

## 6.4 Mémoire

Filtres, recherche et nœud sélectionné sont **enregistrés** (`ui_prefs.json`, comme les filtres de bibliothèque) et restaurés au montage, comme l'inventaire (`SPEC.md` §7bis) : ouvrir une fiche depuis le panneau puis revenir doit retrouver l'écran tel qu'on l'a laissé. Les dépliages de l'arbre, eux, ne sont pas enregistrés (sauf le chemin menant au nœud sélectionné, qui est redéplié).

---

# 7. Recherche

## 7.1 Ce qu'elle cherche

Deux sources, interrogées ensemble :

- **Les chemins de l'index** (§5.3) : chaque nom de fichier et de dossier, et le chemin complet relatif à la racine du jeu.
- **Les éléments de la bibliothèque** : voitures, circuits, apps, livrées, sons, autres mods, par **nom affiché** (celui de l'app, repris à la main compris) et par **id**. Y compris ceux qui ne sont **pas posés**.

C'est la deuxième source qui rend la recherche utile pour « est-ce que j'ai tel mod » : `lanzo` doit trouver « RSS GTM Lanzo V10 » sans savoir que son dossier s'appelle `rss_gtm_lanzo_v10`, et doit le trouver aussi quand il est désactivé.

## 7.2 Comment elle compare

- Insensible à la **casse** et aux **accents**.
- Par **sous-chaîne**. Plusieurs mots séparés par des espaces : **tous** doivent apparaître (ET), dans n'importe quel ordre, dans le nom ou le chemin.
- `/` ou `\` dans la saisie : la recherche porte sur le **chemin complet**, et les deux séparateurs sont équivalents (`skins/red` trouve `content\cars\x\skins\red`).
- Deux caractères minimum. Délai de frappe de 150 ms. Recherche côté Rust, sur l'index en mémoire : un parcours linéaire de 500 000 noms tient sous les 50 ms, pas besoin d'index de texte.

## 7.3 Les résultats

Saisir du texte remplace l'arbre par la **liste de résultats**, comme la bibliothèque passe de l'index à la liste (`INDEX§3`). Vider le champ ramène l'arbre, tel qu'il était.

Trois groupes, dans cet ordre, chacun limité à 50 lignes avec « Voir les N autres » :

| Groupe | Une ligne montre | Tri |
|---|---|---|
| **Mods** | nom, type (voiture, circuit, app, livrée, son, autre mod), **présence** (§7.4), id en `.mono` | correspondance au début du nom d'abord, puis alphabétique |
| **Dossiers** | nom, chemin parent en `.mono` gris, pastilles de décompte (§4.4) | idem |
| **Fichiers** | nom, chemin parent, pastille d'état, fournisseur | idem |

La correspondance est **surlignée** dans le nom et dans le chemin. Le décompte de chaque groupe est affiché dans son titre ; les groupes vides sont omis.

## 7.4 La présence d'un mod

C'est la ligne qui répond à la question. Quatre réponses, mutuellement exclusives :

| Présence | Pastille | Condition |
|---|---|---|
| **Dans le jeu** | verte | géré, actif, dossier présent |
| **Dans la bibliothèque, désactivé** | grise, « désactivé » | géré, inactif |
| **Dans le jeu, non géré** | grise, « non géré » | `is_unmanaged` |
| **Contenu d'origine** | grise, « origine » | officiel Kunos |

Un mod géré et actif dont le dossier manque est « dans le jeu » **avec** une pastille de dérive : l'écart est justement ce qu'il faut voir.

**Aucun résultat** se dit en entier : `Rien ne correspond à « lanzo », ni dans le dossier du jeu, ni dans la bibliothèque.` C'est une réponse, pas un écran vide : « non, tu ne l'as pas ».

## 7.5 Choisir un résultat

| Résultat | Geste principal | Effet |
|---|---|---|
| Dossier, fichier | clic, ou A à la manette | bascule sur l'arbre, **déplié jusqu'au nœud**, nœud sélectionné et amené à l'écran, panneau à jour |
| Mod présent dans le jeu | idem | bascule sur l'arbre, sélectionne son dossier (ou sa junction) |
| Mod absent du jeu (désactivé) | idem | pas de nœud à montrer : le panneau montre la fiche résumée du mod et le lien « Ouvrir la fiche » |

**Le texte de la recherche reste dans le champ** après le basculement, et un lien `← Résultats (37)` au-dessus de l'arbre ramène à la liste sans la recalculer. Effacer le champ, lui, quitte le mode recherche. C'est la même raison qu'à l'inventaire : le geste auquel la recherche sert ne doit pas la détruire.

Chaque ligne de mod porte en plus, à droite, **« Filtrer »**, qui pose la puce Provenance (§6.2) et rend l'arbre élagué.

---

# 8. L'arbre et le panneau de détail

## 8.1 Une ligne de l'arbre

```
▸ [icône] nom                          fournisseur (+2)     [pastilles]
```

| Élément | Contenu |
|---|---|
| Chevron | dossiers seulement |
| Icône | dossier · fichier · **junction** (flèche de lien) · dossier de mod (dossier marqué) |
| Nom | `.mono`, 12 px ; gris `--muted` pour l'« à personne » |
| Mention | « non géré », « origine » en `.lbl-key` après le nom, quand elle s'applique |
| Fournisseur | nom affiché du mod qui fournit, `--txt2` ; « +N » quand d'autres réclament |
| Pastilles | fichier : son état ; dossier : jusqu'à deux décomptes (§4.4) |

Tri : dossiers puis fichiers, puis alphabétique. Les lignes de regroupement (§4.5) viennent en dernier. Hauteur de ligne : celle des listes de l'inventaire. Les dossiers ne se déplient pas tout seuls, sauf le chemin menant à une sélection.

**Le très gros dossier** : un dossier de plus de 500 enfants affichés les charge par tranches de 500, avec « Afficher les 500 suivants ». L'arbre est **virtualisé** (seules les lignes visibles existent dans le DOM) ; l'index Rust sert les enfants d'un nœud à la demande (§9.2).

## 8.2 Le panneau de détail

Il répond à « d'où vient ceci, et qu'est-ce que ça veut dire ». Toujours dans cet ordre, en blocs `.blk` ; un bloc sans objet n'est pas affiché.

**En-tête** : nom, chemin complet relatif au jeu en `.mono` (sélectionnable), pastille d'état, et **une phrase** qui dit l'état en clair. Exemples :

- *Posé* : « Fourni par RSS GTM Lanzo V10. Hardlink vers la bibliothèque. »
- *Remplace le jeu* : « Remplace un fichier d'Assetto Corsa. L'original est sauvegardé et reviendra quand plus aucun mod ne réclamera ce chemin. »
- *En attente* : « Un fichier que Pit Box n'a pas posé occupe ce chemin. L'exemplaire de RSS Formula Hybrid 2023 attend en bibliothèque. »
- *Dérive, modifié* : « Ce fichier a changé depuis que Pit Box l'a posé (taille : 14 832 → 15 107 octets). Dans cette zone, c'est généralement Content Manager. »
- *À personne* : « Ni Pit Box ni un mod de la bibliothèque n'ont posé ce fichier. Pit Box n'y touche pas. »

**Mécanisme** : hardlink vers `<chemin bibliothèque>` · junction vers `<cible>` · remplacement du contenu, original dans `game_backup/…` · fichier réel.

**Provenance** : le fournisseur, puis **les autres réclamants**, chacun avec la raison pour laquelle il ne gagne pas (« exemplaire plus ancien, 12/03/2024 »), ou « autorisé explicitement » (`forced_extras`). Chaque nom est un lien vers sa fiche.

**Revenir en arrière** (état « remplace le jeu » seulement) : ce qui ramènerait l'original, dit en une phrase, sans bouton d'action. « Désactiver RSS GTM Lanzo V10 et RSS GTM Forza V8 restaure l'original. » Suivi du lien vers la fiche du premier. C'est la réponse à la question 3 de §1, et c'est tout ce que la lecture seule permet.

**Dérive** : la sorte (§4.3), ce qui diffère, la cause probable, et le lien **« Aller à la Maintenance »**.

**Pour un dossier** : la décomposition complète par état (tous les décomptes, pas seulement deux), puis **les mods qui y contribuent**, triés par nombre de fichiers, cinq au plus avec « et N autres ». Pour un dossier de mod : la version déployée et ses couches actives.

**Pied du panneau**, trois navigations, jamais plus :

- **Ouvrir la fiche** (quand un mod est en cause)
- **Filtrer sur ce mod** (pose la puce Provenance)
- **Afficher dans l'Explorateur** (`explorer /select,` sur le chemin ; pour une junction, le chemin de la junction, pas sa cible)

## 8.3 Manette et clavier

Rien ne se lit au seul survol (règle de l'app) : tout ce qu'une ligne ne montre pas est dans le panneau, et le panneau suit la **sélection**, pas le pointeur.

| Geste | Clavier | Manette |
|---|---|---|
| Ligne précédente / suivante | ↑ ↓ | croix haut / bas |
| Déplier / aller au premier enfant | → | croix droite |
| Replier / aller au parent | ← | croix gauche |
| Activer (déplier un regroupement, choisir un résultat) | Entrée | A |
| Aller à la recherche | Ctrl+F, `/` | Y (clavier virtuel de l'app) |
| Passer au panneau de détail | Tab | RB, puis la croix parcourt ses liens |
| Retour | Échap, bouton précédent de la souris | B (`goBackOr`, `SPEC.md` §7.2bis) |

À reprendre de `gamepadNav.ts` et des raccourcis de `SPEC.md` §7.4bis ; en cas de conflit avec un raccourci existant, l'existant gagne et ce tableau se corrige.

---

# 9. Implémentation

## 9.1 Backend

Un module **`gamestate.rs`**, qui ne dépend que de lectures :

| Fonction | Rôle |
|---|---|
| `scan(cfg, db) -> Index` | §5, en deux phases (disque, puis classement) avec événements de progression |
| `Index::children(node, filters) -> Vec<Row>` | les enfants d'un nœud, regroupements (§4.5) appliqués, décomptes filtrés |
| `Index::detail(node) -> Detail` | tout le panneau (§8.2) |
| `Index::search(query, filters) -> Results` | §7, chemins et éléments de bibliothèque |
| `Index::reveal(path) -> Vec<NodeId>` | la chaîne des ancêtres, pour déplier jusqu'à un nœud |

Façades dans `commands/gamestate.rs`, inscrites dans `invoke_handler![…]`, bindings dans `src/lib/gamestate.ts` (CLAUDE.md, « trois endroits »).

**Réutiliser, ne pas recopier** : la liste des dossiers de jeu et des zones CM (`acpath.rs`), la table officielle (`kunos_dates::is_official`), le plan de composition (`deploy.rs`, `compose.rs`, sans écriture), la détection de junction (`activation.rs`), la lecture des réclamations (`extras.rs`), des sauvegardes (`gamebackup.rs`). S'il faut exposer une fonction de ces modules en lecture seule, l'exposer ; ne pas réécrire sa logique ici, sinon l'écran finira par dire autre chose que ce que fait le moteur.

**Garde-fou de lecture seule** : le module n'importe aucune fonction qui écrit. Un test le vérifie en listant ses `use`.

**Compteur de génération** (§5.4) : un `AtomicU64` dans l'état de l'app, incrémenté par les opérations qui écrivent dans le jeu. L'index retient la génération à laquelle il a été construit.

## 9.2 Frontend

- Composant `gamestate/GameFolder.svelte`, section `gamefolder` dans `nav.svelte.ts`, quatrième onglet de l'écran Fichiers (issu de `Workshop.svelte`, §3.1).
- Arbre virtualisé ; enfants chargés à la demande par `children`, jamais l'arbre entier côté webview.
- La barre de filtres est **le composant de la bibliothèque**, pas une copie (CLAUDE.md, composants partagés).
- Libellés dans les fichiers de langue ; `npm run check` vérifie les clés.

## 9.3 Tests

Un dossier de jeu de fixture, construit par le test, qui contient au moins un cas de chaque ligne des tables §4.1, §4.2 et §4.3, plus : un fichier partagé par deux mods, une junction cassée, un dossier de 600 enfants (tranches), un nom accentué (recherche). Le test vérifie la population, l'état et la sorte de dérive de chaque chemin, et que le disque est **identique octet pour octet** avant et après scan.

---

# 10. Lot 2 — les tailles

Hors du lot 1, mais préparé par lui (§5.3). À spécifier en détail le moment venu ; ce qui est déjà tranché :

- **Deux poids, jamais un seul.** Un hardlink ne coûte rien de plus sur le disque. Le **poids apparent** (ce que l'Explorateur affiche, la somme des tailles) et le **poids réel** (ce que le dossier du jeu coûte en plus de la bibliothèque : fichiers réels, copies de repli, originaux remplacés) sont deux chiffres différents, et les confondre ferait croire que Pit Box duplique 300 Go.
- Le poids réel d'un fichier se décide par sa **population** (§4.1) : hardlink de bibliothèque et junction comptent zéro ; copie de repli, fichier réel et « à personne » comptent leur taille. Le nombre de liens NTFS n'est pas nécessaire.
- Colonne de poids dans l'arbre, tri par poids, et poids dans le bilan : « Pit Box a posé 38 204 fichiers, 212 Go apparents, 0,4 Go réels ».

# 11. Hors périmètre, à rediscuter plus tard

- **Toute action d'écriture** : désactiver depuis l'arbre, poser quand même, restaurer un original, nettoyer une dérive. Si elles arrivent, elles réutiliseront les commandes existantes de la fiche et de la Maintenance, jamais une logique propre à cet écran.
- **Scan périodique** ou surveillance du dossier (`ReadDirectoryChangesW`), et la pastille de rail qui irait avec.
- **Index persisté** entre deux lancements.
- **Filtre Population** (§6.3).
- **Comparaison par hachage** (§4.3).
