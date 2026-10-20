<script lang="ts">
  // One level of a conversation in the Form view (SPEC-story-mode, Dialogue
  // text): its lines, effects, choice points and end, in playing order, and
  // under each choice the level it leads to - this same component, one level
  // deeper. Every edit goes up through `onchange`, which writes the whole
  // tree back as Ink: a gather the tree lacks after a choice point is the
  // engine's to write, so none is offered here.
  import { t } from "$lib/i18n/index.svelte";
  import { moveItem } from "$lib/story/flow";
  import { conditionsText, freshLabel, newItem, parseConditions, type FormContext } from "$lib/story/script";
  import type { ScriptItem } from "$lib/story/studio";
  import ScriptItems from "./ScriptItems.svelte";

  interface Props {
    items: ScriptItem[];
    ctx: FormContext;
    onchange: () => void;
  }
  const { items, ctx, onchange }: Props = $props();

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLSelectElement).value;
  }

  function move(i: number, delta: number) {
    const moved = moveItem(items, i, delta);
    items.splice(0, items.length, ...moved);
    onchange();
  }

  function remove(i: number) {
    items.splice(i, 1);
    onchange();
  }

  function add(kind: "line" | "effect" | "choices" | "end") {
    items.push(newItem(kind, ctx.root));
    onchange();
  }

  /** Sets or removes an optional key of `target`: an empty field writes nothing. */
  function setOpt<T extends object, K extends keyof T>(target: T, key: K, v: string) {
    if (v.trim()) (target as Record<K, string>)[key] = v.trim();
    else delete target[key];
    onchange();
  }

  function setWhen(target: { when?: string[] }, text: string) {
    const when = parseConditions(text);
    if (when.length) target.when = when;
    else delete target.when;
    onchange();
  }

  /** The expressions a line can take: its speaker's portraits, or the
   * sender's for a line without one. */
  function expressionsOf(speaker: string | undefined): string[] {
    const who = speaker ?? ctx.from;
    return ctx.characters.find((c) => c.id === who)?.expressions ?? [];
  }
</script>

