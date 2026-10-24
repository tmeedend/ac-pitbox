<script lang="ts">
  // The Content screen's live preview (SPEC-story-mode, Create Your Own:
  // Content): the item played by the engine on its own, shown by the very
  // views Story Mode uses - the phone and the template of the app showing the
  // item (thread, inbox, feed, statement), or the dialogue stage - in front
  // of its own place when it has one, silent.
  // It follows every save (`studio.revision`). The author sets what the item
  // reads to preview with, chooses replies, and sees what the conversation
  // changed. An item no frame can show shows the checks' warning instead.
  import { untrack } from "svelte";
  import { errorText } from "$lib/errors";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { setupSentences } from "$lib/story/preview";
  import { inputsOf } from "$lib/story/preview.svelte";
  import { appOf, APP_LABEL } from "$lib/story/player/apps";
  import { dateLabel, whenLabel } from "$lib/story/player/phone";
  import { mediaUrl, speakersOf } from "$lib/story/player/speakers";
  import { previewContent, type PreviewView } from "$lib/story/studio";
  import { studio } from "$lib/story/studioState.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import DialogueStage from "./player/DialogueStage.svelte";
  import IllustrationView from "./player/IllustrationView.svelte";
  import FeedView from "./player/FeedView.svelte";
  import MailView from "./player/MailView.svelte";
  import PhoneFrame from "./player/PhoneFrame.svelte";
  import SceneView from "./player/SceneView.svelte";
  import StatementView from "./player/StatementView.svelte";
  import ThreadView from "./player/ThreadView.svelte";

  interface Props {
    /** The content item shown; the column is made again for another. */
    id: string;
  }
  const { id }: Props = $props();

  const story = $derived(studio.story!);
  // Made once per item, outside any derivation: it writes the inputs table.
  // The column is made again for another item (`{#key}` in the screen), so
  // the first `id` is the one it shows for its whole life.
  const inputs = untrack(() => inputsOf(id));
  let view = $state<PreviewView | null>(null);
  let problem = $state<string | null>(null);

  $effect(() => {
    const [revision, setup, replies, path] = [
      studio.revision,
      setupSentences(inputs.setup),
      [...inputs.replies],
      story.path,
    ];
    void revision;
    let current = true;
    previewContent(path, id, setup, replies)
      .then((v) => {
        if (!current) return;
        view = v;
        problem = null;
      })
      .catch((e) => {
        if (current) problem = errorText(e);
      });
    return () => {
      current = false;
    };
  });

  const speakers = $derived(speakersOf(story.characters.value?.characters ?? [], story.path));
  const names = $derived(Object.fromEntries(Object.entries(speakers).map(([k, s]) => [k, s.name])));
  const scale = $derived(story.manifest.value?.settings.relations.scale ?? []);
  const dateDisplay = $derived(story.manifest.value?.settings.date ?? "none");
  const dateShown = $derived(view ? dateLabel(view.date, dateDisplay, t, i18n.locale) : "");
  const when = $derived(view ? whenLabel(view.date, view.clock, dateDisplay, t, i18n.locale) : "");
  const app = $derived(view ? appOf(view.kind) : null);
  const sender = $derived(view?.from ? (names[view.from] ?? view.from) : t("story.play.unknownSender"));

  function choose(label: string) {
    inputs.replies.push(label);
  }

  function set(variable: string, value: string, start: string) {
    if (value === start) delete inputs.setup[variable];
    else inputs.setup[variable] = value;
    inputs.replies = [];
  }
</script>

