<script lang="ts">
  // Navigation rail (SPEC §7.2): the first of the three territories. The rail
  // carries what does not belong to the session; the session column IS the
  // session, and its parts are destinations in their own right (track card,
  // car card, driver line, session types) — which is why the rail has a single
  // `Session` entry for all of them. The title bar carries the shape of the
  // window. The split must stay watertight: any new entry belongs to one of
  // the three.
  //
  // Apps and Extras are entries of their own, not tabs of an "Add-ons" screen:
  // they ALREADY have their own facets or tabs, and grouping them would stack
  // two identical rows of horizontal tabs with nothing to say which one drives
  // the other. The Workshop, on the contrary, gathers tools that have no
  // sub-section — the only reason that grouping is legitimate.
  import { nav, requestSection, openSessionZone, SESSION_ZONE } from "$lib/shell/nav.svelte";
  import { openContentManager } from "$lib/launch/launch";
  import { bigPictureState, exitBigPicture } from "$lib/shell/bigpicture.svelte";
  import { t } from "$lib/i18n/index.svelte";

  type Entry = {
    /** Section ouverte par un clic. */
    target: string;
    labelKey: string;
    /** Sections that make the entry active — the session covers four, the
     * Workshop seven. */
    sections?: readonly string[];
    /** Filet de séparation AVANT cette entrée. */
    sep?: boolean;
    /** Pousse l'entrée (et ses suivantes) en pied de rail. */
    foot?: boolean;
    /** The entry opens no screen but **leaves** — the application (Content
     * Manager) or Big Picture mode. It is therefore never active and carries
     * no `aria-current`. */
    exit?: () => void;
    /** Shown only while this holds: a way out toward something that is not
     * there has no business taking a line. */
    when?: () => boolean;
  };

  let {
    alerts = {},
    cmAvailable = false,
  }: {
    /** Rubriques qui réclament l'attention, par section. Pas de compteur : le
     * nombre exact ne change pas la décision d'aller voir. */
    alerts?: Record<string, boolean>;
    /** Content Manager détecté au chemin configuré (`validate_config`). */
    cmAvailable?: boolean;
  } = $props();

  const ENTRIES: Entry[] = [
    // **One entry for the whole session zone** (SPEC §7.2), where there were
    // three — Tracks, Cars, Driver — doing exactly what clicking the matching
    // card of the session column does, with a third of its information. It
    // returns to the last library consulted and stays active on every screen
    // of the zone; which one is on display, the column's cards say. Alone
    // above the first rule, it needs no group title: the rule carries the
    // split between the session zone and the installation screens.
    { target: "session", labelKey: "nav.session", sections: SESSION_ZONE },
    // Les deux écrans d'add-ons ont disparu : ils classaient par mécanique de
    // pose, et leur contenu est dans l'inventaire (REFONTE§3.1).
    { target: "apps", labelKey: "nav.apps", sep: true },
    { target: "others", labelKey: "nav.others" },
    { target: "rules", labelKey: "nav.atelier", sections: ["rules", "brands", "categories", "countries", "import", "profiles", "maintenance"] },
    // Second filet : détache le pied.
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
    // Content Manager absent, l'entrée n'est pas rendue du tout — ni grisée,
    // ni suivie d'un message d'erreur au clic.
    {
      target: "cm",
      labelKey: "nav.openCmShort",
      exit: () => void openContentManager().catch((err) => console.error(err)),
      when: () => cmAvailable,
    },
    { target: "about", labelKey: "nav.about" },
    // **The way out of Big Picture** (full screen, no OS chrome, no title
    // bar). It lived at the foot of the session column, which is now hidden
    // outside the session zone: on Apps or Settings, Esc would have been the
    // only way out left. The rail is on every screen, and its foot already
    // holds what is not a place.
    {
      target: "bigpicture",
      labelKey: "nav.exitBigPicture",
      exit: () => void exitBigPicture(),
      when: () => bigPictureState.active,
    },
  ];

  const shown = $derived(ENTRIES.filter((e) => !e.when || e.when()));

  function isActive(e: Entry): boolean {
    if (e.exit) return false;
    return (e.sections ?? [e.target]).includes(nav.section);
  }

  function activate(e: Entry) {
    if (e.exit) e.exit();
    else if (e.target === "session") void openSessionZone();
    else void requestSection(e.target);
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
          {#if e.target === "session"}
            <!-- Le drapeau à damier : ce qu'on prépare ici part en piste. -->
            <path d="M4.5 17.5V2.8" />
            <path d="M4.5 3.3h11.5v8.4H4.5" />
            <path d="M8.3 3.3v8.4M12.2 3.3v8.4M4.5 7.5h11.5" />
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
          {:else if e.target === "bigpicture"}
            <!-- Quatre coins qui rentrent : on quitte le plein écran. -->
            <path d="M7.5 3v4.5H3M12.5 3v4.5H17M7.5 17v-4.5H3M12.5 17v-4.5H17" />
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
       « Apps » (une) donnent deux hauteurs de bloc, et l'espacement des
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
