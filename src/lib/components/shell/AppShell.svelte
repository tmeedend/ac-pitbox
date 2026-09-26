<script lang="ts">
  import { onMount } from "svelte";
  import Settings from "$lib/components/settings/Settings.svelte";
  import About from "$lib/components/settings/About.svelte";
  import Library from "$lib/components/library/Library.svelte";
  import Launch from "$lib/components/launch/Launch.svelte";
  import DriverScreen from "$lib/components/driver/DriverScreen.svelte";
  import Inventory from "$lib/components/inventory/Inventory.svelte";
  import Apps from "$lib/components/inventory/Apps.svelte";
  import NavRail from "./NavRail.svelte";
  import TitleBar from "./TitleBar.svelte";
  import SessionColumn from "./session/SessionColumn.svelte";
  import Workshop from "$lib/components/workshop/Workshop.svelte";
  import ImportOverlay from "$lib/components/workshop/ImportOverlay.svelte";
  import PendingDialog from "$lib/components/workshop/PendingDialog.svelte";
  import ShellToasts from "$lib/components/toasts/ShellToasts.svelte";
  import ControllerSetup from "$lib/components/settings/ControllerSetup.svelte";
  import { nav, inSessionZone, rememberLibrary } from "$lib/shell/nav.svelte";
  import { recordScreen } from "$lib/shell/navHistory";
  import { startShellServices } from "$lib/shell/shellServices";
  import { controllers } from "$lib/shell/gamepadDevices.svelte";
  import { bigPictureState } from "$lib/shell/bigpicture.svelte";
  import { musicEnterMenu, musicEnterGrid } from "$lib/shell/music";
  import { libraryVersion } from "$lib/library/libraryVersion.svelte";
  import { loadBrandLogos } from "$lib/library/brandLogos.svelte";
  import { listOtherMods } from "$lib/inventory/others";
  import { getConfig, validateConfig } from "$lib/config";

  // Three watertight territories (SPEC §7.2): the RAIL carries what does not
  // belong to the session (`NavRail.svelte`), the TITLE BAR the shape of the
  // window, and the SESSION COLUMN is the session itself, each of its parts
  // leading to its own screen (`session/SessionColumn.svelte`). The "Add-ons"
  // and "Workshop" button grids that used to live in the column went to the
  // rail: they were not badly drawn, they were in the wrong place.
  //
  // What stays here is what the three territories share: the screen switch,
  // the navigation history, and the path diagnosis that both the rail and
  // the column read. Everything the shell merely keeps running in the
  // background is started by `startShellServices`.
  onMount(() => startShellServices());

  /** Chemins cassés : Assetto Corsa ou son dossier `content` introuvable. La
   * colonne de session en fait un troisième état de ses emplacements
   * (SPEC SESSION§1). */
  let pathsBroken = $state(false);
  /** Content Manager introuvable : le lien de sortie ne s'affiche pas du tout
   * (SPEC SESSION§1). Ni bouton grisé ni message d'erreur au clic — une sortie vers
   * un outil absent n'a pas à occuper une ligne dans une colonne dont la
   * hauteur est comptée. */
  let cmAvailable = $state(false);

  async function refreshPaths() {
    try {
      const v = await validateConfig(await getConfig());
      pathsBroken = !v.ac_install.ok || !v.content_dir.ok;
      cmAvailable = v.content_manager.ok;
    } catch {
      // Le diagnostic lui-même a échoué : ne pas accuser les chemins pour
      // autant, l'état vide normal reste plus juste qu'une fausse impasse.
      pathsBroken = false;
      cmAvailable = false;
    }
  }
  onMount(refreshPaths);
  // Les chemins ne se réparent que depuis les Réglages : relire en sortant de
  // cet écran suffit, plutôt que de valider à chaque rendu. Le nettoyage d'un
  // effet dont la seule dépendance est `nav.section` s'exécute exactement au
  // moment où l'on quitte l'écran.
  $effect(() => {
    if (nav.section !== "settings") return;
    return () => void refreshPaths();
  });

  // --- Pastilles d'alerte du rail (SPEC §7.2) -------------------------------
  //
  // Un signal de rubrique, pas un agrégat : il faut voir LAQUELLE aller
  // regarder. Une seule source réelle aujourd'hui — les conflits de fichiers
  // entre « autres mods », que le backend calcule déjà. Les autres inventaires
  // n'ont pas de notion de « problème » à remonter ; leur en inventer une
  // serait une décision produit, pas une conséquence de cette spec.
  let alerts = $state<Record<string, boolean>>({});
  // The brand logos and the badges on the light plate (TAXO§4, §5), for every
  // screen: re-read when the library changes - a new car may bring a better
  // logo, or a badge with a baked background.
  $effect(() => {
    libraryVersion();
    void loadBrandLogos();
  });
  $effect(() => {
    libraryVersion();
    listOtherMods()
      .then((rows) => (alerts = { others: rows.some((o) => o.conflicts.length > 0) }))
      .catch(() => (alerts = {}));
  });

  // Historique de navigation (§7.2bis) : l'écran affiché est noté à chaque
  // fois qu'il change, quel que soit le chemin emprunté pour y arriver — la
  // douzaine d'endroits qui posent `nav.openFull` ou appellent
  // `requestSection` n'a donc rien à déclarer. Le détail est dans
  // `navHistory.ts`.
  $effect(() => {
    recordScreen({ section: nav.section, openFull: nav.openFull, openPack: nav.openPack });
  });
  // Same reasoning for the library the rail's `Session` entry returns to:
  // observed here, whichever path changed the screen.
  $effect(() => rememberLibrary(nav.section));

  // The session column is a zone, not permanent furniture (SPEC §7.2): shown
  // on the screens that choose what will be launched, its width given back to
  // the screen everywhere else. No slide — a 328 px panel sliding in and out
  // at every rail click wears thin within an evening.
  const sessionZone = $derived(inSessionZone(nav.section));

  // Ambiance musicale suit l'écran affiché tant que Big Picture est actif
  // (§4 de la spec musique) : GRID sur l'écran de paramétrage de la session
  // ("race", l'équivalent Pit Box de la grille de départ), MENU partout
  // ailleurs.
  $effect(() => {
    if (!bigPictureState.active) return;
    if (nav.section === "race") musicEnterGrid();
    else musicEnterMenu();
  });

  const isLibrary = $derived(nav.section === "cars" || nav.section === "tracks");
  // Écrans « pleine page » qui gèrent leur propre défilement interne plutôt
  // que de compter sur le padding + le scroll de `.content` (évite le hack
  // de marge négative — cause probable de l'espace perdu au-dessus du titre
  // « Réglages » signalé par l'utilisateur, cf. Launch.svelte).
  // « Règles » en fait partie depuis que sa barre d'action du bas est un vrai
  // frère de la zone qui défile : en `position: fixed`, elle se plaçait dans
  // le repère de `100vh` (non divisé par le zoom d'interface) et passait donc
  // sous le bord bas de la fenêtre — voir le commentaire de `.r-footer`.
  const noPad = $derived(isLibrary || nav.section === "race" || nav.section === "rules");
