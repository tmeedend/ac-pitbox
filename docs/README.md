# docs/ — Index

Documentation de conception de Pit Box (gestionnaire de mods Assetto Corsa). Ce fichier liste tout le contenu de `docs/` pour savoir où regarder.

## Référence principale

- **`SPEC.md`** — spécification de référence, organisée par domaine (architecture, identité/import, tags, fiche technique, bibliothèque, skins/sons/apps, lancement de session, maintenance, config, conventions). **Point d'entrée** : commencer ici. Décrit l'app telle qu'elle fonctionne.

## Données embarquées

- **`kunos_content_dates.json`** — table statique du contenu officiel Kunos (178 voitures + 21 circuits, tirés des dossiers réels). Pour chaque entrée : `year` (année du modèle) et `release` (date de sortie dans AC via son pack). Sert à renseigner l'année et la date de publication du contenu de base. Section `packs` = dates des DLC.
- **`default-tag-rules-enriched.json`** — ontologie de tags (vocabulaire fermé + règles fusion/suppression/déduction/extraction/brand_fix). Chargée au démarrage, éditable via l'écran de règles.

## Maquettes (références visuelles / UX)

- **`pitbox-biblio-session2.html`** — barre latérale unifiée : bloc Session en haut + Add-ons/Atelier en deux colonnes. **Référence de l'écran principal.**
- **`pitbox-reglages-session.html`** — écran de réglages de session : adversaires (4 modes + plateau ajustable), météo SVG, fourchette IA, simulation commune à tous les types, options visibles.
- **`pitbox-fiche-B-revisee.html`** — fiche voiture : image héros à gauche, données à droite, skins/distance/son/tags en bas. **Référence de layout de fiche.**
- **`pitbox-vues-transversales.html`** — vues transversales Skins / Sons / Apps.
- **`pitbox-source-pack.html`** — affichage du pack d'origine (voitures sœurs, filtrer/désinstaller par pack).

## Code de référence

- **`archives.py`** — logique d'import/détection de l'ancien outil Python (`isCar`/`isTrack`/`isCarSound`, descente récursive). À **porter** dans le backend Rust, jamais exécutée. ⚠️ Ne jamais réécrire les `ui_*.json` (contrairement à ce code) — l'app est non destructive.

## Méthode de travail

- Le SPEC décrit *ce que l'app est*. Les **bugs** se corrigent directement avec Claude Code (pas dans le SPEC). Les **consignes d'implémentation** ciblées se donnent par sujet, en pointant la section concernée du SPEC — ne jamais donner tout le SPEC en demandant « construis l'app ».
- Langue de travail : français. Thème Rosso Corsa (voir §12 du SPEC).
