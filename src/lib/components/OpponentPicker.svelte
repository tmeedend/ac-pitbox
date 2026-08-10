<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { listModSkins, type SkinItem } from "$lib/launch";
  import { previewSrc, type ModCard } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    /** Vivier déjà filtré par le mode de plateau courant (§8.6). En mode
     * « même voiture » ce vivier ne contient que la voiture du joueur — seul
     * le skin est alors modifiable. */
    pool: ModCard[];
    currentCarId: string;
    currentSkinId: string | null;
    onpick: (carId: string, skinId: string | null) => void;
    onclose: () => void;
  }
  let { pool, currentCarId, currentSkinId, onpick, onclose }: Props = $props();

  // Sélection éditable localement, initialisée depuis la sélection courante :
  // le composant est recréé à chaque ouverture, `untrack` documente que la
  // capture n'est voulue qu'une fois, pas suivie en continu.
  let selectedCarId = $state(untrack(() => currentCarId));
  let selectedSkinId = $state<string | null>(untrack(() => currentSkinId));
  let skins = $state<SkinItem[]>([]);
  let loading = $state(false);
  let query = $state("");

  const filteredPool = $derived(
    query.trim()
      ? pool.filter((c) => (c.display_name ?? c.id_interne).toLowerCase().includes(query.trim().toLowerCase()))
      : pool,
  );
  const selectedCar = $derived(pool.find((c) => c.id_interne === selectedCarId) ?? null);

  async function loadSkins(carId: string) {
    loading = true;
    try {
      skins = await listModSkins(carId);
    } catch {
      skins = [];
    } finally {
      loading = false;
    }
  }

  onMount(() => loadSkins(selectedCarId));

  function selectCar(id: string) {
    if (id === selectedCarId) return;
    selectedCarId = id;
    selectedSkinId = null;
    loadSkins(id);
  }

  function confirm() {
    onpick(selectedCarId, selectedSkinId);
  }
</script>

<div class="backdrop">
  <div class="modal">
    <header>
      <h2>{t("launch.opponentPickTitle")}</h2>
      <button class="btn btn-ghost" type="button" onclick={onclose}>✕</button>
    </header>

    <div class="body" class:solo={pool.length <= 1}>
      {#if pool.length > 1}
        <div class="cars">
          <input class="input" type="search" placeholder={t("launch.opponentPickSearch")} bind:value={query} />
          <div class="car-list">
            {#each filteredPool as c (c.id_interne)}
              <button class="car-row" class:on={c.id_interne === selectedCarId} type="button" onclick={() => selectCar(c.id_interne)}>
                {#if previewSrc(c.preview)}<img src={previewSrc(c.preview)} alt="" />{:else}<span class="car-noimg">🏎</span>{/if}
                <span class="car-name">{c.display_name ?? c.id_interne}</span>
              </button>
            {/each}
            {#if !filteredPool.length}<div class="empty">{t("launch.opponentPickEmpty")}</div>{/if}
          </div>
        </div>
      {/if}

      <div class="skins">
        <div class="lbl skin-lbl">{t("launch.opponentPickSkin")}{#if selectedCar} — {selectedCar.display_name ?? selectedCar.id_interne}{/if}</div>
        {#if loading}
          <div class="empty">{t("common.loading")}</div>
        {:else if !skins.length}
          <div class="empty">{t("launch.opponentPickNoSkin")}</div>
        {:else}
          <div class="skin-grid">
            {#each skins as sk (sk.id)}
              <button class="skin" class:on={sk.id === selectedSkinId} type="button" onclick={() => (selectedSkinId = sk.id)}>
                <div class="skin-img">
                  {#if previewSrc(sk.preview)}<img src={previewSrc(sk.preview)} alt={sk.name} />{:else}<span class="skin-noimg">▦</span>{/if}
                </div>
                <div class="skin-name">{sk.name}</div>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <footer>
      <button class="btn" type="button" onclick={onclose}>{t("common.cancel")}</button>
      <button class="btn btn-primary" type="button" onclick={confirm}>{t("launch.opponentPickConfirm")}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 640px;
    max-width: 92vw;
    max-height: 84vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    font-size: 13px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--txt2);
  }
  .body {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 14px;
    padding: 14px 16px;
    overflow: hidden;
    flex: 1;
    min-height: 0;
  }
  .body.solo {
    grid-template-columns: 1fr;
  }
  .cars {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .car-list {
    overflow-y: auto;
    border: 1px solid var(--line);
  }
  .car-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    background: var(--panel2);
    border-bottom: 1px solid var(--line);
    text-align: left;
    font-size: 11px;
    color: var(--txt2);
  }
  .car-row:hover {
    background: var(--raised);
  }
  .car-row.on {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .car-row img {
    width: 30px;
    height: 20px;
    object-fit: cover;
    flex: none;
    background: var(--bg);
  }
  .car-noimg {
    width: 30px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
  }
  .car-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .skins {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow-y: auto;
  }
  /* Couleur/taille/interlettrage viennent de `.lbl` (global, harmonisation
     §chantier libellés) : ne reste ici que ce que `.lbl` ne couvre pas. */
  .skin-lbl {
    flex: none;
  }
  .skin-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    align-content: start;
  }
  .skin {
    background: var(--card);
    padding: 0;
    text-align: left;
  }
  .skin.on {
    outline: 2px solid var(--rosso);
    outline-offset: -2px;
  }
  .skin-img {
    aspect-ratio: 16 / 9;
    display: flex;
    align-items: center;
    justify-content: center;
    border-bottom: 1px solid var(--line);
    overflow: hidden;
    background: var(--bg);
  }
  .skin-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .skin-noimg {
    color: var(--faint);
    font-size: 16px;
  }
  .skin-name {
    padding: 5px 7px;
    font-size: 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .empty {
    padding: 12px;
    color: var(--faint);
    font-size: 11px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 12px 16px;
    border-top: 1px solid var(--line);
  }
</style>
