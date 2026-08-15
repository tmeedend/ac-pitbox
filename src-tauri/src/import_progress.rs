//! Import progress reporting (§4.2bis): the `import:progress` event, the
//! batch bookkeeping behind it, and the cancellation flag.
//!
//! # Why two bars can be trusted against each other
//!
//! The overall bar is **not** counted in items. Weighting a forty-item batch by
//! item count makes the bar lurch, because a 3 MB skin and a 2 GB track are one
//! step each. It is counted in *estimated seconds* instead
//! (`import_bench`), and the current item's own bar is a slice of the very same
//! total. The overall ratio is recomputed from the per-item ratios on every
//! emission, so it cannot contradict, lag behind, or overtake the item bar —
//! it contains it by construction.
//!
//! # Why extraction and filing are tracked separately
//!
//! They overlap: the batch extracts item N+1 while it files item N (§4.2bis).
//! Two independent cursors over the same weight total let the overall bar
//! account for both streams while the *displayed* item stays the one being
//! filed — the honest answer to "where are we", since that is the one being
//! written into the library.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::import_bench::{self, Bench, Bucket};

/// Phase of the item currently displayed. `sizing` covers the pass that
/// measures the batch before any work starts — without it, picking a folder of
/// forty mods looks frozen for the duration of the walk.
pub const PHASE_SIZING: &str = "sizing";
pub const PHASE_EXTRACT: &str = "extract";
pub const PHASE_SCAN: &str = "scan";
pub const PHASE_FILING: &str = "filing";
pub const PHASE_DONE: &str = "done";
pub const PHASE_CANCELLED: &str = "cancelled";

/// Emitted as `import:progress`. Mirrored by `ImportProgress` in
/// `src/lib/library.ts` — the two must be changed together.
#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    /// 1-based rank of the item being filed, 0 while sizing the batch.
    pub item_index: usize,
    pub item_count: usize,
    /// Whole batch, in [0,1], weighted by estimated seconds.
    pub overall_ratio: f64,
    /// Current item, in [0,1].
    pub item_ratio: f64,
    /// Estimated seconds left for the whole batch, `None` until the batch has
    /// run long enough to say anything.
    pub eta_secs: Option<u64>,
    /// Name of the current item (archive or folder).
    pub archive: String,
    pub phase: String,
    /// Rank of the mod inside the current item, when it holds several.
    pub sub_current: usize,
    pub sub_total: usize,
    pub label: String,
}

/// Minimum delay between two emissions. 7-Zip reports its percentage far more
/// often than a human can read, and forty archives' worth of that would flood
/// the IPC channel — which is precisely the responsiveness this feature is
/// supposed to protect.
const EMIT_INTERVAL_MS: u128 = 100;

/// Smoothing applied to the displayed ETA. An estimate that jumps around is
/// worse than no estimate at all.
const ETA_SMOOTHING: f64 = 0.3;

/// Below this share of the batch, the measured speed says more about start-up
/// noise than about the work — the benchmark alone answers until then.
const ETA_MIN_PROGRESS: f64 = 0.02;

/// One item of the batch, weighted before any work starts.
pub struct ItemPlan {
    pub label: String,
    /// Estimated seconds of extraction, 0 for a folder import.
    pub extract_w: f64,
    /// Estimated seconds of everything else.
    pub file_w: f64,
}

/// How far each item is along, per stream. Both in [0,1].
#[derive(Clone, Copy, Default)]
struct ItemRatios {
    extract: f64,
    file: f64,
}

struct BatchState {
    plans: Vec<ItemPlan>,
    ratios: Vec<ItemRatios>,
    total_w: f64,
    current: usize,
    phase: String,
    label: String,
    sub_current: usize,
    sub_total: usize,
    started: Instant,
    last_emit: Option<Instant>,
    eta: Option<f64>,
}

impl BatchState {
    fn new() -> Self {
        Self {
            plans: Vec::new(),
            ratios: Vec::new(),
            total_w: 0.0,
            current: 0,
            phase: PHASE_SIZING.to_string(),
            label: String::new(),
            sub_current: 0,
            sub_total: 0,
            started: Instant::now(),
            last_emit: None,
            eta: None,
        }
    }

