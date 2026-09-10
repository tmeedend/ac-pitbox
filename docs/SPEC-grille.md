# Pit Box — grille de la bibliothèque

## Spécification d'implémentation

*Lisibilité des cartes, affichage du nom, et régénération des vignettes. Ne concerne ni les filtres (spec séparée, close) ni l'écran de détail d'un mod. Maquette de référence : `pit-box-barre-et-grille.html`, section 2.*

> **État : implémenté, avec un écart de structure.** Ce document reste
> l'intention d'origine et l'argumentaire qui la porte — c'est pour ça qu'on le
> relit. Mais il décrit **un** gabarit de rendu, là où l'implémentation en porte
> **trois**, et le §5.7 qu'il consacre au versionnage de ce gabarit unique est
> devenu sans objet. Le §11, en fin de document, dit ce qui a changé et
> pourquoi. En cas d'écart, `SPEC.md` fait foi.

---

# 1. Diagnostic

Le constat « les voitures sombres sont indiscernables » recouvre **deux problèmes distincts** qui appellent des réponses différentes. Les confondre conduit à régler le mauvais.

**A — Les cartes ne se détachent pas de la page.** Le fond de carte est aussi sombre que le fond de grille. Problème de contenant, réglé par §2.

**B — Deux voitures sombres se ressemblent.** Les `preview.png` sont opaques et portent leur propre fond, cuit dans l'image : aucun réglage du contenant ne peut agir derrière la voiture. Aggravé par l'hétérogénéité des sources — rendus sur fond noir, sur fond blanc, captures en jeu, photos — qui oblige l'œil à se réadapter à chaque carte. Réglé partiellement par §3 (typographie : identifier plutôt que voir) et complètement par §5 (régénération).

**Contrainte permanente.** Certaines voitures sont chiffrées et ne seront **jamais** générables (§8). La grille restera donc mixte indéfiniment. Le traitement du contenant n'est pas un pis-aller en attendant la régénération : c'est ce qui confine l'hétérogénéité à l'intérieur de l'image au lieu de la laisser contaminer la carte entière. Il est requis dans tous les cas.

---

# 2. Traitement de la carte

## 2.1 Ce qui change

| Élément | Avant | Après |
|---|---|---|
| Fond de carte | `#161719` | `--cell` `#1a1b1e` |
| Bordure | `#1c1d20` — invisible | `#2e2f34` — un ton au-dessus du fond de grille |
| Élévation | aucune | `inset 0 1px 0 rgba(255,255,255,.025)` + `0 4px 14px rgba(0,0,0,.35)` |
| Image | pleine largeur, à ras | en retrait, sur un mat neutre |
| Mat derrière l'image | néant | **dégradé radial** `#2b2d33` → `#17181c`, bordure `#2b2c31`, rapport 16:9 |

## 2.2 Le mat

L'image est posée sur un rectangle **au rapport 16:9**, portant un **dégradé radial** — plus clair au centre, sombre aux bords — et sa propre bordure.

Il fait trois métiers :

**Il unifie deux sources.** Les `preview.png` d'origine sont opaques et le recouvrent ; les vignettes régénérées sont transparentes (§5.6) et le laissent transparaître. Le même mat sert donc de fond réel aux unes et d'encadrement aux autres — c'est ce qui rend les deux types comparables dans une grille qui restera mixte pour toujours (§7).

**Il porte le fond des vignettes transparentes.** Comme il est en CSS et non cuit dans l'image, il suit le thème, la densité et les états de carte sans jamais exiger de régénération.

**Il absorbe les formats inattendus.** L'image est `object-fit: contain` ; ce qui ne remplit pas le cadre montre le mat plutôt qu'un vide noir.

Le 16:9 est le rapport des previews d'Assetto Corsa : il évite bandes noires et recadrages sur la source la plus courante.

## 2.3 Ce qu'on ne touche pas

Les images elles-mêmes. Aucun filtre CSS, aucune correction de luminosité, aucun masque. Une preview soignée par son auteur ne doit pas être dégradée pour compenser celles qui ne le sont pas ; le gain vient du contenant, ou de la source (§5).

## 2.4 Marqueurs d'état : suppression

