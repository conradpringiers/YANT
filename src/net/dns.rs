use std::net::IpAddr;
use std::time::{Duration, Instant};

use hickory_resolver::TokioResolver;
use hickory_resolver::proto::rr::RecordType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    A,
    Aaaa,
    Mx,
    Txt,
    Ns,
    Cname,
    Soa,
    Srv,
    Caa,
    Ptr,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::A,
        Kind::Aaaa,
        Kind::Mx,
        Kind::Txt,
        Kind::Ns,
        Kind::Cname,
        Kind::Soa,
        Kind::Srv,
        Kind::Caa,
        Kind::Ptr,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::A => "A — IPv4",
            Self::Aaaa => "AAAA — IPv6",
            Self::Mx => "MX — mail",
            Self::Txt => "TXT — text",
            Self::Ns => "NS — name servers",
            Self::Cname => "CNAME — alias",
            Self::Soa => "SOA — zone",
            Self::Srv => "SRV — service",
            Self::Caa => "CAA — authorities",
            Self::Ptr => "PTR — reverse",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::Aaaa => "AAAA",
            Self::Mx => "MX",
            Self::Txt => "TXT",
            Self::Ns => "NS",
            Self::Cname => "CNAME",
            Self::Soa => "SOA",
            Self::Srv => "SRV",
            Self::Caa => "CAA",
            Self::Ptr => "PTR",
        }
    }

    pub fn record_type(self) -> RecordType {
        match self {
            Self::A => RecordType::A,
            Self::Aaaa => RecordType::AAAA,
            Self::Mx => RecordType::MX,
            Self::Txt => RecordType::TXT,
            Self::Ns => RecordType::NS,
            Self::Cname => RecordType::CNAME,
            Self::Soa => RecordType::SOA,
            Self::Srv => RecordType::SRV,
            Self::Caa => RecordType::CAA,
            Self::Ptr => RecordType::PTR,
        }
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct Answer {
    pub record_type: String,
    pub value: String,
    pub ttl: u32,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub query: String,
    pub kind: Kind,
    pub answers: Vec<Answer>,
    pub elapsed_ms: f64,
    pub resolvers: String,
}

pub async fn lookup(input: &str, kind: Kind) -> Result<Report, String> {
    let input = input.trim().trim_end_matches('.');

    if input.is_empty() {
        return Err("Enter a domain name or IP address".to_owned());
    }

    let query = if kind == Kind::Ptr {
        reverse_name(input).unwrap_or_else(|| input.to_owned())
    } else {
        input.to_owned()
    };

    let resolver = TokioResolver::builder_tokio()
        .map_err(|error| format!("Invalid system DNS configuration: {error}"))?
        .build()
        .map_err(|error| format!("DNS resolver unavailable: {error}"))?;

    let started = Instant::now();
    let lookup = tokio::time::timeout(
        Duration::from_secs(8),
        resolver.lookup(query.clone(), kind.record_type()),
    )
    .await
    .map_err(|_| "DNS query timed out (8 s)".to_owned())?
    .map_err(|error| explain(error.to_string()))?;

    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;

    let answers = lookup
        .answers()
        .iter()
        .filter(|record| record.record_type() != RecordType::OPT)
        .map(|record| Answer {
            record_type: format!("{:?}", record.record_type()),
            value: record.data.to_string(),
            ttl: record.ttl,
        })
        .collect::<Vec<_>>();

    if answers.is_empty() {
        return Err("No records returned for this name and type".to_owned());
    }

    Ok(Report {
        query,
        kind,
        answers,
        elapsed_ms,
        resolvers: system_resolvers(),
    })
}

fn explain(error: String) -> String {
    let lower = error.to_ascii_lowercase();

    if lower.contains("nxdomain") || lower.contains("no records found") {
        format!("Domain not found: {error}")
    } else if lower.contains("timed out") || lower.contains("timeout") {
        format!("The DNS server did not respond: {error}")
    } else {
        error
    }
}

fn reverse_name(input: &str) -> Option<String> {
    let ip: IpAddr = input.parse().ok()?;

    Some(match ip {
        IpAddr::V4(address) => {
            let octets = address.octets();
            format!(
                "{}.{}.{}.{}.in-addr.arpa.",
                octets[3], octets[2], octets[1], octets[0]
            )
        }
        IpAddr::V6(address) => {
            let mut nibbles: Vec<char> = address
                .segments()
                .iter()
                .flat_map(|segment| format!("{segment:04x}").chars().collect::<Vec<_>>())
                .collect();
            nibbles.reverse();
            nibbles
                .iter()
                .map(|nibble| nibble.to_string())
                .collect::<Vec<_>>()
                .join(".")
                + ".ip6.arpa."
        }
    })
}

fn system_resolvers() -> String {
    #[cfg(unix)]
    {
        if let Ok(content) = std::fs::read_to_string("/etc/resolv.conf") {
            let servers: Vec<&str> = content
                .lines()
                .filter_map(|line| line.trim().strip_prefix("nameserver").map(str::trim))
                .collect();

            if !servers.is_empty() {
                return servers.join(", ");
            }
        }

        "system".to_owned()
    }

    #[cfg(not(unix))]
    {
        "system".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_reverse_ipv4_names() {
        assert_eq!(
            reverse_name("192.168.1.42").as_deref(),
            Some("42.1.168.192.in-addr.arpa.")
        );
    }

    #[test]
    fn builds_reverse_ipv6_names() {
        let name = reverse_name("2001:db8::1").unwrap();
        assert!(name.ends_with(".ip6.arpa."));
        assert!(name.starts_with("1.0.0.0."));
    }

    #[test]
    #[ignore = "requires network access"]
    fn resolves_a_public_domain() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let report = runtime
            .block_on(lookup("example.com", Kind::A))
            .expect("example.com must resolve");

        assert!(!report.answers.is_empty());
        assert!(
            report
                .answers
                .iter()
                .all(|answer| answer.record_type == "A")
        );
    }
}
