<script lang="ts">
  // One phone of `frames/frames.yaml` (SPEC-story-mode, Skins: how a frame
  // gets its look): its name; its look - the picture of the device and where
  // its screen is in it, the wallpaper, the status bar, its sounds - every
  // part optional; then its apps in the home screen's order, each with its
  // name, icon, template, the types it shows, when it appears and its skin.
  // Its pictures and sounds are imported into `media/frames/<id>/`. Its id
  // is not renamed here: the start and the Beats name it.
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import { moveItem } from "$lib/story/flow";
  import { newApp, setTemplate, skinsFor, typesFor } from "$lib/story/frameEdit";
  import { CONTENT_TYPE_LABEL } from "$lib/story/labels";
  import { saveStoryFile, studio } from "$lib/story/studioState.svelte";
  import type { ContentType, FrameApp } from "$lib/story/studio";
  import { nameFrom } from "$lib/story/world";
  import Field from "$lib/components/ui/Field.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";
  import AddName from "./AddName.svelte";
  import FilePick from "./FilePick.svelte";
  import SoundPlay from "./SoundPlay.svelte";
  import TargetSelect from "./TargetSelect.svelte";

  interface Props {
    /** Index of the phone in `frames/frames.yaml`. */
    index: number;
  }
  const { index }: Props = $props();

  const story = $derived(studio.story!);
  const frames = $derived(story.frames.value!);
  const frame = $derived(frames.frames[index]);
  const into = $derived(`media/frames/${frame.id}`);
  const appTemplates = $derived(story.templates.filter((tp) => tp.app));

  const PICTURES = ["png", "webp"];
  const WALLPAPERS = ["png", "webp", "jpg", "jpeg"];
  const SOUNDS = ["ogg", "mp3", "wav"];
  const statusBars = $derived([
    { value: "full", label: t("story.frames.statusFull") },
    { value: "simple", label: t("story.frames.statusSimple") },
    { value: "none", label: t("story.frames.statusNone") },
  ]);
  const appearances = $derived([
    { value: "first_item", label: t("story.frames.appearsFirst") },
    { value: "always", label: t("story.frames.appearsAlways") },
    { value: "beat", label: t("story.frames.appearsBeat") },
  ]);

  function save() {
    void saveStoryFile("frames");
  }

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLSelectElement).value;
  }

  /** Sets or removes an optional text: an empty one writes nothing. */
  function setText<T extends object, K extends keyof T>(target: T, key: K, v: string) {
    if (v.trim()) (target as Record<K, string>)[key] = v.trim();
    else delete target[key];
    save();
  }

  function setBody(file: string) {
    if (!file) delete frame.body;
    else frame.body = { file, screen: frame.body?.screen ?? { x: 0, y: 0, width: 1080, height: 2340 } };
    save();
  }

  function setScreen(key: "x" | "y" | "width" | "height" | "radius", n: number) {
    if (!frame.body) return;
    frame.body.screen = { ...frame.body.screen, [key]: n };
    if (key === "radius" && n === 0) delete frame.body.screen.radius;
    save();
  }

  function setSound(key: "tap" | "send", file: string) {
    const sounds = { ...(frame.sounds ?? {}) };
    if (file) sounds[key] = file;
    else delete sounds[key];
    if (Object.keys(sounds).length) frame.sounds = sounds;
    else delete frame.sounds;
    save();
  }

  function toggleType(app: FrameApp, kind: string, on: boolean) {
    app.shows = on ? [...app.shows, kind] : app.shows.filter((k) => k !== kind);
    save();
  }

  function addApp(name: string): string | null {
    const id = nameFrom(name, frame.apps.map((a) => a.id), "app");
    // The first template with a type left to show; the thread otherwise.
    const template = appTemplates.find((tp) => typesFor(frame, { id, template: tp.name, shows: [] }, appTemplates).length);
    frame.apps.push({ ...newApp(frame, id, template ?? appTemplates[0]), name });
    save();
    return null;
  }

  async function removeApp(i: number) {
    const app = frame.apps[i];
    const ok = await confirm(t("story.world.deleteConfirm", { id: app.id }), { title: t("story.world.delete"), kind: "warning" });
    if (!ok) return;
    frame.apps = frame.apps.filter((_, k) => k !== i);
    save();
  }

  async function removeFrame() {
    const ok = await confirm(t("story.world.deleteConfirm", { id: frame.id }), { title: t("story.world.delete"), kind: "warning" });
    if (!ok) return;
    frames.frames = frames.frames.filter((_, k) => k !== index);
    studio.frame = null;
    save();
  }
