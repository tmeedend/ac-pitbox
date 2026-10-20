<script lang="ts">
  // A character's portraits (SPEC-story-mode, In-person dialogues): one
  // picture per expression, picked among the images already in the story's
  // `media/` - the editor never copies a file there (docs/CHANTIERS.md keeps
  // "choose a file" for later). A picture that is not there, or no longer,
  // stays as written and says so; the checks report it too.
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { t } from "$lib/i18n/index.svelte";
  import { expressionsInOrder, nameProblem, portraitImages } from "$lib/story/world";
  import type { Character } from "$lib/story/studio";
  import ImageSelectDropdown from "$lib/components/ui/ImageSelectDropdown.svelte";
  import AddName from "./AddName.svelte";

  interface Props {
    character: Character;
    /** The files of `media/`, as paths inside it. */
    media: string[];
    /** The story folder, to show the pictures. */
    folder: string;
    onchange: () => void;
  }
  const { character, media, folder, onchange }: Props = $props();

  const expressions = $derived(expressionsInOrder(character.portraits));
  const images = $derived(portraitImages(media));
  const options = $derived(
    images.map((p) => ({ id: p, name: p, image: convertFileSrc(`${folder}/media/${p}`) })),
  );

  function set(expression: string, path: string) {
    character.portraits = { ...(character.portraits ?? {}), [expression]: path };
    onchange();
  }

  function remove(expression: string) {
    const rest = { ...character.portraits };
    delete rest[expression];
    if (Object.keys(rest).length) character.portraits = rest;
    else delete character.portraits;
    onchange();
  }

  /** A new expression starts with no picture: the checks show it until one
   * is picked, rather than the editor guessing which. */
  function add(name: string): string | null {
    const why = nameProblem(name, expressions);
    if (why) return t(why);
    set(name, "");
    return null;
  }
</script>

<div class="portraits">
  {#if !images.length}
    <p class="hint">{t("story.world.noImages")}</p>
  {/if}
  <ul>
    {#each expressions as expression (expression)}
      {@const path = character.portraits?.[expression] ?? ""}
      <li>
        <span class="mono expr">{expression}</span>
        <div class="pick">
          <ImageSelectDropdown
            {options}
            selectedId={path || null}
            placeholder={t("story.world.pickPortrait")}
            emptyText={t("story.world.noImagesShort")}
            onselect={(p) => set(expression, p)}
          />
        </div>
        {#if path && !media.includes(path)}
          <span class="missing" title={`media/${path}`}>{t("story.world.portraitMissing")}</span>
        {/if}
        <button class="btn btn-ghost del" onclick={() => remove(expression)} aria-label={t("story.world.delete")}
          >✕</button
        >
      </li>
    {/each}
  </ul>
  <div class="add">
    {#if !expressions.includes("neutral")}
      <button class="btn" onclick={() => add("neutral")}>{t("story.world.addExpression", { name: "neutral" })}</button>
    {/if}
    <AddName placeholder={t("story.world.newExpression")} onadd={add} />
  </div>
</div>

<style>
  .portraits {
    margin: 6px 0 2px;
    padding: 8px 10px;
    background: var(--raised);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 3px 0;
  }
  .expr {
    width: 110px;
    flex-shrink: 0;
    font-size: 12px;
  }
  .pick {
    width: 260px;
  }
  .missing {
    color: var(--yellow);
    font-size: 12px;
  }
  .del {
    margin-left: auto;
    padding: 4px 8px;
    color: var(--muted);
  }
  .hint {
    margin: 0 0 6px;
    color: var(--muted);
    font-size: 12px;
  }
  .add {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-top: 6px;
  }
</style>
