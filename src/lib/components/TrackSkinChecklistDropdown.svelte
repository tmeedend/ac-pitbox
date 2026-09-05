<script lang="ts">
  // Liste déroulante à choix multiple pour les skins de circuit (§8) —
  // plusieurs actifs à la fois, comme le fait Content Manager lui-même dans
  // son propre sélecteur. Reste ouverte tant qu'on coche/décoche.
  import { t } from "$lib/i18n/index.svelte";

  interface TrackSkinOption {
    name: string;
    image: string | null;
    active: boolean;
  }

  interface Props {
    options: TrackSkinOption[];
    busy?: boolean;
    ontoggle: (name: string, active: boolean) => void;
    /** Intitulé du champ, en colonne (SPEC §9.1) — largeur partagée par
     * `--sess-lblw` avec les autres champs de la colonne de session. */
    label?: string;
  }
  let { options, busy = false, ontoggle, label }: Props = $props();

  let open = $state(false);
  let root = $state<HTMLDivElement | undefined>(undefined);

  const activeCount = $derived(options.filter((o) => o.active).length);
  /** **L'état vide est une valeur, pas un intitulé.** « Aucun skin actif » se
   * nommait parfaitement lui-même tant qu'il était vide, et l'information de
   * nature disparaissait dès qu'un skin était coché. Le défaut se dit donc
   * comme partout ailleurs dans le produit — un possessif en italique grise
   * (« Celui d'origine », comme « Celle de la livrée » de l'écran Pilote),
   * jamais une négation. */
  const valueText = $derived.by(() => {
    if (activeCount === 0) return t("session.trackSkinsStock");
    if (activeCount === 1) return options.find((o) => o.active)!.name;
    return t("session.trackSkinsCount", { count: activeCount });
  });
  /** Aucun skin livré avec le circuit : rien à choisir, donc pas de contrôle —
   * mais la ligne reste, elle dit ce que la session utilisera. */
  const isStatic = $derived(options.length === 0);

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    if (options.length) open = !open;
  }

  function onDocClick(e: MouseEvent) {
    if (root && !root.contains(e.target as Node)) open = false;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }

  $effect(() => {
    if (!open) return;
    document.addEventListener("click", onDocClick);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("click", onDocClick);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="isd" bind:this={root}>
  {#if isStatic}
    <div class="isd-static">
      {#if label}<span class="isd-k">{label}</span>{/if}
      <span class="isd-name stock">{t("session.trackSkinsStock")}</span>
    </div>
  {:else}
  <button
    class="isd-trigger"
    class:labelled={label != null}
    type="button"
    onclick={toggle}
    title={t("session.trackSkinsTooltip")}
  >
    {#if label}<span class="isd-k">{label}</span>{/if}
    <span class="isd-name" class:stock={activeCount === 0}>{valueText}</span>
    <span class="isd-caret">▾</span>
  </button>
  {#if open}
    <ul class="isd-list">
      {#each options as o (o.name)}
        <li>
          <label class:on={o.active}>
            <input
              type="checkbox"
              checked={o.active}
              disabled={busy}
              onchange={() => ontoggle(o.name, !o.active)}
            />
            <span class="isd-thumb">
              {#if o.image}<img src={o.image} alt="" />{:else}<span class="isd-noimg"></span>{/if}
            </span>
            <span class="isd-name">{o.name}</span>
          </label>
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
  /* Survol neutre, comme `ImageSelectDropdown` : contrôle secondaire (SPEC §7.2ter). */
  .isd-trigger:hover:not(:disabled) {
    border-color: var(--faint2);
  }
  .isd-trigger.labelled {
    height: 30px;
    padding: 0 9px;
    gap: 9px;
  }
  /* Mêmes valeurs que `ImageSelectDropdown` : les deux composants posent des
     lignes de la même colonne, elles doivent s'aligner au pixel. */
  .isd-k {
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
  .isd-static {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 26px;
    padding: 0 9px;
    font-size: 11px;
  }
  /* Valeur par défaut : italique grise, jamais une négation. */
  .isd-name.stock {
    color: var(--muted);
    font-style: italic;
  }
  .isd-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--txt);
  }
  .isd-caret {
    flex: none;
    color: var(--faint);
    font-size: 9px;
  }
  .isd-list {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 50;
    list-style: none;
    max-height: 260px;
    overflow-y: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.4);
  }
  .isd-list li + li {
    border-top: 1px solid var(--line);
  }
  .isd-list label {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    font-size: 11px;
    color: var(--txt2);
    cursor: pointer;
  }
  .isd-list label:hover {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .isd-list label.on {
    color: var(--rosso-bright);
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
  .isd-noimg {
    width: 100%;
    height: 100%;
    background: var(--raised);
  }
</style>
