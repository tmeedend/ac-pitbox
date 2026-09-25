<script lang="ts">
  // The fields under the session car: livery, driver, performance. Rendered
  // only once a car is picked. The `.field` rows come from `SessionColumn`,
  // which owns that grammar for every line of the column.
  import ImageSelectDropdown from "$lib/components/ui/ImageSelectDropdown.svelte";
  import PerformanceFold from "./PerformanceFold.svelte";
  import { carClassOf, driverFor, isEmpty, wearsFallback } from "$lib/driver/driverOverride.svelte";
  import { bodyThumb, requestBodyThumb } from "$lib/driver/driverThumbs.svelte";
  import { wornOutfit } from "$lib/driver/driverOutfits.svelte";
  import { nav, requestSection, pickSession } from "$lib/shell/nav.svelte";
  import { previewSrc, type getModDetail } from "$lib/library/library";
  import { listModSkins, type SkinItem } from "$lib/launch/launch";
  import { setPreferredSkin } from "$lib/preferred";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    /** Fresh details of the session car, `null` while loading. */
    carDetail: Awaited<ReturnType<typeof getModDetail>>;
    /** The car's name as the column writes it (without its brand). */
    carName: string;
  }
  let { carDetail, carName }: Props = $props();

  // Sélecteur rapide de livrée, directement depuis le bloc SESSION (évite de
  // passer par la fiche détail pour un changement rapide). Mêmes actions que
  // DetailPage.svelte (mémorise le choix + met à jour le duo de session),
  // réutilisées à l'identique.
  let carSkins = $state<SkinItem[]>([]);

  $effect(() => {
    const carId = nav.sessionCar?.id ?? null;
    if (!carId) {
      carSkins = [];
      return;
    }
    listModSkins(carId).then((s) => {
      if (nav.sessionCar?.id === carId) carSkins = s;
    });
  });

  // `livery.png` (couleurs/motif du skin seul) plutôt que `preview` (photo de
  // la voiture entière, SESSION§1) : à 20px dans ce menu compact, la voiture
  // entière écrasée était illisible — repli sur `preview` si le skin n'a pas
  // de livery (convention pas garantie sur tous les skins).
  const carSkinOptions = $derived(
    carSkins.map((s) => ({ id: s.id, name: s.name, image: previewSrc(s.livery ?? s.preview) })),
  );

  function pickCarSkin(skinId: string) {
    const car = nav.sessionCar;
    const sk = carSkins.find((s) => s.id === skinId);
    if (!car || !sk) return;
    setPreferredSkin(car.id, sk);
    // `meta` ne porte plus la livrée (SPEC SESSION§1) : elle a sa propre ligne
    // juste dessous, il n'y a donc plus rien à y réécrire.
    pickSession("Car", {
      ...car,
      preview: sk.preview ?? car.preview,
      skin: sk.id,
    });
  }

  // --- Point d'entrée de l'écran Pilote (PILOTE§3.2) ---------
  //
  // Une ligne, pas trois menus : le choix a quitté cette colonne pour son
  // propre écran, parce que la hauteur y est la ressource rare et que
  // l'arrivée du corps y portait le nombre de listes à quatre (§D1). La ligne
  // porte le libellé, et un badge qui dit en un mot où en est le pilote.
  //
  // Ouvert aussi sur une voiture de course : le corps s'y pose comme sur
  // n'importe quelle voiture (§DRIVER3D_MODEL, docs/csp-driver-research.md),
  // et un verrou qui n'empêchait plus rien — un simple clic le franchissait —
  // ne faisait que décrire un état qui n'était même plus vrai. Retiré avec
  // l'utilisateur : le PILOTE§11.2 de la spec (voiture de course grisée) est donc
  // un écart assumé.
  /** La tenue de **cette** voiture, cascade résolue : la sienne si elle en a
   * une, la tenue par défaut si l'option est active, la livrée sinon. */
  /** La classe décide de **laquelle** des deux tenues par défaut s'applique
   * (course ou rue) ; `carDetail` la porte déjà. */
  const sessionCarClass = $derived(carClassOf(carDetail?.car_class));
  const driverPrefs = $derived(driverFor(nav.sessionCar?.id ?? null, sessionCarClass));

  /** Rien de choisi : la voiture et sa livrée décident de tout. */
  const driverUntouched = $derived(isEmpty(driverPrefs));

  /**
   * Ce que la ligne annonce.
   *
   * « Mon pilote » ne disait rien de ce qu'on porte. Trois cas, trois
   * réponses : le **nom de la tenue** quand on en a enregistré une et qu'on la
   * porte — c'est l'information la plus utile et c'est l'utilisateur qui l'a
   * écrite —, sinon la mention que rien n'a été touché, sinon qu'on a composé
   * quelque chose sans le nommer.
   */
  const driverLabel = $derived.by(() => {
    if (driverUntouched) return t("session.driverStock");
    const named = wornOutfit(driverPrefs)?.name;
    if (named) return named;
    // Une tenue héritée du défaut sans nom ne devrait pas exister — le défaut
    // *est* une tenue enregistrée — mais le dire plutôt que de mentir coûte
    // une ligne.
    return wearsFallback(nav.sessionCar?.id ?? null, sessionCarClass)
      ? t("session.driverFallback")
      : t("session.driverCustom");
  });

  /** Clé du badge, ou `null` (PILOTE§3.2). « Modifié » a disparu de la liste : le
   * libellé le dit déjà, et un badge qui répète la ligne qu'il accompagne
   * n'est que du bruit. */
  const driverBadge = $derived(driverPrefs.body ? "substituted" : null);

  /**
   * Vignette du corps substitué, dans la même colonne que celle de la livrée
   * juste au-dessus.
   *
   * La ligne « Mon pilote » disait qui pilote sans le montrer, seule de la
   * colonne dans ce cas : la voiture, la livrée et le circuit ont tous leur
   * image. Rien à rendre en plus pour autant — c'est la vignette de la
   * galerie de l'écran Pilote (`driverThumbs`), déjà sur disque dès qu'on y
   * est passé une fois, et la clé est celle de cet écran.
   *
   * **Vide quand aucun corps n'est substitué**, et c'est exact : le pilote
   * est alors celui que la voiture embarque, dont on ne connaît pas
   * l'identifiant côté interface. La case reste, la colonne tient.
   */
  const driverBodyThumb = $derived(
    nav.sessionCar && driverPrefs.body ? bodyThumb(nav.sessionCar.id + "|" + driverPrefs.body) : null,
  );
  $effect(() => {
    const car = nav.sessionCar;
    const body = driverPrefs.body;
    // `requestBodyThumb` est idempotent (il sort tout de suite si la vignette
    // est faite ou en cours) : cet effet, abonné à toutes les préférences par
    // `driverPrefs`, peut donc se redéclencher sans rien coûter.
    if (car && body) requestBodyThumb(car.id, car.skin ?? null, body);
  });
