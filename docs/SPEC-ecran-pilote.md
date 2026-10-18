# Pit Box — écran Pilote

## Spécification UX/UI

*Choisir le pilote qu'on voit au volant : son corps (le mannequin 3D) et sa tenue en trois pièces. Ce document est la seule description de l'écran ; ce que l'aperçu 3D fait du pilote (assise, bras, braquage) est au PREVIEW§4.6 et PREVIEW§4.6bis, et la façon de le poser en jeu dans `csp-driver-research.md`. Maquette : `maquettes/pitbox-ecran-pilote.html`.*

---

# 1. Objet et périmètre

## 1.1 Ce que l'écran permet

Choisir le pilote que l'on voit au volant : son **corps** (le mannequin 3D) et sa **tenue** en trois pièces — casque, combinaison, gants —, l'essayer avant de l'adopter, l'enregistrer sous un nom, et le **poser en jeu**.

## 1.2 Ce qu'il ne permet pas

La position dans l'habitacle et l'animation des bras viennent de la voiture (PREVIEW§4.6bis) : elles ne se règlent pas, et ne sont pas représentées, même désactivées.

## 1.3 Asymétrie fondatrice

| | Nature | Ce qui le choisit | Comment on le remplace |
|---|---|---|---|
| **Corps** | modèle 3D | la voiture, dans `data.acd` — le conteneur qu'un serveur vérifie | une surcharge CSP posée à côté, sans toucher `data.acd` (§13) |
| **Casque, combinaison, gants** | images posées sur le corps | la livrée (`skin.ini`) | le `skin.ini` de la livrée |

Cette asymétrie **structure l'écran** : le corps au-dessus, séparé, et il commande les trois autres.

## 1.4 Conditions d'accès

**Le choix se fait par voiture**, résolu en cascade :

1. **la tenue choisie pour cette voiture** gagne toujours ;
2. sinon **la tenue par défaut de sa classe** — l'une des tenues enregistrées (§8.4), désignée par l'utilisateur ; il y en a **deux, une pour les voitures de rue et une pour les voitures de course**, parce que sur une voiture de course la tenue fait souvent partie de la livrée. « Aucune » d'un côté rend ces voitures à leur livrée. Tout ce qui n'est pas annoncé `race` compte comme rue ;
3. sinon **la livrée**.

Activer une tenue par défaut n'écrase jamais un choix fait à la main, et la désactiver ne perd rien. **Rendre une voiture à son origine est un choix comme un autre** : « le corps de la voiture » ou « tout remettre sur la livrée » s'enregistre sur cette voiture dès qu'une tenue par défaut l'habillerait — et une telle voiture ne compte pas dans le filtre « Pilote choisi ». **Aucune voiture n'est mise à part** : une voiture de course s'habille comme une autre.

Sans voiture choisie, l'écran le dit et propose d'en choisir une.

---

# 2. Décisions structurantes

**D1 — Écran dédié**, dans la zone session, au même rang que les bibliothèques : la hauteur de la colonne de session est la ressource rare.

**D2 — Survol = essai, clic = adoption.** Parcourir la galerie applique chaque option en direct sur le pilote affiché ; c'est la seule façon de juger une tenue, une texture à plat ne disant rien du résultat.

**D3 — La vignette est l'image plate d'AC, assumée comme telle** : elle sert de repère, le survol fait le jugement (§7).

**D4 — Le cadrage suit la piste active** : corps → plan large ; casque → tête ; combinaison → buste ; gants → mains.

**D5 — Aucun volant n'est dessiné.** Le pilote est posé comme sa voiture le pose (§5.1) ; un tore générique entre les mains a été essayé deux fois et abandonné.

**D6 — Substituer le corps est un mode, pas un réglage** (§10).

**D7 — Pas de bascule de regroupement** : les règles de regroupement sont fixes (§6.3).

---

# 3. Emplacement et accès

## 3.1 Rang dans la navigation

L'écran Pilote est l'un des quatre écrans de la zone session, avec Voitures, Circuits et les réglages de la session (`SPEC.md`, « Trois territoires »).

## 3.2 Points d'entrée

**La ligne `PILOTE` de la colonne de session**, sous la livrée : elle dit ce que le pilote porte — *Tenue d'origine* quand rien n'est touché, le **nom de la tenue enregistrée** quand on en porte une, *Tenue personnalisée* sinon —, plus un badge *Substitué* quand le corps n'est pas celui de la voiture. Un clic ouvre l'écran, jamais plus. Quand l'écran est ouvert, cette ligne porte le filet rouge de la zone active.

