use iced::widget::{Space, column, container, row, rule, text, text_input};
use iced::{Alignment, Element, Fill, Length};

use super::components::*;
use crate::app::{App, Message};
use crate::net::interfaces;
use crate::theme;
use crate::util;

pub fn view(app: &App) -> Element<'_, Message> {
    let action = if app.scan.running {
        super::danger_button("Stop", Message::ScanStop)
    } else {
        super::primary_button("Scan network", Message::ScanStart)
    };

    let mut content = column![super::page_header(
        "Devices",
        "Discover hosts on the local network",
        vec![action],
    )]
    .spacing(18);

    content = content.push(section_title("Subnet", None));
    content = content.push(configuration(app));

    if let Some(error) = &app.scan.error {
        content = content.push(message_line(error.clone(), theme::RED));
    }

    if app.scan.running {
        content = content.push(rule::horizontal(1));
        content = content.push(section_title("Progress", None));
        content = content.push(progress_details(app));
    }

    content = content.push(rule::horizontal(1));
    content = content.push(section_title("Devices", None));

    if app.scan.devices.is_empty() {
        if !app.scan.running && app.scan.error.is_none() {
            content = content.push(empty_state(
                "No devices found",
                "Start a scan to detect hosts. Devices that block ICMP are added from the ARP cache.",
            ));
        }
    } else {
        content = content.push(device_table(app));
    }

    content = content.push(message_line(
        "Discovery combines ICMP ping with the system ARP cache. Devices that block ICMP may \
         not appear. Only scan networks you are responsible for.",
        theme::FAINT,
    ));

    content.into()
}

fn configuration(app: &App) -> Element<'_, Message> {
    let running = app.scan.running;
    let host_count = util::parse_subnet(&app.scan.subnet)
        .ok()
        .map(|(network, prefix)| util::hosts_in_subnet(network, prefix).len())
        .unwrap_or(0);
    let primary = interfaces::primary(&app.interfaces);
    let gateway = primary
        .and_then(|interface| interface.gateway)
        .map(|gateway| gateway.to_string())
        .unwrap_or_else(|| "—".to_owned());
    let local_ip = primary
        .and_then(|interface| interface.ipv4)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "—".to_owned());

    column![
        row![
            text("CIDR")
                .size(12)
                .color(theme::MUTED)
                .width(Length::Fixed(70.0)),
            text_input("192.168.1.0/24", &app.scan.subnet)
                .padding(6)
                .size(12)
                .on_input_maybe((!running).then_some(Message::ScanSubnet))
                .width(Fill),
            text(if host_count > 0 {
                format!("{host_count} hosts")
            } else {
                "invalid subnet".to_owned()
            })
            .size(11)
            .color(if host_count > 0 {
                theme::MUTED
            } else {
                theme::AMBER
            }),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        row![
            field("Gateway", gateway),
            Space::new().width(24),
            field("This host", local_ip),
        ]
        .spacing(0),
        message_line(
            "ICMP sweep with 48 concurrent probes, completed by the ARP cache.",
            theme::FAINT,
        ),
    ]
    .spacing(7)
    .into()
}

fn progress_details(app: &App) -> Element<'_, Message> {
    let percentage = if app.scan.total == 0 {
        0.0
    } else {
        (app.scan.done as f32 / app.scan.total as f32 * 100.0).clamp(0.0, 100.0)
    };

    column![
        row![
            field("Tested", format!("{} / {}", app.scan.done, app.scan.total)),
            Space::new().width(24),
            field("Found", app.scan.devices.len().to_string()),
        ]
        .spacing(0),
        progress(percentage),
    ]
    .spacing(7)
    .into()
}

fn device_table(app: &App) -> Element<'_, Message> {
    let header = container(
        row![
            super::header_label("Address", Length::Fixed(120.0)),
            super::header_label_fill("Hostname"),
            super::header_label("MAC", Length::Fixed(155.0)),
            super::header_label("RTT", Length::Fixed(70.0)),
            super::header_label("Role", Length::Fixed(100.0)),
            super::header_label("Actions", Length::Fixed(110.0)),
        ]
        .spacing(12),
    )
    .padding([5, 0])
    .width(Fill);

    let mut table = column![header, rule::horizontal(1)].spacing(0);

    for device in &app.scan.devices {
        let ip = device.ip.to_string();
        let role = if device.is_gateway {
            "Gateway"
        } else if device.is_self {
            "This device"
        } else {
            "—"
        };

        table = table.push(
            container(
                row![
                    text(ip.clone())
                        .size(12)
                        .font(theme::MONO)
                        .color(theme::TEXT)
                        .width(Length::Fixed(120.0)),
                    text(device.hostname.clone().unwrap_or_else(|| "—".to_owned()))
                        .size(12)
                        .color(theme::MUTED)
                        .width(Fill),
                    text(device.mac.clone().unwrap_or_else(|| "—".to_owned()))
                        .size(11)
                        .font(theme::MONO)
                        .color(theme::MUTED)
                        .width(Length::Fixed(155.0)),
                    text(
                        device
                            .rtt_ms
                            .map(util::format_latency)
                            .unwrap_or_else(|| "ARP".to_owned())
                    )
                    .size(12)
                    .font(theme::MONO)
                    .color(if device.rtt_ms.is_some() {
                        theme::TEXT
                    } else {
                        theme::FAINT
                    })
                    .width(Length::Fixed(70.0)),
                    text(role)
                        .size(11)
                        .color(theme::MUTED)
                        .width(Length::Fixed(100.0)),
                    row![
                        super::text_button("Ping", Message::PingDevice(ip.clone())),
                        super::text_button("Ports", Message::ScanPortsFor(ip.clone())),
                    ]
                    .spacing(2)
                    .width(Length::Fixed(110.0)),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([7, 0])
            .width(Fill),
        );

        table = table.push(rule::horizontal(1));
    }

    table.into()
}
