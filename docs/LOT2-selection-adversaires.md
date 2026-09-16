# Instruction Claude Code — Lot L2 : sélection d'adversaires

> **Note ajoutée au versionnement, le reste du document est inchangé.**
>
> Ce lot est **livré**. Le document est conservé parce que le code y renvoie
> (`L2§…`) et parce qu'il porte les *arguments* derrière les choix — la
> partie qu'on ne retrouve pas deux fois. `SPEC-session.md` décrit ce que
> l'écran **est** aujourd'hui ; celui-ci dit ce qui a été demandé et pourquoi.
> En cas d'écart, `SPEC-session.md` fait foi.


Maquette de référence : `pitbox-opponent-picker-l2.html`.

**Lire d'abord ceci sur la maquette.** Elle est dessinée à l'échelle réelle — base 13 px,
zoom 100 %, barème de `global.css` respecté (`.lbl` 9 px, `.lbl-key` 8 px, contrôles 10-12 px,
body 13 px). Les zones **hachurées** sont des composants existants : elles ne sont volontairement
pas redessinées, la modale les consomme tels quels. Seul ce qui est dessiné en plein est neuf.
Les proportions et couleurs restent celles du thème ; la maquette porte l'intention, pas le
gabarit.

Spec concernée : SESSION§1 (bibliothèque comme sélecteur), SESSION§3 (écran de réglages),
§7.2ter (barème rouge), §7.4 (multi-sélection en bibliothèque).

---

## 1. Ce que règle ce lot

La modale de sélection d'adversaire actuelle n'offre qu'une recherche par nom, alors que
l'écran bibliothèque voitures offre une barre de filtres complète (Category, Brand, Tag,
Author, Country, Year, Class, State, Description, Note, Has a note) avec jetons tri-état,
favoris et compteur de résultats. Choisir un adversaire dans une bibliothèque de ~300 mods est
donc plus difficile que choisir la voiture qu'on pilote, ce qui est l'inverse du bon sens.

**Ce lot fait servir la même barre de filtres aux deux endroits**, et la pré-remplit avec le
vivier choisi dans le réglage de session.

### Périmètre

**Dans le lot :**
- la modale de sélection d'adversaire, dans ses deux contextes (ajouter / remplacer) ;
- l'extraction de la barre de filtres en composant partagé ;
- la garde « adversaires non activés » de l'écran de session (§5).

**Hors du lot, à ne pas toucher :**
- les blocs `Session type`, `Session options`, `Simulation` livrés en L1 ;
- la colonne météo et `Saved sessions` ;
- le 4ᵉ onglet de vivier, le rapport poids/puissance, la position de départ, l'agressivité —
  tout cela est L3, et L1 a déjà réservé leurs emplacements ;
- l'évolution du grip, toujours non transmise au jeu : point ouvert, traité à part, **ne pas
  y toucher au passage**.

---

## 2. Composants — à constater, pas à supposer

L'état constaté est que la modale actuelle et la liste de la bibliothèque **ne partagent
quasiment rien**. Ce lot est donc une extraction, pas du câblage.

**Aucune affirmation n'est faite ici sur les noms de fichiers, de composants ou de stores
existants.** Les identifier, puis les nommer dans le compte rendu final. Ne pas créer de
doublon d'un composant qui existerait déjà sous un autre nom.

Découpage attendu :

| Élément | Sort |
|---|---|
| Champ de recherche, menu `+ Filter`, jetons tri-état, `Favorites`, `Clear all`, compteur de résultats | **Extrait en composant partagé**, consommé par l'écran bibliothèque et par la modale |
| Liste de résultats, vue liste en tableau | **Réutilisée telle quelle**, plus une couche de sélection (§4.3) |
| Cadre, en-tête, pied, modes ajouter/remplacer | **Neuf**, propre à la modale |
| Commutateur de vues (grille, confortable, dense, liste en tableau), sélecteur de colonnes | **Absent de la modale** — une seule vue, la liste en tableau |

L'extraction ne doit **rien changer au comportement de l'écran bibliothèque**. C'est le risque
principal du lot : point de recette n°1.

**Une seule vue dans la modale : la vue liste en tableau.** On y lit et on y compare des noms,
des catégories et des années ; les vues grille (confortable, dense) montrent des previews, ce
qui est le bon service pour choisir la voiture qu'on pilote mais coûte de la place pour rien
quand on récolte des adversaires. Colonnes affichées : Brand, Model, Category, Year, State.
Pas de sélecteur de colonnes, pas de commutateur de vues.

---

## 3. Héritage du vivier

À l'ouverture, la modale pose des jetons dérivés de l'onglet de vivier actif dans le bloc
`Opponents` :

