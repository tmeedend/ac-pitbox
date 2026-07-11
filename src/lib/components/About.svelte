<script lang="ts">
  // Écran « À propos » (§12, maquette pitbox-a-propos.html) : identité,
  // crédits des outils tiers (non-affiliation), soutien/communauté, licences
  // open source, bandeau légal. Statique hors version app + liste OSS.
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n/index.svelte";
  import licenseData from "$lib/generated/licenses.json";

  const AC_URL = "https://store.steampowered.com/app/244210/Assetto_Corsa/";
  const CM_URL = "https://acstuff.club/app/";
  const QUICKBMS_URL = "https://aluigi.altervista.org/quickbms.htm";
  const DONATE_URL = "https://paypal.me/ktulu77";
  const SOURCE_URL = "https://github.com/tmeedend/ac-pitbox";
  const OVERTAKE_URL = "https://www.overtake.gg/members/ktulu77.1266672/";
  const BUG_URL = "https://github.com/tmeedend/ac-pitbox/issues/new";
  const CHANGELOG_URL = "https://github.com/tmeedend/ac-pitbox/commits/main";

  let version = $state("0.1.0");
  onMount(async () => {
    try {
      version = await getVersion();
    } catch {
      /* hors contexte Tauri (ex. aperçu navigateur) : défaut affiché */
    }
  });

  let ossOpen = $state(false);
  const packages = licenseData.packages;

  function go(url: string) {
    openUrl(url).catch(() => {});
  }
</script>

