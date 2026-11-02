use std::time::Duration;

use crate::net::interfaces;
use crate::util::{
    band_label, frequency_to_channel, quality_from_dbm, quality_from_signal_percent, run_command,
};

#[derive(Debug, Clone, Default)]
pub struct WifiInfo {
    pub interface: Option<String>,
    pub connected: bool,
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    pub signal_dbm: Option<i32>,
    pub signal_percent: Option<u8>,
    pub noise_dbm: Option<i32>,
    pub quality: Option<u8>,
    pub frequency_mhz: Option<u32>,
    pub channel: Option<u32>,
    pub bitrate_mbps: Option<f64>,
    pub tx_power_dbm: Option<f32>,
    pub security: Option<String>,
    pub note: Option<String>,
}

impl WifiInfo {
    pub fn band(&self) -> &'static str {
        self.frequency_mhz.map(band_label).unwrap_or("Unknown")
    }

    pub fn quality_or_default(&self) -> u8 {
        self.quality
            .or_else(|| self.signal_dbm.map(quality_from_dbm))
            .or(self.signal_percent)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct WifiNetwork {
    pub ssid: String,
    pub bssid: Option<String>,
    pub signal_dbm: Option<i32>,
    pub signal_percent: Option<u8>,
    pub quality: u8,
    pub channel: Option<u32>,
    pub frequency_mhz: Option<u32>,
    pub security: String,
    pub in_use: bool,
}

impl WifiNetwork {
    pub fn band(&self) -> &'static str {
        self.frequency_mhz.map(band_label).unwrap_or("Unknown")
    }
}

pub async fn fetch(interface: Option<String>) -> Result<WifiInfo, String> {
    let available = interfaces::wireless_interfaces();

    let interface = match interface {
        Some(interface) if available.contains(&interface) => interface,
        _ => available
            .first()
            .cloned()
            .ok_or_else(|| "No Wi-Fi interface detected on this machine".to_owned())?,
    };

    let mut info = WifiInfo {
        interface: Some(interface.clone()),
        ..Default::default()
    };

    let mut iw_available = false;
    let mut nmcli_available = false;
    let mut last_error = None;

    match run_command("iw", &["dev", &interface, "link"], Duration::from_secs(4)).await {
        Ok(output) => {
            iw_available = true;
            parse_iw_link(&output, &mut info);
        }
        Err(error) => last_error = Some(error),
    }

    if let Ok(content) = std::fs::read_to_string("/proc/net/wireless") {
        merge_proc_wireless(&content, &interface, &mut info);
    }

    if let Ok(output) =
        run_command("iw", &["dev", &interface, "info"], Duration::from_secs(4)).await
    {
        iw_available = true;
        parse_iw_info(&output, &mut info);
    }

    match run_command(
        "nmcli",
        &[
            "-t",
            "-f",
            "IN-USE,SSID,BSSID,CHAN,FREQ,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
            "ifname",
            &interface,
        ],
        Duration::from_secs(8),
    )
    .await
    {
        Ok(output) => {
            nmcli_available = true;
            let networks = parse_nmcli_list(&output);

            if let Some(active) = networks.iter().find(|network| network.in_use) {
                info.connected = true;

                if info.ssid.is_none() {
                    info.ssid = Some(active.ssid.clone());
                }
                if info.bssid.is_none() {
                    info.bssid = active.bssid.clone();
                }
                if info.signal_dbm.is_none() {
                    info.signal_dbm = active.signal_dbm;
                }
                if info.signal_percent.is_none() {
                    info.signal_percent = active.signal_percent;
                }
                if info.quality.is_none() {
                    info.quality = Some(active.quality);
                }
                if info.frequency_mhz.is_none() {
                    info.frequency_mhz = active.frequency_mhz;
                }
                if info.channel.is_none() {
                    info.channel = active.channel;
                }
                if info.security.is_none() {
                    info.security = Some(active.security.clone());
                }
            }
        }
        Err(error) => {
            if last_error.is_none() {
                last_error = Some(error);
            }
        }
    }

    if info.quality.is_none() {
        info.quality = info
            .signal_dbm
            .map(quality_from_dbm)
            .or(info.signal_percent);
    }

    if info.channel.is_none() {
        info.channel = info.frequency_mhz.and_then(frequency_to_channel);
    }

    if info.signal_dbm.is_none() {
        info.signal_dbm = info
            .signal_percent
            .map(|percent| i32::from(percent) / 2 - 100);
    }

    if !iw_available && !nmcli_available {
        return Err(format!(
            "Unable to read Wi-Fi information. {}",
            last_error.unwrap_or_else(|| "Install `iw` or NetworkManager (`nmcli`).".to_owned())
        ));
    }

    if !nmcli_available {
        info.note =
            Some("`nmcli` is unavailable: some information (security) may be missing.".to_owned());
    }

    Ok(info)
}

