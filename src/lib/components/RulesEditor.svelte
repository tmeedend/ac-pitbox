<script lang="ts">
  import { onMount } from "svelte";
  import { getRules, saveRules, rulesImpact, type Rules } from "$lib/rules";
  import { t } from "$lib/i18n/index.svelte";

  let rules = $state<Rules | null>(null);
  let tab = $state<"car" | "track">("car");
  let impact = $state<number | null>(null);
  let saving = $state(false);
  let savedMsg = $state("");
  let removeInput = $state("");

  // Le map pays s'édite via une liste de paires synchronisée vers l'objet.
  let countryPairs = $state<{ tag: string; country: string }[]>([]);

  onMount(async () => {
    rules = await getRules();
    countryPairs = Object.entries(rules.car.extraction_country.map).map(
      ([tag, country]) => ({ tag, country }),
    );
  });

  // Reconstruit le map pays depuis les paires éditées.
  $effect(() => {
    if (!rules) return;
    const map: Record<string, string> = {};
    for (const p of countryPairs) {
      const k = p.tag.trim().toLowerCase();
      if (k) map[k] = p.country.trim();
    }
    rules.car.extraction_country.map = map;
  });

  // Aperçu d'impact à la volée (anti-rebond) — « N mods affectés » (§5.4).
  $effect(() => {
    if (!rules) return;
    JSON.stringify(rules); // dépendance réactive
    savedMsg = "";
    const snapshot = rules;
    const timer = setTimeout(async () => {
      impact = await rulesImpact(snapshot);
    }, 350);
    return () => clearTimeout(timer);
  });

  const parseList = (s: string) =>
    s.split(",").map((x) => x.trim().toLowerCase()).filter(Boolean);
  const joinList = (a: string[]) => a.join(", ");

  function push<T>(list: T[], item: T) {
    list.unshift(item);
  }
  function removeAt<T>(list: T[], i: number) {
    list.splice(i, 1);
  }

  function addRemove(list: string[], value: string) {
    const v = value.trim().toLowerCase();
    if (v && !list.includes(v)) list.unshift(v);
  }

  async function save() {
    if (!rules) return;
    saving = true;
    try {
      const n = await saveRules(rules);
      savedMsg = t("rules.savedMsg", { count: n });
      impact = 0;
    } finally {
      saving = false;
    }
  }
</script>

