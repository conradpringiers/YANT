mod app;
mod net;
mod theme;
mod ui;
mod util;

use iced::Size;

fn main() -> iced::Result {
    iced::application(app::App::boot, app::App::update, app::App::view)
        .title("YANT — yet another networking tool")
        .theme(|app: &app::App| app.theme())
        .subscription(|app: &app::App| app.subscription())
        .style(|_, theme| theme::app_style(theme))
        .window(iced::window::Settings {
            size: Size::new(1220.0, 840.0),
            min_size: Some(Size::new(1040.0, 700.0)),
            ..Default::default()
        })
        .antialiasing(true)
        .run()
}
