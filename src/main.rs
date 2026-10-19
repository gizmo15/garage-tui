mod app;
mod client;
mod config;
mod model;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use tokio::sync::mpsc;

use app::{Action, App};
use client::GarageClient;
use model::{Snapshot, TokenInfo};

#[derive(Parser)]
#[command(
    version,
    about = "Terminal UI for Garage: buckets, keys and cluster usage"
)]
struct Args {
    /// Path to the YAML config file (default: ~/.config/garage-tui/config.yaml)
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,
}

enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Tick,
    Data(Result<Snapshot, String>),
    Token(Result<TokenInfo, String>),
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let (config, warning) = config::load(args.config.as_deref())?;
    let client = GarageClient::new(&config.admin_url, &config.admin_token)?;

    let mut terminal = ratatui::init();
    let result = run(
        &mut terminal,
        client,
        &config,
        args.config.as_deref(),
        warning,
    )
    .await;
    ratatui::restore();
    result
}

async fn run(
    terminal: &mut ratatui::DefaultTerminal,
    client: GarageClient,
    config: &config::Config,
    config_path: Option<&std::path::Path>,
    warning: Option<String>,
) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut app = App::new(warning).with_settings(config.settings(config_path));

    // Terminal input runs on its own thread, crossterm's poll is blocking.
    let key_tx = tx.clone();
    std::thread::spawn(move || {
        loop {
            match event::poll(Duration::from_millis(250)) {
                Ok(true) => {
                    if let Ok(Event::Key(key)) = event::read()
                        && key.kind == KeyEventKind::Press
                        && key_tx.send(AppEvent::Key(key)).is_err()
                    {
                        break;
                    }
                }
                Ok(false) => {}
                Err(_) => break,
            }
            if key_tx.is_closed() {
                break;
            }
        }
    });

    let tick_tx = tx.clone();
    let refresh = Duration::from_secs(config.refresh_secs());
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(refresh);
        loop {
            interval.tick().await;
            if tick_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    let token_client = client.clone();
    let token_tx = tx.clone();
    tokio::spawn(async move {
        let result = token_client.token_info().await.map_err(|e| e.to_string());
        let _ = token_tx.send(AppEvent::Token(result));
    });

    let fetch = |app: &App| {
        let client = client.clone();
        let tx = tx.clone();
        let show_secret = app.show_secret && app.can_reveal_secrets();
        tokio::spawn(async move {
            let result = client
                .snapshot(show_secret)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::Data(result));
        });
    };

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app, &config.admin_url))?;
        let Some(event) = rx.recv().await else { break };
        match event {
            AppEvent::Tick => fetch(&app),
            AppEvent::Data(result) => app.set_result(result),
            AppEvent::Token(result) => app.set_token(result),
            AppEvent::Key(key) => match app.on_key(key) {
                Action::Quit => break,
                Action::Refresh => fetch(&app),
                Action::None => {}
            },
        }
    }
    Ok(())
}
