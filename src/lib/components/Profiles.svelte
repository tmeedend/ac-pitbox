<script lang="ts">
  import { onMount } from "svelte";
  import {
    applyProfile,
    createProfile,
    deleteProfile,
    listProfiles,
    type ApplyReport,
    type ProfileRow,
  } from "$lib/profiles";
  import { t } from "$lib/i18n/index.svelte";

  import { errorText } from "$lib/errors";
  let profiles = $state<ProfileRow[]>([]);
  let newName = $state("");
  let busy = $state(false);
  let report = $state<{ name: string; r: ApplyReport } | null>(null);
  let error = $state("");

  async function refresh() {
    profiles = await listProfiles();
  }

  onMount(refresh);

  async function create() {
    const name = newName.trim();
    if (!name || busy) return;
    busy = true;
    error = "";
    try {
      await createProfile(name);
      newName = "";
      await refresh();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function apply(p: ProfileRow) {
    if (busy) return;
    busy = true;
    error = "";
    report = null;
    try {
      const r = await applyProfile(p.id);
      report = { name: p.name, r };
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function remove(p: ProfileRow) {
    if (busy) return;
    busy = true;
    try {
      await deleteProfile(p.id);
      await refresh();
    } finally {
      busy = false;
    }
  }
</script>

<div class="profiles">
  <header>
    <h2>{t("nav.profiles")}</h2>
    <p class="sub">{t("profiles.subtitle")}</p>
  </header>

  <div class="create">
    <input
      class="input"
      placeholder={t("profiles.namePlaceholder")}
      bind:value={newName}
      onkeydown={(e) => e.key === "Enter" && create()}
    />
    <button class="btn btn-primary" type="button" onclick={create} disabled={busy || !newName.trim()}>
      {t("profiles.capture")}
    </button>
  </div>
  <p class="hint">{t("profiles.captureHint")}</p>

  {#if error}<div class="err">{error}</div>{/if}

  {#if report}
    <div class="report">
      {t("profiles.appliedReport", { name: report.name, activated: report.r.activated, deactivated: report.r.deactivated })}
      {#if report.r.errors.length}
        <ul class="r-errs">
          {#each report.r.errors as e}<li>{e}</li>{/each}
        </ul>
      {/if}
    </div>
  {/if}

  {#if profiles.length === 0}
    <div class="empty">{t("profiles.empty")}</div>
  {:else}
    <ul class="list">
      {#each profiles as p (p.id)}
        <li>
          <div class="p-info">
            <span class="p-name">{p.name}</span>
            <span class="p-count mono">{t("profiles.modCount", { count: p.entry_count })}</span>
          </div>
          <div class="p-actions">
            <button class="btn btn-primary" type="button" onclick={() => apply(p)} disabled={busy}>{t("profiles.apply")}</button>
            <button class="btn-ghost del" type="button" onclick={() => remove(p)} disabled={busy} title={t("common.delete")}>✕</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .profiles {
    max-width: 640px;
  }
  header {
    margin-bottom: 20px;
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
  }
  .sub {
    color: var(--muted);
    margin-top: 6px;
    line-height: 1.5;
  }
  .create {
    display: flex;
    gap: 10px;
  }
  .create .input {
    flex: 1;
  }
  .hint {
    color: var(--muted);
    font-size: 11.5px;
    margin-top: 6px;
  }
  .err {
    margin-top: 12px;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 12px;
  }
  .report {
    margin-top: 14px;
    padding: 10px 12px;
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--txt2);
    font-size: 12.5px;
  }
  .r-errs {
    margin: 6px 0 0 16px;
    color: var(--rosso-bright);
    font-size: 11.5px;
  }
  .list {
    list-style: none;
    margin-top: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 14px;
    background: var(--card);
    border: 1px solid var(--line);
  }
  .p-name {
    font-size: 13px;
    font-weight: 600;
  }
  .p-count {
    color: var(--muted);
    font-size: 11px;
    margin-left: 10px;
  }
  .p-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .p-actions .del {
    padding: 6px 9px;
  }
  .empty {
    color: var(--muted);
    text-align: center;
    padding: 40px 0;
  }
</style>
