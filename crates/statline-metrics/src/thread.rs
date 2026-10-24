//! The sampling thread.
//!
//! One thread, one wait, one publish. The UI reads the latest snapshot through
//! an `ArcSwap` and never blocks; the sampler never touches a window handle.

use crate::Sampler;
use arc_swap::ArcSwap;
use statline_core::model::Snapshot;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

struct Control {
    interval: Duration,
    stopping: bool,
}

/// Handle to the running sampler. Dropping it stops the thread.
pub struct SamplerThread {
    control: Arc<(Mutex<Control>, Condvar)>,
    snapshot: Arc<ArcSwap<Snapshot>>,
    handle: Option<JoinHandle<()>>,
}

impl SamplerThread {
    /// Start sampling immediately, then once per `interval`.
    pub fn start(interval: Duration) -> Self {
        let control = Arc::new((
            Mutex::new(Control {
                interval: interval.max(Duration::from_millis(100)),
                stopping: false,
            }),
            Condvar::new(),
        ));
        let snapshot = Arc::new(ArcSwap::from_pointee(Snapshot::default()));

        let worker_control = Arc::clone(&control);
        let worker_snapshot = Arc::clone(&snapshot);

        let handle = std::thread::Builder::new()
            .name(String::from("statline-sampler"))
            .spawn(move || run(worker_control, worker_snapshot))
            .ok();

        Self {
            control,
            snapshot,
            handle,
        }
    }

    /// Latest published snapshot. Cheap: an atomic load plus refcount bump.
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.load_full()
    }

    /// Change the cadence. Takes effect after the current tick.
    pub fn set_interval(&self, interval: Duration) {
        let (lock, cvar) = &*self.control;
        let mut control = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        control.interval = interval.max(Duration::from_millis(100));
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

fn run(control: Arc<(Mutex<Control>, Condvar)>, snapshot: Arc<ArcSwap<Snapshot>>) {
    let mut sampler = Sampler::new();

    loop {
        let started = Instant::now();
        snapshot.store(Arc::new(sampler.sample()));

        let (lock, cvar) = &*control;
        let mut state = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if state.stopping {
                return;
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
}
