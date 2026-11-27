mod app;
mod draw;
mod screenshot;
mod text;
mod tools;

use std::process::ExitCode;

use relm4::{RelmApp, gtk};

pub const APP_ID: &str = "io.foxiq.FoxRes";

fn main() -> ExitCode {
    // Any arguments: behave as the command-line tool. None: open the window.
    if std::env::args_os().len() > 1 {
        return foxres_cli::run(std::env::args_os());
    }
    let app = RelmApp::new(APP_ID);
    gtk::Window::set_default_icon_name(APP_ID);
    app.run::<app::App>(());
    ExitCode::SUCCESS
}
