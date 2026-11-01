//! GPU name, usage and memory from IOKit: every GPU driver registers an `IOAccelerator` service
//! whose "PerformanceStatistics" property holds live counters (what Activity Monitor's GPU
//! History and `ioreg` show). No root needed.
//!
//! Not available on macOS through public APIs: per-app GPU % (so `per_pid` stays empty and the
//! GPU process list is empty), and temperature / power (need private SMC APIs).

use std::ffi::{CStr, c_char, c_void};

use crate::model::GpuInfo;
use crate::platform::GpuSource;
use crate::sampler::gpu::GpuReading;

// ---- Minimal IOKit / CoreFoundation FFI ----

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFDictionaryRef = *const c_void;
type IoObject = libc::mach_port_t;

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_SINT64_TYPE: isize = 4;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> CFDictionaryRef;
    fn IOServiceGetMatchingServices(main_port: libc::mach_port_t, matching: CFDictionaryRef, iter: *mut IoObject) -> i32;
    fn IOIteratorNext(iter: IoObject) -> IoObject;
    fn IOObjectRelease(obj: IoObject) -> i32;
    fn IORegistryEntryCreateCFProperty(entry: IoObject, key: CFStringRef, allocator: CFTypeRef, options: u32) -> CFTypeRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(alloc: CFTypeRef, s: *const c_char, encoding: u32) -> CFStringRef;
    fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, size: isize, encoding: u32) -> bool;
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: CFTypeRef) -> CFTypeRef;
    fn CFNumberGetValue(num: CFTypeRef, kind: isize, out: *mut c_void) -> bool;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFRelease(cf: CFTypeRef);
}

/// Owned CoreFoundation object, released on drop.
struct Cf(CFTypeRef);

impl Cf {
    fn new(p: CFTypeRef) -> Option<Self> {
        (!p.is_null()).then_some(Self(p))
    }

    fn string(s: &CStr) -> Option<Self> {
        // SAFETY: valid NUL-terminated UTF-8.
        Self::new(unsafe { CFStringCreateWithCString(std::ptr::null(), s.as_ptr(), K_CF_STRING_ENCODING_UTF8) })
    }

    fn is(&self, type_id: unsafe extern "C" fn() -> usize) -> bool {
        // SAFETY: self.0 is a live CF object.
        unsafe { CFGetTypeID(self.0) == type_id() }
    }
}

impl Drop for Cf {
    fn drop(&mut self) {
        // SAFETY: we own one reference.
        unsafe { CFRelease(self.0) }
    }
}

/// CFNumber or CFString -> Rust value helpers for a borrowed (not owned) CF object.
fn cf_i64(v: CFTypeRef) -> Option<i64> {
    // SAFETY: v is a live CF object borrowed from a dictionary.
    if v.is_null() || unsafe { CFGetTypeID(v) != CFNumberGetTypeID() } {
        return None;
    }
    let mut out = 0i64;
    // SAFETY: out is an i64, matching kCFNumberSInt64Type (floats are truncated).
    unsafe { CFNumberGetValue(v, K_CF_NUMBER_SINT64_TYPE, (&mut out as *mut i64).cast()) };
    Some(out)
}

fn cf_string(v: CFTypeRef) -> Option<String> {
    // SAFETY: as above.
    if v.is_null() || unsafe { CFGetTypeID(v) != CFStringGetTypeID() } {
        return None;
    }
    let mut buf = [0 as c_char; 256];
    // SAFETY: buf has buf.len() bytes.
    if !unsafe { CFStringGetCString(v, buf.as_mut_ptr(), buf.len() as isize, K_CF_STRING_ENCODING_UTF8) } {
        return None;
    }
    // SAFETY: CFStringGetCString NUL-terminated it.
    Some(unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().trim().to_string())
}

/// One registry property of `entry`, owned.
fn property(entry: IoObject, key: &CStr) -> Option<Cf> {
    let key = Cf::string(key)?;
    // SAFETY: entry is a live registry entry, key a CFString.
    Cf::new(unsafe { IORegistryEntryCreateCFProperty(entry, key.0, std::ptr::null(), 0) })
}

fn dict_get(dict: &Cf, key: &CStr) -> CFTypeRef {
    let Some(k) = Cf::string(key) else { return std::ptr::null() };
    // SAFETY: dict is a live CFDictionary, k a CFString. The value is borrowed from dict.
    unsafe { CFDictionaryGetValue(dict.0, k.0) }
}

