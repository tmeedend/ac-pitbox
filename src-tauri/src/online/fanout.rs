//! Asking many servers at once. WinHTTP blocks, and the servers to ask run in
//! the hundreds (every busy server to find friends, every visible row to ping
//! it), so the work is spread over a few threads pulling from a shared index.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Requests in flight at once. Enough to scan a busy evening in seconds,
/// few enough not to look like a flood to any one host (most hosts run a
/// handful of servers, and the list is not grouped by host).
pub const PARALLEL: usize = 32;

/// Applies `ask` to every item, `PARALLEL` at a time, and keeps the answers
/// that came back (`Some`), in no particular order. Every item is asked
/// exactly once.
pub fn fan_out<I: Sync, T: Send>(items: &[I], ask: impl Fn(&I) -> Option<T> + Sync) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let found = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..PARALLEL.min(items.len()) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(i) else { break };
                let Some(answer) = ask(item) else { continue };
                // A poisoned lock means another worker panicked: keep what it
                // left, every caller is best-effort anyway.
                found.lock().unwrap_or_else(|e| e.into_inner()).push(answer);
            });
        }
    });
    found.into_inner().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule: every item is asked exactly once, whatever the parallelism —
    /// more items than workers is the normal case.
    #[test]
    fn every_item_is_asked_once() {
        let items: Vec<u32> = (0..100).collect();
        let asked = Mutex::new(Vec::new());
        let answers = fan_out(&items, |&i| {
            asked.lock().unwrap().push(i);
            (i % 2 == 0).then_some(i)
        });
        let mut asked = asked.into_inner().unwrap();
        asked.sort();
        assert_eq!(asked, items, "each item once");
        assert_eq!(answers.len(), 50, "only the answers that came back");
    }
}
