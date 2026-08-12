use anyhow::{Context, Result};
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, EventControllerKey, Label, ListBox,
    ListBoxRow, Orientation, ScrolledWindow, SearchEntry,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use walkdir::{DirEntry, WalkDir};

const APP_ID: &str = "dev.spotter.Launcher";
const PRODUCT_NAME: &str = "Spotter";
const CONFIG_DIR_NAME: &str = "spotter";
const CONFIG_FILE_NAME: &str = "config.toml";
const DEFAULT_MAX_RESULTS: usize = 9;
const DEFAULT_MAX_RESULT_HEIGHT: i32 = 420;
const DEFAULT_MAX_INDEXED_ITEMS: usize = 60_000;
const DEFAULT_INDEX_DEPTH: usize = 5;
const DEFAULT_WINDOW_WIDTH: i32 = 720;
const DEFAULT_SHELL_MARGIN: i32 = 24;
const DEFAULT_SHELL_PADDING: i32 = 18;
const DEFAULT_SHELL_RADIUS: i32 = 18;
const DEFAULT_SEARCH_HEIGHT: i32 = 54;
const DEFAULT_SEARCH_RADIUS: i32 = 12;
const DEFAULT_SEARCH_FONT_SIZE: i32 = 24;
const DEFAULT_RESULT_MARGIN_TOP: i32 = 12;
const DEFAULT_RESULT_ROW_PADDING_Y: i32 = 10;
const DEFAULT_RESULT_ROW_PADDING_X: i32 = 14;
const DEFAULT_RESULT_ROW_RADIUS: i32 = 10;
const DEFAULT_TITLE_FONT_SIZE: i32 = 16;
const DEFAULT_SUBTITLE_FONT_SIZE: i32 = 12;
const DEFAULT_ICON_FONT_SIZE: i32 = 20;
const DEFAULT_CONFIG: &str = include_str!("../config.example.toml");

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Config {
    #[serde(default = "default_max_results")]
    max_results: usize,
    #[serde(default = "default_max_result_height")]
    max_result_height: i32,
    #[serde(default = "default_max_indexed_items")]
    max_indexed_items: usize,
    #[serde(default = "default_index_depth")]
    index_depth: usize,
    #[serde(default)]
    include_hidden: bool,
    #[serde(default = "default_index_dirs")]
    index_dirs: Vec<String>,
    #[serde(default)]
    ai: AiConfig,
    #[serde(default)]
    ui: UiConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_results: default_max_results(),
            max_result_height: default_max_result_height(),
            max_indexed_items: default_max_indexed_items(),
            index_depth: default_index_depth(),
            include_hidden: false,
            index_dirs: default_index_dirs(),
            ai: AiConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AiConfig {
    #[serde(default = "default_ai_base_url")]
    base_url: String,
    #[serde(default)]
    api_key: String,
    #[serde(default = "default_ai_model")]
    model: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            base_url: default_ai_base_url(),
            api_key: String::new(),
            model: default_ai_model(),
        }
    }
}

impl AiConfig {
    fn sanitize(&mut self) {
        if self.base_url.trim().is_empty() {
            self.base_url = default_ai_base_url();
        }
        self.base_url = self.base_url.trim().trim_end_matches('/').to_string();
        if self.model.trim().is_empty() {
            self.model = default_ai_model();
        }
    }

