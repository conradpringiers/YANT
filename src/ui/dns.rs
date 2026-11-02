use iced::widget::{Space, column, container, pick_list, row, rule, text, text_input};
use iced::{Alignment, Element, Length};

use super::components::*;
use crate::app::{App, Message};
use crate::net::dns;
use crate::theme;

pub fn view(app: &App) -> Element<'_, Message> {
    let action: Element<'_, Message> = if app.dns.running {
        text("Resolving…").size(12).color(theme::MUTED).into()
    } else {
        super::primary_button("Resolve", Message::DnsStart)
    };

    let mut content = column![super::page_header(
        "DNS",
        "Resolve DNS records",
        vec![action],
    )]
    .spacing(18);

    content = content.push(section_title("Query", None));
    content = content.push(configuration(app));

    if let Some(error) = &app.dns.error {
        content = content.push(message_line(error.clone(), theme::RED));
    }

    content = content.push(rule::horizontal(1));

    if let Some(report) = &app.dns.report {
        content = content.push(section_title("Result", None));
        content = content.push(result(report));
        content = content.push(rule::horizontal(1));
        content = content.push(section_title("Answers", None));
        content = content.push(answers(report));
    } else if !app.dns.running {
        content = content.push(empty_state(
            "No query yet",
            "Enter a domain name, choose a record type, and run the lookup. For PTR, enter an IP address.",
        ));
    }

    content = content.push(message_line(
        "Queries use the system resolver. For international domain names, use the punycode form.",
        theme::FAINT,
    ));

    content.into()
}

fn configuration(app: &App) -> Element<'_, Message> {
    let running = app.dns.running;

    column![
        row![
            text("Name")
                .size(12)
                .color(theme::MUTED)
                .width(Length::Fixed(70.0)),
            text_input("example.com", &app.dns.input)
                .padding(6)
                .size(12)
                .on_input_maybe((!running).then_some(Message::DnsInput))
                .width(iced::Fill),
            pick_list(&dns::Kind::ALL[..], Some(app.dns.kind), Message::DnsKind,)
                .padding([5, 8])
                .width(Length::Fixed(190.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        message_line(
            if app.dns.kind == dns::Kind::Ptr {
                "PTR accepts an IPv4 or IPv6 address."
            } else {
                "Examples: example.com, api.github.com, _sip._tcp.example.com"
            },
            theme::FAINT,
        ),
    ]
    .spacing(7)
    .into()
}

fn result(report: &dns::Report) -> Element<'_, Message> {
    row![
        column![
            field("Query", report.query.clone()),
            field("Type", report.kind.code().to_owned()),
            field("Answers", report.answers.len().to_string()),
        ]
        .spacing(7)
        .width(iced::Fill),
        Space::new().width(32),
        column![
            field("Resolver", report.resolvers.clone()),
            field("Elapsed", format!("{:.1} ms", report.elapsed_ms)),
        ]
        .spacing(7)
        .width(iced::Fill),
    ]
    .spacing(0)
    .into()
}

fn answers(report: &dns::Report) -> Element<'_, Message> {
    let header = container(
        row![
            super::header_label("Type", Length::Fixed(70.0)),
            super::header_label("TTL", Length::Fixed(70.0)),
            super::header_label_fill("Value"),
            super::header_label("", Length::Fixed(54.0)),
        ]
        .spacing(12),
    )
    .padding([5, 0])
    .width(iced::Fill);

    let mut table = column![header, rule::horizontal(1)].spacing(0);

    for answer in &report.answers {
        table = table.push(
            container(
                row![
                    text(answer.record_type.clone())
                        .size(12)
                        .color(theme::ACCENT)
                        .width(Length::Fixed(70.0)),
                    text(format!("{} s", answer.ttl))
                        .size(12)
                        .font(theme::MONO)
                        .color(theme::MUTED)
                        .width(Length::Fixed(70.0)),
                    text(answer.value.clone())
                        .size(12)
                        .font(theme::MONO)
                        .color(theme::TEXT)
                        .wrapping(iced::widget::text::Wrapping::Word)
                        .width(iced::Fill),
                    container(super::text_button(
                        "Copy",
                        Message::Copy(answer.value.clone()),
                    ))
                    .width(Length::Fixed(54.0)),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([7, 0])
            .width(iced::Fill),
        );

        table = table.push(rule::horizontal(1));
    }

    table.into()
}
