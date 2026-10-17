use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;

use crate::model::{BucketInfo, KeyInfo, Snapshot};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Buckets,
    Keys,
    Cluster,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Buckets, Tab::Keys, Tab::Cluster];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Buckets => "Buckets",
            Tab::Keys => "Keys",
            Tab::Cluster => "Cluster",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    fn next(self) -> Tab {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Size,
    Objects,
}

impl SortBy {
    pub fn label(self) -> &'static str {
        match self {
            SortBy::Name => "name",
            SortBy::Size => "size",
            SortBy::Objects => "objects",
        }
    }

    fn next(self) -> SortBy {
        match self {
            SortBy::Name => SortBy::Size,
            SortBy::Size => SortBy::Objects,
            SortBy::Objects => SortBy::Name,
        }
    }
}

pub enum Action {
    None,
    Quit,
    Refresh,
}

pub struct App {
    pub tab: Tab,
    pub snapshot: Option<Snapshot>,
    pub buckets_state: TableState,
    pub keys_state: TableState,
    pub filter: String,
    pub editing_filter: bool,
    pub sort: SortBy,
    pub show_secret: bool,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub last_refresh: Option<String>,
    pub loading: bool,
}

impl App {
    pub fn new(warning: Option<String>) -> Self {
        Self {
            tab: Tab::Buckets,
            snapshot: None,
            buckets_state: TableState::default().with_selected(0),
            keys_state: TableState::default().with_selected(0),
            filter: String::new(),
            editing_filter: false,
            sort: SortBy::Name,
            show_secret: false,
            error: None,
            warning,
            last_refresh: None,
            loading: true,
        }
    }

    pub fn set_result(&mut self, result: Result<Snapshot, String>) {
        self.loading = false;
        match result {
            Ok(snapshot) => {
                self.snapshot = Some(snapshot);
                self.error = None;
                self.last_refresh = Some(clock());
            }
            // Keep the stale data on screen, only surface the error.
            Err(err) => self.error = Some(err),
        }
        self.clamp_selection();
    }

    /// Buckets after filter and sort.
    pub fn visible_buckets(&self) -> Vec<&BucketInfo> {
        let filter = self.filter.to_lowercase();
        let mut list: Vec<&BucketInfo> = self
            .snapshot
            .iter()
            .flat_map(|s| s.buckets.iter())
            .filter(|b| filter.is_empty() || b.name().to_lowercase().contains(&filter))
            .collect();
        match self.sort {
            SortBy::Name => list.sort_by_key(|b| b.name()),
            SortBy::Size => list.sort_by_key(|b| std::cmp::Reverse(b.bytes)),
            SortBy::Objects => list.sort_by_key(|b| std::cmp::Reverse(b.objects)),
        }
        list
    }

    /// Keys after filter (by name or access key id), sorted by name.
    pub fn visible_keys(&self) -> Vec<&KeyInfo> {
        let filter = self.filter.to_lowercase();
        let mut list: Vec<&KeyInfo> = self
            .snapshot
            .iter()
            .flat_map(|s| s.keys.iter())
            .filter(|k| {
                filter.is_empty()
                    || k.name.to_lowercase().contains(&filter)
                    || k.access_key_id.to_lowercase().contains(&filter)
            })
            .collect();
        list.sort_by_key(|k| k.name.as_str());
        list
    }

    fn list_len(&self) -> usize {
        match self.tab {
            Tab::Buckets => self.visible_buckets().len(),
            Tab::Keys => self.visible_keys().len(),
            Tab::Cluster => 0,
        }
    }

    fn state_mut(&mut self) -> Option<&mut TableState> {
        match self.tab {
            Tab::Buckets => Some(&mut self.buckets_state),
            Tab::Keys => Some(&mut self.keys_state),
            Tab::Cluster => None,
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.list_len();
        if let Some(state) = self.state_mut() {
            state.select(if len == 0 {
                None
            } else {
                Some(state.selected().unwrap_or(0).min(len - 1))
            });
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.list_len();
        if len == 0 {
            return;
        }
        if let Some(state) = self.state_mut() {
            let current = state.selected().unwrap_or(0) as isize;
            state.select(Some((current + delta).clamp(0, len as isize - 1) as usize));
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        if self.editing_filter {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => self.editing_filter = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c) => self.filter.push(c),
                _ => {}
            }
            self.clamp_selection();
            return Action::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }
        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Tab => self.tab = self.tab.next(),
            KeyCode::Char('1') => self.tab = Tab::Buckets,
            KeyCode::Char('2') => self.tab = Tab::Keys,
            KeyCode::Char('3') => self.tab = Tab::Cluster,
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Char('/') => self.editing_filter = true,
            KeyCode::Esc => self.filter.clear(),
            KeyCode::Char('s') => self.sort = self.sort.next(),
            KeyCode::Char('S') => {
                self.show_secret = !self.show_secret;
                self.loading = true;
                return Action::Refresh;
            }
            KeyCode::Char('r') => {
                self.loading = true;
                return Action::Refresh;
            }
            _ => {}
        }
        self.clamp_selection();
        Action::None
    }
}

/// Current UTC time as HH:MM:SS.
fn clock() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "{:02}:{:02}:{:02} UTC",
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60
    )
}
