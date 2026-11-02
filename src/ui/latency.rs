use iced::widget::{Space, column, container, pick_list, row, rule, text, text_input};
use iced::{Alignment, Element, Fill, Length};

use super::components::*;
use crate::app::{App, Message};
use crate::net::ping;
use crate::theme;
use crate::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CountOption(u32);

impl CountOption {
    const ALL: [CountOption; 5] = [
        CountOption(4),
        CountOption(8),
        CountOption(16),
        CountOption(32),
        CountOption(64),
    ];

    fn from_count(count: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|option| option.0 == count)
    }
}

impl std::fmt::Display for CountOption {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} packets", self.0)
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let action = if app.ping.running {
        super::danger_button("Stop", Message::PingStop)
    } else {
        super::primary_button("Start ping", Message::PingStart)
    };

    let mut content = column![super::page_header(
        "Ping",
        "ICMP latency, packet loss and jitter",
        vec![action],
    )]
    .spacing(18);

    content = content.push(section_title("Target", None));
    content = content.push(configuration(app));

    if let Some(error) = &app.ping.error {
        content = content.push(message_line(error.clone(), theme::RED));
    }

    let live = live_stats(
        &app.ping
            .replies
            .iter()
            .map(|reply| reply.rtt_ms)
            .collect::<Vec<_>>(),
        app.ping.count,
    );
    let stats = app.ping.stats.clone().or(live);

    if let Some(stats) = stats {
        content = content.push(rule::horizontal(1));
        content = content.push(section_title("Statistics", None));
        content = content.push(statistics(stats.clone()));

        content = content.push(rule::horizontal(1));
        content = content.push(section_title("Latency over time", None));
        content = content.push(line_chart(
            app.ping.replies.iter().map(|reply| reply.rtt_ms).collect(),
            theme::ACCENT,
            140.0,
        ));

        content = content.push(rule::horizontal(1));
        content = content.push(section_title("Replies", None));
        content = content.push(replies(app));
    } else if !app.ping.running {
        content = content.push(empty_state(
            "No measurements",
            "Enter an IP address or hostname, then start the ping.",
        ));
    }

    content = content.push(message_line(
        "Ping uses the system `ping` command. Some firewalls drop ICMP traffic, so no reply \
         does not always mean the host is offline. On Linux, install iputils-ping.",
        theme::FAINT,
    ));

    content.into()
}

fn configuration(app: &App) -> Element<'_, Message> {
    let running = app.ping.running;

    column![
        row![
            text("Host")
                .size(12)
                .color(theme::MUTED)
                .width(Length::Fixed(70.0)),
            text_input("1.1.1.1", &app.ping.host)
                .padding(6)
                .size(12)
                .on_input_maybe((!running).then_some(Message::PingHost))
                .width(Fill),
            pick_list(
                &CountOption::ALL[..],
                CountOption::from_count(app.ping.count),
                |option| Message::PingCount(option.0),
            )
            .padding([5, 8]),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        row![
            text("Resolved")
                .size(12)
                .color(theme::MUTED)
                .width(Length::Fixed(70.0)),
            text(
                app.ping
                    .resolved
                    .clone()
                    .unwrap_or_else(|| "not resolved yet".to_owned())
            )
            .size(12)
            .font(theme::MONO)
            .color(theme::TEXT),
            Space::new().width(Fill),
            text(if app.ping.running {
                "measurement in progress"
            } else {
                "ICMP"
            })
            .size(11)
            .color(if app.ping.running {
                theme::AMBER
            } else {
                theme::FAINT
            }),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    ]
    .spacing(7)
    .into()
}

fn statistics(stats: ping::Stats) -> Element<'static, Message> {
    row![
        column![
            field("Minimum", util::format_latency(stats.min_ms)),
            field("Average", util::format_latency(stats.avg_ms)),
            field("Maximum", util::format_latency(stats.max_ms)),
            field("Jitter", util::format_latency(stats.jitter_ms)),
        ]
        .spacing(7)
        .width(Fill),
        Space::new().width(32),
        column![
            field("Packet loss", format!("{:.0}%", stats.loss_pct)),
            field("Received", format!("{} / {}", stats.received, stats.sent)),
            field("Standard deviation", util::format_latency(stats.stddev_ms)),
            field("Packets sent", stats.sent.to_string()),
        ]
        .spacing(7)
        .width(Fill),
    ]
    .spacing(0)
    .into()
}

fn replies(app: &App) -> Element<'_, Message> {
    let header = container(
        row![
            super::header_label("Sequence", Length::Fixed(110.0)),
            super::header_label("Round-trip", Length::Fixed(140.0)),
            super::header_label("TTL", Length::Fixed(80.0)),
        ]
        .spacing(12),
    )
    .padding([5, 0])
    .width(Fill);

    let mut table = column![header, rule::horizontal(1)].spacing(0);

    for reply in app.ping.replies.iter().rev() {
        table = table.push(
            container(
                row![
                    text(format!("#{}", reply.seq))
                        .size(12)
                        .font(theme::MONO)
                        .color(theme::MUTED)
                        .width(Length::Fixed(110.0)),
                    text(util::format_latency(reply.rtt_ms))
                        .size(12)
                        .font(theme::MONO)
                        .color(theme::TEXT)
                        .width(Length::Fixed(140.0)),
                    text(
                        reply
                            .ttl
                            .map(|ttl| ttl.to_string())
                            .unwrap_or_else(|| "—".to_owned())
                    )
                    .size(12)
                    .font(theme::MONO)
                    .color(theme::MUTED)
                    .width(Length::Fixed(80.0)),
                ]
                .spacing(12),
            )
            .padding([6, 0])
            .width(Fill),
        );

        table = table.push(rule::horizontal(1));
    }

    table.into()
}

fn live_stats(samples: &[f64], expected: u32) -> Option<ping::Stats> {
    if samples.is_empty() || expected == 0 {
        return None;
    }

    let received = samples.len() as u32;
    let sum = samples.iter().sum::<f64>();
    let avg = sum / samples.len() as f64;
    let min = samples.iter().copied().fold(f64::INFINITY, f64::min);
    let max = samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let variance = samples
        .iter()
        .map(|sample| {
            let delta = sample - avg;
            delta * delta
        })
        .sum::<f64>()
        / samples.len() as f64;
    let jitter = if samples.len() > 1 {
        samples
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .sum::<f64>()
            / (samples.len() - 1) as f64
    } else {
        0.0
    };

    Some(ping::Stats {
        sent: expected,
        received,
        loss_pct: (expected.saturating_sub(received) as f64 / expected as f64) * 100.0,
        min_ms: min,
        avg_ms: avg,
        max_ms: max,
        jitter_ms: jitter,
        stddev_ms: variance.sqrt(),
    })
}
