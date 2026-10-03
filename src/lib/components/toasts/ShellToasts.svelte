<script lang="ts">
  // The notifications the shell shows, in their stacking order. A new kind of
  // notification is added here, next to its siblings, without touching the
  // shell itself — `ToastStack` owns the position, this file owns the list.
  import ToastStack from "./ToastStack.svelte";
  import PrefsToast from "./PrefsToast.svelte";
  import ControllerToast from "./ControllerToast.svelte";
  import UpdateToast from "./UpdateToast.svelte";
  import BulkToasts from "./BulkToasts.svelte";
  import RecoverToast from "./RecoverToast.svelte";
  import RepairToast from "./RepairToast.svelte";
  import SurveyToast from "./SurveyToast.svelte";
  import CatalogToast from "./CatalogToast.svelte";
  import TechSheetToast from "./TechSheetToast.svelte";
  import ImportToasts from "./ImportToasts.svelte";
  import MusicToast from "./MusicToast.svelte";
  import OnlineLayersToast from "./OnlineLayersToast.svelte";
  import OnlineWatchToast from "./OnlineWatchToast.svelte";
  import GridThumbToast from "./GridThumbToast.svelte";
  import { FEATURE_GRID_THUMBS } from "$lib/features";
</script>

<!-- Tout ce que l'app a à dire sans interrompre, dans une seule colonne en bas
     à droite : progression et rapports d'import, nouveau périphérique. -->
<ToastStack>
  <PrefsToast />
  <ControllerToast />
  <UpdateToast />
  <BulkToasts />
  <RecoverToast />
  <RepairToast />
  <SurveyToast />
  <CatalogToast />
  <TechSheetToast />
  <ImportToasts />
  <MusicToast />
  <OnlineLayersToast />
  <OnlineWatchToast />
  <!-- La génération des vignettes en dernier, donc au plus près du coin : elle
       dure des minutes là où les autres passent, et c'est celle qu'on revient
       consulter. -->
  {#if FEATURE_GRID_THUMBS}<GridThumbToast />{/if}
</ToastStack>
