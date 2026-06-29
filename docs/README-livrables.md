# Pit Box — Livrables à jour pour Claude Code

Ce dossier contient l'ensemble de référence du projet. Voici le rôle de chaque fichier
et l'ordre dans lequel s'en servir.

## Documents de référence (le QUOI)

- **acmm-spec.md** — LA spécification, source de vérité. Décrit l'architecture, le modèle
  de données, les règles, les écrans, le découpage en lots (L1→L7) et le guide de démarrage
  Claude Code (§13). À donner en contexte permanent. Ne pas tout implémenter d'un coup :
  procéder lot par lot.

- **default-tag-rules-enriched.json** — l'ontologie de tags par défaut (extraite de archives.py
  puis enrichie par l'analyse du catalogue réel). Cinq familles de règles. Chargée au premier
  démarrage (lot L2). NB : la famille MO2 a été retirée volontairement.

- **archives.py** — code Python de référence pour la logique d'import/détection (isCar, isTrack,
  isCarSound, descente récursive). À PORTER, pas à exécuter. ⚠️ contrairement à ce code,
  l'app ne réécrit JAMAIS le ui_*.json du mod (modèle overlay non destructif, §3.0).

## Maquettes visuelles (le À QUOI ÇA RESSEMBLE) — Claude Code peut les ouvrir

- **pitbox-mockup.html** — maquette principale navigable : bibliothèque (galerie/tableau),
  fiches, écran de course, page de règles. Référence du thème Rosso Corsa et de la navigation.
  (Note : antérieure à quelques évolutions — la spec fait foi en cas d'écart.)

- **pitbox-fiche-B-revisee.html** — LAYOUT DE RÉFÉRENCE de la fiche voiture pleine page
  (héros large à gauche, données à droite, description dépliable, rangée basse skins/distance+son/
  tags+versions). ⚠️ modèle skins correct : SÉLECTION (prévisualisation + étoile « piloté »),
  PAS d'activation par case à cocher.

- **pitbox-vues-transversales.html** — vues transversales Skins / Sons / Apps (lot L6) :
  comment retrouver les sous-éléments indépendamment des fiches.

## Points de vigilance transverses

1. ui_*.json du mod : LECTURE SEULE, jamais réécrit (overlay).
2. Bibliothèques Voitures et Circuits SÉPARÉES, colonnes propres à chaque type.
3. Skins : pas d'activation filesystem, seulement sélection/prévisualisation + skin piloté.
   Le son, lui, est une vraie bascule exclusive de fichiers avec restauration de l'original.
4. Lancement de session = flux séquentiel (Catégorie→Voiture→Circuit→Réglages→Lancer),
   séparé du monde gestion. Réglages dépendants du type de session.
5. Ne pas implémenter le lancement (L4) tant que le pilotage CM (§8.3) n'est pas vérifié.

## Ordre de travail conseillé (voir §13 de la spec)

Préalable (config+squelette) → L1 (biblio+identité+overlay) → L2 (tags+règles+specs) →
L3 (junctions+profils) → L4 (lancement) → L5 (maintenance) → L6 (skins/sons/apps+contenu base) →
L7 (source/extension navigateur).
