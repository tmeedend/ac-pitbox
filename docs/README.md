# docs/ — Index

Documentation de conception de Pit Box (gestionnaire de mods Assetto Corsa). Ce fichier liste tout le contenu de `docs/` pour savoir où regarder.

## Référence principale

- **`SPEC.md`** — spécification de référence, organisée par domaine (architecture, identité/import, tags, fiche technique, bibliothèque, skins/sons/apps, lancement de session, maintenance, config, conventions). **Point d'entrée** : commencer ici. Décrit l'app telle qu'elle fonctionne.

## Import (le domaine le plus dense)

- **`SPEC-import.md`** — l'arbre de décision de l'import et la table des mécanismes de pose, sur une page. Ne remplace pas `SPEC.md` §4, il le **rend vérifiable** : une seule question (« où va ce fichier ? »), un seul arbre, une seule table de destinations. À lire **avant** de toucher à une règle d'import, et à rejouer contre les cinq archives de référence qu'il liste. En cas d'écart, `SPEC.md` fait foi — et l'un des deux est à corriger tout de suite.

## Données embarquées

- **`kunos_content_dates.json`** — table statique du contenu officiel Kunos (178 voitures + 21 circuits, tirés des dossiers réels). Pour chaque entrée : `year` (année du modèle) et `release` (date de sortie dans AC via son pack). Sert à renseigner l'année et la date de publication du contenu de base. Section `packs` = dates des DLC.
- **`default-tag-rules-enriched.json`** — ontologie de tags (vocabulaire fermé + règles fusion/suppression/déduction/extraction/brand_fix). Chargée au démarrage, éditable via l'écran de règles.

## Maquettes (références visuelles / UX)

- **`pitbox-biblio-session2.html`** — barre latérale unifiée : bloc Session en haut + Add-ons/Atelier en deux colonnes. **Référence de l'écran principal.**
- **`pitbox-reglages-session.html`** — écran de réglages de session : adversaires (4 modes + plateau ajustable), météo SVG, fourchette IA, simulation commune à tous les types, options visibles.
- **`pitbox-fiche-B-revisee.html`** — fiche voiture : image héros à gauche, données à droite, skins/distance/son/tags en bas. **Référence de layout de fiche.**
- **`pitbox-vues-transversales.html`** — vues transversales Skins / Sons / Apps.
- **`pitbox-source-pack.html`** — affichage du pack d'origine (voitures sœurs, filtrer/désinstaller par pack).
- **`pitbox-a-propos.html`** — écran « À propos » : identité, outils tiers (Assetto Corsa/Content Manager/QuickBMS), soutien (Patreon/OverTake), licences open source, mentions légales.

## Écran Pilote (chantier en cours)

- **`SPEC-ecran-pilote.md`** — spécification UX/UI de l'écran de choix du pilote : le corps (mannequin 3D) et la tenue en trois pièces. Porte l'asymétrie fondatrice — le corps est imposé par la physique de la voiture, la tenue tient à un fichier de skin — et les sept décisions dont tout le reste découle. **Point d'entrée du chantier.**
- **`csp-driver-research.md`** — **appliquer le pilote en jeu** : ce qui marche (`[DRIVER3D_MODEL]` d'un `ext_config.ini` pour le corps, le `skin.ini` de la livrée pour la tenue), ce qui ne marche pas, et les quatre pistes explorées puis écartées. À lire **avant** de retenter quoi que ce soit dans cette direction.
- **`pitbox-ecran-pilote.html`** — la maquette qui l'accompagne, interactive : survol = essai, clic = adoption, et les trois modes (corps d'origine, corps substitué, corps sans casque applicable).

## Aperçu 3D des voitures (chantier en cours)

- **`SPEC-preview-3d-kn5.md`** — spécification du rendu 3D natif : parsing KN5 en Rust → glTF → three.js dans la webview. Décision d'architecture, layout binaire du format, plan par lots. **Point d'entrée du chantier.**
- **`kn5-format.md`** — ce que le format fait *vraiment*, mesuré sur des fichiers réels : écarts constatés avec la spec et réponses à ses questions ouvertes, avec la méthode de vérification. À mettre à jour à chaque découverte.
- **`SPEC-engine-sound-fmod.md`** — écouter le vrai moteur d'une voiture en passant par les DLL FMOD livrées avec Assetto Corsa, au lieu de deviner le ralenti par analyse du signal. Contient la position prise sur la licence FMOD, et surtout les **écarts d'ABI mesurés** en §2bis, dont une structure dont la disposition documentée est fausse d'une manière qui ressemble à un succès.
- **`fsb5-format.md`** — le format des banks de son FMOD (`.bank`), mesuré de la même façon : conteneur FSB5, codec FADPCM, et les hypothèses écartées. Lu pour auditionner un mod de son sans lancer le jeu. **Son heuristique de ralenti est une impasse assumée** (40 sur 91) : elle ne sert plus que de repli, et ne doit pas être retouchée — le §« Trouver le ralenti » explique pourquoi.
- **`SPEC-engine-sound-fmod.md`** — chantier suivant : jouer l'événement moteur avec le FMOD livré par Assetto Corsa, au lieu de deviner le ralenti. Faits vérifiés, position sur la licence, plan par lots.
- **`showroom-3d-preview-research.md`** — recherche préalable : les trois pistes explorées (fenêtre Content Manager, parser maison, `acShowroom.exe`), et pourquoi l'intégration de la fenêtre native a été abandonnée. À lire avant de retenter quoi que ce soit dans cette direction.

## Code de référence

- **`archives.py`** — logique d'import/détection de l'ancien outil Python (`isCar`/`isTrack`/`isCarSound`, descente récursive). À **porter** dans le backend Rust, jamais exécutée. ⚠️ Ne jamais réécrire les `ui_*.json` (contrairement à ce code) — l'app est non destructive.

## Méthode de travail

- Le SPEC décrit *ce que l'app est*. Les **bugs** se corrigent directement avec Claude Code (pas dans le SPEC). Les **consignes d'implémentation** ciblées se donnent par sujet, en pointant la section concernée du SPEC — ne jamais donner tout le SPEC en demandant « construis l'app ».
- Langue de travail : français. Thème Rosso Corsa (voir §12 du SPEC).
