<script lang="ts">
  // Bloc « Options de session » de l'écran Lancement (§8.4/§8.6) : réglages
  // dont le contenu dépend du type de session choisi (Practice/Hotlap/Course),
  // plus deux réglages communs aux trois (évolution du grip, pénalités). Vue
  // de présentation pure : tout est une lecture/écriture directe de `setup`
  // (état partagé avec le parent, §8.6bis) — aucune logique à faire remonter.
  import type { RaceSetup } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";
  import NumberStepper from "../NumberStepper.svelte";

  let { setup }: { setup: RaceSetup } = $props();

  // Les essais libres n'existent que dans le mode Weekend de CM — celui-là
  // même qui porte la qualification. Sans qualif, le preset bascule sur le
  // mode course sèche, où aucune phase préparatoire n'existe (§9.3) : les
  // laisser cochables afficherait un réglage sans effet en jeu.
  function toggleQualifying(on: boolean) {
    setup.qualify_enabled = on;
    if (!on) setup.practice_enabled = false;
  }
</script>

<!-- Options de session (§8.4/§8.6) : première carte de la colonne, la
     plus consultée — contenu dépendant du type choisi ci-dessus. -->
<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.sessionOptionsLabel")}</span></header>
  <div class="blk-b">
    <!-- Tout sur une ligne : évolution du grip et pénalités sont
         envoyées par le backend quel que soit le type de session
         (Penalties dans les 3 ModeData, TrackPropertiesData au niveau
         racine du preset, pas dans ModeData) — rien ne justifie de les
         cantonner à Course. Faux départ / tours / essais / qualif
         restent Course uniquement : absents des schémas Practice/Hotlap
         (pas de grille, pas de phase weekend). -->
    <div class="opts-row">
      {#if setup.session_type === "hotlap"}
        <label class="check"><input type="checkbox" bind:checked={setup.ghost_car} /><span>{t("launch.ghostCar")}</span></label>
      {:else}
        <label class="grid-fields">
          <NumberStepper min={1} max={99} bind:value={setup.laps} />
          <span class="fk lbl-key">{t("launch.laps")}</span>
        </label>
        <div><span class="fk lbl-key">{t("launch.jumpStart")}</span>
          <div class="seg-v">
            <button type="button" class:on={setup.jump_start_penalty === 0} onclick={() => (setup.jump_start_penalty = 0)}>{t("launch.jumpStartNone")}</button>
            <button type="button" class:on={setup.jump_start_penalty === 1} onclick={() => (setup.jump_start_penalty = 1)}>{t("launch.jumpStartTeleport")}</button>
            <button type="button" class:on={setup.jump_start_penalty === 2} onclick={() => (setup.jump_start_penalty = 2)}>{t("launch.jumpStartDrivethrough")}</button>
          </div>
        </div>
      {/if}

      <div><span class="fk lbl-key">{t("launch.gripEvolution")}</span>
        <div class="seg-v">
          <button type="button" class:on={setup.grip === 86} onclick={() => (setup.grip = 86)}>{t("launch.gripGreen")}</button>
          <button type="button" class:on={setup.grip === 92} onclick={() => (setup.grip = 92)}>{t("launch.gripMedium")}</button>
          <button type="button" class:on={setup.grip === 96} onclick={() => (setup.grip = 96)}>{t("launch.gripRubbered")}</button>
          <button type="button" class:on={setup.grip === 100} onclick={() => (setup.grip = 100)}>{t("launch.gripOptimal")}</button>
        </div>
      </div>

      {#if setup.session_type === "race"}
        <label class="check">
          <input
            type="checkbox"
            checked={setup.qualify_enabled}
            onchange={(e) => toggleQualifying(e.currentTarget.checked)}
          /><span>{t("launch.qualifying")}</span>
        </label>
        {#if setup.qualify_enabled}
          <label class="grid-fields">
            <NumberStepper min={5} max={90} bind:value={setup.qualify_minutes} />
            <span class="fk lbl-key">{t("launch.qualifyMinutes")}</span>
          </label>
          <label class="check"><input type="checkbox" bind:checked={setup.practice_enabled} /><span>{t("launch.freePractice")}</span></label>
          {#if setup.practice_enabled}
            <label class="grid-fields">
              <NumberStepper min={1} max={120} bind:value={setup.practice_minutes} />
              <span class="fk lbl-key">{t("launch.practiceMinutes")}</span>
            </label>
          {/if}
        {/if}
      {/if}
      <label class="check"><input type="checkbox" bind:checked={setup.penalties} /><span>{t("launch.penalties")}</span></label>

      {#if setup.session_type === "practice"}
        <div><span class="fk lbl-key">{t("launch.startFrom")}</span>
          <div class="seg-v">
            <button type="button" class:on={setup.start_from_pit} onclick={() => (setup.start_from_pit = true)}>{t("launch.startFromPit")}</button>
            <button type="button" class:on={!setup.start_from_pit} onclick={() => (setup.start_from_pit = false)}>{t("launch.startFromTrack")}</button>
          </div>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  /* Options de session : tout sur une ligne (retombe à la ligne seulement si
     la largeur manque vraiment) — un groupe par réglage, chacun garde sa
     largeur naturelle plutôt que de s'étirer dans une grille. */
  .opts-row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 14px 16px;
  }
  .opts-row > div {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .grid-fields {
    display: inline-flex;
    align-items: center;
    gap: 12px;
  }
  /* Couleur/taille/interlettrage viennent de `.lbl-key` (global, harmonisation
     §chantier libellés) : ne reste ici que ce que `.lbl-key` ne couvre pas. */
  .fk {
    text-transform: uppercase;
  }
  /* Groupe de boutons rectangulaire (remplace les <select> natifs, dont la
     popup n'est pas pilotable à la manette) : chaque option est un bouton
     focusable, sélectionnable au clic comme au clic manette (bouton A). */
  .seg-v {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
  }
  .seg-v button {
    background: var(--panel2);
    color: var(--txt2);
    text-align: left;
    padding: 7px 9px;
    font-size: 11px;
    border-bottom: 1px solid var(--line);
  }
  .seg-v button:last-child {
    border-bottom: none;
  }
  .seg-v button:hover {
    background: var(--raised);
  }
  .seg-v button.on {
    background: var(--rosso);
    color: #fff;
  }
  /* `.check` est aussi utilisée par le bloc Simulation resté dans
     Launch.svelte (aides à la conduite) — dupliquée ici plutôt que partagée
     (CSS Svelte scopé par composant, §conventions projet). */
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 8px 10px;
    cursor: pointer;
    font-size: 10px;
    color: var(--txt2);
  }
</style>