| Vivier actif | Jetons posés à l'ouverture |
|---|---|
| `Same car` | `Brand : <marque>` + `Model : <modèle>` de la voiture pilotée |
| `By category` | `Category : <catégorie choisie>` |
| `Free` | aucun jeton de vivier |

Dans les trois cas s'ajoute `State : usable` (§5.1).

**Les jetons de vivier sont retirables.** Ils portent l'accent rouge (`--rosso`,
`--rosso-border`, `--rosso-dim`) pour se distinguer d'un filtre posé à la main, mais ils ont
leur croix et se comportent exactement comme les autres. Un jeton qui ressemble aux autres et
résiste au clic abîmerait la confiance dans tout le système tri-état — et le verrou gênerait
surtout l'utilisateur avancé, au moment précis où il veut glisser une voiture hors-vivier dans
son plateau.

**Retirer un jeton de vivier ne change pas l'onglet de vivier.** L'onglet dit dans quoi le `+`
pioche ; la modale dit ce qu'on prend à la main. Les deux peuvent diverger. Conséquence : dès
qu'une voiture est ajoutée par la modale, le plateau est marqué **manuel** et n'est plus
régénéré silencieusement au changement de voiture pilotée. Reprendre le marqueur posé en L1
pour le bouton `Regenerate` ; ne pas basculer l'onglet sur `Free` d'autorité.

`Clear all` retire aussi les jetons de vivier. C'est cohérent : ce sont des filtres comme les
autres.

---

## 4. Les deux contextes

Une seule modale, un mode. Pas deux composants.

### 4.1 Ajouter

Ouverte depuis le bouton `+ Add an opponent` du plateau, ou depuis `Set as opponents` /
`Add as opponents` du clic droit en bibliothèque (§7.4 — ce chemin reste, comme raccourci).

- Sélection multiple, colonne de cases à cocher en tête de ligne.
- En-tête : `Add opponents`, puis en secondaire `· grid has 4 of 11`.
- Pied : `N selected`, `Clear selection`, puis `Cancel` et `Add the N`.
- `Add the N` est désactivé à zéro sélection.
- Validation : les voitures sont ajoutées **en fin de plateau**, dans l'ordre de la liste.
- Chaque ligne ajoutée reçoit un skin tiré comme le fait déjà le `+` du plateau, et une force
  tirée dans la fourchette de difficulté courante. Pas de nouvelle règle.
- Si le nombre d'adversaires dépasse la capacité de la piste (`pit boxes`), appliquer le
  bornage existant, ne pas en inventer un.

### 4.2 Remplacer

Ouverte par un clic sur une ligne du plateau.

- Sélection **simple**, pas de colonne de cases.
- S'ouvre **positionnée et défilée sur la voiture actuelle** de la ligne.
- En-tête : `Replace opponent 3`, puis `· currently BMW Z4 GT3`.
- Pied : rappel `Double-click a row to replace`, puis `Cancel` et `Replace`.
- Double-clic sur une ligne = valider et fermer.
- Le remplacement conserve la force de la ligne ; le skin est retiré au sort dans les skins de
  la nouvelle voiture.

### 4.3 Sélection, clavier et manette

- Ligne sélectionnée : fond `--rosso-dim` + filet gauche 2 px `--rosso`. **Jamais de fond rouge
  plein** — §7.2ter, le plein reste au seul bouton de lancement.
- Survol d'une ligne non sélectionnée : éclaircissement du gris seul, pas de rouge.
- `Espace` coche/décoche en mode ajouter, `Entrée` valide, `Échap` annule et ferme.
- `Maj+clic` étend la sélection depuis la dernière ligne cochée, en mode ajouter uniquement.
- Focus visible jaune sur chaque ligne et chaque contrôle.
- Le focus est piégé dans la modale tant qu'elle est ouverte, et rendu à l'élément déclencheur
  à la fermeture.
- Navigation manette : vérifier le parcours complet — jetons, liste, pied. Attention au test
  `offsetParent` de `gamepadNav.ts` sur les éléments en `position: fixed`.

---

## 5. Garde « adversaires non activés »

La bibliothèque montre les mods désactivés ; Assetto Corsa ne les voit pas. Composer un
plateau avec un mod désactivé produit donc une session qui échoue. Content Manager n'a pas ce
problème — il ne voit que ce qui est installé. C'est un trou propre à Pit Box, et il doit être
bouché.

### 5.1 Prévention — le jeton « jouable »

Un jeton filtrant sur les mods jouables (tout sauf désactivés, cassés, etc.) **existe déjà**
dans la barre de filtres de la bibliothèque. Ce lot ne le crée pas et ne le modifie pas : il
se contente de **l'épingler à l'ouverture de la modale**, dans les deux contextes, en plus des
jetons de vivier.

