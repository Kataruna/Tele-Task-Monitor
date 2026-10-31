//! JSON contract for `/api/stream`. Field names here ARE the public contract (spec §4).

use serde::Serialize;

/// Sent once per connection as `event: info`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub v: u32,
    pub hostname: String,
    pub cpu: CpuInfo,
    pub ram: RamInfo,
    pub gpu: Option<GpuInfo>,
    pub interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub name: String,
    pub logical_cores: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RamInfo {
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vram_total: u64,
}

/// Sent every tick as `event: sample`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub ts: u64,
    pub cpu: CpuSample,
    pub ram: RamSample,
    pub gpu: Option<GpuSample>,
    pub top: Top,
    #[serde(rename = "self")]
    pub self_usage: SelfUsage,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CpuSample {
    pub usage: f32,
    pub cores: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RamSample {
    pub used: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuSample {
    pub usage: f32,
    pub vram_used: u64,
    pub vram_total: u64,
    pub temp_c: Option<f32>,
    pub power_pct: Option<f32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Top {
    pub cpu: Vec<AppEntry>,
    pub mem: Vec<AppEntry>,
    pub gpu: Option<Vec<AppEntry>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub name: String,
    pub count: u32,
    pub cpu: f32,
    pub mem: u64,
    pub gpu: Option<f32>,
    pub vram: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelfUsage {
    pub cpu: f32,
    pub mem: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde_json::{Value, json};

    /// Parse the JSON *text* the client will receive (f32 -> shortest decimal, as sent on the wire).
    fn wire<T: Serialize>(v: &T) -> Value {
        serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap()
    }

    fn entry() -> AppEntry {
        AppEntry { name: "game.exe".into(), count: 1, cpu: 24.1, mem: 6_200_000_000, gpu: Some(97.0), vram: Some(7_100_000_000) }
    }

    #[test]
    fn info_serializes_to_contract_shape() {
        let info = Info {
            v: 1,
            hostname: "Kataru".into(),
            cpu: CpuInfo { name: "AMD Ryzen 5 3600 6-Core Processor".into(), logical_cores: 12 },
            ram: RamInfo { total: 34_281_893_888 },
            gpu: Some(GpuInfo { name: "NVIDIA GeForce RTX 5070".into(), vram_total: 12_523_143_168 }),
            interval_ms: 1000,
        };
        assert_eq!(
            wire(&info),
            json!({
                "v": 1,
                "hostname": "Kataru",
                "cpu": { "name": "AMD Ryzen 5 3600 6-Core Processor", "logicalCores": 12 },
                "ram": { "total": 34_281_893_888u64 },
                "gpu": { "name": "NVIDIA GeForce RTX 5070", "vramTotal": 12_523_143_168u64 },
                "intervalMs": 1000
            })
        );
    }

    #[test]
    fn sample_serializes_to_contract_shape() {
        let s = Sample {
            ts: 1_793_384_000_123,
            cpu: CpuSample { usage: 37.2, cores: vec![12.0, 88.5] },
            ram: RamSample { used: 17_179_869_184, total: 34_359_738_368 },
            gpu: Some(GpuSample { usage: 98.0, vram_used: 9_126_805_504, vram_total: 12_884_901_888, temp_c: Some(71.0), power_pct: Some(84.9) }),
            top: Top { cpu: vec![entry()], mem: vec![], gpu: Some(vec![entry()]) },
            self_usage: SelfUsage { cpu: 0.2, mem: 9_437_184 },
        };
        let e = json!({ "name": "game.exe", "count": 1, "cpu": 24.1, "mem": 6_200_000_000u64, "gpu": 97.0, "vram": 7_100_000_000u64 });
        assert_eq!(
            wire(&s),
            json!({
                "ts": 1_793_384_000_123u64,
                "cpu": { "usage": 37.2, "cores": [12.0, 88.5] },
                "ram": { "used": 17_179_869_184u64, "total": 34_359_738_368u64 },
                "gpu": { "usage": 98.0, "vramUsed": 9_126_805_504u64, "vramTotal": 12_884_901_888u64, "tempC": 71.0, "powerPct": 84.9 },
                "top": { "cpu": [e.clone()], "mem": [], "gpu": [e] },
                "self": { "cpu": 0.2, "mem": 9_437_184 }
            })
        );
    }

    #[test]
    fn f32_values_serialize_without_float_noise() {
        let s = serde_json::to_string(&CpuSample { usage: 37.2, cores: vec![0.1] }).unwrap();
        assert_eq!(s, r#"{"usage":37.2,"cores":[0.1]}"#);
    }

    #[test]
    fn unavailable_gpu_serializes_as_null() {
        let t = Top { cpu: vec![], mem: vec![], gpu: None };
        assert_eq!(wire(&t), json!({ "cpu": [], "mem": [], "gpu": null }));
        let a = AppEntry { gpu: None, vram: None, ..entry() };
        let v = wire(&a);
        assert!(v["gpu"].is_null() && v["vram"].is_null());
    }
}
