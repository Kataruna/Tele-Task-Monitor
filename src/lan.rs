//! LAN addresses the dashboard can be opened from (shown in the tray menu and on start-up).

/// `(url, interface_name)` pairs for every non-loopback, non-link-local IPv4 address,
/// e.g. `("http://192.168.1.50:8800/", "Wi-Fi")`.
pub fn collect_lan_urls(port: u16) -> Vec<(String, String)> {
    let addrs = if_addrs::get_if_addrs().unwrap_or_default();
    let mut out = Vec::new();
    for a in addrs.iter().filter(|a| !a.is_loopback()) {
        if let std::net::IpAddr::V4(ip) = a.ip() {
            if ip.is_link_local() {
                continue;
            }
            out.push((format!("http://{ip}:{port}/"), a.name.clone()));
        }
    }
    out
}