    fn resolve_api_key(&self) -> Option<String> {
        if !self.api_key.trim().is_empty() {
            return Some(self.api_key.trim().to_string());
        }
        env::var("OPENAI_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
    }
}

fn default_ai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_ai_model() -> String {
    "gpt-4o-mini".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UiConfig {
    #[serde(default = "default_position")]
    position: String,
    #[serde(default)]
    x: i32,
    #[serde(default)]
    y: i32,
    #[serde(default = "default_window_width")]
    window_width: i32,
    #[serde(default = "default_max_result_height")]
    result_max_height: i32,
    #[serde(default = "default_shell_margin")]
    shell_margin: i32,
    #[serde(default = "default_shell_padding")]
    shell_padding: i32,
    #[serde(default = "default_shell_radius")]
    shell_radius: i32,
    #[serde(default = "default_search_height")]
    search_height: i32,
    #[serde(default = "default_search_radius")]
    search_radius: i32,
    #[serde(default = "default_search_font_size")]
    search_font_size: i32,
    #[serde(default = "default_result_margin_top")]
    result_margin_top: i32,
    #[serde(default = "default_result_row_padding_y")]
    result_row_padding_y: i32,
    #[serde(default = "default_result_row_padding_x")]
    result_row_padding_x: i32,
    #[serde(default = "default_result_row_radius")]
    result_row_radius: i32,
    #[serde(default = "default_title_font_size")]
    title_font_size: i32,
    #[serde(default = "default_subtitle_font_size")]
    subtitle_font_size: i32,
    #[serde(default = "default_icon_font_size")]
    icon_font_size: i32,
    #[serde(default)]
    colors: UiColors,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            position: default_position(),
            x: 0,
            y: 0,
            window_width: default_window_width(),
            result_max_height: default_max_result_height(),
            shell_margin: default_shell_margin(),
            shell_padding: default_shell_padding(),
            shell_radius: default_shell_radius(),
            search_height: default_search_height(),
            search_radius: default_search_radius(),
            search_font_size: default_search_font_size(),
            result_margin_top: default_result_margin_top(),
            result_row_padding_y: default_result_row_padding_y(),
            result_row_padding_x: default_result_row_padding_x(),
            result_row_radius: default_result_row_radius(),
            title_font_size: default_title_font_size(),
            subtitle_font_size: default_subtitle_font_size(),
            icon_font_size: default_icon_font_size(),
            colors: UiColors::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UiColors {
    #[serde(default = "default_window_background")]
    window_background: String,
    #[serde(default = "default_shell_background")]
    shell_background: String,
    #[serde(default = "default_shell_border")]
    shell_border: String,
    #[serde(default = "default_search_background")]
    search_background: String,
    #[serde(default = "default_search_text")]
    search_text: String,
    #[serde(default = "default_results_background")]
    results_background: String,
    #[serde(default = "default_row_background")]
    row_background: String,
    #[serde(default = "default_row_selected_background")]
    row_selected_background: String,
    #[serde(default = "default_icon_color")]
    icon: String,
    #[serde(default = "default_title_color")]
    title: String,
    #[serde(default = "default_subtitle_color")]
    subtitle: String,
}

impl Default for UiColors {
    fn default() -> Self {
        Self {
            window_background: default_window_background(),
            shell_background: default_shell_background(),
            shell_border: default_shell_border(),
            search_background: default_search_background(),
            search_text: default_search_text(),
            results_background: default_results_background(),
            row_background: default_row_background(),
            row_selected_background: default_row_selected_background(),
            icon: default_icon_color(),
            title: default_title_color(),
            subtitle: default_subtitle_color(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowPosition {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Custom,
}

impl WindowPosition {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "top-left" => Some(Self::TopLeft),
            "top-center" => Some(Self::TopCenter),
            "top-right" => Some(Self::TopRight),
            "center-left" => Some(Self::CenterLeft),
            "center" => Some(Self::Center),
            "center-right" => Some(Self::CenterRight),
            "bottom-left" => Some(Self::BottomLeft),
            "bottom-center" => Some(Self::BottomCenter),
            "bottom-right" => Some(Self::BottomRight),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    fn normalized(self) -> &'static str {
        match self {
            Self::TopLeft => "top-left",
            Self::TopCenter => "top-center",
            Self::TopRight => "top-right",
            Self::CenterLeft => "center-left",
            Self::Center => "center",
            Self::CenterRight => "center-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomCenter => "bottom-center",
            Self::BottomRight => "bottom-right",
            Self::Custom => "custom",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
struct SwayRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Deserialize)]
struct SwayWorkspace {
    focused: bool,
    rect: SwayRect,
}

#[derive(Debug, Deserialize)]
struct SwayCommandResult {
    success: bool,
    error: Option<String>,
}

fn default_max_results() -> usize {
    DEFAULT_MAX_RESULTS
}

fn default_max_result_height() -> i32 {
    DEFAULT_MAX_RESULT_HEIGHT
}

fn default_max_indexed_items() -> usize {
    DEFAULT_MAX_INDEXED_ITEMS
}

fn default_index_depth() -> usize {
    DEFAULT_INDEX_DEPTH
}

fn default_index_dirs() -> Vec<String> {
    [
        "Desktop",
        "Documents",
        "Downloads",
        "Pictures",
        "Music",
        "Videos",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn default_position() -> String {
    "top-left".to_string()
}

fn default_window_width() -> i32 {
    DEFAULT_WINDOW_WIDTH
}

fn default_shell_margin() -> i32 {
    DEFAULT_SHELL_MARGIN
}

fn default_shell_padding() -> i32 {
    DEFAULT_SHELL_PADDING
}

fn default_shell_radius() -> i32 {
    DEFAULT_SHELL_RADIUS
}

fn default_search_height() -> i32 {
    DEFAULT_SEARCH_HEIGHT
}

fn default_search_radius() -> i32 {
    DEFAULT_SEARCH_RADIUS
}

fn default_search_font_size() -> i32 {
    DEFAULT_SEARCH_FONT_SIZE
}

fn default_result_margin_top() -> i32 {
    DEFAULT_RESULT_MARGIN_TOP
}

fn default_result_row_padding_y() -> i32 {
    DEFAULT_RESULT_ROW_PADDING_Y
}

fn default_result_row_padding_x() -> i32 {
    DEFAULT_RESULT_ROW_PADDING_X
}

fn default_result_row_radius() -> i32 {
    DEFAULT_RESULT_ROW_RADIUS
}

fn default_title_font_size() -> i32 {
    DEFAULT_TITLE_FONT_SIZE
}

fn default_subtitle_font_size() -> i32 {
    DEFAULT_SUBTITLE_FONT_SIZE
}

fn default_icon_font_size() -> i32 {
    DEFAULT_ICON_FONT_SIZE
}

fn default_window_background() -> String {
    "transparent".to_string()
}

fn default_shell_background() -> String {
    "rgba(28, 31, 36, 0.96)".to_string()
}

fn default_shell_border() -> String {
    "rgba(255, 255, 255, 0.14)".to_string()
}

fn default_search_background() -> String {
    "rgba(255, 255, 255, 0.1)".to_string()
}

fn default_search_text() -> String {
    "#f5f7fa".to_string()
}

fn default_results_background() -> String {
    "transparent".to_string()
}

fn default_row_background() -> String {
    "transparent".to_string()
}

fn default_row_selected_background() -> String {
    "rgba(108, 160, 255, 0.28)".to_string()
}

fn default_icon_color() -> String {
    "#9fb7ff".to_string()
}

fn default_title_color() -> String {
    "#f5f7fa".to_string()
}

fn default_subtitle_color() -> String {
    "#aeb6c2".to_string()
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ItemKind {
    Application,
    Command,
    File,
    Directory,
    WebSearch,
    AiPrompt,
}

#[derive(Clone, Debug)]
struct SearchItem {
    title: String,
    subtitle: String,
    target: String,
    kind: ItemKind,
    tokens: String,
}

#[derive(Clone, Debug)]
struct SearchResult {
    item: SearchItem,
    score: i64,
}

#[derive(Default)]
struct SearchIndex {
    items: Vec<SearchItem>,
}

type SharedIndex = Arc<RwLock<SearchIndex>>;

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let (config, config_error) = match load_config() {
        Ok(config) => (config, None),
        Err(error) => {
            eprintln!("failed to load config, using defaults: {error:#}");
            let message = "Configuration error in config.toml. Check its TOML syntax, then restart Spotter. Defaults are active for this launch.".to_string();
            (Config::default(), Some(message))
        }
    };
    let config = Arc::new(config);
    let index = Arc::new(RwLock::new(SearchIndex::default()));

    let window = ApplicationWindow::builder()
        .application(app)
        .title(PRODUCT_NAME)
        .default_width(config.ui.window_width.max(320))
        .resizable(false)
        .decorated(false)
        .build();
    window.set_size_request(config.ui.window_width, -1);
    window.set_overflow(gtk::Overflow::Hidden);

    let provider = gtk::CssProvider::new();
    let style = build_style(&config.ui);
    provider.load_from_data(&style);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.set_widget_name("shell");

    let input = SearchEntry::builder()
        .placeholder_text("Search apps, files, and the web — / asks AI")
        .hexpand(true)
        .build();
    input.set_widget_name("search");

    let list = ListBox::new();
    list.set_widget_name("results");
    list.set_selection_mode(gtk::SelectionMode::Single);

    let response = Label::new(None);
    response.set_widget_name("response");
    response.set_wrap(true);
    response.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    response.set_selectable(true);
    response.set_halign(Align::Start);
    response.set_xalign(0.0);
    response.set_visible(false);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.append(&list);
    content.append(&response);

    let scroll = ScrolledWindow::builder()
        .max_content_height(config.ui.result_max_height.max(120))
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&content)
        .build();
    scroll.set_visible(false);

    root.append(&input);
    root.append(&scroll);
    window.set_child(Some(&root));

    let state = Arc::new(RwLock::new(Vec::<SearchResult>::new()));
    render_results(&list, &scroll, &[], "", &config.ui);
    if let Some(error) = config_error {
        list.set_visible(false);
        response.set_text(&error);
        response.set_visible(true);
        scroll.set_visible(true);
    }

    let activate: Rc<dyn Fn(&SearchItem)> = {
        let app = app.clone();
        let config = config.clone();
        let list = list.clone();
        let scroll = scroll.clone();
        let response = response.clone();
        Rc::new(move |item: &SearchItem| {
            if matches!(item.kind, ItemKind::AiPrompt) {
                if item.target.is_empty() {
                    return;
                }
                list.set_visible(false);
                scroll.set_visible(true);
                response.set_text("Thinking…");
                response.set_visible(true);
                ask_ai(&config.ai, &item.target, &response);
                return;
            }
            if let Err(error) = launch(item) {
                eprintln!("launch failed: {error:#}");
            } else {
                app.quit();
            }
        })
    };

    {
        let list = list.clone();
        let scroll = scroll.clone();
        let response = response.clone();
        let index = index.clone();
        let state = state.clone();
        let config = config.clone();
        let window = window.clone();
        input.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            let now = Instant::now();
            let results = search(&index, &query, config.max_results);
            if let Ok(mut state) = state.write() {
                *state = results.clone();
            }
            response.set_visible(false);
            list.set_visible(true);
            render_results(&list, &scroll, &results, &query, &config.ui);
            schedule_window_reposition(&window, config.ui.clone());
            eprintln!(
                "search `{query}`: {} results in {:?}",
                results.len(),
                now.elapsed()
            );
        });
    }

    {
        let app = app.clone();
        let input_for_handler = input.clone();
        let list = list.clone();
        let state = state.clone();
        let activate = activate.clone();
        let key = EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        key.connect_key_pressed(move |_, key, _, _| match key {
            gdk::Key::Escape => {
                if input_for_handler.text().is_empty() {
                    app.quit();
                } else {
                    input_for_handler.set_text("");
                }
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter => {
                if let Some(item) = selected_item(&state, &list) {
                    activate(&item);
                }
                glib::Propagation::Stop
            }
            gdk::Key::Down => {
                move_selection(&list, 1);
                glib::Propagation::Stop
            }
            gdk::Key::Up => {
                move_selection(&list, -1);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
        input.add_controller(key);
    }

    {
        let state = state.clone();
        let list = list.clone();
        let activate = activate.clone();
        let key = EventControllerKey::new();
        key.connect_key_pressed(move |_, key, _, _| match key {
            gdk::Key::Return | gdk::Key::KP_Enter => {
                if let Some(item) = selected_item(&state, &list) {
                    activate(&item);
                }
                glib::Propagation::Stop
            }
            gdk::Key::Down => {
                move_selection(&list, 1);
                glib::Propagation::Stop
            }
            gdk::Key::Up => {
                move_selection(&list, -1);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
        window.add_controller(key);
    }

    window.present();
    schedule_window_position(&window, config.ui.clone());
    schedule_indexer(index.clone(), config.clone());
    input.grab_focus();
}

fn schedule_indexer(index: SharedIndex, config: Arc<Config>) {
    glib::idle_add_once(move || spawn_indexer(index, config));
}

fn schedule_window_position(window: &ApplicationWindow, ui: UiConfig) {
    for attempt in 1..=4 {
        let window = window.clone();
        let ui = ui.clone();
        glib::timeout_add_local_once(Duration::from_millis(120 * attempt), move || {
            apply_window_position(&window, &ui)
        });
    }
}

fn schedule_window_reposition(window: &ApplicationWindow, ui: UiConfig) {
    let window = window.clone();
    glib::timeout_add_local_once(Duration::from_millis(30), move || {
        apply_window_position(&window, &ui)
    });
}

fn apply_window_position(window: &ApplicationWindow, ui: &UiConfig) {
    if env::var_os("SWAYSOCK").is_some() && command_exists("swaymsg") {
        if let Err(error) = position_with_sway(window, ui) {
            eprintln!("failed to position window with swaymsg: {error:#}");
        }
        return;
    }

    eprintln!(
        "window position `{}` requested, but no supported positioning backend is available",
        ui.position
    );
}

fn position_with_sway(window: &ApplicationWindow, ui: &UiConfig) -> Result<()> {
    let position = WindowPosition::parse(&ui.position).unwrap_or(WindowPosition::TopLeft);
    let window_width = window.allocated_width().max(ui.window_width).max(1);
    let window_height = window.allocated_height().max(1);
    let (x, y) = if position == WindowPosition::Custom {
        (ui.x, ui.y)
    } else {
        calculate_window_position(
            position,
            focused_sway_workspace()?,
            window_width,
            window_height,
            ui.x,
            ui.y,
        )
    };

    let command = format!(
        r#"[app_id="{APP_ID}"] floating enable, resize set width {} px, move absolute position {} px {} px"#,
        ui.window_width, x, y
    );

    let output = Command::new("swaymsg")
        .arg(command)
        .output()
        .context("run swaymsg window command")?;
    if !output.status.success() {
        anyhow::bail!(
            "swaymsg window command exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let results: Vec<SwayCommandResult> =
        serde_json::from_slice(&output.stdout).context("parse swaymsg window result")?;
    if let Some(result) = results.into_iter().find(|result| !result.success) {
        anyhow::bail!(
            "swaymsg rejected window command: {}",
            result.error.as_deref().unwrap_or("unknown error")
        );
    }
    Ok(())
}

fn focused_sway_workspace() -> Result<SwayRect> {
    let output = Command::new("swaymsg")
        .args(["-t", "get_workspaces", "-r"])
        .output()
        .context("query Sway workspaces")?;
    if !output.status.success() {
        anyhow::bail!(
            "swaymsg workspace query exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let workspaces: Vec<SwayWorkspace> =
        serde_json::from_slice(&output.stdout).context("parse Sway workspace geometry")?;
    workspaces
        .into_iter()
        .find(|workspace| workspace.focused)
        .map(|workspace| workspace.rect)
        .context("Sway reported no focused workspace")
}

fn calculate_window_position(
    position: WindowPosition,
    workspace: SwayRect,
    window_width: i32,
    window_height: i32,
    x_offset: i32,
    y_offset: i32,
) -> (i32, i32) {
    if position == WindowPosition::Custom {
        return (x_offset, y_offset);
    }

    let left = workspace.x + x_offset.max(0);
    let center_x = workspace.x + (workspace.width - window_width) / 2 + x_offset;
    let right = workspace.x + workspace.width - window_width - x_offset.max(0);
    let top = workspace.y + y_offset.max(0);
    let center_y = workspace.y + (workspace.height - window_height) / 2 + y_offset;
    let bottom = workspace.y + workspace.height - window_height - y_offset.max(0);

    let (x, y) = match position {
        WindowPosition::TopLeft => (left, top),
        WindowPosition::TopCenter => (center_x, top),
        WindowPosition::TopRight => (right, top),
        WindowPosition::CenterLeft => (left, center_y),
        WindowPosition::Center => (center_x, center_y),
        WindowPosition::CenterRight => (right, center_y),
        WindowPosition::BottomLeft => (left, bottom),
        WindowPosition::BottomCenter => (center_x, bottom),
        WindowPosition::BottomRight => (right, bottom),
        WindowPosition::Custom => unreachable!(),
    };

    let min_x = workspace.x;
    let max_x = workspace.x + (workspace.width - window_width).max(0);
    let min_y = workspace.y;
    let max_y = workspace.y + (workspace.height - window_height).max(0);
    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

fn command_exists(command: &str) -> bool {
    env::var_os("PATH")
        .map(|path| {
            env::split_paths(&path).any(|dir| {
                let path = dir.join(command);
                path.is_file()
                    && fs::metadata(&path)
                        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn load_config() -> Result<Config> {
    let config_dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .context("could not resolve config directory")?
        .join(CONFIG_DIR_NAME);
    let config_path = config_dir.join(CONFIG_FILE_NAME);

    if !config_path.exists() {
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("create {}", config_dir.display()))?;
        fs::write(&config_path, DEFAULT_CONFIG)
            .with_context(|| format!("write {}", config_path.display()))?;
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let mut config: Config =
        toml::from_str(&content).with_context(|| format!("parse {}", config_path.display()))?;
    config.sanitize();
    Ok(config)
}

impl Config {
    fn sanitize(&mut self) {
        if self.max_results == 0 {
            self.max_results = DEFAULT_MAX_RESULTS;
        }
        if self.max_result_height < 120 {
            self.max_result_height = 120;
        }
        if self.ui.result_max_height < 120 {
            self.ui.result_max_height = self.max_result_height.max(120);
        }
        if self.max_indexed_items == 0 {
            self.max_indexed_items = DEFAULT_MAX_INDEXED_ITEMS;
        }
        if self.index_depth == 0 {
            self.index_depth = DEFAULT_INDEX_DEPTH;
        }
        if self.index_dirs.is_empty() {
            self.index_dirs = default_index_dirs();
        }
        self.ai.sanitize();
        self.ui.sanitize();
    }
}

impl UiConfig {
    fn sanitize(&mut self) {
        let position = WindowPosition::parse(&self.position).unwrap_or(WindowPosition::TopLeft);
        self.position = position.normalized().to_string();
        self.x = self.x.clamp(-100_000, 100_000);
        self.y = self.y.clamp(-100_000, 100_000);
        self.window_width = self.window_width.clamp(320, 3840);
        self.result_max_height = self.result_max_height.clamp(120, 2160);
        self.shell_margin = self.shell_margin.clamp(0, 96);
        self.shell_padding = self.shell_padding.clamp(0, 96);
        self.shell_radius = self.shell_radius.clamp(0, 48);
        self.search_height = self.search_height.clamp(36, 120);
        self.search_radius = self.search_radius.clamp(0, 48);
        self.search_font_size = self.search_font_size.clamp(12, 48);
        self.result_margin_top = self.result_margin_top.clamp(0, 64);
        self.result_row_padding_y = self.result_row_padding_y.clamp(2, 48);
        self.result_row_padding_x = self.result_row_padding_x.clamp(2, 48);
        self.result_row_radius = self.result_row_radius.clamp(0, 32);
        self.title_font_size = self.title_font_size.clamp(10, 36);
        self.subtitle_font_size = self.subtitle_font_size.clamp(8, 28);
        self.icon_font_size = self.icon_font_size.clamp(10, 36);
        self.colors.sanitize();
    }
}

impl UiColors {
    fn sanitize(&mut self) {
        sanitize_css_color(&mut self.window_background, default_window_background());
        sanitize_css_color(&mut self.shell_background, default_shell_background());
        sanitize_css_color(&mut self.shell_border, default_shell_border());
        sanitize_css_color(&mut self.search_background, default_search_background());
        sanitize_css_color(&mut self.search_text, default_search_text());
        sanitize_css_color(&mut self.results_background, default_results_background());
        sanitize_css_color(&mut self.row_background, default_row_background());
        sanitize_css_color(
            &mut self.row_selected_background,
            default_row_selected_background(),
        );
        sanitize_css_color(&mut self.icon, default_icon_color());
        sanitize_css_color(&mut self.title, default_title_color());
        sanitize_css_color(&mut self.subtitle, default_subtitle_color());
    }
}

fn sanitize_css_color(value: &mut String, fallback: String) {
    let trimmed = value.trim();
    if gdk::RGBA::parse(trimmed).is_err() {
        *value = fallback;
    } else {
        *value = trimmed.to_string();
    }
}

fn spawn_indexer(index: SharedIndex, config: Arc<Config>) {
    thread::spawn(move || {
        let started = Instant::now();
        let mut items = Vec::new();
        items.extend(read_desktop_apps());
        items.extend(read_path_commands());
        items.extend(read_filesystem_items(&config));

        let mut seen = HashSet::new();
        items.retain(|item| seen.insert((item.kind.clone(), item.target.clone())));

        if let Ok(mut index) = index.write() {
            index.items = items;
        }
        eprintln!("indexed in {:?}", started.elapsed());
    });
}

fn search(index: &SharedIndex, query: &str, max_results: usize) -> Vec<SearchResult> {
    let raw = query.trim();
    if raw.is_empty() {
        return Vec::new();
    }

    if let Some(prompt) = raw.strip_prefix('/') {
        return vec![SearchResult {
            item: ai_prompt_item(prompt.trim()),
            score: 0,
        }];
    }

    let query = raw.to_lowercase();

    let items = match index.read() {
        Ok(index) => index.items.clone(),
        Err(_) => return Vec::new(),
    };

    let terms: Vec<&str> = query.split_whitespace().collect();

    let mut results: Vec<_> = items
        .par_iter()
        .filter_map(|item| {
            match_score(&item.tokens, &query, &terms).map(|score| SearchResult {
                item: item.clone(),
                score: score + kind_boost(&item.kind),
            })
        })
        .collect();

    results.sort_unstable_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.item.title.len().cmp(&b.item.title.len()))
            .then_with(|| a.item.title.cmp(&b.item.title))
    });
    results.truncate(max_results);

    if results.is_empty() {
        results.push(SearchResult {
            item: web_search_item(raw),
            score: 0,
        });
    }

    results
}

fn ai_prompt_item(prompt: &str) -> SearchItem {
    SearchItem {
        title: if prompt.is_empty() {
            "Ask AI".to_string()
        } else {
            format!("Ask AI: {prompt}")
        },
        subtitle: if prompt.is_empty() {
            "Type a question after /".to_string()
        } else {
            "Press Enter to ask".to_string()
        },
        target: prompt.to_string(),
        kind: ItemKind::AiPrompt,
        tokens: String::new(),
    }
}

fn web_search_item(query: &str) -> SearchItem {
    SearchItem {
        title: format!("Search Google for \"{query}\""),
        subtitle: "Open in default browser".to_string(),
        target: format!("https://www.google.com/search?q={}", percent_encode(query)),
        kind: ItemKind::WebSearch,
        tokens: query.to_string(),
    }
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

// Every query term must appear as a contiguous substring; scattered
// character matches are treated as no match so unrelated queries fall
// through to the web-search row.
fn match_score(tokens: &str, query: &str, terms: &[&str]) -> Option<i64> {
    let mut score = 0_i64;

    for term in terms {
        let idx = tokens.find(term)?;
        score += 1_000 - (idx as i64).min(900);
        let at_word_start = tokens[..idx]
            .chars()
            .next_back()
            .map(|ch| !ch.is_alphanumeric())
            .unwrap_or(true);
        if at_word_start {
            score += 500;
        }
    }

    if terms.len() > 1 && tokens.contains(query) {
        score += 1_000;
    }

    Some(score)
}

fn kind_boost(kind: &ItemKind) -> i64 {
    match kind {
        ItemKind::Application => 3_000,
        ItemKind::Command => 1_500,
        ItemKind::Directory => 800,
        ItemKind::File => 0,
        ItemKind::WebSearch => 0,
        ItemKind::AiPrompt => 0,
    }
}

fn read_desktop_apps() -> Vec<SearchItem> {
    desktop_dirs()
        .into_iter()
        .flat_map(|dir| {
            WalkDir::new(dir)
                .max_depth(3)
                .into_iter()
                .filter_map(Result::ok)
        })
        .filter(|entry| entry.path().extension() == Some(OsStr::new("desktop")))
        .filter_map(|entry| parse_desktop_file(entry.path()).ok().flatten())
        .collect()
}

fn parse_desktop_file(path: &Path) -> Result<Option<SearchItem>> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let fields = parse_desktop_fields(&content);

    if fields.get("NoDisplay").is_some_and(|value| value == "true")
        || fields.get("Hidden").is_some_and(|value| value == "true")
    {
        return Ok(None);
    }

    let Some(name) = fields.get("Name").cloned() else {
        return Ok(None);
    };
    let Some(exec) = fields.get("Exec").cloned() else {
        return Ok(None);
    };

    let comment = fields.get("Comment").cloned().unwrap_or_default();
    let target = clean_desktop_exec(&exec);
    let tokens = format!("{name} {comment} {target}").to_lowercase();

    Ok(Some(SearchItem {
        title: name,
        subtitle: if comment.is_empty() {
            target.clone()
        } else {
            comment
        },
        target,
        kind: ItemKind::Application,
        tokens,
    }))
}

fn parse_desktop_fields(content: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let mut in_entry = false;

    for line in content.lines().map(str::trim) {
        if line == "[Desktop Entry]" {
            in_entry = true;
            continue;
        }
        if in_entry && line.starts_with('[') {
            break;
        }
        if !in_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            fields
                .entry(key.to_string())
                .or_insert_with(|| value.to_string());
        }
    }

    fields
}

fn clean_desktop_exec(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|part| !part.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_path_commands() -> Vec<SearchItem> {
    env::var_os("PATH")
        .map(|path| {
            env::split_paths(&path)
                .filter_map(|dir| fs::read_dir(dir).ok())
                .flat_map(|entries| entries.filter_map(Result::ok))
                .filter_map(|entry| command_item(entry.path()))
                .collect()
        })
        .unwrap_or_default()
}

fn command_item(path: PathBuf) -> Option<SearchItem> {
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return None;
    }

    let title = path.file_name()?.to_string_lossy().to_string();
    let target = path.to_string_lossy().to_string();
    Some(SearchItem {
        title: title.clone(),
        subtitle: target.clone(),
        target,
        kind: ItemKind::Command,
        tokens: title.to_lowercase(),
    })
}

fn read_filesystem_items(config: &Config) -> Vec<SearchItem> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };

    let roots = config
        .index_dirs
        .iter()
        .map(|name| expand_index_dir(&home, name))
        .filter(|path| path.exists());

    roots
        .flat_map(|root| {
            WalkDir::new(root)
                .max_depth(config.index_depth)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| config.include_hidden || visible_entry(entry))
                .filter_map(Result::ok)
                .take(config.max_indexed_items)
                .filter_map(|entry| filesystem_item(entry.path()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn expand_index_dir(home: &Path, value: &str) -> PathBuf {
    if value == "~" {
        return home.to_path_buf();
    }

    if let Some(rest) = value.strip_prefix("~/") {
        return home.join(rest);
    }

    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        home.join(path)
    }
}

fn visible_entry(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|name| !name.starts_with('.'))
        .unwrap_or(true)
}

fn filesystem_item(path: &Path) -> Option<SearchItem> {
    let title = path.file_name()?.to_string_lossy().to_string();
    let target = path.to_string_lossy().to_string();
    let kind = if path.is_dir() {
        ItemKind::Directory
    } else if path.is_file() {
        ItemKind::File
    } else {
        return None;
    };

    Some(SearchItem {
        title: title.clone(),
        subtitle: target.clone(),
        target: target.clone(),
        kind,
        tokens: format!("{title} {target}").to_lowercase(),
    })
}

fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/usr/share/applications")];
    if let Some(data_home) = env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        dirs.push(data_home.join("applications"));
    } else if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }

    if let Some(data_dirs) = env::var_os("XDG_DATA_DIRS") {
        dirs.extend(env::split_paths(&data_dirs).map(|path| path.join("applications")));
    }

    dirs.into_iter().filter(|path| path.exists()).collect()
}

fn render_results(
    list: &ListBox,
    scroll: &ScrolledWindow,
    results: &[SearchResult],
    query: &str,
    ui: &UiConfig,
) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }

    if query.trim().is_empty() {
        scroll.set_visible(false);
        return;
    }

    scroll.set_visible(true);
    if results.is_empty() {
        list.append(&result_row(
            "No results",
            "Keep typing or try another query",
            "",
            ui,
        ));
    } else {
        for result in results {
            let icon = match result.item.kind {
                ItemKind::Application => "●",
                ItemKind::Command => ">",
                ItemKind::File => "□",
                ItemKind::Directory => "▣",
                ItemKind::WebSearch => "⌕",
                ItemKind::AiPrompt => "✦",
            };
            list.append(&result_row(
                &result.item.title,
                &result.item.subtitle,
                icon,
                ui,
            ));
        }
        list.select_row(list.row_at_index(0).as_ref());
    }
}

fn result_row(title: &str, subtitle: &str, icon: &str, ui: &UiConfig) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_widget_name("result-row");

    let layout = GtkBox::new(Orientation::Horizontal, 14);
    layout.set_margin_top(ui.result_row_padding_y);
    layout.set_margin_bottom(ui.result_row_padding_y);
    layout.set_margin_start(ui.result_row_padding_x);
    layout.set_margin_end(ui.result_row_padding_x);

    let icon = Label::new(Some(icon));
    icon.set_widget_name("icon");
    icon.set_width_chars(2);

    let text = GtkBox::new(Orientation::Vertical, 2);
    let title = Label::new(Some(title));
    title.set_widget_name("title");
    title.set_halign(Align::Start);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let subtitle = Label::new(Some(subtitle));
    subtitle.set_widget_name("subtitle");
    subtitle.set_halign(Align::Start);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::Middle);

    text.append(&title);
    text.append(&subtitle);
    layout.append(&icon);
    layout.append(&text);
    row.set_child(Some(&layout));
    row
}

fn move_selection(list: &ListBox, delta: i32) {
    let current = list
        .selected_row()
        .map(|row| row.index())
        .unwrap_or_default();
    let next = (current + delta).max(0);
    if let Some(row) = list.row_at_index(next) {
        list.select_row(Some(&row));
    }
}

fn selected_item(state: &Arc<RwLock<Vec<SearchResult>>>, list: &ListBox) -> Option<SearchItem> {
    let selected = list
        .selected_row()
        .map(|row| row.index() as usize)
        .unwrap_or_default();
    state
        .read()
        .ok()
        .and_then(|results| results.get(selected).cloned())
        .map(|result| result.item)
}

fn launch(item: &SearchItem) -> Result<()> {
    match item.kind {
        ItemKind::Application => {
            if item.target.trim().is_empty() {
                anyhow::bail!("empty desktop Exec command");
            }
            Command::new("sh").arg("-c").arg(&item.target).spawn()?;
        }
        ItemKind::Command => {
            Command::new(&item.target).spawn()?;
        }
        ItemKind::File | ItemKind::Directory | ItemKind::WebSearch => {
            open::that_detached(&item.target)?;
        }
        // Handled by the activate closure in build_ui; never launched.
        ItemKind::AiPrompt => {}
    }
    Ok(())
}

fn ask_ai(ai: &AiConfig, prompt: &str, output: &Label) {
    let (tx, rx) = async_channel::bounded::<String>(1);
    let ai = ai.clone();
    let prompt = prompt.to_string();
    thread::spawn(move || {
        let message = match request_completion(&ai, &prompt) {
            Ok(text) => text,
            Err(error) => format!("AI request failed: {error:#}"),
        };
        let _ = tx.send_blocking(message);
    });

    let output = output.clone();
    glib::spawn_future_local(async move {
        if let Ok(message) = rx.recv().await {
            output.set_text(&message);
        }
    });
}

fn request_completion(ai: &AiConfig, prompt: &str) -> Result<String> {
    let url = format!("{}/chat/completions", ai.base_url.trim_end_matches('/'));
    let mut request = ureq::post(&url)
        .timeout(Duration::from_secs(60))
        .set("Content-Type", "application/json");
    if let Some(key) = ai.resolve_api_key() {
        request = request.set("Authorization", &format!("Bearer {key}"));
    }

    let response = request.send_json(serde_json::json!({
        "model": ai.model,
        "messages": [{"role": "user", "content": prompt}],
    }));

    let body: serde_json::Value = match response {
        Ok(response) => response.into_json().context("read response body")?,
        Err(ureq::Error::Status(code, response)) => {
            let mut detail = response.into_string().unwrap_or_default();
            detail.truncate(500);
            anyhow::bail!("HTTP {code} from {url}: {detail}");
        }
        Err(error) => return Err(error).with_context(|| format!("request {url}")),
    };

    body["choices"][0]["message"]["content"]
        .as_str()
        .map(|text| text.trim().to_string())
        .context("no completion text in response")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(tokens: &str, query: &str) -> Option<i64> {
        let terms: Vec<&str> = query.split_whitespace().collect();
        match_score(tokens, query, &terms)
    }

    #[test]
    fn example_config_is_valid() {
        let mut config: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        config.sanitize();
        assert_eq!(config.ui.position, "top-left");
        assert_eq!(config.ui.window_width, 720);
        assert_eq!(config.ui.search_height, 54);
    }

    #[test]
    fn anchored_positions_use_window_geometry_and_offsets() {
        let workspace = SwayRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            calculate_window_position(WindowPosition::TopRight, workspace, 720, 100, 96, 72),
            (1104, 72)
        );
        assert_eq!(
            calculate_window_position(WindowPosition::BottomCenter, workspace, 720, 100, 0, 20),
            (600, 960)
        );
        assert_eq!(
            calculate_window_position(WindowPosition::Custom, workspace, 720, 100, -800, 50),
            (-800, 50)
        );
    }

    #[test]
    fn invalid_ui_values_fall_back_or_clamp() {
        let mut ui = UiConfig {
            position: "somewhere".to_string(),
            window_width: 100,
            ..UiConfig::default()
        };
        ui.colors.search_background = "not-a-color".to_string();
        ui.colors.title = "  #12345678  ".to_string();
        ui.sanitize();

        assert_eq!(ui.position, "top-left");
        assert_eq!(ui.window_width, 320);
        assert_eq!(ui.colors.search_background, default_search_background());
        assert_eq!(ui.colors.title, "#12345678");
    }

    #[test]
    fn prefix_matches() {
        assert!(score("firefox web browser", "fir").is_some());
    }

    #[test]
    fn multi_word_matches_in_any_order() {
        assert!(score("firefox web browser", "browser fire").is_some());
    }

    #[test]
    fn word_start_ranks_above_mid_word() {
        let start = score("firefox web browser", "fir").unwrap();
        let mid = score("aafirefox web browser", "fir").unwrap();
        assert!(start > mid, "start {start} mid {mid}");
    }

    #[test]
    fn scattered_letters_are_no_match() {
        assert!(score("gnu image manipulation program", "gimp").is_none());
        assert!(score(
            "how to use graphic design to sell things and explain things",
            "how to cook rice"
        )
        .is_none());
    }

    #[test]
    fn missing_term_is_no_match() {
        assert!(score("readme.txt /home/user/readme.txt", "readme zzz").is_none());
    }

    #[test]
    fn gibberish_query_falls_back_to_web_search() {
        let config = Arc::new(Config::default());
        let index: SharedIndex = Arc::new(RwLock::new(SearchIndex::default()));
        let mut items = Vec::new();
        items.extend(read_desktop_apps());
        items.extend(read_path_commands());
        items.extend(read_filesystem_items(&config));
        index.write().unwrap().items = items;

        let results = search(&index, "ls", 9);
        assert!(
            !matches!(results[0].item.kind, ItemKind::WebSearch),
            "real query `ls` should match indexed items, got web fallback"
        );

        for query in ["weather in tokyo", "how to cook rice", "asdkjqwe"] {
            let results = search(&index, query, 9);
            eprintln!("query `{query}`:");
            for r in &results {
                eprintln!("  {:?} score={} {}", r.item.kind, r.score, r.item.title);
            }
            assert!(
                matches!(results[0].item.kind, ItemKind::WebSearch),
                "query `{query}` did not fall back to web search"
            );
        }
    }

    #[test]
    fn slash_query_becomes_ai_prompt() {
        let index: SharedIndex = Arc::new(RwLock::new(SearchIndex::default()));
        let results = search(&index, "/what is Rust?", 9);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].item.kind, ItemKind::AiPrompt));
        assert_eq!(results[0].item.target, "what is Rust?");

        let empty = search(&index, "/", 9);
        assert!(matches!(empty[0].item.kind, ItemKind::AiPrompt));
        assert_eq!(empty[0].item.target, "");
    }

    #[test]
    fn request_completion_talks_to_openai_compatible_endpoint() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some(headers_end) = text.find("\r\n\r\n") {
                    let content_length = text
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(str::trim)
                                .map(String::from)
                        })
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0);
                    if request.len() >= headers_end + 4 + content_length {
                        break;
                    }
                }
            }
            let body =
                r#"{"choices":[{"message":{"role":"assistant","content":"Hello from mock"}}]}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&request).to_string()
        });

        let ai = AiConfig {
            base_url: format!("http://{addr}/v1"),
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
        };
        let text = request_completion(&ai, "hi there").unwrap();
        assert_eq!(text, "Hello from mock");

        let request = server.join().unwrap();
        assert!(
            request.starts_with("POST /v1/chat/completions"),
            "{request}"
        );
        assert!(request.contains("Bearer test-key"), "{request}");
        assert!(request.contains("test-model"), "{request}");
        assert!(request.contains("hi there"), "{request}");
    }

    #[test]
    fn ai_config_sanitize_trims_base_url() {
        let mut ai = AiConfig {
            base_url: "http://localhost:11434/v1/".to_string(),
            ..AiConfig::default()
        };
        ai.sanitize();
        assert_eq!(ai.base_url, "http://localhost:11434/v1");

        let mut empty = AiConfig {
            base_url: "  ".to_string(),
            ..AiConfig::default()
        };
        empty.sanitize();
        assert_eq!(empty.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn web_search_item_encodes_query() {
        let item = web_search_item("weather in tokyo?");
        assert_eq!(
            item.target,
            "https://www.google.com/search?q=weather+in+tokyo%3F"
        );
        assert!(matches!(item.kind, ItemKind::WebSearch));
    }
}

fn build_style(ui: &UiConfig) -> String {
    format!(
        r#"
window {{
  background-color: {};
  background-image: none;
  border-radius: {}px;
}}

#shell {{
  margin: {}px;
  padding: {}px;
  background-color: {};
  background-image: none;
  border: 1px solid {};
  border-radius: {}px;
  box-shadow: 0 18px 70px rgba(0, 0, 0, 0.38);
}}

#search {{
  min-height: {}px;
  padding: 0 16px;
  border: 0;
  border-radius: {}px;
  background-color: {};
  background-image: none;
  color: {};
  font-size: {}px;
}}

#search text {{
  background-color: transparent;
  background-image: none;
  color: {};
  caret-color: {};
}}