**La bibliothèque des voitures** porte un filtre à trois états « Pilote choisi ».

## 3.3 Coût en hauteur

Une ligne dans la colonne de session, à la place d'une bascule et de trois menus.

---

# 4. Anatomie de l'écran

```
┌──────────────────┬───────────────────────────────────────┐
│  ESSAYAGE        │  BARRE D'OUTILS                       │
│  (fixe)          │  recherche · favoris · récents · compteur
│  ┌────────────┐  ├───────────────────────────────────────┤
│  │  plateau   │  │  GALERIE (défilante)                  │
│  │     3D     │  │  bannière d'invalidation (si besoin)  │
│  └────────────┘  │  ── Red · HELMET_BASE_Red · 7 ──────  │
│  ligne d'état    │  ▨ ▨ ▨ ▨ ▨ ▨ ▨ ▨                      │
│  ▸ CORPS         │                                       │
│  ▸ CASQUE        │                                       │
│  ▸ COMBINAISON   │                                       │
│  ▸ GANTS         │                                       │
│  sortie          │                                       │
│  MES TENUES      │                                       │
└──────────────────┴───────────────────────────────────────┘
```

Le panneau d'essayage est **fixe** ; seule la galerie défile. Le pilote ne quitte jamais le champ de vision.

---

# 5. Le panneau d'essayage

## 5.1 Plateau 3D

Le pilote est montré **seul, sans habitacle, mais posé comme sa voiture le pose** : sa hiérarchie d'assise et son animation de braquage sont appliquées, seul l'ancrage sur les yeux tombe. **Les gestes sont ceux de l'aperçu d'une voiture** (les mêmes contrôles d'orbite) : glisser fait tourner autour du pilote sur les deux axes, le clic droit déplace dans le plan de l'écran, la molette rapproche et éloigne, le double-clic rend la vue de face et le cadrage d'origine. Le zoom s'applique par-dessus le cadrage de la piste et lui survit.

**Éclairage fixe** : une livrée sombre doit rester lisible.

## 5.2 Cadrage par piste

Changer de piste déplace la caméra, en douceur.

| Piste active | Cadrage |
|---|---|
| Corps | plan large, le pilote entier |
| Casque | tête et haut des épaules |
| Combinaison | buste |
| Gants | mains et avant-bras |

**La caméra ne recadre que sur changement de piste, jamais au survol** : une caméra qui bouge pendant qu'on compare rend la comparaison impossible.

## 5.3 Volant générique

Abandonné (D5).

## 5.4 Ligne d'état

Une bande en pied de plateau : la pastille `ESSAYAGE` ; au repos *« Survolez une vignette pour essayer · cliquez pour garder »*, au survol *« Essai — `<identifiant>` »* ; l'aide des gestes. **Nommer l'essai en cours est obligatoire** : sans cela rien ne distingue ce qu'on survole de ce qu'on a retenu.

**Adopter se voit** : le survol échange une texture, instantané, mais adopter redemande le modèle au backend, une bonne seconde pendant laquelle le mannequin précédent reste affiché, avec un badge discret (« Mise à jour… »). Sans lui, le clic ne produirait rien de visible et on cliquerait deux fois.

## 5.5 Les pistes

Quatre lignes sous le plateau, en deux blocs :

```
─── LE CORPS COMMANDE ──────────────
  ▸ CORPS            driver          celui de la voiture
─── LA TENUE EN DÉCOULE ────────────
  ▸ CASQUE           HELMET_BASE_Red / 7          100
  ▸ COMBINAISON      Celle de la livrée            53
  ▸ GANTS            Ceux de la livrée             69
```

Une piste : icône, libellé, **valeur retenue** (mono pour un identifiant de dossier, italique gris pour un défaut), compteur d'options. **Une seule piste active** à la fois, marquée par un fond éclairci et un filet gauche rouge ; elle décide du contenu de la galerie et du cadrage. La piste ouverte à l'arrivée est le **Casque**. **Une pièce retenue qui ne s'applique pas au corps courant est gardée, pas effacée** : la piste la marque ⚠ (« sans effet sur ce corps »), et elle revient sur une voiture compatible.

## 5.6 Sortie

Un lien sous les pistes, dont le sens change avec le mode :

| Mode | Intitulé | Effet |
|---|---|---|
| Corps d'origine | `TOUT REMETTRE SUR LA LIVRÉE` | les trois pièces reviennent au défaut |
| Corps substitué | `REVENIR AU CORPS DE LA VOITURE` | rétablit le corps, puis les trois pièces au défaut |

Un seul bouton, un seul chemin de retour.

---

# 6. La galerie

## 6.1 Contenu

Les options de la **piste active**, groupées, précédées au besoin de la bannière d'invalidation (§10.2). **Ce qu'on peut porter n'est ni deviné ni codé en dur** : un dossier de garde-robe est retenu s'il contient une texture que le corps échantillonne comme couleur de base. Cette règle unique donne la bonne réponse pour les trois listes : combinaisons et gants passent partout (les mannequins Kunos réclament les mêmes fichiers), les casques se filtrent d'eux-mêmes par époque.

## 6.2 Filtrage des casques par époque

**Automatique et non désactivable** : un casque hors époque ne s'appliquerait pas. Le compteur en dit toujours la cause (§8.3). Un corps de mod qui nomme ses images à lui n'a aucun casque à proposer (§11.1).

## 6.3 Regroupement

**Deux règles, selon la piste, et aucune commande pour en changer.**

**Les tenues** se rangent par **premier niveau de dossier** (`driver_helmet/<FAMILLE>/<variante>`) : une couleur, un motif, un pilote, un pack.

**Les corps** se rangent par **pack** : ce que le nom de fichier porte avant le premier `_` (`rss_`, `rh_`, `gt-m_`…), seul regroupement que les données portent. `driver`, `drivers` et `pilote` sont écartés, parce qu'ils ne disent rien : ces corps-là — la famille livrée avec le jeu — retombent sur leur **époque**.

**Trois replis, dans cet ordre** : un pack de moins de trois cases rend la main à l'époque ; un groupe qui reste sous ce seuil rejoint **Autres** (absent s'il est vide) ; **un groupe unique repasse en grille plate**.

