// The words for readiness, shared by the list and the panel so a level reads
// the same in both. Kept out of `readiness.ts`, which Vitest loads without
// the i18n runtime. Keys written out, so the locale check sees each one.
import { t } from "$lib/i18n/index.svelte";
import type { Blocker, Level, Rule } from "./online";
import type { Duration } from "./sessions";

export function durationText(d: Duration): string {
  if (d.unit === "laps") return t("online.laps", { n: d.laps });
  return d.unit === "h" ? t("online.hours", { n: d.hours }) : t("online.minutes", { n: d.minutes });
}

const LEVEL_KEYS: Record<Level, string> = {
  ready: "online.levelReady",
  oneClick: "online.levelOneClick",
  download: "online.levelDownload",
  blocked: "online.levelBlocked",
};

export function levelText(level: Level): string {
  return t(LEVEL_KEYS[level]);
}

/** A rule as a departure from the norm ("ABS forced off", "Damage 0 %"). */
export function ruleText(r: Rule): string {
  switch (r.kind) {
    case "absDenied":
      return t("online.ruleAbsDenied");
    case "absForced":
      return t("online.ruleAbsForced");
    case "tcDenied":
      return t("online.ruleTcDenied");
    case "tcForced":
      return t("online.ruleTcForced");
    case "stabilityAllowed":
      return t("online.ruleStabilityAllowed");
    case "autoclutchDenied":
      return t("online.ruleAutoclutchDenied");
    case "tyreBlanketsAllowed":
      return t("online.ruleTyreBlankets");
    case "virtualMirrorForced":
      return t("online.ruleVirtualMirror");
    case "damage":
      return t("online.ruleDamage", { percent: r.percent });
    case "fuel":
      return t("online.ruleFuel", { percent: r.percent });
    case "tyreWear":
      return t("online.ruleTyreWear", { percent: r.percent });
    case "tyresOut":
      return t("online.ruleTyresOut", { count: r.count });
  }
}

export function blockerText(b: Blocker): string {
  if (b.kind === "dlc") return t("online.blockedDlc", { name: b.name });
  return b.installed === null
    ? t("online.blockedCspMissing", { build: b.required })
    : t("online.blockedCspOld", { build: b.required, installed: b.installed });
}
