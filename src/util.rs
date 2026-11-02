use std::net::Ipv4Addr;
use std::time::Duration;

use tokio::process::Command;

/// Runs a command and captures its output, failing on a non-zero exit code.
pub async fn run_command(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<String, String> {
    let output = run_command_raw(program, args, timeout).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let detail = if !stderr.is_empty() { stderr } else { stdout };

        return Err(if detail.is_empty() {
            format!("`{program}` failed ({})", output.status)
        } else {
            detail
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Runs a command and captures its output, regardless of the exit code.
pub async fn run_command_raw(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<std::process::Output, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(format!("Failed to run `{program}`: {error}")),
        Err(_) => Err(format!(
            "`{program}` did not respond in time ({} s)",
            timeout.as_secs_f32()
        )),
    }
}

pub fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("YANT/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("HTTP client: {error}"))
}

pub fn format_bytes(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes.max(0.0);
    let mut unit = 0;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 || value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

pub fn format_rate(bytes_per_second: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_second))
}

pub fn format_mbps(mbps: f64) -> String {
    if mbps >= 1000.0 {
        format!("{:.2} Gbit/s", mbps / 1000.0)
    } else if mbps >= 100.0 {
        format!("{mbps:.0} Mbit/s")
    } else if mbps >= 10.0 {
        format!("{mbps:.1} Mbit/s")
    } else {
        format!("{mbps:.2} Mbit/s")
    }
}

pub fn format_latency(ms: f64) -> String {
    if ms >= 1000.0 {
        format!("{:.2} s", ms / 1000.0)
    } else if ms >= 10.0 {
        format!("{ms:.0} ms")
    } else {
        format!("{ms:.1} ms")
    }
}

/// Converts an RSSI value in dBm to an approximate link quality percentage.
pub fn quality_from_dbm(dbm: i32) -> u8 {
    ((2 * (dbm + 100)).clamp(0, 100)) as u8
}

pub fn quality_from_signal_percent(signal: u8) -> u8 {
    signal.min(100)
}

pub fn quality_label(quality: u8) -> &'static str {
    match quality {
        80..=100 => "Excellent",
        60..=79 => "Good",
        40..=59 => "Fair",
        20..=39 => "Poor",
        _ => "Very poor",
    }
}

pub fn frequency_to_channel(frequency_mhz: u32) -> Option<u32> {
    match frequency_mhz {
        2412..=2484 => Some((frequency_mhz - 2407) / 5),
        5000..=5925 => Some((frequency_mhz - 5000) / 5),
        5955..=7115 => Some((frequency_mhz - 5950) / 5),
        _ => None,
    }
}

pub fn band_label(frequency_mhz: u32) -> &'static str {
    match frequency_mhz {
        2400..=2500 => "2.4 GHz",
        4900..=5924 => "5 GHz",
        5955..=7125 => "6 GHz",
        _ => "Unknown",
    }
}

pub fn normalize_mac(mac: &str) -> String {
    mac.trim().to_ascii_uppercase()
}

/// Parses a port specification such as `"22,80,443,8000-8100"`.
pub fn parse_ports(spec: &str) -> Result<Vec<u16>, String> {
    let mut ports = Vec::new();

    for part in spec
        .split([',', ';', ' '])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        if let Some((start, end)) = part.split_once('-') {
            let start: u32 = start
                .trim()
                .parse()
                .map_err(|_| format!("Invalid port: {start}"))?;
            let end: u32 = end
                .trim()
                .parse()
                .map_err(|_| format!("Invalid port: {end}"))?;

            if start == 0 || end > 65_535 || start > end {
                return Err(format!("Invalid port range: {part}"));
            }

            if end - start > 4096 {
                return Err("Range too large (4096 ports maximum)".to_owned());
            }

            for port in start..=end {
                ports.push(port as u16);
            }
        } else {
            let port: u32 = part.parse().map_err(|_| format!("Invalid port: {part}"))?;

            if port == 0 || port > 65_535 {
                return Err(format!("Invalid port: {port}"));
            }

            ports.push(port as u16);
        }
    }

    ports.sort_unstable();
    ports.dedup();

    if ports.is_empty() {
        return Err("No ports to scan".to_owned());
    }

    Ok(ports)
}

/// Parses a CIDR notation (`192.168.1.42/24`) and normalises the network address.
pub fn parse_subnet(value: &str) -> Result<(Ipv4Addr, u8), String> {
    let value = value.trim();

    let (address, prefix) = value
        .split_once('/')
        .ok_or_else(|| "Expected format: 192.168.1.0/24".to_owned())?;

    let address: Ipv4Addr = address
        .parse()
        .map_err(|_| format!("Invalid IPv4 address: {address}"))?;

    let prefix: u8 = prefix
        .parse()
        .map_err(|_| format!("Invalid prefix: {prefix}"))?;

    if !(16..=32).contains(&prefix) {
        return Err("Prefix must be between /16 and /32".to_owned());
    }

    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };

    Ok((Ipv4Addr::from(u32::from(address) & mask), prefix))
}

pub fn network_cidr(address: Ipv4Addr, netmask: Ipv4Addr) -> (Ipv4Addr, u8) {
    let mask = u32::from(netmask);
    let prefix = mask.count_ones() as u8;
    (Ipv4Addr::from(u32::from(address) & mask), prefix)
}

/// Enumerates usable hosts in a subnet. Excludes network/broadcast for
/// prefixes of /30 or shorter. Caps the result at 4094 hosts.
pub fn hosts_in_subnet(network: Ipv4Addr, prefix: u8) -> Vec<Ipv4Addr> {
    if prefix >= 31 {
        return vec![network];
    }

    if prefix < 20 {
        return Vec::new();
    }

    let base = u32::from(network);
    let count = 1u32 << (32 - prefix);
    let first = base + 1;
    let last = base + count - 1;

    (first..last).take(4094).map(Ipv4Addr::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_port_specifications() {
        assert_eq!(
            parse_ports("80,443,8000-8002").unwrap(),
            vec![80, 443, 8000, 8001, 8002]
        );
        assert!(parse_ports("0").is_err());
        assert!(parse_ports("100-50").is_err());
        assert!(parse_ports("").is_err());
    }

    #[test]
    fn normalises_subnets() {
        let (network, prefix) = parse_subnet("192.168.1.42/24").unwrap();
        assert_eq!(network, Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(prefix, 24);
        assert_eq!(hosts_in_subnet(network, prefix).len(), 254);
    }

    #[test]
    fn derives_channels() {
        assert_eq!(frequency_to_channel(2412), Some(1));
        assert_eq!(frequency_to_channel(5180), Some(36));
        assert_eq!(frequency_to_channel(5955), Some(1));
    }

    #[test]
    fn formats_values() {
        assert_eq!(format_bytes(512.0), "512 B");
        assert_eq!(format_bytes(1024.0), "1.00 KB");
        assert_eq!(format_mbps(940.0), "940 Mbit/s");
    }
}
