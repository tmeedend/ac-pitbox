// The words for readiness, shared by the list and the panel so a level reads
// the same in both. Kept out of `readiness.ts`, which Vitest loads without
// the i18n runtime. Keys written out, so the locale check sees each one.
import { t } from "$lib/i18n/index.svelte";
import type { Blocker, Level } from "./online";

const LEVEL_KEYS: Record<Level, string> = {
  ready: "online.levelReady",
  oneClick: "online.levelOneClick",
  download: "online.levelDownload",
  blocked: "online.levelBlocked",
};

export function levelText(level: Level): string {
  return t(LEVEL_KEYS[level]);
}

export function blockerText(b: Blocker): string {
  if (b.kind === "dlc") return t("online.blockedDlc", { name: b.name });
  return b.installed === null
    ? t("online.blockedCspMissing", { build: b.required })
    : t("online.blockedCspOld", { build: b.required, installed: b.installed });
}
