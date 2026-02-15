use crate::{
    artifacts::file::own,
    storage::Storage,
};
use clap::Subcommand;
use anyhow::Result;
use comfy_table::{Table, presets::UTF8_FULL, modifiers::UTF8_ROUND_CORNERS, ContentArrangement, Cell, Color, Attribute};
use log::{info, warn};

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Retrieve file ownership information
    /// Usage: rogue file own get "*.txt"
    Get {
        /// Target file path or pattern
        pattern: String,
    },

    /// Change file owner (Spoofing)
    /// Usage: rogue file own set "*.txt" "Administrators"
    Set {
        /// Target file path or pattern
        pattern: String,
        /// New owner name (e.g., "Administrators", "SYSTEM", "Everyone")
        new_owner: String,
    },
}

pub fn run(command: Commands) -> Result<()> {
    match command {
        Commands::Get { pattern } => print_owners(pattern),
        
        Commands::Set { pattern, new_owner } => {
            set_owner_logic(pattern, new_owner)
        }
    }
}

fn print_owners(pattern: String) -> Result<()> {
    info!("Getting ownership info for pattern: {}.", pattern);

    let records = own::get_owner(&pattern)?;

    if Storage::instance().as_system {
        return Ok(());
    }

    if records.is_empty() {
        warn!("No files found matching pattern: {}", pattern);
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic);

        table.set_header(vec![
            Cell::new("Filename").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Owner").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Full Path").add_attribute(Attribute::Bold).fg(Color::Green),
        ]);

        for rec in records {
            table.add_row(vec![
                Cell::new(&rec.filename).add_attribute(Attribute::Bold),
                Cell::new(&rec.owner).fg(Color::Cyan),
                Cell::new(&rec.path).fg(Color::DarkGrey),
            ]);
        }

        println!("{table}");
        info!("Total targets found: {}.", table.row_count());
    }

    Ok(())
}

fn set_owner_logic(pattern: String, new_owner: String) -> Result<()> {
    info!("Spoofing owner for pattern: {} -> {}.", pattern, new_owner);
    own::set_owner(&pattern, &new_owner)?;
    print_owners(pattern)?;
    Ok(())
}
