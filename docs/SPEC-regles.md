# Pit Box — règles et catalogue livré

## Spécification d'implémentation

*Comment livrer des règles par défaut, les faire évoluer d'une version à l'autre, et ne jamais écraser le travail de l'utilisateur. Couvre l'écran `Atelier › Règles`.*

---

# 1. Le problème

Pit Box installe un jeu de règles à la première installation : deviner les catégories, corriger les pays et les marques, compléter la fiche technique à partir des tags. L'utilisateur les modifie, en désactive, en ajoute.

Puis une nouvelle version de Pit Box apporte de meilleures règles. Et il n'existe aucune bonne réponse : écraser détruit son travail, ne rien faire le prive des améliorations, et demander à chaque mise à jour l'épuise.

**La cause n'est pas la mise à jour, c'est le stockage.** Règles livrées et règles utilisateur vivent aujourd'hui dans le même ensemble, donc rien ne permet de savoir ce qui peut être remplacé.

**On ne peut pas fusionner ce qu'on ne sait pas distinguer.**

---

# 2. Deux couches, jamais fusionnées en base

| Couche | Contenu | Cycle de vie |
|---|---|---|
| **Catalogue** | les règles livrées avec Pit Box | **lecture seule**, versionné, remplacé en bloc à chaque mise à jour |
| **Surcouche** | les règles de l'utilisateur **et ses décisions sur celles du catalogue** | jamais touchée par une mise à jour |

L'utilisateur **ne modifie jamais une règle du catalogue en place**. Ses actions sur une règle livrée sont enregistrées dans la surcouche, comme des décisions :

| Action de l'utilisateur | Ce qui est stocké |
|---|---|
| Désactiver une règle livrée | `{ ruleId, disabled: true }` |
| Modifier une règle livrée | `{ ruleId, fork: <la règle modifiée>, forkedFrom: <hash> }` |
| Réordonner ses propres règles | ordre dans la surcouche |
| Créer une règle | une règle utilisateur complète |

Mettre à jour devient alors mécanique : **remplacer la couche catalogue, ne pas toucher à la surcouche.** L'utilisateur ne peut rien perdre, structurellement — pas parce qu'on a fait attention, mais parce que son travail n'est pas dans le fichier qu'on remplace.

Bénéfice secondaire : « rétablir le défaut » n'est pas une fonctionnalité à écrire, c'est la suppression d'une entrée de surcouche.

---

# 3. La cascade

Les règles ne sont qu'un étage d'un empilement plus large. Chaque niveau l'emporte sur celui du dessous.

```
1.  métadonnées brutes du mod
2.  ↓  catalogue Pit Box
3.  ↓  règles de l'utilisateur
4.  ↓  corrections manuelles sur un mod précis
```

C'est la même forme que la résolution des taxonomies (TAXO§2) : une valeur dérivée, surchargeable.

**Conséquence importante :** une nouvelle règle du catalogue qui entrerait en concurrence avec une règle utilisateur, ou avec une correction manuelle, **perd silencieusement**. Il n'y a pas de conflit à arbitrer, donc pas de boîte de dialogue à concevoir. La précédence règle le problème avant qu'il n'apparaisse.

**Ordre d'exécution :** par couche, puis par rang à l'intérieur de la couche. L'utilisateur réordonne ses propres règles ; il ne réordonne pas celles du catalogue entre elles — s'il veut qu'une règle livrée cesse de l'emporter sur une autre, il la désactive.

**Traçabilité obligatoire.** Chaque valeur dérivée retient d'où elle vient : `{ value, source: 'raw' | 'catalog' | 'user' | 'manual', ruleId? }`. C'est ce qui permet d'expliquer « pourquoi cette voiture est classée Drift » sur la fiche d'un mod, et de rendre le rapport de §6 possible.

---

# 4. Identité des règles du catalogue

**Les identifiants sont écrits à la main, stables, et ne dérivent jamais du contenu.**

```
pitbox.country.jp-aliases
pitbox.category.guess-drift
pitbox.techsheet.drivetrain-from-tag
```

