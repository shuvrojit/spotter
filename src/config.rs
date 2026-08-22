use anyhow::{Context, Result};
use gtk::gdk;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    fs::{OpenOptions, Permissions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const CONFIG_DIR_NAME: &str = "spotter";
const CONFIG_FILE_NAME: &str = "config.toml";
const DEFAULT_MAX_RESULTS: usize = 9;
const DEFAULT_MAX_RECENT_SEARCHES: usize = 8;
const DEFAULT_MAX_RECENT_ITEMS: usize = 8;
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
pub(crate) struct Config {
    #[serde(default = "default_max_results")]
    pub(crate) max_results: usize,
    #[serde(default = "default_max_recent_searches")]
    pub(crate) max_recent_searches: usize,
    #[serde(default = "default_max_recent_items")]
    pub(crate) max_recent_items: usize,
    #[serde(default = "default_max_result_height")]
    pub(crate) max_result_height: i32,
    #[serde(default = "default_max_indexed_items")]
    pub(crate) max_indexed_items: usize,
    #[serde(default = "default_index_depth")]
    pub(crate) index_depth: usize,
    #[serde(default)]
    pub(crate) include_hidden: bool,
    #[serde(default)]
    pub(crate) include_path_binaries: bool,
    #[serde(default)]
    pub(crate) system_tray: bool,
    #[serde(default = "default_index_dirs")]
    pub(crate) index_dirs: Vec<String>,
    #[serde(default)]
    pub(crate) ai: AiConfig,
    #[serde(default)]
    pub(crate) ui: UiConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_results: default_max_results(),
            max_recent_searches: default_max_recent_searches(),
            max_recent_items: default_max_recent_items(),
            max_result_height: default_max_result_height(),
            max_indexed_items: default_max_indexed_items(),
            index_depth: default_index_depth(),
            include_hidden: false,
            include_path_binaries: false,
            system_tray: false,
            index_dirs: default_index_dirs(),
            ai: AiConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AiConfig {
    #[serde(default = "default_ai_base_url")]
    pub(crate) base_url: String,
    #[serde(default)]
    pub(crate) api_key: String,
    #[serde(default = "default_ai_model")]
    pub(crate) model: String,
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

    pub(crate) fn resolve_api_key(&self) -> Option<String> {
        if !self.api_key.trim().is_empty() {
            return Some(self.api_key.trim().to_string());
        }
        env::var("OPENAI_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct UiConfig {
    #[serde(default = "default_position")]
    pub(crate) position: String,
    #[serde(default)]
    pub(crate) x: i32,
    #[serde(default)]
    pub(crate) y: i32,
    #[serde(default = "default_window_width")]
    pub(crate) window_width: i32,
    #[serde(default = "default_max_result_height")]
    pub(crate) result_max_height: i32,
    #[serde(default = "default_shell_margin")]
    pub(crate) shell_margin: i32,
    #[serde(default = "default_shell_padding")]
    pub(crate) shell_padding: i32,
    #[serde(default = "default_shell_radius")]
    pub(crate) shell_radius: i32,
    #[serde(default = "default_search_height")]
    pub(crate) search_height: i32,
    #[serde(default = "default_search_radius")]
    pub(crate) search_radius: i32,
    #[serde(default = "default_search_font_size")]
    pub(crate) search_font_size: i32,
    #[serde(default = "default_result_margin_top")]
    pub(crate) result_margin_top: i32,
    #[serde(default = "default_result_row_padding_y")]
    pub(crate) result_row_padding_y: i32,
    #[serde(default = "default_result_row_padding_x")]
    pub(crate) result_row_padding_x: i32,
    #[serde(default = "default_result_row_radius")]
    pub(crate) result_row_radius: i32,
    #[serde(default = "default_title_font_size")]
    pub(crate) title_font_size: i32,
    #[serde(default = "default_subtitle_font_size")]
    pub(crate) subtitle_font_size: i32,
    #[serde(default = "default_icon_font_size")]
    pub(crate) icon_font_size: i32,
    #[serde(default)]
    pub(crate) colors: UiColors,
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
pub(crate) struct UiColors {
    #[serde(default = "default_window_background")]
    pub(crate) window_background: String,
    #[serde(default = "default_shell_background")]
    pub(crate) shell_background: String,
    #[serde(default = "default_shell_border")]
    pub(crate) shell_border: String,
    #[serde(default = "default_search_background")]
    pub(crate) search_background: String,
    #[serde(default = "default_search_text")]
    pub(crate) search_text: String,
    #[serde(default = "default_results_background")]
    pub(crate) results_background: String,
    #[serde(default = "default_row_background")]
    pub(crate) row_background: String,
    #[serde(default = "default_row_selected_background")]
    pub(crate) row_selected_background: String,
    #[serde(default = "default_icon_color")]
    pub(crate) icon: String,
    #[serde(default = "default_title_color")]
    pub(crate) title: String,
    #[serde(default = "default_subtitle_color")]
    pub(crate) subtitle: String,
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
pub(crate) enum WindowPosition {
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
    pub(crate) fn parse(value: &str) -> Option<Self> {
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

pub(crate) fn load() -> Result<Config> {
    let config_path = config_path()?;
    let config_dir = config_path
        .parent()
        .context("configuration path has no parent directory")?;

    if !config_path.exists() {
        fs::create_dir_all(config_dir)
            .with_context(|| format!("create {}", config_dir.display()))?;
        fs::write(&config_path, DEFAULT_CONFIG)
            .with_context(|| format!("write {}", config_path.display()))?;
        fs::set_permissions(&config_path, Permissions::from_mode(0o600))
            .with_context(|| format!("set permissions on {}", config_path.display()))?;
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let mut config: Config =
        toml::from_str(&content).with_context(|| format!("parse {}", config_path.display()))?;
    config.sanitize();
    Ok(config)
}

pub(crate) fn save(config: &Config) -> Result<()> {
    save_to(&config_path()?, config)
}

fn config_path() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .context("could not resolve config directory")?
        .join(CONFIG_DIR_NAME)
        .join(CONFIG_FILE_NAME))
}

fn save_to(path: &Path, config: &Config) -> Result<()> {
    let mut config = config.clone();
    config.sanitize();
    let content = toml::to_string_pretty(&config).context("serialize configuration")?;
    let parent = path
        .parent()
        .with_context(|| format!("configuration path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;

    let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    temp_name.push(format!(".{}.{unique}.tmp", std::process::id()));
    let temp_path = path.with_file_name(temp_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp_path)
        .with_context(|| format!("create {}", temp_path.display()))?;
    let result = (|| {
        file.write_all(content.as_bytes())
            .with_context(|| format!("write {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("sync {}", temp_path.display()))?;
        drop(file);
        fs::rename(&temp_path, path).with_context(|| format!("replace {}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

impl Config {
    fn sanitize(&mut self) {
        if self.max_results == 0 {
            self.max_results = DEFAULT_MAX_RESULTS;
        }
        self.max_recent_searches = self.max_recent_searches.min(50);
        self.max_recent_items = self.max_recent_items.min(50);
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

fn default_ai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_ai_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_max_results() -> usize {
    DEFAULT_MAX_RESULTS
}

fn default_max_recent_searches() -> usize {
    DEFAULT_MAX_RECENT_SEARCHES
}

fn default_max_recent_items() -> usize {
    DEFAULT_MAX_RECENT_ITEMS
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "spotter-config-test-{}-{unique}-{name}.toml",
            std::process::id()
        ))
    }

    #[test]
    fn example_config_is_valid() {
        let mut config: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        config.sanitize();
        assert_eq!(config.ui.position, "top-left");
        assert_eq!(config.ui.window_width, 720);
        assert_eq!(config.ui.search_height, 54);
        assert_eq!(config.max_recent_searches, 8);
        assert_eq!(config.max_recent_items, 8);
        assert!(!config.include_path_binaries);
        assert!(!config.system_tray);
    }

    #[test]
    fn save_round_trips_a_sanitized_private_config() {
        let path = test_config_path("save");
        let config = Config {
            max_recent_items: 99,
            index_dirs: Vec::new(),
            ui: UiConfig {
                window_width: 100,
                ..UiConfig::default()
            },
            ..Config::default()
        };

        save_to(&path, &config).unwrap();

        let saved: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.max_recent_items, 50);
        assert_eq!(saved.ui.window_width, 320);
        assert_eq!(saved.index_dirs, default_index_dirs());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        fs::remove_file(path).unwrap();
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
    fn recent_search_limit_can_be_disabled_and_is_capped() {
        let mut config = Config {
            max_recent_searches: 51,
            ..Config::default()
        };
        config.sanitize();
        assert_eq!(config.max_recent_searches, 50);

        config.max_recent_searches = 0;
        config.sanitize();
        assert_eq!(config.max_recent_searches, 0);
    }

    #[test]
    fn recent_item_limit_can_be_disabled_and_is_capped() {
        let mut config = Config {
            max_recent_items: 51,
            ..Config::default()
        };
        config.sanitize();
        assert_eq!(config.max_recent_items, 50);

        config.max_recent_items = 0;
        config.sanitize();
        assert_eq!(config.max_recent_items, 0);
    }

    #[test]
    fn system_tray_can_be_enabled() {
        let config: Config = toml::from_str("system_tray = true").unwrap();
        assert!(config.system_tray);
    }

    #[test]
    fn path_binaries_can_be_enabled() {
        let config: Config = toml::from_str("include_path_binaries = true").unwrap();
        assert!(config.include_path_binaries);
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
}
