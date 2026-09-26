<script lang="ts">
  // The two tabbed screens of the rail (SPEC §7.2quater, DOSSIER§3.1):
  // **Sorting** (rules, brands, families, countries) and **Files** (import,
  // profiles, maintenance). They split the former Workshop, and share this
  // container because they have exactly the same mechanics - only their tabs
  // and their title differ (`tabGroupOf`).
  //
  // Grouping tools as tabs is legitimate ONLY because none of them has a
  // sub-section of its own - which is what sets them apart from the
  // inventories, whose own tabs would stack two identical horizontal rows with
  // nothing to say which one drives the other.
  //
  // The tab is carried by `nav.section` rather than by local state: the dozen
  // places that already call `requestSection("import")` (global drag and drop,
  // an import report, a link from the library) keep landing on the right tab
  // without knowing anything of this screen, the navigation guard and the
  // history stay in place, and the rail entry - which targets the first tab -
  // always starts from it.
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import RulesEditor from "./RulesEditor.svelte";
  import Categories from "./Categories.svelte";
  import Countries from "./Countries.svelte";
  import Brands from "./Brands.svelte";
  import CatalogBanner from "./CatalogBanner.svelte";
  import Import from "./Import.svelte";
  import Profiles from "./Profiles.svelte";
  import Maintenance from "./Maintenance.svelte";
  import { nav, requestSection, tabGroupOf, SORTING_TABS } from "$lib/shell/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";

  const group = $derived(tabGroupOf(nav.section) ?? SORTING_TABS);
  const sorting = $derived(group === SORTING_TABS);
  // Recomputed on every language change (`t` is reactive): a `const` array of
  // frozen labels would stay in the previous language.
  const tabs = $derived(group.map((id) => ({ id, label: t(`nav.${id}`) })));

  /** The rules editor handles its own scrolling and carries an action bar at
   * its foot (`noPad` in AppShell): the screen must then be a full-height
   * column, not content laid in a scrolling area. The other tabs are ordinary
   * content. */
  const full = $derived(nav.section === "rules");
</script>

<div class="workshop" class:full>
  <div class="head">
    <h2 class="lbl-screen">{t(sorting ? "nav.sorting" : "nav.files")}</h2>
    <Tabs {tabs} active={nav.section} onselect={(id) => requestSection(id)} />
    <!-- The last catalogue update (REGLES§6.2), on every tab whose content it
         changes - which is all of Sorting. -->
    {#if sorting}
      <CatalogBanner />
    {/if}
  </div>
  <div class="body">
    {#if nav.section === "rules"}
      <RulesEditor />
    {:else if nav.section === "categories"}
      <Categories />
    {:else if nav.section === "brands"}
      <Brands />
    {:else if nav.section === "countries"}
      <Countries />
    {:else if nav.section === "import"}
      <Import />
    {:else if nav.section === "profiles"}
      <Profiles />
    {:else if nav.section === "maintenance"}
      <Maintenance />
    {/if}
  </div>
</div>

<style>
  .workshop.full {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .head h2 {
    margin-bottom: 18px;
  }
  /* En mode pleine hauteur, `.content` ne pose plus de retrait (`noPad`) :
     l'en-tête reprend celui qu'il appliquait, et le corps garde le sien. */
  .workshop.full .head {
    padding: 28px 32px 0;
  }
  .workshop.full .body {
    flex: 1;
    min-height: 0;
  }
</style>
