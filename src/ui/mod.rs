mod buckets;
mod cluster;
mod keys;

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
};

use crate::app::{App, Tab};

pub fn draw(frame: &mut Frame, app: &mut App, url: &str) {
    let [tabs_area, body, status, help] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let titles: Vec<&str> = Tab::ALL.iter().map(|t| t.title()).collect();
    frame.render_widget(
        Tabs::new(titles)
            .select(app.tab.index())
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)),
        tabs_area,
    );

    match app.tab {
        Tab::Buckets => buckets::draw(frame, body, app),
        Tab::Keys => keys::draw(frame, body, app),
        Tab::Cluster => cluster::draw(frame, body, app),
    }

    frame.render_widget(Paragraph::new(status_line(app, url)), status);
    let help_text = if app.editing_filter {
        "type to filter - Enter/Esc: done".to_string()
    } else {
        format!(
            "Tab/1-3: tabs  j/k: move  /: filter  s: sort ({})  S: secrets {}  r: refresh  q: quit",
            app.sort.label(),
            if app.show_secret { "on" } else { "off" }
        )
    };
    frame.render_widget(
        Paragraph::new(help_text).style(Style::new().fg(Color::DarkGray)),
        help,
    );
}

fn status_line(app: &App, url: &str) -> Line<'static> {
    let mut spans = vec![Span::raw(url.to_string())];
    if let Some(t) = &app.last_refresh {
        spans.push(Span::raw(format!("  | refreshed {t}")));
    }
    if app.loading {
        spans.push(Span::styled(
            "  | loading...",
            Style::new().fg(Color::Yellow),
        ));
    }
    if !app.filter.is_empty() || app.editing_filter {
        spans.push(Span::raw(format!("  | filter: {}", app.filter)));
    }
    if let Some(token) = &app.token {
        let color = if token.is_excessive() || token.expired {
            Color::Yellow
        } else {
            Color::Green
        };
        spans.push(Span::styled(
            format!("  | token: {}", token.label()),
            Style::new().fg(color),
        ));
    }
    if let Some(w) = &app.warning {
        spans.push(Span::styled(
            format!("  | warning: {w}"),
            Style::new().fg(Color::Yellow),
        ));
    }
    if let Some(e) = &app.error {
        spans.push(Span::styled(
            format!("  | error: {e}"),
            Style::new().fg(Color::Red),
        ));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::model::sample_snapshot;

    fn render(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
        terminal
            .draw(|frame| draw(frame, app, "http://garage:3903"))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer
            .content()
            .chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn loaded(tab: Tab) -> App {
        let mut app = App::new(None);
        app.set_result(Ok(sample_snapshot()));
        app.tab = tab;
        app
    }

    #[test]
    fn buckets_tab_shows_sizes_quota_and_detail() {
        let out = render(&mut loaded(Tab::Buckets));
        assert!(out.contains("Buckets (3)"));
        assert!(out.contains("photos"));
        assert!(out.contains("1 GiB"));
        assert!(out.contains("50%"));
        assert!(out.contains("http://garage:3903"));
        assert!(out.contains("refreshed"));
        // first row (archives) is selected: no keys, no quota
        assert!(out.contains("1000"));
    }

    #[test]
    fn buckets_detail_lists_keys_of_selected_bucket() {
        let mut app = loaded(Tab::Buckets);
        app.buckets_state.select(Some(2));
        let out = render(&mut app);
        assert!(out.contains("RWO backup"));
        assert!(out.contains("pending uploads: 1"));
        assert!(out.contains("size 2 GiB"));
    }

    #[test]
    fn keys_tab_masks_secret_until_revealed() {
        let mut app = loaded(Tab::Keys);
        app.keys_state.select(Some(0));
        let out = render(&mut app);
        assert!(out.contains("Keys (2)"));
        assert!(out.contains("GK1"));
        assert!(out.contains("********"));
        assert!(!out.contains("s3cr3t"));
        assert!(out.contains("RW-  b3b3b3b3b3b3") || out.contains("R--  photos"));

        app.show_secret = true;
        assert!(render(&mut app).contains("s3cr3t"));

        app.keys_state.select(Some(1));
        let out = render(&mut app);
        assert!(out.contains("expired"));
        assert!(out.contains("2026-02-01"));
    }

    #[test]
    fn cluster_tab_shows_nodes_and_gauges() {
        let out = render(&mut loaded(Tab::Cluster));
        assert!(out.contains("layout v3"));
        assert!(out.contains("3 buckets"));
        assert!(out.contains("node1"));
        assert!(out.contains("[dc1]"));
        assert!(out.contains("draining"));
        assert!(out.contains("DOWN"));
        assert!(out.contains("60%"));
        assert!(out.contains("95%"));
        assert!(out.contains("no disk info"));
    }

    #[test]
    fn empty_states_do_not_panic() {
        let mut app = App::new(None);
        for tab in Tab::ALL {
            app.tab = tab;
            let out = render(&mut app);
            assert!(out.contains("loading..."));
        }
        let mut app = loaded(Tab::Buckets);
        app.filter = "zzz".into();
        assert!(render(&mut app).contains("no bucket"));
        app.tab = Tab::Keys;
        assert!(render(&mut app).contains("no key"));
    }

    #[test]
    fn status_bar_shows_filter_warning_error_and_help() {
        let mut app = loaded(Tab::Buckets);
        app.warning = Some("file too open".into());
        app.error = Some("HTTP 403".into());
        app.filter = "pho".into();
        let out = render(&mut app);
        assert!(out.contains("filter: pho"));
        assert!(out.contains("warning: file too open"));
        assert!(out.contains("error: HTTP 403"));
        assert!(out.contains("S: secrets off"));

        app.editing_filter = true;
        assert!(render(&mut app).contains("type to filter"));
    }

    #[test]
    fn status_bar_shows_token_kind() {
        let mut app = loaded(Tab::Buckets);
        assert!(!render(&mut app).contains("token:"));
        app.set_token(Ok(crate::model::TokenInfo {
            scope: vec!["ListBuckets".into()],
            ..Default::default()
        }));
        assert!(render(&mut app).contains("token: read-only"));
    }

    #[test]
    fn buckets_over_quota_threshold_are_highlighted() {
        let mut app = loaded(Tab::Buckets);
        let snap = app.snapshot.as_mut().unwrap();
        snap.buckets[1].bytes = 1900;
        snap.buckets[1].quotas.max_size = Some(2000);
        app.buckets_state.select(Some(2));

        let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, "http://garage:3903"))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let fg_of = |text: &str| {
            let width = buffer.area.width as usize;
            let row = buffer
                .content()
                .chunks(width)
                .position(|r| {
                    r.iter()
                        .map(|c| c.symbol())
                        .collect::<String>()
                        .contains(text)
                })
                .unwrap_or_else(|| panic!("{text} not rendered"));
            let col = buffer.content()[row * width..(row + 1) * width]
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
                .find(text)
                .unwrap();
            buffer.content()[row * width + col].fg
        };
        assert_eq!(fg_of("95%"), Color::Red);
        assert_ne!(fg_of("50%"), Color::Red);
    }
}
