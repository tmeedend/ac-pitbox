# Pit Box — écran Pilote

## Spécification UX/UI

*Sélection du corps et de la tenue du pilote. Fait suite au brief de conception et aux deux passes de maquette. Les chiffres proviennent de l'installation de référence de 312 voitures.*

---

# 1. Objet et périmètre

## 1.1 Ce que l'écran permet

Choisir le pilote que l'on voit au volant : son **corps** (le mannequin 3D) et sa **tenue** en trois pièces — casque, combinaison, gants.

## 1.2 Ce qu'il ne permet pas

La position dans l'habitacle et l'animation des bras ne sont pas modifiables. Elles ne sont pas représentées dans l'interface, pas même sous forme désactivée.

## 1.3 Asymétrie fondatrice

Deux natures d'objet, deux comportements :

| | Nature | Effet en course | Effet dans l'aperçu |
|---|---|---|---|
| **Corps** | modèle 3D désigné par la physique de la voiture | **immuable** — un serveur en ligne vérifie ce fichier | remplaçable |
| **Casque, combinaison, gants** | images posées sur le corps | modifiables via la livrée | modifiables |

Cette asymétrie n'est pas un détail d'implémentation à masquer : elle **structure l'écran**. Le corps est au-dessus, séparé, et il commande les trois autres.

## 1.4 Conditions d'accès

- Voitures de **rue** uniquement. Sur une voiture de course, la tenue vient de l'écurie via la livrée — voir §10.2.
- Le choix est **global** — « mon pilote », pas « le pilote de cette livrée » — et persistant.

---

# 2. Décisions structurantes

Sept décisions dont tout le reste découle. Elles sont arrêtées ; le reste de la spec les applique.

**D1 — Écran dédié.** Le choix quitte la colonne de session et prend le statut d'une rubrique de la zone principale, au même titre que la bibliothèque de voitures et celle de circuits. *Motif : la hauteur de la colonne est la ressource rare, l'arrivée du corps porte le nombre de listes à quatre, et la fonction est trop rare pour peser sur le trajet quotidien.*

**D2 — Survol = essai, clic = adoption.** Parcourir la galerie applique chaque option en direct sur le pilote affiché. *Motif : l'échange de texture coûte quelques millisecondes ; c'est le seul geste qui résout le problème du brief — une texture à plat ne dit rien du résultat — sans construire un second pipeline de rendu.*

**D3 — La vignette est l'image plate d'AC, assumée comme telle.** Elle ne cherche pas à ressembler à un casque. *Motif : le survol fait le jugement, donc la case n'a plus qu'un métier — servir de repère. L'image plate le fait mieux qu'un rendu 3D, qui cacherait la moitié de la livrée. Voir §7.*

**D4 — Le cadrage suit la piste active.** Corps → plan large ; casque → tête ; combinaison → buste ; gants → mains. *Motif : gratuit techniquement, supprime le zoom manuel, rend le changement de piste visible sans texte.*

**D5 — Les gants s'essaient sur un volant générique dessiné par l'application.** Ni mains dans le vide, ni volant de la voiture. *Motif : coût nul et constant, indépendant du nommage des mods, et honnête — on essaie une tenue, on ne prévisualise pas un habitacle.*

**D6 — Substituer le corps est un mode, pas un réglage.** Tant que le corps est celui de la voiture, la livrée est la référence de tout. Dès qu'il est substitué, cette référence disparaît et l'écran change d'état. *Motif : la tenue de la livrée est nommée d'après l'ancien corps ; substituer ne casse pas trois choix, il supprime l'option par défaut elle-même.*

**D7 — Le regroupement se nomme selon l'époque.** « Grouper par couleur / par motif / par pilote / par pack », pas « par famille ». *Motif : le filtre par époque garantit qu'on n'en voit jamais deux sortes à la fois, donc l'axe peut être nommé — et un contrôle qui dit ce qu'il fait vaut mieux qu'un contrôle générique.*

---

# 3. Emplacement et accès

