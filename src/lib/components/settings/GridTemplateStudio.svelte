<script lang="ts">
  // L'aperçu du gabarit des vignettes (docs/SPEC-grille.md GRILLE§6.2).
  //
  // **Une grille, pas une voiture.** C'est la décision structurante du GRILLE§6.2, et
  // elle se justifie en une phrase : régler l'angle sur une seule voiture
  // conduit à l'optimiser pour elle et à massacrer les autres. On édite un
  // catalogue, l'aperçu doit être un catalogue.
  //
  // **Six gabarits volontairement différents**, dit la spec — une berline, une
  // monoplace, un SUV, une GT basse, une compacte, un prototype. Rien dans les
  // données ne dit la silhouette d'une voiture, donc l'échantillon se prend sur
  // le seul axe que la bibliothèque connaisse vraiment : la **catégorie**, une
  // voiture par catégorie distincte. C'est une approximation, et c'est celle
  // qui sépare le mieux une monoplace d'un GT dans une base réelle.
  //
  // Les six modèles sont **chargés une fois et gardés** : bouger un curseur
  // redessine, ne reconvertit jamais. Sans ça, chaque pixel de curseur coûterait
  // six conversions, soit six secondes par image.
  import { listLibrary, previewSrc, type ModCard } from "$lib/library";
  import { nav } from "$lib/nav.svelte";
  import {
    PAUSE_STUDIO,
    createGridStudio,
    pauseGridThumbs,
    resumeGridThumbs,
    type GridStudio,
    type StudioCar,
  } from "$lib/gridThumbs.svelte";
  import type { GridTemplate } from "$lib/gridThumbs";
  import type { GridMat } from "$lib/gridThumbPrefs.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import LoadingState from "../LoadingState.svelte";

  // Le mat vient du preset comme le gabarit : la moitié de ce qui distingue
  // deux presets est leur fond, et régler un éclairage de vitrine sur le fond
  // clair du catalogue reviendrait à le juger sur ce qu'il ne sera pas.
  const { template, mat }: { template: GridTemplate; mat: GridMat } = $props();

  /**
   * Taille de rendu de l'aperçu.
   *
   * **Relevée de 384 à 768** après un constat d'usage : à 384, notre image
   * était *agrandie* pour tenir dans sa case pendant que la `preview.png`
   * d'origine, elle, était réduite depuis 1022 px. La comparaison accusait donc
   * notre rendu d'un flou et d'un crénelage qui n'étaient que ceux de l'aperçu,
   * pas ceux du fichier produit. Une comparaison doit être juste avant d'être
   * rapide.
   *
   * Six cases à 768×432 restent quelques millisecondes par curseur bougé — le
   * coût est dans le chargement des modèles, qui n'a pas changé.
   */
  const W = 768;
  const H = 432;

  /** Combien de voitures dans l'échantillon. Six, comme la spec — assez pour
   * voir un réglage rater une silhouette, assez peu pour tenir à l'écran. */
  const SAMPLE = 6;

  // `$state` et non une variable ordinaire : c'est son affectation, à la fin du
  // montage, qui doit relancer le dessin — voir le commentaire de l'effet.
  let studio = $state<GridStudio | null>(null);
  let cars = $state<StudioCar[]>([]);
  /** La `preview.png` d'origine de chaque voiture de l'échantillon, par id. */
  let originals = $state<Record<string, string>>({});
  let images = $state<Record<string, string>>({});
  /** Le miroir du sol est monté à la demande, et son import est asynchrone :
   * `draw` étant synchrone — c'est ce qui lui permet de suivre un curseur — il
   * faut un passage séparé, et de quoi redessiner une fois qu'il est là. */
  let mirrorReady = $state(false);
  /**
   * Montre la `preview.png` d'origine à côté de chaque rendu.
   *
   * **C'est l'instrument du preset Officiel**, dont le critère n'est pas « est-
   * ce beau » mais « est-ce indiscernable » — un critère mesurable, donc qui se
   * règle contre la vraie image et jamais de mémoire. Utile aux deux autres
   * aussi, où il dit ce qu'on gagne.
   *
   * Volontairement **non persistée** : c'est un geste de comparaison, pas un
   * réglage. Rien à écrire, donc rien à faire survivre à un redémarrage.
   */
  let compare = $state(false);
  let loading = $state(true);

  /**
   * L'échantillon : une voiture par catégorie distincte, dans l'ordre de la
   * bibliothèque pour que l'aperçu ne change pas d'une visite à l'autre.
   *
   * Les voitures sans version active sont écartées : elles n'ont pas de modèle
   * sur le disque, donc elles ne rendraient rien et coûteraient une case.
   */
  function sample(list: ModCard[]): ModCard[] {
    const cars = list.filter((c) => c.kind === "Car" && c.active_version_id);
    // **La voiture de la session en première case.** C'est une voiture que
    // l'utilisateur a choisie lui-même, donc qu'il connaît, donc dont il sait à
    // quoi elle doit ressembler — un repère qu'aucune sélection automatique ne
    // vaut. Les cinq autres restent prises par catégorie.
    const session = cars.find((c) => c.id_interne === nav.sessionCar?.id) ?? null;
    const byCategory = new Map<string, ModCard[]>();
    for (const car of cars) {
      const key = (car.category ?? car.car_class ?? "").toLowerCase();
      const bucket = byCategory.get(key);
      if (bucket) bucket.push(car);
      else byCategory.set(key, [car]);
    }
    // Un tour par catégorie avant d'en reprendre une : avec moins de six
    // catégories, on complète plutôt que de rendre un aperçu à trois cases.
    const picked: ModCard[] = session ? [session] : [];
    const buckets = [...byCategory.values()];
    for (let round = 0; picked.length < SAMPLE && round < 40; round += 1) {
      let added = false;
      for (const bucket of buckets) {
        const car = bucket[round];
        if (!car || picked.some((p) => p.id_interne === car.id_interne)) continue;
        picked.push(car);
        added = true;
        if (picked.length >= SAMPLE) break;
      }
      if (!added) break;
    }
    return picked;
  }

  $effect(() => {
    let alive = true;
    // La file est suspendue le temps de l'aperçu : ses conversions et
    // celles-ci se disputeraient le brouillon et le processeur, et le GRILLE§6.3 dit
    // que manipuler les réglages ne régénère rien.
    pauseGridThumbs(PAUSE_STUDIO);
    void (async () => {
      try {
        const list = await listLibrary().catch(() => [] as ModCard[]);
        if (!alive) return;
        const handle = await createGridStudio(W, H);
        if (!alive) {
          handle.dispose();
          return;
        }
        studio = handle;
        for (const card of sample(list)) {
          if (!alive) return;
          const car = await handle.load(card.id_interne, null, card.display_name ?? card.id_interne);
          if (!alive) return;
          if (!car) continue;
          // La `preview.png` d'origine voyage avec la voiture : c'est elle que
          // la bascule de comparaison montre à côté du rendu.
          const original = previewSrc(card.preview);
          if (original) originals = { ...originals, [car.id]: original };
          cars = [...cars, car];
        }
      } catch (e) {
        // Pas de contexte WebGL, pas de bibliothèque : l'écran reste utilisable
        // sans son aperçu. Sans ce `catch`, `loading` ne redescendait jamais et
        // le spinner tournait pour toujours.
        console.error("aperçu de gabarit indisponible", e);
      } finally {
        loading = false;
      }
    })();
    return () => {
      alive = false;
      studio?.dispose();
      studio = null;
      resumeGridThumbs(PAUSE_STUDIO);
    };
  });

  // Redessine à chaque changement de gabarit. Six images de 384×216 se rendent
  // en quelques millisecondes chacune — l'`$effect` suffit, sans temporisation :
  // c'est le chargement qui coûtait, et il est fait.
  //
  // **Les trois dépendances sont lues AVANT la sortie anticipée**, et ce n'est
  // pas du style. Un `$effect` ne s'abonne qu'à ce qu'il a effectivement lu
  // pendant son exécution : en plaçant le `return` avant la boucle, le premier
  // passage — celui du montage, où le banc n'est pas encore là — ne voyait
  // jamais `cars`, donc les voitures qui arrivaient ensuite ne le
  // redéclenchaient pas. Bug réel signalé : à la première ouverture de l'écran,
  // le mat s'affichait vide, et il fallait choisir un autre preset dans la
  // liste — c'est-à-dire changer `template`, la seule dépendance enregistrée —
  // pour que les voitures apparaissent.
  // Monte le miroir quand le gabarit en demande un. Se relance à chaque
  // changement de gabarit, ne fait rien une fois le miroir là — pas de boucle :
  // `mirrorReady` est lu, donc son écriture redéclenche l'effet, qui sort
  // aussitôt.
  $effect(() => {
    const handle = studio;
    const wants = template.reflection > 0 && template.floor > 0;
    const ready = mirrorReady;
    if (!handle || !wants || ready) return;
    void handle.prepare($state.snapshot(template)).then(() => (mirrorReady = true));
  });

  $effect(() => {
    const handle = studio;
    const list = cars;
    const t = template;
    // Lu pour que l'arrivée du miroir redessine : sans cette ligne, le premier
    // gabarit à reflet s'affiche sans lui.
    void mirrorReady;
    if (!handle || list.length === 0) return;
    const next: Record<string, string> = {};
    for (const car of list) next[car.id] = handle.draw(car, t);
    images = next;
  });
