use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};
use iced::futures::SinkExt;
use iced::futures::stream::Stream;
use tokio::process::Command;

use crate::net::{arp, interfaces};
use crate::util::{hosts_in_subnet, normalize_mac, parse_subnet};

#[derive(Debug, Clone)]
pub struct Device {
    pub ip: Ipv4Addr,
    pub mac: Option<String>,
    pub hostname: Option<String>,
    pub rtt_ms: Option<f64>,
    pub is_gateway: bool,
    pub is_self: bool,
}

#[derive(Debug, Clone)]
pub enum Event {
    Started { subnet: String, total: u32 },
    Progress { done: u32, total: u32, found: usize },
    Found(Device),
    Done { found: usize, duration_s: f64 },
    Error(String),
}

type Sender = iced::futures::channel::mpsc::Sender<Event>;

pub fn stream(subnet: String) -> impl Stream<Item = Event> {
    iced::stream::channel(128, move |mut sender| async move {
        if let Err(error) = run(subnet, &mut sender).await {
            let _ = sender.send(Event::Error(error)).await;
        }
    })
}

async fn run(subnet: String, sender: &mut Sender) -> Result<(), String> {
    let (network, prefix) = parse_subnet(&subnet)?;

    if prefix < 20 {
        return Err("Subnet is too large. Use a prefix of /20 or longer \
             (2048 hosts maximum)."
            .to_owned());
    }

    let hosts = hosts_in_subnet(network, prefix);
    let total = hosts.len() as u32;

    if total == 0 {
        return Err("No hosts to scan in this subnet".to_owned());
    }

    let local = interfaces::fetch().await.unwrap_or_default();
    let self_ips: HashSet<Ipv4Addr> = local.iter().filter_map(|info| info.ipv4).collect();
    let gateway = local.iter().find_map(|info| info.gateway);
    let initial_arp: HashMap<Ipv4Addr, String> = arp::table()
        .into_iter()
        .map(|entry| (entry.ip, entry.mac))
        .collect();

    let _ = sender
        .send(Event::Started {
            subnet: format!("{network}/{prefix}"),
            total,
        })
        .await;

    let started = Instant::now();
    let found_ips = Arc::new(Mutex::new(HashSet::new()));
    let found_count = Arc::new(AtomicUsize::new(0));
    let done_count = Arc::new(AtomicU32::new(0));
    let arp_map = Arc::new(initial_arp);
    let self_ips = Arc::new(self_ips);
    let probe_sender = sender.clone();

    let mut probes = stream::iter(hosts)
        .map(|ip| {
            let mut sender = probe_sender.clone();
            let found_ips = found_ips.clone();
            let found_count = found_count.clone();
            let done_count = done_count.clone();
            let arp_map = arp_map.clone();
            let self_ips = self_ips.clone();

            async move {
                let rtt = probe(ip).await;
                let done = done_count.fetch_add(1, Ordering::Relaxed) + 1;

                if let Some(rtt) = rtt {
                    let seen = {
                        let mut found = found_ips.lock().expect("scan mutex");
                        found.insert(ip)
                    };

                    if seen {
                        let hostname = resolve_hostname(ip).await;
                        let mac = arp_map.get(&ip).cloned();
                        let count = found_count.fetch_add(1, Ordering::Relaxed) + 1;
                        let is_self = self_ips.contains(&ip);
                        let is_gateway = gateway == Some(ip);

                        let _ = sender
                            .send(Event::Found(Device {
                                ip,
                                mac,
                                hostname,
                                rtt_ms: Some(rtt),
                                is_gateway,
                                is_self,
                            }))
                            .await;

                        let _ = sender
                            .send(Event::Progress {
                                done,
                                total,
                                found: count,
                            })
                            .await;

                        return;
                    }
                }

                let _ = sender
                    .send(Event::Progress {
                        done,
                        total,
                        found: found_count.load(Ordering::Relaxed),
                    })
                    .await;
            }
        })
        .buffer_unordered(48);

    while probes.next().await.is_some() {}

    // The ARP cache reveals devices that block ICMP (phones, printers, IoT).
    for entry in arp::table() {
        if !same_subnet(entry.ip, network, prefix) {
            continue;
        }

        let seen = {
            let mut found = found_ips.lock().expect("scan mutex");
            found.insert(entry.ip)
        };

        if !seen {
            found_count.fetch_add(1, Ordering::Relaxed);
        }

        let _ = sender
            .send(Event::Found(Device {
                ip: entry.ip,
                mac: Some(normalize_mac(&entry.mac)),
                hostname: None,
                rtt_ms: None,
                is_gateway: gateway == Some(entry.ip),
                is_self: self_ips.contains(&entry.ip),
            }))
            .await;
    }

    let found = found_count.load(Ordering::Relaxed);

    let _ = sender
        .send(Event::Done {
            found,
            duration_s: started.elapsed().as_secs_f64(),
        })
        .await;

    Ok(())
}

