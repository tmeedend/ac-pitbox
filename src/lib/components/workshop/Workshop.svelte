<script lang="ts">
  // Atelier (SPEC §7.2quater) : Règles, Importer, Profils et Maintenance en
  // quatre onglets d'un même écran.
  //
  // Le regroupement n'est possible QUE parce qu'aucun des quatre n'a de
  // sous-rubrique — c'est ce qui distingue ce cas de celui des inventaires,
  // qui ont déjà leurs propres onglets et produiraient donc deux rangées
  // horizontales de forme identique, sans que rien n'indique laquelle
  // commande l'autre.
  //
  // L'onglet reste porté par `nav.section` plutôt que par un état local : la
  // douzaine d'endroits qui appellent déjà `requestSection("import")` (le
  // glisser-déposer global, un rapport d'import, un renvoi depuis la
  // bibliothèque) continuent d'atterrir sur le bon onglet sans rien savoir de
  // cet écran, la garde de navigation et l'historique restent en place, et
  // l'entrée du rail — qui vise `rules` — repart forcément du premier onglet.
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import RulesEditor from "./RulesEditor.svelte";
  import Categories from "./Categories.svelte";
  import Import from "./Import.svelte";
  import Profiles from "./Profiles.svelte";
  import Maintenance from "./Maintenance.svelte";
  import { nav, requestSection } from "$lib/shell/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";

  const TAB_IDS = ["rules", "categories", "import", "profiles", "maintenance"] as const;
  // Recalculés à chaque changement de langue (`t` est réactif) : un tableau
  // `const` de libellés figés resterait dans l'ancienne langue.
  const tabs = $derived(TAB_IDS.map((id) => ({ id, label: t(`nav.${id}`) })));

  /** L'éditeur de règles gère son propre défilement et porte une barre d'action
   * en pied (`noPad` dans AppShell) : l'écran doit alors être une colonne
   * pleine hauteur, pas un contenu posé dans une zone qui défile. Les trois
   * autres onglets sont du contenu ordinaire. */
  const full = $derived(nav.section === "rules");
</script>

<div class="workshop" class:full>
  <div class="head">
    <h2 class="lbl-screen">{t("nav.atelier")}</h2>
    <Tabs {tabs} active={nav.section} onselect={(id) => requestSection(id)} />
  </div>
  <div class="body">
    {#if nav.section === "rules"}
      <RulesEditor />
    {:else if nav.section === "categories"}
      <Categories />
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
