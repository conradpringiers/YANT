use std::net::Ipv4Addr;

use crate::util::normalize_mac;

#[derive(Debug, Clone)]
pub struct ArpEntry {
    pub ip: Ipv4Addr,
    pub mac: String,
}

/// Reads the kernel ARP cache. Empty on platforms without `/proc/net/arp`.
pub fn table() -> Vec<ArpEntry> {
    #[cfg(target_os = "linux")]
    {
        let Ok(content) = std::fs::read_to_string("/proc/net/arp") else {
            return Vec::new();
        };

        content.lines().skip(1).filter_map(parse_arp_line).collect()
    }

    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

fn parse_arp_line(line: &str) -> Option<ArpEntry> {
    let fields: Vec<&str> = line.split_whitespace().collect();

    if fields.len() < 6 {
        return None;
    }

    let ip = fields[0].parse().ok()?;
    let flags = u32::from_str_radix(fields[2].trim_start_matches("0x"), 16).ok()?;

    // 0x2 means the entry is complete and usable.
    if flags & 0x2 == 0 {
        return None;
    }

    let mac = normalize_mac(fields[3]);

    if mac.is_empty() || mac == "00:00:00:00:00:00" {
        return None;
    }

    Some(ArpEntry { ip, mac })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_arp_entries() {
        let line = "192.168.1.1      0x1         0x2         aa:bb:cc:dd:ee:ff     *        wlan0";
        let entry = parse_arp_line(line).unwrap();
        assert_eq!(entry.ip, Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(entry.mac, "AA:BB:CC:DD:EE:FF");
    }
}
