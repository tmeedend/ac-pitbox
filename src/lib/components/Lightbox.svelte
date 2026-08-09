<script lang="ts">
  // Visionneuse plein écran pour les galeries d'images (screenshots/
  // backgrounds, §6.1). Navigation précédent/suivant à la souris, au clavier
  // (flèches) et à la manette (croix/stick gauche-droite), fermeture par
  // Echap, le bouton ✕, un clic sur le fond, ou le bouton B/Rond manette.
  // Diaporama optionnel (bouton lecture) qui avance automatiquement.
  //
  // Pose `nav.lightboxOpen` tant qu'elle est montée : la navigation manette
  // globale (`gamepadNav.ts`) et le précédent/suivant de mod de la fiche
  // pleine page (`Library.svelte::navigateFull`) l'observent tous les deux
  // pour céder gauche/droite/B — sinon une même pression ferait à la fois
  // défiler les images et changer de mod, ou fermerait la fiche entière.
  import { nav } from "$lib/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";

  export interface LightboxItem {
    src: string;
    caption?: string;
  }

  let {
    items,
    startIndex = 0,
    onclose,
  }: {
    items: LightboxItem[];
    startIndex?: number;
    onclose: () => void;
  } = $props();

  let index = $state(startIndex);
  let playing = $state(false);
  const multi = $derived(items.length > 1);

  function step(delta: 1 | -1) {
    if (!multi) return;
    index = (index + delta + items.length) % items.length;
  }

  function togglePlay() {
    if (!multi) return;
    playing = !playing;
  }

  const PLAY_INTERVAL_MS = 4000;
  $effect(() => {
    if (!playing) return;
    const id = window.setInterval(() => step(1), PLAY_INTERVAL_MS);
    return () => window.clearInterval(id);
  });

  $effect(() => {
    nav.lightboxOpen = true;
    return () => {
      nav.lightboxOpen = false;
    };
  });

  $effect(() => {
    function onKeydown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onclose();
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        step(-1);
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        step(1);
      }
    }
    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });

  // Manette : mêmes conventions que `gamepadNav.ts`/`Library.svelte`
  // (bouton[14]/[15] ou stick gauche = gauche/droite, bouton[1] = B/annuler).
  // Poll local dédié plutôt que le scrutin global : la visionneuse a besoin
  // d'agir sur front montant sans dépendre du focus DOM courant.
  $effect(() => {
    let raf = 0;
    let last = { left: false, right: false, back: false };
    function poll() {
      for (const gp of navigator.getGamepads?.() ?? []) {
        if (!gp) continue;
        const axis = gp.axes[0] ?? 0;
        const left = (gp.buttons[14]?.pressed ?? false) || axis < -0.6;
        const right = (gp.buttons[15]?.pressed ?? false) || axis > 0.6;
        const back = gp.buttons[1]?.pressed ?? false;
        if (left && !last.left) step(-1);
        if (right && !last.right) step(1);
        if (back && !last.back) onclose();
        last = { left, right, back };
      }
      raf = requestAnimationFrame(poll);
    }
    raf = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(raf);
  });
</script>

