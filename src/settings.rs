use crate::config::{self, AiConfig, Config, UiColors, UiConfig};
use gtk::gdk;
use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, ColorDialog, ColorDialogButton,
    DropDown, Entry, Grid, Label, Notebook, Orientation, PasswordEntry, ScrolledWindow, SpinButton,
    Switch, TextView,
};

const SETTINGS_TITLE: &str = "Spotter Settings";
const POSITION_IDS: [&str; 10] = [
    "top-left",
    "top-center",
    "top-right",
    "center-left",
    "center",
    "center-right",
    "bottom-left",
    "bottom-center",
    "bottom-right",
    "custom",
];
const POSITION_LABELS: [&str; 10] = [
    "Top left",
    "Top center",
    "Top right",
    "Center left",
    "Center",
    "Center right",
    "Bottom left",
    "Bottom center",
    "Bottom right",
    "Custom coordinates",
];

#[derive(Clone)]
struct Controls {
    max_results: SpinButton,
    max_recent_items: SpinButton,
    max_recent_searches: SpinButton,
    max_result_height: SpinButton,
    max_indexed_items: SpinButton,
    index_depth: SpinButton,
    include_hidden: Switch,
    include_path_binaries: Switch,
    system_tray: Switch,
    index_dirs: TextView,
    ai_base_url: Entry,
    ai_api_key: PasswordEntry,
    ai_model: Entry,
    position: DropDown,
    x: SpinButton,
    y: SpinButton,
    window_width: SpinButton,
    result_max_height: SpinButton,
    shell_margin: SpinButton,
    shell_padding: SpinButton,
    shell_radius: SpinButton,
    search_height: SpinButton,
    search_radius: SpinButton,
    search_font_size: SpinButton,
    result_margin_top: SpinButton,
    result_row_padding_y: SpinButton,
    result_row_padding_x: SpinButton,
    result_row_radius: SpinButton,
    title_font_size: SpinButton,
    subtitle_font_size: SpinButton,
    icon_font_size: SpinButton,
    window_background: ColorDialogButton,
    shell_background: ColorDialogButton,
    shell_border: ColorDialogButton,
    search_background: ColorDialogButton,
    search_text: ColorDialogButton,
    results_background: ColorDialogButton,
    row_background: ColorDialogButton,
    row_selected_background: ColorDialogButton,
    icon: ColorDialogButton,
    title: ColorDialogButton,
    subtitle: ColorDialogButton,
}

