use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};
use iced::futures::SinkExt;
use iced::futures::stream::Stream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Web,
    Common,
    Remote,
    Databases,
    Custom,
}

impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::Web,
        Preset::Common,
        Preset::Remote,
        Preset::Databases,
        Preset::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Web => "Web (HTTP/HTTPS)",
            Self::Common => "Common ports",
            Self::Remote => "Remote access",
            Self::Databases => "Databases",
            Self::Custom => "Custom",
        }
    }

    pub fn ports(self) -> Vec<u16> {
        match self {
            Self::Web => vec![80, 443, 8080, 8443, 8000, 3000, 5000],
            Self::Common => vec![
                21, 22, 23, 25, 53, 80, 110, 111, 135, 139, 143, 161, 389, 443, 445, 465, 514, 587,
                631, 993, 995, 1433, 1521, 1883, 2049, 3000, 3306, 3389, 5432, 5672, 5900, 6379,
                8080, 8443, 8888, 9090, 9200, 27017,
            ],
            Self::Remote => vec![22, 23, 3389, 5900, 5901, 5985, 5986],
            Self::Databases => vec![1433, 1521, 3306, 5432, 6379, 9042, 9200, 11211, 27017],
            Self::Custom => Vec::new(),
        }
    }
}

impl std::fmt::Display for Preset {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct OpenPort {
    pub port: u16,
    pub service: &'static str,
    pub latency_ms: f64,
}

#[derive(Debug, Clone)]
pub enum Event {
    Started { resolved: String, total: u32 },
    Progress { done: u32, total: u32 },
    Found(OpenPort),
    Done { duration_s: f64 },
    Error(String),
}

type Sender = iced::futures::channel::mpsc::Sender<Event>;

pub fn stream(host: String, ports: Vec<u16>, timeout_ms: u64) -> impl Stream<Item = Event> {
    iced::stream::channel(128, move |mut sender| async move {
        if let Err(error) = run(host, ports, timeout_ms, &mut sender).await {
            let _ = sender.send(Event::Error(error)).await;
        }
    })
}

async fn run(
    host: String,
    ports: Vec<u16>,
    timeout_ms: u64,
    sender: &mut Sender,
) -> Result<(), String> {
    let host = host.trim().to_owned();

    if host.is_empty() {
        return Err("Enter a host to scan".to_owned());
    }

    if ports.is_empty() {
        return Err("No ports to scan".to_owned());
    }

    let address = resolve(&host).await?;
    let total = ports.len() as u32;
    let timeout = Duration::from_millis(timeout_ms.clamp(100, 5_000));
    let started = Instant::now();
    let done_count = Arc::new(AtomicU32::new(0));
    let probe_sender = sender.clone();

    let _ = sender
        .send(Event::Started {
            resolved: address.to_string(),
            total,
        })
        .await;

    let mut probes = stream::iter(ports)
        .map(|port| {
            let mut sender = probe_sender.clone();
            let done_count = done_count.clone();

            async move {
                let target = SocketAddr::new(address, port);
                let started = Instant::now();
                let probe = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(target));

                if let Ok(Ok(_stream)) = probe.await {
                    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;

                    let _ = sender
                        .send(Event::Found(OpenPort {
                            port,
                            service: service_name(port),
                            latency_ms,
                        }))
                        .await;

                    let _ = sender
                        .send(Event::Progress {
                            done: done_count.fetch_add(1, Ordering::Relaxed) + 1,
                            total,
                        })
                        .await;
                } else {
                    let _ = sender
                        .send(Event::Progress {
                            done: done_count.fetch_add(1, Ordering::Relaxed) + 1,
                            total,
                        })
                        .await;
                }
            }
        })
        .buffer_unordered(128);

    while probes.next().await.is_some() {}

    let _ = sender
        .send(Event::Done {
            duration_s: started.elapsed().as_secs_f64(),
        })
        .await;

    Ok(())
}

async fn resolve(host: &str) -> Result<IpAddr, String> {
    if let Ok(address) = host.parse::<IpAddr>() {
        return Ok(address);
    }

    let lookup = tokio::net::lookup_host((host, 80));

    match tokio::time::timeout(Duration::from_secs(5), lookup).await {
        Ok(Ok(addresses)) => {
            let addresses: Vec<SocketAddr> = addresses.collect();

            addresses
                .iter()
                .find(|address| address.is_ipv4())
                .or_else(|| addresses.first())
                .map(|address| address.ip())
                .ok_or_else(|| format!("No address found for {host}"))
        }
        Ok(Err(error)) => Err(format!("Host not found: {host} ({error})")),
        Err(_) => Err(format!("DNS resolution timed out for {host}")),
    }
}

pub fn service_name(port: u16) -> &'static str {
    match port {
        20 | 21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        25 => "SMTP",
        53 => "DNS",
        67 | 68 => "DHCP",
        80 => "HTTP",
        110 => "POP3",
        111 => "RPCbind",
        123 => "NTP",
        135 => "MS RPC",
        137..=139 => "NetBIOS",
        143 => "IMAP",
        161 | 162 => "SNMP",
        389 => "LDAP",
        443 => "HTTPS",
        445 => "SMB",
        465 => "SMTPS",
        514 => "Syslog",
        587 => "SMTP (submission)",
        631 => "IPP",
        993 => "IMAPS",
        995 => "POP3S",
        1433 => "Microsoft SQL",
        1521 => "Oracle",
        1883 => "MQTT",
        2049 => "NFS",
        2375 | 2376 => "Docker",
        3000 => "Dev server",
        3306 => "MySQL",
        3389 => "RDP",
        5432 => "PostgreSQL",
        5672 => "AMQP",
        5900 | 5901 => "VNC",
        6379 => "Redis",
        8080 => "HTTP alternate",
        8443 => "HTTPS alternate",
        8888 => "HTTP alternate",
        9090 => "Prometheus",
        9200 => "Elasticsearch",
        11211 => "Memcached",
        27017 => "MongoDB",
        _ => "Unknown service",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_not_empty() {
        for preset in Preset::ALL {
            if preset != Preset::Custom {
                assert!(!preset.ports().is_empty());
            }
        }
    }

    #[test]
    fn knows_common_services() {
        assert_eq!(service_name(22), "SSH");
        assert_eq!(service_name(443), "HTTPS");
        assert_eq!(service_name(5432), "PostgreSQL");
    }
}