Les cartes portent aujourd'hui trois marquages concurrents — badge `BASE`, pastille verte, absence de marquage — dont un seul est lisible sans apprentissage.

**Ils sont tous supprimés de la grille**, à une exception près (§2.5).

**Raison.** La grille et la vue table servent deux modes d'usage distincts : on **joue** dans la grille, on **gère** dans la table. Un état d'installation est une information de gestion posée sur un écran de jeu : elle ne change rien à la décision « je prends celle-là ce soir ». Les filtres couvrent déjà ces états pour qui veut s'en servir comme critère.

| État | Où il vit désormais |
|---|---|
| Installé, activé | vue table, fiche de détail, filtre `État` |
| Déjà essayé | vue table, filtre `Déjà essayé` |
| Contenu de base | vue table, filtre `Contenu de base` |
| Vignette régénérée | nulle part — invisible par nature |
| Mise à jour disponible | vue table, fiche de détail |

**Il ne reste qu'un seul marqueur dans la grille : le badge `SESSION`.** Et ce n'est pas un état de mod mais la sélection courante — une autre catégorie, qui relève du niveau 2 du barème d'accent.

Bénéfice secondaire : les cartes s'allègent, ce qui sert directement l'objectif de lisibilité de §1.

## 2.5 L'exception : ce qui empêche de jouer

Un état qui rend la voiture **inutilisable** modifie la décision, et doit donc rester visible dans la grille. Sans marquage, l'utilisateur la choisit, lance la session, et découvre le problème dans le jeu.

Concerné : mod cassé, contenu incomplet, dépendance manquante, mod désactivé.

**Traitement — pas de badge.** La carte entière est atténuée :

```css
.card.unusable        { opacity: .42; }
.card.unusable .t     { color: var(--txt-3); }
```

Une carte éteinte se comprend au premier regard, sans légende et sans traduction. Un badge demanderait un vocabulaire à apprendre, dans six langues, pour une information qui se dit très bien par l'absence de vivacité.

La **raison** de l'indisponibilité est portée par l'infobulle de la carte et par la fiche de détail — pas par la grille.

**Ne jamais masquer ces cartes.** Une voiture qui disparaît produit le « où est passée ma Skyline », bien pire qu'une carte éteinte. Elles restent sélectionnables et consultables ; seul le lancement de session est bloqué, avec l'explication au moment du blocage.

---

# 3. Affichage du nom

## 3.1 Le problème réel

Trois cartes de la bibliothèque affichent aujourd'hui, à l'identique :

```
Nissan Skyline GT-R R3…
Nissan · 2000
```

La troncature intervient exactement avant ce qui distingue les modèles — R32, R33, R34 — et le nom commence par un mot déjà écrit sur la ligne suivante.

## 3.2 Deux lignes : obligatoire

Le nom s'affiche sur **deux lignes maximum**, avec troncature en fin de deuxième ligne.

```css
display: -webkit-box;
-webkit-line-clamp: 2;
-webkit-box-orient: vertical;
overflow: hidden;
min-height: 32px;   /* réserve les deux lignes : les cartes gardent la même hauteur */
```

Le `min-height` n'est pas cosmétique : sans lui, les cartes à nom court et à nom long n'ont pas la même hauteur et la grille se déchire.

**C'est la correction principale.** Avec deux lignes, `Nissan Skyline GT-R R34 V-Spec II Nür` tient en entier.

## 3.3 Hiérarchie typographique

| Ligne | Taille | Graisse | Couleur |
|---|---|---|---|
| Nom | 12.5 px | 500 | `#ececed` |
| Marque | 10.5 px | 400 | `--txt-2` `#8d8e92` |
| Séparateur et année | 10.5 px | 400 | `--txt-3` `#5c5d62` |

Aujourd'hui les deux lignes ne se distinguent que par la taille. Creuser le contraste sépare l'identité (le nom) de la métadonnée (marque, année), et rend le balayage possible sans lire.

## 3.4 Retrait de la marque — option

Quand le nom du mod commence par le nom de sa marque, ce préfixe peut être retiré **à l'affichage seulement**.

| | |
|---|---|
| **Défaut** | désactivé — la marque reste dans le nom |
| **Emplacement du réglage** | menu d'affichage accroché à la bascule de vue (§4), pas les réglages globaux |
| **Libellé** | *Masquer la marque dans le nom* |

