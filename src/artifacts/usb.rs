use crate::{
    utils,
    storage::Storage,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{info, debug, error};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    collections::HashSet,
};
use wildmatch::WildMatch;
use winreg::{
    enums::*,
    RegKey,
};

const CM_DEVCAP_REMOVABLE: u32 = 0x00000004;

#[derive(Debug, Clone)]
pub struct UsbDevice {
    pub device_type: String,
    pub name: String,
    pub serial: String,
    pub registry_path: String,
    pub last_write_time: DateTime<Utc>, 
}

fn is_removable(key: &RegKey) -> bool {
    let caps: u32 = match key.get_value("Capabilities") {
        Ok(val) => val,
        Err(_) => return false,
    };
    (caps & CM_DEVCAP_REMOVABLE) != 0
}

fn scan_enum_key(subpath: &str, type_label: &str, check_removable: bool) -> Result<Vec<UsbDevice>> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let base_path = format!("SYSTEM\\CurrentControlSet\\Enum\\{}", subpath);

    let base_key = match hklm.open_subkey(&base_path) {
        Ok(key) => key,
        Err(_) => return Ok(vec![]),
    };

    let mut found_devices = Vec::new();

    for group_key_name in base_key.enum_keys().flatten() {
        if group_key_name.is_empty() { continue; }
        
        let group_key = match base_key.open_subkey(&group_key_name) {
            Ok(k) => k,
            Err(_) => continue,
        };

        for serial_key_name in group_key.enum_keys().flatten() {
            if serial_key_name.is_empty() { continue; }

            let instance_key = match group_key.open_subkey(&serial_key_name) {
                Ok(k) => k,
                Err(_) => continue,
            };

            if check_removable {
                if !is_removable(&instance_key) {
                    continue; 
                }
            }

            if type_label == "USB" {
                let service: String = instance_key.get_value::<String, _>("Service").unwrap_or_default().to_lowercase();
                let class: String = instance_key.get_value::<String, _>("Class").unwrap_or_default().to_lowercase();
                let desc: String = instance_key.get_value::<String, _>("DeviceDesc").unwrap_or_default().to_lowercase();
                let is_storage = service == "usbstor" || 
                                 service.contains("wudfrd") || 
                                 class == "usbdevice" || 
                                 class == "wpd" || 
                                 desc.contains("mass storage");
                
                if !is_storage { continue; }
            }
            
            let info = instance_key.query_info()?;
            let sys_time = info.get_last_write_time_system();
            let datetime = utils::time::systemtime_to_datetime(&sys_time);

            let name: String = instance_key.get_value::<String, _>("FriendlyName")
                .or_else(|_| instance_key.get_value::<String, _>("DeviceDesc"))
                .unwrap_or_else(|_| format!("Unknown Device ({})", group_key_name));

            let final_name = if name.starts_with('@') {
                let desc: String = instance_key.get_value::<String, _>("DeviceDesc").unwrap_or_default();
                if !desc.is_empty() {
                    let clean_desc = desc.split(';').last().unwrap_or(&desc);
                    clean_desc.to_string()
                } else {
                    name
                }
            } else {
                name
            };

            found_devices.push(UsbDevice {
                device_type: type_label.to_string(),
                name: final_name,
                serial: serial_key_name.clone(),
                registry_path: format!("HKLM\\{}\\{}\\{}", base_path, group_key_name, serial_key_name),
                last_write_time: datetime,
            });
        }
    }

    Ok(found_devices)
}

fn scan_wpd_devices() -> Result<Vec<UsbDevice>> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let wpd_path = "SOFTWARE\\Microsoft\\Windows Portable Devices\\Devices";

    let wpd_key = match hklm.open_subkey(wpd_path) {
        Ok(k) => k,
        Err(_) => return Ok(vec![]),
    };

    let mut found_devices = Vec::new();

    for device_id in wpd_key.enum_keys().flatten() {
        if device_id.is_empty() { continue; }

        let device_key = match wpd_key.open_subkey(&device_id) {
            Ok(k) => k,
            Err(_) => continue,
        };

        let info = device_key.query_info()?;
        let sys_time = info.get_last_write_time_system();
        let datetime = utils::time::systemtime_to_datetime(&sys_time);

        let name: String = device_key.get_value::<String, _>("FriendlyName")
            .unwrap_or_else(|_| "Unknown Portable Device".to_string());

        let clean_serial = if let Some(idx) = device_id.rfind('#') {
            let last_part = &device_id[idx+1..];

            if last_part.starts_with('{') && last_part.ends_with('}') {
                let temp_id = &device_id[..idx];
                if let Some(second_idx) = temp_id.rfind('#') {
                    temp_id[second_idx+1..].to_string()
                } else {
                    last_part.to_string()
                }
            } else {
                last_part.to_string()
            }
        } else {
            device_id.clone()
        };

        found_devices.push(UsbDevice {
            device_type: "WPD".to_string(),
            name,
            serial: clean_serial,
            registry_path: format!("HKLM\\{}\\{}", wpd_path, device_id),
            last_write_time: datetime,
        });
    }

    Ok(found_devices)
}

