# Instruction Claude Code — Lot 4 : presets d'état de piste de Content Manager

> **Note ajoutée au versionnement, le reste du document est inchangé.**
>
> Ce lot est **livré**. Le document est conservé parce que le code y renvoie
> (`L4§…`) et parce qu'il porte les *arguments* derrière les choix — la
> partie qu'on ne retrouve pas deux fois. `SPEC-session.md` décrit ce que
> l'écran **est** aujourd'hui ; celui-ci dit ce qui a été demandé et pourquoi.
> En cas d'écart, `SPEC-session.md` fait foi.


**Prérequis** : §2.2 livré (bloc `TRACK CONDITION` dans le rail droit, select des sept entrées natives, ligne de lecture des quatre valeurs, modèle « objet nommé porteur de quatre valeurs »).

**Objet du lot** : lire les presets d'état de piste créés par l'utilisateur dans Content Manager et les proposer à côté des sept entrées natives. En lecture seule.

**Décision prise, à ne pas rouvrir ici** : Pit Box n'ouvre pas l'édition des quatre valeurs et ne permet pas de créer ses propres presets. La fonctionnalité existe dans CM depuis des années et le dossier de presets personnalisés est vide sur la machine de test — ce n'est pas un manque ressenti. Ce lot récupère l'essentiel du bénéfice (l'utilisateur avancé retrouve ses états nommés) sans écran de gestion à construire.

---

## Étape 0 — relevé, avant toute ligne de code

Rien de ce qui suit ne doit être supposé. Relève, puis implémente :

1. **Chemin exact** du dossier de presets d'état de piste de CM, et comment il est déterminé : chemin fixe, relatif à l'installation de CM, ou configurable par l'utilisateur.
2. **Format et extension** des fichiers. Un preset = un fichier, ou un fichier de collection ?
3. **Champs présents, noms et échelles brutes**, par rapport à ce que CM affiche dans son éditeur (`Initial grip`, `Grip transfer`, `Grip randomization`, `Lap gain`). Le point sensible est l'échelle : un pourcentage affiché peut être stocké en 0–1. Toute conversion doit venir du relevé, pas d'une supposition.
4. **La description libre est-elle stockée** dans le preset ?
5. **Identité d'un preset** : nom de fichier, champ interne, identifiant ?
6. **Mécanisme déjà en place** pour lire les six états natifs du jeu — réutilisable tel quel pour ces fichiers, ou chemin de lecture distinct ?

Si le relevé contredit quoi que ce soit dans ce document, remonte-le avant d'implémenter.

---

## 4.1 Les sept entrées natives restent toujours présentes

Deux groupes dans le select :

```
Built-in              Auto (set by weather) / Dusty / Old / Green / Slow / Fast / Optimum
From Content Manager  presets utilisateur, ordre alphabétique
```

Les natives ne sont **jamais** remplacées ni masquées par les presets utilisateur. Elles viennent du fichier du jeu, ce sont les noms que tout le monde emploie, et quelqu'un qui s'est fabriqué une piste verte humide veut quand même pouvoir choisir `Optimum`.

Le second groupe est simplement **absent** quand le dossier est vide ou introuvable. Aucun message, aucune alerte, aucun état d'erreur : c'est le cas nominal, et c'est celui de la machine de test.

## 4.2 Lecture seule, strictement

Pit Box ne crée, ne modifie, ne supprime et ne réécrit **aucun** fichier du dossier de CM. Si la création de presets Pit Box arrive un jour, elle ira dans un dossier propre et formera un troisième groupe.

## 4.3 Moment de la lecture

À l'**ouverture de l'écran Session setup**, pas au démarrage de l'application. Le scénario réel est : créer un preset dans CM, revenir dans Pit Box. Une lecture au démarrage obligerait à relancer l'app.

## 4.4 Validation et tolérance aux fichiers cassés

- **Bornage à la lecture** sur les plages de l'éditeur de CM : initial grip 85–100 %, grip transfer 0–100 %, randomization 0–100 %, lap gain 0–700. À confirmer par le relevé.
- **Conversion d'échelle** s'il y a lieu, d'après le relevé.
- Un fichier illisible, mal formé, ou dont les valeurs sont irrécupérables est **ignoré en silence**. Pas de message, pas de log visible à l'utilisateur, pas de blocage.

Motif : c'est un enrichissement optionnel. Il ne doit jamais empêcher de régler ou de lancer une session.

## 4.5 Description

Si le preset porte une description, l'afficher **sous la ligne des quatre valeurs**, en `--text-secondary`, en sans-serif (c'est de la prose, pas une donnée). Description absente ou vide : rien, pas de placeholder.

C'est la partie la plus utile de ce lot pour l'utilisateur : ce sont ses propres mots pour décrire l'état qu'il a composé, et c'est exactement ce qui manquait pour départager deux états proches.

Si les six états natifs portent eux aussi une description accessible par le même mécanisme (CM en affiche une — `Perfect track for hotlapping.` pour Optimum), l'afficher également. Sinon, ne pas l'inventer.

## 4.6 Identité et collision de noms

Un utilisateur peut nommer son preset `Green`. À l'écran, l'appartenance au groupe suffit à distinguer. **Au stockage, non** : une session sauvegardée référence un état de piste par **origine + nom** (`built-in` / `cm`, puis le nom), jamais par le nom seul.

## 4.7 Portabilité des sessions sauvegardées

Ce qui part au jeu, ce sont les quatre nombres — le nom n'est qu'une étiquette. Une session sauvegardée stocke donc **les quatre valeurs résolues, en plus de la référence au preset**.

Conséquences, à implémenter explicitement :

- Un preset CM supprimé ou renommé ne modifie **jamais silencieusement** une session déjà enregistrée.
- **Référence introuvable au chargement** : les quatre valeurs mémorisées sont appliquées telles quelles, et le select affiche le nom mémorisé suivi de `(missing)` en `--text-disabled`. Aucun blocage, aucune boîte de dialogue.
- **Référence trouvée mais valeurs différentes de celles mémorisées** : les valeurs du preset gagnent. L'utilisateur a modifié son preset dans CM, c'est un geste intentionnel et il attend que ses sessions suivent.

Ce dernier point est le seul arbitrage réellement ouvert de ce lot. L'autre branche (la session prime, le preset n'est qu'un point de départ figé au moment de la sauvegarde) se défend aussi. Signale-le si l'implémentation fait apparaître une raison de trancher autrement.

## 4.8 Aucune édition dans cet écran

Le bloc `TRACK CONDITION` reste tel que livré en §2.2 : un select, une ligne de lecture, rien d'éditable. Pas de curseurs, pas de bouton d'enregistrement, pas de champ de description modifiable.

Qui veut composer un état le fait dans CM et le retrouve ici — ce que ce lot rend précisément possible.

---

## Hors périmètre

- **Édition des quatre valeurs dans Pit Box** et création/gestion de presets Pit Box. Voir la décision en tête de document.
- **`Share link` de CM** et toute reprise de la base en ligne d'AcTools. Si un preset importé par lien atterrit dans le même dossier local, il sera lu comme les autres, sans traitement particulier.
- **SPEC.md** : à mettre à jour au présent, par tes soins, une fois le lot passé.
