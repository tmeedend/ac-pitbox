<script lang="ts">
  // Bloc « Type de session » de l'écran Lancement (§8.4) : bascule
  // Practice/Hotlap/Course. Vue de présentation pure — le changement de type
  // déclenche la sauvegarde du preset courant puis l'application de celui de
  // la cible (§8.4bis), orchestré par Launch.svelte via `onselect`.
  import type { SessionType } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";
  import Seg from "../Seg.svelte";

  let {
    sessionType,
    trackCategories = [],
    onselect,
  }: {
    sessionType: SessionType;
    /** Catégories du circuit choisi (§5bis.2), telles que stockées : `#circuit`,
     * `#drift`… Vide tant qu'aucun circuit n'est sélectionné. */
    trackCategories?: string[];
    onselect: (type: SessionType) => void;
  } = $props();

  // Un hotlap et une course supposent un tracé qui boucle et qu'on chronomètre.
  // Sur une piste de dragster, une montée ou un point-à-point, elles partent
  // mais ne veulent rien dire : pas de tour à boucler, donc pas de temps au
  // tour ni de classement. On avertit sans bloquer — l'app n'arbitre pas ce
  // que l'utilisateur a le droit de lancer, et la catégorie vient de règles
  // que lui-même peut modifier (écran Règles).
  //
  // Comparé sans le `#` : la catégorie est stockée avec, mais un tag saisi à
  // la main ou une règle personnalisée peut l'écrire sans.
  const isCircuit = $derived(trackCategories.some((c) => c.replace(/^#/, "").toLowerCase() === "circuit"));
  const lapBased = $derived(sessionType === "hotlap" || sessionType === "race");
  const warn = $derived(trackCategories.length > 0 && lapBased && !isCircuit);

  const sessionTypes: { id: SessionType; labelKey: string }[] = [
    { id: "practice", labelKey: "launch.typePractice" },
    { id: "hotlap", labelKey: "launch.typeHotlap" },
    { id: "race", labelKey: "launch.typeRace" },
    { id: "trackday", labelKey: "launch.typeTrackday" },
  ];
</script>

<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.sessionTypeLabel")}</span></header>
  <div class="blk-b">
    <div class="types">
      <Seg
        size="main"
        value={sessionType}
        onselect={(v) => onselect(v as SessionType)}
        items={sessionTypes.map((st) => ({ value: st.id, label: t(st.labelKey) }))}
      />
    </div>
    {#if warn}
      <p class="warnbox spaced">⚠ {t("launch.trackNotCircuitWarning")}</p>
    {/if}
  </div>
</section>

<style>
  .types {
    margin-bottom: 16px;
  }
  /* L'encadré vient de `.warnbox` (global.css). La marge négative rattrape
     celle que `.types` réserve sous les boutons. */
  .spaced {
    margin-top: -6px;
  }
</style>
