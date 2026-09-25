<script lang="ts">
  // --- Repli « Performance » de la carte voiture (L5§2.1) ---------------
  //
  // Lest, bride, ABS et contrôle de traction sous une seule ligne, au gabarit
  // de LIVRÉE et PILOTE. Les deux assistances viennent de l'écran de réglages,
  // où elles vivaient parmi les règles de la course : ce sont des **capacités
  // de la voiture**, et le réglage n'existe que parce que la voiture les
  // possède — ce que dit déjà la ligne « Factory » qui les suit ici.
  //
  // Le repli n'est pas mémorisé, et il n'a pas à l'être : la ligne **affiche
  // toujours son état**, replié ou non, donc l'ouvrir ne révèle rien qu'on ne
  // sache déjà — elle ne fait que rendre les champs modifiables.
  //
  // The `.field` rows come from `SessionColumn`, which owns that grammar for
  // every line of the column.
  import Seg from "$lib/components/ui/Seg.svelte";
  import Tooltip from "$lib/components/ui/Tooltip.svelte";
  import {
    BALLAST_MAX,
    RESTRICTOR_MAX,
    playerHandicap,
    setPlayerHandicap,
  } from "$lib/launch/playerHandicap.svelte";
  import { carAssists, assistsTouched } from "$lib/launch/carAssists.svelte";
  import { carFactoryAssists, type AssistLevel, type FactoryAssists } from "$lib/launch/launch";
  import { nav } from "$lib/shell/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    /** The car's name as the column writes it, for the « Factory » line. */
    carName: string;
  }
  let { carName }: Props = $props();

  let perfOpen = $state(false);
  const perfTouched = $derived(playerHandicap.ballast > 0 || playerHandicap.restrictor > 0 || assistsTouched());
  /** Ce que la ligne dit repliée. `Stock` quand rien n'est posé — « absent »
   * et « à zéro » ne doivent pas se ressembler, donc jamais un champ vide. */
  const perfSummary = $derived.by(() => {
    if (!perfTouched) return t("session.perfStock");
    const parts: string[] = [];
    if (playerHandicap.ballast > 0)
      parts.push(t("session.perfBallast", { kg: playerHandicap.ballast }));
    if (playerHandicap.restrictor > 0)
      parts.push(t("session.perfRestrictor", { pct: playerHandicap.restrictor }));
    if (carAssists.abs !== "factory")
      parts.push(`${t("launch.absLabel")} ${t(`launch.assist${carAssists.abs === "on" ? "On" : "Off"}`)}`);
    if (carAssists.tractionControl !== "factory")
      parts.push(
        `${t("launch.tcShort")} ${t(`launch.assist${carAssists.tractionControl === "on" ? "On" : "Off"}`)}`,
      );
    return parts.join(" · ");
  });
  const assistLevels = $derived([
    { value: "off", label: t("launch.assistOff") },
    { value: "factory", label: t("launch.assistFactory") },
    { value: "on", label: t("launch.assistOn") },
  ]);
  /** Ce que `Factory` vaut pour cette voiture, lu dans son `electronics.ini`.
   * La ligne suit le repli : c'est elle qui justifie que le réglage soit là.
   * Une voiture qui ne dit rien n'a pas de ligne du tout — jamais d'« inconnu ». */
  let factoryAssists = $state<FactoryAssists | null>(null);
  $effect(() => {
    const carId = nav.sessionCar?.id;
    if (!carId) {
      factoryAssists = null;
      return;
    }
    let current = true;
    carFactoryAssists(carId)
      .then((found) => {
        if (current) factoryAssists = found;
      })
      .catch(() => {
        if (current) factoryAssists = null;
      });
    return () => {
      current = false;
    };
  });
  const factoryLine = $derived.by(() => {
    const f = factoryAssists;
    if (!f || !carName) return null;
    const key = f.abs
      ? f.tractionControl
        ? "launch.factoryBoth"
        : "launch.factoryAbsOnly"
      : f.tractionControl
        ? "launch.factoryTcOnly"
        : "launch.factoryNeither";
    return t(key, { car: carName });
  });
</script>