    /// Estimated seconds already accounted for, across both streams.
    fn progressed(&self) -> f64 {
        self.plans
            .iter()
            .zip(&self.ratios)
            .map(|(p, r)| p.extract_w * r.extract + p.file_w * r.file)
            .sum()
    }

    fn item_ratio(&self) -> f64 {
        let (Some(plan), Some(r)) = (self.plans.get(self.current), self.ratios.get(self.current)) else {
            return 0.0;
        };
        let w = plan.extract_w + plan.file_w;
        if w <= 0.0 {
            return r.file;
        }
        ((plan.extract_w * r.extract + plan.file_w * r.file) / w).clamp(0.0, 1.0)
    }

    /// Recalibrates the benchmark's estimate against what this batch is
    /// actually doing: `elapsed / progressed` is the correction factor between
    /// estimated and real seconds, so an estimate wrong by 2x converges after
    /// the first item instead of staying wrong for the whole batch.
    fn refresh_eta(&mut self, progressed: f64) {
        if self.total_w <= 0.0 {
            self.eta = None;
            return;
        }
        let remaining = (self.total_w - progressed).max(0.0);
        let elapsed = self.started.elapsed().as_secs_f64();
        let raw = if progressed > self.total_w * ETA_MIN_PROGRESS && elapsed > 1.0 {
            remaining * (elapsed / progressed)
        } else {
            remaining
        };
        self.eta = Some(match self.eta {
            Some(prev) => prev * (1.0 - ETA_SMOOTHING) + raw * ETA_SMOOTHING,
            None => raw,
        });
    }

    fn snapshot(&mut self) -> Progress {
        let progressed = self.progressed();
        let overall = if self.total_w > 0.0 {
            (progressed / self.total_w).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.refresh_eta(progressed);
        Progress {
            item_index: if self.plans.is_empty() { 0 } else { self.current + 1 },
            item_count: self.plans.len(),
            overall_ratio: overall,
            item_ratio: self.item_ratio(),
            eta_secs: self.eta.map(|e| e.max(0.0).round() as u64),
            archive: self
                .plans
                .get(self.current)
                .map(|p| p.label.clone())
                .unwrap_or_default(),
            phase: self.phase.clone(),
            sub_current: self.sub_current,
            sub_total: self.sub_total,
            label: self.label.clone(),
        }
    }
}

/// Everything an import needs to report on itself: the event channel, the
/// benchmark being updated as it goes, and the cancellation flag. Passed down
/// the whole pipeline in place of the bare emit closure it replaces.
type EmitFn = Box<dyn Fn(Progress) + Send + Sync>;
type PersistFn = Box<dyn Fn(&Bench) + Send + Sync>;

pub struct ImportCtx {
    emit: Option<EmitFn>,
    persist: Option<PersistFn>,
    bench: Mutex<Bench>,
    state: Mutex<BatchState>,
    cancel: Arc<AtomicBool>,
}

impl ImportCtx {
    /// Production context: emits to the webview and persists the benchmark.
    pub fn new(app: &AppHandle, cancel: Arc<AtomicBool>) -> Self {
        let emitter = app.clone();
        let saver = app.clone();
        Self {
            emit: Some(Box::new(move |p| {
                let _ = emitter.emit("import:progress", p);
            })),
            persist: Some(Box::new(move |b| import_bench::save(&saver, b))),
            bench: Mutex::new(import_bench::load(app)),
            state: Mutex::new(BatchState::new()),
            cancel,
        }
    }

    /// Test context: same bookkeeping, nothing emitted and nothing written.
    #[cfg(test)]
    pub fn silent() -> Self {
        Self::silent_with_cancel(false)
    }

    /// Test context whose cancellation flag is already raised.
    #[cfg(test)]
    pub fn silent_cancelled() -> Self {
        Self::silent_with_cancel(true)
    }

    #[cfg(test)]
    fn silent_with_cancel(cancel: bool) -> Self {
        Self {
            emit: None,
            persist: None,
            bench: Mutex::new(Bench::default()),
            state: Mutex::new(BatchState::new()),
            cancel: Arc::new(AtomicBool::new(cancel)),
        }
    }

