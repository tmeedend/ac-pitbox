<script lang="ts" module>
  // Trois états, pas deux (SPEC SESSION§1) : un mod choisi, rien de
  // choisi, ou des chemins cassés. Le troisième existe parce que l'invitation
  // « choisir une voiture » mène à une bibliothèque vide quand Assetto Corsa
  // est introuvable — ce n'est pas le même problème, donc pas la même
  // destination (Réglages › Chemins).
  export type SlotState = "picked" | "empty" | "broken";
</script>

<script lang="ts">
  // The part of a session block that NAVIGATES: thumbnail, name, source. The
  // car and the track share it because they have exactly the same anatomy — a
  // track simply has no brand and no badge. The fields below it (livery,
  // layout…) are its siblings in `SessionColumn`, never its children.
  import ModIdentity from "$lib/components/library/ModIdentity.svelte";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    kind: "car" | "track";
    slot: SlotState;
    /** Photo, already resolved by `previewSrc`. */
    preview: string | null;
    /** Layout outline laid over a track photo. */
    outline?: string | null;
    /** Full name, read by screen readers; `null` when nothing is picked (no
     * identity line at all). */
    name: string | null;
    /** What the identity line shows, when it differs from `name` (the car
     * without its brand, which the logo already says). */
    displayName?: string;
    badge?: string | null;
    brand?: string | null;
    year?: number | null;
    /** Reserve the logo's room while the details load (see `ModIdentity`). */
    reserve?: boolean;
    inactive: boolean;
    /** Where the mod comes from — its author. */
    source?: string | null;
    onpress: () => void;
  }
  let {
    kind,
    slot,
    preview,
    outline = null,
    name,
    displayName,
    badge,
    brand,
    year,
    reserve = false,
    inactive,
    source = null,
    onpress,
  }: Props = $props();

  const isCar = $derived(kind === "car");
</script>

<!-- Clic sur la vignette ou le nom : c'est la zone qui NAVIGUE (SPEC SESSION§1).
     Les menus de livrée/layout et la ligne « Mon pilote » sont ses frères dans
     le DOM, jamais ses enfants — un clic qui visait un menu ne doit pas éjecter
     vers la bibliothèque. -->
<button
  class="pick"
  type="button"
  onclick={onpress}
  title={slot === "picked" ? (isCar ? t("session.carTooltip") : t("session.trackTooltip")) : undefined}
  aria-label={name != null
    ? `${name} — ${isCar ? t("session.changeCar") : t("session.changeTrack")}`
    : undefined}