## 3.1 Rang dans la navigation

L'écran Pilote rejoint Voitures et Circuits comme rubrique de la zone principale. Même largeur, même comportement de défilement, même barre d'outils en tête.

## 3.2 Points d'entrée

**Colonne de session — ligne « Mon pilote ».** Sous le sélecteur de livrée, une ligne unique de 34 px. Elle porte une icône de casque aux couleurs du casque retenu, le libellé « Mon pilote », et un badge d'état à droite :

| Badge | Condition |
|---|---|
| *(aucun)* | tout est sur la livrée |
| `MODIFIÉ` | au moins une pièce choisie, corps d'origine |
| `SUBSTITUÉ` | le corps n'est pas celui de la voiture |
| `DÉSACTIVÉ` | voiture de course |

Cliquer la ligne ouvre l'écran Pilote. Un clic, jamais plus.

**Navigation principale.** L'écran est atteignable directement, sans passer par la colonne.

## 3.3 Coût en hauteur

La colonne perd les trois menus déroulants de la version actuelle et gagne une ligne. **Bilan : −68 px.** La bascule de la version actuelle disparaît, absorbée par la ligne.

---

# 4. Anatomie de l'écran

```
┌─────────────┬──────────────────────────────────────────────────────────┐
│  COLONNE    │  BARRE D'OUTILS                                          │
│  SESSION    │  recherche · regroupement · favoris · récents · compteur │
│  (328 px)   ├──────────────────┬───────────────────────────────────────┤
│             │  ESSAYAGE        │  GALERIE                              │
│  voiture    │  (392 px, fixe)  │  (fluide, défilante)                  │
│  livrée     │                  │                                       │
│  ▸ pilote   │  ┌────────────┐  │  bannière d'invalidation (si besoin)  │
│             │  │  plateau   │  │                                       │
│  circuit    │  │     3D     │  │  ── Red · HELMET_BASE_Red · 7 ──────  │
│             │  └────────────┘  │  ▨ ▨ ▨ ▨ ▨ ▨ ▨ ▨                      │
│  boutons    │  ligne d'état    │                                       │
│             │  ─────────────   │  ── Blue · HELMET_BASE_Blue · 5 ────  │
│             │  ▸ CORPS         │  ▨ ▨ ▨ ▨ ▨                            │
│             │  ─────────────   │                                       │
│             │  ▸ CASQUE        │                                       │
│             │  ▸ COMBINAISON   │                                       │
│             │  ▸ GANTS         │                                       │
│             │  sortie          │                                       │
└─────────────┴──────────────────┴───────────────────────────────────────┘
```

Le panneau d'essayage est **fixe** ; seule la galerie défile. Le pilote ne quitte jamais le champ de vision, exactement comme la voiture ne quitte pas la colonne de session.

---

# 5. Le panneau d'essayage

## 5.1 Plateau 3D

Largeur 392 px, hauteur minimale 380 px, fond en dégradé radial du gris panneau vers le noir de fond. Le pilote est présenté en buste, mains sur le volant générique.

**Rotation.** Glisser horizontalement fait tourner le pilote autour de son axe vertical. Pas de zoom manuel — le cadrage est piloté par la piste active (§5.2). Double-clic remet la vue de face.

**Éclairage.** Fixe, trois-quarts avant-gauche. Une livrée sombre doit rester lisible : le contraste du plateau prime sur le réalisme.

## 5.2 Cadrage par piste

Changer de piste déplace la caméra. Transition de 220 ms, courbe d'accélération douce.

| Piste active | Cadrage |
|---|---|
| Corps | plan large — pilote entier, volant compris |
| Casque | tête et haut des épaules |
| Combinaison | buste, du col à la taille |
| Gants | mains et avant-bras sur le volant |

La caméra ne recadre **que** sur changement de piste, jamais au survol d'une vignette. Un cadrage qui bouge pendant qu'on compare des options rendrait la comparaison impossible.

## 5.3 Volant générique