pub async fn scan(interface: Option<String>) -> Result<Vec<WifiNetwork>, String> {
    let available = interfaces::wireless_interfaces();

    let interface = match interface {
        Some(interface) if available.contains(&interface) => interface,
        _ => available
            .first()
            .cloned()
            .ok_or_else(|| "No Wi-Fi interface detected".to_owned())?,
    };

    let mut nmcli_error = None;

    // NetworkManager is the most reliable unprivileged option.
    let nmcli_args = [
        "-t",
        "-f",
        "IN-USE,SSID,BSSID,CHAN,FREQ,SIGNAL,SECURITY",
        "device",
        "wifi",
        "list",
        "ifname",
        &interface,
        "--rescan",
        "yes",
    ];

    match run_command("nmcli", &nmcli_args, Duration::from_secs(25)).await {
        Ok(output) => {
            let mut networks = parse_nmcli_list(&output);

            if !networks.is_empty() {
                sort_networks(&mut networks);
                return Ok(networks);
            }
        }
        Err(error) => {
            nmcli_error = Some(error);

            // Older NetworkManager versions may not accept `--rescan`.
            if let Ok(output) = run_command(
                "nmcli",
                &nmcli_args[..nmcli_args.len() - 2],
                Duration::from_secs(20),
            )
            .await
            {
                let mut networks = parse_nmcli_list(&output);

                if !networks.is_empty() {
                    sort_networks(&mut networks);
                    return Ok(networks);
                }
            }
        }
    }

    let iw_result = run_command("iw", &["dev", &interface, "scan"], Duration::from_secs(25)).await;

    match iw_result {
        Ok(output) => {
            let mut networks = parse_iw_scan(&output);
            sort_networks(&mut networks);
            Ok(networks)
        }
        Err(iw_error) => {
            let nmcli_detail = nmcli_error.unwrap_or_else(|| "no response".to_owned());

            Err(format!(
                "Wi-Fi scan failed.\n\
                 NetworkManager: {nmcli_detail}\n\
                 iw: {iw_error}\n\n\
                 Note: `iw scan` requires root privileges. Use NetworkManager \
                 or run YANT with the required privileges."
            ))
        }
    }
}

fn sort_networks(networks: &mut [WifiNetwork]) {
    networks.sort_by(|a, b| {
        b.in_use
            .cmp(&a.in_use)
            .then_with(|| b.quality.cmp(&a.quality))
            .then_with(|| a.ssid.cmp(&b.ssid))
    });
}