Conserver son libellé réel. La maquette écrit `State : usable` comme espace réservé ; c'est le
libellé existant qui fait foi.

Retirable comme les autres jetons. Il n'est pas épinglé dans l'écran bibliothèque, où montrer
les mods désactivés ou cassés est le comportement normal — c'est la modale qui a besoin de la
restriction, pas la bibliothèque.

### 5.2 Remède — la ligne de garde

Quand le plateau contient au moins un adversaire non activé, afficher au-dessus du bouton de
lancement, dans la colonne de session :

> ⚠ **3 opponents are not activated** — [ Activate ]

- Une seule ligne, quel que soit le nombre.
- **Le bouton de lancement est désactivé tant que la ligne est présente.** Aucune session ne
  doit pouvoir partir en échec.
- `Activate` active les mods concernés via le mécanisme d'activation existant, puis la ligne
  disparaît et le bouton se réarme.
- **Doublons comptés une fois** : trois adversaires sur la même voiture inactive font une seule
  activation, et la ligne annonce `1 opponent`, pas `3`.
- Couleur : `--warn` (`#c8b06a`), la même que l'avertissement de vivier maigre. **Pas de
  rouge** : ce n'est pas une erreur, c'est une condition réparable en un clic.

L'activation reste **explicite**. Ne jamais activer automatiquement au lancement : lancer une
course ne doit pas modifier l'état de la bibliothèque dans le dos de l'utilisateur, même si
les liens durs rendent l'opération réversible et sans coût disque.

Une garde équivalente existe déjà pour la voiture pilotée. **La réutiliser**, pas en écrire une
seconde : si elle est déjà factorisée, y brancher les adversaires ; sinon, l'étendre pour
qu'une seule ligne puisse couvrir plusieurs sujets. Vérifier au passage si le **circuit** est
couvert — si non, le signaler sans le traiter dans ce lot.

---

## 6. Persistance

Rien de neuf. Les adversaires posés par la modale suivent la persistance existante du plateau
et la règle générale : **l'état de chaque réglage est mémorisé par type de session**, les
valeurs ne se propagent jamais d'un type à l'autre.

Les filtres de la modale ne sont **pas** persistés : elle s'ouvre toujours sur les jetons
dérivés du vivier courant, jamais sur les filtres de la fois précédente. Ce sont deux plateaux
différents à deux moments différents.

---

## 7. Recette

1. **L'écran bibliothèque voitures se comporte exactement comme avant l'extraction** : tous
   les filtres, les quatre vues (grille, confortable, dense, liste en tableau), le sélecteur
   de colonnes, les favoris, le compteur. C'est le risque principal du lot.
2. Ouvrir la modale depuis chacun des trois viviers : les jetons posés correspondent au tableau
   du §3, et le compteur de résultats est cohérent.
3. Retirer `Category : #gt3` : la liste s'élargit, **l'onglet de vivier ne change pas**.
4. Ajouter une voiture hors-vivier, puis changer la voiture pilotée : le plateau **n'est pas**
   régénéré silencieusement.
5. `Clear all` retire aussi les jetons de vivier.
6. Mode remplacer : la modale s'ouvre défilée sur la voiture actuelle, le double-clic valide,
   la force de la ligne est conservée.
7. Parcours complet au clavier puis à la manette, dans les deux contextes. `Échap` ferme,
   le focus revient au déclencheur.
8. Retirer `State : usable`, sélectionner un mod désactivé, valider : la ligne de garde
   apparaît, le bouton de lancement est désactivé, `Activate` répare, le bouton se réarme.
9. Trois adversaires sur la même voiture désactivée : la garde annonce `1 opponent`.
10. Test §7.2ter à repasser sur la modale : effondrer les neutres, aucun fond rouge plein ne
    doit apparaître.
11. Vérifier la modale à zoom 125 % et sur une fenêtre étroite : le pied reste visible, la
    liste défile, rien ne déborde.

---

## 8. Compte rendu attendu

- Noms réels des composants identifiés : modale actuelle, liste bibliothèque, barre de filtres,
  store de filtres — et ce qui a été extrait.
- Libellé réel du jeton « jouable » épinglé dans la modale.
- La garde de la voiture pilotée était-elle factorisable ? Le circuit est-il couvert ?
- Toute divergence assumée par rapport à la maquette, avec sa raison.
- Les points où une affirmation de cette instruction s'est révélée inexacte — les signaler
  explicitement, comme pour L1.

Aucune modification de SPEC.md dans ce lot : la spec est mise à jour côté conception une fois
le lot recetté.