Un tore sombre et mat, sans rayons ni moyeu, dans la matière du plateau. **Identique pour toutes les voitures.** Il donne aux doigts une géométrie de contact sans prétendre représenter l'habitacle.

Il est visible sur toutes les pistes, pas seulement sur Gants : le retirer et le remettre créerait un clignotement à chaque changement de piste.

## 5.4 Ligne d'état

Bande de 28 px en pied de plateau, sur fond semi-opaque.

| Zone | Contenu |
|---|---|
| Gauche | pastille `ESSAYAGE` en accent |
| Centre | au repos : *« Survolez une vignette pour essayer · cliquez pour garder »* — au survol : *« Essai — `<identifiant>` »* |
| Droite | affordance de rotation |

Nommer l'essai en cours est **obligatoire**. Sans cela, rien ne distingue visuellement ce qu'on survole de ce qu'on a retenu, et l'utilisateur perd son choix de vue.

## 5.5 Les pistes

Quatre lignes sous le plateau, séparées en deux blocs par des intertitres qui portent la hiérarchie :

```
─── LE CORPS COMMANDE ──────────────
  ▸ CORPS            driver          celui de la voiture
─── LA TENUE EN DÉCOULE ────────────
  ▸ CASQUE           HELMET_BASE_Red / 7          100
  ▸ COMBINAISON      Celle de la livrée            53
  ▸ GANTS            Ceux de la livrée             69
```

Deux lignes de texte remplacent une convention visuelle à apprendre. Elles sont traduites.

**Anatomie d'une piste.** Icône (20 px) · libellé en capitales espacées (9,5 px) · valeur retenue (12,5 px, mono si c'est un identifiant de dossier, italique gris si c'est une valeur par défaut) · compteur d'options à droite.

**État actif.** Fond légèrement éclairci, bordure gauche de 2 px en accent. Une seule piste active à la fois ; elle détermine le contenu de la galerie et le cadrage.

**Le bloc Corps** a en plus un sélecteur de corps replié (§9.1).

## 5.6 Sortie

Un lien centré sous les pistes, séparé par un filet. Son intitulé dépend du mode :

| Mode | Intitulé | Effet |
|---|---|---|
| Corps d'origine | `TOUT REMETTRE SUR LA LIVRÉE` | les trois pièces reviennent au défaut |
| Corps substitué | `REVENIR AU CORPS DE LA VOITURE` | rétablit le corps, puis les trois pièces au défaut |

En mode substitué, la livrée n'est pas une destination atteignable sans d'abord rétablir le corps. Un seul bouton, un seul chemin de retour.

---

# 6. La galerie

## 6.1 Contenu

La galerie affiche les options de la **piste active**, groupées, précédées le cas échéant de la bannière d'invalidation.

| Piste | Options | Contraintes |
|---|---|---|
| Corps | 19 modèles | 2 écartés automatiquement (squelette non standard, fichier illisible) |
| Casque | 176 au catalogue, filtrés par époque du corps | voir §6.2 |
| Combinaison | 53 | compatibles tous corps |
| Gants | 69 | compatibles tous corps |

## 6.2 Filtrage des casques par époque

Le filtre est **automatique et non désactivable** — un casque hors époque ne s'appliquerait pas.

| Époque du corps | Casques proposés | Voitures concernées |
|---|---|---|
| Moderne | 100 | 195 / 312 |
| Années 70 | 44 | 17 |
| Années 60 | 21 | 13 |
| Années 80 | 11 | 40 |
| Mod à nommage propre | 0 | le reste |

Le compteur de la barre d'outils énonce la cause du filtrage, jamais le filtre seul (§8.3).

## 6.3 Regroupement typé

Les identifiants ont la forme `driver_helmet/<FAMILLE>/<variante>` sur deux niveaux. Le premier niveau est déjà signifiant — mais **il ne signifie pas la même chose selon l'époque**.

