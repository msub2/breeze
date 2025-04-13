use std::sync::{LazyLock, Mutex};

use url::Url;

use crate::protocols::Protocol;

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
    let mut history = HISTORY.lock().unwrap();
    let mut index = HISTORY_INDEX.lock().unwrap();

    // If we're not at the end, truncate the forward history
    if *index + 1 < history.len() {
        history.truncate(*index + 1);
    }

    // Only add if different from current
    if history.last().is_none_or(|e| e.url != url) {
        history.push(HistoryEntry::new(url, protocol));
        *index = history.len() - 1;  // Point to the newly added URL
    }
}

pub fn back() -> Option<HistoryEntry> {
    let history = HISTORY.lock().unwrap();
    let mut index = HISTORY_INDEX.lock().unwrap();
    if *index > 0 {
        *index -= 1;
        history.get(*index).cloned()
    } else {
        None
    }
}

pub fn forward() -> Option<HistoryEntry> {
    let history = HISTORY.lock().unwrap();
    let mut index = HISTORY_INDEX.lock().unwrap();
    if *index + 1 < history.len() {
        *index += 1;
        history.get(*index).cloned()
    } else {
        None
    }
}

pub fn can_go_forward() -> bool {
    let index = HISTORY_INDEX.lock().unwrap();
    *index + 1 < HISTORY.lock().unwrap().len()
}

pub fn can_go_back() -> bool {
    let index = HISTORY_INDEX.lock().unwrap();
    *index > 0
}
