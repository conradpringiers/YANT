use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use iced::futures::SinkExt;
use iced::futures::stream::Stream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Latency,
    Download,
    Upload,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Latency => "Latence",
            Self::Download => "Download",
            Self::Upload => "Upload",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub download_url: String,
    pub upload_url: String,
    pub size_bytes: u64,
    pub parallel: u64,
    pub include_upload: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            download_url: "https://speed.cloudflare.com/__down".to_owned(),
            upload_url: "https://speed.cloudflare.com/__up".to_owned(),
            size_bytes: 25 * 1024 * 1024,
            parallel: 4,
            include_upload: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub download_mbps: Option<f64>,
    pub upload_mbps: Option<f64>,
    pub latency_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub bytes_downloaded: u64,
    pub bytes_uploaded: u64,
    pub duration_s: f64,
}

#[derive(Debug, Clone)]
pub enum Event {
    Phase(Phase),
    Latency {
        avg_ms: f64,
        jitter_ms: f64,
    },
    Progress {
        phase: Phase,
        bytes: u64,
        total: u64,
        instant_mbps: f64,
        elapsed_s: f64,
    },
    Done(Report),
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
    let config = Config {
        parallel: config.parallel.clamp(1, 8),
        size_bytes: config.size_bytes.max(1024 * 1024),
        ..config
    };

    if !config.download_url.starts_with("http://") && !config.download_url.starts_with("https://") {
        return Err("The test URL must start with http:// or https://".to_owned());
    }

