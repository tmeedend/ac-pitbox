<script lang="ts">
  // One skin, `frames/skins/<id>/skin.yaml` (SPEC-story-mode, Skins: how a
  // frame gets its look): the template it dresses, its colours, its font and
  // its slots - each an image of the skin's own folder with, for one that
  // stretches, its four fixed margins (9-slice). Every part optional: what
  // is left out keeps Pit Box's look. Its files are imported into its
  // folder, so the folder stays the whole skin, copiable to another story.
  import { t } from "$lib/i18n/index.svelte";
  import { filesOfSkin, setColor, setSlice, setSlot, skinIdOf } from "$lib/story/frameEdit";
  import { removeSkin, saveSkinFile, studio } from "$lib/story/studioState.svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import Field from "$lib/components/ui/Field.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import FilePick from "./FilePick.svelte";
  import FileStatus from "./FileStatus.svelte";

  interface Props {
    /** Its `skin.yaml`. */
    file: string;
  }
  const { file }: Props = $props();

  const story = $derived(studio.story!);
  const entry = $derived(story.skins.find((s) => s.file === file)!);
  const skin = $derived(entry.value);
  const id = $derived(skinIdOf(file) ?? "");
  const into = $derived(`frames/skins/${id}`);
  const files = $derived(filesOfSkin(story.skin_files, id));
  const template = $derived(story.templates.find((tp) => tp.name === skin?.template) ?? null);
  /** The colours a skin gives (`story_engine::skin::COLORS`). */
  const COLORS = ["background", "text", "muted", "accent"];
  const SIDES = ["top", "right", "bottom", "left"] as const;
  let problem = $state<string | null>(null);

  function save() {
    void saveSkinFile(file);
  }

  async function remove() {
    const ok = await confirm(t("story.frames.deleteSkin", { id }), { title: t("story.world.delete"), kind: "warning" });
    if (ok) problem = await removeSkin(file);
  }
</script>

<div class="sheet">
  <header>
    <span class="mono id">{id}</span>
    <button class="btn btn-ghost" onclick={remove}>{t("story.world.delete")}</button>
  </header>
  {#if problem}<p class="errbox">{problem}</p>{/if}
  <FileStatus {entry} />

  {#if skin}
    <Field label={t("story.frames.template")}>
      <select
        class="input"
        value={skin.template}
        onchange={(e) => {
          skin.template = (e.currentTarget as HTMLSelectElement).value;
          // Slots of another template no longer fit: the checks would
          // refuse them.
          const slots = story.templates.find((tp) => tp.name === skin.template)?.slots ?? [];
          for (const name of Object.keys(skin.slots ?? {})) if (!slots.includes(name)) setSlot(skin, name, "");
          save();
        }}
      >
        {#each story.templates as tp (tp.name)}<option value={tp.name}>{t(`story.frames.templates.${tp.name}`)}</option>{/each}
      </select>
    </Field>

    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("story.frames.colors")}</span></div>
      <div class="blk-b colors">
        {#each COLORS as name (name)}
          {@const v = skin.colors?.[name] ?? ""}
          <label class="color">
            <span class="lbl-key">{t(`story.frames.color.${name}`)}</span>
            <input
              type="color"
              value={v || "#808080"}
              class:unset={!v}
              onchange={(e) => {
                setColor(skin, name, (e.currentTarget as HTMLInputElement).value);
                save();
              }}
            />
            {#if v}
              <button
                class="btn btn-ghost clear"
                aria-label={t("story.frames.clearColor")}
                onclick={() => {
                  setColor(skin, name, "");
                  save();
                }}>✕</button
              >
            {/if}
          </label>
        {/each}
      </div>
    </section>

    <Field label={t("story.frames.font")}>
      <FilePick
        value={skin.font ?? ""}
        options={files}
        {into}
        kinds={["ttf", "otf", "woff2"]}
        emptyLabel={t("story.frames.pitBoxLook")}
        onpick={(f) => {
          if (f) skin.font = f;
          else delete skin.font;
          save();
        }}
      />
    </Field>

    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("story.frames.slots")}</span></div>
      <div class="blk-b">
        {#each template?.slots ?? [] as slot (slot)}
          {@const s = skin.slots?.[slot]}
          <div class="slot">
            <Field label={t(`story.frames.slot.${slot}`)}>
              <FilePick
                value={s?.file ?? ""}
                options={files}
                {into}
                kinds={["png", "webp"]}
                emptyLabel={t("story.frames.pitBoxLook")}
                onpick={(f) => {
                  setSlot(skin, slot, f);
                  save();
                }}
              />
            </Field>
            {#if s}
              <div class="margins">
                <span class="lbl-key">{t("story.frames.margins")}</span>
                {#each SIDES as side, i (side)}
                  <NumberStepper
                    value={s.slice?.[i] ?? 0}
                    min={0}
                    max={4000}
                    width={80}
                    title={t(`story.frames.side.${side}`)}
                    onchange={(v) => {
                      const slice = [...(s.slice ?? [0, 0, 0, 0])] as [number, number, number, number];
                      slice[i] = v;
                      setSlice(skin, slot, slice);
                      save();
                    }}
                  />
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .sheet {
    flex: 1;
    min-width: 420px;
    max-width: 760px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  .id {
    font-size: 15px;
    color: var(--txt);
  }
  .colors {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 22px;
  }
  .color {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  /* A colour not given: the swatch is a placeholder, Pit Box's is used. */
  .color input.unset {
    opacity: 0.35;
  }
  .clear {
    padding: 2px 6px;
    color: var(--muted);
  }
  .slot {
    padding: 6px 0;
    border-top: 1px solid var(--line);
  }
  .slot:first-child {
    border-top: none;
  }
  .margins {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }
  .errbox {
    margin: 0 0 10px;
  }
</style>
