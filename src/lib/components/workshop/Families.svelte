<script lang="ts">
  // Classement › Families (TAXO§6): one switch, Cars · Tracks, as on the
  // Rules tab. Cars are the families of categories, tracks the closed list of
  // track categories - two tables that answer the same question ("what kind
  // is it?") for the two libraries. Not remembered, like the Rules tab's.
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import CarFamilies from "./CarFamilies.svelte";
  import TrackCategories from "./TrackCategories.svelte";

  let tab = $state<"car" | "track">("car");
</script>

<div class="families">
  <Tabs
    tabs={[
      { id: "car", label: t("rules.tabCars") },
      { id: "track", label: t("rules.tabTracks") },
    ]}
    active={tab}
    onselect={(v) => (tab = v as "car" | "track")}
  />
  <div class="body">
    {#if tab === "car"}<CarFamilies />{:else}<TrackCategories />{/if}
  </div>
</div>

<style>
  .body {
    margin-top: 14px;
  }
</style>
