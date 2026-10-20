// The phone frame, pure: what its status bar says. The date follows the
// story's `date` setting (SPEC-story-mode, Story settings): a real date, the
// year and month of the story, the season, or nothing.

export type DateDisplay = "relative" | "real" | "seasons" | "none";

/** The status bar's date for story date `date` (`YYYY-MM-DD`); `t` gives
 * the words, `locale` formats a real date. */
export function dateLabel(
  date: string,
  display: DateDisplay,
  t: (key: string, values?: Record<string, string | number>) => string,
  locale: string,
): string {
  const [year, month, day] = date.split("-").map(Number);
  switch (display) {
    case "none":
      return "";
    case "real":
      return new Date(Date.UTC(year, month - 1, day)).toLocaleDateString(locale, {
        day: "numeric",
        month: "long",
        year: "numeric",
        timeZone: "UTC",
      });
    case "relative":
      return t("story.player.yearMonth", { year, month });
    case "seasons": {
      const season = ["winter", "spring", "summer", "autumn"][Math.floor((month % 12) / 3)];
      return t(SEASON[season], { year });
    }
  }
}

/** i18n keys written out in full, for the key check. */
const SEASON: Record<string, string> = {
  winter: "story.player.winter",
  spring: "story.player.spring",
  summer: "story.player.summer",
  autumn: "story.player.autumn",
};

/** The initials of a name, for an avatar without a picture. */
export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  return (words.length > 1 ? words[0][0] + words[words.length - 1][0] : (words[0] ?? "?").slice(0, 2)).toUpperCase();
}
