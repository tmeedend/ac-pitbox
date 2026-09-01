<script lang="ts">
  // Plateau d'essayage de l'écran Pilote (docs/SPEC-ecran-pilote.md §5.1, §5.4).
  //
  // **Ce n'est pas encore le rendu 3D** que la spec décrit : le pilote seul,
  // sur un volant générique, recadré par piste. Le lot qui le construit vient
  // après celui-ci, et il a besoin d'un chemin de conversion qui n'existe pas
  // (un `.glb` de mannequin sans voiture autour). En attendant, le plateau
  // montre l'échantillon plat de la pièce en cours d'essai, en grand — la
  // sortie que §12.4 prévoit quand le moteur 3D ne démarre pas, employée ici
  // comme état intermédiaire assumé plutôt que comme panne.
  //
  // L'interface de ce composant est déjà celle du plateau final : la pièce
  // active, ce qu'on essaie, ce qu'on garde. Remplacer son intérieur par du
  // three.js ne touchera pas à l'écran qui l'utilise.
  import { t } from "$lib/i18n/index.svelte";

  let {
    /** Nom lisible de ce qui est appliqué en ce moment — l'essai s'il y en a
     * un, le choix retenu sinon. Nommer l'essai en cours est obligatoire
     * (§5.4) : sans ça rien ne distingue ce qu'on survole de ce qu'on garde. */
    applied,
    /** Vignette de ce qui est appliqué, quand AC en fournit une. */
    sample = null,
    /** `true` pendant un survol : la ligne d'état passe de la consigne au nom. */
    trying = false,
    /** Corps substitué : bandeau permanent tant que dure le mode (§10.1). */
    substituted = false,
  }: {
    applied: string;
    sample?: string | null;
    trying?: boolean;
    substituted?: boolean;
  } = $props();
</script>

<div class="stage">
  {#if substituted}
    <div class="subst">{t("driver.stage.substituted")}</div>
  {/if}

  <div class="art">
    {#if sample}
      <img src={sample} alt="" />
    {:else}
      <div class="nosample">{t("driver.stage.noSample")}</div>
    {/if}
  </div>

  <p class="pending">{t("driver.stage.pending")}</p>

  <div class="foot">
    <span class="live">{t("driver.stage.live")}</span>
    <span class="hint">{trying ? t("driver.stage.trial", { name: applied }) : t("driver.stage.hint")}</span>
  </div>
</div>

<style>
  /* Dégradé radial du gris panneau vers le noir de fond (§5.1) : le plateau
     doit rester lisible sous une livrée sombre, donc le contraste prime. */
  .stage {
    position: relative;
    flex: 1;
    min-height: 380px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 22px 18px 42px;
    background: radial-gradient(72% 68% at 50% 40%, var(--raised) 0%, var(--bg) 78%);
    overflow: hidden;
  }

  .subst {
    position: absolute;
    left: 12px;
    top: 12px;
    font-size: 10px;
    letter-spacing: 0.14em;
    color: var(--orange);
    border: 1px solid var(--line);
    background: var(--panel);
    border-radius: 2px;
    padding: 3px 7px;
  }

  .art {
    width: 100%;
    max-width: 232px;
    aspect-ratio: 1;
    border: 1px solid var(--line);
    border-radius: 2px;
    overflow: hidden;
    background: var(--card);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .nosample {
    font-size: 11px;
    color: var(--faint);
    text-align: center;
    padding: 0 16px;
    line-height: 1.5;
  }

  .pending {
    margin: 0;
    max-width: 260px;
    text-align: center;
    font-size: 11px;
    line-height: 1.5;
    color: var(--faint);
  }

  .foot {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 28px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    font-size: 10.5px;
    color: var(--muted);
    border-top: 1px solid var(--line);
    background: color-mix(in srgb, var(--bg) 75%, transparent);
  }
  /* Un des trois seuls emplois du rouge saturé sur cet écran (§15). */
  .live {
    color: var(--rosso-bright);
    letter-spacing: 0.1em;
    flex: 0 0 auto;
  }
  .hint {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
