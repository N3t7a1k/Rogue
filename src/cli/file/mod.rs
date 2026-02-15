use crate::storage::Storage;
use anyhow::Result;
use clap::Subcommand;
use log::error;

pub mod own;
pub mod time;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Modify file timestamps
    Time {
        #[command(subcommand)]
        action: time::Commands,
    },
    /// Modify file ownership
    Own {
        #[command(subcommand)]
        action: own::Commands,
    },
}

pub fn run(command: Commands) -> Result<()> {
    let storage = Storage::instance();
    if !storage.is_admin && storage.as_system {
        error!("Administrator privilege required for system option.");
        return Ok(());
    }

    match command {
        Commands::Time { action } => time::run(action),
        Commands::Own { action } => own::run(action),
    }
}