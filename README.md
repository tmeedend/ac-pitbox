# Pit Box — gestionnaire de mods Assetto Corsa

Application desktop (Tauri + SvelteKit) qui remplace Mod Organizer 2 et pilote
Content Manager. Voir la spec fonctionnelle (`acmm-spec.md`) pour le détail.

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

- **Committers, reviewers et approvers** : [Théo (tmeedend)](https://github.com/tmeedend), seul mainteneur du projet.
- **Privacy policy** : This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.

## Stack

- **Frontend** : SvelteKit (SPA, `adapter-static`) + Svelte 5 (runes) + TypeScript + Vite 6
- **Backend** : Rust (Tauri v2)
- Design system repris de `pitbox-mockup.html` (thème rosso/noir) → `src/lib/styles/global.css`

## Prérequis

- **Node 20+** ✅
- **WebView2** ✅ (présent sur Windows 11)
- **Rust + toolchain MSVC** (linker C++ + Windows SDK) — requis pour compiler le backend.
  Voir https://tauri.app/start/prerequisites/

## Lancer en dev

```bash
npm install
npm run tauri dev      # compile le backend Rust + sert le frontend
```

Pendant le dev frontend seul (sans backend) : `npm run dev`.
Vérifier les types : `npm run check`.

## Structure

```
src/                         # Frontend SvelteKit
  routes/+page.svelte        # Orchestration : wizard 1ère config OU shell app
  lib/config.ts              # Pont typé vers les commandes Rust de config
  lib/styles/global.css      # Design tokens (mockup)
  lib/components/
    SetupWizard.svelte       # Assistant de première configuration (§12)
    Settings.svelte          # Écran Réglages (édition des chemins)
    ConfigFields.svelte      # Champs de chemins partagés wizard/réglages
    PathField.svelte         # Champ + bouton « Parcourir… » (dialog natif)
    AppShell.svelte          # Sidebar + zone de contenu (placeholders par lot)
  lib/components/
    Library.svelte           # Vue bibliothèque : galerie/tableau, import, recherche
    ModDetail.svelte         # Panneau détail : preview, versions, historique, tags
  lib/library.ts             # Pont typé vers les commandes L1
src-tauri/src/
  config.rs                  # Modèle AppConfig + load/save + validation (§12)
  detect.rs                  # Détection auto (Steam/AC, Content Manager, 7-Zip)
  uijson.rs                  # Lecture SEULE des ui_car/ui_track.json (§3.0)
  modscan.rs                 # Détection type + descente récursive (porté archives.py)
  archive.rs                 # Extraction 7-Zip + déplacement de dossiers
  identity.rs                # Signature de contenu + empreinte composite (§4.1)
  inspect.rs                 # Features CSP, skins, layouts, preview
  overlay.rs                 # Base d'overlay SQLite (mods/versions/historique)
  importer.rs                # Pipeline d'import + résolution d'identité (§4.2/4.3)
  library.rs                 # Assemblage cartes (preview + état actif) pour l'UI
  lib.rs                     # Commandes Tauri + wiring des plugins + ouverture DB
```

## Trois bases distinctes (§12 de la spec) — à ne pas confondre

1. **Bibliothèque** — fichiers des mods (source de vérité, disque dédié ~300 Go).
2. **Base d'overlay** — métadonnées produites par l'app (tags, specs, favori, historique). *(L1+)*
3. **Fichier de règles** — `default-tag-rules.json`, l'ontologie. *(L2)*

Plus le **fichier de config** (chemins + préférences) : `app_config_dir()/config.json`.

## Roadmap (lots)

- [x] **Préalable** — squelette Tauri + assistant 1ère config + écran Réglages (§12)
- [x] **L1** — Bibliothèque & identité : import zip/rar/7z + descente récursive, détection type, empreinte composite + résolution version (§4.2/4.3), overlay SQLite non destructif, historique, galerie + tableau (tri colonnes), panneau détail + fiche technique native (specs/courbe moteur/description, §5bis.1 partie native), import drag-drop + progression, dialogue conflit flou 2 boutons (garder les deux / écraser). Tests d'intégration : import + conflit/résolution.
- [x] **L2** — Tags & organisation : moteur des 6 familles + extraction (specs/pays), harmonisation non destructive à l'import + réapplication, 3 origines de tags color-codées (catégorie/règle/manuel/fichier), catégorie = tag `#`, fiche technique dérivée (drivetrain/aspiration/moteur/boîte), favori, **écran de règles : toutes les familles éditables** (fusion, suppression, marque, nom→tag, classe, MO2, extraction specs, extraction pays) + aperçu d'impact live. Tags fichier masquables. **Filtres avancés** bibliothèque (catégorie, classe, année min/max, favoris) + recherche étendue à toutes les origines de tags. Tests : harmonisation dans `full_import_pipeline`.
- [x] **L3** — Activation & profils : junctions Windows (`mklink /J`, sans admin), activer/désactiver/changer de version depuis le panneau détail, états actif/inactif (= junction présente), profils (capturer l'état actif + appliquer par réconciliation), garde-fou strict (jamais supprimer un vrai dossier non-junction). Test : `activation::tests::junction_create_remove_and_guard`.
- [x] **L4** — Lancement : §8.3 résolu+validé (`acmanager://race/config?configFile=`, voir `docs/L4-cm-launch-research.md`). Générateur `race.ini` typé (practice/hotlap/course + grille IA + qualif multi-sessions), auto-activation du contenu, écran « Lancer ». Météo adaptative (§8.5) : détection stack CSP/SOL/vanilla, intentions → meilleur dossier, grisé si indisponible, température implicite. Presets de session par type persistants (§8.4). Bouton « Conduire » contextuel (§8.6). Écran à 2 niveaux : bloc « Options de course » repliable (pénalités, faux départ, évolution grip, qualification) + sélection de skins avec miniatures (§8.6). Tests : `launch::tests::*` (4).
- [ ] **L5** — Maintenance & export : export autonome (acd.bms isolé), nettoyage

## Règle d'or

Le fichier `ui_*.json` d'un mod est **lecture seule** — jamais réécrit. Toutes les
métadonnées produites par l'app vivent dans la base d'overlay (surcouche non destructive, §3.0).
