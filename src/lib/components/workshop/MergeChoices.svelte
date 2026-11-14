<script lang="ts">
  // What a merge will do, before anything is written (EXPORT§7.3, step 3;
  // EXPORT§7.4): what arrives, what stays, the notes kept twice — said first,
  // so that whoever imports knows they lose nothing — and, for each group
  // whose entries differ between the two sides, the choice of which wins.
  import { t } from "$lib/i18n/index.svelte";
  import { GROUP_LABEL, GROUPS, type Choices, type Group, type Plan, type Side } from "$lib/workshop/transfer";

  interface Props {
    plan: Plan;
    choices: Choices;
    disabled?: boolean;
  }
  let { plan, choices = $bindable(), disabled = false }: Props = $props();

  const GROUP_WHAT: Record<Group, string> = {
    names: "transfer.groupNamesWhat",
    classification: "transfer.groupClassificationWhat",
    tech: "transfer.groupTechWhat",
    prefs: "transfer.groupPrefsWhat",
  };
  const SIDES: { side: Side; label: string; what: string }[] = [
    { side: "here", label: "transfer.sideHere", what: "transfer.sideHereWhat" },
    { side: "export", label: "transfer.sideExport", what: "transfer.sideExportWhat" },
  ];

  const disputed = $derived(GROUPS.filter((g) => (plan.conflicts[g] ?? 0) > 0));

  function choose(group: Group, side: Side): void {
    choices = { ...choices, [group]: side };
  }
</script>

<section class="merge">
  <h3 class="lbl-sub">{t("transfer.mergeTitle")}</h3>
  <p class="line">{t("transfer.mergeReassure")}</p>
  <p class="line counts">
    {t("transfer.mergeCounts", { added: plan.added, kept: plan.kept, filled: plan.filled })}
  </p>
  {#if plan.notes_joined}
    <p class="line note">{t("transfer.mergeNotes", { count: plan.notes_joined })}</p>
  {/if}
  {#if plan.blocked.length}
    <p class="line">
      {t("transfer.mergeBlocked", { count: plan.blocked.length })}
      <span class="names">{plan.blocked.map((n) => n.name).join(", ")}</span>
    </p>
  {/if}
  {#if plan.attached_skipped}
    <p class="line muted">{t("transfer.mergeAttachedSkipped", { count: plan.attached_skipped })}</p>
  {/if}

  {#if disputed.length === 0}
    <p class="line muted">{t("transfer.mergeNoConflict")}</p>
  {:else}
    <p class="line">{t("transfer.mergeConflictsIntro")}</p>
    {#each disputed as group (group)}
      <div class="group" role="radiogroup" aria-label={t(GROUP_LABEL[group])}>
        <div class="g-head">
          <span class="g-name">{t(GROUP_LABEL[group])}</span>
          <span class="g-count mono">{t("transfer.mergeDifferences", { count: plan.conflicts[group] ?? 0 })}</span>
        </div>
        <p class="g-what">{t(GROUP_WHAT[group])}</p>
        {#each SIDES as option (option.side)}
          <label class="option">
            <input
              type="radio"
              name={`merge-${group}`}
              {disabled}
              checked={choices[group] === option.side}
              onchange={() => choose(group, option.side)}
            />
            <span class="o-body">
              <span class="o-label">{t(option.label)}</span>
              <span class="o-what">{t(option.what)}</span>
            </span>
          </label>
        {/each}
      </div>
    {/each}
  {/if}
</section>

<style>
  .merge {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .line {
    color: var(--txt2);
    font-size: 12px;
    line-height: 1.45;
  }
  .line.muted {
    color: var(--muted);
    font-size: 11.5px;
  }
  .counts {
    color: var(--muted);
  }
  .names {
    display: block;
    color: var(--muted);
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .group {
    margin: 4px 0 0;
    padding: 8px 10px;
    background: var(--panel2);
    border: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .g-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .g-name {
    color: var(--txt2);
    font-size: 12.5px;
  }
  .g-count {
    color: var(--muted);
    font-size: 11px;
  }
  .g-what {
    color: var(--muted);
    font-size: 11.5px;
  }
  .option {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    cursor: pointer;
  }
  .option input {
    flex: none;
    margin-top: 2px;
    accent-color: var(--rosso);
  }
  .o-body {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .o-label {
    color: var(--txt2);
    font-size: 12px;
  }
  .o-what {
    color: var(--muted);
    font-size: 11.5px;
    line-height: 1.4;
  }
</style>