</script>

<label class="compare">
  <input type="checkbox" bind:checked={compare} />
  <span>{t("settings.gridStudioCompare")}</span>
</label>

<div class="studio" class:paired={compare} style:--mat-hi={mat.hi} style:--mat-lo={mat.lo}>
  {#if loading && cars.length === 0}
    <LoadingState />
  {:else}
    {#each cars as car (car.id)}
      <figure>
        <div class="pair">
          <div class="frame">
            {#if images[car.id]}<img src={images[car.id]} alt={car.name} />{/if}
          </div>
          <!-- L'image d'origine **sur le même mat** : les comparer sur deux
               fonds différents ne dirait rien de ce qui les sépare vraiment. -->
          {#if compare}
            <div class="frame">
              {#if originals[car.id]}
                <img src={originals[car.id]} alt={car.name} />
              {:else}
                <span class="none">{t("settings.gridStudioNoOriginal")}</span>
              {/if}
            </div>
          {/if}
        </div>
        <figcaption>{car.name}</figcaption>
      </figure>
    {/each}
    {#if loading}
      <!-- L'échantillon se remplit une voiture à la fois : les cases déjà là
           sont utilisables tout de suite, on ne fait pas attendre six
           conversions pour montrer la première. -->
      <figure class="pending"><div class="pair"><div class="frame"></div></div></figure>
    {/if}
  {/if}
</div>

<style>
  .compare {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
    font-size: 11.5px;
    color: var(--txt2);
    cursor: pointer;
  }
  .studio {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 10px;
  }
  /* Deux images côte à côte prennent deux fois la place : les cases s'élargissent
     plutôt que de rétrécir chaque image de moitié. */
  .studio.paired {
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr;
    gap: 6px;
  }
  .studio.paired .pair {
    grid-template-columns: 1fr 1fr;
  }
  .none {
    color: var(--muted2);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 1px;
  }
  /* Le même mat que la grille de bibliothèque : on règle une image qui sera
     posée dessus, la juger sur un autre fond n'aurait pas de sens. */
  .frame {
    display: flex;
    align-items: center;
    justify-content: center;
    aspect-ratio: 16 / 9;
    background: radial-gradient(ellipse at 50% 44%, var(--mat-hi) 0%, var(--mat-lo) 76%);
    border: 1px solid var(--mat-line);
    overflow: hidden;
  }
  .frame img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }
  figure {
    margin: 0;
  }
  figcaption {
    margin-top: 4px;
    font-size: 10.5px;
    color: var(--muted2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pending .frame {
    opacity: 0.4;
  }
</style>
