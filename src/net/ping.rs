use std::process::Stdio;
use std::time::Duration;

use iced::futures::SinkExt;
use iced::futures::stream::Stream;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub count: u32,
    pub interval_ms: u64,
    pub timeout_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "1.1.1.1".to_owned(),
            count: 4,
            interval_ms: 500,
            timeout_ms: 1000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Reply {
    pub seq: u32,
    pub rtt_ms: f64,
    pub ttl: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub sent: u32,
    pub received: u32,
    pub loss_pct: f64,
    pub min_ms: f64,
    pub avg_ms: f64,
    pub max_ms: f64,
    pub jitter_ms: f64,
    pub stddev_ms: f64,
}

#[derive(Debug, Clone)]
pub enum Event {
    Started { resolved: Option<String> },
    Reply(Reply),
    Done(Stats),
    Error(String),
}

type Sender = iced::futures::channel::mpsc::Sender<Event>;

pub fn stream(config: Config) -> impl Stream<Item = Event> {
    iced::stream::channel(64, move |mut sender| async move {
        if let Err(error) = run(config, &mut sender).await {
            let _ = sender.send(Event::Error(error)).await;
        }
    })
}

async fn run(config: Config, sender: &mut Sender) -> Result<(), String> {
    let host = config.host.trim().to_owned();

    if host.is_empty() {
        return Err("Enter a host to ping".to_owned());
    }

    let resolved = resolve(&host).await;

    if sender
        .send(Event::Started {
            resolved: resolved.clone(),
        })
        .await
        .is_err()
    {
        return Ok(());
    }

    let args = ping_arguments(&config, &host);
    let mut command = Command::new("ping");

    command
        .args(&args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Err(format!(
                "Failed to run `ping`: {error}. On Debian/Ubuntu: \
                 `sudo apt install iputils-ping`."
            ));
        }
    };

    let stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");
    let mut lines = BufReader::new(stdout).lines();

    let mut replies: Vec<Reply> = Vec::new();
    let mut summary: Option<(u32, u32)> = None;
    let mut all_output = Vec::new();

    while let Ok(Some(line)) = lines.next_line().await {
        all_output.push(line.clone());

        if let Some(reply) = parse_reply(&line) {
            replies.push(reply.clone());

            if sender.send(Event::Reply(reply)).await.is_err() {
                let _ = child.kill().await;
                return Ok(());
            }
        }

        if let Some(counts) = parse_summary(&line) {
            summary = Some(counts);
        }
    }

    let status = child.wait().await;
    let mut stderr_text = String::new();
    let _ = tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut stderr_text).await;

    if replies.is_empty() {
        let combined = format!("{}\n{}", all_output.join("\n"), stderr_text);

        if is_permission_error(&combined) {
            return Err("`ping` is not allowed to open an ICMP socket. \
                 Install `iputils-ping` (with setuid) or run YANT with the \
                 required privileges."
                .to_owned());
        }

        if let Ok(status) = status
            && !status.success()
            && !combined.trim().is_empty()
        {
            let detail = stderr_text.trim();

            if detail.contains("Name or service not known")
                || detail.contains("Servname not supported")
                || detail.contains("cannot resolve")
            {
                return Err(format!("Host not found: {host}"));
            }
        }
    }

    let (sent, received_from_summary) = summary.unwrap_or((config.count, replies.len() as u32));
    let received = if received_from_summary > 0 {
        received_from_summary.min(sent)
    } else {
        replies.len() as u32
    };

    let stats = summarize(&replies, sent, received);
    let _ = sender.send(Event::Done(stats)).await;

    Ok(())
}

async fn resolve(host: &str) -> Option<String> {
    let lookup = tokio::net::lookup_host((host, 443));

    match tokio::time::timeout(Duration::from_secs(3), lookup).await {
        Ok(Ok(mut addresses)) => addresses.next().map(|address| address.ip().to_string()),
        _ => None,
    }
}

fn ping_arguments(config: &Config, host: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![
            "-n".to_owned(),
            config.count.to_string(),
            "-w".to_owned(),
            config.timeout_ms.to_string(),
            host.to_owned(),
        ]
    }

    #[cfg(target_os = "macos")]
    {
        vec![
            "-n".to_owned(),
            "-c".to_owned(),
            config.count.to_string(),
            "-i".to_owned(),
            format!("{:.1}", config.interval_ms.max(200) as f64 / 1000.0),
            "-W".to_owned(),
            config.timeout_ms.to_string(),
            host.to_owned(),
        ]
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        vec![
            "-n".to_owned(),
            "-c".to_owned(),
            config.count.to_string(),
            "-i".to_owned(),
            format!("{:.1}", config.interval_ms.max(200) as f64 / 1000.0),
            "-W".to_owned(),
            ((config.timeout_ms.max(200) as f64 / 1000.0).ceil() as u64).to_string(),
            host.to_owned(),
        ]
    }
}

fn parse_reply(line: &str) -> Option<Reply> {
    let lower = line.to_ascii_lowercase();

    let looks_like_reply = lower.contains("icmp_seq")
        || lower.contains("bytes from")
        || lower.contains("octets de")
        || lower.contains("reply from")
        || lower.contains("réponse de")
        || lower.contains("antwort von");

    if !looks_like_reply {
        return None;
    }

    let rtt_ms = parse_number_after(
        &lower,
        &["time=", "temps=", "tiempo=", "zeit=", "time<", "temps<"],
    )?;

    let seq = parse_number_after(&lower, &["icmp_seq=", "icmp_seq ", "seq="])
        .map(|value| value as u32)
        .unwrap_or(0);

    let ttl = parse_number_after(&lower, &["ttl=", "ttl "]).map(|value| value as u32);

    Some(Reply { seq, rtt_ms, ttl })
}

