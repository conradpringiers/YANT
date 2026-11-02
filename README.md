# YANT — yet another networking tool

**YANT** is a Rust desktop application that collects everyday network utilities in one fast,
readable interface. It is built with [iced](https://iced.rs) and does not require a browser or a
proprietary service.

## Features

### Dashboard

- Active interface, link type, MAC address, IPv4 address, subnet, gateway and DNS servers.
- Public address with organization and location when available.
- Live receive and transmit rates.
- Current Wi-Fi summary.

### Wi-Fi

- SSID, BSSID, band, channel, frequency, link rate and security.
- Signal level in dBm, quality percentage and live signal history.
- Nearby network scan through NetworkManager (`nmcli`) or `iw`, sorted by signal strength.

### Speed test

- Download and upload through **speed.cloudflare.com** or custom URLs.
- Selectable transfer size: 10, 25, 50 or 100 MB.
- Parallel connections, latency, jitter, live progress and throughput charts.
- Upload measurement can be disabled.

### Ping

- ICMP ping with 4 to 64 packets.
- Minimum, average, maximum, standard deviation, jitter and packet loss.
- Latency chart and detailed reply table with TTL.

### Devices

- CIDR scan of a local subnet (up to /20).
- Parallel ICMP discovery, completed by the ARP cache for devices that block ICMP.
- Hostname resolution and MAC addresses.
- Direct actions: ping a device or scan its ports.

### DNS

- A, AAAA, MX, TXT, NS, CNAME, SOA, SRV, CAA and PTR lookups.
- Uses the system resolver.
- TTL, query latency and copy action for values.

### Ports

- TCP connect scan of a host.
- Presets for web, common, remote-access and database ports, or a custom range.
- Configurable per-port timeout, live progress, service names and latency.

## Installation

Requirements: **Rust 1.88+** (edition 2024).

```bash
git clone <your-repository>/YANT
cd YANT
cargo run --release
```

Start directly on a tab:

```bash
YANT_TAB=wifi cargo run --release
YANT_TAB=devices cargo run --release
```

Accepted values: `dashboard`, `wifi`, `speed`, `latency`, `devices`, `dns`, `ports`.

## System requirements

| Feature | Dependency |
| --- | --- |
| Ping | system `ping` command (`iputils-ping` on Debian/Ubuntu) |
| Wi-Fi | `iw` and/or NetworkManager (`nmcli`) — **Linux** |
| Wi-Fi scan | `nmcli --rescan yes`, or `iw scan` with privileges |
| Interfaces | system API through `network-interface` (Linux, macOS, Windows) |
| DNS | system resolver (`/etc/resolv.conf` on Unix) |

On Linux, `iw scan` usually requires root. NetworkManager is preferred because it works without
elevation. The rest of the application uses `ping` (setuid) and normal TCP connections.

Detailed Wi-Fi information (channel, RSSI, BSSID) is Linux-specific. The other tabs remain usable
on macOS and Windows, with fewer interface details.

## Privacy and data usage

- The public address is fetched from `ipapi.co`, with a fallback to `api.ipify.org`.
- The speed test contacts `speed.cloudflare.com` by default. URLs can be changed in the interface.
- A 25 MB test transfers roughly 25 MB in each direction.
- No telemetry and no account are required.

## Architecture

```text
src/
├── main.rs              iced entry point
├── app.rs               Global state, messages, update logic, tasks
├── theme.rs             Restrained dark palette
├── util.rs              Formatting, CIDR/port parsing, commands, helpers
├── net/
│   ├── interfaces.rs    Interfaces, counters, gateways, DNS
│   ├── wifi.rs          iw/nmcli parsing and network scan
│   ├── ping.rs          Streaming system ping and statistics
│   ├── speed.rs         Streaming HTTP throughput test
│   ├── scan.rs          ICMP + ARP host discovery
│   ├── arp.rs           Linux ARP cache
│   ├── dns.rs           Hickory-based DNS lookups
│   ├── ports.rs         TCP connect scan
│   └── public_ip.rs     Public address and geolocation
└── ui/
    ├── mod.rs           Navigation, headers, sidebar
    ├── components.rs    Fields, tables, line charts
    └── *.rs             One view per tab
```

Long-running operations run in the background through `Task::stream` and asynchronous channels.
Each run has an identifier and an abort handle, preventing stale results and allowing a scan or
test to be stopped.

## Tests

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Four network integration tests are ignored by default (they require Internet or the `ping`
command):

```bash
cargo test -- --ignored
```

They cover a real DNS lookup, a ping to `1.1.1.1`, a small 2 MB speed test and a scan of
`127.0.0.1/32`.

## Known limitations

- Devices that block ICMP may only appear through the ARP cache, which can be incomplete.
- A port scan can be logged by the target. Only test systems you are authorized to test.
- Throughput depends on the test server and network load; a single test does not characterize a
  connection.
- On Linux, a `MESA: error: Opening /dev/dri/cardN failed` message may appear when no GPU is
  accessible; iced then falls back to software rendering.

## License

MIT — see [LICENSE](LICENSE).
