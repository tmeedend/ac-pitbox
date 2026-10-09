<script lang="ts">
  // The parts of a library transfer, each checkable (EXPORT§3, R5), with what
  // it holds in one line. Shared by the export and the import dialogs: the
  // same parts, the same rule — Profiles names mods, and is greyed with its
  // reason when the library is not taken.
  import { t } from "$lib/i18n/index.svelte";
  import { fmtSize } from "$lib/format";
  import { partAllowed, PARTS, type Counts, type Part } from "$lib/workshop/transfer";

  interface Props {
    /** The parts there are to take: all of them for an export, those the
     * file holds for an import. */
    available: Part[];
    counts: Counts;
    /** Weight per part, when known (the export's estimate). */
    bytes?: Partial<Record<Part, number>> | null;
    selected: Part[];
    disabled?: boolean;
  }
  let { available, counts, bytes = null, selected = $bindable(), disabled = false }: Props = $props();

  const NAME: Record<Part, string> = {
    library: "transfer.partLibrary",
    classification: "transfer.partClassification",
    sessions: "transfer.partSessions",
    profiles: "transfer.partProfiles",
    preferences: "transfer.partPreferences",
  };

  function what(part: Part): string {
    const c = counts;
    switch (part) {
      case "library": {
        const line = t("transfer.whatLibrary", {
          mods: c.cars + c.tracks,
          layers: c.layers,
          addons: c.apps + c.others + c.skins + c.sounds,
        });
        return c.stock_with_user_data ? `${line} · ${t("transfer.whatStock", { count: c.stock_with_user_data })}` : line;
      }
      case "classification":
        return t("transfer.whatClassification");
      case "sessions":
        return t("transfer.whatSessions", { sessions: c.sessions, grids: c.grids });
      case "profiles":
        return t("transfer.whatProfiles", { count: c.profiles });
      case "preferences":
        return t("transfer.whatPreferences");
    }
  }

  function toggle(part: Part, on: boolean): void {
    selected = on ? [...selected, part] : selected.filter((p) => p !== part);
  }
</script>

<ul class="parts">
  {#each PARTS.filter((p) => available.includes(p)) as part (part)}
    {@const allowed = partAllowed(part, selected)}
    <li class:off={!allowed}>
      <label>
        <input
          type="checkbox"
          checked={selected.includes(part) && allowed}
          disabled={disabled || !allowed}
          onchange={(e) => toggle(part, e.currentTarget.checked)}
        />
        <span class="p-body">
          <span class="p-name">{t(NAME[part])}</span>
          <span class="p-what">{allowed ? what(part) : t("transfer.profilesNeedLibrary")}</span>
        </span>
        {#if bytes && bytes[part]}<span class="p-size mono">{fmtSize(bytes[part])}</span>{/if}
      </label>
    </li>
  {/each}
</ul>

<style>
  .parts {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  label {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 8px 10px;
    background: var(--panel2);
    border: 1px solid var(--line);
    cursor: pointer;
  }
  li.off label {
    cursor: default;
  }
  input {
    flex: none;
    margin-top: 2px;
    accent-color: var(--rosso);
  }
  .p-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .p-name {
    color: var(--txt2);
    font-size: 12.5px;
  }
  li.off .p-name {
    color: var(--muted);
  }
  .p-what {
    color: var(--muted);
    font-size: 11.5px;
    line-height: 1.45;
  }
  .p-size {
    flex: none;
    color: var(--muted);
    font-size: 11px;
  }
</style>