| Époque | Ce que la famille désigne | Intitulé de la bascule | Exemple |
|---|---|---|---|
| Moderne | une couleur | `Grouper par couleur` | `HELMET_BASE_Red` → 1…7 |
| Années 80 | une couleur | `Grouper par couleur` | `HELMET_1985_Blue` |
| Années 70 | un motif | `Grouper par motif` | `HELMET_1975_Blue` → plain, checkered, stripe1 |
| Années 60 | **un pilote** | `Grouper par pilote` | `HELMET_1969` → amon, bandini, clark, hill, ickx |
| Mod | un pack | `Grouper par pack` | `ddm` → c-one, shinichi_yamaji, trd |

**Table de correspondance maintenue en code**, indexée sur le préfixe d'époque. Préfixe inconnu → repli sur `Grouper par famille`, sans erreur ni message.

**Bascule.** `Grouper par <axe>` / `Tout`. Le second mode donne une grille plate, tri alphabétique. État mémorisé entre sessions.

## 6.4 En-tête de groupe

```
Chris Amon    HELMET_1969/amon    1    ────────────────────
Red           HELMET_BASE_Red     7    ────────────────────
```

Nom lisible en tête, en typographie courante et en couleur de texte secondaire. Identifiant de dossier ensuite, en mono, 10 px, gris tertiaire. Compteur. Filet horizontal jusqu'au bord.

Cette forme vaut pour **toutes** les époques, pas seulement 1969. Elle donne une place naturelle à la traduction du nom de couleur, tout en gardant l'identifiant visible pour ceux qui le connaissent.

**Cas 1969.** Amon, Bandini, Clark, Hill, Ickx s'affichent en toutes lettres. C'est le seul endroit du produit où le catalogue raconte quelque chose ; c'est aussi du contenu qui ne se traduit pas, donc sans risque de localisation. La correspondance identifiant → nom complet est une table statique de cinq entrées.

## 6.5 Case par défaut

Première position du premier groupe, toujours.

| Mode | Contenu de la case | Libellé |
|---|---|---|
| Corps d'origine | trame diagonale + « Celui de la livrée » | *Par défaut* |
| Corps substitué | trame diagonale + « La livrée ne prévoit rien pour ce corps » | *Aucun* |

Elle se sélectionne et se désélectionne comme n'importe quelle autre case. Le défaut est **un choix parmi les autres**, pas une case à cocher à part.

Survolée, elle applique le défaut sur le plateau comme les autres appliquent leur texture — en mode substitué, elle retire simplement la pièce.

---

# 7. L'échantillon

## 7.1 Nature

**L'image plate fournie par Assetto Corsa**, affichée entière, sans découpe ni mise en scène. 173 casques sur 176 en disposent.

## 7.2 Pourquoi pas un rendu 3D

Trois raisons, dans l'ordre :

1. **Le survol fait le jugement.** La case n'a plus à prouver le résultat ; elle sert de repère pour retrouver, comparer grossièrement, revenir.
2. **Un casque en 3D cache la moitié de sa livrée.** L'image plate montre tout d'un coup. Elle est illisible comme promesse de résultat, excellente comme empreinte.
3. **Le rendu paresseux au défilement produit un scintillement** au moment précis où l'utilisateur balaie la grille et a besoin de stabilité.

Le rendu 3D en galerie est **écarté**, y compris dans sa variante « seulement les cases visibles ».

## 7.3 Traitement visuel

L'échantillon doit être **lisiblement autre chose** qu'un aperçu du résultat. Deux langages pour deux métiers.

| Propriété | Valeur | Raison |
|---|---|---|
| Forme | carré plein, pleine largeur de case | c'est une image, pas un objet |
| Bord | filet 1 px, gris de ligne | franc, pas de rayon |
| Ombre | aucune | pas de plateau, pas de mise en scène |
| Repos | `saturate(.86) brightness(.94)` | cent cases ne doivent pas crier ensemble |
| Survol | saturation pleine, bord en accent atténué | l'attention suit le curseur |
| Retenu | saturation pleine, bord en accent, liseré intérieur | |

