<script lang="ts">
  // Édition groupée (§6.3bis/§6.3ter) : panneau bas en surimpression quand
  // plusieurs mods sont sélectionnés (Ctrl/Maj-clic, Ctrl+A) — il flotte
  // par-dessus la grille sans en réduire la largeur.
  //
  // Il ne garde que **ce qu'un menu contextuel ne peut pas porter** : un champ
  // de saisie (catégorie, tag) et une paire de boutons sans argument (favori).
  // Activer, désactiver, supprimer, exporter et « envoyer en adversaires »
  // sont partis au clic droit, qui agit désormais sur toute la sélection —
  // deux endroits pour la même action, c'était un endroit de trop pour la
  // chercher. La progression et le rapport d'un lot vivent dans la pile de
  // notifications (`BulkToasts`), pas ici : un rapport enfermé dans ce panneau
  // partait avec lui.
  //
  // N'expose que les champs communs à tout mod — jamais ceux propres à un type
  // (specs voiture, skin piloté, version active), réservés à la fiche détail.
  import type { ModCard } from "$lib/library";
  import { bulkSetFavorite, bulkSetCategory, bulkAddTag, bulkRemoveTag } from "$lib/bulkEdit";
  import { t } from "$lib/i18n/index.svelte";

  import { errorText } from "$lib/errors";
  interface Props {
    ids: string[];
    cards: ModCard[];
    onclose: () => void;
    onchange: () => void;
  }
  let { ids, cards, onclose, onchange }: Props = $props();

  let busy = $state(false);
  let error = $state("");
  let categoryInput = $state("");
  let tagInput = $state("");

  const categories = $derived(
    [...new Set(cards.map((c) => c.category).filter((c): c is string => !!c))].sort(),
  );

  // Ces quatre actions sont quelques écritures SQLite : pas de progression, pas
  // de rapport — une barre y serait un clignotement (§6.3bis).
  async function run(action: () => Promise<void>) {
    busy = true;
    error = "";
    try {
      await action();
      onchange();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const markFavorite = () => run(() => bulkSetFavorite(ids, true));
  const unmarkFavorite = () => run(() => bulkSetFavorite(ids, false));

  function applyCategory() {
    const cat = categoryInput.trim();
    if (!cat) return;
    run(() => bulkSetCategory(ids, cat));
  }

  function addTag() {
    const tag = tagInput.trim();
    if (!tag) return;
    tagInput = "";
    run(() => bulkAddTag(ids, tag));
  }

  function removeTag() {
    const tag = tagInput.trim();
    if (!tag) return;
    tagInput = "";
    run(() => bulkRemoveTag(ids, tag));
  }
</script>

<aside class="panel">
  <header>
    <h2>{t("bulkEdit.title", { count: ids.length })}</h2>
    <ul class="chips">
      {#each cards as c (c.id_interne)}
        <li>{c.display_name ?? c.id_interne}</li>
      {/each}
    </ul>
    <button class="btn-ghost close" type="button" onclick={onclose} title={t("bulkEdit.clearTooltip")}>✕</button>
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  <div class="sections">
    <section>
      <h3 class="lbl">{t("bulkEdit.favoriteSection")}</h3>
      <div class="row">
        <button class="btn" type="button" onclick={markFavorite} disabled={busy}>{t("bulkEdit.markFavorite")}</button>
        <button class="btn" type="button" onclick={unmarkFavorite} disabled={busy}>{t("bulkEdit.unmarkFavorite")}</button>
      </div>
    </section>

    <section>
      <h3 class="lbl">{t("bulkEdit.categorySection")}</h3>
      <div class="row">
        <input
          class="input"
          list="bulk-categories"
          placeholder={t("bulkEdit.categoryPlaceholder")}
          bind:value={categoryInput}
          onkeydown={(e) => e.key === "Enter" && applyCategory()}
        />
        <datalist id="bulk-categories">
          {#each categories as cat}<option value={cat}></option>{/each}
        </datalist>
        <button class="btn" type="button" onclick={applyCategory} disabled={busy || !categoryInput.trim()}>{t("bulkEdit.apply")}</button>
      </div>
    </section>

    <section>
      <h3 class="lbl">{t("bulkEdit.tagsSection")}</h3>
      <div class="row">
        <input
          class="input"
          placeholder={t("bulkEdit.tagPlaceholder")}
          bind:value={tagInput}
          onkeydown={(e) => e.key === "Enter" && addTag()}
        />
        <button class="btn" type="button" onclick={addTag} disabled={busy || !tagInput.trim()}>{t("bulkEdit.addTagToAll")}</button>
        <button class="btn" type="button" onclick={removeTag} disabled={busy || !tagInput.trim()}>{t("bulkEdit.removeTagFromAll")}</button>
      </div>
    </section>
  </div>
</aside>

<style>
  /* Bandeau bas en surimpression (§6.3ter) : ancré au bas de `.main-wrap`
     (non-scrollant, voir Library.svelte) — flotte par-dessus la grille sans
     jamais réduire sa largeur, contrairement à l'ancien panneau latéral droit. */
  .panel {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    max-height: 46%;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--rosso-border);
    background: var(--panel2);
    box-shadow: 0 -10px 28px rgba(0, 0, 0, 0.55);
    z-index: 9;
  }
  header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    font-size: 13px;
    font-weight: 600;
    flex: none;
  }
  .close {
    font-size: 14px;
    padding: 4px 8px;
    margin-left: auto;
  }
  .chips {
    list-style: none;
    display: flex;
    gap: 6px;
    overflow-x: auto;
    font-size: 11px;
    color: var(--txt2);
    flex: 1;
    min-width: 0;
  }
  .chips li {
    padding: 3px 8px;
    background: var(--raised);
    border: 1px solid var(--line);
    white-space: nowrap;
    flex: none;
  }
  .err {
    margin: 10px 16px 0;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
    flex: none;
  }
  .sections {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 20px;
    padding: 14px 16px;
  }
  section {
    flex: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
    flex-wrap: wrap;
  }
  .row .input {
    width: 160px;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 10px;
    flex: none;
  }
  .btn:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .btn:disabled {
    opacity: 0.5;
  }
</style>
