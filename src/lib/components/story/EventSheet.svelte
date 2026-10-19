<script lang="ts">
  // One event of the story (SPEC-story-mode, Events, retries and failure):
  // its name, what it launches, what success takes, and what each outcome
  // does. Each edit saves the file at once. Its id renames every sentence
  // and node citing it, and its file, like the World's names (NameRow).
  import { t } from "$lib/i18n/index.svelte";
  import { parseLines } from "$lib/story/flow";
  import { criteriaOf, eventIds, poolIds, successOf, type Criteria } from "$lib/story/events";
  import type { EditedFile, StoryEvent } from "$lib/story/studio";
  import { removeEvent, saveEventFile, studio } from "$lib/story/studioState.svelte";
  import Field from "$lib/components/ui/Field.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";
  import Switch from "$lib/components/ui/Switch.svelte";
  import EventSession from "./EventSession.svelte";
  import FileStatus from "./FileStatus.svelte";
  import NameRow from "./NameRow.svelte";
  import TargetSelect from "./TargetSelect.svelte";

  interface Props {
    entry: EditedFile<StoryEvent>;
  }
  const { entry }: Props = $props();

  const story = $derived(studio.story!);
  const money = $derived(story.manifest.value?.settings.money ?? false);
  const pools = $derived(poolIds(story.content));
  const criteria = $derived(entry.value ? criteriaOf(entry.value.success) : null);

  let removeProblem = $state<string | null>(null);

  function save() {
    void saveEventFile(entry.file);
  }

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value;
  }

  function setCriteria(change: (c: Criteria) => void) {
    if (!entry.value || !criteria) return;
    const next = { ...criteria, others: [...criteria.others] };
    change(next);
    entry.value.success = successOf(next);
    save();
  }

  function setEffects(target: { effects: string[] }, e: Event) {
    target.effects = parseLines(value(e));
    save();
  }
</script>

