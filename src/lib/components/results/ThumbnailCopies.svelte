<script lang="ts">
  // Files › Maintenance: Content Manager's copies of past thumbnail sessions
  // (RESULTAT§5.2). Listed with their date and cars, and sent to the recycle
  // bin only once the user has confirmed — CM's data, so only what Pit Box
  // caused or the user confirmed goes.
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { listThumbnailCopies, trashThumbnailCopies, type ThumbnailCopy } from "$lib/results/results";

  let copies = $state<ThumbnailCopy[] | null>(null);
  let busy = $state(false);
  let error = $state("");
  let message = $state("");
  let open = $state(false);

  async function look() {
    busy = true;
    error = "";
    message = "";
    try {
      copies = await listThumbnailCopies();
      open = true;
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function trashAll() {
    if (!copies?.length) return;
    const ok = await confirm(t("results.thumbCopiesConfirm", { count: copies.length }), {
      title: t("results.thumbCopiesTitle"),
      kind: "warning",
    });
    if (!ok) return;
    busy = true;
    error = "";
    try {
      const gone = await trashThumbnailCopies(copies.map((c) => c.file));
      message = t("results.thumbCopiesDone", { count: gone.length });
      copies = copies.filter((c) => !gone.includes(c.file));
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const fmt = (ms: number) => new Date(ms).toLocaleString();
</script>

<section class="stock-sec">
  <h3>{t("results.thumbCopiesTitle")}</h3>
  <p class="hint">{t("results.thumbCopiesHint")}</p>
  <div class="stock-row">
    <button class="btn" type="button" onclick={() => void look()} disabled={busy}>{t("results.thumbCopiesLook")}</button>
    {#if copies?.length}
      <button class="btn danger" type="button" onclick={() => void trashAll()} disabled={busy}
        >{t("results.thumbCopiesTrash", { count: copies.length })}</button
      >
    {:else if copies && open}
      <span class="stock-msg">{t("results.thumbCopiesNone")}</span>
    {/if}
    {#if message}<span class="stock-msg">{message}</span>{/if}
  </div>
  {#if error}<div class="errbox">{error}</div>{/if}
  {#if copies?.length}
    <ul class="list">
      {#each copies as c (c.file)}
        <li>
          <span class="mono when">{fmt(c.at_ms)}</span>
          <span class="cars">{c.cars.join(", ")}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  /* The look of a Maintenance section (`Maintenance.svelte`, whose styles are
     scoped): kept the same by hand. */
  .stock-sec {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 12px 14px;
    margin-bottom: 20px;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted);
    margin-bottom: 6px;
  }
  .hint {
    font-size: 11.5px;
    color: var(--faint);
    margin-bottom: 10px;
    line-height: 1.5;
  }
  .stock-row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .stock-msg {
    color: var(--muted);
    font-size: 12px;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 12px;
    flex: none;
  }
  .btn.danger:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .errbox {
    margin-top: 10px;
  }
  .list {
    list-style: none;
    padding: 0;
    margin: 10px 0 0;
    max-height: calc(30 * var(--vh));
    overflow: auto;
    font-size: 12px;
  }
  .list li {
    display: flex;
    gap: 14px;
    padding: 4px 0;
    border-bottom: 1px solid var(--line);
  }
  .when {
    color: var(--muted);
    flex: none;
  }
  .cars {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>