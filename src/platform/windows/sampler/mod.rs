//! Windows OS readers for the portable sampler (`crate::sampler`).
//!
//! - `system`       — CPU (total + per core), RAM, CPU brand, hostname
//! - `procs`        — process snapshot (`NtQuerySystemInformation`)
//! - `gpu_adapter`  — GPU identity via DXGI, temperature / power via D3DKMT
//! - `gpu_counters` — GPU usage and per-app GPU / VRAM via PDH performance counters

pub mod gpu_adapter;
pub mod gpu_counters;
pub mod procs;
pub mod system;

pub use procs::ProcSnapshot;
pub use system::{CpuTimes, cpu_brand, hostname, logical_cores, ram};

use crate::model::GpuInfo;
use crate::platform::GpuSource;
use crate::sampler::gpu::GpuReading;
use gpu_adapter::GpuAdapter;
use gpu_counters::GpuCounters;

/// DXGI adapter (identity, temp/power) + PDH counters (usage). Either can be missing: the
/// counters still work without an adapter, they are just not filtered to one GPU.
pub struct Gpu {
    adapter: Option<GpuAdapter>,
    counters: Option<GpuCounters>,
}

impl GpuSource for Gpu {
    fn open() -> Self {
        let adapter = GpuAdapter::open().map_err(|e| eprintln!("warning: GPU info disabled: {e}")).ok();
        let counters =
            GpuCounters::open(adapter.as_ref()).map_err(|e| eprintln!("warning: per-app GPU disabled: {e}")).ok();
        Self { adapter, counters }
    }

    fn info(&self) -> Option<GpuInfo> {
        self.adapter.as_ref().map(|g| GpuInfo { name: g.name.clone(), vram_total: g.vram_total })
    }

    fn vram_total(&self) -> u64 {
        self.adapter.as_ref().map_or(0, |a| a.vram_total)
    }

    fn collect(&mut self) {
        if let Some(c) = self.counters.as_mut() {
            c.collect(); // baseline only: formatting thousands of instances here would be thrown away
        }
    }

    fn read(&mut self) -> Option<GpuReading> {
        self.counters.as_mut().and_then(|c| c.read().ok())
    }

    fn perf(&self) -> (Option<f32>, Option<f32>) {
        self.adapter.as_ref().map(|a| a.perf()).unwrap_or((None, None))
    }

    fn release(&mut self) {
        if let Some(c) = self.counters.as_mut() {
            c.release();
        }
    }
}