</script>

<ImageSelectDropdown
  label={t("session.fieldLivery")}
  options={carSkinOptions}
  selectedId={nav.sessionCar?.skin ?? null}
  placeholder={t("session.pickSkin")}
  emptyText={t("session.noSkinsAvailable")}
  staticWhenSingle
  onselect={pickCarSkin}
/>
<!-- Le chevron est `›` et non `▾` : ce champ n'ouvre pas un menu
     mais l'écran Pilote. Bas pour un menu, droite pour une
     destination — la distinction est ténue mais constante. -->
<button
  class="field"
  type="button"
  title={driverUntouched ? t("session.driverStockTooltip") : t("session.driverTooltip")}
  onclick={() => requestSection("driver")}
>
  <span class="k">{t("session.fieldDriver")}</span>
  <span class="dthumb">
    {#if driverBodyThumb}<img src={driverBodyThumb} alt="" />{/if}
  </span>
  <span class="v" class:stock={driverUntouched}>{driverLabel}</span>
  {#if driverBadge}
    <span class="dl-badge">{t("session.driverBadge." + driverBadge)}</span>
  {/if}
  <span class="chev" aria-hidden="true">›</span>
</button>
<PerformanceFold {carName} />

<style>
  /* Même case que `.isd-thumb` du sélecteur de livrée, aux mêmes dimensions :
     c'est leur alignement vertical qui fait tout l'intérêt. Elle est posée
     même vide — un cadre qui apparaît et disparaît décalerait le nom du
     pilote d'une voiture à l'autre. Pleine hauteur de ligne depuis qu'à 13 px
     on ne reconnaissait rien de ce qu'elle montre, et donc sans cadre à elle :
     le raisonnement complet est dans `ImageSelectDropdown`. */
  .dthumb {
    flex: none;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    overflow: hidden;
  }
  .dthumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    /* **Recadrage sur la tête, en CSS et pas au rendu.** La vignette source est
       un buste (du haut du casque à la poitrine). Mesuré sur les 85 vignettes
       du cache : la tête va du bord haut (médiane 4 %, au pire 16 %) à 39 % de
       la hauteur (p90 47 %). Ce couple montre donc la tranche 0-52 % de
       l'image source, soit la tête entière et un doigt d'épaules.
       **Il tient même à 28 px**, et l'essai inverse a été fait : ouvert au
       haut du buste, la ligne montrait le torse et les jambes, qui ne
       distinguent aucun mannequin d'un autre. Ce qu'on reconnaît d'un pilote
       est la forme de son casque — sur une ligne d'une seule hauteur de
       texte, le reste n'est que du remplissage.
       En CSS et non dans `driverThumbs` parce que le même PNG sert la galerie
       de l'écran Pilote, où il est affiché à 104 px et où le buste est le bon
       cadrage — et parce que le recalculer invaliderait les 85 vignettes déjà
       sur disque pour un problème qui n'existe qu'ici. */
    transform: translateY(46%) scale(1.9);
  }
  .dl-badge {
    flex: 0 0 auto;
    font-size: 9.5px;
    letter-spacing: 0.12em;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 2px;
    padding: 1px 5px;
  }
</style>
