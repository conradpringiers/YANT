use std::collections::HashMap;
use std::time::{Duration, Instant};

use iced::{Element, Subscription, Task, Theme};

use crate::net::{dns, interfaces, ping, ports, public_ip, scan, speed, wifi};
use crate::ui;
use crate::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Wifi,
    Speed,
    Latency,
    Devices,
    Dns,
    Ports,
}

impl Tab {
    pub const ALL: [Tab; 7] = [
        Tab::Dashboard,
        Tab::Wifi,
        Tab::Speed,
        Tab::Latency,
        Tab::Devices,
        Tab::Dns,
        Tab::Ports,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Wifi => "Wi-Fi",
            Self::Speed => "Speed test",
            Self::Latency => "Ping",
            Self::Devices => "Devices",
            Self::Dns => "DNS",
            Self::Ports => "Ports",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Dashboard => "Network status and local interfaces",
            Self::Wifi => "Signal, channel, band and nearby networks",
            Self::Speed => "Download and upload throughput",
            Self::Latency => "ICMP latency, packet loss and jitter",
            Self::Devices => "Discover hosts on the local network",
            Self::Dns => "Resolve DNS records",
            Self::Ports => "Scan TCP ports on a host",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dashboard" | "overview" => Some(Self::Dashboard),
            "wifi" | "wi-fi" => Some(Self::Wifi),
            "speed" | "speedtest" => Some(Self::Speed),
            "latency" | "ping" => Some(Self::Latency),
            "devices" | "scan" => Some(Self::Devices),
            "dns" => Some(Self::Dns),
            "ports" => Some(Self::Ports),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Rate {
    pub rx: f64,
    pub tx: f64,
}

#[derive(Debug, Default)]
pub struct PublicIpState {
    pub value: Option<public_ip::PublicInfo>,
    pub error: Option<String>,
    pub loading: bool,
    pub loaded_once: bool,
}

#[derive(Debug, Default)]
pub struct WifiState {
    pub info: Option<wifi::WifiInfo>,
    pub error: Option<String>,
    pub interfaces: Vec<String>,
    pub selected: Option<String>,
    pub networks: Vec<wifi::WifiNetwork>,
    pub signal_history: Vec<f64>,
    pub scanning: bool,
    pub scan_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SpeedProgress {
    pub bytes: u64,
    pub total: u64,
    pub instant_mbps: f64,
    pub elapsed_s: f64,
}

#[derive(Debug)]
pub struct SpeedState {
    pub url: String,
    pub upload_url: String,
    pub size_mb: u64,
    pub include_upload: bool,
    pub running: bool,
    pub phase: Option<speed::Phase>,
    pub progress: Option<SpeedProgress>,
    pub result: Option<speed::Report>,
    pub error: Option<String>,
    pub download_samples: Vec<f64>,
    pub upload_samples: Vec<f64>,
    pub latency_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub abort: Option<iced::task::Handle>,
    pub run: u64,
}

impl Default for SpeedState {
    fn default() -> Self {
        Self {
            url: "https://speed.cloudflare.com/__down".to_owned(),
            upload_url: "https://speed.cloudflare.com/__up".to_owned(),
            size_mb: 25,
            include_upload: true,
            running: false,
            phase: None,
            progress: None,
            result: None,
            error: None,
            download_samples: Vec::new(),
            upload_samples: Vec::new(),
            latency_ms: None,
            jitter_ms: None,
            abort: None,
            run: 0,
        }
    }
}

#[derive(Debug)]
pub struct PingState {
    pub host: String,
    pub count: u32,
    pub running: bool,
    pub resolved: Option<String>,
    pub replies: Vec<ping::Reply>,
    pub stats: Option<ping::Stats>,
    pub error: Option<String>,
    pub abort: Option<iced::task::Handle>,
    pub run: u64,
}

impl Default for PingState {
    fn default() -> Self {
        Self {
            host: "1.1.1.1".to_owned(),
            count: 4,
            running: false,
            resolved: None,
            replies: Vec::new(),
            stats: None,
            error: None,
            abort: None,
            run: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct ScanState {
    pub subnet: String,
    pub running: bool,
    pub total: u32,
    pub done: u32,
    pub found: usize,
    pub devices: Vec<scan::Device>,
    pub duration_s: Option<f64>,
    pub error: Option<String>,
    pub abort: Option<iced::task::Handle>,
    pub run: u64,
}

#[derive(Debug)]
pub struct DnsState {
    pub input: String,
    pub kind: dns::Kind,
    pub running: bool,
    pub report: Option<dns::Report>,
    pub error: Option<String>,
    pub run: u64,
}

impl Default for DnsState {
    fn default() -> Self {
        Self {
            input: "example.com".to_owned(),
            kind: dns::Kind::A,
            running: false,
            report: None,
            error: None,
            run: 0,
        }
    }
}

#[derive(Debug)]
pub struct PortsState {
    pub host: String,
    pub preset: ports::Preset,
    pub custom: String,
    pub timeout_ms: u64,
    pub running: bool,
    pub resolved: Option<String>,
    pub open: Vec<ports::OpenPort>,
    pub done: u32,
    pub total: u32,
    pub duration_s: Option<f64>,
    pub error: Option<String>,
    pub abort: Option<iced::task::Handle>,
    pub run: u64,
}

impl Default for PortsState {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            preset: ports::Preset::Common,
            custom: "1-1024".to_owned(),
            timeout_ms: 800,
            running: false,
            resolved: None,
            open: Vec::new(),
            done: 0,
            total: 0,
            duration_s: None,
            error: None,
            abort: None,
            run: 0,
        }
    }
}

#[derive(Debug)]
pub struct App {
    pub tab: Tab,
    pub interfaces: Vec<interfaces::InterfaceInfo>,
    pub rates: HashMap<String, Rate>,
    pub public_ip: PublicIpState,
    pub wifi: WifiState,
    pub speed: SpeedState,
    pub ping: PingState,
    pub scan: ScanState,
    pub dns: DnsState,
    pub ports: PortsState,
    pub error: Option<String>,
    previous_counters: HashMap<String, (u64, u64, Instant)>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            tab: Tab::Dashboard,
            interfaces: Vec::new(),
            rates: HashMap::new(),
            public_ip: PublicIpState::default(),
            wifi: WifiState::default(),
            speed: SpeedState::default(),
            ping: PingState::default(),
            scan: ScanState::default(),
            dns: DnsState::default(),
            ports: PortsState::default(),
            error: None,
            previous_counters: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Tab(Tab),
    Tick,
    Refresh,

    Interfaces(Result<Vec<interfaces::InterfaceInfo>, String>),
    PublicIp(Result<public_ip::PublicInfo, String>),

    WifiLoaded(Result<wifi::WifiInfo, String>),
    WifiScan,
    WifiScanResult(Result<Vec<wifi::WifiNetwork>, String>),
    WifiInterface(String),

    SpeedUrl(String),
    SpeedUploadUrl(String),
    SpeedSize(u64),
    SpeedUpload(bool),
    SpeedStart,
    SpeedStop,
    SpeedEvent(u64, speed::Event),

    PingHost(String),
    PingCount(u32),
    PingStart,
    PingStop,
    PingEvent(u64, ping::Event),

    ScanSubnet(String),
    ScanStart,
    ScanStop,
    ScanEvent(u64, scan::Event),
    PingDevice(String),
    ScanPortsFor(String),

    DnsInput(String),
    DnsKind(dns::Kind),
    DnsStart,
    DnsResult(u64, Result<dns::Report, String>),

    PortsHost(String),
    PortsPreset(ports::Preset),
    PortsCustom(String),
    PortsTimeout(u64),
    PortsStart,
    PortsStop,
    PortsEvent(u64, ports::Event),

    Copy(String),
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let mut app = Self::default();
        app.public_ip.loading = true;

        if let Ok(tab) = std::env::var("YANT_TAB")
            && let Some(tab) = Tab::from_key(&tab)
        {
            app.tab = tab;
        }

        let task = Task::batch([
            Task::perform(interfaces::fetch(), Message::Interfaces),
            Task::perform(wifi::fetch(None), Message::WifiLoaded),
            Task::perform(public_ip::fetch(), Message::PublicIp),
        ]);

        (app, task)
    }

    pub fn theme(&self) -> Theme {
        crate::theme::theme()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match self.tab {
            Tab::Dashboard | Tab::Wifi => {
                iced::time::every(Duration::from_secs(2)).map(|_| Message::Tick)
            }
            _ => Subscription::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        ui::view(self)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tab(tab) => {
                self.tab = tab;

                if matches!(tab, Tab::Dashboard | Tab::Wifi) {
                    Task::batch([
                        Task::perform(interfaces::fetch(), Message::Interfaces),
                        Task::perform(wifi::fetch(self.wifi.selected.clone()), Message::WifiLoaded),
                    ])
                } else {
                    Task::none()
                }
            }
            Message::Tick => self.refresh_live_data(),
            Message::Refresh => {
                self.public_ip.loading = true;

                Task::batch([
                    Task::perform(interfaces::fetch(), Message::Interfaces),
                    Task::perform(wifi::fetch(self.wifi.selected.clone()), Message::WifiLoaded),
                    Task::perform(public_ip::fetch(), Message::PublicIp),
                ])
            }

            Message::Interfaces(result) => {
                match result {
                    Ok(interfaces) => self.accept_interfaces(interfaces),
                    Err(error) => self.error = Some(error),
                }

                Task::none()
            }
            Message::PublicIp(result) => {
                self.public_ip.loading = false;
                self.public_ip.loaded_once = true;

                match result {
                    Ok(info) => {
                        self.public_ip.value = Some(info);
                        self.public_ip.error = None;
                    }
                    Err(error) => self.public_ip.error = Some(error),
                }

                Task::none()
            }

            Message::WifiLoaded(result) => {
                match result {
                    Ok(info) => {
                        if info.connected {
                            if let Some(signal) = info.signal_dbm {
                                self.wifi.signal_history.push(signal as f64);

                                if self.wifi.signal_history.len() > 120 {
                                    self.wifi.signal_history.remove(0);
                                }
                            }
                        } else {
                            self.wifi.signal_history.clear();
                        }

                        if let Some(interface) = info.interface.clone() {
                            self.wifi.selected = Some(interface);
                        }

                        self.wifi.info = Some(info);
                        self.wifi.error = None;
                    }
                    Err(error) => self.wifi.error = Some(error),
                }

                Task::none()
            }
            Message::WifiScan => {
                self.wifi.scanning = true;
                self.wifi.scan_error = None;

                Task::perform(
                    wifi::scan(self.wifi.selected.clone()),
                    Message::WifiScanResult,
                )
            }
            Message::WifiScanResult(result) => {
                self.wifi.scanning = false;

                match result {
                    Ok(networks) => {
                        self.wifi.networks = networks;
                        self.wifi.scan_error = None;
                    }
                    Err(error) => self.wifi.scan_error = Some(error),
                }

                Task::none()
            }
            Message::WifiInterface(interface) => {
                self.wifi.selected = Some(interface.clone());
                self.wifi.info = None;
                self.wifi.error = None;
                self.wifi.signal_history.clear();

                Task::perform(wifi::fetch(Some(interface)), Message::WifiLoaded)
            }

            Message::SpeedUrl(url) => {
                self.speed.url = url;
                Task::none()
            }
            Message::SpeedUploadUrl(url) => {
                self.speed.upload_url = url;
                Task::none()
            }
            Message::SpeedSize(size) => {
                self.speed.size_mb = size;
                Task::none()
            }
            Message::SpeedUpload(enabled) => {
                self.speed.include_upload = enabled;
                Task::none()
            }
            Message::SpeedStart => self.start_speed_test(),
            Message::SpeedStop => {
                if let Some(handle) = self.speed.abort.take() {
                    handle.abort();
                }

                self.speed.running = false;
                Task::none()
            }
            Message::SpeedEvent(run, event) => self.handle_speed_event(run, event),

            Message::PingHost(host) => {
                self.ping.host = host;
                Task::none()
            }
            Message::PingCount(count) => {
                self.ping.count = count;
                Task::none()
            }
            Message::PingStart => self.start_ping(),
            Message::PingStop => {
                if let Some(handle) = self.ping.abort.take() {
                    handle.abort();
                }

                self.ping.running = false;
                Task::none()
            }
            Message::PingEvent(run, event) => self.handle_ping_event(run, event),

            Message::ScanSubnet(subnet) => {
                self.scan.subnet = subnet;
                Task::none()
            }
            Message::ScanStart => self.start_scan(),
            Message::ScanStop => {
                if let Some(handle) = self.scan.abort.take() {
                    handle.abort();
                }

                self.scan.running = false;
                Task::none()
            }
            Message::ScanEvent(run, event) => self.handle_scan_event(run, event),
            Message::PingDevice(host) => {
                self.ping.host = host;
                self.tab = Tab::Latency;
                self.start_ping()
            }
            Message::ScanPortsFor(host) => {
                self.ports.host = host;
                self.tab = Tab::Ports;
                Task::none()
            }

            Message::DnsInput(value) => {
                self.dns.input = value;
                Task::none()
            }
            Message::DnsKind(kind) => {
                self.dns.kind = kind;
                Task::none()
            }
            Message::DnsStart => self.start_dns(),
            Message::DnsResult(run, result) => {
                if run != self.dns.run {
                    return Task::none();
                }

                self.dns.running = false;

                match result {
                    Ok(report) => {
                        self.dns.report = Some(report);
                        self.dns.error = None;
                    }
                    Err(error) => self.dns.error = Some(error),
                }

                Task::none()
            }

            Message::PortsHost(host) => {
                self.ports.host = host;
                Task::none()
            }
            Message::PortsPreset(preset) => {
                self.ports.preset = preset;
                Task::none()
            }
            Message::PortsCustom(custom) => {
                self.ports.custom = custom;
                Task::none()
            }
            Message::PortsTimeout(timeout) => {
                self.ports.timeout_ms = timeout;
                Task::none()
            }
            Message::PortsStart => self.start_port_scan(),
            Message::PortsStop => {
                if let Some(handle) = self.ports.abort.take() {
                    handle.abort();
                }

                self.ports.running = false;
                Task::none()
            }
            Message::PortsEvent(run, event) => self.handle_ports_event(run, event),

            Message::Copy(text) => iced::clipboard::write(text),
        }
    }

    fn refresh_live_data(&mut self) -> Task<Message> {
        let mut tasks = vec![Task::perform(interfaces::fetch(), Message::Interfaces)];

        if matches!(self.tab, Tab::Dashboard | Tab::Wifi) {
            tasks.push(Task::perform(
                wifi::fetch(self.wifi.selected.clone()),
                Message::WifiLoaded,
            ));
        }

        Task::batch(tasks)
    }

    fn accept_interfaces(&mut self, list: Vec<interfaces::InterfaceInfo>) {
        let now = Instant::now();
        let mut rates = HashMap::new();

        for interface in &list {
            if let Some((rx, tx, then)) = self.previous_counters.get(&interface.name) {
                let elapsed = now.duration_since(*then).as_secs_f64();

                if elapsed > 0.05 {
                    rates.insert(
                        interface.name.clone(),
                        Rate {
                            rx: interface.rx_bytes.saturating_sub(*rx) as f64 / elapsed,
                            tx: interface.tx_bytes.saturating_sub(*tx) as f64 / elapsed,
                        },
                    );
                }
            }
        }

        self.rates = rates;
        self.previous_counters = list
            .iter()
            .map(|interface| {
                (
                    interface.name.clone(),
                    (interface.rx_bytes, interface.tx_bytes, now),
                )
            })
            .collect();

        self.wifi.interfaces = list
            .iter()
            .filter(|interface| interface.is_wifi)
            .map(|interface| interface.name.clone())
            .collect();

        if self
            .wifi
            .selected
            .as_ref()
            .is_none_or(|selected| !self.wifi.interfaces.contains(selected))
        {
            self.wifi.selected = self.wifi.interfaces.first().cloned();
        }

        if self.scan.subnet.trim().is_empty()
            && let Some(primary) = interfaces::primary(&list)
            && let Some(cidr) = primary.cidr()
        {
            self.scan.subnet = cidr;
        }

        self.interfaces = list;
    }

    fn start_speed_test(&mut self) -> Task<Message> {
        if self.speed.running {
            return Task::none();
        }

        let url = self.speed.url.trim().to_owned();

        if !url.starts_with("http://") && !url.starts_with("https://") {
            self.speed.error = Some("The test URL must start with http:// or https://".to_owned());
            return Task::none();
        }

        if self.speed.include_upload && !self.speed.upload_url.trim().starts_with("http") {
            self.speed.error =
                Some("The upload URL must start with http:// or https://".to_owned());
            return Task::none();
        }

        if let Some(handle) = self.speed.abort.take() {
            handle.abort();
        }

        self.speed.run += 1;
        let run = self.speed.run;

        self.speed.running = true;
        self.speed.error = None;
        self.speed.result = None;
        self.speed.progress = None;
        self.speed.phase = Some(speed::Phase::Latency);
        self.speed.download_samples.clear();
        self.speed.upload_samples.clear();
        self.speed.latency_ms = None;
        self.speed.jitter_ms = None;

        let parallel = if url.contains("__down") { 4 } else { 1 };

        let config = speed::Config {
            download_url: url,
            upload_url: self.speed.upload_url.trim().to_owned(),
            size_bytes: self.speed.size_mb * 1024 * 1024,
            include_upload: self.speed.include_upload,
            parallel,
        };

        let (task, handle) = Task::stream(speed::stream(config))
            .map(move |event| Message::SpeedEvent(run, event))
            .abortable();

        self.speed.abort = Some(handle);
        task
    }

    fn handle_speed_event(&mut self, run: u64, event: speed::Event) -> Task<Message> {
        if run != self.speed.run || !self.speed.running {
            return Task::none();
        }

        match event {
            speed::Event::Phase(phase) => {
                self.speed.phase = Some(phase);
            }
            speed::Event::Latency { avg_ms, jitter_ms } => {
                self.speed.latency_ms = Some(avg_ms);
                self.speed.jitter_ms = Some(jitter_ms);
            }
            speed::Event::Progress {
                phase,
                bytes,
                total,
                instant_mbps,
                elapsed_s,
            } => {
                self.speed.progress = Some(SpeedProgress {
                    bytes,
                    total,
                    instant_mbps,
                    elapsed_s,
                });

                if instant_mbps > 0.0 {
                    let samples = match phase {
                        speed::Phase::Download => &mut self.speed.download_samples,
                        speed::Phase::Upload => &mut self.speed.upload_samples,
                        speed::Phase::Latency => return Task::none(),
                    };

                    samples.push(instant_mbps);
                    if samples.len() > 300 {
                        samples.remove(0);
                    }
                }
            }
            speed::Event::Done(report) => {
                self.speed.running = false;
                self.speed.phase = None;
                self.speed.progress = None;
                self.speed.result = Some(report);
                self.speed.abort = None;
            }
            speed::Event::Error(error) => {
                self.speed.running = false;
                self.speed.phase = None;
                self.speed.error = Some(error);
                self.speed.abort = None;
            }
        }

        Task::none()
    }

    fn start_ping(&mut self) -> Task<Message> {
        if self.ping.running {
            return Task::none();
        }

        if self.ping.host.trim().is_empty() {
            self.ping.error = Some("Enter a host to ping".to_owned());
            return Task::none();
        }

        if let Some(handle) = self.ping.abort.take() {
            handle.abort();
        }

        self.ping.run += 1;
        let run = self.ping.run;

        self.ping.running = true;
        self.ping.error = None;
        self.ping.stats = None;
        self.ping.replies.clear();
        self.ping.resolved = None;

        let config = ping::Config {
            host: self.ping.host.trim().to_owned(),
            count: self.ping.count.clamp(1, 100),
            ..Default::default()
        };

        let (task, handle) = Task::stream(ping::stream(config))
            .map(move |event| Message::PingEvent(run, event))
            .abortable();

        self.ping.abort = Some(handle);
        task
    }

    fn handle_ping_event(&mut self, run: u64, event: ping::Event) -> Task<Message> {
        if run != self.ping.run || !self.ping.running {
            return Task::none();
        }

        match event {
            ping::Event::Started { resolved, .. } => {
                self.ping.resolved = resolved;
            }
            ping::Event::Reply(reply) => {
                self.ping.replies.push(reply);
            }
            ping::Event::Done(stats) => {
                self.ping.running = false;
                self.ping.stats = Some(stats);
                self.ping.abort = None;
            }
            ping::Event::Error(error) => {
                self.ping.running = false;
                self.ping.error = Some(error);
                self.ping.abort = None;
            }
        }

        Task::none()
    }

    fn start_scan(&mut self) -> Task<Message> {
        if self.scan.running {
            return Task::none();
        }

        let subnet = match util::parse_subnet(&self.scan.subnet) {
            Ok((network, prefix)) => {
                self.scan.subnet = format!("{network}/{prefix}");
                (network, prefix)
            }
            Err(error) => {
                self.scan.error = Some(error);
                return Task::none();
            }
        };

        if subnet.1 < 20 {
            self.scan.error =
                Some("Subnet is too large: use a prefix between /20 and /32".to_owned());
            return Task::none();
        }

        if let Some(handle) = self.scan.abort.take() {
            handle.abort();
        }

        self.scan.run += 1;
        let run = self.scan.run;

        self.scan.running = true;
        self.scan.error = None;
        self.scan.devices.clear();
        self.scan.total = 0;
        self.scan.done = 0;
        self.scan.found = 0;
        self.scan.duration_s = None;

        let (task, handle) = Task::stream(scan::stream(self.scan.subnet.clone()))
            .map(move |event| Message::ScanEvent(run, event))
            .abortable();

        self.scan.abort = Some(handle);
        task
    }

    fn handle_scan_event(&mut self, run: u64, event: scan::Event) -> Task<Message> {
        if run != self.scan.run || !self.scan.running {
            return Task::none();
        }

        match event {
            scan::Event::Started { subnet, total } => {
                self.scan.subnet = subnet;
                self.scan.total = total;
            }
            scan::Event::Progress { done, total, found } => {
                self.scan.done = done;
                self.scan.total = total;
                self.scan.found = found;
            }
            scan::Event::Found(device) => {
                self.upsert_device(device);
            }
            scan::Event::Done { found, duration_s } => {
                self.scan.running = false;
                self.scan.found = found;
                self.scan.duration_s = Some(duration_s);
                self.scan.abort = None;
            }
            scan::Event::Error(error) => {
                self.scan.running = false;
                self.scan.error = Some(error);
                self.scan.abort = None;
            }
        }

        Task::none()
    }

    fn upsert_device(&mut self, device: scan::Device) {
        if let Some(existing) = self
            .scan
            .devices
            .iter_mut()
            .find(|candidate| candidate.ip == device.ip)
        {
            if existing.mac.is_none() {
                existing.mac = device.mac;
            }
            if existing.hostname.is_none() {
                existing.hostname = device.hostname;
            }
            if existing.rtt_ms.is_none() {
                existing.rtt_ms = device.rtt_ms;
            }
            existing.is_gateway |= device.is_gateway;
            existing.is_self |= device.is_self;
        } else {
            self.scan.devices.push(device);
            self.scan
                .devices
                .sort_by_key(|candidate| u32::from(candidate.ip));
        }
    }

    fn start_dns(&mut self) -> Task<Message> {
        if self.dns.running {
            return Task::none();
        }

        self.dns.run += 1;
        let run = self.dns.run;

        self.dns.running = true;
        self.dns.error = None;

        let input = self.dns.input.trim().to_owned();
        let kind = self.dns.kind;

        Task::perform(
            async move { dns::lookup(&input, kind).await },
            move |result| Message::DnsResult(run, result),
        )
    }

    fn start_port_scan(&mut self) -> Task<Message> {
        if self.ports.running {
            return Task::none();
        }

        if self.ports.host.trim().is_empty() {
            self.ports.error = Some("Enter a host to scan".to_owned());
            return Task::none();
        }

        let ports = if self.ports.preset == ports::Preset::Custom {
            match util::parse_ports(&self.ports.custom) {
                Ok(ports) => ports,
                Err(error) => {
                    self.ports.error = Some(error);
                    return Task::none();
                }
            }
        } else {
            self.ports.preset.ports()
        };

        if let Some(handle) = self.ports.abort.take() {
            handle.abort();
        }

        self.ports.run += 1;
        let run = self.ports.run;

        self.ports.running = true;
        self.ports.error = None;
        self.ports.open.clear();
        self.ports.resolved = None;
        self.ports.done = 0;
        self.ports.total = ports.len() as u32;
        self.ports.duration_s = None;

        let (task, handle) = Task::stream(ports::stream(
            self.ports.host.trim().to_owned(),
            ports,
            self.ports.timeout_ms,
        ))
        .map(move |event| Message::PortsEvent(run, event))
        .abortable();

        self.ports.abort = Some(handle);
        task
    }

    fn handle_ports_event(&mut self, run: u64, event: ports::Event) -> Task<Message> {
        if run != self.ports.run || !self.ports.running {
            return Task::none();
        }

        match event {
            ports::Event::Started {
                resolved, total, ..
            } => {
                self.ports.resolved = Some(resolved);
                self.ports.total = total;
            }
            ports::Event::Progress { done, total, .. } => {
                self.ports.done = done;
                self.ports.total = total;
            }
            ports::Event::Found(port) => {
                if !self
                    .ports
                    .open
                    .iter()
                    .any(|candidate| candidate.port == port.port)
                {
                    self.ports.open.push(port);
                    self.ports.open.sort_by_key(|candidate| candidate.port);
                }
            }
            ports::Event::Done { duration_s, .. } => {
                self.ports.running = false;
                self.ports.duration_s = Some(duration_s);
                self.ports.abort = None;
            }
            ports::Event::Error(error) => {
                self.ports.running = false;
                self.ports.error = Some(error);
                self.ports.abort = None;
            }
        }

        Task::none()
    }
}
