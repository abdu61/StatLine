//! The sampling thread.
//!
//! One thread, one wait, one publish. The UI reads the latest snapshot through
//! an `ArcSwap` and never blocks; the sampler never touches a window handle.

use crate::Sampler;
use arc_swap::ArcSwap;
use statline_core::model::Snapshot;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

struct Control {
    interval: Duration,
    stopping: bool,
    paused: bool,
    safe_mode: bool,
}

/// Handle to the running sampler. Dropping it stops the thread.
pub struct SamplerThread {
    control: Arc<(Mutex<Control>, Condvar)>,
    snapshot: Arc<ArcSwap<Snapshot>>,
    details_visible: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl SamplerThread {
    /// Start sampling immediately, then once per `interval`.
    pub fn start(interval: Duration) -> Self {
        let control = Arc::new((
            Mutex::new(Control {
                interval: interval.max(Duration::from_millis(100)),
                stopping: false,
                paused: false,
                safe_mode: false,
            }),
            Condvar::new(),
        ));
        let snapshot = Arc::new(ArcSwap::from_pointee(Snapshot::default()));
        let details_visible = Arc::new(AtomicBool::new(false));

        let worker_control = Arc::clone(&control);
        let worker_snapshot = Arc::clone(&snapshot);
        let worker_process = Arc::clone(&details_visible);

        let handle = std::thread::Builder::new()
            .name(String::from("statline-sampler"))
            .spawn(move || run(worker_control, worker_snapshot, worker_process))
            .ok();

        Self {
            control,
            snapshot,
            details_visible,
            handle,
        }
    }

    /// Latest published snapshot. Cheap: an atomic load plus refcount bump.
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.load_full()
    }

    /// Include the process table in snapshots while a process view is visible.
    ///
    /// The next tick picks this up; there is no wake-up, so closing every
    /// process surface simply lets the next tick omit the table.
    pub fn set_detail_surfaces(&self, enabled: bool) {
        self.details_visible.store(enabled, Ordering::Relaxed);
    }

    /// Change the cadence. Takes effect after the current tick.
    pub fn set_interval(&self, interval: Duration) {
        let (lock, cvar) = &*self.control;
        let mut control = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        control.interval = interval.max(Duration::from_millis(100));
        cvar.notify_all();
    }

    /// Restrict sampling to the documented CPU/memory/system counters.
    pub fn set_safe_mode(&self, enabled: bool) {
        let (lock, cvar) = &*self.control;
        let mut control = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        control.safe_mode = enabled;
        cvar.notify_all();
    }

    /// Pause or resume sampling. The last snapshot stays readable.
    ///
    /// While paused the thread waits on the same condition variable, so a paused
    /// app costs no sampling CPU at all.
    pub fn set_paused(&self, paused: bool) {
        let (lock, cvar) = &*self.control;
        let mut control = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        control.paused = paused;
        cvar.notify_all();
    }

    /// Stop sampling and wait for the thread to exit. Idempotent.
    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            {
                let (lock, cvar) = &*self.control;
                let mut control = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                control.stopping = true;
                cvar.notify_all();
            }
            let _ = handle.join();
        }
    }
}

impl Drop for SamplerThread {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(
    control: Arc<(Mutex<Control>, Condvar)>,
    snapshot: Arc<ArcSwap<Snapshot>>,
    details_visible: Arc<AtomicBool>,
) {
    let mut sampler = Sampler::new();

    loop {
        let started = Instant::now();
        {
            let (lock, _) = &*control;
            let state = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.safe_mode != sampler.safe_mode() {
                let enabled = state.safe_mode;
                drop(state);
                sampler.set_safe_mode(enabled);
                if enabled {
                    log::info!("sampler in safe mode");
                }
            }
        }
        sampler.set_detail_surfaces(details_visible.load(Ordering::Relaxed));
        snapshot.store(Arc::new(sampler.sample()));

        let (lock, cvar) = &*control;
        let mut state = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if state.stopping {
                return;
            }
            // Paused means "no work at all until someone wakes us".
            if state.paused {
                state = cvar
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                continue;
            }
            // Recompute the deadline from the current interval so a settings
            // change takes effect without restarting the thread.
            let deadline = started + state.interval;
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            state = cvar
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wait until a real sample has been published, or give up.
    fn wait_for_sample(thread: &SamplerThread) -> Arc<Snapshot> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let snapshot = thread.snapshot();
            if snapshot.cpu.is_some() {
                return snapshot;
            }
            if Instant::now() >= deadline {
                panic!("no snapshot published within 3 s");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn publishes_snapshots_and_stops_cleanly() {
        let mut thread = SamplerThread::start(Duration::from_millis(120));
        let first = wait_for_sample(&thread);
        assert!(first.cpu.is_some());
        assert!(first.memory.is_some());

        std::thread::sleep(Duration::from_millis(260));
        let second = thread.snapshot();
        assert!(second.taken_at.is_some());
        assert!(second.cpu.as_ref().expect("cpu").total_percent <= 100.0);

        thread.stop();
    }

    #[test]
    fn interval_changes_take_effect() {
        let mut thread = SamplerThread::start(Duration::from_secs(30));
        thread.set_interval(Duration::from_millis(100));
        let _ = wait_for_sample(&thread);
        thread.stop();
    }

    #[test]
    fn pausing_stops_publishing_until_resumed() {
        let mut thread = SamplerThread::start(Duration::from_millis(80));
        let _ = wait_for_sample(&thread);

        thread.set_paused(true);
        // Let any in-flight tick land, then take a reference point.
        std::thread::sleep(Duration::from_millis(200));
        let paused_at = thread.snapshot().taken_at;

        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            thread.snapshot().taken_at,
            paused_at,
            "no snapshot should be published while paused"
        );

        thread.set_paused(false);
        std::thread::sleep(Duration::from_millis(300));
        assert_ne!(
            thread.snapshot().taken_at,
            paused_at,
            "sampling should resume"
        );
        thread.stop();
    }

    #[test]
    fn process_table_only_appears_on_request() {
        let mut thread = SamplerThread::start(Duration::from_millis(120));
        let _ = wait_for_sample(&thread);
        assert!(
            thread.snapshot().processes.is_none(),
            "the process table must stay off until a process surface opens"
        );

        thread.set_detail_surfaces(true);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(processes) = thread.snapshot().processes.clone() {
                assert!(processes.total > 0);
                assert!(!processes.top.is_empty());
                break;
            }
            assert!(
                Instant::now() < deadline,
                "process table never appeared after enabling"
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        thread.set_detail_surfaces(false);
        let deadline = Instant::now() + Duration::from_secs(3);
        while thread.snapshot().processes.is_some() {
            assert!(
                Instant::now() < deadline,
                "process table never stopped after disabling"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        thread.stop();
    }
}
