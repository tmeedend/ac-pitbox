<script lang="ts">
  // How to write effects (SPEC-story-mode, Variables; Engine file format):
  // one sentence per line, the forms the engine reads, and the names this
  // story declares - the ones a sentence can cite. The examples are syntax,
  // the same in every language; only what they do is translated.
  import { t } from "$lib/i18n/index.svelte";
  import { studio } from "$lib/story/studioState.svelte";
  import HelpNote from "./HelpNote.svelte";

  interface Props {
    /** The note also says how story time moves (`advance`). */
    advance?: boolean;
  }
  const { advance = false }: Props = $props();

  const story = $derived(studio.story!);
  const money = $derived(story.manifest.value?.settings.money ?? false);
  const flags = $derived(story.variables.value?.flags ?? []);
  const counters = $derived(Object.keys(story.variables.value?.counters ?? {}));
  const characters = $derived(story.characters.value?.characters.map((c) => c.id) ?? []);
  const steps = $derived(story.manifest.value?.settings.relations.scale ?? []);
  const sendable = $derived(story.content.filter((c) => c.placement === "flow").map((c) => c.id));

  const FORMS = [
    { code: "reputation += 2", key: "story.events.help.effectReputation" },
    { code: "counter.<name> += 1", key: "story.events.help.effectCounter" },
    { code: "flag.<name> = true", key: "story.events.help.effectFlag" },
    { code: "rel.<character> += 1", key: "story.events.help.effectRelation" },
    { code: "send <content>", key: "story.events.help.effectSend" },
  ];
</script>

<HelpNote title={t("story.events.help.effectsTitle")}>
  <p>{t("story.events.help.effectsIntro")}</p>
  <dl>
    {#each FORMS as f (f.code)}
      <dt><code>{f.code}</code></dt>
      <dd>{t(f.key)}</dd>
    {/each}
    {#if money}
      <dt><code>money -= 100</code></dt>
      <dd>{t("story.events.help.effectMoney")}</dd>
    {/if}
  </dl>
  {#if advance}
    <p>{t("story.events.help.advance")}</p>
  {/if}
  <div class="names">
    <div class="lbl-key">{t("story.events.help.inThisStory")}</div>
    {#snippet names(label: string, list: string[])}
      {#if list.length}
        <div class="line"><span class="what">{label}</span> <code>{list.join(", ")}</code></div>
      {/if}
    {/snippet}
    {@render names(t("story.world.counters"), counters)}
    {@render names(t("story.world.flags"), flags)}
    {@render names(t("story.world.characters"), characters)}
    {@render names(t("story.events.help.steps"), steps)}
    {@render names(t("story.events.help.sendable"), sendable)}
    {#if !counters.length && !flags.length && !characters.length}
      <div class="line none">{t("story.events.help.noNames")}</div>
    {/if}
  </div>
</HelpNote>

<style>
  dl {
    margin: 0 0 10px;
  }
  dt {
    margin-top: 6px;
  }
  dd {
    margin: 1px 0 0 12px;
    color: var(--muted);
  }
  .names {
    border-top: 1px solid var(--line);
    padding-top: 8px;
  }
  .line {
    margin-top: 4px;
  }
  .what {
    color: var(--muted);
  }
  .none {
    color: var(--faint);
  }
</style>