| Situation | Ce qu'il faut faire |
|---|---|
| Une règle est corrigée, améliorée, réécrite, mais fait le même travail | **garder l'identifiant** — l'utilisateur qui l'avait désactivée la garde désactivée |
| Une règle fait désormais autre chose | nouvel identifiant, et l'ancien passe en `retired` dans le catalogue |
| Une règle disparaît | elle passe en `retired`, elle n'est pas simplement supprimée |

`retired` plutôt que supprimé, parce que la surcouche peut encore référencer l'identifiant. Une entrée de surcouche qui pointe vers une règle retirée est conservée sans erreur ; si l'utilisateur l'avait dérivée, la dérivation devient une règle utilisateur ordinaire, et le rapport de mise à jour le dit.

**Chaque règle du catalogue porte un hash de contenu.** C'est ce qui permet de savoir, à la mise à jour, si une règle a changé depuis que l'utilisateur l'a dérivée.

---

# 5. Désactiver et dériver ne sont pas la même chose

**Distinction structurante, à ne pas confondre à l'implémentation.**

| | Désactivée | Dérivée |
|---|---|---|
| Ce qui est stocké | un drapeau | une copie complète de la règle |
| Reçoit les futures améliorations | **oui** — elle reste simplement éteinte | **non** — elle est gelée sur la version dérivée |
| Réversible par | un clic sur la bascule | « rétablir le défaut » |
| Fréquence attendue | élevée | faible |

Traiter une désactivation comme une dérivation ferait perdre la majorité des améliorations, puisque désactiver est de loin l'action la plus courante.

**Corollaire d'interface :** la bascule d'activation doit être immédiate et sans friction, visible sur la ligne de la règle. Modifier une règle livrée doit au contraire être un geste explicite, qui annonce ce qu'il fait — voir §8.

---

# 6. Mise à jour du catalogue

## 6.1 Les cinq cas

Pour chaque règle du nouveau catalogue :

| # | Cas | Comportement |
|---|---|---|
| 1 | Règle nouvelle | **appliquée**, comptée dans le rapport |
| 2 | Règle modifiée, jamais touchée par l'utilisateur | **mise à jour**, comptée dans le rapport |
| 3 | Règle modifiée, **désactivée** par l'utilisateur | mise à jour, **reste désactivée**, mentionnée dans le rapport |
| 4 | Règle modifiée, **dérivée** par l'utilisateur | la dérivation est conservée ; la ligne porte *« une nouvelle version existe »* avec un aperçu des différences |
| 5 | Règle `retired` | retirée ; si elle était dérivée, la dérivation devient une règle utilisateur ordinaire |

Aucun de ces cas ne détruit quoi que ce soit dans la surcouche.

## 6.2 Appliquer puis rendre compte — jamais faire approuver avant

**Décision de conception, contre-intuitive mais ferme.**

Un assistant de mise à jour qui présente douze règles à cocher demande une décision sur des objets dont l'utilisateur n'a jamais entendu parler, et dont il ne peut pas voir l'effet. Il coche tout, ou rien, au hasard — et dans les deux cas il n'a rien décidé.

Le catalogue s'applique donc **automatiquement**, et l'utilisateur est informé **après**, avec l'effet réel sous les yeux :

```
⚑  Catalogue de règles mis à jour — v13 → v14
    12 règles ajoutées · 3 corrigées · 1 retirée
    47 mods reclassés
                                    [ Voir le détail ]   [ Revenir à v13 ]
```

Bandeau non bloquant, en tête de l'écran Règles et dans les notifications. Il persiste jusqu'à ce que l'utilisateur le referme — ce n'est pas un toast (même raisonnement que GRILLE§8.1).

**L'information vient avant la décision, et l'annulation coûte un clic.** C'est l'inverse d'un assistant : la décision avant l'information, et l'annulation impossible.

## 6.3 Le détail

L'écran de détail liste ce qui a changé, groupé par type :

```
AJOUTÉES (12)
  ⊕  Deviner « Prototype » depuis #lmh          a classé 4 mods     [ Désactiver ]
  ⊕  Corriger le pays « Nurburgring » → DE      a corrigé 2 mods    [ Désactiver ]
  …
CORRIGÉES (3)
  ⟳  Deviner « Drift » depuis le nom de dossier  +6 mods, −1 mod    [ Voir ]
  …
RETIRÉE (1)
  ⊖  Deviner la transmission depuis #awd        3 mods reviennent à leur valeur brute
```