async fn probe(ip: Ipv4Addr) -> Option<f64> {
    #[cfg(target_os = "windows")]
    let args = [
        "-n".to_owned(),
        "1".to_owned(),
        "-w".to_owned(),
        "900".to_owned(),
        ip.to_string(),
    ];

    #[cfg(target_os = "macos")]
    let args = [
        "-n".to_owned(),
        "-c".to_owned(),
        "1".to_owned(),
        "-W".to_owned(),
        "900".to_owned(),
        ip.to_string(),
    ];

    #[cfg(all(unix, not(target_os = "macos")))]
    let args = [
        "-n".to_owned(),
        "-c".to_owned(),
        "1".to_owned(),
        "-W".to_owned(),
        "1".to_owned(),
        ip.to_string(),
    ];

    let mut command = Command::new("ping");
    command
        .args(&args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = tokio::time::timeout(Duration::from_millis(1_700), command.output())
        .await
        .ok()?
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let answered = output.status.success()
        || stdout.to_ascii_lowercase().contains("ttl=")
        || stdout.to_ascii_lowercase().contains("ttl ");

    if !answered {
        return None;
    }

    Some(parse_rtt(&stdout).unwrap_or(0.0))
}

fn parse_rtt(output: &str) -> Option<f64> {
    let lower = output.to_ascii_lowercase();

    for marker in ["time=", "temps=", "tiempo=", "zeit="] {
        if let Some(position) = lower.find(marker) {
            let rest = &lower[position + marker.len()..];
            let number: String = rest
                .chars()
                .take_while(|character| {
                    character.is_ascii_digit() || *character == '.' || *character == ','
                })
                .collect();

            if let Ok(value) = number.replace(',', ".").parse() {
                return Some(value);
            }
        }
    }

    None
}

async fn resolve_hostname(ip: Ipv4Addr) -> Option<String> {
    let address = IpAddr::V4(ip);
    let task = tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&address).ok());

    match tokio::time::timeout(Duration::from_millis(900), task).await {
        Ok(Ok(Some(name))) if name != ip.to_string() => Some(name),
        _ => None,
    }
}

fn same_subnet(ip: Ipv4Addr, network: Ipv4Addr, prefix: u8) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };

    (u32::from(ip) & mask) == (u32::from(network) & mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rtt_from_ping() {
        let output = "64 bytes from 192.168.1.1: icmp_seq=1 ttl=64 time=3.42 ms";
        assert!((parse_rtt(output).unwrap() - 3.42).abs() < 0.001);
    }

    #[test]
    fn checks_subnet_membership() {
        let network = Ipv4Addr::new(192, 168, 1, 0);
        assert!(same_subnet(Ipv4Addr::new(192, 168, 1, 42), network, 24));
        assert!(!same_subnet(Ipv4Addr::new(192, 168, 2, 42), network, 24));
    }

    #[test]
    #[ignore = "requires the ping command"]
    fn scans_localhost() {
        use futures::StreamExt;

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let events = runtime.block_on(async {
            let mut stream = Box::pin(stream("127.0.0.1/32".to_owned()));
            let mut events = Vec::new();

            while let Some(event) = stream.next().await {
                let finished = matches!(event, Event::Done { .. } | Event::Error(_));
                events.push(event);
                if finished {
                    break;
                }
            }

            events
        });

        assert!(
            events
                .iter()
                .any(|event| matches!(event, Event::Done { found, .. } if *found >= 1))
        );
    }
}