<div class="about">
  <header class="hero">
    <div class="logo"><span>PB</span></div>
    <div>
      <div class="hn">PITBOX</div>
      <div class="hs">{t("about.tagline")}</div>
      <div class="hv mono">{t("about.buildLabel", { version, date: licenseData.generatedAt })}</div>
    </div>
  </header>
  <p class="philo">{t("about.philosophy")}</p>

  <section class="sect">
    <div class="st">{t("about.thirdPartyTitle")}</div>

    <div class="credit">
      <div class="credit-ico">🏎</div>
      <div class="credit-b">
        <div class="credit-name">Assetto Corsa <span class="tag">{t("about.acTag")}</span></div>
        <div class="credit-desc">{t("about.acDesc")}</div>
        <div class="credit-author mono">{t("about.acAuthor")}</div>
      </div>
      <button class="ext-link" type="button" onclick={() => go(AC_URL)} title={t("about.visitSite")}>↗</button>
    </div>

    <div class="credit">
      <div class="credit-ico">⚙</div>
      <div class="credit-b">
        <div class="credit-name">Content Manager <span class="tag required">{t("about.cmTag")}</span></div>
        <div class="credit-desc">{t("about.cmDesc")}</div>
        <div class="credit-author mono">{t("about.cmAuthor")}</div>
      </div>
      <button class="ext-link" type="button" onclick={() => go(CM_URL)} title={t("about.visitSite")}>↗</button>
    </div>

    <div class="credit">
      <div class="credit-ico">📦</div>
      <div class="credit-b">
        <div class="credit-name">QuickBMS <span class="tag">{t("about.qbTag")}</span></div>
        <div class="credit-desc">{t("about.qbDesc")}</div>
        <div class="credit-author mono">{t("about.qbAuthor")}</div>
      </div>
      <button class="ext-link" type="button" onclick={() => go(QUICKBMS_URL)} title={t("about.visitSite")}>↗</button>
    </div>
  </section>

  <section class="sect">
    <div class="st">{t("about.supportTitle")}</div>
    <div class="links">
      <button class="link-card" type="button" onclick={() => go(DONATE_URL)}>
        <div class="link-ico">💳</div>
        <div class="link-b"><div class="link-t">{t("about.donate")}</div><div class="link-s">{t("about.donateSub")}</div></div>
        <span class="go">↗</span>
      </button>
      <button class="link-card" type="button" onclick={() => go(SOURCE_URL)}>
        <div class="link-ico">🐙</div>
        <div class="link-b"><div class="link-t">{t("about.sourceCode")}</div><div class="link-s">{t("about.sourceCodeSub")}</div></div>
        <span class="go">↗</span>
      </button>
      <button class="link-card" type="button" onclick={() => go(OVERTAKE_URL)}>
        <div class="link-ico">👥</div>
        <div class="link-b"><div class="link-t">{t("about.overtake")}</div><div class="link-s">{t("about.overtakeSub")}</div></div>
        <span class="go">↗</span>
      </button>
      <button class="link-card" type="button" onclick={() => go(BUG_URL)}>
        <div class="link-ico">🐞</div>
        <div class="link-b"><div class="link-t">{t("about.reportBug")}</div><div class="link-s">{t("about.reportBugSub")}</div></div>
        <span class="go">↗</span>
      </button>
      <button class="link-card" type="button" onclick={() => go(CHANGELOG_URL)}>
        <div class="link-ico">📝</div>
        <div class="link-b"><div class="link-t">{t("about.changelog")}</div><div class="link-s">{t("about.changelogSub")}</div></div>
        <span class="go">↗</span>
      </button>
    </div>
  </section>

  <section class="sect">
    <div class="st">{t("about.ossTitle")}</div>
    <button class="oss-toggle" class:open={ossOpen} type="button" onclick={() => (ossOpen = !ossOpen)}>
      {t("about.ossToggle", { count: packages.length })}
      <span class="chev">▸</span>
    </button>
    {#if ossOpen}
      <div class="oss-body">
        {#each packages as p (p.ecosystem + p.name)}
          <div class="oss-row">
            <span>{p.name} <span class="oss-ver mono">{p.version}</span></span>
            <span>{p.license}</span>
          </div>
        {/each}
      </div>
      <div class="oss-note">{t("about.ossGenerated", { date: licenseData.generatedAt })}</div>
    {/if}
  </section>

  <div class="legal">{t("about.legal")}</div>

  <div class="foot">
    <span>{t("about.license")}</span>
    <span>{t("about.madeWith")}</span>
  </div>
</div>

<style>
  .about {
    max-width: 720px;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 16px;
    padding-bottom: 20px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 18px;
  }
  .logo {
    width: 52px;
    height: 52px;
    background: var(--rosso);
    display: flex;
    align-items: center;
    justify-content: center;
    transform: skewX(-8deg);
    flex: none;
  }
  .logo span {
    transform: skewX(8deg);
    color: #fff;
    font-size: 18px;
    font-weight: 700;
    font-style: italic;
  }
  .hn {
    font-size: 22px;
    font-weight: 700;
    letter-spacing: 2px;
    font-style: italic;
  }
  .hs {
    color: var(--rosso);
    font-size: 9px;
    letter-spacing: 3px;
    margin-top: 3px;
  }
  .hv {
    color: var(--muted);
    font-size: 9.5px;
    margin-top: 6px;
  }
  .philo {
    color: var(--txt2);
    font-size: 12px;
    line-height: 1.6;
    max-width: 560px;
    margin-bottom: 26px;
  }

  .sect {
    margin-bottom: 26px;
  }
  .st {
    color: var(--rosso);
    font-size: 9px;
    letter-spacing: 2.5px;
    font-family: var(--mono);
    text-transform: uppercase;
    margin-bottom: 12px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .st::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--rosso-border);
  }

  .credit {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
  }
  .credit:last-child {
    border-bottom: none;
  }
  .credit-ico {
    width: 34px;
    height: 34px;
    border: 1px solid var(--line);
    background: var(--panel2);
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    font-size: 16px;
  }
  .credit-b {
    flex: 1;
    min-width: 0;
  }
  .credit-name {
    font-size: 12px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .tag {
    font-family: var(--mono);
    font-size: 7.5px;
    letter-spacing: 1px;
    color: var(--muted);
    border: 1px solid var(--line);
    padding: 1px 6px;
    text-transform: uppercase;
  }
  .tag.required {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .credit-desc {
    color: var(--muted);
    font-size: 10.5px;
    margin-top: 3px;
    line-height: 1.5;
  }
  .credit-author {
    color: var(--faint);
    font-size: 9px;
    margin-top: 4px;
  }
  .ext-link {
    background: transparent;
    color: var(--muted2);
    font-size: 14px;
    flex: none;
    padding: 4px;
  }
  .ext-link:hover {
    color: var(--rosso-bright);
  }

  .links {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
  }
  .link-card {
    background: var(--panel2);
    padding: 14px 16px;
    display: flex;
    align-items: center;
    gap: 12px;
    text-align: left;
  }
  .link-card:hover {
    background: var(--raised);
  }
  .link-ico {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    font-size: 15px;
  }
  .link-b {
    min-width: 0;
  }
  .link-t {
    font-size: 11.5px;
    font-weight: 600;
  }
  .link-s {
    color: var(--muted);
    font-size: 9px;
    margin-top: 2px;
  }
  .link-card .go {
    margin-left: auto;
    color: var(--faint);
    font-size: 13px;
    flex: none;
  }

  .oss-toggle {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 14px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 10.5px;
    text-align: left;
  }
  .oss-toggle .chev {
    margin-left: auto;
    color: var(--muted);
    transition: transform 0.15s;
  }
  .oss-toggle.open .chev {
    transform: rotate(90deg);
  }
  .oss-body {
    padding: 12px 14px;
    background: var(--bg);
    border: 1px solid var(--line);
    border-top: none;
    max-height: 220px;
    overflow-y: auto;
  }
  .oss-row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 5px 0;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 9.5px;
  }
  .oss-row:last-child {
    border-bottom: none;
  }
  .oss-row span:first-child {
    color: var(--txt2);
  }
  .oss-row span:last-child {
    color: var(--faint);
    flex: none;
  }
  .oss-ver {
    color: var(--faint);
  }
  .oss-note {
    color: var(--faint);
    font-size: 9px;
    margin-top: 6px;
  }

  .legal {
    color: var(--faint);
    font-size: 9.5px;
    line-height: 1.8;
    padding: 14px 16px;
    background: var(--panel2);
    border-left: 2px solid var(--rosso-border);
  }

  .foot {
    margin-top: 24px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--faint);
    font-size: 9px;
    font-family: var(--mono);
  }
</style>
