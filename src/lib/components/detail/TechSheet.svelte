<script lang="ts">
  // The one tech sheet of a car. Only the full detail page
  // (`DetailPage.svelte`) draws it now, since the compact side panel was
  // removed — but the rows stay here rather than inlined there, because of how
  // they came to be shared in the first place.
  //
  // The two used to each build their own list, and they had drifted: the panel
  // showed the whole native sheet (power, torque, weight, top speed, 0-100,
  // power/weight, range, country) plus the five harmonized fields, while the
  // page showed six rows and left out everything the engine actually says about
  // itself. Same screen name, same title, half the content - reported by the
  // user, and exactly what the "shared components" chantier is about: two
  // copies of one thing drift, and nothing flags it.
  //
  // The frame stays the host's business: what is shared is the content and the
  // row itself.
  import { countryLabel, flagFor, loadFlags } from "$lib/flags.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { ModDetail } from "$lib/library/library";
  import { odometerText } from "$lib/detail/odometer";

  interface Props {
    detail: ModDetail;
    /** Narrowest a column may get before the grid drops one. The sheet fits
     * itself to whatever width the host gives it: two columns in the side
     * panel, three on the page, without either of them saying so. */
    minColumn?: number;
    /** Cell background — the host's own card surface, so the sheet does not
     * look pasted onto it. */
    surface?: "panel" | "panel2";
    /** Outer 1px rule. Off when the host already frames it (a `.blk` card). */
    framed?: boolean;
  }
  let { detail, minColumn = 128, surface = "panel", framed = true }: Props = $props();

  interface Row {
    label: string;
    value: string;
    /** Deduced by the rule engine (§6) rather than read in the mod's own
     * file — shown in the rule colour, with the tooltip that says so. */
    derived?: boolean;
    /** Drapeau du pays, quand la rangée en est un. Les autres n'en ont pas :
     * c'est une propriété de la valeur, pas de la fiche. */
    flag?: string | null;
  }

  /** Engine position, abbreviated: the sheet is a grid of narrow cells, and
   * "REAR" spelled out pushes a column wider than the value deserves. */
  function posLabel(pos: string): string {
    if (pos === "FRONT") return t("detail.posFront");
    if (pos === "MID") return t("detail.posMid");
    if (pos === "REAR") return t("detail.posRear");
    return pos;
  }

  // Idempotent : la barre de filtres a déjà pu la charger, et deux appels ne
  // coûtent qu'un test. `loadFlags` écrit le cache sans le lire, donc cet effet
  // ne s'y abonne pas.
  $effect(() => {
    void loadFlags();
  });

  const rows = $derived.by(() => {
    const d = detail;
    const out: Row[] = [];
    const add = (label: string, value: string | null | undefined, derived = false, flag: string | null = null) => {
      if (value) out.push({ label, value, derived, flag });
    };
    const s = d.specs;
    // Read as-is in the mod's `ui_car.json`, empty rows skipped: a sheet of
    // dashes says nothing that its absence would not.
    add(t("modpanel.specPower"), s?.bhp);
    add(t("modpanel.specTorque"), s?.torque);
    add(t("columns.weight"), s?.weight);
    add(t("modpanel.specTopSpeed"), s?.topspeed);
    add(t("modpanel.specAccel"), s?.acceleration);
    add(t("modpanel.specPwRatio"), s?.pwratio);
    add(t("modpanel.specRange"), s?.range);
    {
      // **Le pays rangé passe devant celui du fichier**, contrairement à toutes
      // les lignes au-dessus. C'est le seul champ de cette fiche que l'app
      // normalise (§5, alias de pays), et montrer ici « U.S.A. » quand le filtre,
      // la colonne et le drapeau disent « United States » ferait douter qu'il
      // s'agisse du même pays. Le fichier du mod n'est pas trahi pour autant :
      // il n'est jamais réécrit, c'est l'overlay qui porte la valeur.
      const country = d.country ?? s?.country;
      add(t("columns.country"), country ? countryLabel(country) : country, false, flagFor(country));
    }
    // Deduced by the rules, hence the separate look.
    add(t("columns.drivetrain"), d.drivetrain, true);
    add(t("columns.aspiration"), d.aspiration, true);
    add(t("columns.engineConfig"), d.engine_config, true);
    add(t("columns.enginePos"), d.engine_pos ? posLabel(d.engine_pos) : null, true);
    add(t("columns.gearbox"), d.gearbox, true);
    out.push({ label: t("detail.odometer"), value: odometerText(d) });
    return out;
  });

  // --- La dernière cellule, quand elle est seule sur sa rangée --------------
  //
  // L'odomètre est toujours présent et toujours dernier : dès que le nombre de
  // lignes au-dessus de lui est un multiple du nombre de colonnes, il se
  // retrouve seul avec une cellule vide à côté. Il prend alors la rangée
  // entière.
  //
  // **Le nombre de colonnes se mesure, il ne se déduit pas.** `auto-fit` le
  // choisit à partir de la largeur reçue, et rien en CSS ne permet d'écrire
  // « si la dernière est seule » ; un compte fixé par l'appelant serait faux la
  // moitié du temps, la fiche passant de deux colonnes à quatre selon que la
  // courbe de puissance occupe ou non la moitié du bloc. Le style calculé, lui,
  // rend les pistes réellement créées.
  let grid = $state<HTMLDivElement | null>(null);
  let cols = $state(0);
  $effect(() => {
    const el = grid;
    if (!el) return;
    // Affectation seule, jamais de lecture de `cols` : la relire ici
    // abonnerait l'effet à ce qu'il écrit, et il se rappellerait sans fin.
    const read = () => {
      cols = getComputedStyle(el).gridTemplateColumns.split(" ").filter(Boolean).length;
    };
    read();
    const observer = new ResizeObserver(read);
    observer.observe(el);
    return () => observer.disconnect();
  });
  const spanLast = $derived(cols > 1 && rows.length % cols === 1);
