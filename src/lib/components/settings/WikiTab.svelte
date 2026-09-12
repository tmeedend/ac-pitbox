<script lang="ts">
  // Réglages Wikipédia (docs/SPEC-wikipedia-fiche-detail.md §8).
  //
  // Trois réglages et un outil. L'interrupteur a une raison qui n'est pas le
  // confort : l'app interroge Wikipédia à l'ouverture d'une fiche, ce qui
  // **révèle indirectement le contenu de la bibliothèque**, et une partie du
  // public joue délibérément hors ligne. Éteint, aucune requête ne sort et le
  // cache déjà constitué reste lisible.
  //
  // L'outil, c'est l'export du §10 : les corrections faites à la main
  // repartent dans le dépôt et profitent à tout le monde à la version
  // suivante. C'est le seul endroit de l'app d'où la connaissance de
  // l'utilisateur ressort.
  import { confirm, save } from "@tauri-apps/plugin-dialog";
  import Field from "../Field.svelte";
  import { t, availableLocales, localeNames } from "$lib/i18n/index.svelte";
  import type { AppConfig } from "$lib/config";
  import { countWikiManualLinks, exportWikiLinks, purgeWikiCache, setWikiLang, wikiLang } from "$lib/wiki";
  import { getUiPref } from "$lib/uiPrefs.svelte";

  interface Props {
    config: AppConfig;
  }
  const { config = $bindable() }: Props = $props();

  /** Langue de lecture : vide = suivre celle de l'application (§5.1). */
  let lang = $state("");
  let manualCount = $state(0);
  let busy = $state(false);
  /** Ce que la dernière action a produit, dit sur place plutôt qu'en toast :
   * on est dans un écran de réglages, on y reste. */
  let done = $state<string | null>(null);

  $effect(() => {
    // Lecture unique à l'ouverture. `getUiPref` remplit le cache que
    // `wikiLang()` lit ensuite de façon synchrone.
    void getUiPref("pitbox.wiki.lang").then((value) => (lang = value ?? ""));
    void countWikiManualLinks().then((n) => (manualCount = n));
  });

  async function onLang(value: string) {
    lang = value;
    await setWikiLang(value || null);
  }

  async function onPurge() {
    const ok = await confirm(t("wiki.purgeConfirm"), { title: t("wiki.purge"), kind: "warning" });
    if (!ok) return;
    busy = true;
    await purgeWikiCache();
    busy = false;
    done = t("wiki.purgeDone");
  }

  async function onExport() {
    const path = await save({
      title: t("wiki.export"),
      defaultPath: "wiki-links.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    busy = true;
    // Le fichier est écrit côté Rust : le chemin vient de la boîte de dialogue
    // du système, donc de l'utilisateur lui-même — même modèle de confiance que
    // l'export de mods, et une dépendance de moins.
    const ok = await exportWikiLinks(path);
    busy = false;
    done = ok ? t("wiki.exportDone", { count: manualCount }) : t("wiki.exportFailed");
  }
</script>

<div class="tab">
  <Field hint={t("wiki.onlineHint")}>
    <label class="check">
      <input type="checkbox" bind:checked={config.prefs.wiki_online} />
      <span>{t("wiki.online")}</span>
    </label>
  </Field>

  <Field label={t("wiki.readingLang")} hint={t("wiki.readingLangHint")}>
    <select class="input" value={lang} onchange={(e) => onLang((e.currentTarget as HTMLSelectElement).value)}>
      <option value="">{t("wiki.readingLangAuto")}</option>
      {#each availableLocales as code (code)}
        <option value={code}>{localeNames[code] ?? code}</option>
      {/each}
    </select>
  </Field>

  <Field label={t("wiki.export")} hint={t("wiki.exportHint")}>
    <div class="row">
      <button class="btn" type="button" onclick={onExport} disabled={busy || manualCount === 0}>
        {t("wiki.exportAction", { count: manualCount })}
      </button>
    </div>
  </Field>

  <Field label={t("wiki.purge")} hint={t("wiki.purgeHint")}>
    <div class="row">
      <button class="btn" type="button" onclick={onPurge} disabled={busy}>{t("wiki.purgeAction")}</button>
    </div>
  </Field>

  {#if done}
    <p class="done">{done}</p>
  {/if}
</div>

<style>
  .tab {
    display: flex;
    flex-direction: column;
    max-width: 620px;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .done {
    margin: 14px 0 0;
    font-size: 12px;
    color: var(--muted);
  }
</style>
