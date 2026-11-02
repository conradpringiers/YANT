mod components;
mod dashboard;
mod devices;
mod dns;
mod latency;
mod ports;
mod speed;
mod wifi;

use iced::widget::{Space, button, column, container, row, rule, scrollable, text};
use iced::{Element, Fill, Length};

use crate::app::{App, Message, Tab};
use crate::net::interfaces;
use crate::theme;

pub use components::*;

pub fn view(app: &App) -> Element<'_, Message> {
    let body = match app.tab {
        Tab::Dashboard => dashboard::view(app),
        Tab::Wifi => wifi::view(app),
        Tab::Speed => speed::view(app),
        Tab::Latency => latency::view(app),
        Tab::Devices => devices::view(app),
        Tab::Dns => dns::view(app),
        Tab::Ports => ports::view(app),
    };

    row![
        sidebar(app),
        rule::vertical(1),
        container(
            scrollable(container(body).padding(24))
                .height(Fill)
                .width(Fill)
        )
        .width(Fill)
        .height(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let mut navigation = column![].spacing(2);

    for tab in Tab::ALL {
        let active = app.tab == tab;

        navigation = navigation.push(
            button(
                text(tab.label())
                    .size(12)
                    .font(if active {
                        theme::BOLD
                    } else {
                        iced::Font::DEFAULT
                    })
                    .color(if active { theme::ACCENT } else { theme::MUTED }),
            )
            .width(Fill)
            .padding([6, 8])
            .style(theme::nav(active))
            .on_press(Message::Tab(tab)),
        );
    }

    let primary = interfaces::primary(&app.interfaces);
    let online = primary.is_some_and(|interface| interface.is_up);
    let connection = primary
        .map(|interface| {
            format!(
                "{} · {}",
                interface.name,
                interface
                    .ipv4
                    .map(|ip| ip.to_string())
                    .unwrap_or_else(|| "no address".to_owned())
            )
        })
        .unwrap_or_else(|| "No interface".to_owned());
    let public_ip = app
        .public_ip
        .value
        .as_ref()
        .map(|info| info.ip.clone())
        .unwrap_or_else(|| "—".to_owned());

    container(
        column![
            column![
                text("YANT").size(18).font(theme::BOLD).color(theme::TEXT),
                text("networking tools").size(10).color(theme::FAINT),
            ]
            .spacing(1),
            Space::new().height(26),
            navigation,
            Space::new().height(Fill),
            column![
                text(if online { "Connected" } else { "Offline" })
                    .size(11)
                    .font(theme::BOLD)
                    .color(if online { theme::GREEN } else { theme::RED }),
                text(connection).size(10).color(theme::MUTED),
                text(public_ip)
                    .size(10)
                    .color(theme::FAINT)
                    .font(theme::MONO),
            ]
            .spacing(3),
        ]
        .spacing(0),
    )
    .width(196)
    .height(Fill)
    .padding([20, 16])
    .into()
}

pub fn refresh_button<'a>() -> Element<'a, Message> {
    button(text("Refresh").size(12))
        .padding([6, 12])
        .style(button::secondary)
        .on_press(Message::Refresh)
        .into()
}

pub fn primary_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    button(text(label).size(12))
        .padding([6, 12])
        .style(button::primary)
        .on_press(message)
        .into()
}

pub fn secondary_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    button(text(label).size(12))
        .padding([6, 12])
        .style(button::secondary)
        .on_press(message)
        .into()
}

pub fn danger_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    button(text(label).size(12))
        .padding([6, 12])
        .style(button::danger)
        .on_press(message)
        .into()
}

pub fn text_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    button(text(label).size(11))
        .padding([2, 4])
        .style(button::text)
        .on_press(message)
        .into()
}

pub fn header_label<'a>(label: &'a str, width: impl Into<Length>) -> Element<'a, Message> {
    text(label).size(11).color(theme::FAINT).width(width).into()
}

pub fn header_label_fill<'a>(label: &'a str) -> Element<'a, Message> {
    text(label).size(11).color(theme::FAINT).width(Fill).into()
}
