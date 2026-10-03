<script lang="ts">
  // What a server tells beyond its slots (SPEC-play-online.md, "Détail d'un
  // serveur", points 5 to 7): conditions as a band, rules as departures from
  // the norm, the description folded — servers pad it with pages of rules
  // and Discord banners. Plus the links found in its name and description,
  // which the name alone could only show as text.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n/index.svelte";
  import { ruleText } from "$lib/online/labels";
  import type { ServerDetail } from "$lib/online/online";

  let { detail }: { detail: ServerDetail } = $props();

  const extended = $derived(detail.extended);
  const c = $derived(extended?.conditions);

  /** `FewClouds` → `Few Clouds`: CSP's weather id, readable as is. */
  function weatherName(id: string): string {
    return id.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/_/g, " ");
  }

  /** What a link shows: its host and path, without the scheme. */
  function linkLabel(url: string): string {
    return url.replace(/^https?:\/\/(www\.)?/i, "").replace(/\/$/, "");
  }

  function open(url: string) {
    openUrl(url).catch((e) => console.error("openUrl", e));
  }
</script>

{#if c && (c.weather || c.ambient !== null || c.road !== null || c.grip !== null)}
  <h4 class="lbl">{t("online.conditions")}</h4>
  <p class="band mono">
    {#if c.weather}<span>{weatherName(c.weather)}</span>{/if}
    {#if c.ambient !== null}<span>{t("online.conditionAir", { n: Math.round(c.ambient) })}</span>{/if}
    {#if c.road !== null}<span>{t("online.conditionRoad", { n: Math.round(c.road) })}</span>{/if}
    {#if c.grip !== null}<span>{t("online.conditionGrip", { n: Math.round(c.grip) })}</span>{/if}
    {#if c.wind_speed}<span>{t("online.conditionWind", { n: Math.round(c.wind_speed) })}</span>{/if}
  </p>
{/if}

{#if extended?.rules.length}
  <h4 class="lbl">{t("online.rules")}</h4>
  <ul class="rules">
    {#each extended.rules as rule, i (i)}<li>{ruleText(rule)}</li>{/each}
  </ul>
{/if}

{#if detail.links.length}
  <h4 class="lbl">{t("online.links")}</h4>
  <ul class="links">
    {#each detail.links as url (url)}
      <li><button class="link" type="button" title={url} onclick={() => open(url)}>{linkLabel(url)}</button></li>
    {/each}
  </ul>
{/if}

{#if extended?.description}
  <details class="desc">
    <summary class="lbl">{t("online.description")}</summary>
    <p>{extended.description}</p>
  </details>
{/if}

<style>
  .lbl {
    margin: 6px 0 8px;
  }
  .band {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 11.5px;
    color: var(--txt2);
    margin-bottom: 14px;
  }
  .rules,
  .links {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-bottom: 14px;
    font-size: 12px;
    color: var(--txt2);
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--blue);
    font-size: 12px;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .link:hover {
    text-decoration: underline;
  }
  .desc summary {
    cursor: pointer;
  }
  .desc p {
    white-space: pre-line;
    font-size: 12px;
    line-height: 1.5;
    color: var(--txt2);
    margin-bottom: 14px;
  }
</style>
