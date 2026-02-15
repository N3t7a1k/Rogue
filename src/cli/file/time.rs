use crate::{
    artifacts::file::time,
    storage::Storage,
};
use clap::Subcommand;
use chrono::Local;
use anyhow::Result;
use comfy_table::{Table, presets::UTF8_FULL, modifiers::UTF8_ROUND_CORNERS, ContentArrangement, Cell, Color, Attribute};
use log::{info, warn};

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Retrieve timestamps (Usage: rogue time get "*.txt")
    Get {
        /// Target file path or pattern
        pattern: String,
    },

    /// Modify timestamps manually
    Set {
        #[command(subcommand)]
        command: SetCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum SetCommands {
    /// Modify ALL timestamps (Created, Accessed, Modified)
    /// Usage: rogue time set all "*.txt" "2025-01-01 12:00:00"
    All {
        pattern: String,
        time: String,
    },
    
    /// Modify only CREATED timestamp
    /// Usage: rogue time set created "*.txt" "2025-01-01 12:00:00"
    Created {
        pattern: String,
        time: String,
    },

    /// Modify only MODIFIED timestamp
    /// Usage: rogue time set modified "*.txt" "2025-01-01 12:00:00"
    Modified {
        pattern: String,
        time: String,
    },

    /// Modify only ACCESSED timestamp
    /// Usage: rogue time set accessed "*.txt" "2025-01-01 12:00:00"
    Accessed {
        pattern: String,
        time: String,
    },
}

pub fn run(command: Commands) -> Result<()> {
    match command {
        Commands::Get { pattern } => print_timestamps(pattern),
        
        Commands::Set { command } => {
            let (pattern, time, mace_type) = match command {
                SetCommands::All { pattern, time } => (pattern, time, time::MaceType::All),
                SetCommands::Created { pattern, time } => (pattern, time, time::MaceType::Created),
                SetCommands::Modified { pattern, time } => (pattern, time, time::MaceType::Modified),
                SetCommands::Accessed { pattern, time } => (pattern, time, time::MaceType::Accessed),
            };

            set_timestamps_logic(pattern, mace_type, time)
        }
    }
}

fn print_timestamps(pattern: String) -> Result<()> {
    info!("Getting timestamps for pattern: {}.", pattern);

    let records = time::get_timestamps(&pattern)?;

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
            Cell::new("Created").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Accessed").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Modified").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Full Path").add_attribute(Attribute::Bold).fg(Color::Green),
        ]);

        for rec in records {
            let fmt = "%Y-%m-%d %H:%M:%S%.3f";

            table.add_row(vec![
                Cell::new(&rec.filename).add_attribute(Attribute::Bold),
                Cell::new(rec.created.with_timezone(&Local).format(fmt)),
                Cell::new(rec.accessed.with_timezone(&Local).format(fmt)),
                Cell::new(rec.modified.with_timezone(&Local).format(fmt)),
                Cell::new(&rec.path).fg(Color::DarkGrey),
            ]);
        }

        println!("{table}");
        info!("Total targets found: {}.", table.row_count());
    }

    Ok(())
}

fn set_timestamps_logic(pattern: String, mace_type: time::MaceType, time: String) -> Result<()> {
    info!("Updating timestamps for pattern: {}, Mode: {:?}, Time: {}.", pattern, mace_type, time);
    time::set_timestamps(&pattern, mace_type, &time)?;
    print_timestamps(pattern)?;
    Ok(())
}