Chaque ligne nomme **l'effet mesuré**, pas la règle seule. `a classé 4 mods` est ce qui permet de juger ; l'énoncé de la règle ne suffit pas.

Désactiver depuis cet écran est un geste ordinaire (§5) : la règle reste dans le catalogue et continuera de recevoir les améliorations.

## 6.4 Revenir en arrière

`Revenir à v13` restaure la version précédente du catalogue **sans toucher à la surcouche**. Disponible jusqu'à la mise à jour suivante ; au-delà, seules les désactivations individuelles restent.

Une seule version précédente est conservée. Un historique complet serait du stockage pour un cas qui ne se présente pas.

---

# 7. Interrupteur global

Un réglage, dans `Atelier › Règles` :

```
[✓]  Utiliser le catalogue de règles Pit Box          v14 · 68 règles
```

Décoché, le catalogue entier cesse de s'appliquer. Les règles de l'utilisateur, elles, continuent — la surcouche est indépendante.

C'est la sortie pour qui veut tout contrôler. Elle doit exister, et elle ne doit pas être enfouie dans les Réglages généraux : elle appartient à l'écran qu'elle gouverne.

---

# 8. L'écran `Atelier › Règles`

## 8.1 Une seule liste

Règles du catalogue et règles utilisateur cohabitent dans une liste unique, **dans l'ordre d'exécution** (§3) — c'est le seul ordre qui explique le résultat.

```
[✓]  PIT BOX    Corriger le pays « Nurburgring » → DE        2 mods      ⋮
[ ]  PIT BOX    Deviner « Drift » depuis le nom de dossier   —           ⋮
[✓]  PIT BOX ✎  Deviner « Classic » si année < 1975          64 mods     ⋮   ⚑ nouvelle version
[✓]             Si le dossier contient « rss » → auteur RSS  31 mods     ⋮
```

| Élément | Rôle |
|---|---|
| Bascule | active / désactive — immédiate, sans confirmation |
| Badge `PIT BOX` | distingue le catalogue ; absence de badge = règle de l'utilisateur |
| `✎` après le badge | règle du catalogue **dérivée** |
| Compteur | nombre de mods réellement affectés, `—` si aucun |
| `⚑` | une nouvelle version de cette règle existe (cas 4 de §6.1) |
| `⋮` | modifier, dupliquer, rétablir le défaut, supprimer |

**Les badges sont légitimes ici.** C'est une surface de gestion, pas une surface de jeu — exactement le raisonnement qui les a fait retirer des cartes de la bibliothèque (GRILLE§2.4). Le critère n'a pas changé : un marqueur a sa place là où l'utilisateur gère, pas là où il choisit une voiture pour rouler.

**Filtre `Mes règles seulement`**, en tête de liste. Soixante-huit règles livrées noient les cinq de l'utilisateur.

## 8.2 Modifier une règle du catalogue

Le geste est explicite et annonce sa conséquence :

```
Cette règle fait partie du catalogue Pit Box.
La modifier en fera une copie à vous : elle ne recevra plus les
améliorations des prochaines versions.

    [ Modifier quand même ]   [ Désactiver plutôt ]   [ Annuler ]
```

`Désactiver plutôt` est proposé en second parce que c'est très souvent ce que l'utilisateur voulait vraiment — et parce que c'est réversible sans perte (§5).

Une fois dérivée, la ligne porte `✎`, et `Rétablir le défaut` apparaît dans son menu.

## 8.3 Compteur d'effet

Chaque règle affiche le nombre de mods qu'elle affecte réellement, dans l'état courant de la cascade.

Une règle du catalogue qui affiche `—` est soit inutile pour cette bibliothèque, soit masquée par une règle utilisateur plus prioritaire. C'est la meilleure information possible pour décider de la désactiver, et c'est aussi le moyen le plus rapide de repérer une règle qui ne fait pas ce qu'on croit.

Recalculé à la volée après chaque changement, pas au lancement seulement.

---

# 9. Export et import

