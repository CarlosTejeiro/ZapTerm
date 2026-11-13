//! File-system watcher for the user-overridable hint catalogs directory.
//!
//! Drops a [`notify::RecommendedWatcher`] on `<app_data>/catalogs/` and calls
//! [`core_hints::catalog::reload`] whenever a `*.json` file in it is created,
//! modified, renamed or deleted. The frontend gets a `hint-catalogs-reloaded`
//! event with the count of overrides currently in effect so it can refresh
//! the HintsPanel's status line without a full settings re-open.
//!
//! Events are debounced ~500 ms — editors like VS Code save by writing to a
//! tmp file and renaming it, which produces a burst of two or three events
//! per save. Without debouncing we'd reload three times in a row.

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

/// Spawn the watcher and return its lifetime guard. The caller MUST keep the
/// returned [`WatcherGuard`] alive for the duration of the app — dropping it
/// stops the watcher. The guard is currently stored on `AppState` for that
/// reason.
///
/// The directory is created on demand: if it doesn't exist when this runs,
/// we create it empty so notify has something concrete to watch from second
/// zero. (The "Open catalogs folder" command additionally seeds README +
/// example files, but that's an orthogonal user action.)
pub fn spawn(app: AppHandle, catalogs_dir: PathBuf) -> Option<WatcherGuard> {
    if let Err(e) = std::fs::create_dir_all(&catalogs_dir) {
        tracing::warn!(
            "hint catalogs dir {} create failed: {e}",
            catalogs_dir.display()
        );
        return None;
    }

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher: RecommendedWatcher = match Watcher::new(tx, Config::default()) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("hint catalogs watcher init failed: {e}");
            return None;
        }
    };
    if let Err(e) = watcher.watch(&catalogs_dir, RecursiveMode::NonRecursive) {
        tracing::warn!(
            "hint catalogs watcher start failed for {}: {e}",
            catalogs_dir.display()
        );
        return None;
    }

    // Move the watcher into the worker thread so it lives as long as the
    // thread does. The guard returned to the caller is the JoinHandle plus
    // a shutdown sender — dropping the guard ends the worker.
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
    let handle = thread::spawn(move || {
        // Keep `watcher` alive in this scope — its Drop unregisters from the OS.
        let _watcher = watcher;
        worker_loop(app, rx, shutdown_rx);
    });

    Some(WatcherGuard {
        handle: Some(handle),
        shutdown: Some(shutdown_tx),
    })
}

fn worker_loop(
    app: AppHandle,
    rx: mpsc::Receiver<notify::Result<Event>>,
    shutdown: mpsc::Receiver<()>,
) {
    const DEBOUNCE: Duration = Duration::from_millis(500);
    let mut pending_since: Option<Instant> = None;

    loop {
        // Check shutdown first so a flood of events can't starve us.
        if shutdown.try_recv().is_ok() {
            return;
        }

        // Block briefly waiting for the next event; if none arrives we still
        // loop so we can fire a pending debounced reload.
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                if event_is_interesting(&event) {
                    pending_since = Some(Instant::now());
                }
            }
            Ok(Err(e)) => {
                tracing::debug!("hint catalogs watch error: {e}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }

        if let Some(since) = pending_since {
            if since.elapsed() >= DEBOUNCE {
                pending_since = None;
                match core_hints::catalog::reload() {
                    Ok(n) => {
                        tracing::debug!("hint catalogs reloaded: {n} overrides applied");
                        let _ = app.emit("hint-catalogs-reloaded", n);
                    }
                    Err(e) => {
                        tracing::warn!("hint catalogs reload failed: {e}");
                        let _ = app.emit("hint-catalogs-error", e.to_string());
                    }
                }
            }
        }
    }
}

/// Only react to events that could change the active catalog set. We ignore
/// metadata-only changes (e.g. atime bumps) and the tmp/lockfile noise some
/// editors emit while writing.
fn event_is_interesting(event: &Event) -> bool {
    use notify::EventKind::*;
    if !matches!(event.kind, Create(_) | Modify(_) | Remove(_)) {
        return false;
    }
    event
        .paths
        .iter()
        .any(|p| matches!(p.extension().and_then(|s| s.to_str()), Some("json")))
}

/// Keeps the watcher thread alive. Drop ends the worker (the watcher itself
/// is dropped inside the worker as the thread exits).
pub struct WatcherGuard {
    handle: Option<thread::JoinHandle<()>>,
    shutdown: Option<mpsc::Sender<()>>,
}

impl Drop for WatcherGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
