use iced::widget::canvas;
use iced::widget::canvas::{Path, Stroke};
use iced::widget::{Space, column, progress_bar, row, rule, text};
use iced::{Alignment, Color, Element, Fill, Length, Rectangle, Renderer, Theme};

use crate::app::Message;
use crate::theme;

pub fn page_header<'a>(
    title: &'a str,
    subtitle: &'a str,
    actions: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut action_row = row![].spacing(8).align_y(Alignment::Center);

    for action in actions {
        action_row = action_row.push(action);
    }

    column![
        row![
            column![
                text(title).size(22).font(theme::BOLD).color(theme::TEXT),
                text(subtitle).size(12).color(theme::MUTED),
            ]
            .spacing(2),
            Space::new().width(Fill),
            action_row,
        ]
        .align_y(Alignment::Center),
        rule::horizontal(1),
    ]
    .spacing(14)
    .into()
}

pub fn section_title<'a>(title: &'a str, hint: Option<&'a str>) -> Element<'a, Message> {
    match hint {
        Some(hint) => column![
            text(title).size(13).font(theme::BOLD).color(theme::TEXT),
            text(hint.to_owned()).size(11).color(theme::MUTED),
        ]
        .spacing(2)
        .into(),
        None => text(title)
            .size(13)
            .font(theme::BOLD)
            .color(theme::TEXT)
            .into(),
    }
}

pub fn field<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    row![
        text(label)
            .size(12)
            .color(theme::MUTED)
            .width(Length::Fixed(168.0)),
        text(value)
            .size(12)
            .color(theme::TEXT)
            .font(theme::MONO)
            .width(Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

pub fn message_line<'a>(message: impl Into<String>, color: Color) -> Element<'a, Message> {
    text(message.into())
        .size(12)
        .color(color)
        .width(Fill)
        .into()
}

pub fn empty_state<'a>(title: &'a str, body: &'a str) -> Element<'a, Message> {
    column![
        text(title).size(13).font(theme::BOLD).color(theme::TEXT),
        text(body).size(12).color(theme::MUTED),
    ]
    .spacing(4)
    .padding([16, 0])
    .into()
}

pub fn progress(value: f32) -> Element<'static, Message> {
    progress_bar(0.0..=100.0, value.clamp(0.0, 100.0))
        .girth(4)
        .into()
}

pub fn line_chart(samples: Vec<f64>, color: Color, height: f32) -> Element<'static, Message> {
    canvas(LineChart {
        samples,
        color,
        height,
    })
    .width(Fill)
    .height(height)
    .into()
}

struct LineChart {
    samples: Vec<f64>,
    color: Color,
    height: f32,
}

impl<Message> canvas::Program<Message> for LineChart {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let height = bounds.height.max(self.height);

        for index in 0..=2 {
            let y = 8.0 + (height - 16.0) * index as f32 / 2.0;
            frame.stroke(
                &Path::line(iced::Point::new(0.0, y), iced::Point::new(width, y)),
                Stroke::default()
                    .with_color(Color {
                        a: 0.28,
                        ..theme::LINE
                    })
                    .with_width(1.0),
            );
        }

        let samples: Vec<f64> = self
            .samples
            .iter()
            .copied()
            .filter(|value| value.is_finite())
            .rev()
            .take(160)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        if samples.len() < 2 {
            frame.fill_text(canvas::Text {
                content: "No data yet".to_owned(),
                position: iced::Point::new(width / 2.0, height / 2.0),
                color: theme::FAINT,
                size: 11.0.into(),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..Default::default()
            });

            return vec![frame.into_geometry()];
        }

        let raw_min = samples.iter().copied().fold(f64::INFINITY, f64::min);
        let raw_max = samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        let (axis_min, axis_max) = if raw_max <= 0.0 {
            let padding = ((raw_max - raw_min).abs() * 0.10).max(1.0);
            (raw_min - padding, raw_max + padding)
        } else {
            (raw_min.min(0.0), raw_max.max(1.0) * 1.08)
        };

        let range = (axis_max - axis_min).max(0.000_001);
        let inner_width = (width - 4.0).max(1.0);
        let inner_height = (height - 16.0).max(1.0);
        let step = inner_width / (samples.len() - 1) as f32;

        let point = |index: usize, value: f64| {
            iced::Point::new(
                2.0 + step * index as f32,
                8.0 + inner_height * (1.0 - ((value - axis_min) / range) as f32),
            )
        };

        let line = Path::new(|builder| {
            builder.move_to(point(0, samples[0]));

            for (index, value) in samples.iter().enumerate().skip(1) {
                builder.line_to(point(index, *value));
            }
        });

        frame.stroke(
            &line,
            Stroke::default().with_color(self.color).with_width(1.5),
        );

        vec![frame.into_geometry()]
    }
}
