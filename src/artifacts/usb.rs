use crate::utils;
use crate::storage::Storage;
use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{info, debug, error};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use wildmatch::WildMatch;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug)]
pub struct UsbDevice {
    pub name: String,
    pub serial: String,
    pub registry_path: String,
    pub last_write_time: DateTime<Utc>, 
}

pub fn get_usb_devices() -> Result<Vec<UsbDevice>> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let usbstor_path = "SYSTEM\\CurrentControlSet\\Enum\\USBSTOR";

    let usbstor = match hklm.open_subkey(usbstor_path) {
        Ok(key) => key,
        Err(_) => return Ok(vec![]),
    };

    let mut devices = Vec::new();

    for vendor_key_name in usbstor.enum_keys().flatten() {
        if vendor_key_name.is_empty() { continue; }
        
        let vendor_path = match usbstor.open_subkey(&vendor_key_name) {
            Ok(k) => k,
            Err(_) => continue,
        };

        for serial_key_name in vendor_path.enum_keys().flatten() {
            if serial_key_name.is_empty() { continue; }

            let instance_key = match vendor_path.open_subkey(&serial_key_name) {
                Ok(k) => k,
                Err(_) => continue,
            };
            
            let info = instance_key.query_info()?;
            let sys_time = info.get_last_write_time_system();
            let datetime = utils::systemtime_to_datetime(&sys_time);

            let name: String = instance_key.get_value("FriendlyName")
                .or_else(|_| instance_key.get_value("DeviceDesc"))
                .unwrap_or_else(|_| "Unknown Device".to_string());

            devices.push(UsbDevice {
                name,
                serial: serial_key_name.clone(),
                registry_path: format!("HKLM\\{}\\{}\\{}", usbstor_path, vendor_key_name, serial_key_name),
                last_write_time: datetime,
            });
        }
    }
    
    Ok(devices)
}

fn clean_mounted_devices(serial: &str) -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mounted_path = "SYSTEM\\MountedDevices";
    let mounted_key = match hklm.open_subkey(mounted_path) { Ok(k) => k, Err(_) => return Ok(()), };
    debug!("Scanning MountedDevices for serial: {}.", serial);
    for (value_name, value_data) in mounted_key.enum_values().flatten() {
        let data_str = String::from_utf8_lossy(&value_data.bytes);
        if data_str.replace('\0', "").contains(serial) {
            let hklm_path = format!("HKLM\\{}", mounted_path);
            if Storage::instance().dry_run { info!("Would delete value: {} from {}.", value_name, hklm_path); continue; }
            let cmd = format!("cmd /c reg delete \"{}\" /v \"{}\" /f", hklm_path, value_name);
            debug!("Found artifact in MountedDevices. Deleting value: {}.", value_name);
            let _ = utils::run_scheduled_command(&cmd, true, 0);
        }
    }
    Ok(())
}

fn clean_device_classes(serial: &str) -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let base_path = "SYSTEM\\CurrentControlSet\\Control\\DeviceClasses";
    let base_key = match hklm.open_subkey(base_path) { Ok(k) => k, Err(_) => return Ok(()), };
    for guid in base_key.enum_keys().map(|x| x.unwrap_or_default()) {
        let guid_path = format!("{}\\{}", base_path, guid);
        if let Ok(guid_key) = hklm.open_subkey(&guid_path) {
            for sub in guid_key.enum_keys().map(|x| x.unwrap_or_default()) {
                if sub.contains(serial) {
                    let full = format!("HKLM\\{}\\{}", guid_path, sub);
                    let _ = utils::delete_registry_key(&full, true);
                }
            }
        }
    }
    Ok(())
}

fn clean_setupapi_log(serial: &str) -> Result<()> {
    let sys_root = &Storage::instance().system_root;
    let log_path = sys_root.join("INF").join("setupapi.dev.log");
    if !log_path.exists() { return Ok(()); }
    if Storage::instance().dry_run { return Ok(()); }
    
    let file = match File::open(&log_path) { Ok(f) => f, Err(_) => return Ok(()), };
    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut block = Vec::new();
    let mut inside = false;
    let mut has_serial = false;
    let target = serial.to_lowercase();
    for l in reader.lines() {
        let line = l.unwrap_or_default();
        let trim = line.trim();
        if trim.starts_with(">>>") {
            if inside { if !has_serial { lines.append(&mut block); } block.clear(); }
            inside = true; has_serial = false;
        }
        if inside {
            block.push(line.clone());
            if line.to_lowercase().contains(&target) { has_serial = true; }
            if trim.starts_with("<<<") && line.contains("[Exit status:") {
                inside = false;
                if !has_serial { lines.append(&mut block); }
                block.clear();
            }
        } else { lines.push(line); }
    }
    if inside && !block.is_empty() && !has_serial { lines.append(&mut block); }
    
    let content = lines.join("\r\n");
    let final_c = if content.is_empty() { content } else { format!("{}\r\n", content) };
    let _ = fs::write(&log_path, final_c);
    Ok(())
}

pub fn clean_devices(field_type: &str, pattern: &str) -> Result<usize> {
    let devices = get_usb_devices()?;
    let mut count = 0;
    
    let matcher = WildMatch::new(pattern);
    info!("Deleting USB devices matching pattern: '{}'...", pattern);

    for dev in devices {
        let target_value = match field_type {
            "name" => &dev.name,
            "serial" => &dev.serial,
            _ => continue,
        };

        if matcher.matches(target_value) {
            let mut delete_path = dev.registry_path.clone();

            info!("Deleting USB device: {}.", dev.name);

            if let Err(e) = clean_mounted_devices(&dev.serial) {
                debug!("Failed to clean MountedDevices: {}.", e);
            }
            if let Err(e) = clean_device_classes(&dev.serial) {
                debug!("Failed to clean DeviceClasses: {}.", e);
            }
            if let Err(e) = clean_setupapi_log(&dev.serial) {
                debug!("Failed to clean setupapi.dev.log: {}.", e);
            }

            if let Some((parent_path, _)) = dev.registry_path.rsplit_once('\\') {
                let relative_parent = parent_path.trim_start_matches("HKLM\\");
                
                let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
                if let Ok(parent_key) = hklm.open_subkey(relative_parent) {
                    let sibling_count = parent_key.enum_keys().count();
                    
                    if sibling_count <= 1 {
                        debug!("Target is the only child. Upgrading target to Parent: {}.", parent_path);
                        delete_path = parent_path.to_string();
                    }
                }
            }

            if let Err(e) = utils::delete_registry_key(&delete_path, true) {
                error!("Failed to delete USB device {}: {}.", dev.name, e);
            } else {
                count += 1;
                info!("Successfully deleted USB device: {}.", dev.name);
            }
        }
    }
    
    Ok(count)
}