**L'export ne contient que la surcouche** — règles de l'utilisateur, désactivations, dérivations, corrections manuelles. Jamais le catalogue.

Exporter le catalogue figerait le destinataire sur une version ancienne, et gonflerait le fichier de données identiques chez tout le monde.

L'export porte la version de catalogue sous laquelle il a été produit. À l'import, si le destinataire est sur une version différente :

- les désactivations et dérivations dont l'identifiant existe encore s'appliquent ;
- celles qui pointent vers une règle `retired` sont ignorées, et listées dans le rapport d'import ;
- les règles utilisateur s'appliquent toutes, elles ne dépendent de rien.

Ce comportement découle de §4 : c'est la stabilité des identifiants qui rend le partage possible entre versions.

---

# 10. Première installation

Le catalogue s'installe complet et actif. La surcouche est vide.

**Pas d'écran de choix des règles à l'installation.** Même raisonnement qu'en §6.2 : demander à quelqu'un qui n'a encore rien vu quelles règles de classement il veut, c'est demander une décision sans information.

---

# 11. Ce qui n'est pas une règle

Rappel de TAXO§8 : les correspondances **exactes** — `Alfa` → `Alfa Romeo`, `JP` → `Japon`, `#lmp1` → famille `Prototype` — ne sont pas des règles. Ce sont des tables d'index, et elles vivent dans les onglets `Marques`, `Pays` et `Catégories`.

Ce qui reste dans `Règles` est **heuristique** : deviner depuis un nom de dossier, depuis une année, depuis la présence d'un mot. C'est précisément ce qui peut se tromper, donc ce qui mérite un compteur d'effet, une bascule et un rapport de mise à jour.

Les deux mécanismes partagent malgré tout le modèle de couches de cette spec : le catalogue livre aussi des alias par défaut, et l'utilisateur peut les surcharger de la même façon.

---

# 12. Repères visuels

| Rôle | Valeur |
|---|---|
| Badge `PIT BOX` | 9 px, `.12em`, `--txt-3`, bordure `--line`, rayon 2 px |
| Marque de dérivation `✎` | `--txt-2` |
| Fanion de nouvelle version `⚑` | `#c88a2a` |
| Bandeau de mise à jour | fond `#1b1811`, filet gauche 2 px `#c88a2a` |
| Ligne désactivée | `opacity: .55` |

Aucun accent rouge dans cet écran hors focus clavier : rien n'y relève de la session (barème de l'accent, `SPEC.md` §7.2ter).

---

# 13. Migration des installations existantes

Sur une installation existante, règles livrées et règles utilisateur sont dans le même ensemble. Rien ne les distingue. C'est la première application du modèle de couches, sur des données qui n'en ont pas.

## 13.1 Prérequis — le manifeste des catalogues passés

**À embarquer dès maintenant, avant toute livraison.** Pour chaque version publiée de Pit Box, la liste des règles qu'elle livrait, avec identifiant et hash de contenu.

```
manifests/
  v1.2.json   → [{ id: 'pitbox.country.jp-aliases', hash: 'a3f…' }, …]
  v1.3.json
  v1.4.json
```

C'est quelques kilo-octets, et c'est la **seule** chose qui rende la migration possible. Sans manifeste, elle est aveugle.

## 13.2 Classement

Pour chaque règle trouvée chez l'utilisateur, comparée au manifeste de **sa** version :

| Ce qu'on trouve | Conclusion | Entrée de surcouche |
|---|---|---|
| Hash identique à une règle du manifeste | règle livrée intacte | **aucune** — simple référence au catalogue |
| Identifiant connu, hash différent | **dérivation** | `{ ruleId, fork, forkedFrom }` |
| Règle du manifeste **absente** chez l'utilisateur | **supprimée par lui** | `{ ruleId, disabled: true }` |
| Aucune correspondance | règle à lui | règle utilisateur |

**La ligne « absente » est la plus importante.** Si le système actuel permet de supprimer une règle livrée plutôt que de la désactiver, et que rien n'en garde trace, la migration la ressusciterait : quarante mods reclassés sans explication. Seule la comparaison au manifeste détecte une absence.

## 13.3 Si les règles actuelles n'ont pas d'identifiant stable