<aside class="preview">
  <div class="head">
    <span class="lbl">{t("story.preview.title")}</span>
    <button class="btn btn-ghost" disabled={!inputs.replies.length} onclick={() => (inputs.replies = [])}>
      {t("story.preview.restart")}
    </button>
  </div>

  {#if problem}
    <p class="errbox">{t("story.preview.unavailable", { reason: problem })}</p>
  {:else if view}
    {#if view.view === "nowhere"}
      <p class="warnbox">{view.nowhere}</p>
    {:else if view.view === "scene" && view.kind === "illustration"}
      <IllustrationView picture={mediaUrl(story.path, view.picture)} steps={view.steps} />
    {:else if view.view === "scene"}
      <div class="placed" class:framed={view.scene !== null}>
        {#if view.scene}
          <SceneView scene={view.scene} folder={story.path} staged transition={view.transition} />
        {/if}
        <DialogueStage
          steps={view.steps}
          choices={view.choices}
          present={view.characters}
          transition={view.transition}
          from={view.from}
          {speakers}
          ended={view.ended}
          bare={view.scene !== null}
          onchoose={choose}
        />
      </div>
    {:else}
      <div class="device">
        <PhoneFrame
          clock={view.clock}
          date={dateShown}
          title={app === "messages" || app === null ? (view.from ? sender : "—") : t(APP_LABEL[app])}
          avatar={app === "messages" && view.from ? (speakers[view.from]?.portraits.neutral ?? null) : null}
        >
          {#if app === "messages"}
            <ThreadView steps={view.steps} choices={view.choices} from={view.from} {names} onchoose={choose} />
          {:else if app === "mail"}
            <MailView
              {sender}
              {when}
              steps={view.steps}
              choices={view.choices}
              from={view.from}
              {names}
              onchoose={choose}
            />
          {:else if app === "news" || app === "social"}
            <FeedView
              posts={[
                {
                  key: id,
                  from: view.from,
                  author: sender,
                  when,
                  steps: view.steps,
                  choices: view.choices,
                },
              ]}
              {speakers}
              onchoose={(_, label) => choose(label)}
            />
          {:else if app === "bank"}
            <StatementView lines={[{ key: id, when, steps: view.steps }]} balance={null} />
          {:else}
            <p class="soon">{t("story.preview.noTemplate")}</p>
          {/if}
        </PhoneFrame>
      </div>
    {/if}

    {#if view.changes.length || view.sent.length}
      <div class="after">
        {#each view.changes as c (c.var)}
          <div class="mono change">{t("story.preview.change", { var: c.var, from: c.from, to: c.to })}</div>
        {/each}
        {#if view.sent.length}
          <div class="sent">{t("story.preview.sent", { items: view.sent.join(", ") })}</div>
        {/if}
      </div>
    {/if}

    {#if view.knobs.length}
      <div class="knobs">
        <div class="head">
          <span class="lbl">{t("story.preview.setWith")}</span>
          <button
            class="btn btn-ghost"
            disabled={!Object.keys(inputs.setup).length}
            onclick={() => {
              inputs.setup = {};
              inputs.replies = [];
            }}>{t("story.preview.reset")}</button
          >
        </div>
        {#each view.knobs as k (k.var)}
          {@const value = inputs.setup[k.var] ?? k.start}
          <label class="knob">
            <span class="mono var">{k.var}</span>
            {#if k.kind === "relation"}
              <select class="input mono" {value} onchange={(e) => set(k.var, e.currentTarget.value, k.start)}>
                {#each scale as step (step)}<option value={step}>{step}</option>{/each}
              </select>
            {:else if k.kind === "flag"}
              <input
                type="checkbox"
                checked={value === "true"}
                onchange={(e) => set(k.var, String(e.currentTarget.checked), k.start)}
              />
            {:else}
              <NumberStepper value={Number(value)} width={90} onchange={(v) => set(k.var, String(v), k.start)} />
            {/if}
          </label>
        {/each}
      </div>
    {/if}
  {/if}
</aside>

<style>
  .preview {
    width: 400px;
    flex-shrink: 0;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  /* A dialogue in front of its place: the scene drawn under the bare stage,
     the stage's own border kept around both. */
  .placed {
    position: relative;
  }
  .placed.framed {
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .device {
    display: flex;
    justify-content: center;
  }
  .soon {
    margin: 20px 14px;
    color: var(--muted);
    font-size: 12px;
  }
  .after {
    margin-top: 10px;
    font-size: 11.5px;
    color: var(--txt2);
  }
  .sent {
    margin-top: 4px;
    color: var(--muted);
  }
  .knobs {
    margin-top: 14px;
  }
  .knob {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 3px 0;
  }
  .var {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    color: var(--txt2);
  }
  .warnbox,
  .errbox {
    margin: 0;
  }
</style>