fn parse_number_after(haystack: &str, markers: &[&str]) -> Option<f64> {
    for marker in markers {
        let Some(position) = haystack.find(marker) else {
            continue;
        };

        let rest = haystack[position + marker.len()..]
            .trim_start_matches(['=', '<', ' '])
            .trim_start();

        let number: String = rest
            .chars()
            .take_while(|character| {
                character.is_ascii_digit() || *character == '.' || *character == ','
            })
            .collect();

        let number = number.replace(',', ".");

        if let Ok(value) = number.parse::<f64>() {
            return Some(value);
        }
    }

    None
}

fn parse_summary(line: &str) -> Option<(u32, u32)> {
    let lower = line.to_ascii_lowercase();

    let is_summary = (lower.contains("packets transmitted") && lower.contains("received"))
        || (lower.contains("sent") && lower.contains("received") && lower.contains("lost"))
        || (lower.contains("envoyés") && lower.contains("reçus"))
        || (lower.contains("paquets transmis") && lower.contains("reçus"))
        || lower.contains("packet loss")
        || lower.contains("perte de paquets")
        || (lower.contains("übertragen") && lower.contains("empfangen"));

    if !is_summary {
        return None;
    }

    let numbers: Vec<u32> = lower
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect();

    if numbers.len() >= 2 {
        Some((numbers[0], numbers[1]))
    } else {
        None
    }
}

fn is_permission_error(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();

    lower.contains("operation not permitted")
        || lower.contains("permission denied")
        || lower.contains("socket")
        || lower.contains("are you root")
        || lower.contains("setuid")
}

fn summarize(replies: &[Reply], sent: u32, received: u32) -> Stats {
    let sent = sent.max(1);
    let received = received.min(sent);
    let loss_pct = (sent.saturating_sub(received) as f64 / sent as f64) * 100.0;

    if replies.is_empty() {
        return Stats {
            sent,
            received: 0,
            loss_pct: 100.0,
            ..Default::default()
        };
    }

    let rtts: Vec<f64> = replies.iter().map(|reply| reply.rtt_ms).collect();
    let sum: f64 = rtts.iter().sum();
    let avg = sum / rtts.len() as f64;
    let min = rtts.iter().copied().fold(f64::INFINITY, f64::min);
    let max = rtts.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    let variance = rtts
        .iter()
        .map(|rtt| {
            let delta = rtt - avg;
            delta * delta
        })
        .sum::<f64>()
        / rtts.len() as f64;

    let jitter = if rtts.len() > 1 {
        rtts.windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .sum::<f64>()
            / (rtts.len() - 1) as f64
    } else {
        0.0
    };

    Stats {
        sent,
        received: rtts.len() as u32,
        loss_pct,
        min_ms: min,
        avg_ms: avg,
        max_ms: max,
        jitter_ms: jitter,
        stddev_ms: variance.sqrt(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_reply() {
        let reply = parse_reply("64 bytes from 1.1.1.1: icmp_seq=3 ttl=57 time=12.4 ms").unwrap();

        assert_eq!(reply.seq, 3);
        assert_eq!(reply.ttl, Some(57));
        assert!((reply.rtt_ms - 12.4).abs() < 0.001);
    }

    #[test]
    fn parses_french_windows_reply() {
        let reply = parse_reply("Réponse de 192.168.1.1 : octets=32 temps=2 ms TTL=64").unwrap();

        assert_eq!(reply.ttl, Some(64));
        assert!((reply.rtt_ms - 2.0).abs() < 0.001);
    }

    #[test]
    fn parses_linux_summary() {
        assert_eq!(
            parse_summary("4 packets transmitted, 3 received, 25% packet loss, time 3004ms"),
            Some((4, 3))
        );
    }

    #[test]
    fn computes_stats() {
        let replies = vec![
            Reply {
                seq: 1,
                rtt_ms: 10.0,
                ttl: None,
            },
            Reply {
                seq: 2,
                rtt_ms: 20.0,
                ttl: None,
            },
            Reply {
                seq: 3,
                rtt_ms: 30.0,
                ttl: None,
            },
        ];
        let stats = summarize(&replies, 4, 3);

        assert_eq!(stats.received, 3);
        assert!((stats.loss_pct - 25.0).abs() < 0.001);
        assert!((stats.avg_ms - 20.0).abs() < 0.001);
        assert!((stats.jitter_ms - 10.0).abs() < 0.001);
    }

    #[test]
    fn parses_french_summary() {
        assert_eq!(
            parse_summary("2 paquets transmis, 2 reçus, 0% packet loss, time 501ms"),
            Some((2, 2))
        );
    }

    #[test]
    #[ignore = "requires network access"]
    fn pings_a_public_address() {
        use futures::StreamExt;

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let events = runtime.block_on(async {
            let mut stream = Box::pin(stream(Config {
                host: "1.1.1.1".to_owned(),
                count: 2,
                interval_ms: 300,
                timeout_ms: 1_500,
            }));

            let mut events = Vec::new();
            while let Some(event) = stream.next().await {
                let finished = matches!(event, Event::Done(_) | Event::Error(_));
                events.push(event);
                if finished {
                    break;
                }
            }
            events
        });

        let done = events
            .iter()
            .find_map(|event| match event {
                Event::Done(stats) => Some(stats),
                _ => None,
            })
            .expect("ping must end with Done");

        assert!(done.received > 0);
        assert!(done.avg_ms > 0.0);
    }
}
