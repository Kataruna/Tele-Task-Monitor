//! The sampler: owns all OS readers, runs on its own thread, and only works while
//! at least one viewer is connected. This file is portable; the OS readers it drives come from
//! `crate::platform::sampler` (see the contract in `src/platform/mod.rs`).

pub mod cpu;
pub mod gpu;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use crate::aggregate::{build_top, group_by_name, round1};
use crate::model::{CpuInfo, CpuSample, GpuSample, Info, RamInfo, RamSample, Sample, SelfUsage};
use crate::platform::sampler::{self as system, CpuTimes, Gpu, ProcSnapshot};
use crate::platform::{CpuLoad, GpuSource, ProcessSource};
use crate::viewers::Viewers;
use cpu::CpuTracker;

/// Latest serialized `Sample` JSON; `None` before the first sample.
pub type SampleTx = watch::Sender<Option<Arc<str>>>;
pub type SampleRx = watch::Receiver<Option<Arc<str>>>;

pub struct Sampler {
    logical_cores: usize,
    interval: Duration,
    self_pid: u32,
    snapshot: ProcSnapshot,
    tracker: CpuTracker,
    cpu_times: CpuTimes,
    gpu: Gpu,
    last: Instant,
}

impl Sampler {
    /// Optional GPU parts that fail to open are logged once and disabled.
    pub fn new(interval: Duration) -> Self {
        let logical_cores = system::logical_cores();
        Self {
            logical_cores,
            interval,
            self_pid: std::process::id(),
            snapshot: ProcSnapshot::new(),
            tracker: CpuTracker::default(),
            cpu_times: CpuTimes::new(logical_cores),
            gpu: Gpu::open(),
            last: Instant::now(),
        }
    }

    pub fn info(&self) -> Info {
        Info {
            v: 1,
            hostname: system::hostname(),
            cpu: CpuInfo { name: system::cpu_brand(), logical_cores: self.logical_cores },
            ram: RamInfo { total: system::ram().1 },
            gpu: self.gpu.info(),
            interval_ms: self.interval.as_millis() as u64,
        }
    }

    /// Free the scratch memory used while sampling (call when the last viewer has left). It is
    /// all re-created on demand, so this only costs a little work on the next wake-up.
    pub fn release_idle(&mut self) {
        self.snapshot.release();
        self.tracker.clear();
        self.gpu.release();
    }

    /// Establish baselines for every delta-based reading. Call after waking from idle,
    /// then wait one interval before `sample()`.
    pub fn prime(&mut self) {
        self.cpu_times.update();
        if let Ok(procs) = self.snapshot.take() {
            self.tracker.update(&procs, Duration::ZERO, self.logical_cores);
        }
        self.gpu.collect();
        self.last = Instant::now();
    }

    pub fn sample(&mut self) -> Sample {
        let now = Instant::now();
        let wall = now - self.last;
        self.last = now;

        let (cpu_usage, cores) = self.cpu_times.update();
        let (ram_used, ram_total) = system::ram();

        let rows = match self.snapshot.take() {
            Ok(procs) => self.tracker.update(&procs, wall, self.logical_cores),
            Err(e) => {
                eprintln!("warning: process snapshot failed: {e}");
                Vec::new()
            }
        };

        let gpu_reading = self.gpu.read();
        // VRAM bounds are applied where the readings are built (in the platform reader) and where
        // per-app sums are formed (`group_by_name`), so they are unit-tested away from the OS.
        let apps = group_by_name(
            &rows,
            gpu_reading.as_ref().map(|r| &r.per_pid),
            gpu_reading.as_ref().and_then(|r| r.app_vram_cap),
        );
        let top = build_top(&apps, crate::aggregate::current_top_n(), gpu_reading.is_some());

        let gpu = gpu_reading.as_ref().map(|r| {
            let (temp_c, power_pct) = self.gpu.perf();
            GpuSample {
                usage: r.usage,
                vram_used: r.vram_used,
                vram_total: self.gpu.vram_total(),
                temp_c,
                power_pct,
            }
        });

        let self_usage = rows
            .iter()
            .find(|r| r.pid == self.self_pid)
            .map(|r| SelfUsage { cpu: round1(r.cpu), mem: r.mem })
            .unwrap_or(SelfUsage { cpu: 0.0, mem: 0 });

        Sample {
            ts: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
            cpu: CpuSample { usage: cpu_usage, cores },
            ram: RamSample { used: ram_used, total: ram_total },
            gpu,
            top,
            self_usage,
        }
    }
}