>
  <div class="thumb" class:track={!isCar} class:car={isCar} class:vacant={slot !== "picked"} class:photo={slot === "picked" && preview}>
    {#if slot === "picked"}
      {#if preview}<img src={preview} alt="" />{:else}<span class="thumb-ic">{isCar ? "🚗" : "🏁"}</span>{/if}
      {#if outline}<img class="outline" src={outline} alt="" />{/if}
      <!-- Le libellé n'est pas supprimé, il est différé (SPEC SESSION§1) : au
           survol et au focus clavier seulement, sur un voile PLEIN — au
           moment où l'on décide de changer, la voiture actuelle n'est
           plus l'information utile, et un voile partiel rendrait le
           libellé illisible sur une photo imprévisible. -->
      <span class="veil"
        ><span aria-hidden="true">✎</span>{isCar ? t("session.changeCar") : t("session.changeTrack")}</span
      >
    {:else if slot === "empty"}
      <span class="invite"
        ><span aria-hidden="true">＋</span>{isCar ? t("session.chooseCar") : t("session.chooseTrack")}</span
      >
    {:else}
      <!-- Impasse (SPEC SESSION§1) : même trame que l'état initial, autre
           destination — ici l'invitation à choisir mènerait à une
           bibliothèque vide. -->
      <span class="invite">
        {isCar ? t("session.noCarDetected") : t("session.noTrackDetected")}
        <small>{t("session.checkPaths")}</small>
      </span>
    {/if}
  </div>
  {#if name != null}
    <!-- Exactement la carte de bibliothèque, composant compris. Un circuit
         n'a simplement ni marque ni badge. `meta` (qui valait « Nissan ·
         1999 ») a disparu du même coup — cette ligne le dit mieux, et le
         répéter ferait deux fois la même phrase dans huit pixels de haut. -->
    <ModIdentity name={displayName ?? name} {badge} {brand} {year} {reserve}>
      {#snippet after()}
        {#if inactive}<span class="warn" title={t("session.inactiveTooltip")}>⚠</span>{/if}
      {/snippet}
    </ModIdentity>
    <!-- L'auteur, sous la voiture comme sous le circuit : c'est une source,
         pas une partie de son nom. -->
    {#if source}<div class="psrc">{source}</div>{/if}
  {/if}
</button>

<style>
  /* Zone qui NAVIGUE (SPEC SESSION§1) : vignette + nom + source, et rien d'autre.
     C'est un `<button>` FRÈRE des champs, jamais leur parent — un bouton qui
     contient des contrôles interactifs est invalide en HTML et casse la
     navigation clavier, et un clic qui visait un menu ne doit jamais éjecter
     vers la bibliothèque. */
  .pick {
    display: block;
    width: 100%;
    padding: 0;
    background: none;
    border: none;
    text-align: left;
    cursor: pointer;
  }
  .thumb {
    /* Rapport de repli, pour les états qui n'ont pas d'image à montrer
       (emplacement vide, chemins cassés, mod sans photo) : sans lui la boîte
       n'aurait aucune hauteur. Dès qu'il y a une photo, c'est ELLE qui donne
       la hauteur — voir `.thumb.photo`. */
    aspect-ratio: 2.3;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    border: 1px solid var(--line);
    overflow: hidden;
    background: linear-gradient(135deg, #1a0808, var(--panel));
  }
  .thumb.track {
    aspect-ratio: 2.9;
    background: linear-gradient(135deg, #0a1a14, var(--panel));
  }
  /* Tracé du layout superposé à la photo du circuit (comme la fiche). */
  /* Le tracé est un CALQUE, pas la photo : il garde la boîte entière, quelle
     que soit la hauteur que la photo lui a donnée (sans les deux dimensions
     explicites, le `height: auto` de la règle du dessus le ferait retomber
     sur sa taille intrinsèque et déborder). */
  .thumb img.outline {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    padding: 8px;
  }
  /* **La photo est montrée entière.** Elle l'était en `cover` sous un rapport
     imposé, donc rognée en haut et en bas — les roues d'une voiture et les
     bords d'un tracé y passaient, et ça se voyait (signalé à l'écran). C'est
     donc la boîte qui prend le rapport de l'image, jamais l'inverse : ni
     recadrage, ni bandes vides. */
  .thumb.photo {
    aspect-ratio: auto;
  }
  .thumb.photo img {
    display: block;
    width: 100%;
    height: auto;
  }
  /* Fenêtre basse : la photo et le plan tombent à la moitié de leur hauteur
     (L5§2.3, premier des trois recours). Un plafond de hauteur plutôt
     qu'un rapport d'image, parce que la boîte prend le rapport de SA photo
     dès qu'il y en a une (`.thumb.photo`) — c'est donc la hauteur qu'il faut
     borner, et le recadrage est préférable à une image écrasée.
     `sidecol` est le conteneur que déclare `.side` dans `SessionColumn`. */
  @container sidecol (max-height: 960px) {
    /* Hauteur EXPLICITE et non un plafond : la boîte tire sa hauteur de son
       image (`aspect-ratio: auto`), donc un `max-height` ne donnerait à
       l'image aucune hauteur de référence à laquelle se rapporter. */
    .thumb.photo {
      height: 68px;
    }
    .thumb.photo img {
      height: 100%;
      object-fit: cover;
    }
    .thumb:not(.photo) {
      aspect-ratio: 4.6;
    }
    .thumb.track:not(.photo) {
      aspect-ratio: 5.8;
    }
  }
  .thumb-ic {
    font-size: 34px;
    opacity: 0.6;
  }
  /* Voile du libellé différé (SPEC SESSION§1) : plein, pas dégradé — au moment où
     l'on décide de changer, la photo n'est plus l'information utile, et un
     voile partiel rendrait le texte illisible sur une image imprévisible. Au
     focus clavier comme au survol : le libellé doit être atteignable sans
     souris. */
  .veil {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    background: rgba(8, 8, 10, 0.72);
    color: var(--txt);
    font-size: 10.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    opacity: 0;
    transition: opacity 0.13s;
  }
  .pick:hover .veil,
  .pick:focus-visible .veil {
    opacity: 1;
  }
  .pick:hover .thumb {
    border-color: var(--faint2);
  }
  /* Rien de choisi : trame diagonale et bordure pointillée disent
     « emplacement à remplir » sans imiter une vignette vide, qui se lirait
     comme un mod sans photo. */
  .thumb.vacant {
    background: repeating-linear-gradient(135deg, #141518, #141518 6px, #17181c 6px, #17181c 12px);
    border-style: dashed;
    border-color: var(--faint2);
  }
  .invite {
    color: var(--txt2);
    font-size: 11.5px;
    letter-spacing: 0.06em;
    text-align: center;
    padding: 0 10px;
  }
  /* Le glyphe est un frère en ligne du libellé, pas un élément de grille :
     l'écart se pose ici plutôt qu'avec une espace dans la chaîne traduite. */
  .invite span {
    margin-right: 5px;
  }
  .invite small {
    display: block;
    margin-top: 3px;
    color: var(--muted);
    font-size: 10px;
    letter-spacing: 0;
  }
  /* Mod sélectionné mais non activé (§ garde-fou lancement) : jaune = alerte,
     cohérent avec les couleurs sémantiques du projet. */
  .warn {
    color: var(--yellow);
    margin-left: 4px;
  }
  /* Source, pas résumé : l'auteur, sous la voiture comme sous le circuit. Ce
     qui est déjà écrit au-dessus (le nom) ou juste en dessous (la livrée, le
     tracé) n'y est pas répété — on ne paie pas des caractères pour une
     information présente à quelques pixels. */
  .psrc {
    margin-top: 1px;
    font-size: 10.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
