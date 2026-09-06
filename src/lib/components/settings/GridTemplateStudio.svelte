<script lang="ts">
  // L'aperçu du gabarit des vignettes (docs/SPEC-grille.md §6.2).
  //
  // **Une grille, pas une voiture.** C'est la décision structurante du §6.2, et
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
  import { listLibrary, type ModCard } from "$lib/library";
  import { createGridStudio, pauseGridThumbs, resumeGridThumbs, type GridStudio, type StudioCar } from "$lib/gridThumbs.svelte";
  import type { GridTemplate } from "$lib/gridThumbs";
  import LoadingState from "../LoadingState.svelte";

  const { template }: { template: GridTemplate } = $props();

  /** Taille de rendu de l'aperçu. Bien plus petit que les 1024×576 de sortie :
   * six cases dans une colonne d'écran de réglages, et il faut pouvoir en
   * redessiner six par mouvement de curseur. */
  const W = 384;
  const H = 216;

  /** Combien de voitures dans l'échantillon. Six, comme la spec — assez pour
   * voir un réglage rater une silhouette, assez peu pour tenir à l'écran. */
  const SAMPLE = 6;

  let studio: GridStudio | null = null;
  let cars = $state<StudioCar[]>([]);
  let images = $state<Record<string, string>>({});
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
    const byCategory = new Map<string, ModCard[]>();
    for (const car of cars) {
      const key = (car.category ?? car.car_class ?? "").toLowerCase();
      const bucket = byCategory.get(key);
      if (bucket) bucket.push(car);
      else byCategory.set(key, [car]);
    }
    // Un tour par catégorie avant d'en reprendre une : avec moins de six
    // catégories, on complète plutôt que de rendre un aperçu à trois cases.
    const picked: ModCard[] = [];
    const buckets = [...byCategory.values()];
    for (let round = 0; picked.length < SAMPLE && round < 40; round += 1) {
      let added = false;
      for (const bucket of buckets) {
        const car = bucket[round];
        if (!car) continue;
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
    // celles-ci se disputeraient le brouillon et le processeur, et le §6.3 dit
    // que manipuler les réglages ne régénère rien.
    pauseGridThumbs();
    void (async () => {
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
        if (car) cars = [...cars, car];
      }
      loading = false;
    })();
    return () => {
      alive = false;
      studio?.dispose();
      studio = null;
      resumeGridThumbs();
    };
  });

  // Redessine à chaque changement de gabarit. Six images de 384×216 se rendent
  // en quelques millisecondes chacune — l'`$effect` suffit, sans temporisation :
  // c'est le chargement qui coûtait, et il est fait.
  $effect(() => {
    const t = template;
    const handle = studio;
    if (!handle) return;
    const next: Record<string, string> = {};
    for (const car of cars) next[car.id] = handle.draw(car, t);
    images = next;
  });
</script>

<div class="studio">
  {#if loading && cars.length === 0}
    <LoadingState />
  {:else}
    {#each cars as car (car.id)}
      <figure>
        <div class="frame">
          {#if images[car.id]}<img src={images[car.id]} alt={car.name} />{/if}
        </div>
        <figcaption>{car.name}</figcaption>
      </figure>
    {/each}
    {#if loading}
      <!-- L'échantillon se remplit une voiture à la fois : les cases déjà là
           sont utilisables tout de suite, on ne fait pas attendre six
           conversions pour montrer la première. -->
      <figure class="pending"><div class="frame"></div></figure>
    {/if}
  {/if}
</div>

<style>
  .studio {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 10px;
  }
  /* Le même mat que la grille de bibliothèque : on règle une image qui sera
     posée dessus, la juger sur un autre fond n'aurait pas de sens. */
  .frame {
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
