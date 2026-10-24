<script lang="ts">
  // The scene behind the frame and the dialogues (SPEC-story-mode, Scenes,
  // frame and sound): a still image or a video loop, blurred behind the frame
  // and nearer into focus while the scene holds the stage; a plain background
  // without a place. A change of place or variant fades the new picture in
  // over the last one, or cuts (`SceneChanges`).
  //
  // Each picture is its own layer, so that two can stand during a fade; the
  // layers are oversized so that the blur never shows their edges.
  import { untrack } from "svelte";
  import type { SceneShown } from "$lib/story/play";
  import { SceneChanges, sceneBlur } from "$lib/story/player/scene";
  import { mediaUrl } from "$lib/story/player/speakers";
  import type { Transition } from "$lib/story/studio";

  interface Props {
    /** The place shown; `null`, a plain background. */
    scene: SceneShown | null;
    /** The story's folder, `null` for the sample (`mediaUrl`). */
    folder: string | null;
    /** The scene holds the stage: a dialogue plays, or the journal is open. */
    staged: boolean;
    /** How the item on stage comes and leaves; `null` without one. */
    transition: Transition | null;
  }
  const { scene, folder, staged, transition }: Props = $props();

  interface Layer {
    id: number;
    scene: SceneShown | null;
    /** Fades in over the layers below. */
    fading: boolean;
  }
  let layers = $state<Layer[]>([]);
  let next = 0;
  const changes = new SceneChanges();

  $effect(() => {
    // Both read first: a change of either is a new state of the stage.
    const [s, tr] = [scene, transition];
    untrack(() => {
      const how = changes.next(s, tr);
      // The same picture: no fade, but what else changed - its blur, said
      // anew - applies to the picture shown at once.
      if (how === null) {
        if (layers.length) layers = [...layers.slice(0, -1), { ...layers[layers.length - 1], scene: s }];
        return;
      }
      const layer = { id: next++, scene: s, fading: how === "fade" };
      layers = how === "cut" ? [layer] : [...layers, layer];
    });
  });

  /** The new picture is in: the ones under it are hidden, gone. */
  function arrived(id: number) {
    const at = layers.findIndex((l) => l.id === id);
    if (at > 0) layers = layers.slice(at);
  }
</script>

<div class="scene-view" aria-hidden="true">
  {#each layers as l (l.id)}
    {@const src = mediaUrl(folder, l.scene?.file)}
    <div
      class="picture"
      class:fading={l.fading}
      class:plain={!src}
      style:filter={`blur(${sceneBlur(l.scene, staged)}px)`}
      onanimationend={() => arrived(l.id)}
    >
      {#if src && l.scene?.video}
        <video {src} autoplay loop muted playsinline></video>
      {:else if src}
        <img {src} alt="" />
      {/if}
    </div>
  {/each}
</div>

<style>
  .scene-view {
    position: absolute;
    inset: 0;
    overflow: hidden;
  }
  .picture {
    position: absolute;
    inset: -16px;
    transition: filter 0.4s ease;
  }
  .picture.plain {
    background: radial-gradient(ellipse at 50% 30%, var(--panel2), var(--bg));
  }
  .picture.fading {
    animation: scene-in 0.6s ease both;
  }
  @keyframes scene-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  img,
  video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