fn parse_iw_link(output: &str, info: &mut WifiInfo) {
    for line in output.lines() {
        let line = line.trim();

        if line.starts_with("Not connected") {
            info.connected = false;
            continue;
        }

        if let Some(rest) = line.strip_prefix("Connected to ") {
            info.connected = true;
            info.bssid = rest.split_whitespace().next().map(str::to_ascii_uppercase);
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim();

        match key {
            "SSID" => {
                if !value.is_empty() {
                    info.ssid = Some(value.to_owned());
                    info.connected = true;
                }
            }
            "freq" => {
                info.frequency_mhz = value.parse().ok();
            }
            "signal" => {
                info.signal_dbm = value
                    .split_whitespace()
                    .next()
                    .and_then(|number| number.parse::<f64>().ok())
                    .map(|dbm| dbm.round() as i32);
            }
            "tx bitrate" | "rx bitrate" => {
                if let Some(bitrate) = value
                    .split_whitespace()
                    .next()
                    .and_then(|number| number.parse::<f64>().ok())
                {
                    info.bitrate_mbps = Some(
                        info.bitrate_mbps
                            .map_or(bitrate, |current| current.max(bitrate)),
                    );
                }
            }
            "bssid" => info.bssid = Some(value.to_ascii_uppercase()),
            _ => {}
        }
    }
}

fn parse_iw_info(output: &str, info: &mut WifiInfo) {
    for line in output.lines() {
        let line = line.trim();

        if let Some(value) = line.strip_prefix("txpower") {
            info.tx_power_dbm = value
                .split_whitespace()
                .next()
                .and_then(|number| number.parse().ok());
        }
    }
}

fn merge_proc_wireless(content: &str, interface: &str, info: &mut WifiInfo) {
    for line in content.lines() {
        let Some((name, values)) = line.split_once(':') else {
            continue;
        };

        if name.trim() != interface {
            continue;
        }

        let fields: Vec<&str> = values.split_whitespace().collect();

        if fields.len() < 4 {
            continue;
        }

        let quality = parse_float(fields[1]);
        let level = parse_float(fields[2]);
        let noise = parse_float(fields[3]);

        if let Some(quality) = quality {
            let percent = if quality <= 100.0 {
                (quality / 70.0 * 100.0).clamp(0.0, 100.0)
            } else {
                quality
            };

            info.signal_percent = Some(percent.round() as u8);
            if info.quality.is_none() {
                info.quality = Some(percent.round() as u8);
            }
        }

        if info.signal_dbm.is_none()
            && let Some(level) = level
            && level < 0.0
        {
            info.signal_dbm = Some(level.round() as i32);
        }

        if info.noise_dbm.is_none()
            && let Some(noise) = noise
            && noise < 0.0
            && noise > -150.0
        {
            info.noise_dbm = Some(noise.round() as i32);
        }
    }
}

fn parse_float(value: &str) -> Option<f64> {
    value.trim_end_matches('.').parse().ok()
}

fn split_nmcli(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut escaped = false;

    for character in line.chars() {
        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == ':' {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(character);
        }
    }

    fields.push(current);
    fields
}

fn parse_nmcli_list(output: &str) -> Vec<WifiNetwork> {
    let mut networks = Vec::new();

    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let fields = split_nmcli(line);

        if fields.len() < 7 {
            continue;
        }

        let in_use = {
            let value = fields[0].trim().to_ascii_lowercase();
            value == "*" || value == "yes" || value == "oui" || value == "1"
        };

        let ssid_raw = fields[1].trim();

        if ssid_raw.is_empty() {
            continue;
        }

        let ssid = if ssid_raw == "--" {
            "(hidden)".to_owned()
        } else {
            ssid_raw.to_owned()
        };

        let bssid = if fields[2].trim().is_empty() {
            None
        } else {
            Some(fields[2].trim().to_ascii_uppercase())
        };

        let channel = fields[3].trim().parse().ok();
        let frequency_mhz = fields[4]
            .split_whitespace()
            .next()
            .and_then(|value| value.parse().ok());
        let signal_percent = fields[5].trim().parse::<u8>().ok();
        let signal_dbm = signal_percent.map(|percent| i32::from(percent) / 2 - 100);
        let quality = signal_percent
            .map(quality_from_signal_percent)
            .or_else(|| signal_dbm.map(quality_from_dbm))
            .unwrap_or(0);
        let security = {
            let value = fields[6].trim();
            if value.is_empty() { "Open" } else { value }.to_owned()
        };

        networks.push(WifiNetwork {
            ssid,
            bssid,
            signal_dbm,
            signal_percent,
            quality,
            channel,
            frequency_mhz,
            security,
            in_use,
        });
    }

    networks
}