</script>

<div
  class="ts {surface}"
  class:framed
  class:span-last={spanLast}
  style:--ts-min="{minColumn}px"
  bind:this={grid}
>
  {#each rows as r (r.label)}
    <div class="cell" class:derived={r.derived} title={r.derived ? t("modpanel.derivedTooltip") : undefined}>
      <div class="lbl-key k">{r.label}</div>
      <div class="v">{#if r.flag}<img class="flag" src={r.flag} alt="" />{/if}{r.value}</div>
    </div>
  {/each}
</div>

<style>
  /* Les lignes du quadrillage sont le fond qui transparaît dans l'interstice
     de 1px entre les cellules — pas des bordures, qui doubleraient d'épaisseur
     entre deux voisines. `auto-fit` : c'est la largeur reçue qui décide du
     nombre de colonnes, pas l'appelant. */
  .ts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(var(--ts-min, 128px), 1fr));
    gap: 1px;
    background: var(--line);
  }
  /* Dernière cellule seule sur sa rangée : elle la prend entière plutôt que de
     laisser un trou à côté d'elle. La condition est mesurée côté script
     (`spanLast`) : CSS ne sait pas dire combien de colonnes `auto-fit` a
     créées. */
  .ts.span-last .cell:last-child {
    grid-column: 1 / -1;
  }
  /* Le trait extérieur, quand l'hôte n'encadre pas déjà la fiche lui-même. */
  .ts.framed {
    padding: 1px;
  }
  /* Mêmes valeurs que la colonne Nationalité du plateau et que les filtres :
     une seule taille de drapeau dans l'app. Aligné sur la ligne de base du
     texte plutôt que sur sa boîte, sans quoi il pend sous les lettres. */
  .flag {
    width: 16px;
    height: 12px;
    object-fit: cover;
    border: 1px solid var(--line);
    vertical-align: -1px;
    margin-right: 5px;
  }
  .cell {
    background: var(--panel);
    padding: 6px 9px;
    min-width: 0;
  }
  .ts.panel2 .cell {
    background: var(--panel2);
  }
  .k {
    text-transform: uppercase;
    margin-bottom: 3px;
  }
  .v {
    font-size: 11.5px;
    color: var(--txt2);
    font-family: var(--mono);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Vert = déduit par une règle, même code couleur que partout ailleurs. */
  .cell.derived .v {
    color: var(--green);
  }
</style>
