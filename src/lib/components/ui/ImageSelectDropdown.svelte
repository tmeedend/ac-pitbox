<script lang="ts">
  // Liste déroulante compacte à choix unique, chaque entrée avec une
  // miniature (skin voiture, layout circuit…) — un <select> natif ne peut
  // pas afficher d'image par option, d'où ce composant maison.
  import { tick } from "svelte";
  import { zoomFactor } from "$lib/shell/zoom.svelte";

  interface ImageOption {
    id: string;
    name: string;
    image: string | null;
  }

  interface Props {
    options: ImageOption[];
    selectedId: string | null;
    placeholder: string;
    emptyText: string;
    onselect: (id: string) => void;
    /** "contain" pour un tracé (forme complète, pas de recadrage) — défaut "cover" pour une photo/skin. */
    fit?: "cover" | "contain";
    /** Intitulé du champ, en colonne à gauche de la valeur (SPEC SESSION§1). Sa
     * largeur est partagée par tous les champs de la colonne de session via
     * `--sess-lblw` : c'est l'alignement des valeurs qui fait tout l'intérêt
     * du dispositif, et il disparaît si chaque ligne se dimensionne seule. */
    label?: string;
    /** Un sélecteur à une seule option n'en est pas un : il devient une ligne
     * statique — même colonne d'intitulé, ni bordure ni chevron. La ligne
     * n'est PAS masquée pour autant : elle dit ce que la session utilisera,
     * c'est le contrôle qui disparaît, pas le fait. */
    staticWhenSingle?: boolean;
    /** Mention accolée à la valeur d'une ligne statique à une seule option
     * (« Imola — tracé unique ») : elle explique pourquoi il n'y a rien à
     * choisir, là où le nom seul laisserait croire à un menu cassé. */
    singleNote?: string;
  }
  let {
    options,
    selectedId,
    placeholder,
    emptyText,
    onselect,
    fit = "cover",
    label,
    staticWhenSingle = false,
    singleNote,
  }: Props = $props();

  let open = $state(false);
  let root = $state<HTMLDivElement | undefined>(undefined);
  let triggerEl = $state<HTMLButtonElement | undefined>(undefined);
  let listEl = $state<HTMLUListElement | undefined>(undefined);

  const selected = $derived(options.find((o) => o.id === selectedId) ?? null);
  const valueText = $derived(selected?.name ?? (options.length ? placeholder : emptyText));
  const isStatic = $derived(staticWhenSingle && options.length < 2);
  /** Ce que dit la ligne statique : l'option unique — c'est bien celle que la
   * session utilisera, choisie ou non — sinon l'état vide. */
  const staticText = $derived.by(() => {
    const only = options[0];
    if (!only) return emptyText;
    return singleNote ? `${only.name} — ${singleNote}` : only.name;
  });

  // Position de la liste ouverte, en `position: fixed` (voir plus bas pour le
  // pourquoi) : calculée à l'ouverture depuis le déclencheur, puis resserrée
  // contre le bord droit de la fenêtre une fois la liste rendue et sa largeur
  // réelle connue (elle grandit avec son plus long libellé, `width:
  // max-content`, jusqu'à ce que ça ne tienne plus).
  let listStyle = $state("");
  /** Même valeur que le `max-height` de `.isd-list` : le calcul de place ne
   * fait que la réduire, jamais l'augmenter. */
  const LIST_MAX_HEIGHT = 260;

  function positionList() {
    if (!triggerEl) return;
    // **Tout est ramené en pixels CSS avant d'être écrit** : la mesure est en
    // pixels réels, et un `position: fixed` sous un `<html>` zoomé
    // remultiplierait par le même facteur (voir `zoomFactor`). Sans ça, la
    // liste s'ouvrait très en dessous de son bouton — d'autant plus bas que le
    // bouton était bas dans la fenêtre, jusqu'à sortir de l'écran par le bas
    // sur la tenue par défaut (bug réel, à 110 % comme à 125 %).
    const f = zoomFactor();
    const r = triggerEl.getBoundingClientRect();
    const top = r.bottom / f + 4;
    listStyle = `top:${top}px; left:${r.left / f}px; min-width:${r.width / f}px;`;
  }

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    if (!options.length) return;
    open = !open;
  }

  function pick(o: ImageOption) {
    open = false;
    onselect(o.id);
  }

  function onDocClick(e: MouseEvent) {
    // La liste est `position: fixed` mais reste un enfant DOM de `.isd`
    // (voir plus bas) : `root.contains` couvre donc déjà un clic dedans.
    if (root && !root.contains(e.target as Node)) open = false;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }

  $effect(() => {
    if (!open) {
      listStyle = "";
      return;
    }
    positionList();
    document.addEventListener("click", onDocClick);
    document.addEventListener("keydown", onKey);
    // La liste est `position: fixed` : elle ne suit pas le défilement de la
    // barre latérale (`.side`, `overflow-y: auto`) ni un redimensionnement de
    // la fenêtre. Plutôt que la faire dériver du déclencheur, on la ferme —
    // même geste qu'un clic à l'extérieur. `capture: true` sur le scroll :
    // l'événement ne remonte pas (bulle), seule la phase de capture le voit
    // depuis `window` quel que soit le conteneur qui défile réellement.
    document.addEventListener("scroll", closeOnScroll, true);
    window.addEventListener("resize", closeOnScroll);
    return () => {
      document.removeEventListener("click", onDocClick);
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("scroll", closeOnScroll, true);
      window.removeEventListener("resize", closeOnScroll);
    };
  });

  function closeOnScroll(e: Event) {
    // Exception nécessaire : la liste elle-même défile (`overflow-y: auto`,
    // beaucoup d'options) et ce scroll-là passe aussi par la capture sur
    // `document`. Sans cette garde, faire défiler la liste la refermait
    // avant qu'on ait pu cliquer quoi que ce soit.
    if (listEl && e.target instanceof Node && listEl.contains(e.target)) return;
    open = false;
  }

  // Une fois la liste rendue, sa largeur réelle (`max-content`) est connue :
  // si elle déborde le bord droit de la fenêtre, on la recale plutôt que de
  // la laisser rognée — c'est exactement le problème que `position: fixed`
  // règle pour le bord gauche (échapper au clip de `.side`), il ne doit pas
  // réapparaître à droite pour un libellé assez long.
  $effect(() => {
    if (!open || !listEl) return;
    tick().then(() => {
      if (!listEl || !triggerEl) return;
      const margin = 8;
      const f = zoomFactor();
      const r = listEl.getBoundingClientRect();
      if (r.right > window.innerWidth - margin) {
        const shifted = Math.max(margin, (window.innerWidth - r.width) / f - margin);
        listStyle = listStyle.replace(/left:[^;]+;/, `left:${shifted}px;`);
      }
      // **Et le même soin en bas**, qui manquait : un sélecteur posé en pied de
      // panneau (la tenue par défaut) ouvrait une liste tronquée par le bord de
      // la fenêtre, sans rien pour atteindre le reste. Elle bascule donc
      // au-dessus du bouton quand la place manque dessous, et se contente de la
      // hauteur disponible si elle manque des deux côtés — sa propre barre de
      // défilement fait le reste.
      const trigger = triggerEl.getBoundingClientRect();
      const below = window.innerHeight - trigger.bottom - margin;
      const above = trigger.top - margin;
      // Le plafond de la feuille de style reste la hauteur voulue ; ce calcul
      // ne fait que la réduire quand la fenêtre est plus courte.
      const cap = (available: number) => `${Math.min(LIST_MAX_HEIGHT, Math.max(available, 0) / f)}px`;
      if (r.height > below && above > below) {
        const top = (trigger.top - Math.min(r.height, above)) / f - 4;
        listStyle = listStyle.replace(/top:[^;]+;/, `top:${top}px;`);
        listEl.style.maxHeight = cap(above);
      } else {
        listEl.style.maxHeight = cap(below);
      }
    });
  });
