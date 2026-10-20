<script lang="ts">
  // Edition sur place de l'identité d'une voiture ou d'un circuit : nom, marque
  // (voitures) et année, depuis le crayon de l'en-tête.
  //
  // Même promesse qu'`InlineEdit` pour le nom, étendue aux deux autres : rien
  // n'est écrit dans les fichiers du mod (règle d'or n°1), chaque saisie vit
  // dans la base de Pit Box à côté de la valeur du mod, et chaque champ
  // surchargé propose d'y revenir. Vider un champ revient aussi à l'origine —
  // le geste naturel pour annuler, comme pour `InlineEdit`.
  import Pencil from "$lib/components/ui/Pencil.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import {
    parseYear,
    textChange,
    yearChange,
    type FicheIdentity,
    type IdentityField,
    type IdentityPatch,
  } from "$lib/detail/identityEdit";

  interface Props {
    identity: FicheIdentity;
    /** Lets the header hide the name and the subtitle while the form is open:
     * the form is the title then, not a field squeezed next to it. */
    editing?: boolean;
  }
  let { identity, editing = $bindable(false) }: Props = $props();

  let name = $state("");
  let brand = $state("");
  let year = $state("");
  let suggestions = $state<string[]>([]);
  let form = $state<HTMLElement | null>(null);

  const yearInvalid = $derived(year.trim() !== "" && parseYear(year) === null);

  function open() {
    name = identity.name.value ?? "";
    brand = identity.brand?.value ?? "";
    year = identity.year.value != null ? String(identity.year.value) : "";
    editing = true;
    // A failed suggestion list costs the autocompletion, not the edit.
    identity.brand
      ?.suggestions()
      .then((list) => (suggestions = list))
      .catch((e) => console.warn("brand suggestions", e));
  }

  function commit() {
    if (yearInvalid) return;
    const patch: IdentityPatch = {};
    const n = textChange(name, identity.name);
    if (n) patch.name = n.change;
    if (identity.brand) {
      const b = textChange(brand, identity.brand);
      if (b) patch.brand = b.change;
    }
    const y = yearChange(year, identity.year);
    if (y) patch.year = y.change;
    editing = false;
    if (Object.keys(patch).length) identity.onsave(patch);
  }

  function cancel() {
    editing = false;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      cancel();
    } else if (e.key === "Enter") {
      e.preventDefault();
      commit();
    }
  }

  /** The brand field leaves Enter alone: with its suggestions open it picks one,
   * and must not save a half-typed brand along with the other two fields. */
  function brandKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onkeydown(e);
  }

  // Focus on the name at opening: the pencil is for writing, not for revealing
  // a field that still needs a click. The name is the form's first input.
  $effect(() => {
    if (editing) form?.querySelector("input")?.select();
  });
</script>

{#if editing}
  <div class="edit" bind:this={form}>
    {@render row(t("columns.name"), identity.name, name, (v) => (name = v))}
    {#if identity.brand}
      {@render row(t("columns.brand"), identity.brand, brand, (v) => (brand = v), { list: "identity-brands", onkeydown: brandKeydown })}
      <datalist id="identity-brands">
        {#each suggestions as s (s)}<option value={s}></option>{/each}
      </datalist>
    {/if}
    {@render row(t("columns.year"), identity.year, year, (v) => (year = v), { inputmode: "numeric" }, yearInvalid)}
    <!-- The reassurance is HERE, where one types: a tooltip on the pencil is
         gone at the very moment it would be needed. -->
    <p class="note">{t("detail.editStoredInApp")}</p>
    <div class="acts">
      <button class="btn" type="button" onclick={cancel}>{t("common.cancel")}</button>
      <button class="btn btn-primary" type="button" onclick={commit} disabled={yearInvalid}>
        {t("settings.save")}
      </button>
    </div>
  </div>
{:else}
  <Pencil label={t("detail.editIdentity")} onclick={open} />
{/if}

{#snippet row(
  label: string,
  field: IdentityField<string | number>,
  text: string,
  set: (v: string) => void,
  attrs: Record<string, unknown> = {},
  invalid = false,
)}
  <div class="f">
    <span class="lbl-key">{label}</span>
    <div class="line">
      <input
        class="input"
        class:bad={invalid}
        type="text"
        autocomplete="off"
        aria-label={label}
        value={text}
        oninput={(e) => set(e.currentTarget.value)}
        placeholder={field.original != null ? String(field.original) : ""}
        {onkeydown}
        {...attrs}
      />
      {#if field.overridden}
        <!-- Puts the original in the field: saving then gives the decision up
             (`textChange`/`yearChange`), and the field shows what it goes back to. -->
        <button
          class="rev"
          type="button"
          onclick={() => set(field.original != null ? String(field.original) : "")}
        >
          {t("detail.editRevert")}
        </button>
      {/if}
    </div>
    {#if field.overridden && field.original != null}
      <span class="orig">{t("detail.editOriginal", { value: String(field.original) })}</span>
    {/if}
  </div>
{/snippet}

<style>
  .edit {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    max-width: 460px;
    min-width: 0;
  }
  .f {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  /* The column of the "back to original" link is always there, so the three
     fields have one width whichever of them is a decision. */
  .line {
    display: grid;
    grid-template-columns: 1fr 9em;
    align-items: center;
    gap: 8px;
  }
  .line .input {
    min-width: 0;
  }
  .input.bad {
    border-color: var(--rosso);
  }
  .rev {
    justify-self: start;
    background: transparent;
    border: none;
    padding: 0;
    color: var(--muted);
    font-size: 11px;
    cursor: pointer;
    text-decoration: underline;
  }
  .rev:hover,
  .rev:focus-visible {
    color: var(--rosso-bright);
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
    /* A long original name must not stretch the sheet. */
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
