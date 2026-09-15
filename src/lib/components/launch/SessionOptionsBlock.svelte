<script lang="ts">
  // "Session options" block of the launch screen (SESSION§3): the settings
  // whose content depends on the chosen session type. Pure presentation:
  // everything is a direct read/write of `setup` (state shared with the
  // parent, SESSION§3.3) — no logic to lift up.
  import { type RaceSetup, type StartMode } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";
  import NumberStepper from "../NumberStepper.svelte";
  import Seg from "../Seg.svelte";
  import Tooltip from "../Tooltip.svelte";

  let { setup }: { setup: RaceSetup } = $props();

  // --- Durations: a value of 0 turns its phase off (lot 5 §3.1) -------------
  //
  // The two tick boxes are gone. A tick and a duration were one control drawn
  // as two, and the pair carried a rule of its own — grey the field out, keep
  // the value when unticked — that a single field makes moot: there is nothing
  // left to grey out, and the value one comes back to is the one on screen.
  //
  // The booleans stay in `RaceSetup`: they are what picks Content Manager's
  // mode (`QuickDrive_Weekend` carries qualifying, `QuickDrive_Race` does not),
  // so they are derived from the minutes rather than typed by hand.
  function setQualifyMinutes(v: number) {
    setup.qualify_minutes = v;
    setup.qualify_enabled = v > 0;
    // Free practice only exists in the Weekend mode — the very one qualifying
    // carries. Without qualifying it has nowhere to happen, so it follows.
    if (v === 0) setup.practice_enabled = false;
    else setup.practice_enabled = setup.practice_minutes > 0;
  }
  function setPracticeMinutes(v: number) {
    setup.practice_minutes = v;
    setup.practice_enabled = v > 0 && setup.qualify_minutes > 0;
  }
  /** Free practice has no effect at all without qualifying: the field says so
   * rather than accepting a value that goes nowhere. */
  const practiceUnavailable = $derived(setup.qualify_minutes === 0);

  // --- Starting position (§2.6) ---------------------------------------------
  //
  // **Neutralised by qualifying, and that is Content Manager's own behaviour**,
  // not a choice made here: its starting-position control lives in
  // `QuickDrive_Race` only — the `QuickDrive_Weekend` view, the one that
  // carries qualifying, has no such binding at all. When a qualifying session
  // precedes the race, the grid comes from its results.
  //
  // Dimmed rather than removed, because this is an internal dependency (a
  // duration set here) and not a setting without meaning in this session type.
  //
  // `Random / 1st / 2nd / Last` and no longer `Random / First / 2nd / Last`:
  // the row mixed two ways of writing the same kind of thing.
  const startModes: { value: StartMode; labelKey: string }[] = [
    { value: "random", labelKey: "launch.startRandom" },
    { value: "first", labelKey: "launch.startFirst" },
    { value: "second", labelKey: "launch.startSecond" },
    { value: "last", labelKey: "launch.startLast" },
  ];
  const startFixedByQualifying = $derived(setup.session_type === "race" && setup.qualify_minutes > 0);
</script>

<!-- Session options (SESSION§3): first card of the column, the most consulted.
     Everything here depends on the session type — what does not has left, to
     the car card (ballast, restrictor, aids), to the conditions rail (track
     state) or to the simulation block (penalties).

     Jump start / laps / practice / qualifying are absent from the
     Practice/Hotlap schemas (no grid, no weekend phase): never shown for those
     two types. Track day shares the grid and the jump start with Race, but not
     the lap count: the session never ends on a lap count in game (tested), so
     the setting has no effect there despite being present in the `ModeData`
     that is sent. -->
