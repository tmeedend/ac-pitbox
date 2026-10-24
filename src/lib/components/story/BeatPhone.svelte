<script lang="ts">
  // What a Beat changes in the phone (SPEC-story-mode, Skins: how a frame
  // gets its look): its status bar - date label, battery, signal, Wi-Fi,
  // airplane mode - and the skin an app wears. Each left alone stays as it
  // was: nothing set here writes nothing.
  import { t } from "$lib/i18n/index.svelte";
  import { skinsFor } from "$lib/story/frameEdit";
  import type { FlowNode } from "$lib/story/studio";
  import { studio } from "$lib/story/studioState.svelte";
  import Field from "$lib/components/ui/Field.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import TargetSelect from "./TargetSelect.svelte";

  interface Props {
    beat: NonNullable<FlowNode["beat"]>;
    onchange: () => void;
  }
  const { beat, onchange }: Props = $props();

  const story = $derived(studio.story!);
  /** The apps of the story's phones, by id, with their template. */
  const apps = $derived.by(() => {
    const out = new Map<string, string>();
    for (const f of story.frames.value?.frames ?? []) for (const a of f.apps) if (!out.has(a.id)) out.set(a.id, a.template);
    return [...out.entries()];
  });

  type Status = NonNullable<typeof beat.status>;

  /** Sets one value of the status bar, or leaves it alone when `v` is
   * `undefined`; an empty status writes nothing. */
  function setStatus<K extends keyof Status>(key: K, v: Status[K] | undefined) {
    const status: Status = { ...(beat.status ?? {}) };
    if (v === undefined) delete status[key];
    else status[key] = v;
    if (Object.keys(status).length) beat.status = status;
    else delete beat.status;
    onchange();
  }

  /** A yes / no / unchanged choice. */
  function flag(v: string): boolean | undefined {
    return v === "" ? undefined : v === "on";
  }

  function setSkin(app: string, skin: string) {
    const skins = { ...(beat.skins ?? {}) };
    if (skin) skins[app] = skin;
    else delete skins[app];
    if (Object.keys(skins).length) beat.skins = skins;
    else delete beat.skins;
    onchange();
  }
</script>

<Field label={t("story.frames.beatStatus")}>
  <div class="grid">
    <label>
      <span class="lbl-key">{t("story.frames.battery")}</span>
      <NumberStepper
        value={beat.status?.battery ?? -1}
        emptyValue={-1}
        emptyStart={100}
        min={0}
        max={100}
        width={80}
        onchange={(v) => setStatus("battery", v < 0 ? undefined : v)}
      />
    </label>
    <label>
      <span class="lbl-key">{t("story.frames.signal")}</span>
      <NumberStepper
        value={beat.status?.signal ?? -1}
        emptyValue={-1}
        emptyStart={4}
        min={0}
        max={4}
        width={80}
        onchange={(v) => setStatus("signal", v < 0 ? undefined : v)}
      />
    </label>
    {#each [["wifi", beat.status?.wifi], ["airplane", beat.status?.airplane]] as [key, v] (key)}
      <label>
        <span class="lbl-key">{t(`story.frames.${key}`)}</span>
        <select
          class="input"
          value={v === undefined ? "" : v ? "on" : "off"}
          onchange={(e) => setStatus(key as "wifi", flag((e.currentTarget as HTMLSelectElement).value))}
        >
          <option value="">{t("story.scenes.unchanged")}</option>
          <option value="on">{t("story.frames.on")}</option>
          <option value="off">{t("story.frames.off")}</option>
        </select>
      </label>
    {/each}
  </div>
  <input
    class="input"
    value={beat.status?.date ?? ""}
    placeholder={t("story.frames.dateLabel")}
    onchange={(e) => {
      const v = (e.currentTarget as HTMLInputElement).value.trim();
      setStatus("date", v || undefined);
    }}
  />
</Field>
{#if apps.length}
  <Field label={t("story.frames.beatSkins")}>
    {#each apps as [app, template] (app)}
      <div class="skin">
        <span class="mono app">{app}</span>
        <TargetSelect
          value={beat.skins?.[app] ?? ""}
          options={skinsFor(story.skins, template)}
          emptyLabel={t("story.scenes.unchanged")}
          onchange={(v) => setSkin(app, v)}
        />
      </div>
    {/each}
  </Field>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px 10px;
    margin-bottom: 8px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .skin {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }
  .app {
    width: 90px;
    font-size: 12px;
    color: var(--txt2);
  }
</style>
