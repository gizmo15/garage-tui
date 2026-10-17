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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::sample_snapshot;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn press(app: &mut App, c: char) -> Action {
        app.on_key(key(KeyCode::Char(c)))
    }

    fn loaded() -> App {
        let mut app = App::new(None);
        app.set_result(Ok(sample_snapshot()));
        app
    }

    fn names(app: &App) -> Vec<String> {
        app.visible_buckets().iter().map(|b| b.name()).collect()
    }

    #[test]
    fn set_result_stores_data_and_keeps_stale_on_error() {
        let mut app = App::new(Some("warn".into()));
        assert!(app.loading && app.snapshot.is_none());
        app.set_result(Ok(sample_snapshot()));
        assert!(!app.loading && app.error.is_none());
        assert!(app.last_refresh.as_deref().unwrap().ends_with("UTC"));

        app.set_result(Err("boom".into()));
        assert_eq!(app.error.as_deref(), Some("boom"));
        assert!(app.snapshot.is_some());
        app.set_result(Ok(sample_snapshot()));
        assert!(app.error.is_none());
    }

    #[test]
    fn buckets_sorted_by_name_size_objects() {
        let mut app = loaded();
        assert_eq!(names(&app), ["archives", "b3b3b3b3b3b3", "photos"]);
        press(&mut app, 's');
        assert_eq!(app.sort, SortBy::Size);
        assert_eq!(names(&app), ["photos", "b3b3b3b3b3b3", "archives"]);
        press(&mut app, 's');
        assert_eq!(app.sort.label(), "objects");
        assert_eq!(names(&app), ["archives", "photos", "b3b3b3b3b3b3"]);
        press(&mut app, 's');
        assert_eq!(app.sort.label(), "name");
    }

    #[test]
    fn filter_applies_to_buckets_and_keys() {
        let mut app = loaded();
        press(&mut app, '/');
        assert!(app.editing_filter);
        // 'q' and digits are text while editing, not commands
        for c in "pHo".chars() {
            press(&mut app, c);
        }
        assert_eq!(names(&app), ["photos"]);
        app.on_key(key(KeyCode::Backspace));
        assert_eq!(app.filter, "pH");
        app.on_key(key(KeyCode::Enter));
        assert!(!app.editing_filter);

        app.tab = Tab::Keys;
        app.filter = "gk2".into();
        assert_eq!(app.visible_keys().len(), 1);
        app.filter = "backup".into();
        assert_eq!(app.visible_keys()[0].access_key_id, "GK1");

        app.on_key(key(KeyCode::Esc));
        assert!(app.filter.is_empty());
        assert_eq!(app.visible_keys().len(), 2);
    }

    #[test]
    fn filter_edit_can_be_cancelled_with_esc() {
        let mut app = loaded();
        press(&mut app, '/');
        press(&mut app, 'z');
        app.on_key(key(KeyCode::Esc));
        assert!(!app.editing_filter);
        // no match: selection is cleared and moving does nothing
        assert!(app.buckets_state.selected().is_none());
        press(&mut app, 'j');
        assert!(app.buckets_state.selected().is_none());
    }

    #[test]
    fn navigation_is_clamped() {
        let mut app = loaded();
        assert_eq!(app.buckets_state.selected(), Some(0));
        press(&mut app, 'k');
        assert_eq!(app.buckets_state.selected(), Some(0));
        for _ in 0..10 {
            app.on_key(key(KeyCode::Down));
        }
        assert_eq!(app.buckets_state.selected(), Some(2));
        app.on_key(key(KeyCode::Up));
        assert_eq!(app.buckets_state.selected(), Some(1));

        app.tab = Tab::Keys;
        press(&mut app, 'j');
        press(&mut app, 'j');
        assert_eq!(app.keys_state.selected(), Some(1));
    }

    #[test]
    fn selection_survives_shrinking_list() {
        let mut app = loaded();
        app.buckets_state.select(Some(2));
        app.filter = "arch".into();
        app.set_result(Ok(sample_snapshot()));
        assert_eq!(app.buckets_state.selected(), Some(0));
    }

    #[test]
    fn tabs_switch_by_key_and_cycle() {
        let mut app = loaded();
        press(&mut app, '2');
        assert!(app.tab == Tab::Keys);
        press(&mut app, '3');
        assert!(app.tab == Tab::Cluster);
        // navigation on the cluster tab is a no-op
        press(&mut app, 'j');
        press(&mut app, '1');
        assert!(app.tab == Tab::Buckets);
        app.on_key(key(KeyCode::Tab));
        app.on_key(key(KeyCode::Tab));
        app.on_key(key(KeyCode::Tab));
        assert!(app.tab == Tab::Buckets);
        assert_eq!(Tab::Cluster.title(), "Cluster");
        assert_eq!(Tab::Keys.index(), 1);
    }

    #[test]
    fn quit_and_refresh_actions() {
        let mut app = loaded();
        assert!(matches!(press(&mut app, 'q'), Action::Quit));
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(app.on_key(ctrl_c), Action::Quit));
        assert!(matches!(press(&mut app, 'x'), Action::None));

        app.loading = false;
        assert!(matches!(press(&mut app, 'r'), Action::Refresh));
        assert!(app.loading);

        assert!(!app.show_secret);
        assert!(matches!(press(&mut app, 'S'), Action::Refresh));
        assert!(app.show_secret);
    }

    #[test]
    fn empty_app_has_no_rows() {
        let mut app = App::new(None);
        assert!(app.visible_buckets().is_empty() && app.visible_keys().is_empty());
        press(&mut app, 'j');
        app.set_result(Err("down".into()));
        assert!(app.buckets_state.selected().is_none());
    }
}
