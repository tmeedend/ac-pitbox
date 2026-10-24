<script lang="ts">
  // The Settings screen (SPEC-story-mode, Story settings): the rules of this
  // story, in `story.yaml` - only those the engine already applies; the
  // others appear with the lot that gives them an effect. Each edit saves the
  // file at once. A relation step renamed takes every sentence citing it
  // along, and the characters starting on it (NameRow).
  import { localeNames, t } from "$lib/i18n/index.svelte";
  import { allNodeIds, moveItem } from "$lib/story/flow";
  import { nameProblem, parseList } from "$lib/story/world";
  import { frameLabel, framesWith } from "$lib/story/frames";
  import type { Settings } from "$lib/story/studio";
  import { saveStoryFile, studio } from "$lib/story/studioState.svelte";
  import Field from "$lib/components/ui/Field.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";
  import Switch from "$lib/components/ui/Switch.svelte";
  import AddName from "./AddName.svelte";
  import FileStatus from "./FileStatus.svelte";
  import NameRow from "./NameRow.svelte";
  import StoryChecks from "./StoryChecks.svelte";
  import TargetSelect from "./TargetSelect.svelte";

  const story = $derived(studio.story!);
  const manifest = $derived(story.manifest.value);
  const settings = $derived(manifest?.settings ?? null);
  const nodeIds = $derived(allNodeIds(story.chapters.map((c) => c.chapter)));
  const language = $derived(manifest?.languages[0] ?? "");
  const languages = $derived(
    language && !(language in localeNames) ? [language, ...Object.keys(localeNames)] : Object.keys(localeNames),
  );

  const places = $derived(studio.story?.scenes.value?.scenes ?? []);

  function save() {
    void saveStoryFile("manifest");
  }

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLSelectElement).value;
  }

  /** Sets a setting and saves. */
  function set<K extends keyof Settings>(key: K, v: Settings[K]) {
    if (!settings) return;
    settings[key] = v;
    save();
  }

  function addStep(name: string): string | null {
    if (!settings) return null;
    const why = nameProblem(name, settings.relations.scale);
    if (why) return t(why);
    settings.relations.scale = [...settings.relations.scale, name];
    save();
    return null;
  }