Le défaut est « conservé » parce que la troncature est déjà réglée par §3.2 : le retrait n'est plus une réparation mais un gain de densité, et quatorze ans d'habitude désignent ces voitures par leur nom complet. Le réglage vit à côté de la bascule de vue parce qu'il faut en voir l'effet pour le juger — dans un écran de réglages, il est invisible.

**Règle de retrait :**

```
si  nom.toLowerCase() commence par (marque.toLowerCase() + " ")
et  le reste après retrait n'est pas vide
alors afficher le reste
sinon afficher le nom entier
```

**Alias de marque.** La comparaison doit couvrir les formes courantes : `Alfa Romeo` / `Alfa`, `Mercedes-Benz` / `Mercedes` / `Mercedes Benz`, `Chevrolet` / `Chevy`, `Volkswagen` / `VW`. Table d'alias maintenue en code, comparaison insensible à la casse et aux tirets.

## 3.5 Portée du retrait — trois interdits

Ces trois points sont la raison pour laquelle le retrait est purement cosmétique. Les enfreindre casse des comportements que l'utilisateur tient pour acquis.

**Le tri porte toujours sur le nom complet, marque comprise.** C'est ce qui maintient les Nissan groupées sous N, les Porsche sous P. Trier sur le nom amputé disperserait la Skyline en S et la 350Z en 3, et ferait perdre le regroupement par marque que l'utilisateur obtient aujourd'hui gratuitement. **Le tri par nom complet est le tri par marque.**

**L'index de recherche contient le nom complet.** Taper `nissan skyline` doit fonctionner même quand la carte affiche `Skyline GT-R R34`.

**Le nom stocké n'est jamais modifié.** Le retrait se fait au rendu, à partir du nom et de la marque. Aucune écriture en base, aucune migration, réversible instantanément.

---

# 4. Densité

La bascule de vue actuelle passe de deux à **trois positions** :

| Position | Icône | Grille |
|---|---|---|
| Grille dense | ▦ | `repeat(auto-fill, minmax(190px, 1fr))` |
| Grille confortable | ▤ | `repeat(auto-fill, minmax(280px, 1fr))` |
| Liste | ☰ | inchangée |

Le choix est persisté globalement.

**Menu d'affichage.** Un chevron accolé à la bascule ouvre un petit popover portant les préférences de présentation de la grille : la densité (redondante avec les icônes, mais nommée), et *Masquer la marque dans le nom* (§3.4). C'est le point de rassemblement des réglages de présentation ; il évite d'en disperser dans les réglages globaux.

---

# 5. Vignettes régénérées

## 5.1 Principe

L'application convertit et rend déjà les voitures en 3D pour l'aperçu de la fiche. Le même moteur peut produire les vignettes de la grille : **même cadrage, même éclairage, même fond, pour toute la bibliothèque.**

Le gain ne se limite pas au confort visuel. Une fois les fonds homogènes, l'œil compare les *formes* — ce qu'il ne peut pas faire aujourd'hui parce qu'il passe son temps à se réadapter aux fonds. C'est un changement de nature, pas de degré.

## 5.2 Coût disque — deux caches distincts

| | Contenu | Ordre de grandeur | Éviction |
|---|---|---|---|
| **Cache 3D interactif** | modèles convertis des voitures consultées | ~20 Mo × N | LRU, plafonné (2 ou 4 Go selon le profil) |
| **Vignettes** | images finies | ~150 Ko × 312 ≈ **45 Mo** | jamais — supprimée avec le mod |

Les 20 Mo par voiture sont **transitoires**, consommés pendant le rendu. Ce qui subsiste est un PNG.

## 5.3 Le pipeline de vignettes ne passe pas par le cache LRU

**Point critique.** Une implémentation naturelle réutiliserait le cache 3D existant. Il ne faut pas : la génération remplirait le cache avec 312 voitures dont l'utilisateur ne consultera aucune, éjecterait celles qu'il consultait vraiment, et le cache se mettrait à travailler contre lui.

Le pipeline de vignettes est une **voie parallèle** : convertir → rendre → écrire le PNG → **jeter immédiatement**. Pic disque d'une voiture à la fois.

## 5.4 Génération au fil de l'eau