</script>

{#if !bigPictureState.active}
  <TitleBar />
{/if}
<div class="frame" class:bigpicture={bigPictureState.active} class:no-session={!sessionZone}>
  <div class="topbar"></div>
  <div class="shell">
    <!-- Zones parcourues par les gâchettes hautes de la manette (§7.4bis) :
         la colonne de session d'un côté, l'écran actif de l'autre. La
         bibliothèque redécoupe sa moitié en deux (liste et fiche) — les zones
         imbriquées les plus internes gagnent, voir `regions()`. -->
    <NavRail {alerts} {cmAvailable} />
    <SessionColumn {pathsBroken} shown={sessionZone} />

    <div class="main-col">
      <main class="content" class:fixed={noPad} data-gp-region="main">
        {#if nav.section === "settings"}
          <Settings />
        {:else if nav.section === "about"}
          <About />
        {:else if nav.section === "cars"}
          <Library kind="Car" />
        {:else if nav.section === "tracks"}
          <Library kind="Track" />
        {:else if nav.section === "rules" || nav.section === "brands" || nav.section === "categories" || nav.section === "countries" || nav.section === "import" || nav.section === "profiles" || nav.section === "maintenance"}
          <Workshop />
        {:else if nav.section === "driver"}
          <DriverScreen />
        {:else if nav.section === "race"}
          <Launch />
        {:else if nav.section === "apps"}
          <!-- Les apps ont leur écran (REFONTE§3.2) : une app a un nom, une
               identité, on l'installe volontairement — elle n'est la dépendance
               de rien, et n'avait rien à faire dans un tiroir avec les polices
               et les fragments de config. -->
          <Apps />
        {:else if nav.section === "others"}
          <Inventory />
        {/if}
      </main>
    </div>
  </div>
</div>

<ImportOverlay />
<PendingDialog />

<ShellToasts />
{#if controllers.setupOpen}
  <ControllerSetup onclose={() => (controllers.setupOpen = false)} />
{/if}

<style>
  .frame {
    background: var(--panel);
    border: 1px solid var(--rosso);
    /* `zoom` (voir zoom.svelte.ts) agrandit tout le rendu, mais vh/vw restent
       relatifs à la fenêtre réelle — sans cette division, .frame devient plus
       haut que la fenêtre à >100% (rien à scroller pour atteindre le bas :
       bouton Enregistrer hors champ, coquille tronquée un peu partout). */
    height: calc(100vh / var(--ui-zoom, 1));
    /* Barre de titre custom en position fixe (voir TitleBar.svelte) : réserve
       sa hauteur ici plutôt que de la compter comme un enfant flex, pour
       qu'elle reste toujours à l'écran quel que soit ce qui défile en dessous. */
    padding-top: 32px;
    display: flex;
    flex-direction: column;
  }
  /* Big Picture : pas de barre de titre custom (masquée, gagne en hauteur)
     ni de bordure — rendu bord à bord, immersif. */
  .frame.bigpicture {
    padding-top: 0;
    border: none;
  }
  .topbar {
    background: var(--rosso);
    height: 3px;
    flex: none;
  }
  .frame.bigpicture .topbar {
    display: none;
  }
  .shell {
    flex: 1;
    min-height: 0;
    display: grid;
    /* Rail (hors session) · colonne de session (la session) · contenu.
       222px tant que la bibliothèque gardait son panneau de détail à droite ;
       celui-ci retiré, la zone principale n'a plus besoin d'autant de largeur
       et la colonne de session peut respirer — c'est elle qui porte le duo
       voiture/circuit et ses menus. Sa contrainte reste la **hauteur** : tout
       ce qu'on y ajoute doit tenir sans allonger la colonne, d'où la largeur
       prise ici (réglée à l'œil avec l'utilisateur). */
    grid-template-columns: 74px 328px 1fr;
  }
  /* Outside the session zone the column is `display: none`, so it takes no
     grid cell and the content moves into the second track. */
  .frame.no-session .shell {
    grid-template-columns: 74px 1fr;
  }

  .main-col {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .content {
    flex: 1;
    min-height: 0;
    padding: 28px 32px;
    overflow: auto;
  }
  /* Bibliothèques : hauteur fixe + défilement interne (évite la double scrollbar). */
  .content.fixed {
    padding: 0;
    overflow: hidden;
  }
</style>
