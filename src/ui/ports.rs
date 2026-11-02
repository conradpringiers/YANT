use iced::widget::{Space, column, container, pick_list, row, rule, slider, text, text_input};
use iced::{Alignment, Element, Fill, Length};

use super::components::*;
use crate::app::{App, Message};
use crate::net::ports;
use crate::theme;
use crate::util;

pub fn view(app: &App) -> Element<'_, Message> {
    let action = if app.ports.running {
        super::danger_button("Stop", Message::PortsStop)
    } else {
        super::primary_button("Scan ports", Message::PortsStart)
    };

    let mut content = column![super::page_header(
        "Ports",
        "Scan TCP ports on a host",
        vec![action],
    )]
    .spacing(18);

    content = content.push(section_title("Target", None));
    content = content.push(configuration(app));

    if let Some(error) = &app.ports.error {
        content = content.push(message_line(error.clone(), theme::RED));
    }

    if app.ports.running {
        content = content.push(rule::horizontal(1));
        content = content.push(section_title("Progress", None));
        content = content.push(progress_details(app));
    }

    content = content.push(rule::horizontal(1));
    content = content.push(section_title("Open ports", None));

    if !app.ports.open.is_empty() {
        content = content.push(results(app));
    } else if !app.ports.running && app.ports.duration_s.is_some() && app.ports.error.is_none() {
        content = content.push(empty_state(
            "No open ports",
            "None of the tested ports accepted a TCP connection. A firewall may also be filtering traffic.",
        ));
    } else if !app.ports.running && app.ports.duration_s.is_none() {
        content = content.push(empty_state(
            "No scan yet",
            "Choose a host and a port list, then start the scan.",
        ));
    }

    content = content.push(message_line(
        "The scan opens a TCP connection to each port. It may be logged by the target. \
         Only test systems you are authorized to test.",
        theme::FAINT,
    ));

    content.into()
}

fn configuration(app: &App) -> Element<'_, Message> {
    let running = app.ports.running;

    let mut content = column![
        row![
            text("Host")
                .size(12)
                .color(theme::MUTED)
                .width(Length::Fixed(70.0)),
            text_input("127.0.0.1", &app.ports.host)
                .padding(6)
                .size(12)
                .on_input_maybe((!running).then_some(Message::PortsHost))
                .width(Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        row![
            text("Port list")
                .size(12)
                .color(theme::MUTED)
                .width(Length::Fixed(70.0)),
            pick_list(
                &ports::Preset::ALL[..],
                Some(app.ports.preset),
                Message::PortsPreset,
            )
            .padding([5, 8])
            .width(Length::Fixed(210.0)),
            Space::new().width(20),
            text("Timeout").size(12).color(theme::MUTED),
            slider(200.0..=3000.0, app.ports.timeout_ms as f32, |value| {
                Message::PortsTimeout(value.round() as u64)
            },)
            .step(100.0_f32)
            .width(Length::Fixed(180.0)),
            text(format!("{} ms", app.ports.timeout_ms))
                .size(12)
                .font(theme::MONO)
                .color(theme::TEXT),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(7);

    if app.ports.preset == ports::Preset::Custom {
        content = content.push(
            row![
                text("Custom")
                    .size(12)
                    .color(theme::MUTED)
                    .width(Length::Fixed(70.0)),
                text_input("22,80,443,8000-8100", &app.ports.custom)
                    .padding(6)
                    .size(12)
                    .on_input_maybe((!running).then_some(Message::PortsCustom))
                    .width(Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
        content = content.push(message_line(
            "Accepted formats: 22, 80,443, 8000-8100. Maximum 4096 ports.",
            theme::FAINT,
        ));
    }

    content = content.push(message_line(
        "TCP connect scan. Up to 128 probes run in parallel.",
        theme::FAINT,
    ));

    content.into()
}

fn progress_details(app: &App) -> Element<'_, Message> {
    let percentage = if app.ports.total == 0 {
        0.0
    } else {
        (app.ports.done as f32 / app.ports.total as f32 * 100.0).clamp(0.0, 100.0)
    };

    column![
        row![
            field(
                "Resolved",
                app.ports
                    .resolved
                    .clone()
                    .unwrap_or_else(|| "resolving…".to_owned())
            ),
            Space::new().width(24),
            field(
                "Tested",
                format!("{} / {}", app.ports.done, app.ports.total)
            ),
        ]
        .spacing(0),
        row![
            field("Open", app.ports.open.len().to_string()),
            Space::new().width(24),
            field("Progress", format!("{percentage:.0}%")),
        ]
        .spacing(0),
        progress(percentage),
    ]
    .spacing(7)
    .into()
}

fn results(app: &App) -> Element<'_, Message> {
    let header = container(
        row![
            super::header_label("Port", Length::Fixed(70.0)),
            super::header_label("Service", Length::Fixed(180.0)),
            super::header_label("Latency", Length::Fixed(120.0)),
        ]
        .spacing(12),
    )
    .padding([5, 0])
    .width(Fill);

    let mut table = column![header, rule::horizontal(1)].spacing(0);

    for port in &app.ports.open {
        table = table.push(
            container(
                row![
                    text(port.port.to_string())
                        .size(12)
                        .font(theme::MONO)
                        .color(theme::TEXT)
                        .width(Length::Fixed(70.0)),
                    text(port.service)
                        .size(12)
                        .color(theme::MUTED)
                        .width(Length::Fixed(180.0)),
                    text(util::format_latency(port.latency_ms))
                        .size(12)
                        .font(theme::MONO)
                        .color(theme::TEXT)
                        .width(Length::Fixed(120.0)),
                ]
                .spacing(12),
            )
            .padding([7, 0])
            .width(Fill),
        );

        table = table.push(rule::horizontal(1));
    }

    table.into()
}