**Il n'y a pas d'option « générer maintenant » à l'installation.** La génération se fait à l'affichage : quand une carte sans vignette générée entre dans le viewport, elle est mise en file. La bibliothèque se normalise pendant qu'on l'utilise, sans attente initiale.

Ordre de la file : ce qui est visible d'abord, puis le reste de la liste filtrée courante, puis le catalogue.

Un bouton **« Générer toute la bibliothèque »** existe dans l'écran de réglage des vignettes (§6). Ce n'est pas le même besoin : à l'installation on exprime une intention, dans les réglages on déclenche un travail — typiquement après avoir ajouté cinquante mods.

## 5.6 Paramètres de rendu — le gabarit

Ces valeurs constituent le **gabarit d'origine Pit Box**, celui que rétablit le bouton de §6.3. Elles sont un point de départ à affiner sur la grille d'aperçu à six voitures (§6.2), pas des constantes sacrées — mais toute valeur doit rester **identique pour les 312**, c'est la seule propriété non négociable.

### Format

| | Valeur |
|---|---|
| Rapport | **16:9**, comme les `preview.png` d'Assetto Corsa |
| Rendu | 1024 × 576 |
| Sortie | PNG **avec canal alpha**, fond entièrement transparent |
| Poids visé | 120 à 250 Ko |

Le 16:9 n'est pas un choix esthétique : c'est ce qui permet aux vignettes générées et aux `preview.png` d'origine d'occuper le même cadre sans bande noire ni recadrage. La grille restant mixte pour toujours (§7), les deux sources doivent partager un format.

### Fond transparent — et pourquoi la carte doit fournir le sien

**Une vignette transparente ne résout rien à elle seule.** Détourée sur le fond de carte `#1a1b1e`, une carrosserie noire devient *moins* lisible qu'aujourd'hui : on supprime le fond que le rendu apportait et on retombe sur le problème B de §1, aggravé.

La transparence n'est bonne qu'accompagnée d'un **fond fourni par la carte** — un dégradé radial, plus clair au centre, sombre aux bords (§2.2). Trois bénéfices, dans l'ordre :

- le fond devient du CSS : il suit le thème, la densité et les états de carte — survol, carte en session — **sans jamais régénérer une image** ;
- il est identique pour les 312 **par construction**, la comparabilité ne dépend plus du rendu ;
- le fichier est plus léger.

Deux précautions techniques :

- **Ombre de contact cuite dans l'alpha.** Sans elle la voiture flotte. Ellipse douce sous les roues, opacité ~35 %, rayon de flou de l'ordre de la largeur d'un pneu.
- **Alpha prémultipliée.** Un détourage non prémultiplié produit un liseré clair sur fond sombre, visible précisément sur les carrosseries noires.

**Pas de reflet miroir.** Il exige un sol, donc un fond, et à ~98 px de haut il consommerait la moitié du cadre. L'ombre de contact suffit.

### Caméra

| | Valeur de départ |
|---|---|
| Vue | trois-quarts avant |
| Azimut | à **mesurer sur les previews Kunos** — voir ci-dessous |
| Élévation | 8° au-dessus de l'horizon |
| Champ | 22° — équivalent long focal, faible distorsion |
| Cible | centre de la boîte englobante |
| Cadrage | ajusté à la largeur de la boîte englobante, marge 6 % |
| Roues | **braquage nul**, hauteur de caisse au repos |

**L'azimut et le sens doivent être relevés, pas inventés.** Les previews Kunos sont cohérentes entre elles : échantillonner une centaine d'entre elles donne la convention exacte — angle et sens dans lequel la voiture pointe.

**Pourquoi suivre leur angle.** Les voitures chiffrées garderont leur `preview.png` pour toujours (§7) : la grille est mixte de façon permanente. Or l'orientation est ce que l'œil compare en premier — une grille où la moitié des voitures pointe à gauche et l'autre à droite fatigue bien plus qu'une grille aux fonds hétérogènes. Aligner l'angle réduit durablement l'écart entre les deux sources.

**Pourquoi ne pas suivre leur fond.** Il est cuit dans l'image, donc inimitable ; et c'est précisément le défaut qu'on répare.

