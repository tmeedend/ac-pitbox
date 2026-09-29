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
import { activateMod, deactivateMod, openModFolder } from "./library";
import { exportMod } from "$lib/workshop/maintenance";
import { bulkActivate, bulkDeactivate, bulkExport } from "./bulkEdit";
import { exportToReport, runBulkOp } from "./bulkState.svelte";
import { deleteMods, isPlayable } from "./showcase.svelte";
import { recoverMods } from "./recovery.svelte";
import { canStartUpdate } from "./modUpdates.svelte";
import { open, message } from "@tauri-apps/plugin-dialog";
import { nav, requestSection, queueOpponentsAction } from "$lib/shell/nav.svelte";
import { t } from "$lib/i18n/index.svelte";

import { errorText } from "$lib/errors";
export interface ModContextTarget {
  id_interne: string;
  is_stock: boolean;
  active: boolean;
  display_name: string | null;
  kind: "Car" | "Track";
  /** The card's own image — what the showcase freezes when nothing else is
   * preferred (ESPACE§3.3). */
  preview: string | null;
  /** In the showcase (ESPACE§4.1): no activation, no export, no session. */
  showcase: boolean;
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

  // Several mods in the showcase: their files are fetched one after the
  // other (ESPACE§7.2). One alone has its fiche, where every source is listed.
  const showcased = targets.filter((m) => m.showcase);
  if (!single && showcased.length) {
    items.push({
      label: t("showcase.recoverN", { count: showcased.length }),
      onclick: async () => {
        if (!canStartUpdate()) {
          await message(t("showcase.recoverBusy"), { title: t("showcase.recover"), kind: "info" });
          return;
        }
        void recoverMods(showcased);
      },
    });
  }

  if (mods.length) {
    if (single?.showcase) {
      // Its files are gone: what it needs is the way back, on its fiche
      // (ESPACE§7.1), not an activation that would be refused.
      items.push({ label: t("showcase.recover"), onclick: () => (nav.openFull = single.id_interne) });
    } else if (single) {
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

  // A car in the showcase never joins a grid (ESPACE§6).
  const racers = targets.filter(isPlayable);
  if (racers.length && racers.every((m) => m.kind === "Car")) {
    const allIds = racers.map((m) => m.id_interne);
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

  // An archive of a skeleton would be a mod without its files (ESPACE§6).
  const exportable = mods.filter(isPlayable);
  const exportIds = exportable.map((m) => m.id_interne);
  if (exportable.length) {
    items.push({
      label: label("modpanel.exportFull", "modpanel.exportFullN", exportable.length),
      onclick: async () => {
        try {
          const dir = await open({ directory: true, multiple: false, title: t("detail.exportDirTitle") });
          if (!dir || typeof dir !== "string") return;
          if (exportIds.length === 1) await exportMod(exportIds[0], dir);
          else
            await runBulkOp("export", exportIds.length, async () =>
              exportToReport(await bulkExport(exportIds, dir), exportIds.length),
            );
        } catch (e) {
          await reportError(e);
        }
      },
    });
  }

  // One gesture, and the confirmation offers the showcase or the complete
  // deletion (ESPACE§5.1). Mods already in the showcase have only the second.
  const allShowcase = mods.every((m) => m.showcase);
  items.push({
    label: allShowcase
      ? label("showcase.deleteCompletelyMenu", "showcase.deleteCompletelyMenuN", mods.length)
      : label("detail.deleteFromLibrary", "modpanel.ctxDeleteN", mods.length),
    danger: true,
    onclick: async () => {
      try {
        if (await deleteMods(mods)) onchange();
      } catch (e) {
        await reportError(e);
      }
    },
  });

  return items;
}
