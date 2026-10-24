<script lang="ts">
  // One place of `scenes/scenes.yaml` (SPEC-story-mode, Scenes, frame and
  // sound): its name, which renames every Beat and dialogue citing it, its
  // display name, blur and ambient sound, then its variants in the order the
  // engine tries them - the first that holds shows, the last is the default.
  // Files are picked among those already in `media/`: the editor never
  // copies one there (as for portraits).
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import { moveItem } from "$lib/story/flow";
  import {
    PERIODS,
    SEASONS,
    sceneFiles,
    setCriterion,
    soundFiles,
    toggled,
    type SceneVariant,
  } from "$lib/story/scenes";
  import { saveStoryFile, studio } from "$lib/story/studioState.svelte";
  import { nameFrom, nameProblem, parseList } from "$lib/story/world";
  import Field from "$lib/components/ui/Field.svelte";
  import AddName from "./AddName.svelte";
  import NameRow from "./NameRow.svelte";
  import SoundPlay from "./SoundPlay.svelte";

  interface Props {
    /** Index of the place in `scenes/scenes.yaml`. */
    index: number;
  }
  const { index }: Props = $props();

  const story = $derived(studio.story!);
  const scenes = $derived(story.scenes.value!);
  const place = $derived(scenes.scenes[index]);
  const ids = $derived(scenes.scenes.map((p) => p.id));
  const variantIds = $derived(place.variants.map((v) => v.id));
  const files = $derived(sceneFiles(story.media));
  const sounds = $derived(soundFiles(story.media));
  /** Why a variant's new id was refused, by its position. */
  let idProblem = $state<{ at: number; why: string } | null>(null);

  function save() {
    void saveStoryFile("scenes");
  }

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLSelectElement).value;
  }

  function setText(target: { name?: string; ambient?: string }, key: "name" | "ambient", v: string) {
    const text = v.trim();
    if (text) target[key] = text;
    else delete target[key];
    save();
  }

  function renameVariant(at: number, e: Event) {
    const field = e.currentTarget as HTMLInputElement;
    const v = place.variants[at];
    const to = field.value.trim();
    if (to === v.id) return (idProblem = null);
    const why = nameProblem(
      to,
      variantIds.filter((id) => id !== v.id),
    );
    if (why) {
      idProblem = { at, why: t(why) };
      field.value = v.id;
      return;
    }
    idProblem = null;
    v.id = to;
    save();
  }

  /** A new variant goes before the last one: the last is the default, so
   * the general case stays last. */
  function addVariant(name: string): string | null {
    const id = nameFrom(name, variantIds, "variant");
    const at = Math.max(0, place.variants.length - 1);
    place.variants.splice(at, 0, { id, file: "" });
    save();
    return null;
  }

  async function removeVariant(at: number) {
    const v = place.variants[at];
    const ok = await confirm(t("story.world.deleteConfirm", { id: v.id }), {
      title: t("story.world.delete"),
      kind: "warning",
    });
    if (!ok) return;
    place.variants = place.variants.filter((_, k) => k !== at);
    save();
  }

  function move(at: number, delta: number) {
    place.variants = moveItem(place.variants, at, delta);
    save();
  }

  function setList(v: SceneVariant, key: "weather" | "mood", e: Event) {
    setCriterion(v, key, parseList(value(e)));
    save();
  }

  /** A variant's blur: the place's (left out), blurred, or sharp. */
  function setBlur(target: { blur?: boolean }, choice: string) {
    if (choice === "") delete target.blur;
    else target.blur = choice === "on";
    save();
  }
</script>