Le trois-quarts avant d'Assetto Corsa n'est pas une mauvaise pratique — c'est la convention universelle du catalogue automobile. Ce qui ne va pas chez eux, c'est le fond sombre sur carrosseries sombres et l'absence de cadrage normalisé entre mods. **On garde leur géométrie, on corrige leur lumière.**

**Cadrage : ajusté, pas à l'échelle.** Chaque voiture remplit le cadre, quelle que soit sa taille réelle. On perd l'information de gabarit relatif ; on gagne que chaque vignette est lisible à 190 px. Pour un catalogue dont le métier est l'identification, c'est le bon échange.

### Éclairage

| | Valeur de départ |
|---|---|
| Clé | 45° d'azimut depuis la caméra, 35° d'élévation, 5500 K |
| Complément | opposé, 25 % de l'intensité de la clé |
| Contre-jour | derrière le sujet, rasant sur la ligne de toit, 60 % |
| Teinte | strictement neutre, aucune dominante |
| Exposition | **fixe pour les 312** |
| Post-traitement | aucun bloom, tone mapping neutre |

**Le contre-jour est le paramètre le plus directement utile au problème de départ.** C'est lui qui détache la silhouette d'une carrosserie noire — plus que n'importe quel réglage de fond.

**Aucune auto-exposition par voiture.** Elle ramènerait une voiture noire et une voiture blanche au même gris moyen — c'est-à-dire qu'elle effacerait exactement la différence qu'on cherche à montrer. Rig fixe, exposition fixe.

Aucune dominante colorée non plus : les livrées de mods couvrent tout le spectre, et une clé chaude déplacerait chaque teinte.

### Ce que l'utilisateur peut régler

Le gabarit exposé dans `Réglages › Vignettes` couvre : azimut, élévation, champ, marge de cadrage, intensité des trois lumières, opacité de l'ombre. Le format, la transparence, l'exposition fixe et l'absence de post-traitement **ne sont pas réglables** — ce sont les propriétés qui garantissent la comparabilité.

## 5.7 Versionnage du gabarit d'origine — *abandonné, voir §11*

Le gabarit d'origine porte un numéro de version, stocké avec le cache.

| Situation à la mise à jour de Pit Box | Comportement |
|---|---|
| L'utilisateur n'a jamais modifié le gabarit | il **hérite automatiquement** du nouveau défaut, avec proposition de régénérer |
| L'utilisateur a personnalisé | son gabarit est conservé, et un message discret dans `Réglages › Vignettes` signale qu'un nouveau défaut existe |

Dans les deux cas la régénération est **proposée, jamais forcée** (§6.5).

> **Ce mécanisme n'existe pas, et n'a pas besoin d'exister.** Il répond à une
> question — « l'utilisateur a-t-il personnalisé le gabarit ? » — que les
> presets embarqués rendent sans objet : ceux-ci sont en lecture seule et
> évoluent avec l'app, une copie ne bouge jamais, et il n'y a donc plus rien à
> deviner. Ce qui reste utile du §5.7 a été gardé ailleurs et sous une autre
> forme : `RENDERER_VERSION` périme les images quand le **code de rendu**
> change, ce que ni le gabarit ni le convertisseur ne savent voir (§11).

## 5.5 Profils à l'installation

Trois profils, présentés **avec les chiffres de la bibliothèque réellement scannée**, pas dans l'abstrait.

```
312 voitures détectées.

  ○  Léger
     Pas d'aperçu 3D, pas de vignettes régénérées.
     Aucun espace disque supplémentaire.

  ●  Normal                                    (présélectionné)
     Aperçu 3D sur la fiche d'une voiture. Vignettes d'origine.
     Cache : 2 Go.

  ○  Soigné
     Aperçu 3D, plus des vignettes régénérées pour toute la
     bibliothèque — même cadrage, même éclairage.
     Cache : 4 Go · vignettes : environ 47 Mo, générées au fil
     de la navigation.

  Modifiable à tout moment dans les réglages.
```

Quatre exigences :

**Nommer par le résultat.** *Léger*, *Normal*, *Soigné* — pas « optimiser l'espace disque », qui décrit un moyen.

**Chiffrer sur sa bibliothèque.** Le scan d'installation connaît déjà le nombre de voitures ; le volume et la durée s'en déduisent.

