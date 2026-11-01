//! Command-line flags. Hand-rolled (no clap) to keep the binary small.

use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "Usage: tele-task-monitor [--port <u16>] [--bind <ip>] [--web-dir <path>]
  --port     TCP port to listen on (default 8800)
  --bind     IP address to listen on (default 0.0.0.0 = all interfaces)
  --web-dir  Folder with index.html (default: 'web' next to the exe, else ./web)";

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub port: u16,
    pub bind: IpAddr,
    /// `None` = use the default lookup (see `resolve_web_dir`).
    pub web_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self { port: 8800, bind: IpAddr::from([0, 0, 0, 0]), web_dir: None }
    }
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Config, String> {
    let mut cfg = Config::default();
    let mut it = args.into_iter();
    while let Some(flag) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--port" => {
                let v = value("--port")?;
                cfg.port = v.parse().map_err(|_| format!("invalid --port '{v}'"))?;
            }
            "--bind" => {
                let v = value("--bind")?;
                cfg.bind = v.parse().map_err(|_| format!("invalid --bind '{v}'"))?;
            }
            "--web-dir" => cfg.web_dir = Some(PathBuf::from(value("--web-dir")?)),
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok(cfg)
}

/// Explicit flag wins; otherwise `<exe dir>/web`, then `<exe dir>/../Resources/web` (inside a
/// macOS `.app` bundle) if either exists; otherwise `./web`.
pub fn resolve_web_dir(explicit: Option<PathBuf>, exe_dir: Option<&Path>) -> PathBuf {
    if let Some(p) = explicit {
        return p;
    }
    if let Some(dir) = exe_dir {
        for candidate in [dir.join("web"), dir.join("../Resources/web")] {
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    PathBuf::from("web")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use std::path::PathBuf;

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn defaults() {
        assert_eq!(parse(args(&[])).unwrap(), Config::default());
        assert_eq!(Config::default().port, 8800);
    }

    #[test]
    fn parses_all_flags() {
        let c = parse(args(&["--port", "9000", "--bind", "192.168.1.122", "--web-dir", "D:\\ui"])).unwrap();
        assert_eq!(c.port, 9000);
        assert_eq!(c.bind, "192.168.1.122".parse::<IpAddr>().unwrap());
        assert_eq!(c.web_dir, Some(PathBuf::from("D:\\ui")));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(args(&["--port", "70000"])).unwrap_err().contains("--port"));
        assert!(parse(args(&["--port"])).unwrap_err().contains("needs a value"));
        assert!(parse(args(&["--bind", "nope"])).unwrap_err().contains("--bind"));
        assert!(parse(args(&["--frobnicate"])).unwrap_err().contains("unknown"));
        assert_eq!(parse(args(&["--help"])).unwrap_err(), "");
    }

    #[test]
    fn web_dir_resolution() {
        assert_eq!(resolve_web_dir(Some("X".into()), None), PathBuf::from("X"));
        let exe_dir = std::env::temp_dir().join(format!("ttm-exe-{}", std::process::id()));
        std::fs::create_dir_all(exe_dir.join("web")).unwrap();
        assert_eq!(resolve_web_dir(None, Some(&exe_dir)), exe_dir.join("web"));
        let empty = std::env::temp_dir().join(format!("ttm-empty-{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(resolve_web_dir(None, Some(&empty)), PathBuf::from("web"));
    }
}
