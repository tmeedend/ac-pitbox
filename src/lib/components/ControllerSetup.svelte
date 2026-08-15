<script lang="ts">
  // Panneau « périphérique de contrôle » (§7.4) : la question « lequel pilote
  // l'interface ? », puis la calibration guidée d'un volant inconnu.
  //
  // Ouvert AU CLIC (bandeau ou Réglages), jamais tout seul : à ce moment
  // l'utilisateur est disponible, un dialogue est légitime. Il suspend la
  // navigation manette globale tant qu'il est ouvert (`nav.inputCapture`) —
  // sa zone d'essai consomme les entrées du périphérique calibré, sinon
  // « haut » validerait un bouton derrière le panneau. Tout y est opérable à
  // la souris et au clavier : c'est un panneau au sujet d'un périphérique qui
  // ne marche peut-être pas.
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { nav } from "$lib/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import {
    controllers,
    deviceRecords,
    answerDevices,
    saveDeviceProfile,
    type LiveDevice,
  } from "$lib/gamepadDevices.svelte";
  import { resolveProfile, type ProfileSource } from "$lib/gamepadNav";
  import {
    EMPTY_REST,
    axisMode,
    bindingActive,
    bindingsEqual,
    deviceKey,
    describeBinding,
    isIntermediate,
    measureRest,
    profileReport,
    sampleEqual,
    strongestChange,
    type Binding,
    type Direction,
    type NavProfile,
    type RestSnapshot,
  } from "$lib/gamepadProfile";

  interface Props {
    onclose: () => void;
  }
  let { onclose }: Props = $props();

  const ISSUE_URL = "https://github.com/tmeedend/ac-pitbox/issues/new";

  type StepId = Direction | "confirm" | "back";
  const STEPS: StepId[] = ["up", "down", "left", "right", "confirm", "back"];

  /** Repos mesuré sur 2 s au début de la calibration — assez long pour que
   * l'utilisateur ait vraiment lâché le volant, assez court pour ne pas
   * donner l'impression d'un écran figé. */
  const REST_MS = 2000;
  /** Stabilité exigée avant d'enregistrer un geste : sans elle, un rebond de
   * contact ou une valeur intermédiaire d'axe analogique est capté à la place
   * du geste voulu. */
  const CAPTURE_STABLE_MS = 150;
  /** Beaucoup de volants n'ont pas de croix : « Passer » est un chemin
   * normal, pas un échec. */
  const CAPTURE_TIMEOUT_MS = 10000;

  // --- Liste des périphériques ---------------------------------------------

  // Dédoublonné par clé : un volant peut présenter la même clé sur deux
  // entrées `Gamepad` (base + boîtier de boutons).
  const listed = $derived.by(() => {
    const seen = new Set<string>();
    const out: LiveDevice[] = [];
    for (const d of controllers.live) {
      if (seen.has(d.key)) continue;
      seen.add(d.key);
      out.push(d);
    }
    return out;
  });

  let selectedKey = $state<string | null>(null);

  function sourceOf(d: LiveDevice): ProfileSource {
    return resolveProfile({ id: d.id, mapping: d.mapping }, deviceRecords()[d.key]).source;
  }

  const badgeKeys: Record<ProfileSource, string> = {
    calibrated: "controller.badge.calibrated",
    override: "controller.badge.known",
    standard: "controller.badge.standard",
    none: "controller.badge.unknown",
  };

  const anyUnknown = $derived(listed.some((d) => sourceOf(d) === "none"));

  function identity(d: LiveDevice): string {
    return `${d.key} · ${d.axes.length} ${t("controller.panel.axes")} · ${d.buttonCount} ${t("controller.panel.buttons")}`;
  }

  function useSelected() {
    answerDevices(listed, selectedKey);
    onclose();
  }

  /** « Aucun pour l'instant » clôt le sujet ; « Fermer » ne répond rien et le
   * bandeau reviendra. Les confondre donne soit une sollicitation qui
   * harcèle, soit une décision prise par accident. */
  function answerNone() {
    answerDevices(listed, null);
    onclose();
  }

  // --- Calibration ---------------------------------------------------------

  let calKey = $state<string | null>(null);
  let calDevice = $state<LiveDevice | null>(null);
  let phase = $state<"rest" | "neutral" | "capture" | "timeout" | "review">("rest");
  let stepIdx = $state(0);
  let captured = $state<Partial<Record<StepId, Binding>>>({});
  let rest = $state<RestSnapshot | null>(null);
  let restProgress = $state(0);
  let duplicateOf = $state<StepId | null>(null);
  let testFocus = $state(0);
  let testFlash = $state(false);
  let copied = $state(false);
  let version = $state("0.1.0");

  // État de la boucle rAF : volontairement hors `$state` — relu et réécrit à
  // chaque image, il ne doit déclencher aucun rendu.
  let sample: RestSnapshot | null = null;
  let stableSince = 0;
  let candidate: Binding | null = null;
  let candidateSince = 0;
  let stepStart = 0;
  let sawIntermediate = false;
  let lastTestEdges: Record<string, boolean> = {};

  const currentStep = $derived(STEPS[stepIdx]);

  const builtProfile = $derived<NavProfile>({
    dirs: { up: captured.up, down: captured.down, left: captured.left, right: captured.right },
    confirm: captured.confirm,
    back: captured.back,
    rest: rest ?? EMPTY_REST,
  });

  function stepLabel(s: StepId): string {
    return t(`controller.calib.dir.${s}`);
  }

  function startCalibration(d: LiveDevice) {
    calKey = d.key;
    calDevice = d;
    phase = "rest";
    stepIdx = 0;
    captured = {};
    rest = null;
    restProgress = 0;
    duplicateOf = null;
    copied = false;
    sample = null;
    stableSince = 0;
  }

  function cancelCalibration() {
    calKey = null;
    calDevice = null;
  }

  function beginStep(i: number) {
    stepIdx = i;
    phase = "neutral";
    candidate = null;
    sawIntermediate = false;
  }

  function restartCalibration() {
    if (calDevice) startCalibration(calDevice);
  }

  function skipStep() {
    duplicateOf = null;
    if (stepIdx + 1 >= STEPS.length) enterReview();
    else beginStep(stepIdx + 1);
  }

  function retryStep() {
    duplicateOf = null;
    beginStep(stepIdx);
  }

  function enterReview() {
    phase = "review";
    testFocus = 0;
    lastTestEdges = {};
  }

  function livePad(key: string): Gamepad | null {
    for (const gp of navigator.getGamepads?.() ?? []) {
      // `getGamepads()` renvoie un instantané troué et vieillissant : relu à
      // chaque image, jamais conservé d'une frame à l'autre.
      if (gp?.connected && deviceKey(gp.id) === key) return gp;
    }
    return null;
  }

  function commit(binding: Binding) {
    const dup = STEPS.find((s) => s !== currentStep && captured[s] && bindingsEqual(captured[s]!, binding));
    if (dup) {
      // Deux directions sur la même liaison est pire qu'un profil incomplet :
      // on refait l'étape plutôt que d'enregistrer un profil ambigu.
      duplicateOf = dup;
      phase = "neutral";
      candidate = null;
      return;
    }
    duplicateOf = null;
    const final: Binding =
      binding.kind === "axis" ? { ...binding, mode: axisMode(sawIntermediate, binding.value) } : binding;
    captured = { ...captured, [currentStep]: final };
    if (stepIdx + 1 >= STEPS.length) enterReview();
    else beginStep(stepIdx + 1);
  }

  function tickCalibration(gp: Gamepad, now: number) {
    if (phase === "rest") {
      const s = measureRest(gp);
      if (!sample || !sampleEqual(sample, s)) {
        sample = s;
        stableSince = now;
      }
      restProgress = Math.min(1, (now - stableSince) / REST_MS);
      if (now - stableSince >= REST_MS) {
        rest = s;
        beginStep(0);
      }
      return;
    }
    if (!rest) return;

    if (phase === "neutral") {
      // Retour au repos exigé avant l'étape suivante, sinon le même maintien
      // est capté deux fois.
      if (!strongestChange(gp, rest)) {
        phase = "capture";
        stepStart = now;
        candidate = null;
        sawIntermediate = false;
      }
      return;
    }

    if (phase === "capture") {
      const change = strongestChange(gp, rest);
      if (!change) {
        candidate = null;
      } else {
        if (!candidate || !bindingsEqual(candidate, change.binding)) {
          candidate = change.binding;
          candidateSince = now;
        }
        if (change.binding.kind === "axis") {
          // Hat ou stick ? La différence se lit *pendant* la capture : un
          // stick traverse des valeurs intermédiaires, un hat saute d'une
          // valeur discrète à une autre (voir `axisMode`).
          const i = change.binding.hint;
          if (isIntermediate(gp.axes[i] ?? 0, rest.axes[i] ?? 0)) sawIntermediate = true;
        }
        if (now - candidateSince >= CAPTURE_STABLE_MS) {
          commit(candidate);
          return;
        }
      }
      if (now - stepStart >= CAPTURE_TIMEOUT_MS) phase = "timeout";
      return;
    }

    if (phase === "review") tickTest(gp);
  }

  /** Zone d'essai : quatre cases où le repère bouge réellement avec le profil
   * construit. Lire « Haut → axe 9 = -1,00 » ne prouve rien à un utilisateur. */
  function tickTest(gp: Gamepad) {
    if (!rest) return;
    const edge = (name: string, b: Binding | undefined): boolean => {
      const active = !!b && bindingActive(gp, b, rest!);
      const rising = active && !lastTestEdges[name];
      lastTestEdges[name] = active;
      return rising;
    };
    const col = testFocus % 2;
    if (edge("up", captured.up) && testFocus >= 2) testFocus -= 2;
    if (edge("down", captured.down) && testFocus < 2) testFocus += 2;
    if (edge("left", captured.left) && col > 0) testFocus -= 1;
    if (edge("right", captured.right) && col < 1) testFocus += 1;
    if (edge("confirm", captured.confirm) || edge("back", captured.back)) {
      testFlash = true;
      setTimeout(() => (testFlash = false), 180);
    }
  }

  onMount(() => {
    // Suspend la navigation manette globale tant que le panneau est ouvert.
    nav.inputCapture = "controller";
    getVersion()
      .then((v) => (version = v))
      .catch(() => {
        /* hors contexte Tauri : défaut affiché */
      });
    if (controllers.calibrateKey) {
      const d = controllers.live.find((x) => x.key === controllers.calibrateKey);
      if (d) startCalibration(d);
      controllers.calibrateKey = null;
    }
    let raf = 0;
    function frame(now: number) {
      if (calKey) {
        const gp = livePad(calKey);
        // Périphérique endormi ou débranché : on attend, la calibration
        // reprend d'elle-même quand il se remet à parler.
        if (gp) tickCalibration(gp, now);
      }
      raf = requestAnimationFrame(frame);
    }
    raf = requestAnimationFrame(frame);
    return () => {
      cancelAnimationFrame(raf);
      nav.inputCapture = null;
    };
  });

  // --- Partage du profil ---------------------------------------------------

  function reportText(): string {
    const d = calDevice;
    if (!d) return "";
    return profileReport(
      { id: d.id, mapping: d.mapping, axisCount: d.axes.length, buttonCount: d.buttonCount },
      builtProfile,
      version,
    );
  }

  async function copyProfile() {
    try {
      await navigator.clipboard.writeText(reportText());
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (e) {
      // Le profil reste affiché juste en dessous : la sélection à la souris
      // est le repli, pas une impasse.
      console.warn("clipboard", e);
    }
  }

  function openTicket() {
    const title = `Profil manette : ${calDevice?.id ?? ""}`;
    const body = ["```json", reportText(), "```"].join("\n");
    openUrl(`${ISSUE_URL}?title=${encodeURIComponent(title)}&body=${encodeURIComponent(body)}`).catch(() => {});
  }

  function saveProfile() {
    if (!calDevice) return;
    saveDeviceProfile(calDevice.key, calDevice.id, builtProfile);
    selectedKey = calDevice.key;
    cancelCalibration();
  }
</script>

<div class="backdrop">
  <div class="modal">
    <header>
      <h2>{calKey ? t("controller.calib.title") : t("controller.panel.title")}</h2>
      <button class="btn btn-ghost" type="button" onclick={onclose}>✕</button>
    </header>

    {#if !calKey}
      <div class="body">
        <p class="intro">{t("controller.panel.intro")}</p>

        {#if listed.length}
          <div class="devices">
            {#each listed as d (d.key)}
              {@const source = sourceOf(d)}
              <div class="device" class:on={selectedKey === d.key}>
                <label class="pick">
                  <input type="radio" name="device" value={d.key} checked={selectedKey === d.key} onchange={() => (selectedKey = d.key)} />
                  <span class="dev-b">
                    <span class="dev-name">{d.id}</span>
                    <span class="dev-id mono">{identity(d)}</span>
                  </span>
                </label>
                <span class="badge" class:warn={source === "none"}>{t(badgeKeys[source])}</span>
                <button class="btn" type="button" onclick={() => startCalibration(d)}>{t("controller.panel.calibrate")}</button>
              </div>
            {/each}
          </div>
        {:else}
          <!-- Un périphérique n'existe pas tant qu'on ne l'a pas touché
               (Chromium ne l'expose qu'après une première entrée) : donc
               jamais « aucun périphérique détecté », qui ferait croire à un
               écran cassé volant branché et allumé. -->
          <p class="empty">{t("controller.panel.empty")}</p>
        {/if}

        {#if anyUnknown}
          <p class="help">{t("controller.panel.unknownHelp")}</p>
        {/if}

        <details class="tech">
          <summary>{t("controller.panel.tech")}</summary>
          <div class="tech-body mono">
            {#each controllers.live as d (d.index)}
              <div class="tech-row">
                <div class="tech-id">{d.id}</div>
                <div>{t("controller.panel.techMapping")}: {d.mapping}</div>
                <div>{t("controller.panel.techAxes")}: {d.axes.map((a) => a.toFixed(2)).join(", ") || "-"}</div>
                <div>{t("controller.panel.techButtons")}: {d.pressed.join(", ") || "-"}</div>
              </div>
            {:else}
              <div class="tech-row">{t("controller.panel.empty")}</div>
            {/each}
          </div>
        </details>
      </div>

      <footer>
        <button class="btn" type="button" onclick={onclose}>{t("controller.panel.close")}</button>
        <button class="btn" type="button" onclick={answerNone}>{t("controller.panel.none")}</button>
        <button class="btn btn-primary" type="button" disabled={!selectedKey} onclick={useSelected}>
          {t("controller.panel.confirm")}
        </button>
      </footer>
    {:else}
      <div class="body">
        <div class="cal-head">
          <span class="dev-name">{calDevice?.id}</span>
          {#if phase !== "review"}
            <span class="lbl-key">{t("controller.calib.step", { n: stepIdx + 1, total: STEPS.length })}</span>
          {/if}
        </div>

        {#if phase === "rest"}
          <div class="cal-stage">
            <div class="cal-ask">{t("controller.calib.restTitle")}</div>
            <p class="cal-hint">{t("controller.calib.rest")}</p>
            <div class="gauge"><span style="width:{Math.round(restProgress * 100)}%"></span></div>
          </div>
        {:else if phase === "review"}
          <div class="recap">
            {#each STEPS as s (s)}
              <div class="recap-row">
                <span class="lbl-key">{stepLabel(s)}</span>
                <span class="mono">{describeBinding(captured[s])}</span>
              </div>
            {/each}
          </div>
          <p class="cal-hint">{t("controller.calib.tryIt")}</p>
          <div class="testgrid" class:flash={testFlash}>
            {#each [0, 1, 2, 3] as cell (cell)}
              <div class="cell" class:on={testFocus === cell}></div>
            {/each}
          </div>
          <p class="share-note">{t("controller.calib.share")}</p>
          <pre class="report mono">{reportText()}</pre>
        {:else}
          <div class="cal-stage">
            <div class="cal-ask">{t("controller.calib.press", { what: stepLabel(currentStep) })}</div>
            {#if duplicateOf}
              <p class="cal-warn">{t("controller.calib.duplicate", { dir: stepLabel(duplicateOf) })}</p>
            {:else if phase === "timeout"}
              <p class="cal-warn">{t("controller.calib.timeout")}</p>
            {:else if phase === "neutral"}
              <p class="cal-hint">{t("controller.calib.neutral")}</p>
            {:else}
              <p class="cal-hint">{t("controller.calib.waiting")}</p>
            {/if}
          </div>
        {/if}
      </div>

      <footer>
        <button class="btn" type="button" onclick={cancelCalibration}>{t("controller.calib.cancel")}</button>
        <button class="btn" type="button" onclick={restartCalibration}>{t("controller.calib.restart")}</button>
        {#if phase === "review"}
          <button class="btn" type="button" onclick={copyProfile}>
            {copied ? t("controller.calib.copied") : t("controller.calib.copy")}
          </button>
          <button class="btn" type="button" onclick={openTicket}>{t("controller.calib.ticket")}</button>
          <button class="btn btn-primary" type="button" onclick={saveProfile}>{t("controller.calib.save")}</button>
        {:else}
          {#if phase === "timeout"}
            <button class="btn" type="button" onclick={retryStep}>{t("controller.calib.retry")}</button>
          {/if}
          <button class="btn" type="button" onclick={skipStep} disabled={phase === "rest"}>
            {t("controller.calib.skip")}
          </button>
        {/if}
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 120;
  }
  .modal {
    width: 640px;
    max-width: 92vw;
    max-height: 84vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  /* Même traitement que les autres popups (OpponentPicker,
     SavedSessionsDialog) : pas un quatrième niveau de libellé. */
  h2 {
    font-size: 13px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--txt2);
  }
  .body {
    padding: 14px 16px;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  .intro {
    color: var(--txt2);
    font-size: 12px;
    line-height: 1.5;
    margin-bottom: 12px;
  }
  .devices {
    border: 1px solid var(--line);
  }
  .device {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    background: var(--panel2);
  }
  .device + .device {
    border-top: 1px solid var(--line);
  }
  .device.on {
    background: var(--rosso-dim);
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    cursor: pointer;
  }
  .dev-b {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .dev-name {
    font-size: 11.5px;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dev-id {
    font-size: 9px;
    color: var(--faint);
  }
  .badge {
    flex: none;
    font-size: 9px;
    letter-spacing: 0.5px;
    padding: 3px 7px;
    border: 1px solid var(--blue-border);
    background: var(--blue-dim);
    color: var(--blue);
  }
  .badge.warn {
    border-color: #4a4110;
    background: #1a1706;
    color: var(--yellow);
  }
  .empty {
    padding: 14px;
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 11.5px;
    line-height: 1.5;
  }
  .help {
    margin-top: 12px;
    padding: 10px 12px;
    border: 1px solid #4a4110;
    background: #12100a;
    color: var(--txt2);
    font-size: 11.5px;
    line-height: 1.6;
  }
  .tech {
    margin-top: 14px;
  }
  .tech summary {
    color: var(--muted);
    font-size: 10.5px;
    cursor: pointer;
  }
  .tech-body {
    margin-top: 8px;
    border: 1px solid var(--line);
    background: var(--bg);
  }
  .tech-row {
    padding: 8px 10px;
    font-size: 10px;
    color: var(--txt2);
    line-height: 1.6;
  }
  .tech-row + .tech-row {
    border-top: 1px solid var(--line);
  }
  .tech-id {
    color: var(--faint);
    word-break: break-all;
  }
  .cal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--line);
  }
  .cal-stage {
    padding: 26px 0;
    text-align: center;
  }
  .cal-ask {
    font-size: 18px;
    font-weight: 600;
    color: var(--txt);
  }
  .cal-hint {
    margin-top: 10px;
    color: var(--muted);
    font-size: 11.5px;
    line-height: 1.5;
  }
  .cal-warn {
    margin-top: 10px;
    color: var(--yellow);
    font-size: 11.5px;
    line-height: 1.5;
  }
  .gauge {
    margin: 16px auto 0;
    width: 220px;
    height: 4px;
    background: var(--line);
  }
  .gauge span {
    display: block;
    height: 100%;
    background: var(--rosso);
  }
  .recap {
    border: 1px solid var(--line);
    margin-top: 12px;
  }
  .recap-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 10px;
    font-size: 10.5px;
    color: var(--txt2);
  }
  .recap-row + .recap-row {
    border-top: 1px solid var(--line);
  }
  .testgrid {
    margin: 10px auto 0;
    width: 180px;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
  }
  .testgrid.flash {
    border-color: var(--rosso);
  }
  .cell {
    height: 44px;
    background: var(--card);
  }
  .cell.on {
    background: var(--rosso-dim);
    outline: 2px solid var(--rosso);
    outline-offset: -2px;
  }
  .share-note {
    margin-top: 14px;
    color: var(--faint);
    font-size: 10.5px;
    line-height: 1.5;
  }
  .report {
    margin-top: 8px;
    max-height: 140px;
    overflow: auto;
    padding: 8px 10px;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--muted);
    font-size: 9.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-all;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 12px 16px;
    border-top: 1px solid var(--line);
    flex-wrap: wrap;
  }
</style>
