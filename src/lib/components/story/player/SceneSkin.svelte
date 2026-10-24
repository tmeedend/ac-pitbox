<script lang="ts">
  // A skin worn by what plays on the scene (SPEC-story-mode, Skins: how a
  // frame gets its look): a dialogue, a document (newspaper, letter), a TV
  // report, a call on the landline. The same mechanism as the phone's
  // (`skins.ts`): colours as the app's tokens, slots laid as 9-slices over
  // the pieces they dress - by those pieces' class names, written once here
  // - and the font, loaded once. Drawn for a scene 1920 px wide. Without a
  // skin, only a wrapper: the stage keeps Pit Box's look.
  import type { Snippet } from "svelte";
  import type { SkinShown } from "$lib/story/play";
  import { skinFont } from "$lib/story/player/phoneDress";
  import { SCENE_WIDTH, skinDress } from "$lib/story/player/skins";
  import { storyFileUrl } from "$lib/story/player/speakers";

  interface Props {
    skin: SkinShown | null;
    /** The story's folder; `null` for the sample. */
    folder: string | null;
    children: Snippet;
  }
  const { skin, folder, children }: Props = $props();

  let font = $state<string | null>(null);
  $effect(() => {
    const [f, path] = [folder, skin?.font];
    font = null;
    if (!path) return;
    let current = true;
    void skinFont(f, path).then((family) => {
      if (current) font = family;
    });
    return () => {
      current = false;
    };
  });
  const dress = $derived(skinDress(skin, (p) => storyFileUrl(folder, p), font, SCENE_WIDTH));
</script>

<div class="worn {dress.classes}" style={dress.style}>
  {@render children()}
</div>

<style>
  .worn {
    width: 100%;
    /* A skin's slots scale with the stage's width. */
    container-type: inline-size;
  }
  .worn.skinned {
    font-family: var(--skin-font, inherit);
  }
  .has-textbox :global(.box),
  .has-nameplate :global(.speaker),
  .has-choice :global(.choice),
  .has-paper :global(.paper),
  .has-masthead :global(.masthead),
  .has-photo :global(.photo),
  .has-set :global(.tv-set),
  .has-lower_third :global(.lower-third) {
    background: none;
    border: 0 solid transparent;
    border-radius: 0;
  }
  .has-textbox :global(.box) {
    border-image: var(--slot-textbox) var(--slot-textbox-slice) / var(--slot-textbox-width);
  }
  .has-nameplate :global(.speaker) {
    border-image: var(--slot-nameplate) var(--slot-nameplate-slice) / var(--slot-nameplate-width);
  }
  .has-choice :global(.choice) {
    border-image: var(--slot-choice) var(--slot-choice-slice) / var(--slot-choice-width);
  }
  .has-paper :global(.paper) {
    border-image: var(--slot-paper) var(--slot-paper-slice) / var(--slot-paper-width);
  }
  .has-masthead :global(.masthead) {
    border-image: var(--slot-masthead) var(--slot-masthead-slice) / var(--slot-masthead-width);
  }
  .has-photo :global(.photo) {
    border-image: var(--slot-photo) var(--slot-photo-slice) / var(--slot-photo-width);
  }
  .has-set :global(.tv-set) {
    border-image: var(--slot-set) var(--slot-set-slice) / var(--slot-set-width);
  }
  .has-lower_third :global(.lower-third) {
    border-image: var(--slot-lower_third) var(--slot-lower_third-slice) / var(--slot-lower_third-width);
  }
</style>
