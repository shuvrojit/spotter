use crate::config::Config;
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    env,
    ffi::OsStr,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread,
    time::Instant,
};
use walkdir::{DirEntry, WalkDir};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ItemKind {
    Application,
    Command,
    File,
    Directory,
    WebSearch,
    AiPrompt,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchItem {
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) target: String,
    pub(crate) kind: ItemKind,
    tokens: String,
    pub(crate) desktop_icon: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchResult {
    pub(crate) item: SearchItem,
    score: i64,
}

#[derive(Default)]
pub(crate) struct SearchIndex {
    items: Vec<SearchItem>,
    complete: bool,
}

pub(crate) type SharedIndex = Arc<RwLock<SearchIndex>>;

pub(crate) fn empty_index() -> SharedIndex {
    Arc::new(RwLock::new(SearchIndex::default()))
}

pub(crate) fn spawn_indexer(
    index: SharedIndex,
    config: Arc<Config>,
) -> async_channel::Receiver<()> {
    let (updates, receiver) = async_channel::bounded(2);
    thread::spawn(move || {
        let started = Instant::now();
        let mut items = Vec::new();
        items.extend(read_desktop_apps());
        items.extend(read_path_commands());
        let mut seen = HashSet::new();
        items.retain(|item| seen.insert((item.kind.clone(), item.target.clone())));

        if let Ok(mut index) = index.write() {
            index.items = items;
        }
        let _ = updates.send_blocking(());

        let files = read_filesystem_items(&config);
        if let Ok(mut index) = index.write() {
            let mut seen: HashSet<_> = index
                .items
                .iter()
                .map(|item| (item.kind.clone(), item.target.clone()))
                .collect();
            index.items.extend(
                files
                    .into_iter()
                    .filter(|item| seen.insert((item.kind.clone(), item.target.clone()))),
            );
            index.complete = true;
        }
        let _ = updates.send_blocking(());
        eprintln!("indexed in {:?}", started.elapsed());
    });
    receiver
}

pub(crate) fn is_complete(index: &SharedIndex) -> bool {
    index.read().map(|index| index.complete).unwrap_or(false)
}

pub(crate) fn search(index: &SharedIndex, query: &str, max_results: usize) -> Vec<SearchResult> {
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
    let (items, complete) = match index.read() {
        Ok(index) => (index.items.clone(), index.complete),
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

    if results.is_empty() && complete {
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
        desktop_icon: None,
    }
}

fn web_search_item(query: &str) -> SearchItem {
    SearchItem {
        title: format!("Search Google for \"{query}\""),
        subtitle: "Open in default browser".to_string(),
        target: format!("https://www.google.com/search?q={}", percent_encode(query)),
        kind: ItemKind::WebSearch,
        tokens: query.to_string(),
        desktop_icon: None,
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

// Every query term must appear as a contiguous substring; scattered character
// matches are ignored so unrelated queries fall through to web search.
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
        ItemKind::File | ItemKind::WebSearch | ItemKind::AiPrompt => 0,
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
    Ok(parse_desktop_item(&content))
}

fn parse_desktop_item(content: &str) -> Option<SearchItem> {
    let fields = parse_desktop_fields(content);

    if fields.get("NoDisplay").is_some_and(|value| value == "true")
        || fields.get("Hidden").is_some_and(|value| value == "true")
    {
        return None;
    }

    let name = fields.get("Name")?.clone();
    let exec = fields.get("Exec")?.clone();
    let comment = fields.get("Comment").cloned().unwrap_or_default();
    let desktop_icon = fields
        .get("Icon")
        .map(|icon| icon.trim())
        .filter(|icon| !icon.is_empty())
        .map(String::from);
    let target = clean_desktop_exec(&exec);
    let tokens = format!("{name} {comment} {target}").to_lowercase();

    Some(SearchItem {
        title: name,
        subtitle: if comment.is_empty() {
            target.clone()
        } else {
            comment
        },
        target,
        kind: ItemKind::Application,
        tokens,
        desktop_icon,
    })
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
        desktop_icon: None,
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
        desktop_icon: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn score(tokens: &str, query: &str) -> Option<i64> {
        let terms: Vec<&str> = query.split_whitespace().collect();
        match_score(tokens, query, &terms)
    }

    #[test]
    fn desktop_entry_preserves_original_icon() {
        let item = parse_desktop_item(
            "[Desktop Entry]\nName=Example App\nComment=An example\nExec=example-app %U\nIcon=example-app\n",
        )
        .unwrap();

        assert!(matches!(item.kind, ItemKind::Application));
        assert_eq!(item.desktop_icon.as_deref(), Some("example-app"));
        assert_eq!(item.target, "example-app");

        let without_icon =
            parse_desktop_item("[Desktop Entry]\nName=No Icon\nExec=no-icon\n").unwrap();
        assert_eq!(without_icon.desktop_icon, None);
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
        let index = empty_index();
        let mut items = Vec::new();
        items.extend(read_desktop_apps());
        items.extend(read_path_commands());
        items.extend(read_filesystem_items(&config));
        {
            let mut index = index.write().unwrap();
            index.items = items;
            index.complete = true;
        }

        let results = search(&index, "ls", 9);
        assert!(
            !matches!(results[0].item.kind, ItemKind::WebSearch),
            "real query `ls` should match indexed items, got web fallback"
        );

        for query in ["weather in tokyo", "how to cook rice", "asdkjqwe"] {
            let results = search(&index, query, 9);
            eprintln!("query `{query}`:");
            for result in &results {
                eprintln!(
                    "  {:?} score={} {}",
                    result.item.kind, result.score, result.item.title
                );
            }
            assert!(
                matches!(results[0].item.kind, ItemKind::WebSearch),
                "query `{query}` did not fall back to web search"
            );
        }
    }

    #[test]
    fn incomplete_index_does_not_fall_back_to_web_search() {
        let index = empty_index();
        assert!(search(&index, "firefox", 9).is_empty());

        index.write().unwrap().complete = true;
        let results = search(&index, "firefox", 9);
        assert!(matches!(results[0].item.kind, ItemKind::WebSearch));
    }

    #[test]
    fn slash_query_becomes_ai_prompt() {
        let index = empty_index();
        let results = search(&index, "/what is Rust?", 9);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].item.kind, ItemKind::AiPrompt));
        assert_eq!(results[0].item.target, "what is Rust?");

        let empty = search(&index, "/", 9);
        assert!(matches!(empty[0].item.kind, ItemKind::AiPrompt));
        assert_eq!(empty[0].item.target, "");
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
