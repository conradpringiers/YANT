use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};

#[derive(Debug, Clone, Default)]
pub struct InterfaceInfo {
    pub name: String,
    pub mac: Option<String>,
    pub ipv4: Option<Ipv4Addr>,
    pub netmask: Option<Ipv4Addr>,
    pub broadcast: Option<Ipv4Addr>,
    pub ipv6: Vec<Ipv6Addr>,
    pub gateway: Option<Ipv4Addr>,
    pub dns: Vec<String>,
    pub is_wifi: bool,
    pub is_up: bool,
    pub link_speed_mbps: Option<u64>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

impl InterfaceInfo {
    pub fn kind(&self) -> &'static str {
        if self.is_wifi {
            "Wi-Fi"
        } else if self.name == "lo" {
            "Loopback"
        } else if self.name.starts_with("eth")
            || self.name.starts_with("en")
            || self.name.starts_with("eno")
            || self.name.starts_with("enp")
        {
            "Ethernet"
        } else if self.name.starts_with("tun")
            || self.name.starts_with("wg")
            || self.name.starts_with("tap")
        {
            "VPN"
        } else {
            "Network"
        }
    }

    pub fn cidr(&self) -> Option<String> {
        let address = self.ipv4?;
        let mask = self.netmask?;
        let (network, prefix) = crate::util::network_cidr(address, mask);
        Some(format!("{network}/{prefix}"))
    }
}

/// Refreshes the list of network interfaces.
pub async fn fetch() -> Result<Vec<InterfaceInfo>, String> {
    match tokio::task::spawn_blocking(fetch_blocking).await {
        Ok(result) => result,
        Err(error) => Err(format!("Interfaces : {error}")),
    }
}

fn fetch_blocking() -> Result<Vec<InterfaceInfo>, String> {
    let raw =
        NetworkInterface::show().map_err(|error| format!("Failed to list interfaces: {error}"))?;
    let counters = read_counters();
    let gateways = read_gateways();
    let dns = read_dns_servers();

    let mut interfaces: Vec<InterfaceInfo> = Vec::new();

    for interface in raw {
        if interface.internal {
            continue;
        }

        if !interfaces.iter().any(|info| info.name == interface.name) {
            let (rx_bytes, tx_bytes) = counters.get(&interface.name).copied().unwrap_or((0, 0));

            interfaces.push(InterfaceInfo {
                name: interface.name.clone(),
                mac: interface.mac_addr.clone(),
                is_wifi: is_wireless(&interface.name),
                is_up: is_up(&interface.name),
                link_speed_mbps: link_speed(&interface.name),
                gateway: gateways.get(&interface.name).copied(),
                dns: dns.clone(),
                rx_bytes,
                tx_bytes,
                ..Default::default()
            });
        }

        let info = interfaces
            .iter_mut()
            .find(|info| info.name == interface.name)
            .expect("interface registered above");

        if info.mac.is_none() {
            info.mac = interface.mac_addr.clone();
        }

        for address in interface.addr {
            match address {
                Addr::V4(address) => {
                    if info.ipv4.is_none() {
                        info.ipv4 = Some(address.ip);
                        info.netmask = address.netmask;
                        info.broadcast = address.broadcast;
                    }
                }
                Addr::V6(address) => info.ipv6.push(address.ip),
            }
        }
    }

    interfaces.retain(|info| info.ipv4.is_some() || !info.ipv6.is_empty() || info.is_wifi);

    interfaces.sort_by_key(|info| {
        (
            !info.is_up,
            !info.is_wifi,
            !info.gateway.is_some(),
            info.name.clone(),
        )
    });

    Ok(interfaces)
}

pub fn primary(interfaces: &[InterfaceInfo]) -> Option<&InterfaceInfo> {
    interfaces
        .iter()
        .find(|info| info.gateway.is_some() && info.ipv4.is_some())
        .or_else(|| {
            interfaces
                .iter()
                .find(|info| info.is_up && info.ipv4.is_some())
        })
}

pub fn wireless_interfaces() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        let mut names = Vec::new();

        if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.path().join("wireless").exists() {
                    names.push(name);
                }
            }
        }

        names.sort();
        names
    }

    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

fn is_wireless(name: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new("/sys/class/net")
            .join(name)
            .join("wireless")
            .exists()
    }

    #[cfg(target_os = "macos")]
    {
        name == "en0"
    }

    #[cfg(target_os = "windows")]
    {
        let lowered = name.to_ascii_lowercase();
        lowered.contains("wi-fi") || lowered.contains("wifi") || lowered.contains("wlan")
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        name.starts_with("wl")
    }
}

fn is_up(name: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string(
            std::path::Path::new("/sys/class/net")
                .join(name)
                .join("operstate"),
        )
        .map(|state| {
            let state = state.trim();
            state == "up" || state == "unknown"
        })
        .unwrap_or(true)
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = name;
        true
    }
}

fn link_speed(name: &str) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let value = std::fs::read_to_string(
            std::path::Path::new("/sys/class/net")
                .join(name)
                .join("speed"),
        )
        .ok()?;
        let speed: u64 = value.trim().parse().ok()?;

        if speed == 0 || speed == u64::MAX / 1000 {
            None
        } else {
            Some(speed)
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = name;
        None
    }
}

/// Returns cumulative `(rx, tx)` byte counters per interface.
fn read_counters() -> HashMap<String, (u64, u64)> {
    let mut counters = HashMap::new();

    #[cfg(target_os = "linux")]
    if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
        for line in content.lines().skip(2) {
            let Some((name, values)) = line.split_once(':') else {
                continue;
            };
            let fields: Vec<&str> = values.split_whitespace().collect();

            if fields.len() < 9 {
                continue;
            }

            let rx = fields[0].parse().unwrap_or(0);
            let tx = fields[8].parse().unwrap_or(0);
            counters.insert(name.trim().to_owned(), (rx, tx));
        }
    }

    counters
}

fn read_gateways() -> HashMap<String, Ipv4Addr> {
    let mut gateways: HashMap<String, (u32, Ipv4Addr)> = HashMap::new();

    #[cfg(target_os = "linux")]
    if let Ok(content) = std::fs::read_to_string("/proc/net/route") {
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();

            if fields.len() < 8 || fields[1] != "00000000" {
                continue;
            }

            let Ok(gateway) = u32::from_str_radix(fields[2], 16) else {
                continue;
            };
            let Ok(metric) = fields[6].parse::<u32>() else {
                continue;
            };

            let address = Ipv4Addr::from(gateway.swap_bytes());
            let entry = gateways
                .entry(fields[0].to_owned())
                .or_insert((metric, address));

            if metric < entry.0 {
                *entry = (metric, address);
            }
        }
    }

    gateways
        .into_iter()
        .map(|(name, (_, address))| (name, address))
        .collect()
}

fn read_dns_servers() -> Vec<String> {
    #[cfg(unix)]
    {
        let Ok(content) = std::fs::read_to_string("/etc/resolv.conf") else {
            return Vec::new();
        };

        content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let line = line.strip_prefix("nameserver")?.trim();
                line.split_whitespace()
                    .next()
                    .map(std::string::ToString::to_string)
            })
            .collect()
    }

    #[cfg(not(unix))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_kind_detection() {
        let mut info = InterfaceInfo {
            name: "wlan0".to_owned(),
            is_wifi: true,
            ..Default::default()
        };
        assert_eq!(info.kind(), "Wi-Fi");

        info.name = "eth0".to_owned();
        info.is_wifi = false;
        assert_eq!(info.kind(), "Ethernet");
    }
}