**Présélectionner le milieu.**

**Dire que c'est modifiable.** La phrase de pied transforme une décision en préférence et divise le poids ressenti de l'écran.

---

# 6. Réglage du gabarit des vignettes

## 6.1 Séparé de l'aperçu 3D

Deux réglages distincts, dans deux écrans distincts :

| | Portée | Conséquence d'un changement |
|---|---|---|
| **Aperçu 3D de la fiche** | vue interactive, libre | aucune, jamais |
| **Gabarit des vignettes** | les 312 images de la grille | régénération |

L'utilisateur tourne, zoome et règle l'aperçu 3D comme il veut sans qu'aucun avertissement n'apparaisse jamais. C'est cette séparation qui rend le second écran acceptable.

## 6.2 L'aperçu de réglage est une grille, pas une voiture

**Décision structurante.** Régler l'angle sur une seule voiture conduit à l'optimiser pour elle et à massacrer les autres. L'aperçu montre **six voitures de gabarits volontairement différents** — une berline, une monoplace, un pick-up ou SUV, une GT basse, une compacte, un prototype — mises à jour en direct pendant la manipulation des réglages.

On édite un catalogue : l'aperçu doit être un catalogue.

## 6.3 Rien ne s'applique avant « Appliquer »

Manipuler les réglages ne régénère rien. Le pied de l'écran affiche la facture en continu :

```
Appliquer régénérera 298 vignettes · environ 6 min      [ Annuler ]  [ Appliquer ]
```

Annuler ne coûte rien, ce qui rend l'expérimentation gratuite.

**Un bouton « Rétablir le gabarit d'origine »** reste visible en permanence et ramène aux valeurs du produit.

## 6.4 Changement de gabarit en cours de génération

Si une génération est en cours au moment où un nouveau gabarit est appliqué : **elle est annulée et la nouvelle démarre immédiatement**, sur l'ordre de file habituel (visible d'abord). Les vignettes déjà produites avec l'ancien gabarit sont écrasées au fur et à mesure.

L'état mixte transitoire est assumé : l'utilisateur qui applique un gabarit vient de valider une facture affichée, il sait ce qu'il a déclenché.

## 6.5 Régénération sur mise à jour de l'application

Une version de Pit Box qui modifie le rendu marque les vignettes existantes comme périmées. Un numéro de version stocké avec le cache suffit. L'utilisateur reçoit une **proposition**, jamais une régénération forcée : la grille reste parfaitement utilisable avec des vignettes d'une version antérieure.

---

# 7. Voitures chiffrées et autres échecs

Certaines voitures sont chiffrées et ne pourront jamais être rendues. Repli sur la `preview.png` d'origine — c'est-à-dire le comportement actuel.

**Trois règles pour que l'échec soit invisible plutôt que signalé :**

**Pas de badge sur la carte.** L'utilisateur n'y peut rien, aucune action n'est proposable, et un marqueur d'erreur sur une carte parfaitement utilisable ne fait que salir la grille.

**Le décompte se dit une fois, à la fin**, dans le rapport de génération : `298 générées · 14 impossibles (voitures protégées)`. Une ligne factuelle, sans tonalité d'échec.

**L'échec est mémorisé avec l'empreinte du mod** et n'est pas retenté à chaque lancement. Si le mod est mis à jour, l'empreinte change et la tentative se refait d'elle-même.

C'est ce cas qui rend §2 obligatoire : la grille restant mixte pour toujours, le mat neutre et la carte détachée sont ce qui confine la disparité à l'intérieur de l'image.

---

# 8. Suivi de la génération

## 8.1 Ce n'est pas un toast

312 voitures à environ une seconde, c'est cinq minutes et plus. Un toast est éphémère par définition. Il faut une **tâche de fond persistante**, au même emplacement en bas à droite, mais avec d'autres règles : ne se ferme pas seule, survit à la navigation, se réduit au lieu de disparaître, porte une annulation.

## 8.2 États

**Déplié**
```
┌──────────────────────────────────────────┐
│  Génération des vignettes                │
│  ████████████░░░░░░░░░░░░  47 / 312      │
│  Nissan Skyline GT-R R34                 │
│                        ~4 min   [Annuler]│
└──────────────────────────────────────────┘
```

