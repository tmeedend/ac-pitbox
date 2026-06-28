<script lang="ts">
  import { onMount } from "svelte";
  import { getRules, saveRules, rulesImpact, type Rules } from "$lib/rules";

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
    const t = setTimeout(async () => {
      impact = await rulesImpact(snapshot);
    }, 350);
    return () => clearTimeout(t);
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
      savedMsg = `Enregistré — ${n} mod(s) retraité(s)`;
      impact = 0;
    } finally {
      saving = false;
    }
  }
</script>

<div class="rules">
  <header class="r-header">
    <h2>Règles de tags</h2>
    <p class="sub">Ontologie éditable (§5.4) — appliquée de façon non destructive. Le fichier du mod n'est jamais modifié.</p>
  </header>

  {#if !rules}
    <div class="empty">Chargement des règles…</div>
  {:else}
    {@const fam = tab === "car" ? rules.car : rules.track}
    <div class="tabs">
      <button class:on={tab === "car"} onclick={() => (tab = "car")}>Voitures</button>
      <button class:on={tab === "track"} onclick={() => (tab = "track")}>Circuits</button>
    </div>

    <!-- FUSION / DÉDUCTION -->
    <section>
      <div class="s-head">
        <h3>Fusion / déduction <span class="cnt">{fam.tag_merge.length}</span></h3>
        <button class="btn" type="button" onclick={() => push(fam.tag_merge, { from: [], to: [] })}>+ Règle</button>
      </div>
      <p class="hint">Synonymes → tag(s) canonique(s). Valeurs séparées par des virgules. Un tag <span class="mono">#</span> marque la catégorie.</p>
      <div class="rows">
        {#each fam.tag_merge as rule, i}
          <div class="row">
            <input class="input mono" value={joinList(rule.from)} oninput={(e) => (rule.from = parseList(e.currentTarget.value))} placeholder="hothatch, hot hatchback" />
            <span class="arrow">→</span>
            <input class="input mono" value={joinList(rule.to)} oninput={(e) => (rule.to = parseList(e.currentTarget.value))} placeholder="hatchback" />
            <button class="btn-ghost del" type="button" onclick={() => removeAt(fam.tag_merge, i)} title="Supprimer">✕</button>
          </div>
        {/each}
      </div>
    </section>

    <!-- SUPPRESSION -->
    <section>
      <div class="s-head">
        <h3>Suppression (bruit) <span class="cnt">{fam.remove.length}</span></h3>
      </div>
      <input class="input add-input" placeholder="ajouter un tag à supprimer (Entrée)…" bind:value={removeInput}
        onkeydown={(e) => { if (e.key === "Enter") { addRemove(fam.remove, removeInput); removeInput = ""; } }} />
      <div class="chips">
        {#each fam.remove as tag, i}
          <span class="chip">{tag}<button class="x" type="button" onclick={() => removeAt(fam.remove, i)}>×</button></span>
        {/each}
      </div>
    </section>

    <!-- CATÉGORIES MO2 -->
    <section>
      <div class="s-head">
        <h3>Catégories MO2 → tags <span class="cnt">{fam.mo2_category_map.length}</span></h3>
        <button class="btn" type="button" onclick={() => push(fam.mo2_category_map, { from: "", add: [] })}>+ Règle</button>
      </div>
      <p class="hint">Migration d'un catalogue Mod Organizer 2 : nom de catégorie → tags ajoutés.</p>
      <div class="rows">
        {#each fam.mo2_category_map as rule, i}
          <div class="row">
            <input class="input" value={rule.from} oninput={(e) => (rule.from = e.currentTarget.value)} placeholder="Car - GT3" />
            <span class="arrow">→</span>
            <input class="input mono" value={joinList(rule.add)} oninput={(e) => (rule.add = parseList(e.currentTarget.value))} placeholder="#gt3" />
            <button class="btn-ghost del" type="button" onclick={() => removeAt(fam.mo2_category_map, i)} title="Supprimer">✕</button>
          </div>
        {/each}
      </div>
    </section>

    {#if tab === "car"}
      {@const car = rules.car}
      <!-- CORRECTION DE MARQUE -->
      <section>
        <div class="s-head">
          <h3>Correction de marque <span class="cnt">{car.brand_fix.length}</span></h3>
          <button class="btn" type="button" onclick={() => push(car.brand_fix, { name_contains: "", set_brand: "" })}>+ Règle</button>
        </div>
        <p class="hint">Si le nom contient X → forcer la marque.</p>
        <div class="rows">
          {#each car.brand_fix as rule, i}
            <div class="row">
              <input class="input" value={rule.name_contains} oninput={(e) => (rule.name_contains = e.currentTarget.value)} placeholder="bayro" />
              <span class="arrow">→</span>
              <input class="input" value={rule.set_brand} oninput={(e) => (rule.set_brand = e.currentTarget.value)} placeholder="BMW" />
              <button class="btn-ghost del" type="button" onclick={() => removeAt(car.brand_fix, i)} title="Supprimer">✕</button>
            </div>
          {/each}
        </div>
      </section>

      <!-- NOM → TAG -->
      <section>
        <div class="s-head">
          <h3>Nom → tag <span class="cnt">{car.name_to_tag.length}</span></h3>
          <button class="btn" type="button" onclick={() => push(car.name_to_tag, { name_contains: "", add: [] })}>+ Règle</button>
        </div>
        <p class="hint">Si le nom contient X → ajouter tag(s).</p>
        <div class="rows">
          {#each car.name_to_tag as rule, i}
            <div class="row">
              <input class="input" value={rule.name_contains} oninput={(e) => (rule.name_contains = e.currentTarget.value)} placeholder="police" />
              <span class="arrow">→</span>
              <input class="input mono" value={joinList(rule.add)} oninput={(e) => (rule.add = parseList(e.currentTarget.value))} placeholder="police" />
              <button class="btn-ghost del" type="button" onclick={() => removeAt(car.name_to_tag, i)} title="Supprimer">✕</button>
            </div>
          {/each}
        </div>
      </section>

      <!-- NORMALISATION DE CLASSE -->
      <section>
        <div class="s-head">
          <h3>Normalisation de classe <span class="cnt">{car.class_fix.length}</span></h3>
          <button class="btn" type="button" onclick={() => push(car.class_fix, { from: [], set_class: null, add: [] })}>+ Règle</button>
        </div>
        <p class="hint">Valeur de <span class="mono">class</span> du ui → classe race/street + tags déduits.</p>
        <div class="rows">
          {#each car.class_fix as rule, i}
            <div class="row class-row">
              <input class="input mono" value={joinList(rule.from)} oninput={(e) => (rule.from = parseList(e.currentTarget.value))} placeholder="rally, hillclimb" />
              <select class="input sel" value={rule.set_class ?? ""} onchange={(e) => (rule.set_class = e.currentTarget.value || null)}>
                <option value="">— aucune —</option>
                <option value="race">race</option>
                <option value="street">street</option>
              </select>
              <input class="input mono" value={joinList(rule.add)} oninput={(e) => (rule.add = parseList(e.currentTarget.value))} placeholder="#rally" />
              <button class="btn-ghost del" type="button" onclick={() => removeAt(car.class_fix, i)} title="Supprimer">✕</button>
            </div>
          {/each}
        </div>
      </section>

      <!-- EXTRACTION SPECS -->
      <section>
        <div class="s-head"><h3>Extraction fiche technique</h3></div>
        <p class="hint">Tag technique → champ structuré. Le tag est retiré du vocabulaire.</p>
        {#each [["drivetrain", "Transmission"], ["aspiration", "Aspiration"], ["engine_config", "Moteur"], ["engine_pos", "Position"], ["gearbox", "Boîte"]] as [field, label]}
          {@const list = car.extraction_specs[field as keyof typeof car.extraction_specs]}
          <div class="sub-fam">
            <div class="sub-head">
              <span class="sub-label">{label} <span class="cnt">{list.length}</span></span>
              <button class="btn-ghost mini" type="button" onclick={() => push(list, { from: [], set: "" })}>+ </button>
            </div>
            {#each list as rule, i}
              <div class="row">
                <input class="input mono" value={joinList(rule.from)} oninput={(e) => (rule.from = parseList(e.currentTarget.value))} placeholder="rwd, propulsion" />
                <span class="arrow">→</span>
                <input class="input mono" value={rule.set} oninput={(e) => (rule.set = e.currentTarget.value)} placeholder="RWD" />
                <button class="btn-ghost del" type="button" onclick={() => removeAt(list, i)} title="Supprimer">✕</button>
              </div>
            {/each}
          </div>
        {/each}
      </section>

      <!-- EXTRACTION PAYS -->
      <section>
        <div class="s-head">
          <h3>Extraction pays <span class="cnt">{countryPairs.length}</span></h3>
          <button class="btn" type="button" onclick={() => push(countryPairs, { tag: "", country: "" })}>+ Pays</button>
        </div>
        <p class="hint">Tag pays → champ <span class="mono">country</span> (si vide). Le tag est retiré du vocabulaire.</p>
        <div class="rows">
          {#each countryPairs as pair, i}
            <div class="row">
              <input class="input mono" bind:value={pair.tag} placeholder="germany" />
              <span class="arrow">→</span>
              <input class="input" bind:value={pair.country} placeholder="Germany" />
              <button class="btn-ghost del" type="button" onclick={() => removeAt(countryPairs, i)} title="Supprimer">✕</button>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <footer class="r-footer">
      <div class="impact">
        {#if impact !== null}
          <span class="i-num">{impact}</span> mod(s) seraient affecté(s) par ces règles
        {:else}
          <span class="muted">calcul de l'impact…</span>
        {/if}
      </div>
      <div class="f-actions">
        {#if savedMsg}<span class="pill pill-ok">{savedMsg}</span>{/if}
        <button class="btn btn-primary" type="button" onclick={save} disabled={saving}>
          {saving ? "Application…" : "Enregistrer & réappliquer"}
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
