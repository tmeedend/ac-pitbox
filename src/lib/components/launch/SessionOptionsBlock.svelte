<script lang="ts">
  // "Session options" block of the launch screen (§8.4/§8.6): the settings
  // whose content depends on the chosen session type, plus the two that never
  // do (grip evolution, penalties). Pure presentation: everything is a direct
  // read/write of `setup` (state shared with the parent, §8.6bis) — no logic
  // to lift up.
  import { GRIP_WEATHER, TRACK_GRIPS, type RaceSetup } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";
  import NumberStepper from "../NumberStepper.svelte";
  import Seg from "../Seg.svelte";

  let { setup }: { setup: RaceSetup } = $props();

  // Free practice only exists in CM's Weekend mode — the very one that carries
  // qualifying. Without qualifying the preset falls back to the plain race
  // mode, where no preparatory phase exists (§9.3): leaving it tickable would
  // show a setting with no effect in game.
  function toggleQualifying(on: boolean) {
    setup.qualify_enabled = on;
    if (!on) setup.practice_enabled = false;
  }
</script>

<!-- Session options (§8.4/§8.6): first card of the column, the most consulted.
     Two zones, and only the left one varies.

     The left zone holds what depends on the session type, the right one what
     never does. Grip evolution and penalties are sent to the Quick Drive
     preset whatever the type (`Penalties` sits in all three `ModeData`,
     `TrackPropertiesData` at the preset root, not in `ModeData`), so they keep
     the exact same place in the four types — before this split they travelled
     across the row as the type-dependent controls appeared and disappeared,
     and the two settings one never changes were the hardest to find again.

     Jump start / laps / practice / qualifying are absent from the
     Practice/Hotlap schemas (no grid, no weekend phase): never shown for those
     two types. Track day shares the grid and the jump start with Race, but not
     the lap count: the session never ends on a lap count in game (tested), so
     the setting has no effect there despite being present in the `ModeData`
     that is sent. -->
<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.sessionOptionsLabel")}</span></header>
  <div class="blk-b">
    <div class="opts">
      <div class="varies">
        {#if setup.session_type === "hotlap"}
          <label class="check"><input type="checkbox" bind:checked={setup.ghost_car} /><span>{t("launch.ghostCar")}</span></label>
        {/if}

        {#if setup.session_type === "race"}
          <!-- Libellé AU-DESSUS du champ, comme tout le reste du bloc : il
               était le seul posé à droite du sien, ce qui faisait lire la
               rangée en zigzag. -->
          <div>
            <span class="fk lbl-key">{t("launch.laps")}</span>
            <NumberStepper min={1} max={99} bind:value={setup.laps} />
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

        <!-- Tick and duration are one control, not two. Left apart, the
             duration kept living next to a box that no longer commanded it —
             and an unticked phase still showed an editable field. Welded, and
             the frame dims as a whole: the value stays readable (one comes
             back to it) but says it applies to nothing. -->
        {#if setup.session_type === "race"}
          <div class="phase" class:off={!setup.qualify_enabled}>
            <label class="tick">
              <input
                type="checkbox"
                checked={setup.qualify_enabled}
                onchange={(e) => toggleQualifying(e.currentTarget.checked)}
              /><span>{t("launch.qualifying")}</span>
            </label>
            <span class="dur">
              <NumberStepper min={5} max={90} width={58} disabled={!setup.qualify_enabled} bind:value={setup.qualify_minutes} />
              <span class="unit lbl-key">{t("launch.minutesUnit")}</span>
            </span>
          </div>

          <div class="phase" class:off={!setup.practice_enabled}>
            <label class="tick">
              <input type="checkbox" bind:checked={setup.practice_enabled} disabled={!setup.qualify_enabled} /><span
                >{t("launch.freePractice")}</span
              >
            </label>
            <span class="dur">
              <NumberStepper min={1} max={120} width={58} disabled={!setup.practice_enabled} bind:value={setup.practice_minutes} />
              <span class="unit lbl-key">{t("launch.minutesUnit")}</span>
            </span>
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

      <div class="fixed">
        <!-- Les six états que le jeu embarque (`cfg/templates/tracks.ini`), dans
             l'ordre et avec les noms de sa propre liste — celle que Content
             Manager affiche aussi. Vertical : six libellés nommés ne tiennent
             pas sur une ligne dans cette colonne, et une liste d'états se lit
             de haut en bas. Chacun porte au survol ce que le jeu en dit.

             « Auto » en tête, comme chez CM : ce n'est pas un état mais le
             drapeau `WeatherDefined`, qui laisse la météo décider et retombe
             sur Verte si elle ne dit rien. -->
        <div>
          <span class="fk lbl-key">{t("launch.gripEvolution")}</span>
          <Seg
            vertical
            value={String(setup.grip)}
            onselect={(v) => (setup.grip = Number(v))}
            items={[
              { value: String(GRIP_WEATHER), label: t("launch.gripAuto"), title: t("launch.gripAutoDesc") },
              ...TRACK_GRIPS.map((g) => ({
                value: String(g),
                label: t(`launch.grip${g}`),
                title: t(`launch.grip${g}Desc`),
              })),
            ]}
          />
        </div>
      </div>
    </div>
  </div>
</section>

<style>
  /* The threshold is a container query, not a media one: what decides whether
     the two zones fit side by side is the width this block actually got — the
     rail and the session column have already taken theirs, and the interface
     zoom moves a window threshold without moving anything here (§13). */
  .blk-b {
    container: sessopts / inline-size;
  }
  .opts {
    display: grid;
    /* The right zone is sized by its content, not by a number: its widest
       control is the grip segmented, whose six options never change, so its
       width is the same in the four session types — which is the whole point.
       A fixed width would only be a guess about a column whose own width
       depends on the window. */
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 14px 24px;
    align-items: start;
  }
  @container sessopts (max-width: 520px) {
    .opts {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  /* Each setting keeps its natural width rather than stretching into a grid:
     a slot layout would move every control each time the type changes one of
     them, which is exactly what the two zones exist to avoid.

     `flex-end` and not `flex-start`: some settings carry a label above them
     and some do not, so aligning on the top edge left the bare tick boxes
     floating a label's height above their neighbours' controls. What the eye
     lines up is the row of controls, not the row of boxes' top corners. */
  .varies {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 14px 16px;
  }
  /* Stacked, in this order, in the four session types — that is the whole
     point of this zone. Side by side they would re-flow with the width. */
  .fixed {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  .varies > div,
  .fixed > div {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  /* Colour/size/letter-spacing come from `.lbl-key` (global, §labels): only
     what `.lbl-key` does not cover stays here. */
  .fk {
    text-transform: uppercase;
  }
  /* `.check` is also defined by the Simulation block — duplicated rather than
     shared (Svelte CSS is scoped per component, §project conventions). */
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
  /* One control out of a tick and a duration. The stepper keeps its own
     border, and the negative margin collapses it onto the tick's: a single
     shared line, which is the separator. */
  .phase {
    display: flex;
    align-items: stretch;
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
    display: flex;
    align-items: center;
    gap: 7px;
    margin-left: -1px;
  }
  .phase .unit {
    text-transform: none;
  }
  .phase.off {
    opacity: 0.55;
  }
</style>