**Réduit** — pastille circulaire avec l'anneau de progression et le pourcentage. Le nom de la voiture **n'apparaît pas** : il ferait clignoter le coin de l'écran chaque seconde.

**Terminé** — le rapport reste affiché jusqu'à fermeture explicite :
```
298 vignettes générées · 14 impossibles (voitures protégées)
```

## 8.3 Trois règles de comportement

**Le temps restant n'apparaît qu'après une dizaine de voitures.** Avant, l'estimation est fantaisiste et détruit la confiance dans toutes les suivantes. Afficher `47 / 312` seul, puis `47 / 312 · ~4 min`.

**Annuler conserve ce qui est fait, et le dit.** `148 vignettes générées · reprendre plus tard`, pas une disparition silencieuse. Sans cette phrase, l'utilisateur se retrouve avec une grille mixte sans savoir qu'il peut reprendre.

**Les cartes se mettent à jour une par une**, sans recharger la grille. Une vignette qui apparaît pendant qu'on regarde est une bonne surprise ; un rafraîchissement complet qui perd la position de défilement est une agression.

## 8.4 Génération et navigation

La génération ne bloque rien : filtres, tri, navigation, lancement de session restent disponibles pendant qu'elle tourne. Changer de filtre réordonne la file — ce qui est nouvellement visible passe devant.

---

# 9. Repères visuels

| Rôle | Valeur |
|---|---|
| Fond de grille | `--bg` `#0b0b0d` |
| Fond de carte | `--cell` `#1a1b1e` |
| Bordure de carte | `#2e2f34` |
| Mat derrière l'image | `#242529`, bordure `#2b2c31` |
| Nom | `#ececed`, 12.5 px, 500 |
| Marque | `--txt-2` `#8d8e92`, 10.5 px |
| Année et séparateur | `--txt-3` `#5c5d62`, 10.5 px |
| Carte en session | bordure `--accent` `#c9331f` + badge `SESSION` |
| Carte inutilisable | `opacity: .42`, nom en `--txt-3` |

| Mesure | Valeur |
|---|---|
| Carte, largeur mini (dense) | 190 px |
| Carte, largeur mini (confortable) | 280 px |
| Gouttière de grille | 9 px |
| Padding de carte | 8 px |
| Mat | rapport 16:9, largeur de la carte moins le padding |
| Réserve du nom | 32 px (deux lignes) |
| Rayon | 2 px |

---

# 10. Hors périmètre

**Les vignettes du contenu de base ne sont pas embarquées dans l'application.** Les previews Kunos sont des œuvres protégées et le fait que l'utilisateur possède le jeu ne donne pas de droit de redistribution ; des rendus produits à partir des modèles Kunos seraient des œuvres dérivées, plus exposées encore. Le contenu de base passe par le pipeline de génération comme le reste, sur la machine de l'utilisateur.

**Le traitement des images d'origine** — correction de luminosité, détourage, harmonisation automatique — est écarté. Soit on garde la source telle quelle, soit on change de source (§5). Il n'y a pas de milieu satisfaisant.

**La vue liste et la vue tableau** ne sont pas couvertes ici, sauf pour les trois interdits de §3.5 qui s'y appliquent identiquement.

---

# 11. Ce que l'implémentation a changé

Trois écarts de **structure** — ceux qui changent la forme de la solution, pas
ses valeurs. Les écarts de détail sont consignés dans `CLAUDE.md`.

## 11.1 Un gabarit est devenu trois presets

Le §5.6 décrit un gabarit unique, et sa propriété non négociable : « toute
valeur doit rester identique pour les 312 ». Cette propriété tient toujours —
elle porte sur un jeu d'images, pas sur le nombre de jeux possibles.

Ce qui l'a fait bouger est un constat d'usage : **la preview d'origine
d'Assetto Corsa est plus jolie que notre rendu, et le nôtre plus lisible.** Ce
ne sont pas deux qualités d'exécution du même objectif, ce sont deux
objectifs — identifier vite, ou avoir envie de regarder — et un compromis
unique les servait mal tous les deux. D'où trois presets embarqués :