<div class="sheet">
  <ul class="rows">
    <NameRow
      kind="scene"
      id={place.id}
      taken={ids}
      onremove={() => {
        scenes.scenes = scenes.scenes.filter((_, k) => k !== index);
        studio.scene = scenes.scenes[0]?.id ?? null;
        save();
      }}
    >
      <input
        class="input display"
        value={place.name ?? ""}
        aria-label={t("story.world.displayName")}
        onchange={(e) => setText(place, "name", value(e))}
      />
    </NameRow>
  </ul>

  <Field label={t("story.scenes.ambient")}>
    <div class="sound">
      <select class="input mono" value={place.ambient ?? ""} onchange={(e) => setText(place, "ambient", value(e))}>
        <option value="">{t("story.scenes.noAmbient")}</option>
        {#if place.ambient && !sounds.includes(place.ambient)}<option value={place.ambient}>{place.ambient}</option>{/if}
        {#each sounds as s (s)}<option value={s}>{s}</option>{/each}
      </select>
      <SoundPlay folder={story.path} path={place.ambient ?? ""} />
    </div>
    {#if place.ambient && !story.media.includes(place.ambient)}
      <span class="missing">{t("story.scenes.fileMissing")}</span>
    {/if}
  </Field>
  <Field>
    <label class="check">
      <input
        type="checkbox"
        checked={place.blur !== false}
        onchange={(e) => setBlur(place, (e.currentTarget as HTMLInputElement).checked ? "" : "off")}
      />
      {t("story.scenes.blurred")}
    </label>
  </Field>

  <section class="blk">
    <div class="blk-h">
      <span class="blk-t">{t("story.scenes.variants")}</span>
      <span class="blk-n">{place.variants.length}</span>
    </div>
    <div class="blk-b">
      {#if !files.length}<p class="hint">{t("story.scenes.noFiles")}</p>{/if}
      <ol class="variants">
        {#each place.variants as v, i (v.id + ":" + i)}
          <li class="variant">
            <div class="line">
              <button class="btn btn-ghost mv" disabled={i === 0} onclick={() => move(i, -1)} aria-label={t("story.scenes.moveUp")}
                >↑</button
              >
              <button
                class="btn btn-ghost mv"
                disabled={i === place.variants.length - 1}
                onclick={() => move(i, 1)}
                aria-label={t("story.scenes.moveDown")}>↓</button
              >
              <input class="input mono vid" value={v.id} aria-label={t("story.world.name")} onchange={(e) => renameVariant(i, e)} />
              {#if i === place.variants.length - 1}<span class="default">{t("story.scenes.default")}</span>{/if}
              <button class="btn btn-ghost del" onclick={() => removeVariant(i)} aria-label={t("story.world.delete")}
                >✕</button
              >
            </div>
            {#if idProblem?.at === i}<p class="errbox">{idProblem.why}</p>{/if}

            <Field label={t("story.scenes.file")}>
              <select
                class="input mono"
                value={v.file}
                onchange={(e) => {
                  v.file = value(e);
                  save();
                }}
              >
                <option value="">{t("story.scenes.pickFile")}</option>
                {#if v.file && !files.includes(v.file)}<option value={v.file}>{v.file}</option>{/if}
                {#each files as f (f)}<option value={f}>{f}</option>{/each}
              </select>
              {#if v.file && !story.media.includes(v.file)}
                <span class="missing">{t("story.scenes.fileMissing")}</span>
              {/if}
            </Field>

            <div class="criteria">
              <span class="lbl-key">{t("story.scenes.season")}</span>
              {#each SEASONS as s (s)}
                <label class="check">
                  <input
                    type="checkbox"
                    checked={v.season?.includes(s) ?? false}
                    onchange={(e) => {
                      setCriterion(v, "season", toggled(v.season, s, (e.currentTarget as HTMLInputElement).checked, SEASONS));
                      save();
                    }}
                  />
                  {t(`story.scenes.seasons.${s}`)}
                </label>
              {/each}
            </div>
            <div class="criteria">
              <span class="lbl-key">{t("story.scenes.time")}</span>
              {#each PERIODS as p (p)}
                <label class="check">
                  <input
                    type="checkbox"
                    checked={v.time?.includes(p) ?? false}
                    onchange={(e) => {
                      setCriterion(v, "time", toggled(v.time, p, (e.currentTarget as HTMLInputElement).checked, PERIODS));
                      save();
                    }}
                  />
                  {t(`story.scenes.periods.${p}`)}
                </label>
              {/each}
            </div>
            <div class="pair">
              <Field label={t("story.scenes.weather")}>
                <input class="input mono" value={(v.weather ?? []).join(", ")} onchange={(e) => setList(v, "weather", e)} />
              </Field>
              <Field label={t("story.scenes.mood")}>
                <input class="input mono" value={(v.mood ?? []).join(", ")} onchange={(e) => setList(v, "mood", e)} />
              </Field>
            </div>
            <div class="pair">
              <Field label={t("story.scenes.blur")}>
                <select
                  class="input"
                  value={v.blur === undefined ? "" : v.blur ? "on" : "off"}
                  onchange={(e) => setBlur(v, value(e))}
                >
                  <option value="">{t("story.scenes.likePlace")}</option>
                  <option value="on">{t("story.scenes.blurOn")}</option>
                  <option value="off">{t("story.scenes.blurOff")}</option>
                </select>
              </Field>
              <Field label={t("story.scenes.ambient")}>
                <div class="sound">
                  <select class="input mono" value={v.ambient ?? ""} onchange={(e) => setText(v, "ambient", value(e))}>
                    <option value="">{t("story.scenes.likePlace")}</option>
                    {#if v.ambient && !sounds.includes(v.ambient)}<option value={v.ambient}>{v.ambient}</option>{/if}
                    {#each sounds as s (s)}<option value={s}>{s}</option>{/each}
                  </select>
                  <!-- What the variant plays: its own sound, else the place's. -->
                  <SoundPlay folder={story.path} path={v.ambient ?? place.ambient ?? ""} />
                </div>
              </Field>
            </div>
          </li>
        {/each}
      </ol>
      <AddName placeholder={t("story.scenes.newVariant")} onadd={addVariant} />
    </div>
  </section>
</div>

<style>
  .sheet {
    flex: 1;
    min-width: 420px;
    max-width: 760px;
  }
  .rows {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
  }
  .display {
    flex: 1;
    min-width: 0;
  }
  .pair {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    align-items: flex-end;
  }
  /* A sound and the button to listen to it, on one line. */
  .sound {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sound select {
    flex: 1;
    min-width: 0;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  .missing {
    color: var(--yellow);
    font-size: 12px;
  }
  .hint {
    margin: 0 0 8px;
    color: var(--muted);
    font-size: 12px;
  }
  .variants {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
  }
  .variant {
    padding: 10px 0;
    border-top: 1px solid var(--line);
  }
  .variant:first-child {
    border-top: none;
    padding-top: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
  }
  .mv {
    padding: 2px 6px;
  }
  .vid {
    width: 190px;
  }
  .default {
    color: var(--muted);
    font-size: 12px;
  }
  .del {
    margin-left: auto;
    padding: 4px 8px;
    color: var(--muted);
  }
  .criteria {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    margin: 6px 0;
  }
  .criteria .lbl-key {
    width: 90px;
  }
  .errbox {
    margin: 0 0 6px;
  }
</style>
