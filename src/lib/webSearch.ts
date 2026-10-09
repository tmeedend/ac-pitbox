// The one web search Pit Box opens when it has no address for something:
// DuckDuckGo, no site in particular unless one is known (the user's choice,
// 2026-10-04). Pure, for Vitest.

/** Where `query` is looked for, restricted to `site` when the archive's origin
 * is known (ESPACE§8.2): `https://www.overtake.gg/` → `site:overtake.gg`.
 * Without quotes: an exact-phrase search on a file name found nothing on the
 * real case, the same words without them found the page. */
export function webSearchUrl(query: string, site: string | null = null): string {
  const q = site ? `site:${siteHost(site)} ${query}` : query;
  return `https://duckduckgo.com/?q=${encodeURIComponent(q)}`;
}

/** The host of a site address, without its `www.` — what a person reads, and
 * what a `site:` search wants. An address that does not parse is returned as
 * it came. */
export function siteHost(site: string): string {
  try {
    return new URL(site).hostname.replace(/^www\./, "");
  } catch {
    return site;
  }
}