</script>

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.settings.title")}</span>
  </header>
  {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}
  <FileStatus entry={story.manifest} />

  {#if manifest && settings}
    <div class="grid">
      <section class="blk">
        <div class="blk-h"><span class="blk-t">{t("story.settings.story")}</span></div>
        <div class="blk-b">
          <Field label={t("story.studio.storyTitle")}>
            <input
              class="input"
              value={manifest.title}
              onchange={(e) => {
                const title = value(e).trim();
                if (!title) return;
                manifest.title = title;
                save();
              }}
            />
          </Field>
          <Field label={t("story.settings.authors")}>
            <input
              class="input"
              value={manifest.authors.join(", ")}
              onchange={(e) => {
                manifest.authors = parseList(value(e));
                save();
              }}
            />
          </Field>
          <Field label={t("story.settings.version")}>
            <input
              class="input mono short"
              value={manifest.version}
              onchange={(e) => {
                const version = value(e).trim();
                if (!version) return;
                manifest.version = version;
                save();
              }}
            />
          </Field>
          <Field label={t("story.settings.language")}>
            <select
              class="input"
              value={language}
              onchange={(e) => {
                manifest.languages = [value(e), ...manifest.languages.slice(1)];
                save();
              }}
            >
              {#each languages as code (code)}
                <option value={code}>{localeNames[code] ?? code}</option>
              {/each}
            </select>
          </Field>
        </div>
      </section>

      <section class="blk">
        <div class="blk-h"><span class="blk-t">{t("story.settings.start")}</span></div>
        <div class="blk-b">
          <Field label={t("story.settings.startNode")}>
            <TargetSelect
              value={manifest.start.node}
              options={nodeIds}
              onchange={(v) => {
                if (!v) return;
                manifest.start.node = v;
                save();
              }}
            />
          </Field>
          <Field label={t("story.settings.startFrame")}>
            <select
              class="input"
              value={manifest.start.frame}
              onchange={(e) => {
                manifest.start.frame = value(e);
                save();
              }}
            >
              {#each framesWith(manifest.start.frame) as f (f)}
                <option value={f}>{frameLabel(f)}</option>
              {/each}
            </select>
          </Field>
          <Field label={t("story.settings.startScene")}>
            <TargetSelect
              value={manifest.start.scene ?? ""}
              options={places.map((p) => p.id)}
              labels={Object.fromEntries(places.map((p) => [p.id, p.name ?? p.id]))}
              emptyLabel={t("story.settings.noStartScene")}
              onchange={(v) => {
                if (v) manifest.start.scene = v;
                else delete manifest.start.scene;
                save();
              }}
            />
          </Field>
          <div class="pair">
            <Field label={t("story.settings.startDate")}>
              <input
                class="input"
                type="date"
                value={manifest.start.date}
                onchange={(e) => {
                  const date = value(e);
                  if (!date) return;
                  manifest.start.date = date;
                  save();
                }}
              />
            </Field>
            <Field label={t("story.settings.startClock")}>
              <input
                class="input"
                type="time"
                value={manifest.start.clock}
                onchange={(e) => {
                  const clock = value(e);
                  if (!clock) return;
                  manifest.start.clock = clock;
                  save();
                }}
              />
            </Field>
          </div>
          <div class="pair">
            <Field label={t("story.settings.reputation")}>
              <NumberStepper
                value={manifest.start.reputation}
                width={90}
                onchange={(v) => {
                  manifest.start.reputation = v;
                  save();
                }}
              />
            </Field>
            <Field label={t("story.settings.followers")}>
              <NumberStepper
                value={manifest.start.followers}
                min={0}
                width={90}
                onchange={(v) => {
                  manifest.start.followers = v;
                  save();
                }}
              />
            </Field>
            {#if settings.money}
              <Field label={t("story.settings.money")}>
                <NumberStepper
                  value={manifest.start.money}
                  width={110}
                  step={50}
                  onchange={(v) => {
                    manifest.start.money = v;
                    save();
                  }}
                />
              </Field>
            {/if}
          </div>
        </div>
      </section>

      <section class="blk">
        <div class="blk-h"><span class="blk-t">{t("story.settings.failure")}</span></div>
        <div class="blk-b">
          <Field label={t("story.settings.failedEvent")}>
            <Seg
              value={settings.failure}
              onselect={(v) => set("failure", v as Settings["failure"])}
              items={[
                { value: "retry", label: t("story.settings.failureRetry") },
                { value: "final", label: t("story.settings.failureFinal") },
              ]}
            />
          </Field>
          {#if settings.failure === "retry"}
            <Field>
              <div class="toggle">
                <Switch
                  on={settings.max_attempts !== null}
                  label={t("story.settings.limitAttempts")}
                  onclick={() => set("max_attempts", settings.max_attempts === null ? 3 : null)}
                />
                <span>{t("story.settings.limitAttempts")}</span>
                {#if settings.max_attempts !== null}
                  <NumberStepper
                    value={settings.max_attempts}
                    min={1}
                    width={70}
                    onchange={(v) => set("max_attempts", v)}
                  />
                {/if}
              </div>
            </Field>
          {/if}
          {#if settings.money}
            <Field>
              <div class="toggle">
                <Switch
                  on={settings.retry_cost === "paid"}
                  label={t("story.settings.paidRetries")}
                  onclick={() => set("retry_cost", settings.retry_cost === "paid" ? "free" : "paid")}
                />
                <span>{t("story.settings.paidRetries")}</span>
              </div>
            </Field>
          {/if}
          <Field>
            <div class="toggle">
              <Switch
                on={settings.adaptive_difficulty !== null}
                label={t("story.settings.adaptive")}
                onclick={() =>
                  set("adaptive_difficulty", settings.adaptive_difficulty ? null : { every: 2, cap: 2 })}
              />
              <span>{t("story.settings.adaptive")}</span>
            </div>
          </Field>
          {#if settings.adaptive_difficulty}
            {@const help = settings.adaptive_difficulty}
            <div class="pair">
              <Field label={t("story.settings.adaptiveEvery")}>
                <NumberStepper
                  value={help.every}
                  min={1}
                  width={70}
                  onchange={(v) => set("adaptive_difficulty", { ...help, every: v })}
                />
              </Field>
              <Field label={t("story.settings.adaptiveCap")}>
                <NumberStepper
                  value={help.cap}
                  min={1}
                  width={70}
                  onchange={(v) => set("adaptive_difficulty", { ...help, cap: v })}
                />
              </Field>
            </div>
          {/if}
        </div>
      </section>

      <section class="blk">
        <div class="blk-h"><span class="blk-t">{t("story.settings.moneyAndSaves")}</span></div>
        <div class="blk-b">
          <Field>
            <div class="toggle">
              <Switch on={settings.money} label={t("story.settings.economy")} onclick={() => set("money", !settings.money)} />
              <span>{t("story.settings.economy")}</span>
            </div>
          </Field>
          <Field label={t("story.settings.autosave")}>
            <Seg
              value={settings.saves.autosave}
              onselect={(v) => set("saves", { ...settings.saves, autosave: v as Settings["saves"]["autosave"] })}
              items={[
                { value: "per_day", label: t("story.settings.autosavePerDay") },
                { value: "manual", label: t("story.settings.autosaveManual") },
              ]}
            />
          </Field>
          <Field>
            <div class="toggle">
              <Switch
                on={settings.saves.reload}
                label={t("story.settings.reload")}
                onclick={() => set("saves", { ...settings.saves, reload: !settings.saves.reload })}
              />
              <span>{t("story.settings.reload")}</span>
            </div>
          </Field>
        </div>
      </section>

      <section class="blk">
        <div class="blk-h"><span class="blk-t">{t("story.settings.presentation")}</span></div>
        <div class="blk-b">
          <Field label={t("story.settings.dateShown")}>
            <Seg
              value={settings.date}
              onselect={(v) => set("date", v as Settings["date"])}
              items={[
                { value: "relative", label: t("story.settings.dateRelative") },
                { value: "real", label: t("story.settings.dateReal") },
                { value: "seasons", label: t("story.settings.dateSeasons") },
                { value: "none", label: t("story.settings.dateNone") },
              ]}
            />
          </Field>
          <Field label={t("story.settings.protagonist")}>
            <Seg
              value={settings.protagonist}
              onselect={(v) => set("protagonist", v as Settings["protagonist"])}
              items={[
                { value: "fixed", label: t("story.settings.protagonistFixed") },
                { value: "chosen", label: t("story.settings.protagonistChosen") },
              ]}
            />
          </Field>
        </div>
      </section>

      <section class="blk">
        <div class="blk-h">
          <span class="blk-t">{t("story.settings.relations")}</span>
          <span class="blk-n">{settings.relations.scale.length}</span>
        </div>
        <div class="blk-b">
          <ol class="rows">
            {#each settings.relations.scale as step, i (step + ":" + i)}
              <NameRow
                kind="step"
                id={step}
                taken={settings.relations.scale}
                onremove={() => {
                  settings.relations.scale = settings.relations.scale.filter((_, k) => k !== i);
                  save();
                }}
              >
                {#snippet lead()}
                  <span class="moves">
                    <button
                      class="btn btn-ghost"
                      disabled={i === 0}
                      aria-label={t("story.studio.moveUp")}
                      onclick={() => {
                        settings.relations.scale = moveItem(settings.relations.scale, i, -1);
                        save();
                      }}>↑</button
                    >
                    <button
                      class="btn btn-ghost"
                      disabled={i === settings.relations.scale.length - 1}
                      aria-label={t("story.studio.moveDown")}
                      onclick={() => {
                        settings.relations.scale = moveItem(settings.relations.scale, i, 1);
                        save();
                      }}>↓</button
                    >
                  </span>
                {/snippet}
                {#if step === settings.relations.start}<span class="pill">{t("story.settings.startStep")}</span>{/if}
              </NameRow>
            {/each}
          </ol>
          <AddName placeholder={t("story.settings.newStep")} onadd={addStep} />
          <Field label={t("story.settings.startStepLabel")}>
            <TargetSelect
              value={settings.relations.start}
              options={settings.relations.scale}
              onchange={(v) => {
                if (!v) return;
                settings.relations.start = v;
                save();
              }}
            />
          </Field>
        </div>
      </section>
    </div>
  {/if}

  <StoryChecks section="settings" />
</div>

<style>
  .main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 22px 28px;
  }
  header {
    margin-bottom: 14px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 0 14px;
    max-width: 1100px;
    align-items: start;
  }
  .short {
    width: 120px;
  }
  /* Fields side by side: the row takes the gap a field would take after the
     one above (Field.svelte), and the fields in it take none - doubled, the
     class outweighs Field's `.field + .field`. */
  .pair {
    display: flex;
    gap: 18px;
    flex-wrap: wrap;
    margin-top: 20px;
  }
  .pair.pair > :global(.field) {
    margin-top: 0;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--txt2);
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .moves {
    display: flex;
    flex-shrink: 0;
  }
  .moves .btn {
    padding: 2px 6px;
  }
</style>
