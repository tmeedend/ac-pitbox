<script lang="ts">
  // Barre de titre custom (décorations OS désactivées, tauri.conf.json) : plus
  // immersif que la barre Windows par défaut, intégrée au thème rosso corsa.
  // data-tauri-drag-region sur la zone vide = déplace la fenêtre ; les
  // boutons eux-mêmes n'ont pas cet attribut donc restent cliquables.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { t } from "$lib/i18n/index.svelte";
  import { requestSection } from "$lib/nav.svelte";
  import { enterBigPicture } from "$lib/bigpicture.svelte";

  const win = getCurrentWindow();
  let maximized = $state(false);

  onMount(() => {
    win.isMaximized().then((m) => (maximized = m));
    const unlisten = win.onResized(async () => {
      maximized = await win.isMaximized();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  function minimize() {
    win.minimize();
  }
  function toggleMaximize() {
    win.toggleMaximize();
  }
  function closeWin() {
    win.close();
  }
</script>

<div class="titlebar">
  <!-- Zone de déplacement de fenêtre (chrome OS, pas du contenu de document) :
       double-clic = agrandir/restaurer, comme une vraie barre de titre. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="drag" data-tauri-drag-region ondblclick={toggleMaximize}>
    <span class="name">PIT BOX</span>
  </div>
  <div class="win-controls">
    <button class="wbtn" type="button" title={t("titlebar.bigPicture")} onclick={enterBigPicture}>
      <svg viewBox="0 0 10 10">
        <path d="M1.5 3.5 V1.5 H3.5" fill="none" />
        <path d="M6.5 1.5 H8.5 V3.5" fill="none" />
        <path d="M8.5 6.5 V8.5 H6.5" fill="none" />
        <path d="M3.5 8.5 H1.5 V6.5" fill="none" />
      </svg>
    </button>
    <!-- « À propos » : écran rarement ouvert, il encombrait la navigation. -->
    <button class="wbtn help" type="button" title={t("nav.about")} onclick={() => requestSection("about")}>?</button>
    <button class="wbtn" type="button" title={t("titlebar.minimize")} onclick={minimize}>
      <svg viewBox="0 0 10 10"><line x1="1.5" y1="8.5" x2="8.5" y2="8.5" /></svg>
    </button>
    <button class="wbtn" type="button" title={maximized ? t("titlebar.restore") : t("titlebar.maximize")} onclick={toggleMaximize}>
      {#if maximized}
        <svg viewBox="0 0 10 10">
          <rect x="1.5" y="3" width="5.5" height="5.5" />
          <path d="M3.5 3 V1.5 H9 V7 H7.5" fill="none" />
        </svg>
      {:else}
        <svg viewBox="0 0 10 10"><rect x="1.5" y="1.5" width="7" height="7" /></svg>
      {/if}
    </button>
    <button class="wbtn close" type="button" title={t("titlebar.close")} onclick={closeWin}>
      <svg viewBox="0 0 10 10"><line x1="1.5" y1="1.5" x2="8.5" y2="8.5" /><line x1="8.5" y1="1.5" x2="1.5" y2="8.5" /></svg>
    </button>
  </div>
</div>

<style>
  /* Position fixe (pas un enfant flex de .frame) : garantit qu'elle reste
     toujours visible en haut de la fenêtre, quel que soit ce qui défile
     dans le contenu en dessous (bug signalé : les boutons disparaissaient
     au scroll quand la barre faisait juste partie du flux normal). */
  .titlebar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 1000;
    display: flex;
    align-items: stretch;
    height: 32px;
    background: var(--panel2);
    border-bottom: 1px solid var(--line);
    user-select: none;
  }
  .drag {
    flex: 1;
    display: flex;
    align-items: center;
    padding-left: 12px;
  }
  .name {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 2px;
    color: var(--muted2);
  }
  .win-controls {
    display: flex;
  }
  .wbtn {
    width: 44px;
    background: transparent;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .wbtn.help {
    font-family: var(--mono);
    font-size: 14px;
    color: var(--muted);
  }
  .wbtn.help:hover {
    color: var(--rosso-bright);
  }
  .wbtn:hover {
    background: var(--raised);
  }
  .wbtn.close:hover {
    background: var(--rosso);
  }
  .wbtn svg {
    width: 10px;
    height: 10px;
  }
  .wbtn svg line,
  .wbtn svg rect,
  .wbtn svg path {
    stroke: var(--muted2);
    stroke-width: 1;
    fill: none;
  }
  .wbtn:hover svg line,
  .wbtn:hover svg rect,
  .wbtn:hover svg path {
    stroke: var(--txt);
  }
</style>