/// Start the sampler thread. Blocks until the sampler is built and returns its `Info`.
pub fn spawn(viewers: Arc<Viewers>, tx: SampleTx, interval: Duration) -> Info {
    let (info_tx, info_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("sampler".into())
        .spawn(move || {
            // Built on this thread: OS handles (e.g. PDH/D3DKMT on Windows) stay on the thread that uses them.
            let mut sampler = Sampler::new(interval);
            info_tx.send(sampler.info()).expect("main thread waiting for info");
            run(&mut sampler, &viewers, &tx, interval);
        })
        .expect("spawn sampler thread");
    info_rx.recv().expect("sampler thread sent info")
}

/// How long the monitor must stay unwatched before its working set is trimmed. A tab switch or a
/// locked iPad should not make the next wake-up fault everything back in.
const IDLE_TRIM_AFTER: Duration = Duration::from_secs(30);

/// Sampling loop. Sleeps (zero OS calls) while there are no viewers, frees its scratch memory
/// when the last one leaves, and trims its working set after a longer idle spell. Missed ticks
/// are skipped, never queued.
fn run(sampler: &mut Sampler, viewers: &Viewers, tx: &SampleTx, interval: Duration) {
    loop {
        sampler.release_idle();
        if !viewers.wait_for_viewer_timeout(IDLE_TRIM_AFTER) {
            crate::platform::priority::trim_working_set();
            viewers.wait_for_viewer();
        }
        sampler.prime();
        let mut next = Instant::now() + interval;
        while viewers.count() > 0 {
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            }
            let sample = sampler.sample();
            match serde_json::to_string(&sample) {
                Ok(json) => {
                    tx.send_replace(Some(json.into()));
                }
                Err(e) => eprintln!("warning: could not serialize sample: {e}"),
            }
            next += interval;
            let now = Instant::now();
            if next <= now {
                next = now + interval;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::watch;

    #[test]
    fn info_describes_this_pc() {
        let s = Sampler::new(Duration::from_millis(1000));
        let info = s.info();
        assert_eq!(info.v, 1);
        assert!(info.cpu.logical_cores >= 1);
        assert!(info.ram.total > 0);
        assert_eq!(info.interval_ms, 1000);
    }

    #[test]
    fn primed_sample_has_consistent_shape() {
        let mut s = Sampler::new(Duration::from_millis(300));
        s.prime();
        std::thread::sleep(Duration::from_millis(300));
        let sample = s.sample();
        assert_eq!(sample.cpu.cores.len(), s.logical_cores);
        assert!(sample.ram.total > 0);
        let limit = crate::aggregate::current_top_n().max(crate::aggregate::TOP_N);
        assert!(!sample.top.cpu.is_empty() && sample.top.cpu.len() <= limit);
        assert!(!sample.top.mem.is_empty() && sample.top.mem.len() <= limit);
        assert!(sample.self_usage.mem > 0, "own process should be found");
        let json = serde_json::to_string(&sample).unwrap();
        assert!(json.len() < 8192, "sample JSON should be small, was {} bytes", json.len());
    }

    #[test]
    fn spawn_publishes_only_while_watched() {
        let viewers = crate::viewers::Viewers::new();
        let (tx, mut rx) = watch::channel(None);
        let info = spawn(Arc::clone(&viewers), tx, Duration::from_millis(200));
        assert!(info.cpu.logical_cores >= 1);

        std::thread::sleep(Duration::from_millis(500));
        assert!(!rx.has_changed().unwrap(), "no samples without viewers");

        let guard = viewers.enter();
        std::thread::sleep(Duration::from_millis(700));
        assert!(rx.has_changed().unwrap(), "samples flow while watched");
        let json = rx.borrow_and_update().clone().expect("a sample");
        assert!(json.contains("\"top\""));

        drop(guard);
        std::thread::sleep(Duration::from_millis(500)); // let the in-flight tick finish
        rx.borrow_and_update();
        std::thread::sleep(Duration::from_millis(600));
        assert!(!rx.has_changed().unwrap(), "sampler went idle after last viewer left");
    }
}
