mod app;
mod draw;
mod text;

use relm4::{RelmApp, gtk};

pub const APP_ID: &str = "io.foxiq.FoxRes";

fn main() {
    let app = RelmApp::new(APP_ID);
    gtk::Window::set_default_icon_name(APP_ID);
    app.run::<app::App>(());
}