// ---- Readings ----

struct Stats {
    usage: f32,
    mem_used: u64,
    /// Only reported by discrete GPUs (`vramUsedBytes` + `vramFreeBytes`).
    vram_total: Option<u64>,
}

fn read_stats(entry: IoObject) -> Option<Stats> {
    let perf = property(entry, c"PerformanceStatistics")?;
    if !perf.is(CFDictionaryGetTypeID) {
        return None;
    }
    let usage = [c"Device Utilization %", c"GPU Activity(%)"].iter().find_map(|k| cf_i64(dict_get(&perf, k)))?;
    let vram_used = cf_i64(dict_get(&perf, c"vramUsedBytes"));
    let vram_free = cf_i64(dict_get(&perf, c"vramFreeBytes"));
    let mem_used = vram_used.or_else(|| cf_i64(dict_get(&perf, c"In use system memory"))).unwrap_or(0);
    Some(Stats {
        usage: (usage as f32).clamp(0.0, 100.0),
        mem_used: mem_used.max(0) as u64,
        vram_total: vram_used.zip(vram_free).map(|(u, f)| (u + f).max(0) as u64),
    })
}

/// All `IOAccelerator` services (one per GPU). The caller owns the returned objects.
fn accelerators() -> Vec<IoObject> {
    let mut out = Vec::new();
    // SAFETY: the matching dictionary is consumed by IOServiceGetMatchingServices.
    unsafe {
        let matching = IOServiceMatching(c"IOAccelerator".as_ptr());
        if matching.is_null() {
            return out;
        }
        let mut iter: IoObject = 0;
        // 0 = kIOMainPortDefault.
        if IOServiceGetMatchingServices(0, matching, &mut iter) != 0 {
            return out;
        }
        loop {
            let s = IOIteratorNext(iter);
            if s == 0 {
                break;
            }
            out.push(s);
        }
        IOObjectRelease(iter);
    }
    out
}

pub struct Gpu {
    /// The GPU reported on: the first accelerator with performance statistics.
    entry: Option<IoObject>,
    name: String,
    vram_total: u64,
}

impl GpuSource for Gpu {
    fn open() -> Self {
        let mut entry = None;
        for s in accelerators() {
            if entry.is_none() && read_stats(s).is_some() {
                entry = Some(s);
            } else {
                // SAFETY: we own s and are not keeping it.
                unsafe { IOObjectRelease(s) };
            }
        }
        let Some(e) = entry else {
            eprintln!("warning: GPU info disabled: no IOAccelerator with PerformanceStatistics");
            return Self { entry: None, name: String::new(), vram_total: 0 };
        };
        let name = property(e, c"model")
            .and_then(|m| cf_string(m.0))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "GPU".into());
        // Discrete GPUs report their VRAM. Apple Silicon has unified memory the GPU can use, so
        // report the system RAM as its capacity.
        let vram_total = read_stats(e).and_then(|s| s.vram_total).unwrap_or_else(|| super::system::ram().1);
        Self { entry: Some(e), name, vram_total }
    }

    fn info(&self) -> Option<GpuInfo> {
        self.entry.map(|_| GpuInfo { name: self.name.clone(), vram_total: self.vram_total })
    }

    fn vram_total(&self) -> u64 {
        self.vram_total
    }

    /// The driver keeps its own running average, so no baseline is needed.
    fn collect(&mut self) {}

    fn read(&mut self) -> Option<GpuReading> {
        let s = read_stats(self.entry?)?;
        let cap = (self.vram_total > 0).then_some(self.vram_total);
        Some(GpuReading {
            usage: s.usage,
            vram_used: cap.map_or(s.mem_used, |c| s.mem_used.min(c)),
            per_pid: Default::default(),
            app_vram_cap: cap,
        })
    }

    fn perf(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }

    fn release(&mut self) {}
}

impl Drop for Gpu {
    fn drop(&mut self) {
        if let Some(e) = self.entry {
            // SAFETY: we own e.
            unsafe { IOObjectRelease(e) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_this_macs_gpu() {
        let mut gpu = Gpu::open();
        let info = gpu.info().expect("every Mac has an IOAccelerator");
        assert!(!info.name.is_empty());
        assert!(info.vram_total > 0);
        let r = gpu.read().expect("a reading");
        assert!((0.0..=100.0).contains(&r.usage));
        assert!(r.vram_used <= info.vram_total);
    }
}
