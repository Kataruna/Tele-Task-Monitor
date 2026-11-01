//! Portable GPU types shared by every platform's GPU reader.

use std::collections::HashMap;

use crate::aggregate::ProcGpu;

/// One GPU reading, as produced by `platform::sampler::Gpu::read`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GpuReading {
    /// Overall GPU busy %, 0–100 (on Windows: the busiest engine, Task Manager's headline GPU %).
    pub usage: f32,
    /// GPU memory in use, bytes (never above the adapter's capacity when that is known).
    /// Dedicated VRAM on discrete GPUs; on unified-memory machines (Apple Silicon) the GPU's
    /// share of system memory.
    pub vram_used: u64,
    /// Per-PID GPU % and GPU memory, as the OS reports them. Empty when the platform has no
    /// per-process GPU source. Per-process figures can over-count shared allocations, so they
    /// are bounded per *app* via `app_vram_cap`.
    pub per_pid: HashMap<u32, ProcGpu>,
    /// Upper bound for one app's summed GPU memory: no app can hold more than the adapter
    /// reports in use, or, when that is unknown, more than its capacity. `None` when neither is
    /// known.
    pub app_vram_cap: Option<u64>,
}
