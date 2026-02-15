use anyhow::Result;
use clap::{Parser, Subcommand};
use env_logger::Env;
use log::info;

pub mod artifacts;
pub mod storage;
pub mod types;
pub mod utils;
mod cli;

use storage::Storage;

#[derive(Parser, Debug)]
#[command(name = "rogue")]
#[command(about = "Rogue - System Artifact Wiper", long_about = None)]
struct Cli {
    /// Dry run mode (changes will NOT be committed).
    #[arg(short, long, global = true)]
    dry_run: bool,

    /// Run as system (Requires Administrator privilege)
    #[arg(short, long)]
    system: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage USB artifacts
    Usb {
        #[command(subcommand)]
        action: cli::usb::Commands,
    },
    /// Manage file artifacts (timestamps, ownership)
    File {
        #[command(subcommand)]
        action: cli::file::Commands,
    },
}

fn init() -> Cli {
   let cli = Cli::parse(); 
   Storage::init(cli.dry_run, cli.system);
   env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
   cli
}

fn main() -> Result<()> {
    let cli = init();

    println!("\n========================================");
    println!("   R O G U E - System Artifact Wiper    ");
    println!("========================================");

    if cli.dry_run {
        info!("DRY RUN MODE ENABLED: No changes will be committed.");
    }

    match cli.command {
        Commands::Usb { action } => cli::usb::run(action)?,
        Commands::File { action } => cli::file::run(action)?,
    }

    Ok(())
}