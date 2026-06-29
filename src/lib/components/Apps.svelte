<script lang="ts">
  // Vue Apps (§12bis.4) : type autonome, simplement activable/désactivable par
  // junction. Pas de fiche ni de tags en v1 — nom, état, activation.
  import { onMount } from "svelte";
  import { listApps, activateApp, deactivateApp, deleteApp, type AppItem } from "$lib/apps";
  import { confirm } from "@tauri-apps/plugin-dialog";

  let apps = $state<AppItem[]>([]);
  let query = $state("");
  let busy = $state<string | null>(null);
  let error = $state("");

  async function load() {
    apps = await listApps();
  }
  onMount(load);

  async function toggle(a: AppItem) {
    busy = a.id;
    error = "";
    try {
      if (a.active) await deactivateApp(a.id);
      else await activateApp(a.id);
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function remove(a: AppItem) {
    const ok = await confirm(`Supprimer l'app « ${a.id} » ? Elle sera désactivée et ses fichiers retirés.`, {
      title: "Supprimer",
      kind: "warning",
    });
    if (!ok) return;
    busy = a.id;
    error = "";
    try {
      await deleteApp(a.id);
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  const filtered = $derived(
    apps.filter((a) => !query.trim() || a.id.toLowerCase().includes(query.toLowerCase())),
  );
</script>

<div class="apps">
  <header class="head">
    <div>
      <h2>Apps</h2>
      <p class="sub">Apps Python d'Assetto Corsa (§12bis.4) — type autonome, activées/désactivées par junction comme le reste.</p>
    </div>
    {#if apps.length}
      <input class="input search" placeholder="Rechercher…" bind:value={query} />
    {/if}
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  {#if apps.length === 0}
    <div class="empty">
      <p>Aucune app importée.</p>
      <p class="hint">Importe une app via l'import général (dossier <span class="mono">apps/python/&lt;App&gt;/</span>).</p>
    </div>
  {:else}
    <ul class="list">
      {#each filtered as a (a.id)}
        <li class:active={a.active}>
          <span class="a-name mono">{a.id}</span>
          {#if a.source_archive}<span class="src mono">{a.source_archive}</span>{/if}
          {#if a.active}<span class="state on">actif</span>{:else}<span class="state">inactif</span>{/if}
          <button class="btn" type="button" onclick={() => toggle(a)} disabled={busy === a.id}>
            {busy === a.id ? "…" : a.active ? "Désactiver" : "Activer"}
          </button>
          <button class="btn del" type="button" title="Supprimer" onclick={() => remove(a)} disabled={busy === a.id}>✕</button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .apps {
    max-width: 760px;
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
    margin-bottom: 18px;
  }
  h2 {
    font-size: 18px;
    font-weight: 600;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
    max-width: 520px;
  }
  .search {
    width: 220px;
    flex: none;
  }
  .err {
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    padding: 10px 12px;
    font-size: 12px;
    margin-bottom: 14px;
  }
  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 12px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 9px 12px;
  }
  .list li.active {
    border-left: 3px solid var(--green-border);
  }
  .a-name {
    flex: 1;
    font-weight: 600;
    color: var(--txt);
    font-size: 12.5px;
  }
  .src {
    color: var(--muted2);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }
  .state {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
  }
  .state.on {
    color: var(--green);
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 12px;
    flex: none;
  }
  .btn:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .btn.del {
    padding: 6px 9px;
    color: var(--muted);
  }
  .btn.del:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .empty {
    color: var(--muted);
    text-align: center;
    padding: 50px 0;
  }
  .empty .hint {
    font-size: 12px;
    color: var(--faint);
    margin-top: 8px;
  }
</style>