    /// Avancement de l'item courant, pour les tests d'autres modules qui
    /// vérifient ce qu'ils poussent dans la barre.
    #[cfg(test)]
    pub fn item_ratio_for_test(&self) -> f64 {
        self.state.lock().expect("progress mutex").item_ratio()
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Estimated seconds for `bytes` in this bucket. The per-item overhead is
    /// added once by the caller, not once per phase.
    pub fn estimate(&self, bucket: Bucket, bytes: u64) -> f64 {
        let bench = self.bench.lock().expect("bench mutex");
        bench.estimate(bucket, bytes)
    }

    /// Folds a real measurement into the benchmark and writes it out. Persisted
    /// per item rather than per batch: an import interrupted halfway has still
    /// learnt something.
    pub fn record(&self, bucket: Bucket, bytes: u64, secs: f64) {
        let mut bench = self.bench.lock().expect("bench mutex");
        bench.record(bucket, bytes, secs);
        if let Some(persist) = &self.persist {
            persist(&bench);
        }
    }

    /// Declares the batch. Called once the sizing pass is done.
    pub fn plan(&self, plans: Vec<ItemPlan>) {
        let mut st = self.state.lock().expect("progress mutex");
        st.total_w = plans.iter().map(|p| p.extract_w + p.file_w).sum();
        st.ratios = vec![ItemRatios::default(); plans.len()];
        st.plans = plans;
        st.current = 0;
        drop(st);
        self.emit(true);
    }

    /// Reports the sizing pass, before the batch is known.
    pub fn sizing(&self, current: usize, total: usize, label: String) {
        let mut st = self.state.lock().expect("progress mutex");
        st.phase = PHASE_SIZING.to_string();
        st.sub_current = current;
        st.sub_total = total;
        st.label = label;
        drop(st);
        self.emit(false);
    }

    /// Moves the display to the item now being filed.
    pub fn set_current(&self, index: usize, phase: &str, label: String) {
        let mut st = self.state.lock().expect("progress mutex");
        st.current = index;
        st.phase = phase.to_string();
        st.label = label;
        st.sub_current = 0;
        st.sub_total = 0;
        drop(st);
        self.emit(true);
    }

    /// Changes the phase of the current item.
    pub fn phase(&self, phase: &str, label: String) {
        let mut st = self.state.lock().expect("progress mutex");
        st.phase = phase.to_string();
        st.label = label;
        drop(st);
        self.emit(true);
    }

    /// Rank of the mod being filed inside the current item.
    pub fn sub(&self, current: usize, total: usize, label: String) {
        let mut st = self.state.lock().expect("progress mutex");
        st.sub_current = current;
        st.sub_total = total;
        st.label = label;
        drop(st);
        self.emit(false);
    }

    /// Extraction cursor for `index` — which is not necessarily the displayed
    /// item when the batch runs ahead of itself.
    pub fn extract_ratio(&self, index: usize, ratio: f64) {
        let mut st = self.state.lock().expect("progress mutex");
        if let Some(r) = st.ratios.get_mut(index) {
            // Never walks back: 7-Zip restarts its percentage on every volume
            // of a multi-part archive.
            r.extract = r.extract.max(ratio.clamp(0.0, 1.0));
        }
        drop(st);
        self.emit(false);
    }

    /// Filing cursor for `index`.
    pub fn file_ratio(&self, index: usize, ratio: f64) {
        let mut st = self.state.lock().expect("progress mutex");
        if let Some(r) = st.ratios.get_mut(index) {
            r.file = r.file.max(ratio.clamp(0.0, 1.0));
        }
        drop(st);
        self.emit(false);
    }

    /// Marks an item entirely accounted for — including one that errored or was
    /// skipped, otherwise the overall bar would never reach its end.
    pub fn finish_item(&self, index: usize) {
        let mut st = self.state.lock().expect("progress mutex");
        if let Some(r) = st.ratios.get_mut(index) {
            r.extract = 1.0;
            r.file = 1.0;
        }
        drop(st);
        self.emit(true);
    }

    /// Final event of the batch.
    pub fn finish_batch(&self, cancelled: bool) {
        let mut st = self.state.lock().expect("progress mutex");
        st.phase = if cancelled { PHASE_CANCELLED } else { PHASE_DONE }.to_string();
        st.label = String::new();
        drop(st);
        self.emit(true);
    }

    fn emit(&self, force: bool) {
        let Some(emit) = &self.emit else { return };
        let mut st = self.state.lock().expect("progress mutex");
        let now = Instant::now();
        if !force {
            if let Some(last) = st.last_emit {
                if now.duration_since(last).as_millis() < EMIT_INTERVAL_MS {
                    return;
                }
            }
        }
        st.last_emit = Some(now);
        let payload = st.snapshot();
        drop(st);
        emit(payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plans(weights: &[(f64, f64)]) -> Vec<ItemPlan> {
        weights
            .iter()
            .enumerate()
            .map(|(i, (e, f))| ItemPlan {
                label: format!("item{i}"),
                extract_w: *e,
                file_w: *f,
            })
            .collect()
    }

    /// Règle : la barre globale contient la barre de l'item courant — elle est
    /// recalculée à partir d'elle, donc les deux ne peuvent pas se contredire.
    /// C'est tout l'intérêt d'un poids en secondes plutôt qu'en nombre d'items.
    #[test]
    fn overall_bar_contains_the_current_item_bar() {
        let ctx = ImportCtx::silent();
        // Deux items de poids très différents : 1 s et 9 s.
        ctx.plan(plans(&[(0.0, 1.0), (0.0, 9.0)]));
        ctx.finish_item(0);
        ctx.set_current(1, PHASE_FILING, "item1".into());
        ctx.file_ratio(1, 0.5);

        let mut st = ctx.state.lock().unwrap();
        let p = st.snapshot();
        assert!((p.item_ratio - 0.5).abs() < 1e-9, "item courant à mi-parcours");
        // 1 s finie + 4,5 s sur 10 s au total.
        assert!(
            (p.overall_ratio - 0.55).abs() < 1e-9,
            "globale pondérée par le temps, obtenu {}",
            p.overall_ratio
        );
    }

    /// Règle : un item en erreur ou ignoré est quand même consommé, sinon la
    /// barre globale n'atteint jamais sa fin et l'ETA ne retombe jamais à zéro.
    #[test]
    fn failed_item_still_completes_the_overall_bar() {
        let ctx = ImportCtx::silent();
        ctx.plan(plans(&[(1.0, 1.0), (1.0, 1.0)]));
        ctx.finish_item(0);
        ctx.finish_item(1);

        let mut st = ctx.state.lock().unwrap();
        let p = st.snapshot();
        assert!((p.overall_ratio - 1.0).abs() < 1e-9, "lot entièrement consommé");
        assert_eq!(p.eta_secs, Some(0), "plus rien à attendre");
    }

    /// Règle : la progression ne recule jamais. 7-Zip repart de 0 % sur chaque
    /// volume d'une archive multi-parties — sans garde, la barre reculerait.
    #[test]
    fn progress_never_walks_backwards() {
        let ctx = ImportCtx::silent();
        ctx.plan(plans(&[(1.0, 1.0)]));
        ctx.extract_ratio(0, 0.8);
        ctx.extract_ratio(0, 0.1);

        let mut st = ctx.state.lock().unwrap();
        let p = st.snapshot();
        assert!(
            (p.item_ratio - 0.4).abs() < 1e-9,
            "0,8 conservé, obtenu {}",
            p.item_ratio
        );
    }

    /// Règle : l'extraction en avance sur le rangement (pipeline, §4.2bis)
    /// alimente la barre globale sans déplacer l'item affiché — c'est celui
    /// qu'on écrit en bibliothèque qui répond à « où en est-on ».
    #[test]
    fn prefetched_extraction_feeds_only_the_overall_bar() {
        let ctx = ImportCtx::silent();
        ctx.plan(plans(&[(1.0, 1.0), (1.0, 1.0)]));
        ctx.set_current(0, PHASE_FILING, "item0".into());
        ctx.extract_ratio(0, 1.0);
        // L'item suivant s'extrait déjà pendant qu'on range le premier.
        ctx.extract_ratio(1, 1.0);

        let mut st = ctx.state.lock().unwrap();
        let p = st.snapshot();
        assert_eq!(p.item_index, 1, "l'item affiché reste celui qu'on range");
        assert!((p.item_ratio - 0.5).abs() < 1e-9, "item 0 : extrait, pas encore rangé");
        assert!(
            (p.overall_ratio - 0.5).abs() < 1e-9,
            "les deux extractions comptent dans la globale, obtenu {}",
            p.overall_ratio
        );
    }
}
