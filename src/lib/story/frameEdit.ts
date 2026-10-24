// The Frame screen's logic, pure (SPEC-story-mode, Skins: how a frame gets
// its look): the story's phones and their apps (`frames/frames.yaml`), its
// skins (`frames/skins/<id>/skin.yaml`). What a template shows and which
// slots it has come from the engine (`OpenStory.templates`), never from a
// table of the frontend's own. Whether the story is right is still the
// engine's validator, run after every save.
import type { AppShown, PhoneLook, SeenItem, SkinShown } from "./play";
import type { PlayerStep } from "./player/types";
import type { EditedFile, FrameApp, FrameDecl, SkinFile, TemplateInfo } from "./studio";

export const SKINS = "frames/skins/";

/** A skin's id from its `skin.yaml`'s path; `null` for another file. */
export function skinIdOf(file: string): string | null {
  const m = /^frames\/skins\/([^/]+)\/skin\.yaml$/.exec(file);
  return m ? m[1] : null;
}

export function skinFile(id: string): string {
  return `${SKINS}${id}/skin.yaml`;
}

/** The skins a piece of `template` can wear, by id. */
export function skinsFor(skins: EditedFile<SkinFile>[], template: string | null): string[] {
  return skins
    .filter((s) => s.value?.template === template)
    .map((s) => skinIdOf(s.file))
    .filter((id): id is string => id !== null);
}

/** A new phone: Pit Box's look, and a messaging app to start with. */
export function newFrame(id: string, name: string): FrameDecl {
  return {
    id,
    kind: "phone",
    name,
    apps: [{ id: "messages", template: "thread", shows: ["message"], appears: "always" }],
  };
}

/** A new app of `template`, showing every type it can that no other app of
 * `frame` shows already - a type goes to one app. */
export function newApp(frame: FrameDecl, id: string, template: TemplateInfo): FrameApp {
  const taken = new Set(frame.apps.flatMap((a) => a.shows));
  return { id, template: template.name, shows: template.shows.filter((k) => !taken.has(k)) };
}

/** The types `app` can show in `frame`: those of its template that no other
 * app of the frame shows. */
export function typesFor(frame: FrameDecl, app: FrameApp, templates: TemplateInfo[]): string[] {
  const own = templates.find((t) => t.name === app.template)?.shows ?? [];
  const elsewhere = new Set(frame.apps.filter((a) => a !== app).flatMap((a) => a.shows));
  return own.filter((k) => !elsewhere.has(k));
}

/** `app` as `template`: it keeps only the types the new template shows,
 * and drops a skin made for the old one. */
export function setTemplate(app: FrameApp, template: TemplateInfo): void {
  if (app.template === template.name) return;
  app.template = template.name;
  app.shows = app.shows.filter((k) => template.shows.includes(k));
  if (!app.shows.length) app.shows = [...template.shows];
  delete app.skin;
}

/** The files of a skin's own folder, by name: what its slots and font can be. */
export function filesOfSkin(skinFiles: string[], id: string): string[] {
  const prefix = `${SKINS}${id}/`;
  return skinFiles.filter((f) => f.startsWith(prefix)).map((f) => f.slice(prefix.length));
}

/** Sets a slot's file, or removes the slot when `file` is empty: a slot
 * left out keeps Pit Box's look. */
export function setSlot(skin: SkinFile, slot: string, file: string): void {
  const slots = { ...(skin.slots ?? {}) };
  if (file) slots[slot] = { ...slots[slot], file };
  else delete slots[slot];
  if (Object.keys(slots).length) skin.slots = slots;
  else delete skin.slots;
}

/** A slot's four stretch margins (top, right, bottom, left), or none: the
 * picture is then stretched whole. */
export function setSlice(skin: SkinFile, slot: string, slice: [number, number, number, number] | null): void {
  const entry = skin.slots?.[slot];
  if (!entry) return;
  if (slice && slice.some((n) => n > 0)) entry.slice = slice;
  else delete entry.slice;
}

/** Sets a colour, or removes it when empty: it keeps Pit Box's then. */
export function setColor(skin: SkinFile, name: string, value: string): void {
  const colors = { ...(skin.colors ?? {}) };
  if (value) colors[name] = value;
  else delete colors[name];
  if (Object.keys(colors).length) skin.colors = colors;
  else delete skin.colors;
}

/** `frame` as the preview draws it: its look, with the status bar as a
 * story starts it (`story_engine::view::phone_look`: full battery, full
 * signal, Wi-Fi on). */
export function lookOf(frame: FrameDecl): PhoneLook {
  return {
    body: frame.body ?? null,
    wallpaper: frame.wallpaper ?? null,
    status_bar: frame.status_bar ?? "full",
    status: { date: null, battery: 100, signal: 4, wifi: true, airplane: false },
    sounds: frame.sounds ?? {},
  };
}

/** Skin `id` as the preview draws it: its files as paths inside the story
 * (`story_engine::view::skin_shown`). */
export function skinShownOf(id: string, skin: SkinFile): SkinShown {
  const inside = (file: string) => `${SKINS}${id}/${file}`;
  const slots = Object.entries(skin.slots ?? {}).map(([name, s]) => [name, { file: inside(s.file), slice: s.slice ?? null }]);
  return { id, colors: skin.colors ?? {}, font: skin.font ? inside(skin.font) : null, slots: Object.fromEntries(slots) };
}

/** An app of `frame` as the preview's phone shows it: on the home screen,
 * in its own skin if the story has it. */
export function appShownOf(app: FrameApp, skins: EditedFile<SkinFile>[]): AppShown {
  const worn = app.skin ? skins.find((s) => skinIdOf(s.file) === app.skin && s.value?.template === app.template) : null;
  return {
    id: app.id,
    name: app.name ?? null,
    icon: app.icon ?? null,
    template: app.template as AppShown["template"],
    shows: app.shows,
    available: true,
    skin: worn?.value ? skinShownOf(app.skin!, worn.value) : null,
  };
}

/** The type a template's sample shows: the first it can. */
export function sampleKind(template: TemplateInfo | undefined): string {
  return template?.shows[0] ?? "message";
}

/** One sample item of content `kind`, saying `lines` (texts the screen
 * translated), the last waiting for `reply` when there is one: what the
 * preview fills an app or a scene skin with. */
export function sampleItem(kind: string, lines: string[], reply: string | null): SeenItem {
  return {
    content: `sample_${kind}`,
    kind: kind as SeenItem["kind"],
    from: null,
    characters: [],
    picture: null,
    transition: "fade",
    scene: null,
    staged_as: null,
    skin: null,
    date: "2001-03-01",
    clock: "09:00",
    steps: lines.map((text): PlayerStep => ({ kind: "said", speaker: null, text, expression: null, position: null })),
    choices: reply ? [{ label: "reply", text: reply }] : [],
    open: reply !== null,
    seen: false,
  };
}