impl Controls {
    fn new(config: &Config) -> Self {
        let color_dialog = ColorDialog::builder()
            .title("Choose a color")
            .with_alpha(true)
            .build();
        let position = DropDown::from_strings(&POSITION_LABELS);
        let selected_position = POSITION_IDS
            .iter()
            .position(|id| *id == config.ui.position)
            .unwrap_or_default();
        position.set_selected(selected_position as u32);
        position.set_hexpand(true);

        let index_dirs = TextView::new();
        index_dirs.buffer().set_text(&config.index_dirs.join("\n"));
        index_dirs.set_monospace(true);
        index_dirs.set_wrap_mode(gtk::WrapMode::None);

        let ai_api_key = PasswordEntry::new();
        ai_api_key.set_text(&config.ai.api_key);
        ai_api_key.set_show_peek_icon(true);
        ai_api_key.set_hexpand(true);

        Self {
            max_results: spin(config.max_results as f64, 1.0, 1_000.0, 1.0),
            max_recent_items: spin(config.max_recent_items as f64, 0.0, 50.0, 1.0),
            max_recent_searches: spin(config.max_recent_searches as f64, 0.0, 50.0, 1.0),
            max_result_height: spin(config.max_result_height as f64, 120.0, 2_160.0, 10.0),
            max_indexed_items: spin(config.max_indexed_items as f64, 1.0, 10_000_000.0, 1_000.0),
            index_depth: spin(config.index_depth as f64, 1.0, 100.0, 1.0),
            include_hidden: toggle(config.include_hidden),
            include_path_binaries: toggle(config.include_path_binaries),
            system_tray: toggle(config.system_tray),
            index_dirs,
            ai_base_url: entry(&config.ai.base_url),
            ai_api_key,
            ai_model: entry(&config.ai.model),
            position,
            x: spin(config.ui.x as f64, -100_000.0, 100_000.0, 1.0),
            y: spin(config.ui.y as f64, -100_000.0, 100_000.0, 1.0),
            window_width: spin(config.ui.window_width as f64, 320.0, 3_840.0, 10.0),
            result_max_height: spin(config.ui.result_max_height as f64, 120.0, 2_160.0, 10.0),
            shell_margin: spin(config.ui.shell_margin as f64, 0.0, 96.0, 1.0),
            shell_padding: spin(config.ui.shell_padding as f64, 0.0, 96.0, 1.0),
            shell_radius: spin(config.ui.shell_radius as f64, 0.0, 48.0, 1.0),
            search_height: spin(config.ui.search_height as f64, 36.0, 120.0, 1.0),
            search_radius: spin(config.ui.search_radius as f64, 0.0, 48.0, 1.0),
            search_font_size: spin(config.ui.search_font_size as f64, 12.0, 48.0, 1.0),
            result_margin_top: spin(config.ui.result_margin_top as f64, 0.0, 64.0, 1.0),
            result_row_padding_y: spin(config.ui.result_row_padding_y as f64, 2.0, 48.0, 1.0),
            result_row_padding_x: spin(config.ui.result_row_padding_x as f64, 2.0, 48.0, 1.0),
            result_row_radius: spin(config.ui.result_row_radius as f64, 0.0, 32.0, 1.0),
            title_font_size: spin(config.ui.title_font_size as f64, 10.0, 36.0, 1.0),
            subtitle_font_size: spin(config.ui.subtitle_font_size as f64, 8.0, 28.0, 1.0),
            icon_font_size: spin(config.ui.icon_font_size as f64, 10.0, 36.0, 1.0),
            window_background: color_button(&color_dialog, &config.ui.colors.window_background),
            shell_background: color_button(&color_dialog, &config.ui.colors.shell_background),
            shell_border: color_button(&color_dialog, &config.ui.colors.shell_border),
            search_background: color_button(&color_dialog, &config.ui.colors.search_background),
            search_text: color_button(&color_dialog, &config.ui.colors.search_text),
            results_background: color_button(&color_dialog, &config.ui.colors.results_background),
            row_background: color_button(&color_dialog, &config.ui.colors.row_background),
            row_selected_background: color_button(
                &color_dialog,
                &config.ui.colors.row_selected_background,
            ),
            icon: color_button(&color_dialog, &config.ui.colors.icon),
            title: color_button(&color_dialog, &config.ui.colors.title),
            subtitle: color_button(&color_dialog, &config.ui.colors.subtitle),
        }
    }

    fn config(&self) -> Config {
        let buffer = self.index_dirs.buffer();
        let directories = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(String::from)
            .collect();

        Config {
            max_results: spin_usize(&self.max_results),
            max_recent_searches: spin_usize(&self.max_recent_searches),
            max_recent_items: spin_usize(&self.max_recent_items),
            max_result_height: self.max_result_height.value_as_int(),
            max_indexed_items: spin_usize(&self.max_indexed_items),
            index_depth: spin_usize(&self.index_depth),
            include_hidden: self.include_hidden.is_active(),
            include_path_binaries: self.include_path_binaries.is_active(),
            system_tray: self.system_tray.is_active(),
            index_dirs: directories,
            ai: AiConfig {
                base_url: self.ai_base_url.text().to_string(),
                api_key: self.ai_api_key.text().to_string(),
                model: self.ai_model.text().to_string(),
            },
            ui: UiConfig {
                position: POSITION_IDS
                    .get(self.position.selected() as usize)
                    .copied()
                    .unwrap_or("top-left")
                    .to_string(),
                x: self.x.value_as_int(),
                y: self.y.value_as_int(),
                window_width: self.window_width.value_as_int(),
                result_max_height: self.result_max_height.value_as_int(),
                shell_margin: self.shell_margin.value_as_int(),
                shell_padding: self.shell_padding.value_as_int(),
                shell_radius: self.shell_radius.value_as_int(),
                search_height: self.search_height.value_as_int(),
                search_radius: self.search_radius.value_as_int(),
                search_font_size: self.search_font_size.value_as_int(),
                result_margin_top: self.result_margin_top.value_as_int(),
                result_row_padding_y: self.result_row_padding_y.value_as_int(),
                result_row_padding_x: self.result_row_padding_x.value_as_int(),
                result_row_radius: self.result_row_radius.value_as_int(),
                title_font_size: self.title_font_size.value_as_int(),
                subtitle_font_size: self.subtitle_font_size.value_as_int(),
                icon_font_size: self.icon_font_size.value_as_int(),
                colors: UiColors {
                    window_background: color_value(&self.window_background),
                    shell_background: color_value(&self.shell_background),
                    shell_border: color_value(&self.shell_border),
                    search_background: color_value(&self.search_background),
                    search_text: color_value(&self.search_text),
                    results_background: color_value(&self.results_background),
                    row_background: color_value(&self.row_background),
                    row_selected_background: color_value(&self.row_selected_background),
                    icon: color_value(&self.icon),
                    title: color_value(&self.title),
                    subtitle: color_value(&self.subtitle),
                },
            },
        }
    }

