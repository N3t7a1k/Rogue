use crate::{
    artifacts::usb,
    storage::Storage,
};
use anyhow::Result;
use clap::Subcommand;
use chrono::Local;
use comfy_table::{
    Attribute, Cell, Color, ContentArrangement, Table,
    modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use log::{info, error};

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List all connected USB devices history
    List,
    /// Delete USB devices history
    Delete {
        #[command(subcommand)]
        command: DeleteCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum DeleteCommands {
    /// Delete by Device Name pattern (Case-sensitive, supports *)
    /// e.g., rogue usb delete name "SanDisk*"
    Name {
        pattern: String,
    },
    /// Delete by Serial Number pattern (Case-sensitive, supports *)
    /// e.g., rogue usb delete serial "1234*"
    Serial {
        pattern: String,
    },
}

pub fn run(action: Commands) -> Result<()> {
    match action {
        Commands::List => print_usb_devices()?,
        
        Commands::Delete { command } => {
            if !Storage::instance().dry_run && !Storage::instance().is_admin {
                error!("Deleting USB history requires Administrator privileges.");
                return Ok(());
            }
            match command {
                DeleteCommands::Name { pattern } => {
                    info!("Deleting USB history by name pattern '{}'.", pattern);
                    let count = usb::clean_devices("name", &pattern)?;
                    info!("Deleted {} devices by name pattern '{}'.", count, pattern);
                }
                DeleteCommands::Serial { pattern } => {
                    info!("Deleting USB history by serial pattern '{}'.", pattern);
                    let count = usb::clean_devices("serial", &pattern)?;
                    info!("Deleted {} devices by serial pattern '{}'.", count, pattern);
                }
            }
            print_usb_devices()?;
        },
    }
    Ok(())
}

fn print_usb_devices() -> Result<()> {
    info!("Getting USB history.");

    let usb_devices = usb::get_usb_devices()?;

    if usb_devices.is_empty() {
        info!("No USB history found.");
    } else {
        let mut table = Table::new();
        
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic);

        table.set_header(vec![
            Cell::new("Type").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Serial Number").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Device Name").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Last Activity").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Registry Path").add_attribute(Attribute::Bold).fg(Color::Green),
        ]);

        for device in usb_devices {
            table.add_row(vec![
                Cell::new(&device.device_type),
                Cell::new(&device.serial),
                Cell::new(&device.name),
                Cell::new(device.last_write_time.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S%.3f")),
                Cell::new(&device.registry_path).fg(Color::DarkGrey),
            ]);
        }

        println!("{table}");
        info!("Total devices found: {}.", table.row_count());
    }

    Ok(())
}