</script>

<div class="isd" bind:this={root}>
  {#if isStatic}
    <div class="isd-static">
      {#if label}<span class="isd-k">{label}</span>{/if}
      <span class="isd-name muted">{staticText}</span>
    </div>
  {:else}
  <button
    bind:this={triggerEl}
    class="isd-trigger"
    class:labelled={label != null}
    type="button"
    onclick={toggle}
    disabled={options.length === 0}
  >
    {#if label}<span class="isd-k">{label}</span>{/if}
    <span class="isd-thumb" class:contain={fit === "contain"}>
      {#if selected?.image}<img src={selected.image} alt="" />{:else}<span class="isd-noimg"></span>{/if}
    </span>
    <span class="isd-name" class:muted={!selected}>{valueText}</span>
    <span class="isd-caret">▾</span>
  </button>
  <!-- Info-bulle maison plutôt que `Tooltip.svelte` : celui-ci enveloppe son
       déclencheur dans un `inline-flex` qui casse le `width: 100%` du
       bouton ici. Même mécanique (`:hover`/`:focus`, pas de JS), et le
       sélecteur de sœur exige que ce span suive `.isd-trigger` dans le DOM.
       But : le nom complet du libellé sélectionné (survol souris ou focus
       manette, `gamepadNav.ts` posant un vrai focus DOM) là où le libellé du
       déclencheur, lui, est tronqué (`.isd-name`, `text-overflow: ellipsis`)
       pour ne pas pousser le reste de la barre latérale en largeur. Absente
       tant que la liste est ouverte — juste en dessous, à quasi la même
       position, elle ferait doublon avec elle (qui montre déjà les noms en
       entier) et se chevaucherait visuellement. -->
  {#if !open}
    <span class="isd-tt" role="tooltip">{valueText}</span>
  {/if}
  {#if open}
    <ul class="isd-list" bind:this={listEl} style={listStyle}>
      {#each options as o (o.id)}
        <li>
          <button type="button" class:on={o.id === selectedId} onclick={() => pick(o)}>
            <span class="isd-thumb" class:contain={fit === "contain"}>
              {#if o.image}<img src={o.image} alt="" />{:else}<span class="isd-noimg"></span>{/if}
            </span>
            <span class="isd-name">{o.name}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  {/if}
</div>

<style>
  .isd {
    position: relative;
  }
  .isd-trigger {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt2);
    padding: 5px 8px;
    font-size: 11px;
    text-align: left;
  }
  /* Version « champ nommé » de la colonne de session : hauteur fixe pour que
     les lignes s'alignent, et une vignette qui prend **toute la hauteur de la
     ligne**.
     Elle a longtemps été réduite au rôle de pastille de couleur (13 px, « ce
     qu'on lit d'un skin c'est sa teinte, pas son motif ») — sauf qu'une
     livrée, un tracé et un pilote sont justement ce qu'on cherche à
     reconnaître d'un coup d'œil, et à 13 px aucun des trois ne se
     reconnaissait : signalé à l'usage. 28 px = les 30 px de la ligne moins ses
     deux filets, donc le maximum à hauteur de ligne inchangée — c'est
     l'alignement des valeurs qui coûterait cher, pas les pixels d'image.
     Pas de cadre à elle : à pleine hauteur, il doublerait celui de la ligne à
     un pixel de distance.
     `gap: 8px` et non 9 : même gouttière que `.field` d'`AppShell`, dont les
     lignes voisinent les nôtres dans la même colonne. L'écart valait 2 px sur
     le début de la valeur, et il se voit d'autant mieux que les vignettes sont
     maintenant larges. */
  .isd-trigger.labelled {
    height: 30px;
    padding: 0 9px;
    gap: 8px;
  }
  .isd-trigger.labelled .isd-thumb {
    width: 28px;
    height: 28px;
    border: 0;
  }
  /* La gouttière de 2 px protégeait le tracé d'un cadre qui n'existe plus
     ici : elle lui coûterait un septième de sa hauteur pour rien. */
  .isd-trigger.labelled .isd-thumb.contain img {
    padding: 0;
  }
  /* Le libellé ne bouge jamais, la valeur change : c'est toute la règle. Un
     intitulé qui se nomme lui-même (« Aucun skin de circuit ») disparaît dès
     que la valeur est renseignée, et l'utilisateur perd la nature du champ. */
  .isd-k {
    flex: 0 0 var(--sess-lblw, 60px);
    /* Plafond de la colonne : au-delà, l'intitulé tronque plutôt que de
       manger la valeur (une locale peut être bien plus longue — LACKIERUNG). */
    max-width: 88px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 8.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .isd-static {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 26px;
    padding: 0 9px;
    font-size: 11px;
  }
  .isd-static .isd-name {
    color: var(--muted);
  }
  /* Survol neutre : un menu de livrée est un contrôle secondaire, il n'a pas
     droit au rouge au repos, donc pas davantage sous le curseur (SPEC
     §7.2ter). */
  .isd-trigger:hover:not(:disabled) {
    border-color: var(--faint2);
  }
  .isd-trigger:disabled {
    opacity: 0.5;
  }
  .isd-thumb {
    flex: none;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .isd-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .isd-thumb.contain img {
    object-fit: contain;
    padding: 2px;
  }
  .isd-noimg {
    width: 100%;
    height: 100%;
    background: var(--raised);
  }
  .isd-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--txt);
  }
  .isd-name.muted {
    color: var(--muted);
  }
  .isd-caret {
    flex: none;
    color: var(--faint);
    font-size: 9px;
  }
  /* Repris de `Tooltip.svelte` (bulle alignée à gauche, largeur limitée,
     texte qui s'enroule) : même clipping en jeu ici, la barre latérale
     (`.side`, `overflow-x` calculé à `auto` dès que `overflow-y: auto` est
     posé) rognerait une bulle plus large qu'elle plutôt que de la montrer. */
  .isd-tt {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 60;
    width: max-content;
    max-width: 200px;
    background: var(--panel);
    border: 1px solid var(--rosso-border);
    color: var(--txt2);
    font-size: 11px;
    font-weight: 400;
    line-height: 1.5;
    padding: 8px 10px;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.5);
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition: opacity 0.12s;
  }
  .isd-trigger:hover ~ .isd-tt,
  .isd-trigger:focus ~ .isd-tt {
    opacity: 1;
    visibility: visible;
  }
  /* `position: fixed`, pas `absolute` : `.side` (barre latérale) calcule son
     `overflow-x` à `auto` dès qu'on lui pose `overflow-y: auto` (règle CSS —
     un seul axe visible ne reste pas "visible" si l'autre ne l'est pas), donc
     tout ce qui dépasse sa largeur y est rogné, y compris un `position:
     absolute` qui déborderait sur la zone centrale. `fixed` échappe à ce
     clip et se positionne par rapport à la fenêtre (calculé en JS depuis le
     déclencheur, `positionList()`) — aucun ancêtre ici ne pose `transform`/
     `filter`/`will-change`, ce qui aurait recréé un cadre de référence local
     et annulé l'échappée. Les boutons qu'elle contient restent bien trouvés
     par la navigation manette (`gamepadNav.ts`, `el.offsetParent !== null`) :
     un élément `position: fixed` EST « positionné » au sens de cette API,
     donc il sert d'`offsetParent` à ses propres enfants — seul l'élément
     fixe lui-même aurait un `offsetParent` nul, pas ce qu'il contient.
     `width: max-content` plutôt que `left: 0; right: 0` (l'ancienne largeur,
     calée sur le déclencheur) : c'est justement ce qui tronquait un long nom
     de layout, dans la liste comme dans le déclencheur. `min-width` la garde
     au moins aussi large que lui. */
  .isd-list {
    position: fixed;
    z-index: 200;
    list-style: none;
    width: max-content;
    max-width: min(420px, calc(100vw - 16px));
    max-height: 260px;
    overflow-y: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.4);
  }
  .isd-list li + li {
    border-top: 1px solid var(--line);
  }
  .isd-list button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    border: none;
    color: var(--txt2);
    padding: 6px 8px;
    font-size: 11px;
    text-align: left;
    cursor: pointer;
  }
  /* Ici, `.isd-name` ne doit plus tronquer : la liste est maintenant assez
     large pour son plus long libellé (voir `.isd-list` ci-dessus), donc rien
     ne doit plus le faire passer par l'ellipse — seul le déclencheur, resté
     à la largeur de la barre latérale, en a encore besoin. */
  .isd-list .isd-name {
    overflow: visible;
    white-space: normal;
  }
  .isd-list button:hover {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .isd-list button.on {
    color: var(--rosso-bright);
    background: var(--rosso-dim);
  }
</style>
