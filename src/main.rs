#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis;
mod app;
mod audio;
mod diagnostics;
mod drop_target;
mod library;
mod mixer;
mod settings;
mod warp;
mod win_drop;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "ArcLive", about = "ArcLive DJ — native Rust DJ workstation")]
struct Cli {
    /// Load this audio file onto Deck A after launch.
    #[arg(long)]
    load_a: Option<PathBuf>,
    /// Load this audio file onto Deck B after launch.
    #[arg(long)]
    load_b: Option<PathBuf>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    // Select winit before any Slint window is created (required for Explorer DnD hook).
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;

    let app = app::ArcLiveApp::new()?;
    if let Some(path) = cli.load_a {
        app.queue_load_a(path);
    }
    if let Some(path) = cli.load_b {
        app.queue_load_b(path);
    }
    app.run()?;
    Ok(())
}