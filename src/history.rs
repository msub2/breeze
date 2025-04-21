use std::sync::{LazyLock, Mutex, MutexGuard};

use url::Url;

use crate::handlers::Protocol;

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub url: Url,
    pub protocol: Protocol,
}

impl HistoryEntry {
    fn new(url: Url, protocol: Protocol) -> Self {
        Self { url, protocol }
    }
}

static HISTORY: LazyLock<Mutex<Vec<HistoryEntry>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static HISTORY_INDEX: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

pub fn add_entry(url: Url, protocol: Protocol) {
    let mut history = history();
    let mut index = index();

    // If we're not at the end, truncate the forward history
    if *index + 1 < history.len() {
        history.truncate(*index + 1);
    }

    // Only add if different from current
    if history.last().is_none_or(|e| e.url != url) {
        history.push(HistoryEntry::new(url, protocol));
        *index = history.len() - 1; // Point to the newly added URL
    }
}

// Silencing lint since I plan to allow more fine-grained history handling at some point
#[allow(dead_code)]
pub fn remove_entry(index: usize) -> HistoryEntry {
    let mut history = history();
    history.remove(index)
}

pub fn remove_latest_entry() -> HistoryEntry {
    let mut history = history();
    history.pop().unwrap()
}

pub fn back() -> Option<HistoryEntry> {
    let history = history();
    let mut index = index();
    if *index > 0 {
        *index -= 1;
        history.get(*index).cloned()
    } else {
        None
    }
}

pub fn forward() -> Option<HistoryEntry> {
    let history = history();
    let mut index = index();
    if *index + 1 < history.len() {
        *index += 1;
        history.get(*index).cloned()
    } else {
        None
    }
}

pub fn can_go_forward() -> bool {
    let index = index();
    *index + 1 < history().len()
}

pub fn can_go_back() -> bool {
    let index = index();
    *index > 0
}

fn history() -> MutexGuard<'static, Vec<HistoryEntry>> {
    HISTORY.lock().expect("Failed to lock history mutex")
}

fn index() -> MutexGuard<'static, usize> {
    HISTORY_INDEX.lock().expect("Failed to lock history index mutex")
}