# PLAN — Refonte de la navigation et des fiches

> Plan de livraison de `SPEC-refonte-navigation-et-fiches.md`. La spec dit **quoi**,
> ce document dit **dans quel ordre, avec quoi, et ce qui a été mesuré avant de commencer**.
> En cas d'écart, la spec fait foi — et l'un des deux est à corriger tout de suite.
>
> Branche : `feature/refonte-navigation`. Deux paliers de fusion dans `main`.

---

## 1. Ce que la mesure a changé au plan

Mesuré le **2026-09-11** sur la bibliothèque réelle (`overlay.sqlite`, 335 mods dont
199 de base), parce que le §14.4 de la spec le demande explicitement et parce que
conclure sur un échantillon a déjà produit trois bugs de placement dans ce projet.

| Table | Compte |
|---|---|
| `mods` | 335 (311 voitures, 24 circuits — 199 de base) |
| `sub_mods` | 38 (26 skins, 10 sons, 2 skins de circuit) |
| `other_mods` | 28 (toutes actives, aucune prioritaire) |
| `layers` | 3, sur 3 hôtes distincts |
| `apps` | 3 |

**Trois résultats qui modifient le plan.**

**① Le lot 10 de la spec n'existe pas.** Le §7.5 affirme que la détection des extensions
CSP « n'existe aujourd'hui que pour les circuits ». C'est faux : `inspect::csp_features`
lit l'`ext_config.ini` du mod sans regarder son type, et `csp_features_loaded` a une
branche `ModKind::Car`. Mesuré : **167 voitures sur 311** portent des `csp_features` non
vides (et 24 circuits sur 24). Le lot se réduit à *démoter l'affichage existant* vers
l'onglet Installation, ce que fait déjà le lot 4. **Supprimé du plan.**

**② Le fourre-tout « le jeu » redouté au §14.4 ne se matérialise pas.** Sur les 28
mods « autres », **19 sont des modèles de pilote** (`content/driver/*.kn5`) — c'est
exactement le doublon que le §5 supprime en rangeant l'inventaire des mannequins avec
leur sélecteur, dans l'écran Pilote. Le reste : 2 extensions de son rattachées à une
voiture (`content/cars/<id>/extension`), 2 dossiers de polices, 3 notices PDF/TXT, un
`ext_config.ini` et un dossier posé à la racine d'AC.

Donc l'écran Compléments ramasse en réalité **≈ 50 lignes** (38 sub_mods + 3 couches +
9 autres), dont **41 rattachées par construction** (`parent_id`), et il reste **4 ou 5
entrées** sans rattachement déductible — pas 26 sur 61 comme le suppose la maquette.
**Aucune troisième facette n'est nécessaire.** À re-mesurer si la bibliothèque change
de nature.