| | Ce qu'il cherche | Ce que porte l'image |
|---|---|---|
| **Catalogue** | identifier vite | la voiture seule, détourée |
| **Vitrine** | avoir envie de regarder | + une flaque de lumière et un reflet |
| **Officiel** | être indiscernable d'une `preview.png` du jeu | + son fond entier |

Ils sont **en lecture seule et se dupliquent** : un preset vide serait une
douzaine de curseurs de rien, une copie de Vitrine est à un réglage d'être la
sienne. C'est aussi ce qui remplace le « Rétablir le gabarit d'origine » du
§6.3 — l'original est toujours là, juste à côté, intact — et ce qui rend le
§5.7 sans objet.

**Un preset est lié à chaque densité de grille** (dense → Catalogue,
confortable → Vitrine par défaut). La densité était déjà une déclaration
d'intention : passer en dense, c'est dire « je cherche » ; passer en
confortable, c'est dire « je regarde ». Les deux jeux d'images coexistent sur
disque, donc rebasculer est instantané une fois les deux produits.

## 11.2 La transparence était un moyen, pas une fin

Le §5.6 impose un fond entièrement transparent, et son argument est juste : le
fond devient du CSS, donc il suit le thème sans jamais régénérer une image.
Mais c'est un moyen au service d'un but — que la carte possède le fond — et
deux des trois presets poursuivent un autre but :

- un **reflet** est une modulation de la luminosité d'un sol ; il n'a rien à
  moduler sur du transparent. Vitrine cuit donc une flaque dans l'image.
- **indiscernable** exige que le fond soit dans le fichier, comme il l'est chez
  Kunos. Officiel cuit son fond entier.

Conséquence à ne pas rater : une image qui porte son fond doit **correspondre à
sa carte**, sinon la flaque se lit comme une soucoupe posée dessus. C'est
pourquoi le fond de carte fait partie du preset, et pourquoi il entre dans
l'empreinte d'une vignette — mais **seulement quand il est cuit**. Toujours le
compter ferait régénérer 312 images pour un changement de thème ; ne jamais le
compter laisserait un fond peint qui ne correspond plus à rien.

Le §5.6 écarte aussi le reflet miroir : « il consommerait la moitié du cadre ».
L'argument valait pour un objectif de lisibilité et pour un reflet pleine
hauteur ; coupé court, il en prend un quart, et c'est un échange légitime quand
l'objectif est le plaisir des yeux.

## 11.3 Ce que le §5.6 ne pouvait pas prévoir : la version du code qui dessine

Une vignette porte le `.kn5` et sa date, la livrée, les `ext_config.ini`, la
version du convertisseur et le gabarit. Elle ne portait **rien du code qui
dessine** — si bien qu'une correction de rendu laissait ses images fausses
servies pour toujours, parfaitement valides au regard de tout ce que leur nom
savait vérifier. Vécu au premier bug de rendu : un fond cuit qui recouvrait la
voiture, et 107 vignettes noires que rien n'aurait ramassées.

`RENDERER_VERSION` comble ça, exact analogue de `CONVERTER_VERSION` côté
conversion : à incrémenter dès qu'une correction change les pixels produits.

## 11.4 Deux mesures qui ont corrigé la spec

Le §5.6 demande de relever l'azimut et le cadrage sur les previews Kunos plutôt
que de les inventer. Fait, sur les 178 voitures officielles — et deux résultats
allaient contre ce que l'œil suggérait :

- **leur fond est plat et quasi noir** (luminance 4 dans les coins comme
  au-dessus de la voiture). Ce qu'on prend pour un halo derrière la voiture est
  la flaque au sol devant elle, seule zone claire du cadre. Un dégradé radial
  derrière la voiture aurait été une erreur ;
- **la voiture n'occupe que 66 % de la largeur** du cadre, et elle y est basse.

Piège méthodologique qui a coûté un aller-retour : **les deux côtés se mesurent
avec le même instrument**. Mesurer leur cadrage et *estimer* le nôtre a donné
une marge trois fois trop grande, parce que la marge de Pit Box n'est pas « le
pourcentage de cadre laissé vide » — à marge nulle, la voiture n'occupe déjà
que ~75 % de la largeur, le cadrage ajustant la boîte englobante **en 3D**
projetée, plus grande que la silhouette visible.

---

*Pit Box · spécification grille · maquette de référence `pit-box-barre-et-grille.html`*
