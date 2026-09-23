<script lang="ts">
  // Rail de navigation (SPEC §7.2) : le premier des trois territoires — le
  // rail porte les LIEUX, la barre de titre la forme de la fenêtre, la colonne
  // de session ce qu'on lance. La répartition doit rester étanche : toute
  // entrée nouvelle se rattache à l'un des trois, et « Ouvrir CM » vit dans la
  // colonne parce que c'est un chemin de lancement, pas une destination.
  //
  // Les quatre inventaires sont des entrées à part entière et non les onglets
  // d'un écran « Add-ons » : ils possèdent DÉJÀ leurs propres onglets internes
  // (skins/sons, catégories…), et les grouper produirait deux rangées
  // d'onglets horizontales de forme identique, sans que rien n'indique
  // laquelle commande l'autre. L'Atelier, à l'inverse, réunit quatre outils
  // qui n'ont aucune sous-rubrique — c'est la seule raison pour laquelle ce
  // regroupement-là est légitime.
  import { nav, requestSection } from "$lib/shell/nav.svelte";
  import { openContentManager } from "$lib/launch/launch";
  import { t } from "$lib/i18n/index.svelte";

  type Entry = {
    /** Section ouverte par un clic. */
    target: string;
    labelKey: string;
    /** Sections qui rendent l'entrée active — l'Atelier en couvre quatre,
     * « Compléments » en couvre deux depuis qu'il a absorbé les apps. */
    sections?: string[];
    /** Filet de séparation AVANT cette entrée. */
    sep?: boolean;
    /** Intitulé du rang, posé au-dessus de l'entrée (REFONTE§3). Les deux
     * rangs ne classent pas par type de contenu mais par **durée de validité**
     * de ce qu'on y règle : ce qui se décide à chaque session, et ce qui reste
     * vrai jusqu'à nouvel ordre. */
    group?: string;
    /** Pousse l'entrée (et ses suivantes) en pied de rail. */
    foot?: boolean;
    /** L'entrée n'ouvre pas un écran mais **sort de l'application**. Une
     * seule aujourd'hui : Content Manager. Elle n'est donc jamais active, ne
     * porte pas d'`aria-current`, et disparaît entièrement quand sa cible
     * n'est pas détectée — une sortie vers un outil absent n'a pas à occuper
     * une ligne. */
    exit?: boolean;
  };

  const ENTRIES: Entry[] = [
    // **Le circuit avant la voiture**, comme dans la colonne de session : c'est
    // l'ordre de la décision (SESSION§1), et deux listes qui portent les mêmes
    // entités dans deux ordres différents se paient à chaque coup d'œil.
    { target: "tracks", labelKey: "nav.tracks", group: "nav.groupSession" },
    { target: "cars", labelKey: "nav.cars" },
    { target: "driver", labelKey: "nav.driver" },
    // Les deux écrans d'add-ons ont disparu : ils classaient par mécanique de
    // pose, et leur contenu est dans l'inventaire (REFONTE§3.1).
    { target: "apps", labelKey: "nav.apps", sep: true, group: "nav.groupGame" },
    { target: "others", labelKey: "nav.others" },
    // Deuxième filet : isole les outils.
    { target: "rules", labelKey: "nav.atelier", sections: ["rules", "categories", "countries", "import", "profiles", "maintenance"], sep: true },
    // Troisième filet : détache le pied.
    { target: "settings", labelKey: "nav.settings", sep: true, foot: true },
    // **Ouvrir Content Manager vit ici**, entre les deux entrées du pied. Ce
    // n'est toujours pas une destination — CM ne reçoit ni la voiture ni le
    // circuit, il s'ouvre sur son propre état — mais le pied du rail ne porte
    // déjà plus des lieux qu'on parcourt : il porte ce qu'on ouvre à part, et
    // l'autre outil de la chaîne y a sa place mieux que dans une colonne dont
    // la hauteur est comptée. Décidé avec l'utilisateur.
    // Libellé court : le rail fait 74 px, « Ouvrir Content Manager » y tenait
    // sur trois lignes quand toutes les autres entrées en font deux. L'icône —
    // la flèche qui sort du cadre — porte le « ouvrir », le nom suffit.
    { target: "cm", labelKey: "nav.openCmShort", exit: true },
    { target: "about", labelKey: "nav.about" },
  ];

  /** Rubriques qui réclament l'attention, par section. Pas de compteur : le
   * nombre exact ne change pas la décision d'aller voir. */
  let {
    alerts = {},
    cmAvailable = false,
  }: {
    alerts?: Record<string, boolean>;
    /** Content Manager détecté au chemin configuré (`validate_config`). Faux,
     * l'entrée n'est pas rendue du tout — ni grisée, ni suivie d'un message
     * d'erreur au clic. */
    cmAvailable?: boolean;
  } = $props();

  const shown = $derived(ENTRIES.filter((e) => !e.exit || cmAvailable));

  function isActive(e: Entry): boolean {
    if (e.exit) return false;
    return (e.sections ?? [e.target]).includes(nav.section);
  }

  function activate(e: Entry) {
    if (e.exit) {
      void openContentManager().catch((err) => console.error(err));
      return;
    }
    void requestSection(e.target);
  }
  function hasAlert(e: Entry): boolean {
    return (e.sections ?? [e.target]).some((s) => alerts[s]);
  }

  /** Flèches haut/bas pour circuler dans le rail : c'est ce qu'un utilisateur
   * au clavier attend d'une barre de navigation verticale, et sans ça il faut
   * neuf tabulations pour atteindre « À propos ». `Entrée` reste le clic
   * natif du bouton, rien à faire pour lui. Boucle aux deux extrémités —
   * remonter depuis la première entrée est plus court que huit descentes. */
  function onKeydown(ev: KeyboardEvent) {
    if (ev.key !== "ArrowDown" && ev.key !== "ArrowUp") return;
    const rail = ev.currentTarget as HTMLElement;
    const items = Array.from(rail.querySelectorAll<HTMLButtonElement>("button.entry"));
    const here = items.indexOf(document.activeElement as HTMLButtonElement);
    if (here < 0) return;
    ev.preventDefault();
    const next = (here + (ev.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
    items[next].focus();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<nav class="rail" data-gp-region="rail" aria-label={t("nav.railLabel")} onkeydown={onKeydown}>
  {#each shown as e (e.target)}
    {#if e.foot}<div class="spacer"></div>{/if}
    {#if e.sep}<div class="sep"></div>{/if}
    {#if e.group}<span class="grp">{t(e.group)}</span>{/if}
    {@const active = isActive(e)}
    {@const alert = hasAlert(e)}
    <button
      class="entry"
      class:on={active}
      type="button"
      aria-current={active ? "page" : undefined}
      aria-label={alert ? `${t(e.labelKey)}, ${t("nav.alertLabel")}` : undefined}
      onclick={() => activate(e)}
    >
      <span class="ic">
        <svg viewBox="0 0 20 20" aria-hidden="true">
          {#if e.target === "cars"}
            <path d="M2.5 12.5v-1.2l1.6-3.6A2 2 0 0 1 6 6.5h8a2 2 0 0 1 1.9 1.2l1.6 3.6v1.2" />
            <path d="M2.5 12.5h15v1.6a.8.8 0 0 1-.8.8h-1.4a.8.8 0 0 1-.8-.8v-.6M5.5 13.5v.6a.8.8 0 0 1-.8.8H3.3a.8.8 0 0 1-.8-.8v-1.6" />
            <path d="M4.4 10.5h11.2" />
          {:else if e.target === "tracks"}
            <path d="M4.5 14.8c-2 0-3-1.3-3-2.7 0-1.5 1.2-2.4 2.7-2.7 1.9-.4 3.4-.2 4.7-1.1 1.1-.8.8-2.3 2.2-3 1.6-.8 4.4-.4 5.6 1 1.4 1.7.6 4-1.4 4.7-1.7.6-3.2 0-4.6.4-1.2.4-1.4 1.5-2.4 2.4-.9.7-2.2 1-3.8 1z" />
          {:else if e.target === "driver"}
            <path d="M10 2.6a7 7 0 0 1 7 7v1.3a1.4 1.4 0 0 1-1.4 1.4H10z" />
            <path d="M3 11.4a7 7 0 0 1 7-7" />
            <path d="M3 11.4v2.6a1.6 1.6 0 0 0 1.6 1.6h9.2" />
            <path d="M6.6 15.6v-1.4a1.6 1.6 0 0 1 1.6-1.6h1" />
          {:else if e.target === "apps"}
            <path d="M3.2 3.2h5.4v5.4H3.2zM11.4 3.2h5.4v5.4h-5.4zM3.2 11.4h5.4v5.4H3.2z" />
            <path d="M14.1 11.4v5.4M11.4 14.1h5.4" />
          {:else if e.target === "others"}
            <path d="M2.6 6.4 10 3.1l7.4 3.3-7.4 3.3z" />
            <path d="M2.6 6.4v7.2l7.4 3.3 7.4-3.3V6.4" />
            <path d="M10 9.7v7.2" />
          {:else if e.target === "rules"}
            <path d="M12.4 2.9a4.2 4.2 0 0 0-4 5.5l-5.1 5.1a1.6 1.6 0 0 0 2.2 2.2l5.1-5.1a4.2 4.2 0 0 0 5.5-4l-2.4 2.4-2.4-.7-.7-2.4z" />
          {:else if e.target === "cm"}
            <!-- La flèche qui sort du cadre : on quitte l'application. -->
            <path d="M10.6 3.5h5.9v5.9" />
            <path d="M16.5 3.5 9.4 10.6" />
            <path d="M13.8 12.2v3.5a.8.8 0 0 1-.8.8H4.3a.8.8 0 0 1-.8-.8V7a.8.8 0 0 1 .8-.8h3.5" />
          {:else if e.target === "settings"}
            <circle cx="10" cy="10" r="2.6" />
            <path d="M10 1.9v2.2M10 15.9v2.2M18.1 10h-2.2M4.1 10H1.9M15.7 4.3l-1.5 1.5M5.8 14.2l-1.5 1.5M15.7 15.7l-1.5-1.5M5.8 5.8 4.3 4.3" />
          {:else}
            <circle cx="10" cy="10" r="7.4" />
            <path d="M10 9v4.6" />
            <path d="M10 6.3v.1" />
          {/if}
        </svg>
        {#if alert}<span class="dot"></span>{/if}
      </span>
      <span class="name">{t(e.labelKey)}</span>
    </button>
  {/each}
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    /* Une SURFACE, pas une découpe (voir `--rail` dans `global.css`). Le rail
       partageait la valeur du fond de contenu et de la barre de titre : rien
       ne le posait sur l'écran. Il monte donc d'un ton — et perd son filet
       droit du même coup, parce que relief et trait font le même travail et
       que les cumuler surcharge un bord qu'on longe en permanence. */
    background: var(--rail);
    overflow-y: auto;
    padding: 8px 0;
  }
  .spacer {
    flex: 1;
    /* Jamais moins que la respiration d'un filet : à fenêtre basse, le pied
       se colle au reste plutôt que de disparaître sous le bord. */
    min-height: 12px;
  }
  .sep {
    height: 1px;
    background: var(--line);
    margin: 7px 14px;
  }
  /* Intitulé de rang : de la STRUCTURE, pas un état — d'où le gris et non le
     rouge (barème du §7.2ter, même règle que les titres de section de la
     coquille). */
  .grp {
    color: var(--muted2);
    font-size: 8px;
    letter-spacing: 1.2px;
    text-transform: uppercase;
    text-align: center;
    padding: 2px 0 4px;
  }
  .entry {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 8px 4px;
    background: none;
    color: var(--muted);
    text-align: center;
  }
  .ic {
    position: relative;
    display: block;
    width: 20px;
    height: 20px;
    /* Le repos n'est pas éteint, il est en retrait : l'icône reste lisible,
       c'est l'écart avec l'entrée active qui porte l'information. */
    opacity: 0.72;
  }
  .ic svg {
    width: 20px;
    height: 20px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  /* **`.name` et non `.lbl` : `.lbl` est une classe GLOBALE** (une rubrique de
     formulaire, `global.css`), que le rail attrapait sans le vouloir. Elle lui
     imposait trois choses non désirées — capitales, `--muted` en dur, qui
     annulait l'éclaircissement de l'entrée active, et 8 px de marge basse. */
  .name {
    /* Le rail n'est PAS iconographique seul : « Add-ons voiture » contre
       « Compléments » n'est pas une distinction qu'une icône peut porter, et
       un rail muet se paie en infobulles pour un gain de largeur sans valeur
       ici. D'où deux lignes autorisées.
       Casse normale : c'est de la NAVIGATION, elle se lit d'un coup d'œil, et
       les capitales espacées se paient en vitesse de lecture. Elles gardent
       tout leur sens sur les titres de section, plus courts et plus rares. Un
       demi-point de plus qu'avant compense la bas-de-casse, dont la hauteur
       d'x est plus petite que celle des capitales qu'elle remplace. */
    font-size: 9.5px;
    line-height: 1.25;
    /* **Deux lignes réservées pour TOUTES les entrées**, pas seulement pour
       celles qui en ont besoin : sinon « Add-ons voiture » (deux lignes) et
       « Pilote » (une) donnent deux hauteurs de bloc, et l'espacement des
       icônes sautille sur toute la colonne. Dans un rail, c'est l'alignement
       des icônes ENTRE ELLES qu'on lit — l'inverse d'une carte, où c'est le
       haut du bloc qui compte. */
    min-height: 2.5em;
    /* Une locale peut poser un mot plus large que les 66 px utiles du rail
       (« Einstellungen ») : il se coupe plutôt qu'il ne déborde. */
    overflow-wrap: break-word;
  }
  .entry:hover {
    color: var(--txt2);
  }
  .entry:hover .ic {
    opacity: 0.9;
  }
  /* Seule apparition du rouge dans le rail — niveau 2 du barème (SPEC
     §7.2ter) : un filet gauche, pas un fond ni un libellé coloré. */
  .entry.on {
    color: var(--txt);
  }
  .entry.on .ic {
    opacity: 1;
  }
  .entry.on::before {
    content: "";
    position: absolute;
    left: 0;
    top: 5px;
    bottom: 5px;
    width: 2px;
    background: var(--rosso);
  }
  /* Pastille d'alerte : une rubrique réclame l'attention. Pas d'agrégat sur
     une entrée parente et pas de compteur — il faut voir LAQUELLE aller
     regarder, et le nombre exact ne change pas cette décision. */
  .dot {
    position: absolute;
    top: -2px;
    right: -3px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--orange);
    /* La bordure est la couleur du rail, pas une couleur en plus : elle
       détache la pastille du trait de l'icône qu'elle chevauche. */
    border: 1.5px solid var(--rail);
  }
</style>