    fn notebook(&self) -> Notebook {
        let notebook = Notebook::new();
        notebook.set_scrollable(true);
        notebook.append_page(&self.general_page(), Some(&Label::new(Some("General"))));
        notebook.append_page(&self.interface_page(), Some(&Label::new(Some("Interface"))));
        notebook.append_page(&self.colors_page(), Some(&Label::new(Some("Colors"))));
        notebook.append_page(&self.ai_page(), Some(&Label::new(Some("AI"))));
        notebook
    }

    fn general_page(&self) -> ScrolledWindow {
        let grid = settings_grid();
        let mut row = 0;
        add_row(
            &grid,
            &mut row,
            "Maximum results",
            "Rows shown for a query.",
            &self.max_results,
        );
        add_row(
            &grid,
            &mut row,
            "Recent items",
            "Apps and web searches shown when the query is empty.",
            &self.max_recent_items,
        );
        add_row(
            &grid,
            &mut row,
            "Query history",
            "Queries retained for Readline history navigation.",
            &self.max_recent_searches,
        );
        add_row(
            &grid,
            &mut row,
            "Legacy result height",
            "Fallback for older configurations.",
            &self.max_result_height,
        );
        add_row(
            &grid,
            &mut row,
            "Maximum indexed items",
            "Upper limit for filesystem entries.",
            &self.max_indexed_items,
        );
        add_row(
            &grid,
            &mut row,
            "Index depth",
            "Maximum directory traversal depth.",
            &self.index_depth,
        );
        add_row(
            &grid,
            &mut row,
            "Include hidden files",
            "Index dotfiles and hidden directories.",
            &self.include_hidden,
        );
        add_row(
            &grid,
            &mut row,
            "Include PATH binaries",
            "Index executable files found in PATH.",
            &self.include_path_binaries,
        );
        add_row(
            &grid,
            &mut row,
            "System tray",
            "Keep Spotter available from the tray.",
            &self.system_tray,
        );

        let dirs = ScrolledWindow::builder()
            .min_content_height(150)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&self.index_dirs)
            .build();
        add_row(
            &grid,
            &mut row,
            "Indexed directories",
            "One path per line; relative paths use your home directory.",
            &dirs,
        );
        settings_page(&grid)
    }

    fn interface_page(&self) -> ScrolledWindow {
        let grid = settings_grid();
        let mut row = 0;
        add_row(
            &grid,
            &mut row,
            "Window position",
            "Sway anchor or custom coordinates.",
            &self.position,
        );
        add_row(
            &grid,
            &mut row,
            "X offset",
            "Horizontal margin, offset, or custom coordinate.",
            &self.x,
        );
        add_row(
            &grid,
            &mut row,
            "Y offset",
            "Vertical margin, offset, or custom coordinate.",
            &self.y,
        );
        add_row(
            &grid,
            &mut row,
            "Window width",
            "Exact launcher width on Sway.",
            &self.window_width,
        );
        add_row(
            &grid,
            &mut row,
            "Result pane height",
            "Maximum natural result pane height.",
            &self.result_max_height,
        );
        add_row(
            &grid,
            &mut row,
            "Shell margin",
            "Outer spacing around the launcher shell.",
            &self.shell_margin,
        );
        add_row(
            &grid,
            &mut row,
            "Shell padding",
            "Inner spacing inside the launcher shell.",
            &self.shell_padding,
        );
        add_row(
            &grid,
            &mut row,
            "Shell radius",
            "Outer corner radius.",
            &self.shell_radius,
        );
        add_row(
            &grid,
            &mut row,
            "Search height",
            "Search entry height.",
            &self.search_height,
        );
        add_row(
            &grid,
            &mut row,
            "Search radius",
            "Search entry corner radius.",
            &self.search_radius,
        );
        add_row(
            &grid,
            &mut row,
            "Search font size",
            "Search entry text size.",
            &self.search_font_size,
        );
        add_row(
            &grid,
            &mut row,
            "Result top margin",
            "Space between search and results.",
            &self.result_margin_top,
        );
        add_row(
            &grid,
            &mut row,
            "Row vertical padding",
            "Vertical padding inside result rows.",
            &self.result_row_padding_y,
        );
        add_row(
            &grid,
            &mut row,
            "Row horizontal padding",
            "Horizontal padding inside result rows.",
            &self.result_row_padding_x,
        );
        add_row(
            &grid,
            &mut row,
            "Row radius",
            "Result selection corner radius.",
            &self.result_row_radius,
        );
        add_row(
            &grid,
            &mut row,
            "Title font size",
            "Primary result text size.",
            &self.title_font_size,
        );
        add_row(
            &grid,
            &mut row,
            "Subtitle font size",
            "Secondary result text size.",
            &self.subtitle_font_size,
        );
        add_row(
            &grid,
            &mut row,
            "Icon size",
            "Application and fallback icon size.",
            &self.icon_font_size,
        );
        settings_page(&grid)
    }

    fn colors_page(&self) -> ScrolledWindow {
        let grid = settings_grid();
        let mut row = 0;
        add_row(
            &grid,
            &mut row,
            "Window background",
            "Outer GTK window color.",
            &self.window_background,
        );
        add_row(
            &grid,
            &mut row,
            "Shell background",
            "Launcher shell fill color.",
            &self.shell_background,
        );
        add_row(
            &grid,
            &mut row,
            "Shell border",
            "Launcher shell border color.",
            &self.shell_border,
        );
        add_row(
            &grid,
            &mut row,
            "Search background",
            "Search entry fill color.",
            &self.search_background,
        );
        add_row(
            &grid,
            &mut row,
            "Search text",
            "Search entry text and icon color.",
            &self.search_text,
        );
        add_row(
            &grid,
            &mut row,
            "Results background",
            "Result pane fill color.",
            &self.results_background,
        );
        add_row(
            &grid,
            &mut row,
            "Row background",
            "Normal result row fill color.",
            &self.row_background,
        );
        add_row(
            &grid,
            &mut row,
            "Selected row",
            "Selected result row fill color.",
            &self.row_selected_background,
        );
        add_row(
            &grid,
            &mut row,
            "Icon",
            "Fallback result icon color.",
            &self.icon,
        );
        add_row(
            &grid,
            &mut row,
            "Title",
            "Primary result text color.",
            &self.title,
        );
        add_row(
            &grid,
            &mut row,
            "Subtitle",
            "Secondary result text color.",
            &self.subtitle,
        );
        settings_page(&grid)
    }

    fn ai_page(&self) -> ScrolledWindow {
        let grid = settings_grid();
        let mut row = 0;
        add_row(
            &grid,
            &mut row,
            "Base URL",
            "OpenAI-compatible API base URL.",
            &self.ai_base_url,
        );
        add_row(
            &grid,
            &mut row,
            "API key",
            "Stored privately; the environment variable remains supported.",
            &self.ai_api_key,
        );
        add_row(
            &grid,
            &mut row,
            "Model",
            "Chat-completions model identifier.",
            &self.ai_model,
        );
        settings_page(&grid)
    }
}

