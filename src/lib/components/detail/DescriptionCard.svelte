<script lang="ts" module>
  /** Sous-onglet du bloc textuel (§7.4). **Jamais « Notes » par défaut** : la
   * description est ce qu'on vient lire, la note ce qu'on vient ajouter. */
  export type TextTab = "desc" | "notes" | "wiki";
</script>

<script lang="ts">
  // Bloc textuel à sous-onglets (§7.4), identique pour une voiture et pour un
  // circuit : seule la source du texte change. **Hauteur constante**, c'est le
  // contenu qui change — la longueur d'un texte cesse ainsi d'être un problème
  // de mise en page, ce qui comptera le jour où un article encyclopédique
  // viendra s'y ajouter. Rendu MÊME VIDE, sans quoi un mod sans description
  // n'offrirait aucun endroit où en écrire une (§5bis.3).
  //
  // The page keeps the sub-tab and the Wikipedia panel: both outlive this
  // card, which is unmounted each time another tab of the page is opened.
  import InlineEdit from "$lib/components/ui/InlineEdit.svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import WikiBlock from "./WikiBlock.svelte";
  import NoteBlock from "./NoteBlock.svelte";
  import type { WikiPanel } from "$lib/wiki/wiki";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    car: boolean;
    /** Mod id, the key of its Wikipedia pairing. */
    modKey: string;
    /** Description shown: the user's own when there is one, else the mod's. */
    text: string | null;
    /** The description is the user's, not the mod's. */
    overridden: boolean;
    notes: string | null;
    /** `null` while the Wikipedia search is still running. */
    wikiPanel: WikiPanel | null;
    tab: TextTab;
    ondescription: (value: string | null) => void;
    onnote: (value: string | null) => void;
    onreloadwiki: (lang?: string) => void;
  }
  let {
    car,
    modKey,
    text,
    overridden,
    notes,
    wikiPanel,
    tab = $bindable(),
    ondescription,
    onnote,
    onreloadwiki,
  }: Props = $props();

  function decodeDescription(html: string): string {
    return html
      .replace(/<\/?br\s*\/?>/gi, "\n")
      .replace(/<[^>]+>/g, "")
      .replace(/&#(\d+);/g, (_, n) => String.fromCharCode(+n))
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      .replace(/&#39;|&apos;/g, "'")
      .trim();
  }
</script>

<section class="text-zone">
  <!-- Largeur de mesure, marges automatiques : les sous-onglets partagent le
       même conteneur que le texte, sans quoi la bande courrait sur toute la
       page au-dessus d'un paragraphe de 430 px. -->
  <div class="reading">
    <Tabs
      gamepad={false}
      tabs={[
        { id: "desc", label: t("common.description") },
        { id: "notes", label: t("notes.title"), marker: !!notes },
        // **Permanent**, contre la §7.1 et décidé avec l'utilisateur : un
        // onglet absent ne se distingue ni d'une recherche en cours, ni d'une
        // fonctionnalité qui n'existe pas — et c'est quand rien n'est trouvé
        // qu'il y a le plus à faire. La pastille dit qu'il y a un article à
        // lire, sans rien promettre quand il n'y en a pas.
        {
          id: "wiki",
          label: car ? t("wiki.tabCar") : t("wiki.tabTrack"),
          marker: !!wikiPanel?.article,
        },
      ]}
      active={tab}
      onselect={(v) => (tab = v as TextTab)}
    >
      {#snippet trailing()}
        {#if tab === "desc"}
          <InlineEdit
            value={text}
            original={overridden ? null : text}
            {overridden}
            multiline
            label={t("detail.editDescriptionLabel")}
            placeholder={t("detail.editDescriptionPlaceholder")}
            onsave={ondescription}
          />
        {/if}
      {/snippet}
    </Tabs>
    <div class="text-body">
      {#if tab === "desc"}
        <div class="read-box desc-body" class:empty-desc={!text}>
          {text ? decodeDescription(text) : t("detail.noDescription")}
        </div>
      {:else if tab === "wiki"}
        <!-- Même boîte que les deux autres : le texte de Wikipédia est un
             contenu de plus dans le même cadre, jamais fondu dans la
             description (§2). -->
        <div class="read-box">
          <WikiBlock {modKey} panel={wikiPanel} onreload={onreloadwiki} />
        </div>
      {:else}
        <!-- Même boîte que la description : les deux sous-onglets échangent un
             contenu, pas une mise en page. -->
        <div class="read-box">
          <NoteBlock bare value={notes} onsave={onnote} />
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  /* ZONE 2 — le bloc de lecture.
     Pleine largeur pour le fond et le filet qui le sépare de la rangée, mais
     contenu ramené à une largeur de mesure et centré. */
  .text-zone {
    border-top: 1px solid var(--line);
    background: var(--card);
    padding: 22px 32px 30px;
  }
  /* Largeur du bloc de lecture : **des paliers, pas un pourcentage** — le
     `.container` de Bootstrap, et pour les mêmes raisons.
     Un `%` donne une largeur différente à chaque résolution, donc un rendu
     qu'on ne peut régler pour personne : correct sur l'écran où on l'a
     choisi, étalé sur le suivant. Une largeur fixe, elle, déborde dès que la
     fenêtre rétrécit. Les paliers prennent les deux : **100 % tant que la
     place manque**, puis une largeur arrêtée qui laisse la marge croître à
     gauche et à droite — ce sont les valeurs de Bootstrap, éprouvées et
     reconnaissables.
     **Requêtes de conteneur et non de média** (§13, same reason as
     DetailPage's `.row.top`) : le seuil doit se mesurer sur la largeur qui
     reste à la fiche, rail et colonne de session déduits, et le zoom
     d'interface déplace un seuil de média sans déplacer cette largeur-là.
     The `detail` container is declared by DetailPage's `.page`. */
  .reading {
    width: 100%;
    margin: 0 auto;
    /* Le corps de l'interface, comme partout ailleurs sur la fiche : un bloc
       plus large n'est pas une raison d'écrire plus gros. Défini ici plutôt
       que sur le paragraphe, pour que description et note — deux contenus du
       même bloc — ne puissent pas diverger. */
    font-size: 11px;
    line-height: 1.55;
  }
  @container detail (min-width: 576px) {
    .reading {
      max-width: 540px;
    }
  }
  @container detail (min-width: 768px) {
    .reading {
      max-width: 720px;
    }
  }
  @container detail (min-width: 992px) {
    .reading {
      max-width: 960px;
    }
  }
  @container detail (min-width: 1200px) {
    .reading {
      max-width: 1140px;
    }
  }
  @container detail (min-width: 1400px) {
    .reading {
      max-width: 1320px;
    }
  }
  .reading :global(.tabs) {
    margin-bottom: 0;
  }
  /* **Plus de hauteur minimale.** Elle valait 150 px pour qu'une description de
     trois mots ne fasse pas sauter la colonne d'à côté en changeant de
     sous-onglet — mais il n'y a plus de colonne à côté : le bloc est seul sur
     sa rangée, donc une ligne de texte occupe une ligne. */
  .text-body {
    min-width: 0;
  }
  /* La boîte du texte : bord supérieur absent, c'est celui de la bande
     d'onglets qui la ferme — les deux se lisent comme un seul panneau. */
  .read-box {
    border: 1px solid var(--line);
    border-top: none;
    background: var(--panel2);
    padding: 12px 14px 14px;
  }
  /* La note reprend la taille de la prose : dans la même boîte, sous le même
     onglet, deux corps différents se verraient. Imposé d'ici plutôt que dans
     `NoteBlock`, qui sert aussi les quatre autres fiches où il n'est pas dans
     une colonne de lecture. */
  .reading :global(.read-box button),
  .reading :global(.read-box textarea) {
    font-size: inherit;
    line-height: inherit;
  }
  .desc-body {
    color: var(--txt2);
    /* Hérite de `.reading` : la taille de la prose est définie à un seul
       endroit, celui qui calcule aussi la mesure — sans quoi les deux
       divergeraient et la colonne ne ferait plus le nombre de caractères
       annoncé. */
    font-size: inherit;
    line-height: inherit;
    white-space: pre-line;
    /* Une description de mod contient volontiers une URL de cent caractères
       sans une seule césure possible (bug réel : `ddm_daihatsu_copen_street`
       et son lien vers un dyno, qui poussait la carte hors de la colonne et
       décalait toute la fiche). `anywhere` et non `break-word` : le second
       n'agit qu'après avoir déjà tenté de placer le mot entier, donc il ne
       corrige la mise en page qu'une fois sur deux. */
    overflow-wrap: anywhere;
  }
</style>