{#snippet tools(i: number)}
  <span class="tools">
    <button class="btn btn-ghost" disabled={i === 0} aria-label={t("story.content.moveUp")} onclick={() => move(i, -1)}
      >↑</button
    >
    <button
      class="btn btn-ghost"
      disabled={i === items.length - 1}
      aria-label={t("story.content.moveDown")}
      onclick={() => move(i, 1)}>↓</button
    >
    <button class="btn btn-ghost" aria-label={t("story.world.delete")} onclick={() => remove(i)}>✕</button>
  </span>
{/snippet}

<ol class="items">
  {#each items as item, i (i)}
    <li class="item {item.kind}">
      {#if item.kind === "line"}
        <div class="row">
          <select
            class="input who"
            value={item.speaker ?? ""}
            aria-label={t("story.content.speaker")}
            onchange={(e) => setOpt(item, "speaker", value(e))}
          >
            <option value="">{t("story.content.speakerSender")}</option>
            {#if item.speaker && !ctx.characters.some((c) => c.id === item.speaker)}
              <option value={item.speaker}>{item.speaker}</option>
            {/if}
            {#each ctx.characters as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
          </select>
          <input
            class="input said"
            value={item.text}
            aria-label={t("story.content.lineText")}
            onchange={(e) => {
              item.text = value(e).trim();
              onchange();
            }}
          />
          {@render tools(i)}
        </div>
        <div class="row sub">
          <input
            class="input mono cond"
            value={conditionsText(item.when)}
            placeholder={t("story.content.condition")}
            aria-label={t("story.content.condition")}
            onchange={(e) => setWhen(item, value(e))}
          />
          {#if ctx.dialogue}
            {@const expressions = expressionsOf(item.speaker)}
            <select
              class="input mono"
              value={item.expression ?? ""}
              aria-label={t("story.content.expression")}
              onchange={(e) => setOpt(item, "expression", value(e))}
            >
              <option value="">{t("story.content.expressionNone")}</option>
              {#if item.expression && !expressions.includes(item.expression)}
                <option value={item.expression}>{item.expression}</option>
              {/if}
              {#each expressions as x (x)}<option value={x}>{x}</option>{/each}
            </select>
            <select
              class="input"
              value={item.position ?? ""}
              aria-label={t("story.content.position")}
              onchange={(e) => setOpt(item, "position", value(e))}
            >
              <option value="">{t("story.content.positionNone")}</option>
              <option value="left">{t("story.content.positionLeft")}</option>
              <option value="center">{t("story.content.positionCenter")}</option>
              <option value="right">{t("story.content.positionRight")}</option>
            </select>
          {/if}
        </div>
      {:else if item.kind === "effect"}
        <div class="row">
          <span class="lbl-key">{t("story.content.effect")}</span>
          <input
            class="input mono said"
            value={item.effect}
            placeholder="rel.friend += 1"
            aria-label={t("story.content.effect")}
            onchange={(e) => {
              item.effect = value(e).trim();
              onchange();
            }}
          />
          {@render tools(i)}
        </div>
      {:else if item.kind === "choices"}
        <div class="row">
          <span class="lbl-key">{t("story.content.choices")}</span>
          {@render tools(i)}
        </div>
        {#each item.choices as c, k (k)}
          <div class="choice">
            <div class="row">
              <input
                class="input mono label"
                value={c.label}
                aria-label={t("story.content.label")}
                onchange={(e) => {
                  c.label = value(e).trim();
                  onchange();
                }}
              />
              <input
                class="input said"
                value={c.text}
                aria-label={t("story.content.choiceText")}
                onchange={(e) => {
                  c.text = value(e).trim();
                  onchange();
                }}
              />
              <button
                class="btn btn-ghost"
                aria-label={t("story.world.delete")}
                onclick={() => {
                  item.choices.splice(k, 1);
                  if (item.choices.length === 0) items.splice(i, 1);
                  onchange();
                }}>✕</button
              >
            </div>
            <div class="row sub">
              <input
                class="input mono cond"
                value={conditionsText(c.when)}
                placeholder={t("story.content.condition")}
                aria-label={t("story.content.condition")}
                onchange={(e) => setWhen(c, value(e))}
              />
            </div>
            <div class="nested">
              <ScriptItems items={c.body} {ctx} {onchange} />
            </div>
          </div>
        {/each}
        <button
          class="btn btn-ghost more"
          onclick={() => {
            item.choices.push({ label: freshLabel(ctx.root), text: "", body: [] });
            onchange();
          }}>+ {t("story.content.addChoice")}</button
        >
      {:else if item.kind === "gather"}
        <div class="row quiet">
          <span class="lbl-key">{t("story.content.gather")}</span>
          {@render tools(i)}
        </div>
      {:else}
        <div class="row quiet">
          <span class="lbl-key">{t("story.content.end")}</span>
          {@render tools(i)}
        </div>
      {/if}
    </li>
  {/each}
</ol>
<div class="add">
  <button class="btn btn-ghost" onclick={() => add("line")}>+ {t("story.content.addLine")}</button>
  <button class="btn btn-ghost" onclick={() => add("choices")}>+ {t("story.content.addChoices")}</button>
  <button class="btn btn-ghost" onclick={() => add("effect")}>+ {t("story.content.addEffect")}</button>
  <button class="btn btn-ghost" onclick={() => add("end")}>+ {t("story.content.addEnd")}</button>
</div>

<style>
  .items {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .item {
    padding: 6px 0;
    border-top: 1px solid var(--line);
  }
  .item:first-child {
    border-top: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .row.sub {
    margin-top: 4px;
  }
  .row.quiet .lbl-key {
    color: var(--muted);
  }
  .who {
    width: 150px;
    flex-shrink: 0;
  }
  .said {
    flex: 1;
    min-width: 0;
  }
  .cond {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
  }
  .label {
    width: 130px;
    flex-shrink: 0;
  }
  .tools {
    display: flex;
    flex-shrink: 0;
    margin-left: auto;
  }
  .tools .btn {
    padding: 4px 6px;
    color: var(--muted);
  }
  .choice {
    margin: 6px 0 0 18px;
    padding-left: 10px;
    box-shadow: inset 2px 0 0 var(--line);
  }
  .nested {
    margin-top: 4px;
  }
  .more {
    margin: 4px 0 0 18px;
  }
  .add {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }
</style>