</script>

<div class="sheet">
  <header>
    <span class="mono id">{frame.id}</span>
    <button class="btn btn-ghost" onclick={removeFrame}>{t("story.world.delete")}</button>
  </header>
  <Field label={t("story.world.displayName")}>
    <input class="input" value={frame.name ?? ""} onchange={(e) => setText(frame, "name", value(e))} />
  </Field>

  <section class="blk">
    <div class="blk-h"><span class="blk-t">{t("story.frames.look")}</span></div>
    <div class="blk-b">
      <Field label={t("story.frames.body")}>
        <FilePick
          value={frame.body?.file ?? ""}
          options={story.media}
          {into}
          kinds={PICTURES}
          emptyLabel={t("story.frames.pitBoxLook")}
          onpick={setBody}
        />
      </Field>
      {#if frame.body}
        {@const s = frame.body.screen}
        <div class="screen lbl-key">{t("story.frames.screen")}</div>
        <div class="row">
          {#each [["x", s.x], ["y", s.y], ["width", s.width], ["height", s.height], ["radius", s.radius ?? 0]] as [key, n] (key)}
            <label class="num">
              <span class="lbl-key">{t(`story.frames.screenPart.${key}`)}</span>
              <NumberStepper value={Number(n)} min={0} max={10000} width={90} onchange={(v) => setScreen(key as "x", v)} />
            </label>
          {/each}
        </div>
      {/if}
      <Field label={t("story.frames.wallpaper")}>
        <FilePick
          value={frame.wallpaper ?? ""}
          options={story.media}
          {into}
          kinds={WALLPAPERS}
          emptyLabel={t("story.frames.pitBoxLook")}
          onpick={(f) => setText(frame, "wallpaper", f)}
        />
      </Field>
      <Field label={t("story.frames.statusBar")}>
        <Seg
          items={statusBars}
          value={frame.status_bar ?? "full"}
          onselect={(v) => {
            // A full bar is what a phone has when nothing is said: not written.
            if (v === "full") delete frame.status_bar;
            else frame.status_bar = v as "simple";
            save();
          }}
        />
      </Field>
      <div class="pair">
        <Field label={t("story.frames.soundTap")}>
          <div class="sound">
            <FilePick
              value={frame.sounds?.tap ?? ""}
              options={story.media}
              {into}
              kinds={SOUNDS}
              emptyLabel={t("story.frames.noSound")}
              onpick={(f) => setSound("tap", f)}
            />
            <SoundPlay folder={story.path} path={frame.sounds?.tap ?? ""} />
          </div>
        </Field>
        <Field label={t("story.frames.soundSend")}>
          <div class="sound">
            <FilePick
              value={frame.sounds?.send ?? ""}
              options={story.media}
              {into}
              kinds={SOUNDS}
              emptyLabel={t("story.frames.noSound")}
              onpick={(f) => setSound("send", f)}
            />
            <SoundPlay folder={story.path} path={frame.sounds?.send ?? ""} />
          </div>
        </Field>
      </div>
    </div>
  </section>

  <section class="blk">
    <div class="blk-h">
      <span class="blk-t">{t("story.frames.apps")}</span>
      <span class="blk-n">{frame.apps.length}</span>
    </div>
    <div class="blk-b">
      <ol class="apps">
        {#each frame.apps as app, i (app.id + ":" + i)}
          <li class="app">
            <div class="line">
              <button
                class="btn btn-ghost mv"
                disabled={i === 0}
                aria-label={t("story.scenes.moveUp")}
                onclick={() => {
                  frame.apps = moveItem(frame.apps, i, -1);
                  save();
                }}>↑</button
              >
              <button
                class="btn btn-ghost mv"
                disabled={i === frame.apps.length - 1}
                aria-label={t("story.scenes.moveDown")}
                onclick={() => {
                  frame.apps = moveItem(frame.apps, i, 1);
                  save();
                }}>↓</button
              >
              <span class="mono app-id">{app.id}</span>
              <input
                class="input name"
                value={app.name ?? ""}
                placeholder={t("story.world.displayName")}
                onchange={(e) => setText(app, "name", value(e))}
              />
              <button class="btn btn-ghost del" aria-label={t("story.world.delete")} onclick={() => removeApp(i)}>✕</button>
            </div>
            <div class="pair">
              <Field label={t("story.frames.template")}>
                <select
                  class="input"
                  value={app.template}
                  onchange={(e) => {
                    const tp = appTemplates.find((x) => x.name === value(e));
                    if (tp) setTemplate(app, tp);
                    save();
                  }}
                >
                  {#each appTemplates as tp (tp.name)}<option value={tp.name}>{t(`story.frames.templates.${tp.name}`)}</option>{/each}
                </select>
              </Field>
              <Field label={t("story.frames.appears")}>
                <select
                  class="input"
                  value={app.appears ?? "first_item"}
                  onchange={(e) => {
                    const v = value(e);
                    if (v === "first_item") delete app.appears;
                    else app.appears = v as "always";
                    save();
                  }}
                >
                  {#each appearances as a (a.value)}<option value={a.value}>{a.label}</option>{/each}
                </select>
              </Field>
            </div>
            <div class="shows">
              <span class="lbl-key">{t("story.frames.shows")}</span>
              {#each typesFor(frame, app, appTemplates) as kind (kind)}
                <label class="check">
                  <input
                    type="checkbox"
                    checked={app.shows.includes(kind)}
                    onchange={(e) => toggleType(app, kind, (e.currentTarget as HTMLInputElement).checked)}
                  />
                  {t(CONTENT_TYPE_LABEL[kind as ContentType])}
                </label>
              {/each}
            </div>
            <div class="pair">
              <Field label={t("story.frames.icon")}>
                <FilePick
                  value={app.icon ?? ""}
                  options={story.media}
                  into={`${into}/icons`}
                  kinds={PICTURES}
                  emptyLabel={t("story.frames.pitBoxLook")}
                  onpick={(f) => setText(app, "icon", f)}
                />
              </Field>
              <Field label={t("story.content.skin")}>
                <TargetSelect
                  value={app.skin ?? ""}
                  options={skinsFor(story.skins, app.template)}
                  emptyLabel={t("story.content.noSkin")}
                  onchange={(v) => setText(app, "skin", v)}
                />
              </Field>
            </div>
          </li>
        {/each}
      </ol>
      <AddName placeholder={t("story.frames.newApp")} onadd={addApp} />
    </div>
  </section>
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
  .screen {
    margin: 8px 0 4px;
  }
  .row,
  .pair {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 16px;
    align-items: flex-end;
  }
  /* A sound and the button to listen to it, on one line. */
  .sound {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .sound :global(.pick) {
    flex: 1;
    min-width: 0;
  }
  .num {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .apps {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
  }
  .app {
    padding: 10px 0;
    border-top: 1px solid var(--line);
  }
  .app:first-child {
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
  .app-id {
    width: 110px;
    font-size: 12px;
    color: var(--txt2);
  }
  .name {
    flex: 1;
    min-width: 0;
  }
  .del {
    padding: 4px 8px;
    color: var(--muted);
  }
  .shows {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    margin: 6px 0;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
</style>
