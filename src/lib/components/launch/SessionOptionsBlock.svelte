<script lang="ts">
  // Bloc « Options de session » de l'écran Lancement (§8.4/§8.6) : réglages
  // dont le contenu dépend du type de session choisi (Practice/Hotlap/Course),
  // plus deux réglages communs aux trois (évolution du grip, pénalités). Vue
  // de présentation pure : tout est une lecture/écriture directe de `setup`
  // (état partagé avec le parent, §8.6bis) — aucune logique à faire remonter.
  import type { RaceSetup } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";
  import NumberStepper from "../NumberStepper.svelte";
  import Seg from "../Seg.svelte";

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
         cantonner à Course. Faux départ / tours / essais / qualif sont
         absents des schémas Practice/Hotlap (pas de grille, pas de phase
         weekend) : jamais affichés pour ces deux types. Track day partage
         la grille et le faux départ avec Course (schéma confirmé), mais
         pas le nombre de tours : la session ne se termine jamais par un
         compte de tours en jeu (retour testé), donc le réglage n'a aucun
         effet malgré sa présence dans le ModeData envoyé. -->
    <div class="opts-row">
      {#if setup.session_type === "hotlap"}
        <label class="check"><input type="checkbox" bind:checked={setup.ghost_car} /><span>{t("launch.ghostCar")}</span></label>
      {:else if setup.session_type === "race" || setup.session_type === "trackday"}
        {#if setup.session_type === "race"}
          <label class="grid-fields">
            <NumberStepper min={1} max={99} bind:value={setup.laps} />
            <span class="fk lbl-key">{t("launch.laps")}</span>
          </label>
        {/if}
        <div><span class="fk lbl-key">{t("launch.jumpStart")}</span>
          <Seg
            vertical
            value={String(setup.jump_start_penalty)}
            onselect={(v) => (setup.jump_start_penalty = Number(v))}
            items={[
              { value: "0", label: t("launch.jumpStartNone") },
              { value: "1", label: t("launch.jumpStartTeleport") },
              { value: "2", label: t("launch.jumpStartDrivethrough") },
            ]}
          />
        </div>
      {/if}

      <div><span class="fk lbl-key">{t("launch.gripEvolution")}</span>
        <Seg
          vertical
          value={String(setup.grip)}
          onselect={(v) => (setup.grip = Number(v))}
          items={[
            { value: "86", label: t("launch.gripGreen") },
            { value: "92", label: t("launch.gripMedium") },
            { value: "96", label: t("launch.gripRubbered") },
            { value: "100", label: t("launch.gripOptimal") },
          ]}
        />
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
          <Seg
            vertical
            value={setup.practice_start}
            onselect={(v) => (setup.practice_start = v as RaceSetup["practice_start"])}
            items={[
              { value: "pit", label: t("launch.startFromPit") },
              { value: "track", label: t("launch.startFromTrack") },
              { value: "hotlap", label: t("launch.startFromHotlap") },
            ]}
          />
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
