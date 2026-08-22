<script lang="ts">
  // Édition sur place d'un champ que l'utilisateur surcharge (§5bis.3) : nom
  // et description d'un mod. Le crayon ouvre le champ à l'endroit même où la
  // valeur s'affiche, plutôt que d'envoyer vers un formulaire ailleurs.
  //
  // Un seul composant pour les deux, parce que les deux posent exactement les
  // mêmes questions : où va la saisie, comment y renoncer, et pourquoi elle ne
  // sera pas perdue. Trois réponses recopiées à deux endroits auraient divergé
  // à la première retouche (§ chantier « composants partagés »).
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    /** Valeur affichée aujourd'hui : la saisie de l'utilisateur si elle
     * existe, sinon ce qu'annonce le fichier du mod. */
    value: string | null;
    /** Valeur d'origine (fichier du mod), pour proposer d'y revenir et pour
     * la montrer en repère pendant l'édition. `null` = le mod n'en donne pas. */
    original: string | null;
    /** Vrai quand la valeur affichée vient d'une saisie de l'utilisateur. */
    overridden: boolean;
    /** Champ multiligne (description) plutôt qu'une ligne (nom). */
    multiline?: boolean;
    label: string;
    placeholder?: string;
    /** `null` = renoncer à la surcharge et revenir au fichier du mod. */
    onsave: (value: string | null) => void;
  }
  const { value, original, overridden, multiline = false, label, placeholder, onsave }: Props = $props();

  let editing = $state(false);
  let draft = $state("");
  let field = $state<HTMLInputElement | HTMLTextAreaElement | null>(null);

  function open() {
    draft = value ?? "";
    editing = true;
  }

  function commit() {
    const next = draft.trim();
    // Vider le champ vaut « reviens au fichier du mod » : c'est le geste
    // naturel pour annuler une surcharge, et il évite d'avoir à distinguer
    // « vide » de « pas de surcharge », qui n'ont aucune différence utile.
    onsave(next.length ? next : null);
    editing = false;
  }

  function cancel() {
    editing = false;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      cancel();
      return;
    }
    // Entrée valide sur une ligne ; sur un champ multiligne elle sert à aller
    // à la ligne, donc c'est Ctrl+Entrée qui valide (convention des zones de
    // commentaire un peu partout).
    if (e.key === "Enter" && (!multiline || e.ctrlKey)) {
      e.preventDefault();
      commit();
    }
  }

  // Focus posé à l'ouverture : le crayon sert à écrire, pas à révéler un champ
  // sur lequel il faudrait encore cliquer.
  $effect(() => {
    if (editing && field) field.select();
  });
</script>

{#if editing}
  <div class="edit" class:multi={multiline}>
    {#if multiline}
      <textarea
        class="input"
        bind:this={field}
        bind:value={draft}
        rows="6"
        {placeholder}
        onkeydown={onkeydown}
      ></textarea>
    {:else}
      <input class="input" type="text" bind:this={field} bind:value={draft} {placeholder} onkeydown={onkeydown} />
    {/if}
    <!-- La phrase qui rassure est ICI, au moment de la saisie : une infobulle
         sur le crayon disparaît à l'instant précis où on en aurait besoin. -->
    <p class="note">{t("detail.editStoredInApp")}</p>
    {#if original}
      <p class="orig">{t("detail.editOriginal", { value: original })}</p>
    {/if}
    <div class="acts">
      <button class="btn" type="button" onclick={cancel}>{t("common.cancel")}</button>
      {#if overridden}
        <button class="btn" type="button" onclick={() => { onsave(null); editing = false; }}>
          {t("detail.editRevert")}
        </button>
      {/if}
      <button class="btn btn-primary" type="button" onclick={commit}>{t("settings.save")}</button>
    </div>
  </div>
{:else}
  <button class="pencil" type="button" onclick={open} title={label} aria-label={label}>
    <!-- Crayon : le trait de la mine et le corps de l'outil. -->
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path d="M11.3 1.9 L14.1 4.7 L5.3 13.5 L1.8 14.2 L2.5 10.7 Z" fill="none" />
      <path d="M10.1 3.1 L12.9 5.9" fill="none" />
    </svg>
  </button>
{/if}

<style>
  .pencil {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    flex: none;
    background: transparent;
    border: none;
    color: var(--muted2);
    cursor: pointer;
    /* Effacé tant qu'on ne s'y intéresse pas : c'est une action secondaire,
       elle ne doit pas concurrencer la valeur qu'elle modifie. */
    opacity: 0.55;
    transition: opacity 0.15s ease, color 0.15s ease;
  }
  .pencil:hover,
  .pencil:focus-visible {
    opacity: 1;
    color: var(--rosso-bright);
  }
  .pencil svg {
    width: 13px;
    height: 13px;
    stroke: currentColor;
    stroke-width: 1.4;
    fill: none;
  }
  .edit {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
    min-width: 0;
  }
  .edit .input {
    width: 100%;
  }
  .edit textarea {
    resize: vertical;
    font-family: inherit;
    line-height: 1.5;
  }
  .note {
    font-size: 10.5px;
    color: var(--muted);
    line-height: 1.4;
  }
  .orig {
    font-size: 10.5px;
    color: var(--faint);
    line-height: 1.4;
    /* Un nom d'origine à rallonge ne doit pas étirer la fiche. */
    overflow-wrap: anywhere;
  }
  .acts {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
  .acts .btn {
    font-size: 11px;
    padding: 5px 10px;
  }
</style>
