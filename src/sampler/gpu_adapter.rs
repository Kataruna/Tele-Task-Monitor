//! GPU identity (name, total VRAM, LUID) via DXGI, and temperature / power via D3DKMT
//! adapter performance data (the same source Task Manager uses).

use windows::Wdk::Graphics::Direct3D::{
    D3DKMT_ADAPTER_PERFDATA, D3DKMT_CLOSEADAPTER, D3DKMT_OPENADAPTERFROMLUID, D3DKMT_QUERYADAPTERINFO, D3DKMTCloseAdapter,
    D3DKMTOpenAdapterFromLuid, D3DKMTQueryAdapterInfo, KMTQAITYPE_ADAPTERPERFDATA,
};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1};

use crate::aggregate::round1;

/// D3DKMT reports temperature in deci-°C and power in tenths of a percent of the
/// board power limit. 0 means "not reported".
pub fn convert_perf(temperature_deci_c: u32, power_tenths_pct: u32) -> (Option<f32>, Option<f32>) {
    let temp = (temperature_deci_c > 0).then(|| round1(temperature_deci_c as f32 / 10.0));
    let power = (power_tenths_pct > 0).then(|| round1(power_tenths_pct as f32 / 10.0));
    (temp, power)
}

/// Format a Windows `LUID` into the `luid_0xHHHHHHHH_0xLLLLLLLL` tag used in PDH
/// GPU Engine / GPU Memory counter instance names.
pub fn format_luid_tag(luid: windows::Win32::Foundation::LUID) -> String {
    format!("luid_0x{:08x}_0x{:08x}", luid.HighPart as u32, luid.LowPart)
}

pub struct GpuAdapter {
    pub name: String,
    pub vram_total: u64,
    pub luid_tag: String,
    handle: u32,
}

impl GpuAdapter {
    /// Pick the hardware adapter with the most dedicated VRAM (first one wins ties),
    /// skipping software adapters such as "Microsoft Basic Render Driver".
    pub fn open() -> Result<Self, String> {
        // SAFETY: plain COM factory creation; the factory is dropped at the end of this fn.
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(|e| format!("CreateDXGIFactory1: {e}"))?;
        let mut best: Option<(String, u64, windows::Win32::Foundation::LUID)> = None;
        let mut i = 0;
        // SAFETY: EnumAdapters1 returns Err(DXGI_ERROR_NOT_FOUND) after the last adapter.
        while let Ok(adapter) = unsafe { factory.EnumAdapters1(i) } {
            i += 1;
            let Ok(d) = (unsafe { adapter.GetDesc1() }) else { continue };
            if d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            let vram = d.DedicatedVideoMemory as u64;
            if best.as_ref().is_none_or(|b| vram > b.1) {
                let name = String::from_utf16_lossy(&d.Description).trim_end_matches('\0').trim().to_string();
                best = Some((name, vram, d.AdapterLuid));
            }
        }
        let (name, vram_total, luid) = best.ok_or("no hardware GPU adapter found")?;
        let luid_tag = format_luid_tag(luid);
        let mut open = D3DKMT_OPENADAPTERFROMLUID { AdapterLuid: luid, hAdapter: 0 };
        // SAFETY: valid in/out struct.
        let status = unsafe { D3DKMTOpenAdapterFromLuid(&mut open) };
        if status.is_err() {
            return Err(format!("D3DKMTOpenAdapterFromLuid: {:#x}", status.0));
        }
        Ok(Self { name, vram_total, luid_tag, handle: open.hAdapter })
    }

    /// (temperature °C, power % of limit). Either is None if the driver does not report it.
    pub fn perf(&self) -> (Option<f32>, Option<f32>) {
        let mut perf = D3DKMT_ADAPTER_PERFDATA::default();
        let mut q = D3DKMT_QUERYADAPTERINFO {
            hAdapter: self.handle,
            Type: KMTQAITYPE_ADAPTERPERFDATA,
            pPrivateDriverData: (&mut perf as *mut D3DKMT_ADAPTER_PERFDATA).cast(),
            PrivateDriverDataSize: std::mem::size_of::<D3DKMT_ADAPTER_PERFDATA>() as u32,
        };
        // SAFETY: q points at a correctly sized perf struct.
        let status = unsafe { D3DKMTQueryAdapterInfo(&mut q) };
        if status.is_err() {
            return (None, None);
        }
        convert_perf(perf.Temperature, perf.Power)
    }
}

impl Drop for GpuAdapter {
    fn drop(&mut self) {
        let close = D3DKMT_CLOSEADAPTER { hAdapter: self.handle };
        // SAFETY: handle came from D3DKMTOpenAdapterFromLuid.
        unsafe {
            let _ = D3DKMTCloseAdapter(&close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_driver_units() {
        assert_eq!(convert_perf(404, 349), (Some(40.4), Some(34.9)));
        assert_eq!(convert_perf(0, 0), (None, None));
        assert_eq!(convert_perf(715, 0), (Some(71.5), None));
    }

    #[test]
    fn opens_this_pcs_gpu_and_reads_temperature() {
        let gpu = GpuAdapter::open().expect("hardware GPU present on this PC");
        assert!(!gpu.name.is_empty());
        assert!(gpu.luid_tag.starts_with("luid_0x"));
        assert!(gpu.vram_total > 1_000_000_000, "expected > 1 GB VRAM, got {}", gpu.vram_total);
        let (temp, _power) = gpu.perf();
        let t = temp.expect("driver reports temperature");
        assert!((10.0..=110.0).contains(&t), "temperature {t}");
    }
}
