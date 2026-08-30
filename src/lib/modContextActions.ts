// Actions de clic droit de la liste (Library.svelte, cartes et lignes) —
// activer/désactiver, ouvrir dossier, exporter, supprimer. Elles étaient
// partagées avec le panneau compact, retiré depuis ; le module reste séparé
// parce que la liste n'a pas à porter cette table d'actions. Pas de « réinstaller » ici (nécessite l'archive source
// conservée de la version active, seulement connue via la fiche complète) —
// reste disponible sur la fiche détail.
//
// Le menu agit sur une **sélection**, pas sur une carte : clic droit sur un
// mod déjà sélectionné à plusieurs, et l'action porte sur tout le lot (§6.3ter).
// C'est ce qui permet au panneau de sélection groupée de se limiter à ce qu'un
// menu ne peut pas porter — un champ de saisie (catégorie, tag).
import { activateMod, deactivateMod, openModFolder } from "$lib/library";
import { exportMod, deleteBrokenMod } from "$lib/maintenance";
import { bulkActivate, bulkDeactivate, bulkDelete, bulkExport } from "$lib/bulkEdit";
import { exportToReport, runBulkOp } from "$lib/bulkState.svelte";
import { open, confirm, message } from "@tauri-apps/plugin-dialog";
import { nav, requestSection, queueOpponentsAction } from "$lib/nav.svelte";
import { t } from "$lib/i18n/index.svelte";

import { errorText } from "$lib/errors";
export interface ModContextTarget {
  id_interne: string;
  is_stock: boolean;
  active: boolean;
  display_name: string | null;
  kind: "Car" | "Track";
}

// Même action que le panneau de sélection groupée (§6.3ter) — pose l'action
// puis navigue vers l'écran de réglages, où Launch.svelte la consomme une fois
// prêt.
async function sendAsOpponents(ids: string[], mode: "set" | "add") {
  queueOpponentsAction(mode, ids);
  if (!(await requestSection("race"))) nav.opponentsAction = null;
}

export interface ModContextItem {
  label: string;
  onclick: () => void;
  danger?: boolean;
}

async function reportError(e: unknown) {
  await message(errorText(e), { title: t("common.error"), kind: "error" });
}

/** Libellé d'une action : au singulier tel quel, au pluriel avec son décompte.
 * Sans le décompte, rien ne distingue « je supprime celui que je vise » de
 * « je supprime les douze sélectionnés » — la même phrase pour deux gestes
 * dont l'un est irréversible. */
function label(single: string, plural: string, n: number): string {
  return n > 1 ? t(plural, { count: n }) : t(single);
}

export function buildModContextItems(targets: ModContextTarget[], onchange: () => void): ModContextItem[] {
  if (!targets.length) return [];
  const items: ModContextItem[] = [];
  const single = targets.length === 1 ? targets[0] : null;

  // Une fiche s'ouvre pour un mod, pas pour douze.
  if (single) {
    items.push({ label: t("modpanel.ctxOpenDetail"), onclick: () => (nav.openFull = single.id_interne) });
  }

  // Le contenu de base ne s'active, ne s'exporte et ne se supprime pas : il est
  // écarté de ces actions plutôt que de faire échouer le lot ligne par ligne.
  const mods = targets.filter((m) => !m.is_stock);
  const ids = mods.map((m) => m.id_interne);

  if (mods.length) {
    if (single) {
      items.push({
        label: single.active ? t("common.deactivate") : t("common.activate"),
        onclick: async () => {
          try {
            if (single.active) await deactivateMod(single.id_interne);
            else await activateMod(single.id_interne);
            onchange();
          } catch (e) {
            await reportError(e);
          }
        },
      });
    } else {
      // À plusieurs, les mods visés ne sont pas tous dans le même état : les
      // deux actions sont proposées, aucune ne se devine d'une bascule.
      items.push(
        {
          label: t("modpanel.ctxActivateN", { count: mods.length }),
          onclick: () => void runBulkOp("activate", ids.length, () => bulkActivate(ids)).then(onchange),
        },
        {
          label: t("modpanel.ctxDeactivateN", { count: mods.length }),
          onclick: () => void runBulkOp("deactivate", ids.length, () => bulkDeactivate(ids)).then(onchange),
        },
      );
    }
  }

  if (targets.every((m) => m.kind === "Car")) {
    const allIds = targets.map((m) => m.id_interne);
    items.push(
      {
        label: label("modpanel.ctxSetOpponent", "modpanel.ctxSetOpponentN", allIds.length),
        onclick: () => sendAsOpponents(allIds, "set"),
      },
      {
        label: label("modpanel.ctxAddOpponent", "modpanel.ctxAddOpponentN", allIds.length),
        onclick: () => sendAsOpponents(allIds, "add"),
      },
    );
  }

  // Ouvrir douze explorateurs d'un clic est hostile : réservé au mod unique.
  if (single) {
    items.push({
      label: t("detail.openFolder"),
      onclick: () => {
        openModFolder(single.id_interne).catch(reportError);
      },
    });
  }

  if (!mods.length) return items;

  items.push({
    label: label("modpanel.exportFull", "modpanel.exportFullN", mods.length),
    onclick: async () => {
      try {
        const dir = await open({ directory: true, multiple: false, title: t("detail.exportDirTitle") });
        if (!dir || typeof dir !== "string") return;
        if (ids.length === 1) await exportMod(ids[0], dir);
        else await runBulkOp("export", ids.length, async () => exportToReport(await bulkExport(ids, dir), ids.length));
      } catch (e) {
        await reportError(e);
      }
    },
  });

  items.push({
    label: label("detail.deleteFromLibrary", "modpanel.ctxDeleteN", mods.length),
    danger: true,
    onclick: async () => {
      const ok = await confirm(
        mods.length > 1
          ? t("bulkEdit.confirmDelete", { count: mods.length })
          : t("detail.deleteConfirm", { name: mods[0].display_name ?? mods[0].id_interne }),
        { title: t("detail.deleteTitle"), kind: "warning" },
      );
      if (!ok) return;
      try {
        if (ids.length === 1) await deleteBrokenMod(ids[0]);
        else await runBulkOp("delete", ids.length, () => bulkDelete(ids));
        onchange();
      } catch (e) {
        await reportError(e);
      }
    },
  });

  return items;
}