#search image {{
  color: {};
}}

#results {{
  margin-top: {}px;
  background-color: {};
  background-image: none;
}}

#result-row {{
  border-radius: {}px;
  background-color: {};
  background-image: none;
}}

#result-row:selected {{
  background-color: {};
  background-image: none;
}}

#icon {{
  color: {};
  font-size: {}px;
}}

#title {{
  color: {};
  font-size: {}px;
  font-weight: 600;
}}

#subtitle {{
  color: {};
  font-size: {}px;
}}

#response {{
  padding: 10px 12px;
  color: {};
  font-size: {}px;
}}
"#,
        ui.colors.window_background,
        ui.shell_radius + ui.shell_margin,
        ui.shell_margin,
        ui.shell_padding,
        ui.colors.shell_background,
        ui.colors.shell_border,
        ui.shell_radius,
        ui.search_height,
        ui.search_radius,
        ui.colors.search_background,
        ui.colors.search_text,
        ui.search_font_size,
        ui.colors.search_text,
        ui.colors.search_text,
        ui.colors.search_text,
        ui.result_margin_top,
        ui.colors.results_background,
        ui.result_row_radius,
        ui.colors.row_background,
        ui.colors.row_selected_background,
        ui.colors.icon,
        ui.icon_font_size,
        ui.colors.title,
        ui.title_font_size,
        ui.colors.subtitle,
        ui.subtitle_font_size,
        ui.colors.title,
        ui.subtitle_font_size + 2
    )
}
