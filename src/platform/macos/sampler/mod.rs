//! macOS OS readers for the portable sampler (`crate::sampler`).
//!
//! - `system` — CPU (total + per core), RAM, CPU brand, hostname
//! - `procs`  — process snapshot
//! - `gpu`    — GPU identity, usage, temperature

pub mod gpu;
pub mod procs;
pub mod system;

pub use gpu::Gpu;
pub use procs::ProcSnapshot;
pub use system::{CpuTimes, cpu_brand, hostname, logical_cores, ram};
