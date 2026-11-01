//! Rede utilitária (mesh frota Wi‑Fi + wired na mesma LAN).
//!
//! Calcula broadcast da subnet da iface com rota default — evita
//! `ENETUNREACH` com `255.255.255.255` sem rota, e mistura notes Wi‑Fi
//! com PC AMD em cabo no mesmo router.

/// Broadcast IPv4 da iface com rota default (`A.B.C.255` típico /24).
/// `None` em Windows/lab sem `ip`, ou se não houver rota.
pub fn default_iface_broadcast() -> Option<String> {
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
    #[cfg(target_os = "linux")]
    {
        linux_default_broadcast()
    }
}

#[cfg(target_os = "linux")]
fn linux_default_broadcast() -> Option<String> {
    let iface = default_iface_from_proc()?;
    let out = std::process::Command::new("ip")
        .args(["-4", "-o", "addr", "show", "dev", &iface])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // Ex: `2: wlan0    inet 192.168.1.42/24 brd 192.168.1.255 ...`
    for tok in text.split_whitespace() {
        if let Some(rest) = tok.strip_prefix("brd") {
            // raramente colado; normalmente token seguinte
            let _ = rest;
        }
    }
    let parts: Vec<&str> = text.split_whitespace().collect();
    for i in 0..parts.len() {
        if parts[i] == "brd" {
            if let Some(b) = parts.get(i + 1) {
                if b.parse::<std::net::Ipv4Addr>().is_ok() {
                    return Some((*b).to_string());
                }
            }
        }
        if parts[i] == "inet" {
            if let Some(cidr) = parts.get(i + 1) {
                if let Some(bc) = broadcast_from_cidr(cidr) {
                    return Some(bc);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn default_iface_from_proc() -> Option<String> {
    let raw = std::fs::read_to_string("/proc/net/route").ok()?;
    for line in raw.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Iface Destination Gateway Flags …
        if cols.len() < 2 {
            continue;
        }
        if cols[1] == "00000000" {
            return Some(cols[0].to_string());
        }
    }
    None
}

/// `192.168.1.42/24` → `192.168.1.255`
pub fn broadcast_from_cidr(cidr: &str) -> Option<String> {
    let (ip_s, pref_s) = cidr.split_once('/')?;
    let ip: std::net::Ipv4Addr = ip_s.parse().ok()?;
    let pref: u32 = pref_s.parse().ok()?;
    if pref > 32 {
        return None;
    }
    let ip_u = u32::from(ip);
    let mask = if pref == 0 {
        0
    } else {
        (!0u32) << (32 - pref)
    };
    let bc = std::net::Ipv4Addr::from(ip_u | !mask);
    Some(bc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_24() {
        assert_eq!(
            broadcast_from_cidr("192.168.50.10/24").as_deref(),
            Some("192.168.50.255")
        );
    }

    #[test]
    fn cidr_16() {
        assert_eq!(
            broadcast_from_cidr("10.1.2.3/16").as_deref(),
            Some("10.1.255.255")
        );
    }
}