**À vérifier dans le code avant de chiffrer la migration.** Si les règles sont stockées avec un identifiant auto-incrémenté et aucun nom stable, l'appariement ne peut se faire que par hash de contenu. Conséquence : les règles intactes et les suppressions sont détectées, **les dérivations ne le sont pas**.

Le cas non détecté est bénin, et la cascade le neutralise :

- la dérivation est classée comme règle utilisateur — rien n'est perdu ;
- la règle du catalogue correspondante est installée en plus — doublon ;
- mais la règle utilisateur est prioritaire (§3), donc **elle gagne** ;
- la règle du catalogue affiche `—` dans son compteur d'effet (§8.3) : le doublon est inerte et visible.

Le rapport de migration peut alors proposer, sans rien imposer :

```
3 règles du catalogue n'ont aucun effet — vos propres règles les masquent.
                                              [ Les désactiver ]   [ Laisser ]
```

## 13.4 Version inconnue du manifeste

Installation trop ancienne, ou base modifiée à la main : aucun manifeste ne correspond.

**Repli sûr :** toutes les règles existantes deviennent des règles utilisateur, et le catalogue est installé **entièrement désactivé**. La classification de la bibliothèque est préservée à l'identique, et l'utilisateur peut activer le catalogue quand il veut, avec le rapport d'effet de §6.

Ne jamais installer le catalogue actif dans ce cas : ce serait soixante-huit doublons et une bibliothèque reclassée sans raison visible.

## 13.5 Règle d'or — diff nul

**La bibliothèque doit être classée exactement pareil avant et après la restructuration.**

C'est le test d'acceptation, et il est automatisable :

1. instantané de la classification dérivée de tous les mods ;
2. migration ;
3. recalcul complet ;
4. diff — **attendu vide**.

Un diff non vide à cette étape est un bug de migration, pas une amélioration.

## 13.6 Deux temps, jamais un seul

| Temps | Contenu | Diff attendu |
|---|---|---|
| **1 — restructuration** | passage en deux couches, avec le contenu de l'**ancien** catalogue | **vide** (§13.5) |
| **2 — mise à jour** | nouvelles règles de la version, via le mécanisme ordinaire de §6 | non vide, **expliqué par le rapport** |

Faire les deux d'un coup produit un diff que personne ne peut expliquer — ni l'utilisateur, ni toi en support.

## 13.7 Ordre avec la migration des taxonomies

La spec taxonomies (TAXO§8) extrait les correspondances exactes de `Règles` vers les onglets `Marques`, `Pays` et `Catégories`.

**Cette extraction vient après le classement en couches, et transporte la couche :**

| Origine de la correction | Devient |
|---|---|
| Règle du catalogue | alias **catalogue** |
| Règle de l'utilisateur, ou dérivation | alias **utilisateur** |

Dans l'ordre inverse, tous les alias deviennent « utilisateur » — et l'utilisateur ne recevra plus jamais d'améliorations d'alias, sans que rien ne le signale.

## 13.8 Sauvegarde

La base est copiée avant migration, et conservée jusqu'à la mise à jour suivante. C'est quelques méga-octets, et ça rend le `Revenir à v13` de §6.4 vrai pour la migration aussi.

La migration est atomique : elle réussit entièrement ou ne s'applique pas.

---

# 14. Points ouverts

**La couche de corrections manuelles** (niveau 4 de §3) existe-t-elle déjà dans l'application ? Si l'utilisateur peut aujourd'hui corriger la catégorie d'un mod précis sans passer par une règle, ces corrections doivent être identifiées et préservées par la migration, au même titre que les règles. Sinon la couche est à créer.

**Le partage communautaire de règles** n'est pas couvert ici. Il utiliserait l'export de §9, mais poserait la question de la confiance : une règle importée est du code de classement écrit par un inconnu. À traiter séparément.

**Décidé et écarté :** un catalogue téléchargeable hors des mises à jour de l'application. Le modèle de couches le permettrait sans rien changer, mais le besoin n'existe pas et il imposerait une politique réseau.

---

*Pit Box · spécification règles et catalogue · à lire avec les specs taxonomies, navigation et accent*