    let client = reqwest::Client::builder()
        .user_agent(concat!("YANT/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| format!("Client HTTP : {error}"))?;

    let started = Instant::now();
    let mut report = Report::default();

    emit(sender, Event::Phase(Phase::Latency)).await?;

    if let Some((latency, jitter)) = measure_latency(&client, &config).await {
        report.latency_ms = Some(latency);
        report.jitter_ms = Some(jitter);

        emit(
            sender,
            Event::Latency {
                avg_ms: latency,
                jitter_ms: jitter,
            },
        )
        .await?;
    }

    emit(sender, Event::Phase(Phase::Download)).await?;

    let download = download(&client, &config, sender).await?;
    report.download_mbps = Some(download.0);
    report.bytes_downloaded = download.1;

    if config.include_upload && config.upload_url.starts_with("http") {
        emit(sender, Event::Phase(Phase::Upload)).await?;

        let upload = upload(&client, &config, sender).await?;
        report.upload_mbps = Some(upload.0);
        report.bytes_uploaded = upload.1;
    }

    report.duration_s = started.elapsed().as_secs_f64();
    emit(sender, Event::Done(report)).await?;

    Ok(())
}

async fn emit(sender: &mut Sender, event: Event) -> Result<(), String> {
    sender
        .send(event)
        .await
        .map_err(|_| "Test cancelled".to_owned())
}

async fn measure_latency(client: &reqwest::Client, config: &Config) -> Option<(f64, f64)> {
    let mut samples = Vec::new();
    let cloudflare = config.download_url.contains("__down");

    for index in 0..5 {
        let started = Instant::now();

        let result = if cloudflare {
            let url = format!("{}?bytes=0&r={index}", config.download_url);
            client
                .get(url)
                .header("Cache-Control", "no-store")
                .send()
                .await
        } else {
            client.head(&config.download_url).send().await
        };

        match result {
            Ok(response) if response.status().is_success() => {
                samples.push(started.elapsed().as_secs_f64() * 1000.0);
            }
            Ok(_) => {}
            Err(_) => break,
        }

        tokio::time::sleep(Duration::from_millis(80)).await;
    }

    if samples.is_empty() {
        return None;
    }

    let avg = samples.iter().sum::<f64>() / samples.len() as f64;
    let jitter = if samples.len() > 1 {
        samples
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .sum::<f64>()
            / (samples.len() - 1) as f64
    } else {
        0.0
    };

    Some((avg, jitter))
}

async fn download(
    client: &reqwest::Client,
    config: &Config,
    sender: &mut Sender,
) -> Result<(f64, u64), String> {
    let started = Instant::now();
    let counter = Arc::new(AtomicU64::new(0));
    let parallel = config.parallel;
    let chunk = (config.size_bytes / parallel).max(1);
    let mut tasks = FuturesUnordered::new();

    for index in 0..parallel {
        let client = client.clone();
        let url = download_url(&config.download_url, chunk, index);
        let counter = counter.clone();

        tasks.push(async move { download_chunk(&client, &url, &counter).await });
    }

    let mut ticker = tokio::time::interval(Duration::from_millis(200));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut remaining = parallel;
    let mut last_bytes = 0u64;
    let mut last_time = started;

    while remaining > 0 {
        tokio::select! {
            _ = ticker.tick() => {
                let bytes = counter.load(Ordering::Relaxed);
                let now = Instant::now();
                let delta_time = now.duration_since(last_time).as_secs_f64();
                let delta_bytes = bytes.saturating_sub(last_bytes) as f64;
                let instant = if delta_time > 0.0 {
                    delta_bytes * 8.0 / 1_000_000.0 / delta_time
                } else {
                    0.0
                };

                emit(sender, Event::Progress {
                    phase: Phase::Download,
                    bytes,
                    total: config.size_bytes,
                    instant_mbps: instant,
                    elapsed_s: now.duration_since(started).as_secs_f64(),
                }).await?;

                last_bytes = bytes;
                last_time = now;
            }
            Some(result) = tasks.next() => {
                remaining -= 1;
                result?;
            }
        }
    }

    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let bytes = counter.load(Ordering::Relaxed);
    let mbps = bytes as f64 * 8.0 / 1_000_000.0 / elapsed;

    emit(
        sender,
        Event::Progress {
            phase: Phase::Download,
            bytes,
            total: bytes.max(config.size_bytes),
            instant_mbps: mbps,
            elapsed_s: elapsed,
        },
    )
    .await?;

    Ok((mbps, bytes))
}

async fn download_chunk(
    client: &reqwest::Client,
    url: &str,
    counter: &AtomicU64,
) -> Result<(), String> {
    let mut response = client
        .get(url)
        .header("Cache-Control", "no-store")
        .send()
        .await
        .map_err(|error| format!("Download failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("Server responded {} ({url})", response.status()));
    }

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Lecture du flux interrompue : {error}"))?
    {
        counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);
    }

    Ok(())
}

fn download_url(base: &str, chunk: u64, index: u64) -> String {
    if base.contains("__down") {
        let separator = if base.contains('?') { '&' } else { '?' };
        format!("{base}{separator}bytes={chunk}&r={index}")
    } else {
        base.to_owned()
    }
}

async fn upload(
    client: &reqwest::Client,
    config: &Config,
    sender: &mut Sender,
) -> Result<(f64, u64), String> {
    let started = Instant::now();
    let counter = Arc::new(AtomicU64::new(0));
    let parallel = config.parallel;
    let chunk = (config.size_bytes / parallel).max(1);
    let payload = Arc::new(pseudo_random_payload(chunk as usize));
    let mut tasks = FuturesUnordered::new();

    for index in 0..parallel {
        let client = client.clone();
        let url = if config.upload_url.contains("__up") {
            let separator = if config.upload_url.contains('?') {
                '&'
            } else {
                '?'
            };
            format!("{}{separator}r={index}", config.upload_url)
        } else {
            config.upload_url.clone()
        };
        let body = payload.as_ref().clone();
        let counter = counter.clone();
        let size = chunk;

        tasks.push(async move {
            let response = client
                .post(url)
                .header("Content-Type", "application/octet-stream")
                .body(body)
                .send()
                .await
                .map_err(|error| format!("Upload failed: {error}"))?;

            if !response.status().is_success() {
                return Err(format!(
                    "Server rejected the upload ({})",
                    response.status()
                ));
            }

            counter.fetch_add(size, Ordering::Relaxed);
            Ok::<(), String>(())
        });
    }

    let mut ticker = tokio::time::interval(Duration::from_millis(200));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut remaining = parallel;
    let mut last_bytes = 0u64;
    let mut last_time = started;

    while remaining > 0 {
        tokio::select! {
            _ = ticker.tick() => {
                let bytes = counter.load(Ordering::Relaxed);
                let now = Instant::now();
                let delta_time = now.duration_since(last_time).as_secs_f64();
                let delta_bytes = bytes.saturating_sub(last_bytes) as f64;
                let instant = if delta_time > 0.0 {
                    delta_bytes * 8.0 / 1_000_000.0 / delta_time
                } else {
                    0.0
                };

                emit(sender, Event::Progress {
                    phase: Phase::Upload,
                    bytes,
                    total: config.size_bytes,
                    instant_mbps: instant,
                    elapsed_s: now.duration_since(started).as_secs_f64(),
                }).await?;

                last_bytes = bytes;
                last_time = now;
            }
            Some(result) = tasks.next() => {
                remaining -= 1;
                result?;
            }
        }
    }

    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let bytes = counter.load(Ordering::Relaxed);
    let mbps = bytes as f64 * 8.0 / 1_000_000.0 / elapsed;

    emit(
        sender,
        Event::Progress {
            phase: Phase::Upload,
            bytes,
            total: bytes.max(config.size_bytes),
            instant_mbps: mbps,
            elapsed_s: elapsed,
        },
    )
    .await?;

    Ok((mbps, bytes))
}

fn pseudo_random_payload(size: usize) -> Vec<u8> {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut payload = Vec::with_capacity(size);

    while payload.len() < size {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        payload.extend_from_slice(&state.to_le_bytes());
    }

    payload.truncate(size);
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_cloudflare_urls() {
        assert_eq!(
            download_url("https://speed.cloudflare.com/__down", 1000, 2),
            "https://speed.cloudflare.com/__down?bytes=1000&r=2"
        );
        assert_eq!(
            download_url("https://example.com/file.bin", 1000, 2),
            "https://example.com/file.bin"
        );
    }

    #[test]
    fn generates_requested_payload_size() {
        let payload = pseudo_random_payload(1024);
        assert_eq!(payload.len(), 1024);
        assert!(payload.iter().any(|byte| *byte != 0));
    }

    #[test]
    #[ignore = "requires network access and consumes data"]
    fn runs_a_small_speed_test() {
        use futures::StreamExt;

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let report = runtime.block_on(async {
            let mut stream = Box::pin(stream(Config {
                size_bytes: 2 * 1024 * 1024,
                parallel: 2,
                include_upload: true,
                ..Default::default()
            }));

            let mut report = None;
            while let Some(event) = stream.next().await {
                match event {
                    Event::Done(result) => {
                        report = Some(result);
                        break;
                    }
                    Event::Error(error) => panic!("speed test failed: {error}"),
                    _ => {}
                }
            }
            report
        });

        let report = report.expect("the test must produce a report");
        assert!(report.bytes_downloaded > 0);
        assert!(report.download_mbps.unwrap_or(0.0) > 0.0);
        assert!(report.upload_mbps.unwrap_or(0.0) > 0.0);
    }
}
