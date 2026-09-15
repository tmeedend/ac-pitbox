<script lang="ts">
  // Note libre d'une entité (refonte §9). Une par mod, quel que soit son type :
  // exclure un type créerait une règle à apprendre pour une économie nulle.
  //
  // **Ce n'est pas une description**, et la différence porte sur un seul geste :
  // effacer. Une description surcharge ce que dit le fichier du mod, donc la
  // vider veut dire « reviens au fichier » ; une note n'a pas de valeur
  // d'origine, donc la vider veut dire vide. Les fusionner rendrait « effacer »
  // ambigu — d'où deux champs, et deux colonnes en base.
  //
  // Trois choix de comportement, tous du SESSION§3 :
  //
  //  - **texte brut, pas de markdown.** Un rendu à moitié interprété (des
  //    `**` qui s'affichent ici et gras là) est pire que pas de rendu du tout ;
  //  - **sauvegarde à la perte du focus**, sans bouton. Une note se prend en
  //    passant ; un bouton « Enregistrer » à côté d'un champ qu'on quitte pour
  //    aller cliquer ailleurs est exactement le moyen de la perdre ;
  //  - **la phrase qui rassure s'affiche pendant la saisie**, au moment où on
  //    se demande si on écrit dans les fichiers du mod. Même convention que le
  //    renommage (`InlineEdit`).
  import { t } from "$lib/i18n/index.svelte";
  import Pencil from "./Pencil.svelte";

  interface Props {
    /** Note enregistrée, `null` quand il n'y en a pas. */
    value: string | null;
    /** Reçoit `null` quand le champ est vidé. Doit remonter une erreur
     * d'écriture à l'appelant : une note qu'on croit prise et qui n'est pas
     * écrite est pire que pas de note. */
    onsave: (value: string | null) => void;
    /** Sans la carte ni son bandeau : le bloc est alors le contenu d'un
     * sous-onglet, qui porte déjà le titre et le marqueur (§7.4). */
    bare?: boolean;
  }
  let { value, onsave, bare = false }: Props = $props();

  let editing = $state(false);
  let draft = $state("");
  let field = $state<HTMLTextAreaElement | null>(null);

  function open() {
    draft = value ?? "";
    editing = true;
  }

  function commit() {
    editing = false;
    const next = draft.trim();
    // Rien de neuf à écrire : ouvrir une note pour la relire ne doit pas
    // repartir en commande, ni marquer le mod comme modifié.
    if ((next || null) === (value ?? null)) return;
    onsave(next.length ? next : null);
  }

  function onkeydown(e: KeyboardEvent) {
    // Échap annule la saisie en cours ; Entrée va à la ligne, comme dans
    // n'importe quelle zone de texte — c'est le focus perdu qui enregistre.
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      editing = false;
    }
  }

  $effect(() => {
    if (editing && field) field.focus();
  });
</script>

{#snippet body()}
  {#if editing}
    <textarea
      class="input"
      bind:this={field}
      bind:value={draft}
      rows="5"
      placeholder={t("notes.placeholder")}
      onblur={commit}
      onkeydown={onkeydown}
    ></textarea>
    <p class="hint">{t("detail.editStoredInApp")}</p>
  {:else if value}
    <!-- Bouton et pas `<div onclick>` : on doit pouvoir y revenir au clavier
         et à la manette comme sur n'importe quel champ de la fiche. -->
    <button class="text" type="button" onclick={open} title={t("notes.edit")}>{value}</button>
  {:else}
    <button class="add" type="button" onclick={open}>{t("notes.add")}</button>
  {/if}
{/snippet}

{#if bare}
  {@render body()}
{:else}
  <section class="blk">
    <header class="blk-h">
      <span class="blk-t">{t("notes.title")}</span>
      <!-- Même crayon que le nom et la description, et il ouvre le champ : le
           premier jet posait ici un ✎ décoratif, qui ressemblait au crayon des
           autres blocs sans rien faire au clic. -->
      {#if value && !editing}<Pencil label={t("notes.edit")} onclick={open} />{/if}
    </header>
    <div class="blk-b">{@render body()}</div>
  </section>
{/if}

<style>
  /* `pre-wrap` : les retours à la ligne saisis sont le seul formatage qu'une
     note en texte brut possède, les perdre à l'affichage reviendrait à ne pas
     les avoir acceptés. */
  .text {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    padding: 0;
    color: var(--txt2);
    font: inherit;
    font-size: 12px;
    line-height: 1.55;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    cursor: text;
  }
  .add {
    background: transparent;
    border: 1px dashed var(--line);
    color: var(--muted);
    font-size: 11.5px;
    padding: 7px 12px;
    width: 100%;
    text-align: left;
  }
  .add:hover {
    border-color: var(--faint2);
    color: var(--txt2);
  }
  textarea {
    resize: vertical;
    font-size: 12px;
    line-height: 1.55;
  }
  .hint {
    color: var(--muted2);
    font-size: 10.5px;
    margin-top: 6px;
  }
</style>
