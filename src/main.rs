mod ai;
mod config;
mod platform;
mod search;
mod ui;

use gtk::{prelude::*, Application};

const APP_ID: &str = "dev.spotter.Launcher";
const PRODUCT_NAME: &str = "Spotter";

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(ui::build);
    app.run();
}