<div class="sheet">
  <FileStatus entry={entry} />
  {#if entry.value}
    {@const ev = entry.value}
    <header>
      <input
        class="input title"
        value={ev.name ?? ""}
        placeholder={ev.id}
        aria-label={t("story.events.name")}
        onchange={(e) => {
          const name = value(e).trim();
          ev.name = name || null;
          save();
        }}
      />
      <ul class="id-row">
        <NameRow
          kind="event"
          id={ev.id}
          taken={eventIds(story.events)}
          onremove={async () => (removeProblem = await removeEvent(entry.file))}
        />
      </ul>
      {#if removeProblem}<p class="errbox">{removeProblem}</p>{/if}
    </header>

    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("story.events.event")}</span></div>
      <div class="blk-b">
        <EventSession event={ev} {money} onsave={save} />
      </div>
    </section>

    {#if criteria}
      <section class="blk">
        <div class="blk-h">
          <span class="blk-t">{t("story.events.success")}</span>
          {#if ev.success.length === 0}<span class="blk-n">{t("story.events.finishingEnough")}</span>{/if}
        </div>
        <div class="blk-b">
          <div class="toggle">
            <Switch
              on={criteria.position !== null}
              label={t("story.events.position")}
              onclick={() => setCriteria((c) => (c.position = c.position === null ? 3 : null))}
            />
            <span>{t("story.events.position")}</span>
            {#if criteria.position !== null}
              <NumberStepper
                value={criteria.position}
                min={1}
                width={80}
                onchange={(v) => setCriteria((c) => (c.position = v))}
              />
            {/if}
          </div>
          <div class="toggle">
            <Switch
              on={criteria.finished}
              label={t("story.events.finished")}
              onclick={() => setCriteria((c) => (c.finished = !c.finished))}
            />
            <span>{t("story.events.finished")}</span>
          </div>
          <div class="toggle">
            <Switch
              on={criteria.lapUnder !== null}
              label={t("story.events.lapUnder")}
              onclick={() => setCriteria((c) => (c.lapUnder = c.lapUnder === null ? "1:30.000" : null))}
            />
            <span>{t("story.events.lapUnder")}</span>
            {#if criteria.lapUnder !== null}
              <input
                class="input mono lap"
                value={criteria.lapUnder}
                placeholder="1:18.200"
                onchange={(e) => setCriteria((c) => (c.lapUnder = value(e).trim() || null))}
              />
            {/if}
          </div>
          {#each criteria.others as other (other)}
            <div class="other mono">{other}</div>
          {/each}
        </div>
      </section>
    {/if}

    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("story.events.onSuccess")}</span></div>
      <div class="blk-b">
        <Field label={t("story.studio.effects")}>
          <textarea
            class="input mono"
            rows="3"
            value={ev.on_success.effects.join("\n")}
            placeholder="reputation += 2"
            onchange={(e) => setEffects(ev.on_success, e)}
          ></textarea>
        </Field>
        <Field label={t("story.studio.advance")}>
          <input
            class="input mono short"
            value={ev.on_success.advance ?? ""}
            placeholder="1 month"
            onchange={(e) => {
              ev.on_success.advance = value(e).trim() || null;
              save();
            }}
          />
        </Field>
      </div>
    </section>

    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("story.events.onFailure")}</span></div>
      <div class="blk-b">
        <div class="pair">
          <Field label={t("story.events.pool")}>
            <TargetSelect
              value={ev.on_failure.pool ?? ""}
              options={pools}
              emptyLabel={t("story.events.noPool")}
              onchange={(v) => {
                ev.on_failure.pool = v || null;
                save();
              }}
            />
          </Field>
          {#if ev.on_failure.pool}
            <Field label={t("story.events.pick")}>
              <Seg
                value={String(ev.on_failure.pick)}
                onselect={(v) => {
                  ev.on_failure.pick = Number(v);
                  save();
                }}
                items={[
                  { value: "1", label: "1" },
                  { value: "2", label: "2" },
                ]}
              />
            </Field>
          {/if}
        </div>
        {#if ev.on_failure.pool}
          {@const variants = story.content.filter((c) => c.pool === ev.on_failure.pool)}
          <ul class="variants">
            {#each variants as v (v.id)}
              <li>
                <span class="from">{v.from ?? "—"}</span>
                <span class="when mono">{v.when.length ? v.when.join(" · ") : t("story.studio.always")}</span>
                <span class="line">{v.firstLine ?? ""}</span>
              </li>
            {/each}
          </ul>
        {/if}
        <Field label={t("story.studio.effects")}>
          <textarea
            class="input mono"
            rows="2"
            value={ev.on_failure.effects.join("\n")}
            onchange={(e) => setEffects(ev.on_failure, e)}
          ></textarea>
        </Field>
      </div>
    </section>
  {/if}
</div>

<style>
  .sheet {
    flex: 1;
    min-width: 0;
    max-width: 760px;
  }
  header {
    margin-bottom: 14px;
  }
  .title {
    font-size: 16px;
    font-weight: 600;
    width: 100%;
    max-width: 420px;
  }
  .id-row {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
  }
  .errbox {
    margin: 6px 0 0;
  }
  textarea.input {
    resize: vertical;
    display: block;
    width: 100%;
  }
  .short {
    width: 160px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 32px;
    font-size: 12.5px;
    color: var(--txt2);
  }
  .toggle + .toggle {
    margin-top: 8px;
  }
  .lap {
    width: 110px;
  }
  .other {
    margin-top: 8px;
    font-size: 12px;
    color: var(--muted);
  }
  .pair {
    display: flex;
    gap: 18px;
    flex-wrap: wrap;
  }
  .pair.pair > :global(.field) {
    margin-top: 0;
  }
  .variants {
    list-style: none;
    margin: 10px 0 0;
    padding: 6px 10px;
    background: var(--raised);
  }
  .variants li {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 3px 0;
    font-size: 11.5px;
  }
  .from {
    color: var(--txt2);
    flex-shrink: 0;
  }
  .when {
    color: var(--muted);
    flex-shrink: 0;
  }
  .line {
    color: var(--muted);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
