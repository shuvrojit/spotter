use crate::{
    ai,
    config::{self, Config, UiConfig},
    history::RecentSearches,
    platform,
    search::{self, ItemKind, SearchItem, SearchResult},
    PRODUCT_NAME,
};
use anyhow::Result;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, EventControllerKey, Label, ListBox,
    ListBoxRow, Orientation, ScrolledWindow, SearchEntry,
};
use std::{
    cell::RefCell,
    path::Path,
    process::Command,
    rc::Rc,
    sync::{Arc, RwLock},
    time::Instant,
};

pub(crate) fn build(app: &Application) {
    let (config, config_error) = match config::load() {
        Ok(config) => (config, None),
        Err(error) => {
            eprintln!("failed to load config, using defaults: {error:#}");
            let message = "Configuration error in config.toml. Check its TOML syntax, then restart Spotter. Defaults are active for this launch.".to_string();
            (Config::default(), Some(message))
        }
    };
    let history = match RecentSearches::load(config.max_recent_searches) {
        Ok(history) => history,
        Err(error) => {
            eprintln!("failed to load recent searches: {error:#}");
            RecentSearches::empty(config.max_recent_searches)
        }
    };
    let history = Rc::new(RefCell::new(history));
    let config = Arc::new(config);
    let index = search::empty_index();

    let window = ApplicationWindow::builder()
        .application(app)
        .title(PRODUCT_NAME)
        .default_width(config.ui.window_width)
        .resizable(false)
        .decorated(false)
        .build();
    window.set_size_request(config.ui.window_width, -1);
    window.set_overflow(gtk::Overflow::Hidden);

    let provider = gtk::CssProvider::new();
    provider.load_from_data(&build_style(&config.ui));
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
        .max_content_height(config.ui.result_max_height)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&content)
        .build();
    scroll.set_visible(false);

    root.append(&input);
    root.append(&scroll);
    window.set_child(Some(&root));

    let initial_results =
        search::recent_search_results(history.borrow().entries(), config.max_recent_searches);
    let state = Arc::new(RwLock::new(initial_results.clone()));
    render_results(&list, &scroll, &initial_results, "", false, &config.ui);
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
        let input = input.clone();
        let history = history.clone();
        Rc::new(move |item: &SearchItem| {
            if matches!(item.kind, ItemKind::RecentSearch) {
                input.set_text(&item.target);
                input.set_position(-1);
                input.grab_focus();
                return;
            }
            if matches!(item.kind, ItemKind::AiPrompt) && item.target.is_empty() {
                return;
            }

            if let Err(error) = history.borrow_mut().record(&input.text()) {
                eprintln!("failed to save recent search: {error:#}");
            }

            if matches!(item.kind, ItemKind::AiPrompt) {
                list.set_visible(false);
                scroll.set_visible(true);
                response.set_text("Thinking…");
                response.set_visible(true);
                ai::ask(&config.ai, &item.target, &response);
                return;
            }
            if let Err(error) = launch(item) {
                eprintln!("launch failed: {error:#}");
            } else {
                app.quit();
            }
        })
    };

    let refresh: Rc<dyn Fn(&str)> = {
        let list = list.clone();
        let scroll = scroll.clone();
        let response = response.clone();
        let index = index.clone();
        let state = state.clone();
        let config = config.clone();
        let window = window.clone();
        let history = history.clone();
        Rc::new(move |query: &str| {
            let now = Instant::now();
            let query_is_empty = query.trim().is_empty();
            let results = if query_is_empty {
                search::recent_search_results(
                    history.borrow().entries(),
                    config.max_recent_searches,
                )
            } else {
                search::search(&index, query, config.max_results)
            };
            let indexing = !query_is_empty && !search::is_complete(&index);
            if let Ok(mut state) = state.write() {
                *state = results.clone();
            }
            response.set_visible(false);
            list.set_visible(true);
            render_results(&list, &scroll, &results, query, indexing, &config.ui);
            platform::schedule_reposition(&window, config.ui.clone());
            eprintln!(
                "search `{query}`: {} results in {:?}",
                results.len(),
                now.elapsed()
            );
        })
    };

    {
        let refresh = refresh.clone();
        input.connect_search_changed(move |entry| {
            refresh(&entry.text());
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
    platform::schedule_position(&window, config.ui.clone());
    let updates = search::spawn_indexer(index, config);
    {
        let input = input.clone();
        let refresh = refresh.clone();
        glib::spawn_future_local(async move {
            while updates.recv().await.is_ok() {
                let query = input.text().to_string();
                if !query.trim().is_empty() {
                    refresh(&query);
                }
            }
        });
    }
    input.grab_focus();
}

fn render_results(
    list: &ListBox,
    scroll: &ScrolledWindow,
    results: &[SearchResult],
    query: &str,
    indexing: bool,
    ui: &UiConfig,
) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }

    if query.trim().is_empty() && results.is_empty() {
        scroll.set_visible(false);
        return;
    }

    scroll.set_visible(true);
    if results.is_empty() {
        list.append(&result_row(
            if indexing {
                "Indexing…"
            } else {
                "No results"
            },
            if indexing {
                "Applications, commands, and files will appear automatically"
            } else {
                "Keep typing or try another query"
            },
            "",
            None,
            ui,
        ));
    } else {
        for result in results {
            let icon = match result.item.kind {
                ItemKind::RecentSearch => "↶",
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
                result.item.desktop_icon.as_deref(),
                ui,
            ));
        }
        list.select_row(list.row_at_index(0).as_ref());
    }
}

fn result_row(
    title: &str,
    subtitle: &str,
    fallback_icon: &str,
    desktop_icon: Option<&str>,
    ui: &UiConfig,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_widget_name("result-row");

    let layout = GtkBox::new(Orientation::Horizontal, 14);
    layout.set_margin_top(ui.result_row_padding_y);
    layout.set_margin_bottom(ui.result_row_padding_y);
    layout.set_margin_start(ui.result_row_padding_x);
    layout.set_margin_end(ui.result_row_padding_x);

    let icon = GtkBox::new(Orientation::Horizontal, 0);
    icon.set_size_request(ui.icon_font_size + 8, ui.icon_font_size + 8);
    icon.set_halign(Align::Center);
    icon.set_valign(Align::Center);
    if let Some(image) = desktop_icon.and_then(|name| load_desktop_icon(name, ui.icon_font_size)) {
        icon.append(&image);
    } else {
        let fallback = Label::new(Some(fallback_icon));
        fallback.set_widget_name("icon");
        fallback.set_width_chars(2);
        icon.append(&fallback);
    }

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

fn load_desktop_icon(name: &str, size: i32) -> Option<gtk::Image> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    let path = Path::new(name);
    let image = if path.is_absolute() {
        if !path.is_file() {
            return None;
        }
        gtk::Image::from_file(path)
    } else {
        let display = gdk::Display::default()?;
        let theme = gtk::IconTheme::for_display(&display);
        if !theme.has_icon(name) {
            return None;
        }
        gtk::Image::from_icon_name(name)
    };

    image.set_widget_name("app-icon");
    image.set_pixel_size(size);
    Some(image)
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
        ItemKind::RecentSearch => {}
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
        ItemKind::AiPrompt => {}
    }
    Ok(())
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
