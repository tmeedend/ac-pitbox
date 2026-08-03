// Actions de clic droit partagées entre la liste (Library.svelte, cartes/lignes)
// et le panneau compact (ModDetail.svelte) — activer/désactiver, ouvrir dossier,
// exporter, supprimer. Pas de « réinstaller » ici (nécessite l'archive source
// conservée de la version active, seulement connue via la fiche complète) —
// reste disponible sur la fiche détail.
import { activateMod, deactivateMod, openModFolder } from "$lib/library";
import { exportMod, deleteBrokenMod } from "$lib/maintenance";
import { open, confirm, message } from "@tauri-apps/plugin-dialog";
import { nav } from "$lib/nav.svelte";
import { t } from "$lib/i18n/index.svelte";

import { errorText } from "$lib/errors";
export interface ModContextTarget {
  id_interne: string;
  is_stock: boolean;
  active: boolean;
  display_name: string | null;
}

export interface ModContextItem {
  label: string;
  onclick: () => void;
  danger?: boolean;
}

async function reportError(e: unknown) {
  await message(errorText(e), { title: t("common.error"), kind: "error" });
}

export function buildModContextItems(m: ModContextTarget, onchange: () => void): ModContextItem[] {
  const items: ModContextItem[] = [
    { label: t("modpanel.ctxOpenDetail"), onclick: () => (nav.openFull = m.id_interne) },
  ];

  if (!m.is_stock) {
    items.push({
      label: m.active ? t("common.deactivate") : t("common.activate"),
      onclick: async () => {
        try {
          if (m.active) await deactivateMod(m.id_interne);
          else await activateMod(m.id_interne);
          onchange();
        } catch (e) {
          await reportError(e);
        }
      },
    });
  }

  items.push({
    label: t("detail.openFolder"),
    onclick: () => {
      openModFolder(m.id_interne).catch(reportError);
    },
  });
  if (m.is_stock) return items;

  items.push({
    label: t("modpanel.exportFull"),
    onclick: async () => {
      try {
        const dir = await open({ directory: true, multiple: false, title: t("detail.exportDirTitle") });
        if (!dir || typeof dir !== "string") return;
        await exportMod(m.id_interne, dir);
      } catch (e) {
        await reportError(e);
      }
    },
  });

  items.push({
    label: t("detail.deleteFromLibrary"),
    danger: true,
    onclick: async () => {
      const ok = await confirm(t("detail.deleteConfirm", { name: m.display_name ?? m.id_interne }), {
        title: t("detail.deleteTitle"),
        kind: "warning",
      });
      if (!ok) return;
      try {
        await deleteBrokenMod(m.id_interne);
        onchange();
      } catch (e) {
        await reportError(e);
      }
    },
  });

  return items;
}