## 6.4 En-tête de groupe

```
Chris Amon    HELMET_1969/amon    1    ────────────────────
Red           HELMET_BASE_Red     7    ────────────────────
```

Nom lisible en tête, identifiant de dossier ensuite en mono, compteur, filet jusqu'au bord. Les casques de 1969 portent le nom complet de leur pilote (Amon, Bandini, Clark, Hill, Ickx), par une table statique.

## 6.5 Case par défaut

Première position du premier groupe, toujours :

| Mode | Contenu | Libellé |
|---|---|---|
| Corps d'origine | trame diagonale + « Celui de la livrée » (« Celui de la voiture » pour le corps) | *Par défaut* |
| Corps substitué | trame diagonale + « La livrée ne prévoit rien pour ce corps » | *Aucun* |

**Le défaut est un choix parmi les autres**, qui se survole et se sélectionne comme une case ; en mode substitué, il retire la pièce.

---

# 7. L'échantillon

## 7.1 Nature

**L'image plate fournie par Assetto Corsa**, entière, sans découpe — le `.jpg` qu'AC range à côté de chaque `.dds` de garde-robe. C'est aussi elle qui sert au survol (§12.1).

## 7.2 Pourquoi pas un rendu 3D

Le survol fait le jugement ; un casque en 3D cache la moitié de sa livrée ; et un rendu paresseux au défilement scintille au moment où l'on balaie la grille. Le rendu 3D est réservé aux **corps** (§9.1), faute d'image plate qui veuille dire quelque chose.

## 7.3 Traitement visuel

L'échantillon est **lisiblement autre chose** qu'un aperçu du résultat : carré plein, filet d'un pixel, aucune ombre, saturation atténuée au repos, pleine au survol et quand il est retenu (bord rouge, §15). Le nom de dossier en mono sous l'image, tronqué. Rien n'est posé sur l'image : l'étoile de favori préfixe le nom.

## 7.4 Les trois casques sans image

Case neutre : trame diagonale, nom seul. Survolable et sélectionnable : le plateau montre le résultat.

---

# 8. Barre d'outils

## 8.1 Composition

Recherche · Favoris · Récents · compteur.

## 8.2 Recherche

Sur le nom de dossier **et** le nom lisible de famille (« clark » comme « amon »), instantanée ; les groupes vides disparaissent.