**③ Les quatre signaux du §2.1 sont tous présents dans les données réelles**, y compris
les deux exemples que la spec cite de mémoire : `policeman__ext_config.ini` (signal 3,
config CSP nommée d'après une entité) et les polices RSS arrivées dans l'archive de la
voiture (signal 4, conjecture). `DORIKIN_DRIVER_MOD` — le cas du §4.5, listé deux fois
aujourd'hui — existe bien lui aussi. Le calcul du §2.1 est donc éprouvable sur cette
bibliothèque dès le lot 6, sans corpus fabriqué.

**À vérifier séparément, hors refonte** : `SOME1_NSX_2026-02-15.7z__..._content_fonts`
a sa junction sur `assettocorsa\SOME1_NSX_2026-02-15`, un dossier à la **racine** de
l'installation AC et non dans `content/fonts`. Ça ressemble à une pose ratée, pas à
un choix.

---

## 2. Décisions prises avec l'utilisateur

1. **Runner de tests front : Vitest, logique pure uniquement.** `src/lib/*.ts`, aucun
   test de composant, pas de jsdom. Ajouté à `npm run verify`. La refonte produit
   enfin la matière qui manquait : conversion d'unités (§7.6), nom de couche dérivé
   (§8.3), règles de facettes et de rattachement — en plus de `displayName.ts` qui
   attendait déjà.
2. **Ordre : les fiches d'abord, deux paliers.** L1→L5, fusion dans `main`, puis
   L6→L9. La spec le permet (§15 : « les lots 1 à 4 peuvent être livrés seuls ») et
   ça évite une branche qui diverge pendant des mois.
3. **L'écran transversal est absorbé, mais le groupement par hôte survit.**
   L'inventaire gagne « Grouper par : archive · hôte · aucun ». Le geste « voir tous
   les skins de cette voiture » ne se perd pas, et c'est le même mécanisme que le
   groupement par archive du §4.4.

---

## 3. Phase 0 — le socle, avant de toucher à l'UI

**0.a — Mesures.** Faites, §1.

**0.b — Composants partagés.** C'est le chantier « composants partagés plutôt que
styles recopiés » de CLAUDE.md, et il passe **avant** la refonte, pas après : trois
écrans neufs et quatre fiches vont écrire ces briques. Le CSS Svelte étant scopé, les
recopier maintenant, c'est les faire diverger tout de suite.

- ✅ `.errbox` globale — **21** copies locales, pas 14 : l'inventaire datait de trois
  semaines et quatre composants s'étaient ajoutés depuis. Sœur de `.warnbox`, ce qui a
  tranché sa géométrie sans avoir à arbitrer entre les copies.
- ✅ `Seg.svelte` — **7** copies. Trois axes de variation (`vertical`, `tone`, `size`
  nommée par son rôle) et rien d'autre. Le groupe zoom de la visionneuse PDF en est
  exclu : ses trois boutons sont des actions, pas un choix parmi trois.
- `Toolbar.svelte` (§10), `ListRow.svelte` (§4.2) et la coquille de fiche (§6.1) :
  **repoussés à leur premier client** (L2 et L7). Une brique sans consommateur pourrit
  comme une colonne SQL que rien n'écrit ni ne lit.
- ✅ `.lbl-sub` — **9** copies, identiques à `max-width` près, qui reste à l'appelant :
  la largeur de mesure d'un paragraphe dépend de la colonne qui l'accueille, pas du
  rôle du texte. Tranché sur la maintenabilité, l'utilisateur n'ayant pas d'avis :
  `.lbl-screen` était global et son sous-titre non, si bien que déplacer un en-tête
  n'emportait que la moitié de son style. Trois homonymes renommés au passage — `.sub`
  désignait aussi un en-tête de dialogue, un message sous un champ et un surtitre posé
  **au-dessus** de son titre.

Reformatage pur → commits isolés, jamais mélangés au fonctionnel (sinon `git blame`
devient inexploitable).

**0.c — Vitest.** ✅ Fait. `vitest.config.ts` (config séparée, sans le plugin
SvelteKit), `npm run test` enchaîné dans `npm run verify`, un fichier `*.test.ts` à
côté de son module. Neuf cas sur `withoutBrand` au démarrage — dont un rouge : la
table d'alias de `displayName.ts` s'indexait sur des clés écrites à la main
(`mercedes-benz`) quand la recherche passe par `normalise`, qui ne laisse jamais de
tiret. Toutes les Mercedes gardaient donc leur marque sur la carte. À l'œil, la table
paraît juste.

---

## 4. Les lots

| Lot | Contenu | Dépend de |
|---|---|---|
| **L1** | ✅ **Socle overlay** : `notes_user` et `display_name_user` sur les cinq tables d'entités, module `usermeta.rs`, un couple de commandes pour tous les types, binding typé. ALTER idempotents. Backend seul, aucun écran touché. `attachment_user` est **reporté en L6**, avec le code qui le calcule : une colonne que rien n'écrit ni ne lit pourrit. | — |
| **L2** | ✅ **Coquille de fiche** (§6, §12) : `FicheHeader` sur les cinq fiches, tuile réelle ou pictogramme de type (jamais deux lettres du nom), nom repris à la main partout — les DTO d'app, de son et de mod « autre » portent enfin `display_name_user` jusqu'à l'écran —, auteur au sous-titre, et « en attente » ajouté à `StateBadge`. Les rangées de boutons des quatre fiches passent dans le ⋮, comme la fiche voiture l'avait fait avant elles. | L1, 0.b |
| **L3** | ✅ **Notes** (§9) : `NoteBlock` sur les quatre fiches d'entité, recherche plein texte, filtre « Note contient… » **et** filtre booléen « A une note », colonne Note, marqueur ✎ sur la carte et sur la ligne. **L'export du §9.6 est sans objet** : il n'existe aujourd'hui aucun export/import des métadonnées de l'overlay — seulement l'export d'un mod en archive autonome, qui ne transporte pas la base. À rouvrir le jour où cet export existera. | L1, L2 |
| **L4** | ✅ (sauf §7.6 et §7.7) **Fiche voiture / circuit** (§7 + §7.8) — le gros morceau, sur `DetailPage.svelte` (2 159 lignes). ① découpage en blocs, commit de déplacement pur ② les trois onglets, blocs déplacés **sans être modifiés** (§17) ③ sélecteur compact partagé livrée/tracé ④ bloc textuel à sous-onglets, catégorie près du titre, CSP démotées. **Restent** : les unités d'origine en gris (§7.6) et l'état composé des tracés (§7.7), tous deux dans `TechSheet`/la carte Tracés. | L2, L3 |
| **L5** | **Fiche de couche** (§8) et fin du déversement de fichiers. Nom dérivé, donc faillible, donc corrigeable. Carte Ordre à partir de deux couches. | L2 |
| — | **Palier : fusion dans `main`.** | |
| **L6** | **Rattachement et nature** (§2) : module `attach.rs`, déduction par ordre de force **avec traçabilité du signal**, stockage, correction utilisateur, recalcul au réindex. Un test par signal — les quatre existent dans la bibliothèque réelle (§1③). | L1 |
| **L7** | **Écran Compléments** (§4) : facettes tri-état sur le système de jetons existant, groupement par archive **et par hôte**, « aussi dans … ». Absorbe `OtherMods` et les trois variantes de `Transversal`. | L6, 0.b |
| **L8** | **Rail à deux rangs, Apps en écran, Pilote fusionné** (§3, §5) : `NavRail` groupé, trois sections retirées de `nav.svelte.ts`, historique, inventaire des mannequins à droite de `DriverScreen`. | L7 |
| **L9** | **Fiches de mods greffés simples** (§6.2, §11) : emplacements, conflits, origine, notes — en panneau latéral. | L6 |

L'onglet « Le modèle réel » du bloc textuel (§7.4) reçoit sa place dès **L4**, vide,
pour que le chantier Wikipédia n'ait pas à rouvrir la mise en page.

---

## 4bis. Demandes venues de l'usage

- **Markdown dans les notes** (demandé le 2026-09-11, à faire dans un lot à
  part). Le §9.3 de la spec l'interdit — « un rendu à moitié interprété est
  pire que rien » — mais cet argument tombe ici : `src/lib/markdown.ts` existe
  déjà, écrit à la main pour les `readme.md` des mods, **échappe avant de
  produire la moindre balise**, et *dégrade en texte brut ce qu'il ne connaît
  pas*. C'est exactement la garantie qui manquait. Reste à décider si le rendu
  s'applique toujours (recommandé : une case « interpréter le markdown » par
  note est un réglage de plus pour une question que l'utilisateur ne devrait
  pas avoir à se poser) et ce que devient la recherche plein texte, qui doit
  continuer de porter sur la **source**, pas sur le rendu.

---

## 5. Ce qui reste à trancher

- **Le retour du panneau latéral** (§6.2). Il a été retiré du projet parce qu'il
  doublait la page pleine ; la spec le réintroduit pour des fiches qui n'ont **pas**
  de page pleine. À confirmer au moment de L9.
- **Le nom « Compléments »**, que la spec met elle-même « à confirmer » (§4).
- **`.lbl-sub`** : quatrième niveau de libellé global, ou pas.
- Les points ouverts §14.1, 14.2, 14.3, 14.5 et 14.7, à reposer **avec l'inventaire
  réel sous les yeux** — c'est ce que la spec demande, et le §1② montre que les
  hypothèses de volume de la maquette ne tiennent pas ici.
