use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Gauge, Paragraph},
};

use crate::app::App;
use crate::model::{Node, human_bytes};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let Some(snapshot) = &app.snapshot else {
        frame.render_widget(
            Paragraph::new("no data yet").block(Block::bordered().title("Cluster")),
            area,
        );
        return;
    };
    let cluster = &snapshot.cluster;
    let stored: u64 = snapshot.buckets.iter().map(|b| b.bytes).sum();
    let objects: u64 = snapshot.buckets.iter().map(|b| b.objects).sum();

    let outer = Block::bordered().title(format!(
        "Cluster - layout v{} - {} buckets, {} objects, {} stored",
        cluster.layout_version,
        snapshot.buckets.len(),
        objects,
        human_bytes(stored)
    ));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Two lines per node: header text + gauge.
    let rows = Layout::vertical(cluster.nodes.iter().map(|_| Constraint::Length(2))).split(inner);
    for (node, row) in cluster.nodes.iter().zip(rows.iter()) {
        draw_node(frame, *row, node);
    }
}

fn draw_node(frame: &mut Frame, area: Rect, node: &Node) {
    let [title, gauge] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    let name = node
        .hostname
        .clone()
        .unwrap_or_else(|| node.id.chars().take(12).collect());
    let zone = node.role.as_ref().map_or("no role", |r| r.zone.as_str());
    let state = if node.is_up { "up" } else { "DOWN" };
    let color = if node.is_up { Color::Green } else { Color::Red };
    frame.render_widget(
        Paragraph::new(format!(
            "{name}  [{zone}]  {state}{}",
            if node.draining { "  draining" } else { "" }
        ))
        .style(Style::new().fg(color)),
        title,
    );

    if let Some(p) = node.data_partition.as_ref().filter(|p| p.total > 0) {
        let used = p.total.saturating_sub(p.available);
        let ratio = (used as f64 / p.total as f64).clamp(0.0, 1.0);
        let gauge_color = if ratio > 0.9 {
            Color::Red
        } else if ratio > 0.75 {
            Color::Yellow
        } else {
            Color::Cyan
        };
        frame.render_widget(
            Gauge::default()
                .ratio(ratio)
                .gauge_style(Style::new().fg(gauge_color))
                .label(format!(
                    "data {} / {} ({:.0}%)",
                    human_bytes(used),
                    human_bytes(p.total),
                    ratio * 100.0
                )),
            gauge,
        );
    } else {
        frame.render_widget(Paragraph::new("no disk info"), gauge);
    }
}