pub fn get_usb_devices() -> Result<Vec<UsbDevice>> {
    let mut all_devices = Vec::new();
    let mut known_serials = HashSet::new();

    if let Ok(mut devs) = scan_enum_key("USBSTOR", "USBSTOR", false) {
        for dev in &devs {
            let core_serial = dev.serial.split('&').next().unwrap_or(&dev.serial).to_string();
            known_serials.insert(core_serial);
        }
        all_devices.append(&mut devs);
    }

    if let Ok(mut devs) = scan_enum_key("SCSI", "SCSI", true) {
        for dev in &devs {
            let core_serial = dev.serial.split('&').next().unwrap_or(&dev.serial).to_string();
            known_serials.insert(core_serial);
        }
        all_devices.append(&mut devs);
    }

    if let Ok(mut devs) = scan_wpd_devices() {
        all_devices.append(&mut devs);
    }

    if let Ok(devs) = scan_enum_key("USB", "USB", false) { 
         for dev in devs {
             let serial_check = dev.serial.clone();
             if known_serials.contains(&serial_check) {
                 continue;
             }
             all_devices.push(dev);
         }
    }

    all_devices.sort_by(|a, b| b.last_write_time.cmp(&a.last_write_time));

    Ok(all_devices)
}

fn delete_with_parent_cleanup(full_path: &str) -> Result<()> {
    utils::system::delete_registry_key(full_path, true)?;

    let (parent_path, _) = match full_path.rsplit_once('\\') {
        Some(res) => res,
        None => return Ok(()),
    };

    if !parent_path.contains("CurrentControlSet\\Enum") {
        return Ok(());
    }

    let relative_parent = parent_path.trim_start_matches("HKLM\\");
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    
    if let Ok(parent_key) = hklm.open_subkey(relative_parent) {
        if parent_key.enum_keys().count() == 0 {
            debug!("Parent key is now empty. Cleaning up parent: {}", parent_path);
            if let Err(e) = utils::system::delete_registry_key(parent_path, true) {
                debug!("Failed to clean empty parent (might be locked): {}", e);
            } else {
                debug!("Successfully cleaned empty parent.");
            }
        }
    }

    Ok(())
}

fn clean_parent_usb_device(serial: &str) -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let usb_root_path = "SYSTEM\\CurrentControlSet\\Enum\\USB";
    
    let usb_root = match hklm.open_subkey(usb_root_path) {
        Ok(k) => k,
        Err(_) => return Ok(()),
    };

    let mut targets_to_delete = Vec::new();

    for vid_pid in usb_root.enum_keys().flatten() {
        if let Ok(vid_pid_key) = usb_root.open_subkey(&vid_pid) {
            for instance_id in vid_pid_key.enum_keys().flatten() {
                if instance_id.eq_ignore_ascii_case(serial) {
                    let full_path = format!("HKLM\\{}\\{}\\{}", usb_root_path, vid_pid, instance_id);
                    targets_to_delete.push(full_path);
                }
            }
        }
    }

    for target_path in targets_to_delete {
        debug!("Found parent USB device trace: {}. Deleting...", target_path);
        
        if let Err(e) = delete_with_parent_cleanup(&target_path) {
            debug!("Failed to delete parent USB key: {}", e);
        }
    }

    Ok(())
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
            let _ = utils::system::run_scheduled_command(&cmd, true, 0);
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
                    let _ = utils::system::delete_registry_key(&full, true);
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
    info!("Starting artifact wipe. Target pattern: '{}' ({})", pattern, field_type);

    for dev in devices {
        let is_match = match field_type {
            "name" => matcher.matches(&dev.name),
            "serial" => matcher.matches(&dev.serial),
            _ => continue,
        };

        if is_match {
            info!("Target identified: {} [{}] (Serial: {})", dev.name, dev.device_type, dev.serial);

            if let Err(e) = clean_mounted_devices(&dev.serial) {
                debug!("Failed to clean MountedDevices: {}.", e);
            }
            if let Err(e) = clean_device_classes(&dev.serial) {
                debug!("Failed to clean DeviceClasses: {}.", e);
            }
            if let Err(e) = clean_setupapi_log(&dev.serial) {
                debug!("Failed to clean setupapi.dev.log: {}.", e);
            }

            if dev.device_type == "USBSTOR" || dev.device_type == "SCSI" || dev.device_type == "USB" {
                if let Err(e) = delete_with_parent_cleanup(&dev.registry_path) {
                     error!("Failed to delete registry key {}: {}", dev.registry_path, e);
                     continue;
                }
                
                if dev.device_type != "USB" {
                    let core_serial = dev.serial.split('&').next().unwrap_or(&dev.serial);
                    if let Err(e) = clean_parent_usb_device(core_serial) {
                        debug!("Parent cleanup warning: {}", e);
                    }
                }

                count += 1;
                info!("Successfully wiped artifact: {}", dev.name);
            }
            else if dev.device_type == "WPD" {
                if let Err(e) = utils::system::delete_registry_key(&dev.registry_path, true) {
                    error!("Failed to delete WPD registry key: {}", e);
                    continue; 
                }

                if let Err(e) = clean_parent_usb_device(&dev.serial) {
                     debug!("WPD Hardware trace cleanup warning: {}", e);
                }

                count += 1;
                info!("Successfully wiped WPD artifact: {}", dev.name);
            }
        }
    }

    if count == 0 {
        info!("No devices found matching the pattern.");
    } else {
        info!("Wipe operation completed. {} devices cleaned.", count);
        info!("IMPORTANT: Please REBOOT your system to allow Windows to re-detect the devices correctly.");
    }
    
    Ok(count)
}
