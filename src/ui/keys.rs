use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Paragraph, Row, Table},
};

use crate::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App) {
    let [list, detail] =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(area);

    let keys = app.visible_keys();
    let rows: Vec<Row> = keys
        .iter()
        .map(|k| {
            Row::new(vec![
                k.name.clone(),
                k.access_key_id.clone(),
                k.buckets.len().to_string(),
                if k.expired { "expired" } else { "ok" }.to_string(),
            ])
        })
        .collect();

    let selected_detail = app
        .keys_state
        .selected()
        .and_then(|i| keys.get(i))
        .map(|k| {
            let secret = match (&k.secret_access_key, app.show_secret) {
                (Some(s), true) if app.can_reveal_secrets() => s.expose().to_string(),
                _ if !app.can_reveal_secrets() => "********  (admin token required)".to_string(),
                _ => "********  (press S to reveal)".to_string(),
            };
            let mut lines = vec![
                format!("name:     {}", k.name),
                format!("key id:   {}", k.access_key_id),
                format!("secret:   {secret}"),
                format!("created:  {}", k.created.as_deref().unwrap_or("-")),
                format!("expires:  {}", k.expiration.as_deref().unwrap_or("never")),
                format!("can create buckets: {}", k.permissions.create_bucket),
                String::new(),
                "buckets:".to_string(),
            ];
            for b in &k.buckets {
                lines.push(format!("  {}  {}", b.permissions.label(), b.name()));
            }
            lines.join("\n")
        })
        .unwrap_or_else(|| "no key".to_string());

    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Length(28),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec!["Name", "Access key id", "Buckets", "State"]).style(Style::new().bold()))
    .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
    .block(Block::bordered().title(format!("Keys ({})", keys.len())));

    frame.render_stateful_widget(table, list, &mut app.keys_state);
    frame.render_widget(
        Paragraph::new(selected_detail).block(Block::bordered().title("Detail")),
        detail,
    );
}
