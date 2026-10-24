<script lang="ts">
  // The Frame screen's preview (SPEC-story-mode, Create Your Own: the Frame
  // screen): what is edited, as the player sees it, filled with sample
  // content. A phone is the game's own phone (`PhoneScreen`) with the
  // frame's look and apps, each app holding a sample of what it shows. A
  // skin is worn by what it dresses: an app of its template on Pit Box's
  // phone, or what plays on the scene in its way. It follows each edit
  // before it is saved; nothing here is written. The phone's sounds play
  // as the author taps through it, whatever the game's sound button says.
  import { t } from "$lib/i18n/index.svelte";
  import { appShownOf, lookOf, sampleItem, sampleKind, skinShownOf } from "$lib/story/frameEdit";
  import { mediaUrl } from "$lib/story/player/speakers";
  import { studio } from "$lib/story/studioState.svelte";
  import type { FrameDecl, SkinFile } from "$lib/story/studio";
  import type { AppShown } from "$lib/story/play";
  import DialogueStage from "./player/DialogueStage.svelte";
  import DocumentView from "./player/DocumentView.svelte";
  import SceneSkin from "./player/SceneSkin.svelte";
  import TvView from "./player/TvView.svelte";
  import PhoneScreen from "./PhoneScreen.svelte";

  interface Props {
    /** The phone shown; `null` for a skin. */
    frame: FrameDecl | null;
    skin: { id: string; value: SkinFile } | null;
  }
  const { frame, skin }: Props = $props();

  const story = $derived(studio.story!);
  const template = $derived(skin ? story.templates.find((tp) => tp.name === skin.value.template) : undefined);

  /** The sample lines of content `kind`, in the screen's language. */
  function lines(kind: string): string[] {
    return [t(`story.frames.sample.${kind}`), t("story.frames.sample.second")];
  }

  /** The apps of the phone shown: the frame's, or one wearing the skin. */
  const apps = $derived.by((): AppShown[] => {
    if (frame) return frame.apps.map((a) => appShownOf(a, story.skins));
    if (!skin || !template?.app) return [];
    return [
      {
        id: "sample",
        name: null,
        icon: null,
        template: skin.value.template as AppShown["template"],
        shows: template.shows,
        available: true,
        skin: skinShownOf(skin.id, skin.value),
      },
    ];
  });
  const items = $derived(
    apps.flatMap((a) => a.shows.slice(0, 1)).map((kind, i) => sampleItem(kind, lines(kind), i === 0 ? t("story.frames.sample.reply") : null)),
  );
  const sceneSkin = $derived(skin && template && !template.app ? skinShownOf(skin.id, skin.value) : null);
  const sceneLines = $derived(lines(sampleKind(template)));
  const sceneSteps = $derived(sampleItem(sampleKind(template), sceneLines, null).steps);
</script>

<aside class="preview">
  <div class="lbl">{t("story.preview.title")}</div>
  {#if apps.length}
    {#key apps}
      <div class="device">
        <PhoneScreen
          {apps}
          folder={story.path}
          look={frame ? lookOf(frame) : null}
          muted={false}
          {items}
          clock="09:41"
          date=""
          dateDisplay="none"
          speakers={{}}
          money={null}
          onreply={() => {}}
          onseen={() => {}}
        />
      </div>
    {/key}
  {:else if sceneSkin && template}
    <SceneSkin skin={sceneSkin} folder={story.path}>
      {#if template.name === "newspaper" || template.name === "letter"}
        <DocumentView
          layout={template.name}
          from={t("story.frames.sample.sender")}
          picture={mediaUrl(story.path, null)}
          steps={sceneSteps}
        />
      {:else if template.name === "tv"}
        <TvView channel={t("story.frames.sample.sender")} picture={null} steps={sceneSteps} />
      {:else}
        <DialogueStage
          steps={sceneSteps}
          choices={[{ label: "reply", text: t("story.frames.sample.reply") }]}
          present={[]}
          from={null}
          speakers={{}}
          ended={false}
          onchoose={() => {}}
        />
      {/if}
    </SceneSkin>
  {/if}
</aside>

<style>
  .preview {
    width: 520px;
    flex-shrink: 0;
  }
  .device {
    display: flex;
    justify-content: center;
    margin-top: 8px;
  }
</style>