<!-- PERFORMANCE (L5§2.1) : lest, bride, ABS et contrôle de
     traction sous une ligne unique, au gabarit de LIVRÉE et
     PILOTE. Quatre réglages valaient quatre lignes dans une colonne
     dont la hauteur est la ressource rare, et les deux assistances
     vivaient à l'autre bout de l'écran parmi les règles de la
     course alors que ce sont des capacités de la VOITURE.

     **La ligne affiche toujours son état**, repliée ou non : rien
     n'est masqué, seulement rendu non modifiable — « absent » et
     « à zéro » ne doivent pas se ressembler.

     Chevron vers le bas et non vers la droite, contrairement à la
     maquette : dans cette colonne, `›` annonce une destination
     (l'écran Pilote) et `▾` un dépliement sur place. La distinction
     est ténue mais constante, et c'est elle qui fait foi. -->
<button class="field" type="button" aria-expanded={perfOpen} onclick={() => (perfOpen = !perfOpen)}>
  <span class="k">{t("session.fieldPerformance")}</span>
  <span class="v" class:stock={!perfTouched} class:set={perfTouched}>{perfSummary}</span>
  <span class="chev" aria-hidden="true">{perfOpen ? "▴" : "▾"}</span>
</button>
{#if perfOpen}
  <div class="perf">
    <label class="field">
      <span class="k">{t("session.fieldBallast")}</span>
      <input
        class="hcap mono"
        class:set={playerHandicap.ballast > 0}
        type="number"
        min="0"
        max={BALLAST_MAX}
        value={playerHandicap.ballast}
        onchange={(e) => setPlayerHandicap(Number(e.currentTarget.value), playerHandicap.restrictor)}
      />
      <span class="unit">{t("session.ballastUnit")}</span>
    </label>
    <label class="field">
      <span class="k">{t("session.fieldRestrictor")}</span>
      <input
        class="hcap mono"
        class:set={playerHandicap.restrictor > 0}
        type="number"
        min="0"
        max={RESTRICTOR_MAX}
        value={playerHandicap.restrictor}
        onchange={(e) => setPlayerHandicap(playerHandicap.ballast, Number(e.currentTarget.value))}
      />
      <span class="unit">%</span>
    </label>
    <!-- Trois états et non une case : une case ne saurait pas
         distinguer « ce que la vraie voiture avait » de « forcé »,
         et c'est le milieu qui est le défaut. -->
    <div class="assist">
      <span class="k"
        >{t("launch.absLabel")}<Tooltip text={t("launch.absTooltip")} align="left"
          ><button type="button" class="info-i">ⓘ</button></Tooltip
        ></span
      >
      <Seg
        size="mini"
        value={carAssists.abs}
        onselect={(v) => (carAssists.abs = v as AssistLevel)}
        items={assistLevels}
      />
    </div>
    <div class="assist">
      <span class="k"
        >{t("launch.tcShort")}<Tooltip text={t("launch.tractionTooltip")} align="left"
          ><button type="button" class="info-i">ⓘ</button></Tooltip
        ></span
      >
      <Seg
        size="mini"
        value={carAssists.tractionControl}
        onselect={(v) => (carAssists.tractionControl = v as AssistLevel)}
        items={assistLevels}
      />
    </div>
    {#if factoryLine}
      <p class="factory-note"><span class="fw">{t("launch.assistFactory")}</span> — {factoryLine}</p>
    {/if}
  </div>
{/if}

<style>
  /* Champ nu : c'est la ligne qui porte le cadre, comme les cellules du
     plateau. Un `NumberStepper` y mettrait un second cadre dans le premier. */
  .hcap {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    padding: 0;
    color: var(--faint);
    font-size: 11px;
    text-align: right;
    appearance: textfield;
  }
  .hcap::-webkit-outer-spin-button,
  .hcap::-webkit-inner-spin-button {
    appearance: none;
    margin: 0;
  }
  /* Zéro est éteint — il n'y a pas de handicap —, toute autre valeur est rouge.
     C'est le seul moyen qu'un lest oublié se voie sans lire la ligne, et le
     rouge est ici au sens du barème : un réglage qui change la course. */
  .hcap.set {
    color: var(--rosso-bright);
  }
  .perf {
    display: flex;
    flex-direction: column;
    gap: 5px;
    /* Retrait et filet : les quatre réglages appartiennent à la ligne qui les
       a ouverts, comme la sous-entrée appartient à son type. */
    margin-left: 8px;
    padding-left: 8px;
    border-left: 1px solid var(--line);
  }
  /* Même gouttière d'intitulé que `.field`, sans son cadre : ce n'est pas un
     champ mais un réglage posé sous la ligne qui le commande. */
  .assist {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
  }
  .assist .k {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    flex: 0 0 var(--sess-lblw, 60px);
    max-width: 88px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 8.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--muted);
  }
  /* Même ⓘ que partout : une explication permanente vit là, jamais dans un
     encart jaune. */
  .info-i {
    background: transparent;
    border: none;
    padding: 0;
    color: var(--muted2);
    font-size: 10px;
    line-height: 1;
  }
  .info-i:hover {
    background: transparent;
    color: var(--txt2);
  }
  /* Ce que l'app sait et que le volant seul apprendrait — même registre que la
     note implicite de la météo. Le bleu est l'information au barème (§7.2ter) :
     rien ne va mal ici. */
  .factory-note {
    color: var(--muted);
    font-size: 10px;
    margin: 2px 0 0;
    line-height: 1.45;
  }
  .factory-note .fw {
    color: var(--blue);
  }
</style>
