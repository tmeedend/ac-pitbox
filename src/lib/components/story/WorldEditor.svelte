<script lang="ts">
  // The World screen (SPEC-story-mode, Create Your Own; Variables): the
  // story's characters, flags and counters - `world/characters.yaml` and
  // `world/variables.yaml`. Each edit saves its file at once; a name renamed
  // takes every sentence citing it along (NameRow).
  import { t } from "$lib/i18n/index.svelte";
  import { nameFrom, nameProblem, variableNames } from "$lib/story/world";
  import { saveStoryFile, studio } from "$lib/story/studioState.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import AddName from "./AddName.svelte";
  import FileStatus from "./FileStatus.svelte";
  import NameRow from "./NameRow.svelte";
  import StoryChecks from "./StoryChecks.svelte";

  const story = $derived(studio.story!);
  const characters = $derived(story.characters.value);
  const variables = $derived(story.variables.value);
  const scale = $derived(story.manifest.value?.settings.relations ?? null);
  const characterIds = $derived(characters?.characters.map((c) => c.id) ?? []);
  const variableIds = $derived(variables ? variableNames(variables) : []);

  function saveCharacters() {
    void saveStoryFile("characters");
  }

  function saveVariables() {
    void saveStoryFile("variables");
  }

  function addCharacter(label: string): string | null {
    if (!characters) return null;
    const id = nameFrom(label, characterIds, "character");
    characters.characters.push({ id, name: label });
    saveCharacters();
    return null;
  }

  function addFlag(name: string): string | null {
    if (!variables) return null;
    const why = nameProblem(name, variableIds);
    if (why) return t(why);
    variables.flags = [...(variables.flags ?? []), name];
    saveVariables();
    return null;
  }

  function addCounter(name: string): string | null {
    if (!variables) return null;
    const why = nameProblem(name, variableIds);
    if (why) return t(why);
    variables.counters = { ...(variables.counters ?? {}), [name]: 0 };
    saveVariables();
    return null;
  }

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLSelectElement).value;
  }
</script>

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.world.title")}</span>
  </header>
  {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}

  <section class="blk">
    <div class="blk-h">
      <span class="blk-t">{t("story.world.characters")}</span>
      <span class="blk-n">{characterIds.length}</span>
    </div>
    <div class="blk-b">
      <FileStatus entry={story.characters} />
      {#if characters}
        {#if characters.characters.length}
          <div class="cols lbl-key">
            <span class="c-name">{t("story.world.name")}</span>
            <span>{t("story.world.displayName")}</span>
            <span>{t("story.world.startRelation")}</span>
          </div>
        {/if}
        <ul class="rows">
          {#each characters.characters as c, i (c.id + ":" + i)}
            <NameRow
              kind="character"
              id={c.id}
              taken={characterIds}
              onremove={() => {
                characters.characters = characters.characters.filter((_, k) => k !== i);
                saveCharacters();
              }}
            >
              <input
                class="input display"
                value={c.name ?? ""}
                aria-label={t("story.world.displayName")}
                onchange={(e) => {
                  const name = value(e).trim();
                  if (name) c.name = name;
                  else delete c.name;
                  saveCharacters();
                }}
              />
              <select
                class="input mono relation"
                value={c.relation ?? ""}
                aria-label={t("story.world.startRelation")}
                onchange={(e) => {
                  const step = value(e);
                  if (step) c.relation = step;
                  else delete c.relation;
                  saveCharacters();
                }}
              >
                <option value="">{t("story.world.relationDefault", { step: scale?.start ?? "" })}</option>
                {#if c.relation && !scale?.scale.includes(c.relation)}
                  <option value={c.relation}>{c.relation}</option>
                {/if}
                {#each scale?.scale ?? [] as step (step)}
                  <option value={step}>{step}</option>
                {/each}
              </select>
            </NameRow>
          {/each}
        </ul>
        <AddName placeholder={t("story.world.newCharacter")} onadd={addCharacter} />
      {/if}
    </div>
  </section>

  <FileStatus entry={story.variables} />
  {#if variables}
    <section class="blk">
      <div class="blk-h">
        <span class="blk-t">{t("story.world.flags")}</span>
        <span class="blk-n">{variables.flags?.length ?? 0}</span>
      </div>
      <div class="blk-b">
        <ul class="rows">
          {#each variables.flags ?? [] as flag, i (flag + ":" + i)}
            <NameRow
              kind="flag"
              id={flag}
              taken={variableIds}
              onremove={() => {
                variables.flags = (variables.flags ?? []).filter((_, k) => k !== i);
                saveVariables();
              }}
            />
          {/each}
        </ul>
        <AddName placeholder={t("story.world.newFlag")} onadd={addFlag} />
      </div>
    </section>

    <section class="blk">
      <div class="blk-h">
        <span class="blk-t">{t("story.world.counters")}</span>
        <span class="blk-n">{Object.keys(variables.counters ?? {}).length}</span>
      </div>
      <div class="blk-b">
        {#if Object.keys(variables.counters ?? {}).length}
          <div class="cols lbl-key">
            <span class="c-name">{t("story.world.name")}</span>
            <span>{t("story.world.startValue")}</span>
          </div>
        {/if}
        <ul class="rows">
          {#each Object.entries(variables.counters ?? {}) as [name, start] (name)}
            <NameRow
              kind="counter"
              id={name}
              taken={variableIds}
              onremove={() => {
                const rest = { ...variables.counters };
                delete rest[name];
                variables.counters = rest;
                saveVariables();
              }}
            >
              <NumberStepper
                value={start}
                width={80}
                onchange={(v) => {
                  variables.counters = { ...(variables.counters ?? {}), [name]: v };
                  saveVariables();
                }}
              />
            </NameRow>
          {/each}
        </ul>
        <AddName placeholder={t("story.world.newCounter")} onadd={addCounter} />
      </div>
    </section>
  {/if}

  <StoryChecks section="world" />
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
  .blk {
    max-width: 820px;
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .cols {
    display: flex;
    gap: 10px;
    margin-bottom: 4px;
  }
  .cols span {
    width: 190px;
  }
  .display {
    width: 190px;
  }
  .relation {
    width: 170px;
  }
</style>
