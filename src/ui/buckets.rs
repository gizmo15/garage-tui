use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Cell, Paragraph, Row, Table},
};

use crate::app::App;
use crate::model::human_bytes;

/// Suffix flagging a quota that is almost full.
fn warn(hot: bool, percent: Option<f64>) -> String {
    match percent {
        Some(p) if hot => format!(" (!{p:.0}% used)"),
        _ => String::new(),
    }
}

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App) {
    let [list, detail] =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(area);

    let threshold = app.settings.quota_warn_percent;
    let buckets = app.visible_buckets();
    let rows: Vec<Row> = buckets
        .iter()
        .map(|b| {
            let quota = b
                .quota_percent()
                .map_or("-".to_string(), |p| format!("{p:.0}%"));
            let alert = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
            let cell = |text: String, hot: bool| {
                if hot {
                    Cell::new(text).style(alert)
                } else {
                    Cell::new(text)
                }
            };
            let objects = match b.quotas.max_objects {
                Some(max) => format!("{} / {max}", b.objects),
                None => b.objects.to_string(),
            };
            Row::new(vec![
                cell(b.name(), b.quota_nearly_full(threshold)),
                Cell::new(human_bytes(b.bytes)),
                cell(objects, b.objects_nearly_full(threshold)),
                cell(quota, b.size_nearly_full(threshold)),
            ])
        })
        .collect();

    let selected_detail = app
        .buckets_state
        .selected()
        .and_then(|i| buckets.get(i))
        .map(|b| {
            let mut lines = vec![
                format!("id:       {}", b.id),
                format!("aliases:  {}", b.global_aliases.join(", ")),
                format!("created:  {}", b.created.as_deref().unwrap_or("-")),
                format!("size:     {} ({} bytes)", human_bytes(b.bytes), b.bytes),
                format!(
                    "objects:  {}{}",
                    b.objects,
                    b.objects_percent().map_or(String::new(), |p| format!(
                        " / {} ({p:.0}%)",
                        b.quotas.max_objects.unwrap_or_default()
                    ))
                ),
                format!("pending uploads: {}", b.unfinished_uploads),
                format!(
                    "quota:    size {}{} / objects {}{}",
                    b.quotas.max_size.map_or("none".into(), human_bytes),
                    warn(b.size_nearly_full(threshold), b.quota_percent()),
                    b.quotas
                        .max_objects
                        .map_or("none".into(), |n| n.to_string()),
                    warn(b.objects_nearly_full(threshold), b.objects_percent()),
                ),
                String::new(),
                "keys:".to_string(),
            ];
            for k in &b.keys {
                lines.push(format!(
                    "  {} {}  {}",
                    k.permissions.label(),
                    k.name,
                    k.access_key_id
                ));
            }
            lines.join("\n")
        })
        .unwrap_or_else(|| "no bucket".to_string());

    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(7),
        ],
    )
    .header(Row::new(vec!["Bucket", "Size", "Objects", "Quota"]).style(Style::new().bold()))
    .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
    .block(Block::bordered().title(format!("Buckets ({})", buckets.len())));

    frame.render_stateful_widget(table, list, &mut app.buckets_state);
    frame.render_widget(
        Paragraph::new(selected_detail).block(Block::bordered().title("Detail")),
        detail,
    );
}