<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.sessionOptionsLabel")}</span></header>
  <div class="blk-b">
    <div class="varies">
      {#if setup.session_type === "race"}
        <!-- The three durations on one row, all three the same gauge: they
             answer the same question — how long does each part last — and a
             zero is the answer "not at all". -->
        <div>
          <span class="fk lbl-key">{t("launch.laps")}</span>
          <NumberStepper min={1} max={99} width={64} bind:value={setup.laps} />
        </div>
        <div>
          <span class="fk lbl-key">{t("launch.qualifying")}</span>
          <span class="dur">
            <NumberStepper
              min={0}
              max={90}
              width={64}
              zeroDim
              value={setup.qualify_minutes}
              onchange={setQualifyMinutes}
            />
            <span class="unit lbl-key">{t("launch.minutesUnit")}</span>
          </span>
        </div>
        <div class:off={practiceUnavailable}>
          <span class="fk lbl-key"
            >{t("launch.freePractice")}{#if practiceUnavailable}<Tooltip
                text={t("launch.practiceNeedsQualifying")}
                align="left"><button type="button" class="info-i">ⓘ</button></Tooltip
              >{/if}</span
          >
          <span class="dur">
            <NumberStepper
              min={0}
              max={120}
              width={64}
              zeroDim
              disabled={practiceUnavailable}
              value={setup.practice_minutes}
              onchange={setPracticeMinutes}
            />
            <span class="unit lbl-key">{t("launch.minutesUnit")}</span>
          </span>
        </div>
      {/if}

      {#if setup.session_type === "hotlap"}
        <!-- One control out of a tick and a value: the advantage only means
             anything while the ghost is on. Its value survives an untick — one
             comes back to it. -->
        <div class="phase" class:off={!setup.ghost_car}>
          <label class="tick"
            ><input type="checkbox" bind:checked={setup.ghost_car} /><span>{t("launch.ghostCar")}</span></label
          >
          <span class="dur">
            <span class="unit lbl-key">{t("launch.ghostAdvantage")}</span>
            <NumberStepper
              min={0}
              max={5}
              step={0.1}
              decimals={2}
              width={64}
              disabled={!setup.ghost_car}
              bind:value={setup.ghost_advantage}
            />
            <span class="unit lbl-key">{t("launch.secondsUnit")}</span>
          </span>
        </div>
      {/if}

      {#if setup.session_type === "race" || setup.session_type === "trackday"}
        <div>
          <span class="fk lbl-key">{t("launch.jumpStart")}</span>
          <Seg
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

      {#if setup.session_type === "race"}
        <div class:off={startFixedByQualifying}>
          <span class="fk lbl-key"
            >{t("launch.startLabel")}{#if startFixedByQualifying}<Tooltip
                text={t("launch.startFixedByQualifying")}
                align="left"><button type="button" class="info-i">ⓘ</button></Tooltip
              >{/if}</span
          >
          <Seg
            value={setup.start_mode}
            disabled={startFixedByQualifying}
            onselect={(v) => (setup.start_mode = v as StartMode)}
            items={startModes.map((m) => ({ value: m.value, label: t(m.labelKey) }))}
          />
        </div>
      {/if}

      {#if setup.session_type === "practice"}
        <div>
          <span class="fk lbl-key">{t("launch.startFrom")}</span>
          <Seg
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
  /* Each setting keeps its natural width rather than stretching into a grid:
     a slot layout would move every control each time the type changes one of
     them (lot 5 §3.3).

     `flex-end` and not `flex-start`: some settings carry a label above them
     and some do not, so aligning on the top edge left the bare tick boxes
     floating a label's height above their neighbours' controls. What the eye
     lines up is the row of controls, not the row of boxes' top corners. */
  .varies {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 14px 16px;
    /* **La même hauteur dans les quatre types.** Le bloc en changeait à chaque
       fois — un segmenté seul est plus court qu'un champ numérique, et la case
       du ghost n'a pas d'intitulé au-dessus d'elle — si bien que SIMULATION,
       juste dessous, sautait de quelques pixels d'un type à l'autre. Un
       plancher égal à la rangée la plus haute (intitulé + champ de 32 px) le
       fige, et `flex-end` cale les contrôles plus courts sur la même ligne de
       base que les autres plutôt que de les laisser flotter. */
    min-height: 47px;
  }
  .varies > div {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  /* Colour/size/letter-spacing come from `.lbl-key` (global, §labels): only
     what `.lbl-key` does not cover stays here. */
  .fk {
    text-transform: uppercase;
  }
  /* Même ⓘ que SIMULATION : une explication permanente vit là, jamais dans un
     encart jaune. */
  .info-i {
    background: transparent;
    border: none;
    padding: 0 0 0 4px;
    color: var(--muted2);
    font-size: 10px;
    line-height: 1;
  }
  .info-i:hover {
    color: var(--txt2);
  }
  .dur {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .dur .unit {
    text-transform: none;
  }
  /* One control out of a tick and a duration. The stepper keeps its own
     border, and the negative margin collapses it onto the tick's: a single
     shared line, which is the separator.

     **`.varies > div.phase` et non `.phase`** : la règle générique ci-dessus
     empile les réglages en colonne (intitulé puis contrôle), et elle est plus
     spécifique qu'une classe seule — l'avance du ghost passait donc SOUS sa
     case au lieu de se poser à côté d'elle. Le sélecteur remet la rangée à
     l'horizontale en gagnant la même spécificité. */
  .varies > div.phase {
    display: flex;
    flex-direction: row;
    align-items: stretch;
    gap: 0;
  }
  .phase .tick {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 0 10px;
    height: 32px;
    cursor: pointer;
    font-size: 10px;
    color: var(--txt2);
  }
  .phase .dur {
    margin-left: -1px;
  }
  .off {
    opacity: 0.55;
  }
</style>
