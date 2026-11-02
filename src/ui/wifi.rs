use iced::widget::{Space, column, container, pick_list, row, rule, text};
use iced::{Element, Fill, Length};

use super::components::*;
use crate::app::{App, Message};
use crate::net::wifi;
use crate::theme;
use crate::util;

pub fn view(app: &App) -> Element<'_, Message> {
    let scan_action: Element<'_, Message> = if app.wifi.scanning {
        text("Scanning…").size(12).color(theme::MUTED).into()
    } else {
        super::primary_button("Scan networks", Message::WifiScan)
    };

    let mut content = column![super::page_header(
        "Wi-Fi",
        "Signal, channel, band and nearby networks",
        vec![scan_action, super::refresh_button()],
    )]
    .spacing(18);

    if app.wifi.interfaces.len() > 1 {
        content = content.push(
            row![
                text("Interface").size(12).color(theme::MUTED),
                pick_list(
                    &app.wifi.interfaces[..],
                    app.wifi.selected.clone(),
                    Message::WifiInterface,
                )
                .padding([5, 8]),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        );
    }

    if let Some(info) = app.wifi.info.as_ref().filter(|info| info.connected) {
        content = content.push(section_title("Connection", None));
        content = content.push(connection_details(info));
        content = content.push(rule::horizontal(1));
        content = content.push(section_title("Signal history", None));

        if app.wifi.signal_history.len() > 1 {
            content = content.push(line_chart(
                app.wifi.signal_history.clone(),
                theme::ACCENT,
                96.0,
            ));
        } else {
            content = content.push(message_line("Collecting samples…", theme::MUTED));
        }
    } else if let Some(error) = &app.wifi.error {
        content = content.push(message_line(error.clone(), theme::RED));
    } else if app.wifi.interfaces.is_empty() {
        content = content.push(empty_state(
            "No Wi-Fi interface",
            "YANT did not detect a wireless interface on this machine.",
        ));
    } else {
        content = content.push(empty_state(
            "Not connected",
            "Join a wireless network, or scan for nearby networks.",
        ));
    }

    content = content.push(rule::horizontal(1));
    content = content.push(section_title("Nearby networks", None));

    if let Some(error) = &app.wifi.scan_error {
        content = content.push(message_line(error.clone(), theme::RED));
    }

    content = content.push(network_table(app));

    content.into()
}

fn connection_details(info: &wifi::WifiInfo) -> Element<'_, Message> {
    let quality = info.quality_or_default();
    let snr = match (info.signal_dbm, info.noise_dbm) {
        (Some(signal), Some(noise)) => format!("{} dB", signal - noise),
        _ => "—".to_owned(),
    };

    row![
        column![
            field(
                "Network name",
                info.ssid.clone().unwrap_or_else(|| "(hidden)".to_owned())
            ),
            field(
                "BSSID",
                info.bssid.clone().unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Signal",
                info.signal_dbm
                    .map(|dbm| format!("{dbm} dBm"))
                    .unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Quality",
                format!("{quality}% · {}", util::quality_label(quality))
            ),
            field(
                "Band",
                info.frequency_mhz
                    .map(|frequency| format!("{} ({frequency} MHz)", util::band_label(frequency)))
                    .unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Channel",
                info.channel
                    .map(|channel| channel.to_string())
                    .unwrap_or_else(|| "—".to_owned())
            ),
        ]
        .spacing(7)
        .width(Fill),
        Space::new().width(32),
        column![
            field(
                "Link rate",
                info.bitrate_mbps
                    .map(util::format_mbps)
                    .unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "TX power",
                info.tx_power_dbm
                    .map(|power| format!("{power:.1} dBm"))
                    .unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Security",
                info.security.clone().unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Noise",
                info.noise_dbm
                    .map(|noise| format!("{noise} dBm"))
                    .unwrap_or_else(|| "—".to_owned())
            ),
            field("Signal / noise", snr),
            field(
                "Interface",
                info.interface.clone().unwrap_or_else(|| "—".to_owned())
            ),
        ]
        .spacing(7)
        .width(Fill),
    ]
    .spacing(0)
    .into()
}

fn network_table(app: &App) -> Element<'_, Message> {
    let header = container(
        row![
            super::header_label_fill("Network"),
            super::header_label("Signal", Length::Fixed(80.0)),
            super::header_label("Quality", Length::Fixed(70.0)),
            super::header_label("Channel", Length::Fixed(60.0)),
            super::header_label("Band", Length::Fixed(70.0)),
            super::header_label("Security", Length::Fixed(120.0)),
            super::header_label("BSSID", Length::Fixed(150.0)),
        ]
        .spacing(12),
    )
    .padding([5, 0])
    .width(Fill);

    let mut table = column![header, rule::horizontal(1)].spacing(0);

    if app.wifi.networks.is_empty() {
        table = table.push(
            container(
                text(if app.wifi.scanning {
                    "Scanning for networks…"
                } else {
                    "No scan results. Run a scan to list nearby networks."
                })
                .size(12)
                .color(theme::MUTED),
            )
            .padding([14, 0]),
        );

        return table.into();
    }

    for network in &app.wifi.networks {
        let quality = network.quality;
        let ssid = text(network.ssid.clone())
            .size(12)
            .color(if network.in_use {
                theme::ACCENT
            } else {
                theme::TEXT
            })
            .width(Fill);

        table = table.push(
            container(
                row![
                    container(ssid).width(Fill),
                    container(
                        text(
                            network
                                .signal_dbm
                                .map(|dbm| format!("{dbm} dBm"))
                                .unwrap_or_else(|| "—".to_owned())
                        )
                        .size(12)
                        .font(theme::MONO)
                        .color(theme::TEXT),
                    )
                    .width(Length::Fixed(80.0)),
                    container(
                        text(format!("{quality}%"))
                            .size(12)
                            .font(theme::MONO)
                            .color(theme::MUTED),
                    )
                    .width(Length::Fixed(70.0)),
                    container(
                        text(
                            network
                                .channel
                                .map(|channel| channel.to_string())
                                .unwrap_or_else(|| "—".to_owned())
                        )
                        .size(12)
                        .font(theme::MONO)
                        .color(theme::TEXT),
                    )
                    .width(Length::Fixed(60.0)),
                    container(text(network.band().to_owned()).size(12).color(theme::MUTED))
                        .width(Length::Fixed(70.0)),
                    container(text(network.security.clone()).size(12).color(theme::MUTED))
                        .width(Length::Fixed(120.0)),
                    container(
                        text(network.bssid.clone().unwrap_or_else(|| "—".to_owned()))
                            .size(11)
                            .font(theme::MONO)
                            .color(theme::FAINT),
                    )
                    .width(Length::Fixed(150.0)),
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
