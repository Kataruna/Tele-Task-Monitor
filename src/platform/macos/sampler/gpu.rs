//! GPU identity, usage and temperature.

use crate::model::GpuInfo;
use crate::platform::GpuSource;
use crate::sampler::gpu::GpuReading;

pub struct Gpu;

impl GpuSource for Gpu {
    fn open() -> Self {
        // TODO(macos): find the `IOAccelerator` service in IOKit; its "model" property is the name.
        // Apple Silicon has unified memory, so "VRAM total" is not a fixed number: report the
        // GPU's current allocation, or leave it 0.
        Self
    }

    fn info(&self) -> Option<GpuInfo> {
        None
    }

    fn vram_total(&self) -> u64 {
        0
    }

    fn collect(&mut self) {}

    fn read(&mut self) -> Option<GpuReading> {
        // TODO(macos): `IOAccelerator` → "PerformanceStatistics" → "Device Utilization %" and
        // "In use system memory". There is no public per-process GPU API, so `per_pid` stays empty.
        None
    }

    fn perf(&self) -> (Option<f32>, Option<f32>) {
        // TODO(macos): temperature needs private SMC / IOHIDEventSystem APIs; optional.
        (None, None)
    }

    fn release(&mut self) {}
}
