<script lang="ts">
  import Settings from "./Settings.svelte";
  import Library from "./Library.svelte";
  import RulesEditor from "./RulesEditor.svelte";
  import Profiles from "./Profiles.svelte";
  import Launch from "./Launch.svelte";
  import { nav } from "$lib/nav.svelte";

  const navItems: { id: string; label: string; lot: string }[] = [
    { id: "library", label: "Bibliothèque", lot: "L1" },
    { id: "profiles", label: "Profils", lot: "L3" },
    { id: "rules", label: "Règles de tags", lot: "L2" },
    { id: "race", label: "Lancer une course", lot: "L4" },
    { id: "settings", label: "Réglages", lot: "—" },
  ];
</script>

<div class="frame">
  <div class="topbar"></div>
  <div class="shell">
    <aside class="side">
      <div class="brand">
        <div class="logo"><span>PB</span></div>
        <div>
          <div class="brand-name">PIT BOX</div>
          <div class="brand-sub">AC MOD MANAGER</div>
        </div>
      </div>
      <nav>
        <div class="nav-sec">Navigation</div>
        {#each navItems as item}
          <button
            class="nav-item"
            class:active={nav.section === item.id}
            onclick={() => (nav.section = item.id)}
          >
            <span>{item.label}</span>
            {#if item.lot !== "—"}<span class="lot">{item.lot}</span>{/if}
          </button>
        {/each}
      </nav>
    </aside>

    <main class="content">
      {#if nav.section === "settings"}
        <Settings />
      {:else if nav.section === "library"}
        <Library />
      {:else if nav.section === "rules"}
        <RulesEditor />
      {:else if nav.section === "profiles"}
        <Profiles />
      {:else if nav.section === "race"}
        <Launch />
      {:else}
        <div class="placeholder">
          <div class="ph-tag">{navItems.find((n) => n.id === nav.section)?.lot}</div>
          <h2>{navItems.find((n) => n.id === nav.section)?.label}</h2>
          <p>Module à venir dans le lot correspondant.</p>
        </div>
      {/if}
    </main>
  </div>
</div>

<style>
  .frame {
    background: var(--panel);
    border: 1px solid var(--rosso);
    min-height: 100vh;
  }
  .topbar {
    background: var(--rosso);
    height: 3px;
  }
  .shell {
    display: grid;
    grid-template-columns: 180px 1fr;
    min-height: calc(100vh - 3px);
  }
  .side {
    background: var(--bg);
    border-right: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 14px;
    border-bottom: 1px solid var(--line);
  }
  .logo {
    width: 28px;
    height: 28px;
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
    font-size: 11px;
    font-weight: 700;
    font-style: italic;
  }
  .brand-name {
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 1.5px;
    font-style: italic;
    line-height: 1;
  }
  .brand-sub {
    color: var(--rosso);
    font-size: 7px;
    letter-spacing: 3px;
    margin-top: 3px;
  }
  .nav-sec {
    color: var(--faint);
    font-size: 8.5px;
    font-weight: 600;
    letter-spacing: 2px;
    padding: 14px 14px 6px;
    text-transform: uppercase;
  }
  .nav-item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 14px;
    background: transparent;
    border-left: 3px solid transparent;
    color: var(--muted);
    font-size: 11.5px;
    text-align: left;
    transition: background 0.12s;
  }
  .nav-item:hover {
    background: var(--raised);
  }
  .nav-item.active {
    background: var(--raised);
    border-left-color: var(--rosso);
    color: var(--txt);
  }
  .nav-item .lot {
    margin-left: auto;
    color: var(--faint);
    font-family: var(--mono);
    font-size: 10px;
  }
  .content {
    padding: 28px 32px;
    overflow: auto;
  }
  .placeholder {
    color: var(--muted);
    max-width: 460px;
  }
  .ph-tag {
    display: inline-block;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--rosso-bright);
    border: 1px solid var(--rosso-border);
    padding: 2px 7px;
    margin-bottom: 14px;
  }
  .placeholder h2 {
    font-size: 18px;
    color: var(--txt);
    font-weight: 600;
    margin-bottom: 10px;
  }
  .placeholder p {
    line-height: 1.6;
  }
</style>