## 8.3 Compteur

```
100 casques compatibles
corps driver · époque moderne
```

Le nombre et la **cause** du filtrage ; vide : `Aucun casque applicable`. Il se replie sur plusieurs lignes plutôt que de tronquer.

## 8.4 Favoris et récents

**Deux filtres à trois états** (uniquement, sauf, sans filtre), cumulables avec la recherche. « Récents » retient les **douze derniers essais adoptés**, jamais survolés.

**Mes tenues** : sous la sortie du panneau d'essayage, enregistrer le corps et les trois pièces sous un nom, pour les reposer d'un clic ; c'est parmi elles qu'on désigne les tenues par défaut (§1.4).

---

# 9. Le corps

## 9.1 Sélection

La piste Corps ouvre la **galerie des corps**, au même format que les autres. **Ses vignettes sont des rendus 3D** — un buste, du haut du casque à la poitrine, dans l'éclairage du plateau :

- **produites à la demande, une à la fois, seulement pour ce qui entre dans le champ de vision** ; une vignette ne retarde jamais le plateau ni une autre vignette ;
- **le PNG est conservé hors du plafond du cache d'aperçus**, sous le nom de l'entrée de cache du mannequin : il se périme quand le mod change, sans invalidation à écrire ;
- **elle ne dépend que du corps** : assis par une voiture de référence du jeu de base (l'Abarth 500), dans ses propres textures, jamais par la voiture de session — sans quoi chaque voiture choisie reconvertirait la galerie. Sans Abarth 500, le corps garde sa pose de modélisation.

## 9.2 Chargement

Changer de corps demande une conversion complète. **Le corps précédent reste affiché jusqu'à ce que le nouveau soit prêt. Jamais de plateau vide.**

## 9.3 Corps écartés

Un mannequin illisible ou sans squelette (les variantes de LOD basse, par exemple) **n'apparaît pas** dans la galerie. L'écran annonce, en une ligne discrète, combien de `.kn5` de `content/driver` il a ainsi écartés — sans quoi l'absence d'un mannequin importé resterait inexpliquée.

---

# 10. Le mode « corps substitué »

## 10.1 Ce qui change

Dès que le corps n'est plus celui de la voiture, **la référence « livrée » disparaît** : la tenue de la livrée est nommée d'après l'ancien corps et n'a plus de destinataire.

| Élément | Corps d'origine | Corps substitué |
|---|---|---|
| Case par défaut | « Celui de la livrée » | « La livrée ne prévoit rien pour ce corps » |
| Pistes non choisies | *Celle de la livrée* | *Aucune* |
| Mention de la piste Corps | *celui de la voiture* | *choisi par vous* |
| Bouton de sortie | remise sur la livrée | retour au corps de la voiture |
| Ligne de la colonne de session | — | badge *Substitué* |

Le corps substitué **est posé en jeu** comme le reste (§13).

## 10.2 Bannière d'invalidation

Elle apparaît **au moment du changement**, en tête de galerie, jamais en modale : **une bannière unique** qui énonce ce qui tombe — le casque, s'il n'est pas de l'époque du nouveau corps ; la tenue de la livrée — avec « Choisir une tenue » et « Revenir au corps de la voiture ». Elle n'annonce que ce qui est réellement perdu, et se referme au premier choix ou par son second bouton.

---

# 11. États vides et cas particuliers

## 11.1 Corps sans casque applicable

Un mannequin de mod qui nomme ses images à lui : *« Ce corps porte son propre casque. »*, avec « Passer à la combinaison » et « Changer de corps ». Une propriété du mod, pas une erreur ; on ne propose jamais un choix sans effet.

## 11.2 Voiture de course

**Rien de particulier** : le corps et la tenue se posent comme sur une voiture de rue (§1.4). C'est la tenue par défaut de la classe qui permet de laisser leur livrée aux voitures de course.

## 11.3 Recherche sans résultat

Une ligne sous la barre d'outils, avec le terme et un lien d'effacement.

## 11.4 Mannequin à pièces statiques

Un mannequin dont le casque est fait de maillages **statiques** accrochés à l'os de tête, et non skinnés (`rh_schuberth_helmet_driver_19`, le seul de l'installation de référence), garde son casque de travers sur le plateau : **accepté tel quel** (2026-09-16), pour un cas sur quarante-cinq.

