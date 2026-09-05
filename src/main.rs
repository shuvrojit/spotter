mod ai;
mod config;
mod history;
mod platform;
mod readline;
mod search;
mod settings;
mod tray;
mod ui;

use gtk::{gio, glib, prelude::*, Application};

const APP_ID: &str = "dev.spotter.Launcher";
const PRODUCT_NAME: &str = "Spotter";

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.add_main_option(
        "toggle",
        glib::Char(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Show or hide the launcher without stopping the system tray",
        None,
    );
    app.connect_activate(|app| ui::activate(app, false));
    // GApplication forwards each invocation to the existing process, so a
    // hidden tray instance receives the command instead of being restarted.
    app.connect_command_line(|app, command_line| {
        ui::activate(app, command_line.options_dict().contains("toggle"));
        0
    });
    app.run()
}