pub(crate) fn present(app: &Application, parent: &ApplicationWindow) {
    if let Some(window) = app
        .windows()
        .into_iter()
        .find(|window| window.title().as_deref() == Some(SETTINGS_TITLE))
    {
        window.present();
        return;
    }

    let (current, load_error) = match config::load() {
        Ok(config) => (config, None),
        Err(error) => (
            Config::default(),
            Some(format!(
                "Could not load config.toml: {error:#}. Saving will replace it with these values."
            )),
        ),
    };
    let controls = Controls::new(&current);
    let window = ApplicationWindow::builder()
        .application(app)
        .title(SETTINGS_TITLE)
        .transient_for(parent)
        .modal(false)
        .default_width(780)
        .default_height(720)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let heading = Label::new(Some("Spotter Settings"));
    heading.add_css_class("title-1");
    heading.set_halign(Align::Start);
    root.append(&heading);

    let note = Label::new(Some(
        "Changes are saved to config.toml and take effect after restarting Spotter.",
    ));
    note.set_halign(Align::Start);
    note.set_wrap(true);
    root.append(&note);

    let notebook = controls.notebook();
    notebook.set_vexpand(true);
    root.append(&notebook);

    let status = Label::new(load_error.as_deref());
    status.set_halign(Align::Start);
    status.set_hexpand(true);
    status.set_wrap(true);

    let close = Button::with_label("Close");
    let save = Button::with_label("Save");
    save.add_css_class("suggested-action");
    let footer = GtkBox::new(Orientation::Horizontal, 8);
    footer.append(&status);
    footer.append(&close);
    footer.append(&save);
    root.append(&footer);

    {
        let window = window.clone();
        close.connect_clicked(move |_| window.close());
    }
    {
        let controls = controls.clone();
        let status = status.clone();
        save.connect_clicked(move |_| match config::save(&controls.config()) {
            Ok(()) => status.set_text("Saved. Restart Spotter to apply the new settings."),
            Err(error) => status.set_text(&format!("Could not save settings: {error:#}")),
        });
    }

    window.set_child(Some(&root));
    window.present();
}

