# Maquettes

**Une maquette est la trace d'une décision à une date, pas une cible qui se met
à jour.** Elle ne se maintient donc pas : quand l'écran évolue, c'est la spec
qui suit, et la maquette passe dans `archive/` en gardant le *pourquoi* qu'elle
porte. C'est la raison pour laquelle chaque ligne ci-dessous est datée — sans la
date, on ne sait pas si on regarde la cible ou un souvenir.

**Elles apportent l'UX, jamais l'UI.** On en reprend la disposition, l'ordre de
lecture, ce qui est groupé avec quoi, quel geste fait quoi. Les couleurs, les
polices et les hauteurs de contrôle viennent de l'application — voir
`CLAUDE.md`, « Le design system fait foi, pas la maquette ». Une maquette qui
écrit `--text-secondary` ou un cinquième gris se trompe sur ce point précis :
c'est elle qui s'aligne sur l'app.

En cas d'écart entre une maquette et sa spec, **la spec fait foi**.

## À jour

| Maquette | Date | Ce qu'elle a servi à décider |
| --- | --- | --- |
| `pitbox-index-pays.html` | 2026-09-23 | L'index par pays de l'écran Circuits : la tuile pose une puce, la croix ramène à l'index. Sa bascule vers des voitures « groupées par marque » est une piste **non retenue** — c'est un index de marques qui a été construit. Spec : `SPEC-index-bibliotheque.md`. |
| `pitbox-index-voitures.html` | 2026-09-23 | L'index des voitures, familles puis marques, et le croisement ET d'une seconde famille. **Sa section « Parcourir par pays » est à ne pas construire** (INDEX§6.3) : elle n'existe que pour juger de l'effet de mur. Spec : `SPEC-index-bibliotheque.md`. |
| `pitbox-maquettes.html` | 2026-09-11 | Les dix écrans de la refonte de navigation : rail à deux rangs, inventaire unique des compléments, anatomie de fiche commune. Sélecteurs de livrée et de tracé interactifs. Spec : `SPEC-refonte-navigation-et-fiches.md`. |
| `pitbox-ecran-pilote.html` | 2026-09-01 | L'écran Pilote et son geste central : survol = essai, clic = adoption. Montre les trois modes (corps d'origine, corps substitué, corps sans casque applicable). Spec : `SPEC-ecran-pilote.md`. |
| `pitbox-onglet-medias_1.html` | 2026-08-09 | L'onglet Médias d'une fiche voiture — captures, replays, backgrounds. |
| `pitbox-a-propos.html` | 2026-07-11 | L'écran « À propos » : identité, outils tiers, soutien, licences open source, mentions légales. Citée par `About.svelte`. |

## `archive/` — périmées, gardées pour le *pourquoi*

Elles montrent une navigation ou des écrans qui n'existent plus. On ne les
consulte pas pour savoir à quoi ressemble l'app, mais pour retrouver l'intention
d'origine d'un choix — et parfois pour comprendre *pourquoi* il a été défait.

| Maquette | Date | Ce qu'elle montrait, et ce qui l'a remplacée |
| --- | --- | --- |
| `filtre-tags-mockup.html` | 2026-08-22 | Les filtres de bibliothèque en barre de contrôles permanente. Remplacée par les **puces** (`SPEC.md` §7.1) : onze contrôles sur ~200 px de hauteur pour quelqu'un qui en emploie un ou deux. Reste utile pour le *vocabulaire* des filtres. |
| `pitbox-biblio-session2.html` | 2026-07-03 | Barre latérale unifiée, bloc Session en haut, Add-ons et Atelier en deux colonnes. Antérieure au rail à deux rangs. |
| `pitbox-reglages-session.html` | 2026-07-04 | Réglages de session : adversaires en quatre modes, météo SVG, fourchette IA. Antérieure à la refonte du plateau et du bloc Conditions. |
| `pitbox-source-pack.html` | 2026-06-30 | L'affichage du pack d'origine : voitures sœurs, filtrer et désinstaller par pack. |
| `pitbox-fiche-B-revisee.html` | 2026-06-29 | Fiche voiture, image héros à gauche et données à droite. Antérieure à l'anatomie de fiche commune de la refonte. Encore citée par `DetailPage.svelte`, qui a depuis été réécrit. |
| `pitbox-vues-transversales.html` | 2026-06-29 | Les trois écrans transversaux Skins / Sons / Apps. **Supprimés** : ils classaient par mécanique d'installation, et leur contenu est dans l'inventaire des compléments. |
| `pitbox-mockup.html` | 2026-06-29 | La toute première maquette interactive. C'est d'elle que `styles/global.css` a tiré les jetons du design system — le fichier le dit en tête. À ce titre elle est la source d'origine de l'UI, même si aucun de ses écrans ne ressemble encore à l'app. |
