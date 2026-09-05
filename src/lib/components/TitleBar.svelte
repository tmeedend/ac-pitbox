<script lang="ts">
  // Barre de titre custom (décorations OS désactivées, tauri.conf.json) : plus
  // immersif que la barre Windows par défaut, intégrée au thème rosso corsa.
  // data-tauri-drag-region sur la zone vide = déplace la fenêtre ; les
  // boutons eux-mêmes n'ont pas cet attribut donc restent cliquables.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { t } from "$lib/i18n/index.svelte";
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
    <!-- Big Picture porte un LIBELLÉ, pas seulement une icône : l'action
         transforme toute l'application, elle est assez engageante pour
         mériter son mot et assez rare pour ne pas encombrer. Elle reste ici
         parce que c'est un mode d'affichage — même famille que la fenêtre.
         « À propos », lui, est parti au pied du rail : c'est du contenu
         (version, liens, dépôt), pas un état de fenêtre (SPEC §7.2). -->
    <button class="wbtn bp" type="button" title={t("titlebar.bigPicture")} onclick={enterBigPicture}>
      <svg viewBox="0 0 10 10">
        <path d="M1.5 3.5 V1.5 H3.5" fill="none" />
        <path d="M6.5 1.5 H8.5 V3.5" fill="none" />
        <path d="M8.5 6.5 V8.5 H6.5" fill="none" />
        <path d="M3.5 8.5 H1.5 V6.5" fill="none" />
      </svg>
      <span>{t("titlebar.bigPictureLabel")}</span>
    </button>
    <!-- Sépare ce qui appartient à Pit Box de ce qui appartient au
         gestionnaire de fenêtres. -->
    <div class="tb-sep"></div>
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
    align-items: stretch;
  }
  .tb-sep {
    width: 1px;
    background: var(--line);
    margin: 9px 4px;
  }
  .wbtn {
    width: 44px;
    background: transparent;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .wbtn.bp {
    width: auto;
    gap: 7px;
    padding: 0 12px;
    color: var(--muted2);
    font-size: 10px;
    letter-spacing: 0.12em;
  }
  .wbtn.bp:hover {
    color: var(--txt2);
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
