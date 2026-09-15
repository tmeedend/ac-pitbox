<script lang="ts">
  // Barre de titre custom (décorations OS désactivées, tauri.conf.json) : plus
  // immersif que la barre Windows par défaut, intégrée au thème rosso corsa.
  // data-tauri-drag-region sur la zone vide = déplace la fenêtre ; les
  // boutons eux-mêmes n'ont pas cet attribut donc restent cliquables.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { t } from "$lib/i18n/index.svelte";
  import { enterBigPicture } from "$lib/shell/bigpicture.svelte";

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

  /**
   * Les huit bords de la fenêtre, en poignées invisibles.
   *
   * **Sans elles, la fenêtre n'est pas redimensionnable du tout** —
   * `decorations: false` (tauri.conf.json) retire la zone non-cliente de
   * Windows, et avec elle les bordures de redimensionnement ET le curseur en
   * double flèche qui les annonce. Le défaut est là depuis que la barre de
   * titre est faite maison ; il ne se voyait pas fenêtre maximisée, qui est
   * l'état dans lequel on développe.
   *
   * Rien à dessiner : chaque poignée est une bande transparente de six pixels
   * posée sur le bord, dont le seul rôle est de porter un curseur et de passer
   * la main à `startResizeDragging`. C'est Windows qui redimensionne ensuite,
   * donc l'aperçu d'accrochage et le double-clic d'étirement vertical
   * fonctionnent comme sur n'importe quelle fenêtre.
   *
   * Les coins passent devant les côtés (`z-index`) : sur six pixels, viser un
   * coin est déjà difficile, et un côté qui gagnerait la superposition rendrait
   * la diagonale inatteignable.
   */
  const EDGES = [
    ["n", "North"],
    ["s", "South"],
    ["e", "East"],
    ["w", "West"],
    ["nw", "NorthWest"],
    ["ne", "NorthEast"],
    ["sw", "SouthWest"],
    ["se", "SouthEast"],
  ] as const;

  function grab(event: PointerEvent, direction: (typeof EDGES)[number][1]) {
    // Bouton gauche seulement : un clic droit sur un bord n'est pas un geste
    // de redimensionnement, et l'attraper couperait le menu contextuel.
    if (event.button !== 0) return;
    event.preventDefault();
    void win.startResizeDragging(direction);
  }
</script>

<!-- Masquées quand la fenêtre est maximisée : il n'y a plus de bord à tirer, et
     une bande active sur le bord de l'écran gênerait le pointage des boutons. -->
{#if !maximized}
  {#each EDGES as [side, direction] (side)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="edge {side}" onpointerdown={(e) => grab(e, direction)}></div>
  {/each}
{/if}

<div class="titlebar">
  <!-- Zone de déplacement de fenêtre (chrome OS, pas du contenu de document) :
       double-clic = agrandir/restaurer, comme une vraie barre de titre. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- **La marque vit ici, et nulle part ailleurs.** Elle occupait un bandeau
       en tête de la colonne de session — logo, nom, sous-titre empilés — soit
       une cinquantaine de pixels pris sur la seule ressource rare de cette
       colonne, sa hauteur, pour une information qui ne change jamais. La barre
       de titre est déjà l'endroit où une application se nomme, et elle portait
       le nom sans le logo ni le sous-titre : les trois s'y rangent sur une
       ligne, à coût de hauteur nul. -->
  <div class="drag" data-tauri-drag-region ondblclick={toggleMaximize}>
    <div class="logo" aria-hidden="true"><span>PB</span></div>
    <span class="name">PIT BOX</span>
    <span class="brand-sub">AC MOD MANAGER</span>
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
  .edge {
    position: fixed;
    z-index: 1000;
  }
  .n,
  .s {
    left: 0;
    right: 0;
    height: 6px;
    cursor: ns-resize;
  }
  .e,
  .w {
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: ew-resize;
  }
  .n {
    top: 0;
  }
  .s {
    bottom: 0;
  }
  .w {
    left: 0;
  }
  .e {
    right: 0;
  }
  /* Les coins par-dessus les côtés : sur six pixels, un côté qui gagnerait la
     superposition rendrait la diagonale inatteignable. */
  .nw,
  .ne,
  .sw,
  .se {
    width: 12px;
    height: 12px;
    z-index: 1001;
  }
  .nw,
  .ne {
    top: 0;
  }
  .sw,
  .se {
    bottom: 0;
  }
  .nw,
  .sw {
    left: 0;
  }
  .ne,
  .se {
    right: 0;
  }
  .nw,
  .se {
    cursor: nwse-resize;
  }
  .ne,
  .sw {
    cursor: nesw-resize;
  }
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
    gap: 9px;
    padding-left: 10px;
  }
  /* Même tuile que celle de la colonne de session, réduite pour tenir dans les
     32 px de la barre : le biseau et l'italique sont la marque, ils ne se
     redessinent pas ici. */
  .logo {
    width: 18px;
    height: 18px;
    flex: none;
    background: var(--rosso);
    display: flex;
    align-items: center;
    justify-content: center;
    transform: skewX(-8deg);
  }
  .logo span {
    transform: skewX(8deg);
    color: #fff;
    font-size: 7.5px;
    font-weight: 700;
    font-style: italic;
  }
  .name {
    font-size: 11px;
    font-weight: 600;
    font-style: italic;
    letter-spacing: 1.5px;
    color: var(--txt2);
  }
  /* `brand-sub` et non `sub` : ce dernier a déjà voulu dire trois choses dans
     l'app (en-tête de dialogue, message sous un champ, surtitre rouge), et un
     nom qui veut dire trois choses est un piège au premier déplacement de
     markup. */
  .brand-sub {
    font-size: 6.5px;
    letter-spacing: 2.5px;
    color: var(--muted);
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