**Taille.** Case de 104 px minimum, grille en `auto-fill`, gouttière de 9 px. Le nom de dossier en mono 10 px sous l'image, tronqué par ellipse.

**Favoris.** Étoile en accent atténué en préfixe du nom. Pas de bouton flottant sur l'image — l'image est le contenu, on ne pose rien dessus.

## 7.4 Les trois casques sans image

Case neutre : trame diagonale, nom seul, aucun rendu de substitution. Trois exceptions ne justifient pas un second pipeline. Elles restent survolables et sélectionnables — le plateau, lui, montre bien le résultat.

---

# 8. Barre d'outils

## 8.1 Composition

`RECHERCHE` · `AFFICHAGE` (bascule de regroupement) · `Favoris` · `Récents` · compteur aligné à droite.

## 8.2 Recherche

Filtre sur le nom de dossier **et** sur le nom lisible de famille — taper « clark » comme taper « amon » doit fonctionner. Filtrage instantané, sans validation. Les groupes vides disparaissent.

## 8.3 Compteur

Deux lignes, alignées à droite :

```
100 casques compatibles
corps driver · époque moderne
```

Il porte trois choses : le nombre, la cause du filtrage, et implicitement l'avertissement que changer de corps changera ce nombre. En cas vide : `Aucun casque applicable`.

**Localisation.** Le risque est la largeur en allemand, pas le sens. Le bloc a une largeur maximale de 240 px et se replie sur trois lignes si nécessaire ; il ne tronque jamais.

## 8.4 Favoris et récents

Deux cases à cocher, cumulables avec la recherche. « Récents » retient les douze derniers essais **adoptés**, pas survolés. La distinction est importante : le survol est exploratoire par nature et polluerait l'historique.

---

# 9. Le corps

## 9.1 Sélection

Le bloc Corps de la piste porte un sélecteur replié : la valeur courante et une mention d'état (`celui de la voiture` / `substitué`). Cliquer la piste Corps ouvre la **galerie des corps** dans la zone de droite, au même format que les autres pistes — même geste, même grammaire.

Les 19 corps se présentent en échantillons : ici, faute d'image plate signifiante, la case porte un **rendu 3D du corps**, généré une fois et conservé. Le coût est mesuré à 19 × 4,2 Mo, soit 80 Mo au pire pour un cache de 2 Gio.

## 9.2 Chargement

C'est le seul endroit du produit où le changement n'est pas instantané : environ 0,4 s à la première visite d'un corps.

**Comportement obligatoire :** le corps précédent reste affiché jusqu'à ce que le nouveau soit prêt. Un filet de progression en accent, de 2 px, court en pied de plateau, et la ligne d'état affiche *« Chargement du corps… »*.

**Jamais de plateau vide.** Un mannequin qui disparaît puis réapparaît coûte plus cher en perception que 0,4 s d'attente sur une image stable.

## 9.3 Corps écartés

Deux modèles sur les dix-neuf sont détectés comme inutilisables — squelette non standard, fichier illisible. Ils **n'apparaissent pas** dans la galerie. Aucun message : une option qu'on ne peut pas prendre n'a pas à être montrée.

---

# 10. Le mode « corps substitué »

## 10.1 Ce qui change

Dès que le corps n'est plus celui de la voiture, la référence « livrée » disparaît — elle est nommée d'après l'ancien corps et n'a plus de destinataire. **Six changements simultanés :**

| Élément | Corps d'origine | Corps substitué |
|---|---|---|
| Bandeau de plateau | absent | `CORPS SUBSTITUÉ · APERÇU SEULEMENT`, angle supérieur gauche, en teinte d'alerte |
| Case par défaut | « Celui de la livrée » | « La livrée ne prévoit rien pour ce corps » |
| Pistes non choisies | *Celle de la livrée* | *Aucune* |
| Mention de piste Corps | *celui de la voiture* | *substitué* |
| Bouton de sortie | remise sur la livrée | retour au corps de la voiture |
| Badge de colonne | `MODIFIÉ` | `SUBSTITUÉ` |

