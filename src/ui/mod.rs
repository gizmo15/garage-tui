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
