use crate::search::RecentItem;
use anyhow::{Context, Result};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

const DATA_DIR_NAME: &str = "spotter";
const HISTORY_FILE_NAME: &str = "recent-searches.json";
const RECENT_ITEMS_FILE_NAME: &str = "recent-items.json";

pub(crate) struct RecentSearches {
    path: Option<PathBuf>,
    entries: Vec<String>,
    limit: usize,
}

pub(crate) struct RecentItems {
    path: Option<PathBuf>,
    entries: Vec<RecentItem>,
    limit: usize,
}

impl RecentItems {
    pub(crate) fn load(limit: usize) -> Result<Self> {
        Self::load_from(default_file_path(RECENT_ITEMS_FILE_NAME)?, limit)
    }

    pub(crate) fn empty(limit: usize) -> Self {
        Self {
            path: default_file_path(RECENT_ITEMS_FILE_NAME).ok(),
            entries: Vec::new(),
            limit,
        }
    }

    pub(crate) fn entries(&self) -> &[RecentItem] {
        &self.entries
    }

    pub(crate) fn record(&mut self, mut item: RecentItem) -> Result<()> {
        item.title = item.title.trim().to_string();
        item.subtitle = item.subtitle.trim().to_string();
        item.target = item.target.trim().to_string();
        if self.limit == 0
            || item.title.is_empty()
            || item.target.is_empty()
            || !item.is_supported()
        {
            return Ok(());
        }

        self.entries
            .retain(|entry| entry.kind != item.kind || entry.target != item.target);
        self.entries.insert(0, item);
        self.entries.truncate(self.limit);
        self.persist()
    }

    fn load_from(path: PathBuf, limit: usize) -> Result<Self> {
        let entries = match fs::read(&path) {
            Ok(content) => serde_json::from_slice::<Vec<RecentItem>>(&content)
                .with_context(|| format!("parse {}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };

        Ok(Self {
            path: Some(path),
            entries: clean_recent_items(entries, limit),
            limit,
        })
    }

    fn persist(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        persist_entries(path, &self.entries, "serialize recent items")
    }
}

impl RecentSearches {
    pub(crate) fn load(limit: usize) -> Result<Self> {
        Self::load_from(default_file_path(HISTORY_FILE_NAME)?, limit)
    }

    pub(crate) fn empty(limit: usize) -> Self {
        Self {
            path: default_file_path(HISTORY_FILE_NAME).ok(),
            entries: Vec::new(),
            limit,
        }
    }

    pub(crate) fn entries(&self) -> &[String] {
        &self.entries
    }

    pub(crate) fn record(&mut self, query: &str) -> Result<()> {
        let query = query.trim();
        if query.is_empty() || self.limit == 0 {
            return Ok(());
        }

        let query_key = query.to_lowercase();
        self.entries
            .retain(|entry| entry.to_lowercase() != query_key);
        self.entries.insert(0, query.to_string());
        self.entries.truncate(self.limit);
        self.persist()
    }

    fn load_from(path: PathBuf, limit: usize) -> Result<Self> {
        let entries = match fs::read(&path) {
            Ok(content) => serde_json::from_slice::<Vec<String>>(&content)
                .with_context(|| format!("parse {}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };

        Ok(Self {
            path: Some(path),
            entries: clean_entries(entries, limit),
            limit,
        })
    }

    fn persist(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        persist_entries(path, &self.entries, "serialize search history")
    }
}

fn persist_entries<T: serde::Serialize>(
    path: &Path,
    entries: &[T],
    serialization_context: &str,
) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("history path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;

    let temp_path = temporary_path(path);
    let content = serde_json::to_vec_pretty(entries).context(serialization_context.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temp_path)
        .with_context(|| format!("create {}", temp_path.display()))?;
    file.write_all(&content)
        .with_context(|| format!("write {}", temp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("sync {}", temp_path.display()))?;
    fs::rename(&temp_path, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

fn default_file_path(file_name: &str) -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")))
        .context("could not resolve local data directory")?;
    Ok(data_dir.join(DATA_DIR_NAME).join(file_name))
}

fn clean_entries(entries: Vec<String>, limit: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .filter(|entry| seen.insert(entry.to_lowercase()))
        .take(limit)
        .collect()
}

fn clean_recent_items(entries: Vec<RecentItem>, limit: usize) -> Vec<RecentItem> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .map(|mut entry| {
            entry.title = entry.title.trim().to_string();
            entry.subtitle = entry.subtitle.trim().to_string();
            entry.target = entry.target.trim().to_string();
            entry
        })
        .filter(|entry| entry.is_supported() && !entry.title.is_empty() && !entry.target.is_empty())
        .filter(|entry| seen.insert((entry.kind.clone(), entry.target.clone())))
        .take(limit)
        .collect()
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::ItemKind;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "spotter-history-test-{}-{unique}-{name}.json",
            std::process::id()
        ))
    }

    fn recent_item(title: &str, target: &str, kind: ItemKind) -> RecentItem {
        RecentItem {
            title: title.to_string(),
            subtitle: String::new(),
            target: target.to_string(),
            kind,
            desktop_icon: None,
        }
    }

    #[test]
    fn recent_items_store_apps_and_web_searches_newest_first() {
        let path = test_path("recent-items");
        let mut history = RecentItems::load_from(path.clone(), 3).unwrap();
        history
            .record(recent_item("Firefox", "firefox", ItemKind::Application))
            .unwrap();
        history
            .record(recent_item(
                "Search Google for Rust",
                "https://www.google.com/search?q=Rust",
                ItemKind::WebSearch,
            ))
            .unwrap();
        history
            .record(recent_item(
                "Firefox updated",
                "firefox",
                ItemKind::Application,
            ))
            .unwrap();
        history
            .record(recent_item("Ignored command", "/bin/ls", ItemKind::Command))
            .unwrap();

        assert_eq!(history.entries().len(), 2);
        assert_eq!(history.entries()[0].title, "Firefox updated");
        assert!(matches!(history.entries()[1].kind, ItemKind::WebSearch));

        let reloaded = RecentItems::load_from(path.clone(), 3).unwrap();
        assert_eq!(reloaded.entries().len(), 2);
        assert_eq!(reloaded.entries()[0].target, "firefox");

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn records_newest_first_and_moves_duplicates_to_the_front() {
        let path = test_path("ordering");
        let mut history = RecentSearches::load_from(path.clone(), 3).unwrap();
        history.record("Firefox").unwrap();
        history.record("terminal").unwrap();
        history.record("firefox").unwrap();
        history.record("settings").unwrap();
        history.record("files").unwrap();

        assert_eq!(history.entries(), ["files", "settings", "firefox"]);
        let reloaded = RecentSearches::load_from(path.clone(), 3).unwrap();
        assert_eq!(reloaded.entries(), history.entries());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn load_cleans_empty_and_duplicate_entries() {
        let path = test_path("clean");
        fs::write(
            &path,
            serde_json::to_vec(&vec![" Alpha ", "", "alpha", "Beta"]).unwrap(),
        )
        .unwrap();

        let history = RecentSearches::load_from(path.clone(), 5).unwrap();
        assert_eq!(history.entries(), ["Alpha", "Beta"]);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn zero_limit_disables_history() {
        let path = test_path("disabled");
        let mut history = RecentSearches::load_from(path.clone(), 0).unwrap();
        history.record("ignored").unwrap();

        assert!(history.entries().is_empty());
        assert!(!path.exists());
    }
}