fn spin(value: f64, min: f64, max: f64, step: f64) -> SpinButton {
    let input = SpinButton::with_range(min, max, step);
    input.set_value(value);
    input.set_numeric(true);
    input.set_hexpand(true);
    input
}

fn spin_usize(input: &SpinButton) -> usize {
    input.value_as_int().max(0) as usize
}

fn toggle(active: bool) -> Switch {
    Switch::builder().active(active).halign(Align::End).build()
}

fn entry(value: &str) -> Entry {
    Entry::builder().text(value).hexpand(true).build()
}

fn color_button(dialog: &ColorDialog, value: &str) -> ColorDialogButton {
    let color = gdk::RGBA::parse(value).unwrap_or(gdk::RGBA::BLACK);
    ColorDialogButton::builder()
        .dialog(dialog)
        .rgba(&color)
        .halign(Align::End)
        .build()
}

fn color_value(button: &ColorDialogButton) -> String {
    button.rgba().to_string()
}

fn settings_grid() -> Grid {
    Grid::builder()
        .column_spacing(24)
        .row_spacing(14)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build()
}

fn add_row(
    grid: &Grid,
    row: &mut i32,
    title: &str,
    description: &str,
    control: &impl IsA<gtk::Widget>,
) {
    let labels = GtkBox::new(Orientation::Vertical, 2);
    labels.set_hexpand(true);
    let title = Label::new(Some(title));
    title.set_halign(Align::Start);
    title.set_xalign(0.0);
    let description = Label::new(Some(description));
    description.add_css_class("dim-label");
    description.set_halign(Align::Start);
    description.set_xalign(0.0);
    description.set_wrap(true);
    labels.append(&title);
    labels.append(&description);

    grid.attach(&labels, 0, *row, 1, 1);
    grid.attach(control, 1, *row, 1, 1);
    *row += 1;
}

fn settings_page(grid: &Grid) -> ScrolledWindow {
    ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(grid)
        .build()
}
