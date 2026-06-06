mod app;
mod blit;
mod clipboard;
mod config;
mod dynamodb;
mod keys;
mod ui;

pub(crate) fn clipboard_copy(text: &str) -> Result<(), String> {
    clipboard::copy(text)
}

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "mnml-db-dynamodb",
    version,
    about = "Amazon DynamoDB table browser for mnml"
)]
struct Cli {
    #[arg(long)]
    check: bool,
    #[arg(long, value_name = "SOCKET")]
    blit: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::load()?;

    if cli.check {
        println!("config: {}", config::config_path().display());
        println!("region: {:?}", cfg.region);
        for (i, t) in cfg.tabs.iter().enumerate() {
            println!(
                "  tab {} ({}): table={} scan_limit={}",
                i + 1,
                t.name,
                t.table,
                t.scan_limit
            );
        }
        println!("(auth: defers to the `aws` CLI's own credential chain)");
        return Ok(());
    }

    let mut app = app::App::new(cfg)?;

    if let Some(socket) = cli.blit {
        blit::run(&mut app, std::path::Path::new(&socket)).await
    } else {
        ui::run(&mut app).await
    }
}