fn parse_iw_scan(output: &str) -> Vec<WifiNetwork> {
    let mut networks = Vec::new();
    let mut current: Option<WifiNetwork> = None;

    for line in output.lines() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("BSS ") {
            if let Some(network) = current.take() {
                networks.push(network);
            }

            let bssid = rest
                .split_whitespace()
                .next()
                .filter(|value| value.contains(':'))
                .map(str::to_ascii_uppercase);

            current = Some(WifiNetwork {
                bssid,
                ssid: "(hidden)".to_owned(),
                ..Default::default()
            });

            continue;
        }

        let Some(network) = current.as_mut() else {
            continue;
        };

        if let Some(value) = trimmed.strip_prefix("freq:") {
            network.frequency_mhz = value.trim().parse().ok();
        } else if let Some(value) = trimmed.strip_prefix("signal:") {
            let dbm = value
                .split_whitespace()
                .next()
                .and_then(|number| number.parse::<f64>().ok())
                .map(|dbm| dbm.round() as i32);

            network.signal_dbm = dbm;
            network.quality = dbm.map(quality_from_dbm).unwrap_or(0);
        } else if let Some(value) = trimmed.strip_prefix("SSID:") {
            let ssid = value.trim();
            if !ssid.is_empty() {
                network.ssid = ssid.to_owned();
            }
        } else if trimmed.starts_with("RSN:") {
            network.security = "WPA2/WPA3".to_owned();
        } else if trimmed.starts_with("WPA:") {
            if network.security.is_empty() {
                network.security = "WPA".to_owned();
            }
        } else if trimmed.starts_with("capability:")
            && network.security.is_empty()
            && trimmed.contains("Privacy")
        {
            network.security = "WEP".to_owned();
        }
    }

    if let Some(network) = current {
        networks.push(network);
    }

    for network in &mut networks {
        if network.security.is_empty() {
            network.security = "Open".to_owned();
        }
        if network.channel.is_none() {
            network.channel = network.frequency_mhz.and_then(frequency_to_channel);
        }
    }

    networks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iw_link() {
        let output = "\
Connected to AA:BB:CC:DD:EE:FF (on wlan0)
\tSSID: Maison
\tfreq: 5180
\tsignal: -52 dBm
\ttx bitrate: 866.7 MBit/s
\trx bitrate: 780.0 MBit/s
";
        let mut info = WifiInfo::default();
        parse_iw_link(output, &mut info);

        assert!(info.connected);
        assert_eq!(info.ssid.as_deref(), Some("Maison"));
        assert_eq!(info.signal_dbm, Some(-52));
        assert_eq!(info.frequency_mhz, Some(5180));
        assert_eq!(info.bitrate_mbps, Some(866.7));
    }

    #[test]
    fn splits_escaped_nmcli_fields() {
        let fields = split_nmcli("*:Mon reseau:AA\\:BB\\:CC\\:DD\\:EE\\:FF:36:5180 MHz:87:WPA2");
        assert_eq!(fields[0], "*");
        assert_eq!(fields[1], "Mon reseau");
        assert_eq!(fields[2], "AA:BB:CC:DD:EE:FF");
        assert_eq!(fields[6], "WPA2");
    }

    #[test]
    fn parses_nmcli_list() {
        let output = "*:Maison:AA\\:BB\\:CC\\:DD\\:EE\\:FF:36:5180 MHz:87:WPA2\n:Voisin:11\\:22\\:33\\:44\\:55\\:66:1:2412 MHz:42:WPA1 WPA2\n";
        let networks = parse_nmcli_list(output);

        assert_eq!(networks.len(), 2);
        assert!(networks[0].in_use);
        assert_eq!(networks[0].quality, 87);
        assert_eq!(networks[1].channel, Some(1));
    }

    #[test]
    fn parses_iw_scan() {
        let output = "\
BSS AA:BB:CC:DD:EE:FF (on wlan0)
\tfreq: 2437
\tsignal: -61.00 dBm
\tcapability: ESS Privacy
\tSSID: Bureau
\tRSN: * Version: 1
";
        let networks = parse_iw_scan(output);

        assert_eq!(networks.len(), 1);
        assert_eq!(networks[0].ssid, "Bureau");
        assert_eq!(networks[0].quality, 78);
        assert_eq!(networks[0].security, "WPA2/WPA3");
    }
}
