<script lang="ts">
  // Habillages de circuit : deuxième carte de la colonne de données, et non
  // une rangée à part sous la fiche. C'est tout l'argument de l'écran 7 —
  // sans fiche technique ni courbe, la colonne droite a la place, et une
  // rangée basse d'une carte et demie rouvrait précisément le trou qu'on
  // venait de fermer.
  //
  // The page owns the list and the toggle: it reloads them when the library
  // changes, which this card, unmounted on every tab switch, could not do.
  import type { SubModRow } from "$lib/inventory/submods";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    skins: SubModRow[];
    /** Names of the active track skins. */
    active: string[];
    loading: boolean;
    busy: boolean;
    ontoggle: (name: string) => void;
  }
  let { skins, active, loading, busy, ontoggle }: Props = $props();
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.trackSkinsLabelPlain")}</span>
    {#if !loading}<span class="blk-n">{skins.length}</span>{/if}
  </header>
  <div class="blk-b">
    {#if loading}
      <div class="muted small loading-inline"><span class="spinner-sm"></span>{t("common.loading")}</div>
    {:else if skins.length}
      <ul class="tsk-list">
        {#each skins as sk (sk.id)}
          {@const on = active.includes(sk.name)}
          <li class:inactive={!on}>
            <label class="tog" title={on ? t("detail.trackSkinActiveOn") : t("detail.trackSkinActiveOff")}>
              <input type="checkbox" checked={on} disabled={busy} onchange={() => ontoggle(sk.name)} />
            </label>
            <span class="tsk-name">{sk.name}</span>
            {#if sk.source_archive}<span class="tsk-src mono">{sk.source_archive}</span>{/if}
          </li>
        {/each}
      </ul>
      <div class="muted small">{t("detail.trackSkinsNote")}</div>
    {:else}
      <div class="muted small">{t("detail.noTrackSkins")}</div>
    {/if}
  </div>
</section>

<style>
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 11px;
  }

  .tsk-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 6px;
  }
  .tsk-list li {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 5px 9px;
  }
  .tsk-list li.inactive {
    opacity: 0.6;
  }
  .tsk-list .tog {
    flex: none;
    display: flex;
    align-items: center;
    cursor: pointer;
  }
  .tsk-name {
    flex: 1;
    font-size: 11px;
    color: var(--txt2);
  }
  .tsk-src {
    font-size: 9px;
    color: var(--muted2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 120px;
  }
  .loading-inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .spinner-sm {
    flex: none;
    width: 12px;
    height: 12px;
    border: 2px solid var(--line);
    border-top-color: var(--rosso);
    border-radius: 50%;
    animation: tsk-spin 0.8s linear infinite;
  }
  @keyframes tsk-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
