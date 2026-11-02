use iced::widget::{Space, column, row, rule, text};
use iced::{Element, Fill};

use super::components::*;
use crate::app::{App, Message, Tab};
use crate::net::interfaces;
use crate::theme;
use crate::util;

pub fn view(app: &App) -> Element<'_, Message> {
    let primary = interfaces::primary(&app.interfaces);
    let online = primary.is_some_and(|interface| interface.is_up);
    let rate = primary
        .and_then(|interface| app.rates.get(&interface.name))
        .copied()
        .unwrap_or_default();

    let mut content = column![super::page_header(
        "Dashboard",
        Tab::Dashboard.subtitle(),
        vec![super::refresh_button()],
    )]
    .spacing(18);

    content = content.push(section_title("Connection", None));

    if let Some(interface) = primary {
        content = content.push(
            row![
                column![
                    field(
                        "Status",
                        if online {
                            "Online".to_owned()
                        } else {
                            "Offline".to_owned()
                        }
                    ),
                    field("Interface", interface.name.clone()),
                    field("Type", interface.kind().to_owned()),
                    field(
                        "MAC address",
                        interface.mac.clone().unwrap_or_else(|| "—".to_owned())
                    ),
                    field(
                        "IPv4 address",
                        interface
                            .ipv4
                            .map(|ip| ip.to_string())
                            .unwrap_or_else(|| "—".to_owned())
                    ),
                    field("Subnet", interface.cidr().unwrap_or_else(|| "—".to_owned())),
                ]
                .spacing(7)
                .width(Fill),
                Space::new().width(32),
                column![
                    field(
                        "Gateway",
                        interface
                            .gateway
                            .map(|ip| ip.to_string())
                            .unwrap_or_else(|| "—".to_owned())
                    ),
                    field(
                        "DNS servers",
                        if interface.dns.is_empty() {
                            "—".to_owned()
                        } else {
                            interface.dns.join(", ")
                        }
                    ),
                    field(
                        "Link speed",
                        interface
                            .link_speed_mbps
                            .map(|speed| format!("{speed} Mbit/s"))
                            .unwrap_or_else(|| "Unknown".to_owned())
                    ),
                    field("Download rate", util::format_rate(rate.rx)),
                    field("Upload rate", util::format_rate(rate.tx)),
                    field(
                        "Interface state",
                        if online {
                            "Up".to_owned()
                        } else {
                            "Down".to_owned()
                        }
                    ),
                ]
                .spacing(7)
                .width(Fill),
            ]
            .spacing(0),
        );
    } else {
        content = content.push(empty_state(
            "No active connection",
            "Connect an Ethernet cable or join a Wi-Fi network.",
        ));
    }

    content = content.push(rule::horizontal(1));
    content = content.push(section_title("Public address", None));
    content = content.push(public_address(app));

    content = content.push(rule::horizontal(1));
    content = content.push(section_title("Wi-Fi", None));
    content = content.push(wifi_summary(app));

    content.into()
}

fn public_address(app: &App) -> Element<'_, Message> {
    let Some(info) = app.public_ip.value.as_ref() else {
        let message = if app.public_ip.loading {
            "Fetching public address…".to_owned()
        } else {
            app.public_ip
                .error
                .clone()
                .unwrap_or_else(|| "Public address unavailable".to_owned())
        };

        return message_line(message, theme::MUTED);
    };

    let location = [info.city.clone(), info.region.clone(), info.country.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");

    row![
        column![
            field("IPv4 / IPv6", info.ip.clone()),
            field(
                "Organization",
                info.org.clone().unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Location",
                if location.is_empty() {
                    "—".to_owned()
                } else {
                    location
                }
            ),
            field(
                "Timezone",
                info.timezone.clone().unwrap_or_else(|| "—".to_owned())
            ),
            Space::new().height(4),
            super::secondary_button("Copy address", Message::Copy(info.ip.clone())),
        ]
        .spacing(7)
        .width(Fill),
    ]
    .into()
}

fn wifi_summary(app: &App) -> Element<'_, Message> {
    let Some(info) = app.wifi.info.as_ref().filter(|info| info.connected) else {
        return column![
            text("Not connected").size(12).color(theme::MUTED),
            Space::new().height(4),
            super::secondary_button("Open Wi-Fi", Message::Tab(Tab::Wifi)),
        ]
        .spacing(2)
        .into();
    };

    let quality = info.quality_or_default();
    let signal = info
        .signal_dbm
        .map(|dbm| format!("{dbm} dBm"))
        .unwrap_or_else(|| "—".to_owned());

    row![
        column![
            field(
                "Network",
                info.ssid.clone().unwrap_or_else(|| "(hidden)".to_owned())
            ),
            field("Signal", signal),
            field(
                "Quality",
                format!("{quality}% · {}", util::quality_label(quality))
            ),
            field(
                "Band / channel",
                format!(
                    "{} · channel {}",
                    info.band(),
                    info.channel
                        .map(|channel| channel.to_string())
                        .unwrap_or_else(|| "—".to_owned())
                )
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
                "Security",
                info.security.clone().unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "BSSID",
                info.bssid.clone().unwrap_or_else(|| "—".to_owned())
            ),
            field(
                "Frequency",
                info.frequency_mhz
                    .map(|frequency| format!("{frequency} MHz"))
                    .unwrap_or_else(|| "—".to_owned())
            ),
        ]
        .spacing(7)
        .width(Fill),
    ]
    .spacing(0)
    .into()
}