<div class="rules">
  <header class="r-header">
    <h2>{t("rules.title")}</h2>
    <p class="sub">{t("rules.subtitle")}</p>
  </header>

  {#if !rules}
    <div class="empty">{t("rules.loading")}</div>
  {:else}
    {@const fam = tab === "car" ? rules.car : rules.track}
    <div class="tabs">
      <button class:on={tab === "car"} onclick={() => (tab = "car")}>{t("rules.tabCars")}</button>
      <button class:on={tab === "track"} onclick={() => (tab = "track")}>{t("rules.tabTracks")}</button>
    </div>

    <!-- FUSION / DÉDUCTION -->
    <section>
      <div class="s-head">
        <h3>{t("rules.tagMerge")} <span class="cnt">{fam.tag_merge.length}</span></h3>
        <button class="btn" type="button" onclick={() => push(fam.tag_merge, { from: [], to: [] })}>+ {t("rules.addRule")}</button>
      </div>
      <p class="hint">{t("rules.tagMergeHint")} <span class="mono">#</span> {t("rules.tagMergeHintCat")}</p>
      <div class="rows">
        {#each fam.tag_merge as rule, i}
          <div class="row">
            <input class="input mono" value={joinList(rule.from)} oninput={(e) => (rule.from = parseList(e.currentTarget.value))} placeholder="hothatch, hot hatchback" />
            <span class="arrow">→</span>
            <input class="input mono" value={joinList(rule.to)} oninput={(e) => (rule.to = parseList(e.currentTarget.value))} placeholder="hatchback" />
            <button class="btn-ghost del" type="button" onclick={() => removeAt(fam.tag_merge, i)} title={t("common.delete")}>✕</button>
          </div>
        {/each}
      </div>
    </section>

    <!-- SUPPRESSION -->
    <section>
      <div class="s-head">
        <h3>{t("rules.removalTitle")} <span class="cnt">{fam.remove.length}</span></h3>
      </div>
      <input class="input add-input" placeholder={t("rules.addRemovePlaceholder")} bind:value={removeInput}
        onkeydown={(e) => { if (e.key === "Enter") { addRemove(fam.remove, removeInput); removeInput = ""; } }} />
      <div class="chips">
        {#each fam.remove as tag, i}
          <span class="chip">{tag}<button class="x" type="button" onclick={() => removeAt(fam.remove, i)}>×</button></span>
        {/each}
      </div>
    </section>

    {#if tab === "car"}
      {@const car = rules.car}
      <!-- CORRECTION DE MARQUE -->
      <section>
        <div class="s-head">
          <h3>{t("rules.brandFixTitle")} <span class="cnt">{car.brand_fix.length}</span></h3>
          <button class="btn" type="button" onclick={() => push(car.brand_fix, { name_contains: "", set_brand: "" })}>+ {t("rules.addRule")}</button>
        </div>
        <p class="hint">{t("rules.brandFixHint")}</p>
        <div class="rows">
          {#each car.brand_fix as rule, i}
            <div class="row">
              <input class="input" value={rule.name_contains} oninput={(e) => (rule.name_contains = e.currentTarget.value)} placeholder="bayro" />
              <span class="arrow">→</span>
              <input class="input" value={rule.set_brand} oninput={(e) => (rule.set_brand = e.currentTarget.value)} placeholder="BMW" />
              <button class="btn-ghost del" type="button" onclick={() => removeAt(car.brand_fix, i)} title={t("common.delete")}>✕</button>
            </div>
          {/each}
        </div>
      </section>

      <!-- NOM → TAG -->
      <section>
        <div class="s-head">
          <h3>{t("rules.nameToTagTitle")} <span class="cnt">{car.name_to_tag.length}</span></h3>
          <button class="btn" type="button" onclick={() => push(car.name_to_tag, { name_contains: "", add: [] })}>+ {t("rules.addRule")}</button>
        </div>
        <p class="hint">{t("rules.nameToTagHint")}</p>
        <div class="rows">
          {#each car.name_to_tag as rule, i}
            <div class="row">
              <input class="input" value={rule.name_contains} oninput={(e) => (rule.name_contains = e.currentTarget.value)} placeholder="police" />
              <span class="arrow">→</span>
              <input class="input mono" value={joinList(rule.add)} oninput={(e) => (rule.add = parseList(e.currentTarget.value))} placeholder="police" />
              <button class="btn-ghost del" type="button" onclick={() => removeAt(car.name_to_tag, i)} title={t("common.delete")}>✕</button>
            </div>
          {/each}
        </div>
      </section>

      <!-- NORMALISATION DE CLASSE -->
      <section>
        <div class="s-head">
          <h3>{t("rules.classFixTitle")} <span class="cnt">{car.class_fix.length}</span></h3>
          <button class="btn" type="button" onclick={() => push(car.class_fix, { from: [], set_class: null, add: [] })}>+ {t("rules.addRule")}</button>
        </div>
        <p class="hint">{t("rules.classFixHint")}</p>
        <div class="rows">
          {#each car.class_fix as rule, i}
            <div class="row class-row">
              <input class="input mono" value={joinList(rule.from)} oninput={(e) => (rule.from = parseList(e.currentTarget.value))} placeholder="rally, hillclimb" />
              <select class="input sel" value={rule.set_class ?? ""} onchange={(e) => (rule.set_class = e.currentTarget.value || null)}>
                <option value="">{t("rules.noneOption")}</option>
                <option value="race">race</option>
                <option value="street">street</option>
              </select>
              <input class="input mono" value={joinList(rule.add)} oninput={(e) => (rule.add = parseList(e.currentTarget.value))} placeholder="#rally" />
              <button class="btn-ghost del" type="button" onclick={() => removeAt(car.class_fix, i)} title={t("common.delete")}>✕</button>
            </div>
          {/each}
        </div>
      </section>

      <!-- EXTRACTION SPECS -->
      <section>
        <div class="s-head"><h3>{t("rules.specsExtractionTitle")}</h3></div>
        <p class="hint">{t("rules.specsExtractionHint")}</p>
        {#each [["drivetrain", "rules.fieldDrivetrain"], ["aspiration", "rules.fieldAspiration"], ["engine_config", "rules.fieldEngineConfig"], ["engine_pos", "rules.fieldEnginePos"], ["gearbox", "rules.fieldGearbox"]] as [field, labelKey]}
          {@const list = car.extraction_specs[field as keyof typeof car.extraction_specs]}
          <div class="sub-fam">
            <div class="sub-head">
              <span class="sub-label">{t(labelKey)} <span class="cnt">{list.length}</span></span>
              <button class="btn-ghost mini" type="button" onclick={() => push(list, { from: [], set: "" })}>+ </button>
            </div>
            {#each list as rule, i}
              <div class="row">
                <input class="input mono" value={joinList(rule.from)} oninput={(e) => (rule.from = parseList(e.currentTarget.value))} placeholder="rwd, propulsion" />
                <span class="arrow">→</span>
                <input class="input mono" value={rule.set} oninput={(e) => (rule.set = e.currentTarget.value)} placeholder="RWD" />
                <button class="btn-ghost del" type="button" onclick={() => removeAt(list, i)} title={t("common.delete")}>✕</button>
              </div>
            {/each}
          </div>
        {/each}
      </section>

      <!-- EXTRACTION PAYS -->
      <section>
        <div class="s-head">
          <h3>{t("rules.countryExtractionTitle")} <span class="cnt">{countryPairs.length}</span></h3>
          <button class="btn" type="button" onclick={() => push(countryPairs, { tag: "", country: "" })}>+ {t("columns.country")}</button>
        </div>
        <p class="hint">{t("rules.countryExtractionHint")}</p>
        <div class="rows">
          {#each countryPairs as pair, i}
            <div class="row">
              <input class="input mono" bind:value={pair.tag} placeholder="germany" />
              <span class="arrow">→</span>
              <input class="input" bind:value={pair.country} placeholder="Germany" />
              <button class="btn-ghost del" type="button" onclick={() => removeAt(countryPairs, i)} title={t("common.delete")}>✕</button>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <footer class="r-footer">
      <div class="impact">
        {#if impact !== null}
          <span class="i-num">{impact}</span> {t("rules.impactText")}
        {:else}
          <span class="muted">{t("rules.calculatingImpact")}</span>
        {/if}
      </div>
      <div class="f-actions">
        {#if savedMsg}<span class="pill pill-ok">{savedMsg}</span>{/if}
        <button class="btn btn-primary" type="button" onclick={save} disabled={saving}>
          {saving ? t("rules.applying") : t("rules.saveAndReapply")}
        </button>
      </div>
    </footer>
  {/if}
</div>

<style>
  .rules {
    max-width: 760px;
    padding-bottom: 80px;
  }
  .r-header {
    margin-bottom: 16px;
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
  }
  .sub {
    color: var(--muted);
    margin-top: 6px;
    line-height: 1.5;
  }
  .tabs {
    display: flex;
    border: 1px solid var(--line);
    width: fit-content;
    margin-bottom: 20px;
  }
  .tabs button {
    background: var(--panel2);
    color: var(--muted);
    padding: 8px 18px;
    font-size: 12px;
    border-right: 1px solid var(--line);
  }
  .tabs button:last-child {
    border-right: none;
  }
  .tabs button.on {
    background: var(--raised);
    color: var(--txt);
  }
  section {
    margin-bottom: 26px;
  }
  .s-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--txt2);
  }
  .cnt {
    color: var(--faint);
    font-family: var(--mono);
    margin-left: 6px;
  }
  .hint {
    color: var(--muted);
    font-size: 11.5px;
    margin-bottom: 10px;
    line-height: 1.5;
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .row .input {
    flex: 1;
    min-width: 0;
  }
  .class-row .sel {
    flex: 0 0 110px;
  }
  .arrow {
    color: var(--rosso-bright);
    flex: none;
  }
  .del {
    flex: none;
    padding: 4px 8px;
  }
  .add-input {
    margin-bottom: 10px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    padding: 2px 4px 2px 8px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .chip .x {
    background: transparent;
    color: var(--muted2);
    font-size: 13px;
    line-height: 1;
    padding: 0 2px;
  }
  .chip .x:hover {
    color: var(--rosso-bright);
  }
  .sub-fam {
    margin: 10px 0;
    padding-left: 10px;
    border-left: 2px solid var(--line);
  }
  .sub-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 5px;
  }
  .sub-label {
    font-size: 11px;
    color: var(--muted);
  }
  .mini {
    padding: 2px 8px;
    font-size: 13px;
  }
  .r-footer {
    position: fixed;
    bottom: 0;
    left: 180px;
    right: 0;
    background: var(--panel);
    border-top: 1px solid var(--rosso-border);
    padding: 12px 32px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    z-index: 20;
  }
  .impact {
    font-size: 13px;
    color: var(--txt2);
  }
  .i-num {
    font-family: var(--mono);
    color: var(--rosso-bright);
    font-size: 16px;
    font-weight: 600;
  }
  .muted {
    color: var(--muted);
  }
  .f-actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .empty {
    color: var(--muted);
    padding: 40px 0;
  }
</style>
