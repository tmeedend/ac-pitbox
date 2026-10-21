<script lang="ts">
  // The feed template (SPEC-story-mode, Content types, templates and
  // frames): articles or posts as cards, the latest first - each with its
  // author, when it came, its text and the replies it offers. A newspaper is
  // a feed too. Shared by the preview and Story Mode.
  import { initials } from "$lib/story/player/phone";
  import type { Post, Speaker } from "$lib/story/player/types";
  import ReplyChoices from "./ReplyChoices.svelte";
  import TextBody from "./TextBody.svelte";

  interface Props {
    posts: Post[];
    speakers: Record<string, Speaker>;
    onchoose: (key: string, label: string) => void;
  }
  const { posts, speakers, onchoose }: Props = $props();

  const names = $derived(Object.fromEntries(Object.entries(speakers).map(([k, s]) => [k, s.name])));
</script>

<div class="feed">
  {#each posts as p (p.key)}
    {@const avatar = p.from ? speakers[p.from]?.portraits.neutral : null}
    <article class="post">
      <div class="head">
        {#if avatar}
          <img class="avatar" src={avatar} alt="" />
        {:else}
          <span class="avatar initials">{initials(p.author)}</span>
        {/if}
        <span class="meta">
          <span class="author">{p.author}</span>
          <span class="when">{p.when}</span>
        </span>
      </div>
      <TextBody steps={p.steps} from={p.from} {names} />
      <ReplyChoices choices={p.choices} onchoose={(label) => onchoose(p.key, label)} />
    </article>
  {/each}
</div>

<style>
  .feed {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .post {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-bottom: 1px solid var(--line);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .avatar {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    object-fit: cover;
    flex-shrink: 0;
  }
  .initials {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    color: var(--txt2);
    font-size: 10.5px;
  }
  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .author {
    color: var(--txt);
    font-size: 12px;
    font-weight: 600;
  }
  .when {
    color: var(--muted);
    font-size: 10.5px;
  }
</style>