<div class="lightbox" role="dialog" aria-modal="true" aria-label={t("lightbox.open")}>
  <!-- Bouton plein cadre plutôt qu'un `onclick` sur le conteneur : un vrai
       élément interactif pour le clic sur le fond, sans avoir à intercepter
       la propagation sur chacun des éléments posés par-dessus. Seulement
       utile s'il n'y a qu'une image : sinon les zones précédent/suivant
       occupent déjà tout l'espace hors du centre. -->
  <button class="lb-backdrop" type="button" onclick={onclose} aria-label={t("lightbox.close")}></button>

  <!-- En haut à droite, sous la barre de titre custom (32px, z-index 1000,
       toujours au-dessus) — en haut à gauche elle passait dessous. -->
  <button class="lb-close" type="button" onclick={onclose} title={t("lightbox.close")}>✕</button>

  <div class="lb-row">
    {#if multi}
      <!-- Grande zone cliquable (pas un petit bouton à viser) : un clic
           approximatif sur la moitié gauche/droite de l'écran suffit,
           réduit le risque de tomber sur le fond et fermer par erreur. -->
      <button class="lb-zone lb-prev" type="button" onclick={() => step(-1)} title={t("lightbox.prev")}>
        <span class="lb-chevron">‹</span>
      </button>
    {:else}
      <div class="lb-zone-spacer"></div>
    {/if}

    <div class="lb-center">
      <div class="lb-stage">
        <img src={items[index].src} alt={items[index].caption ?? ""} />
      </div>

      <div class="lb-bar">
        {#if multi}
          <button
            class="lb-play"
            type="button"
            onclick={togglePlay}
            title={playing ? t("lightbox.pause") : t("lightbox.play")}
          >
            {playing ? "⏸" : "▶"}
          </button>
          <span class="lb-count mono">{index + 1} / {items.length}</span>
        {/if}
        {#if items[index].caption}<span class="lb-caption">{items[index].caption}</span>{/if}
      </div>
    </div>

    {#if multi}
      <button class="lb-zone lb-next" type="button" onclick={() => step(1)} title={t("lightbox.next")}>
        <span class="lb-chevron">›</span>
      </button>
    {:else}
      <div class="lb-zone-spacer"></div>
    {/if}
  </div>
</div>

<style>
  .lightbox {
    position: fixed;
    inset: 0;
    z-index: 300;
  }
  .lb-backdrop {
    position: absolute;
    inset: 0;
    z-index: 0;
    background: rgba(3, 3, 5, 0.94);
    border: none;
    padding: 0;
    cursor: zoom-out;
  }
  /* Barre de titre custom de l'app : 32px, z-index 1000, toujours au-dessus
     (voir TitleBar.svelte) — le bouton fermer doit rester sous elle sans
     être recouvert. */
  .lb-close {
    position: absolute;
    /* Au-dessus de `.lb-row` (z-index 1) : la zone "suivant" couvre tout le
       côté droit, y compris là où ce bouton est posé, et vient après lui
       dans le DOM — sans ce z-index plus élevé, elle capterait le clic. */
    z-index: 2;
    top: 40px;
    right: 14px;
    background: transparent;
    color: var(--txt2);
    font-size: 20px;
    line-height: 1;
    padding: 6px 10px;
  }
  .lb-close:hover {
    color: var(--rosso-bright);
  }
  .lb-row {
    position: relative;
    z-index: 1;
    height: 100%;
    display: flex;
    align-items: center;
  }
  /* Zone précédent/suivant : (quasiment) toute la moitié de l'écran de chaque
     côté de l'image, pas un petit bouton à viser précisément — un clic
     approximatif suffit, au lieu de retomber sur le fond et fermer par
     erreur (retour utilisateur direct). */
  .lb-zone {
    flex: 1 1 0;
    align-self: stretch;
    background: transparent;
    color: var(--txt2);
    display: flex;
    align-items: center;
    padding: 0;
  }
  .lb-zone:hover {
    color: var(--rosso-bright);
  }
  .lb-zone:hover .lb-chevron {
    background: rgba(8, 8, 12, 0.5);
  }
  .lb-zone-spacer {
    flex: 1 1 0;
  }
  .lb-prev {
    justify-content: flex-start;
  }
  .lb-next {
    justify-content: flex-end;
  }
  .lb-chevron {
    font-size: 34px;
    line-height: 1;
    padding: 10px 16px;
  }
  .lb-center {
    flex: none;
    max-width: min(80vw, 1400px);
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .lb-stage {
    max-height: 78vh;
    pointer-events: none;
  }
  .lb-stage img {
    max-width: 100%;
    max-height: 78vh;
    object-fit: contain;
    display: block;
  }
  .lb-bar {
    margin-top: 14px;
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .lb-play {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 13px;
    width: 30px;
    height: 30px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .lb-play:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .lb-count {
    color: var(--muted);
    font-size: 11px;
  }
  .lb-caption {
    color: var(--txt2);
    font-size: 11.5px;
  }
</style>
