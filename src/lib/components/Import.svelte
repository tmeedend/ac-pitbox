<script lang="ts">
  // Écran d'import dédié (§4.6bis) : remplace l'ancienne barre de boutons en
  // haut de la bibliothèque — trop discrète pour expliquer les choix, et
  // limitée à Voitures/Circuits. Le glisser-déposer reste le geste rapide,
  // disponible partout dans l'app (voir initGlobalDragDrop dans AppShell).
  import { open } from "@tauri-apps/plugin-dialog";
  import BulkImport from "./BulkImport.svelte";
  import {
    importState,
    setCopyMode,
    pickAndImportArchive,
    pickAndImportFolder,
    reportBulkDone,
  } from "$lib/importState.svelte";

  let bulkParent = $state<string | null>(null);
  async function pickBulkImport() {
    const sel = await open({ directory: true, multiple: false });
    if (sel && typeof sel === "string") bulkParent = sel;
  }
</script>

<div class="import-screen">
  <header class="head">
    <h2>Importer</h2>
    <p class="sub">
      Ajoute des mods à la bibliothèque. Un mod importé (ou mis à jour) est
      <b>activé tout de suite</b> — pas d'étape séparée pour pouvoir le
      conduire.
    </p>
  </header>

  <section class="cards">
    <div class="card">
      <h3>Une archive</h3>
      <p class="hint">
        Le cas courant : un fichier <span class="mono">.zip</span>,
        <span class="mono">.rar</span> ou <span class="mono">.7z</span>
        téléchargé tel quel. L'app l'extrait, détecte le type (voiture, circuit,
        skin, son…) et le range dans la bibliothèque.
      </p>
      <button class="btn btn-primary" type="button" onclick={pickAndImportArchive} disabled={importState.importing}>
        {importState.importing ? "Import…" : "Choisir une archive…"}
      </button>
    </div>

    <div class="card">
      <h3>Un dossier déjà décompressé</h3>
      <p class="hint">
        Pour un mod que tu as déjà extrait toi-même, sans repasser par
        l'archive d'origine.
      </p>
      <div class="copy-choice">
        <span class="cc-label">À l'import :</span>
        <div class="copy-toggle">
          <button class:on={importState.copyMode} onclick={() => setCopyMode(true)}>Copier</button>
          <button class:on={!importState.copyMode} onclick={() => setCopyMode(false)}>Déplacer</button>
        </div>
      </div>
      <p class="hint small">
        <b>Copier</b> (recommandé) laisse le dossier source intact.
        <b>Déplacer</b> vide le dossier source — plus rapide s'il est sur le
        même disque que la bibliothèque (simple renommage), sinon revient à
        une copie suivie d'une suppression.
      </p>
      <button class="btn" type="button" onclick={pickAndImportFolder} disabled={importState.importing} title="Import unitaire d'un dossier (§4.5)">
        {importState.importing ? "Import…" : "Choisir un dossier…"}
      </button>
    </div>
  </section>

  <section class="mass">
    <h3>Import en masse</h3>
    <p class="hint">
      Pour migrer un catalogue entier en une fois — par exemple un dossier
      <span class="mono">mods/</span> venant de Mod Organizer 2, où
      <b>chaque sous-dossier direct est un mod</b>. L'app analyse d'abord tout
      sans rien écrire (nouveaux, mises à jour, doublons, cas ambigus), tu
      arbitres les cas ambigus <b>en une fois</b>, puis l'import s'exécute —
      il peut reprendre si interrompu.
    </p>
    <button class="btn" type="button" onclick={pickBulkImport} disabled={importState.importing}>
      Choisir un dossier parent…
    </button>
  </section>

  <section class="dnd">
    <h3>Glisser-déposer</h3>
    <p class="hint">
      Le geste rapide : dépose une archive n'importe où dans la fenêtre, sur
      n'importe quel écran — l'import démarre immédiatement, avec le mode
      copier/déplacer choisi ci-dessus pour un éventuel dossier.
    </p>
  </section>
</div>

{#if bulkParent}
  <BulkImport
    parent={bulkParent}
    copy={importState.copyMode}
    onclose={() => (bulkParent = null)}
    ondone={(r) => {
      reportBulkDone(r);
      bulkParent = null;
    }}
  />
{/if}

<style>
  .import-screen {
    max-width: 760px;
  }
  .head {
    margin-bottom: 22px;
  }
  h2 {
    font-size: 18px;
    font-weight: 600;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
    max-width: 560px;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted);
    margin-bottom: 8px;
  }
  .cards {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    margin-bottom: 24px;
  }
  .card {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .hint {
    font-size: 11.5px;
    color: var(--faint);
    line-height: 1.55;
  }
  .hint.small {
    font-size: 11px;
  }
  .hint b {
    color: var(--txt2);
  }
  .copy-choice {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .cc-label {
    font-size: 11px;
    color: var(--muted);
  }
  .copy-toggle {
    display: flex;
    border: 1px solid var(--line);
  }
  .copy-toggle button {
    background: var(--panel);
    color: var(--muted);
    font-size: 10.5px;
    padding: 6px 10px;
    border-right: 1px solid var(--line);
  }
  .copy-toggle button:last-child {
    border-right: none;
  }
  .copy-toggle button.on {
    background: var(--raised);
    color: var(--rosso-bright);
  }
  .mass,
  .dnd {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 14px 16px;
    margin-bottom: 16px;
  }
  .mass .hint,
  .dnd .hint {
    max-width: 620px;
    margin-bottom: 12px;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 7px 14px;
    align-self: flex-start;
  }
  .btn.btn-primary {
    background: var(--rosso);
    color: #fff;
    border-color: var(--rosso);
  }
  .btn:disabled {
    opacity: 0.5;
  }
</style>
