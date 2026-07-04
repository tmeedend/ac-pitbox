<script lang="ts">
  // Vue « Autres mods » (§6.1bis) : tout mod importé qui n'est ni voiture,
  // circuit, skin, son, ni app (shaders, configs CSP, mods d'UI…). Jamais
  // perdu. Activable par junction comme les autres types, avec le même
  // garde-fou — ce n'est PAS un moteur de superposition complet façon MO2 :
  // juste priorité notée + détection de conflits de fichiers.
  import { onMount } from "svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import {
    listOtherMods,
    setOtherPriority,
    activateOther,
    deactivateOther,
    deleteOtherMod,
    type OtherModRow,
  } from "$lib/others";

  let others = $state<OtherModRow[]>([]);
  let query = $state("");
  let busy = $state<string | null>(null);
  let error = $state("");
  let warnings = $state<Record<string, string[]>>({});

  async function load() {
    others = await listOtherMods();
  }
  onMount(load);

  function name(id: string): string {
    return others.find((o) => o.id === id)?.id ?? id;
  }

  async function toggle(o: OtherModRow) {
    busy = o.id;
    error = "";
    const { [o.id]: _drop, ...rest } = warnings;
    warnings = rest;
    try {
      if (o.is_active) {
        await deactivateOther(o.id);
      } else {
        const res = await activateOther(o.id);
        if (res.warnings.length) warnings = { ...warnings, [o.id]: res.warnings };
      }
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function togglePriority(o: OtherModRow) {
    busy = o.id;
    error = "";
    try {
      await setOtherPriority(o.id, !o.is_priority);
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function remove(o: OtherModRow) {
    const ok = await confirm(`Supprimer « ${o.id} » ? Il sera désactivé (jonctions retirées) et ses fichiers effacés.`, {
      title: "Supprimer",
      kind: "warning",
    });
    if (!ok) return;
    busy = o.id;
    error = "";
    try {
      await deleteOtherMod(o.id);
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  const filtered = $derived(
    others.filter((o) => !query.trim() || o.id.toLowerCase().includes(query.toLowerCase())),
  );
</script>

<div class="others">
  <header class="head">
    <div>
      <h2>Autres mods</h2>
      <p class="sub">
        Mods d'un type non reconnu (shaders, configs CSP, mods d'UI…), §6.1bis — jamais perdus. Activés par
        junction comme les autres types, uniquement là où AC n'a encore rien (pas de fusion de fichier isolé
        dans un dossier déjà existant). En cas d'emplacement disputé entre deux « autres mods », le mod marqué
        prioritaire l'emporte à sa prochaine activation.
      </p>
    </div>
    {#if others.length}
      <input class="input search" placeholder="Rechercher…" bind:value={query} />
    {/if}
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  {#if others.length === 0}
    <div class="empty">
      <p>Aucun mod « autre » importé.</p>
      <p class="hint">Un mod dont le type n'est reconnu par aucun import (voiture, circuit, skin, son, app) atterrit ici automatiquement.</p>
    </div>
  {:else}
    <ul class="list">
      {#each filtered as o (o.id)}
        <li class:active={o.is_active}>
          <div class="row">
            <span class="o-name mono">{o.id}</span>
            {#if o.source_archive}<span class="src mono">{o.source_archive}</span>{/if}
            {#if o.is_active}<span class="state on">actif</span>{:else}<span class="state">inactif</span>{/if}
            <button class="btn prio" class:on={o.is_priority} type="button" onclick={() => togglePriority(o)} disabled={busy === o.id} title="Marquer prioritaire : gagne en cas d'emplacement disputé">
              ★ Prioritaire
            </button>
            <button class="btn" type="button" onclick={() => toggle(o)} disabled={busy === o.id}>
              {busy === o.id ? "…" : o.is_active ? "Désactiver" : "Activer"}
            </button>
            <button class="btn del" type="button" title="Supprimer" onclick={() => remove(o)} disabled={busy === o.id}>✕</button>
          </div>
          {#if o.conflicts.length}
            <div class="conflicts">
              ⚠ Fichiers en commun avec {#each o.conflicts as c, i}{i > 0 ? ", " : ""}<b>{name(c.other_id)}</b> ({c.count}){/each}
            </div>
          {/if}
          {#if warnings[o.id]?.length}
            <ul class="warn-list">
              {#each warnings[o.id] as w}<li>{w}</li>{/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .others {
    max-width: 820px;
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
    max-width: 620px;
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
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 9px 12px;
  }
  .list li.active {
    border-left: 3px solid var(--green-border);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .o-name {
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
  .btn.prio {
    color: var(--muted);
  }
  .btn.prio.on {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .btn.del {
    padding: 6px 9px;
    color: var(--muted);
  }
  .btn.del:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .conflicts {
    margin-top: 7px;
    font-size: 11px;
    color: var(--yellow);
  }
  .warn-list {
    margin-top: 6px;
    padding-left: 16px;
    font-size: 11px;
    color: var(--muted);
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