Le bandeau de plateau est **permanent** tant que dure le mode. Il tranche du même coup la question non résolue de l'application en course : quoi qu'il advienne côté jeu pour la tenue, un corps substitué ne suivra jamais.

## 10.2 Bannière d'invalidation

Elle apparaît **au moment du changement**, en tête de galerie, et non en modale. Le regard y va naturellement après l'action.

```
⚠  Ce corps n'est pas celui de la voiture. Ce qui tombe :
   • Le casque retenu — il est daté et ne s'applique pas à ce corps.
   • La tenue de la livrée — elle est nommée d'après « driver » et n'a plus de référent.
   • L'application en course — un corps substitué ne vit que dans cet aperçu.
                                    [ Choisir une tenue ]  [ Revenir à driver ]
```

**Une bannière unique** listant ce qui tombe, jamais trois messages séparés. Le premier bouton est l'action réparatrice, le second la marche arrière. Elle se referme au premier choix effectué, ou par le second bouton.

Si le casque retenu se trouve compatible avec le nouveau corps, la première puce est retirée — la bannière n'annonce que ce qui est réellement perdu.

---

# 11. États vides et cas particuliers

## 11.1 Corps sans casque applicable

Mannequin de mod à nommage propre : aucun casque du jeu ne s'y pose.

```
              Ce corps porte son propre casque.

  « yk2_kana » nomme ses images autrement : aucun casque du jeu
  ne peut s'y poser. La combinaison et les gants, eux, restent
  modifiables.

        [ Passer à la combinaison ]   [ Changer de corps ]
```

Ce n'est pas une erreur, c'est une propriété du mod. Le texte l'énonce sans s'excuser, et propose les deux sorties utiles. On ne propose jamais un choix sans effet.

## 11.2 Voiture de course

L'écran reste **accessible**, jamais grisé sans explication.

```
          Ce pilote porte les couleurs de son écurie.

  Sur une voiture de course, la tenue fait partie de la livrée.
  Votre pilote reprendra sa tenue dès que vous choisirez une
  voiture de rue.

                 [ Voir mon pilote quand même ]
```

Le choix reste enregistré : rien n'est perdu, seulement suspendu. Le bouton ouvre le panneau d'essayage en lecture, sur un corps neutre, sans galerie.

## 11.3 Recherche sans résultat

Message d'une ligne sous la barre d'outils, avec le terme cherché et un lien d'effacement. Pas d'illustration, pas de bloc vide de 200 px.

---

# 12. Interaction et accessibilité

## 12.1 Souris

| Geste | Effet |
|---|---|
| Survol d'une case | applique la texture au plateau, met à jour la ligne d'état |
| Sortie du survol | rétablit la texture retenue |
| Clic sur une case | adopte, met à jour la piste, marque la case |
| Clic sur une piste | active la piste, recadre, remplace le contenu de la galerie |
| Glisser sur le plateau | rotation |

**Latence.** L'application au survol doit être perçue comme immédiate. Aucune temporisation d'entrée ; à la sortie, 80 ms de délai avant rétablissement pour éviter le clignotement entre deux cases adjacentes.

## 12.2 Clavier

Le survol n'est pas accessible au clavier : le **focus vaut survol**. Parcourir la grille aux flèches applique chaque option au plateau, exactement comme la souris. `Entrée` adopte. `Échap` rétablit le choix retenu et rend le focus à la piste. `Tab` circule entre barre d'outils, pistes et grille.

Focus visible en permanence, jamais supprimé.

## 12.3 Mouvement réduit

Le recadrage de caméra devient instantané. L'application au survol, elle, reste — ce n'est pas une animation décorative mais le mécanisme central de l'écran.

## 12.4 Rendu indisponible

Si le moteur 3D ne peut pas démarrer, le panneau d'essayage affiche l'échantillon plat de la pièce retenue à grande taille, avec une mention d'une ligne. La galerie et la sélection restent pleinement fonctionnelles. **L'écran ne se bloque jamais sur l'absence de 3D.**

