use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Block, Paragraph},
};

use crate::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let s = &app.settings;
    let row = |key: &str, value: String| Line::from(format!("{key:<20} {value}"));
    let lines = vec![
        row("admin_url", s.admin_url.clone()),
        row(
            "config file",
            s.config_path.clone().unwrap_or_else(|| "-".to_string()),
        ),
        row("refresh_secs", s.refresh_secs.to_string()),
        row("quota_warn_percent", format!("{}%", s.quota_warn_percent)),
        row("disk_warn_percent", format!("{}%", s.disk_warn_percent)),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Configuration")),
        area,
    );
}
