use iced::widget::{Space, checkbox, column, pick_list, row, rule, text, text_input};
use iced::{Alignment, Element, Fill, Length};

use super::components::*;
use crate::app::{App, Message};
use crate::net::speed;
use crate::theme;
use crate::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SizeOption {
    Mb10,
    Mb25,
    Mb50,
    Mb100,
}

impl SizeOption {
    const ALL: [SizeOption; 4] = [
        SizeOption::Mb10,
        SizeOption::Mb25,
        SizeOption::Mb50,
        SizeOption::Mb100,
    ];

    fn mb(self) -> u64 {
        match self {
            Self::Mb10 => 10,
            Self::Mb25 => 25,
            Self::Mb50 => 50,
            Self::Mb100 => 100,
        }
    }

    fn from_mb(size_mb: u64) -> Option<Self> {
        Self::ALL.into_iter().find(|option| option.mb() == size_mb)
    }
}

impl std::fmt::Display for SizeOption {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} MB", self.mb())
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let action = if app.speed.running {
        super::danger_button("Stop", Message::SpeedStop)
    } else {
        super::primary_button("Start test", Message::SpeedStart)
    };

    let mut content = column![super::page_header(
        "Speed test",
        "Download and upload throughput",
        vec![action],
    )]
    .spacing(18);

    content = content.push(section_title("Configuration", None));
    content = content.push(configuration(app));

    if let Some(error) = &app.speed.error {
        content = content.push(message_line(error.clone(), theme::RED));
    }

    content = content.push(rule::horizontal(1));

    if app.speed.running {
        content = content.push(section_title("Test in progress", None));
        content = content.push(running(app));
    } else if let Some(report) = &app.speed.result {
        content = content.push(section_title("Result", None));
        content = content.push(result(report));
        content = content.push(rule::horizontal(1));
        content = content.push(section_title("Throughput over time", None));
        content = content.push(charts(app));
    } else {
        content = content.push(empty_state(
            "No test yet",
            "Choose a size and start the test. Download and upload throughput are measured separately.",
        ));
    }

    content = content.push(message_line(
        "The default endpoint is speed.cloudflare.com. Change the URLs to use another test server. \
         The selected size is transferred in each direction.",
        theme::FAINT,
    ));

    content.into()
}

fn configuration(app: &App) -> Element<'_, Message> {
    let running = app.speed.running;

    let mut content = column![
        text("Download URL").size(11).color(theme::MUTED),
        text_input("https://speed.cloudflare.com/__down", &app.speed.url)
            .padding(6)
            .size(12)
            .on_input_maybe((!running).then_some(Message::SpeedUrl)),
        text("Upload URL").size(11).color(theme::MUTED),
        text_input("https://speed.cloudflare.com/__up", &app.speed.upload_url)
            .padding(6)
            .size(12)
            .on_input_maybe((!running).then_some(Message::SpeedUploadUrl)),
        row![
            text("Test size")
                .size(12)
                .color(theme::MUTED)
                .width(Length::Fixed(80.0)),
            pick_list(
                &SizeOption::ALL[..],
                SizeOption::from_mb(app.speed.size_mb),
                |option| Message::SpeedSize(option.mb()),
            )
            .padding([5, 8]),
            Space::new().width(20),
            checkbox(app.speed.include_upload)
                .label("Measure upload")
                .on_toggle_maybe((!running).then_some(Message::SpeedUpload)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        message_line(
            "Upload requires an endpoint that accepts POST requests.",
            theme::FAINT,
        ),
    ]
    .spacing(6);

    if running {
        content = content.push(message_line(
            "Stop the test before changing the configuration.",
            theme::AMBER,
        ));
    }

    content.into()
}

fn running(app: &App) -> Element<'_, Message> {
    let phase = app
        .speed
        .phase
        .map(speed::Phase::label)
        .unwrap_or("Preparing");
    let progress_data = app.speed.progress.as_ref();
    let instant = progress_data
        .map(|progress| progress.instant_mbps)
        .unwrap_or(0.0);
    let percentage = progress_data
        .map(|progress| {
            if progress.total == 0 {
                0.0
            } else {
                (progress.bytes as f64 / progress.total as f64 * 100.0).clamp(0.0, 100.0) as f32
            }
        })
        .unwrap_or(0.0);
    let samples = match app.speed.phase {
        Some(speed::Phase::Upload) => &app.speed.upload_samples,
        _ => &app.speed.download_samples,
    };

    let mut details = column![
        field("Phase", phase.to_owned()),
        field("Instantaneous rate", util::format_mbps(instant)),
        field(
            "Transferred",
            progress_data
                .map(|progress| format!(
                    "{} / {}",
                    util::format_bytes(progress.bytes as f64),
                    util::format_bytes(progress.total as f64)
                ))
                .unwrap_or_else(|| "—".to_owned())
        ),
        field(
            "Elapsed",
            progress_data
                .map(|progress| format!("{:.1} s", progress.elapsed_s))
                .unwrap_or_else(|| "0.0 s".to_owned())
        ),
        field("Progress", format!("{percentage:.0}%")),
    ]
    .spacing(7);

    details = details.push(progress(percentage));
    details = details.push(line_chart(samples.clone(), theme::ACCENT, 120.0));

    details.into()
}

fn result(report: &speed::Report) -> Element<'_, Message> {
    row![
        column![
            field(
                "Download",
                report
                    .download_mbps
                    .map(util::format_mbps)
                    .unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Upload",
                report
                    .upload_mbps
                    .map(util::format_mbps)
                    .unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Latency",
                report
                    .latency_ms
                    .map(util::format_latency)
                    .unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Jitter",
                report
                    .jitter_ms
                    .map(util::format_latency)
                    .unwrap_or_else(|| "—".to_owned())
            ),
        ]
        .spacing(7)
        .width(Fill),
        Space::new().width(32),
        column![
            field(
                "Downloaded",
                util::format_bytes(report.bytes_downloaded as f64)
            ),
            field("Uploaded", util::format_bytes(report.bytes_uploaded as f64)),
            field("Duration", format!("{:.1} s", report.duration_s)),
        ]
        .spacing(7)
        .width(Fill),
    ]
    .spacing(0)
    .into()
}

fn charts(app: &App) -> Element<'_, Message> {
    let mut content = column![].spacing(12);

    if !app.speed.download_samples.is_empty() {
        content = content.push(
            column![
                text("Download").size(11).color(theme::MUTED),
                line_chart(app.speed.download_samples.clone(), theme::ACCENT, 110.0),
            ]
            .spacing(2),
        );
    }

    if !app.speed.upload_samples.is_empty() {
        content = content.push(
            column![
                text("Upload").size(11).color(theme::MUTED),
                line_chart(app.speed.upload_samples.clone(), theme::GREEN, 110.0),
            ]
            .spacing(2),
        );
    }

    content.into()
}