---

# 13. Persistance

| Donnée | Portée | Note |
|---|---|---|
| Casque, combinaison, gants | globale | survivent au changement de voiture |
| Corps substitué | globale | mais sans effet en course, par nature |
| Favoris | globale | |
| Récents | globale, 12 entrées | adoptions seulement |
| Piste active | session | retour sur Casque à l'ouverture |
| Mode de regroupement | globale | |

Choix incompatible avec la voiture courante : **conservé, pas effacé**. Il s'applique de nouveau dès qu'une voiture compatible est chargée. C'est le principe même du choix global.

---

# 14. Textes et localisation

**Aucun texte en dur.** Six langues.

Ne se traduisent pas : les identifiants de dossier (`HELMET_BASE_Red / 7`), les noms de corps (`driver_60`), les noms propres de pilotes historiques.

Se traduisent : libellés de piste, intertitres de hiérarchie, noms lisibles de famille (« Red » → « Rouge »), intitulés de regroupement, tous les messages d'état.

**Points de vigilance en allemand :** le compteur (§8.3), les intitulés de bascule (`Grouper par couleur`), le bouton de sortie. Tous trois disposent d'un repli sur deux lignes plutôt que d'une troncature.

**Registre.** Actif, phrase capitalisée, pas de filler. Un bouton nomme ce qui se passe : « Revenir au corps de la voiture », pas « Réinitialiser ». Les états vides donnent une direction, pas une humeur.

---

# 15. Repères visuels

Le langage existant s'applique sans exception : fond quasi noir, panneaux gris très foncés, Rosso Corsa en accent unique, typographie petite et serrée, rayons de 2 px.

| Rôle | Valeur |
|---|---|
| Fond | `#0b0b0d` |
| Panneau | `#141416` |
| Case de galerie | `#1a1b1e` |
| Filet | `#26272b` / `#1f2023` (atténué) |
| Texte | `#dcdcdd` / `#8d8e92` / `#5c5d62` |
| Accent | `#c9331f`, atténué `#7e2415` |
| Alerte | `#c88a2a` sur `#1b1811`, filet `#4a3a1c` |

**Discipline d'accent.** Dans cet écran, le rouge saturé est réservé à trois usages : la pastille `ESSAYAGE`, la bordure gauche de la piste active, le bord de la case retenue. Le filet de chargement l'emploie aussi, brièvement. Rien d'autre.

| Mesure | Valeur |
|---|---|
| Colonne de session | 328 px |
| Panneau d'essayage | 392 px |
| Plateau, hauteur mini | 380 px |
| Case de galerie, mini | 104 px |
| Gouttière de grille | 9 px |
| Hauteur de piste | 34 px |
| Transition de cadrage | 220 ms |
| Délai de sortie de survol | 80 ms |
| Chargement de corps | ~400 ms |

---

# 16. Ce qui reste ouvert

**L'application en course.** Aujourd'hui le choix ne vaut que dans l'aperçu. S'il devient effectif en jeu pour la tenue, le bandeau « corps substitué · aperçu seulement » devient la seule chose qui distingue les deux moitiés de l'écran — et il devra probablement être plus qu'une étiquette d'angle. La spec fonctionne dans les deux cas ; seule la place de ce bandeau est à revoir le jour où la décision tombe.

**Le nom lisible des familles modernes.** « Red » → « Rouge » suppose une table de correspondance couleur. Faisable pour les douze familles `HELMET_BASE`, à confirmer pour les packs de mods, où le premier niveau n'est pas toujours un mot traduisible.

**La galerie des combinaisons et des gants.** Cette spec les traite par symétrie avec les casques. Elles n'ont ni époque ni contrainte de compatibilité, donc pas de filtrage — mais leurs identifiants suivent-ils la même structure à deux niveaux ? Si non, le regroupement typé ne s'y applique pas et elles passent en grille plate.

---

*Pit Box · spécification UX/UI · écran Pilote · à lire avec les deux maquettes HTML associées*