---

# 12. Interaction et accessibilité

## 12.1 Souris

| Geste | Effet |
|---|---|
| Survol d'une case | applique la pièce au plateau (texture échangée sur place), met à jour la ligne d'état |
| Sortie du survol | rétablit la pièce retenue, après un court délai pour éviter le clignotement entre deux cases |
| Clic sur une case | adopte, met à jour la piste, marque la case |
| Clic sur une piste | active la piste, recadre, remplace la galerie |
| Gestes sur le plateau | §5.1 |

**Deux vitesses** : le survol ne passe jamais par le backend — la texture est échangée sur place avec l'image plate —, l'adoption, si, et rétablit le rendu exact. Le corps fait exception : seul son adoption le change, par une conversion.

## 12.2 Clavier

**Le focus vaut survol** : parcourir la grille aux flèches applique chaque option au plateau ; la case visée est choisie **par sa géométrie**, ce qui traverse les groupes comme le fait l'œil. `Entrée` adopte, `Échap` rétablit le choix retenu et rend le focus à la piste, `Tab` circule entre barre d'outils, pistes et grille. Le focus reste visible.

## 12.3 Mouvement réduit

Le recadrage devient instantané ; l'application au survol reste — c'est le mécanisme central de l'écran.

## 12.4 Rendu indisponible

Si la 3D ne démarre pas, le panneau affiche en grand l'échantillon plat de la pièce retenue, avec une ligne d'explication. La galerie et la sélection restent pleinement fonctionnelles : **l'écran ne se bloque jamais sur l'absence de 3D.**

---

# 13. Persistance

| Donnée | Portée |
|---|---|
| Corps, casque, combinaison, gants | **par voiture** (`ui_prefs.json`, une clé par voiture) |
| Tenues enregistrées et tenues par défaut (rue, course) | globales |
| Favoris | globaux |
| Récents | globaux, 12 adoptions |
| Piste active | session — l'écran rouvre sur le Casque |

**Le pilote choisi est posé en jeu au lancement de la session**, et retiré dès qu'on ne le choisit plus (`driverapply.rs`). Deux fichiers, aucun dans `data.acd` : **le corps** par une section `[DRIVER3D_MODEL]` dans l'`ext_config.ini` de la voiture — CSP surcharge une section de `data.acd` sans toucher au conteneur —, **la tenue** par le `skin.ini` de la livrée, **sous le nom du mannequin**, donc réécrite sous le nom du nouveau corps quand il change. Trois disciplines, chacune obligatoire :

- **l'original est sauvegardé avant écriture et rendu à la reprise** (règle d'or n°5, `gamebackup.rs`) ;
- **seule la section concernée est remplacée**, le reste du fichier conservé au caractère près ;
- **le fichier est effacé avant d'être réécrit**, sans quoi on modifierait la copie de bibliothèque à travers le hardlink.

**Le retour en arrière ne touche que ce qu'on a écrit** : un fichier sauvegardé revient de sa sauvegarde, un fichier créé (il porte notre en-tête) est supprimé, et tout autre fichier est laissé tel quel — le nom d'une section n'est pas à nous seuls, une livrée et un auteur emploient les mêmes.

---

# 14. Textes et localisation

**Aucun texte en dur.** Ne se traduisent pas : les identifiants de dossier, les noms de corps, les noms propres de pilotes historiques. Se traduisent : libellés de piste, intertitres, noms lisibles de famille, messages d'état. Un bouton nomme ce qui se passe (« Revenir au corps de la voiture », pas « Réinitialiser ») ; un état vide donne une direction. Le compteur et le bouton de sortie se replient sur deux lignes plutôt que de tronquer.

---

# 15. Repères visuels

Le design system de l'app s'applique sans exception (jetons de `global.css`, barème du rouge, §7.2ter de `SPEC.md`). **Dans cet écran, le rouge est réservé à** la pastille `ESSAYAGE`, le filet de la piste active et le bord de la case retenue. Rien d'autre.

---

# 16. Ce qui reste ouvert

**Le nom lisible des familles modernes.** « Red » → « Rouge » suppose une table de correspondance des couleurs : faisable pour les familles `HELMET_BASE`, à confirmer pour les packs de mods.

**La galerie des combinaisons et des gants** suit la structure à deux niveaux des casques ; une famille de mod qui ne la suit pas passerait en grille plate.
