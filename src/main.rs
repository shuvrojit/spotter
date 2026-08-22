mod ai;
mod config;
mod history;
mod platform;
mod readline;
mod search;
mod settings;
mod tray;
mod ui;

use gtk::{prelude::*, Application};

const APP_ID: &str = "dev.spotter.Launcher";
const PRODUCT_NAME: &str = "Spotter";

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        if let Some(window) = app
            .windows()
            .into_iter()
            .find(|window| window.title().as_deref() == Some(PRODUCT_NAME))
        {
            platform::present(&window);
        } else {
            ui::build(app);
        }
    });
    app.run();
}
