use crate::storage::Storage;
use anyhow::{anyhow, Result};
use log::{debug, info}; 
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
}; 
use winreg::{
    enums::*,
    RegKey,
};

pub fn delete_registry_key(path_input: &str, as_system: bool) -> Result<()> {
    if Storage::instance().dry_run {
        info!("Would delete: {}.", path_input);
        return Ok(());
    }

    let cmd_to_run = format!("cmd /c reg delete \"{}\" /f", path_input);
    
    if let Err(e) = run_scheduled_command(&cmd_to_run, as_system, 0) {
        debug!("Failed to execute deletion task: {}.", e);
        return Err(e);
    }

    let (root_str, sub_path) = match path_input.split_once('\\') {
        Some((r, s)) => (r, s),
        None => {
            return Ok(()); 
        }
    };

    let root_key = match root_str.to_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => RegKey::predef(HKEY_LOCAL_MACHINE),
        "HKCU" | "HKEY_CURRENT_USER" => RegKey::predef(HKEY_CURRENT_USER),
        "HKU"  | "HKEY_USERS" => RegKey::predef(HKEY_USERS),
        "HKCR" | "HKEY_CLASSES_ROOT" => RegKey::predef(HKEY_CLASSES_ROOT),
        "HKCC" | "HKEY_CURRENT_CONFIG" => RegKey::predef(HKEY_CURRENT_CONFIG),
        _ => {
            debug!("Unknown registry root: {}", root_str);
            return Ok(());
        }
    };

    let mut retries = 20;
    while retries > 0 {
        if let Some((parent_path, child_name)) = sub_path.rsplit_once('\\') {
            if let Ok(parent) = root_key.open_subkey(parent_path) {
                if parent.open_subkey(child_name).is_err() {
                    debug!("Deleted: {}.", path_input);
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        } else {
            if root_key.open_subkey(sub_path).is_err() {
                 return Ok(());
            }
        }
        
        std::thread::sleep(std::time::Duration::from_millis(100));
        retries -= 1;
    }

    Err(anyhow!("Failed to verify deletion of: {}", path_input))
}


pub fn run_scheduled_command(script_content: &str, as_system: bool, delay_secs: u64) -> Result<()> {
    let is_admin = Storage::instance().is_admin;

    if as_system && !is_admin {
        return Err(anyhow!("Administrator privilege required for SYSTEM execution."));
    }

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let task_name = format!("RogueTask_{}", timestamp);
    let bat_filename = format!("rogue_{}.bat", timestamp);
    let vbs_filename = format!("rogue_{}.vbs", timestamp);

    let temp_dir = if as_system {
        PathBuf::from("C:\\Windows\\Temp")
    } else {
        std::env::temp_dir()
    };

    let bat_path = temp_dir.join(&bat_filename);
    let vbs_path = temp_dir.join(&vbs_filename);

    let mut batch_script = String::new();
    batch_script.push_str("@echo off\n");
    if delay_secs > 0 {
        batch_script.push_str(&format!("ping 127.0.0.1 -n {} > nul\n", delay_secs + 1));
    }
    batch_script.push_str(&format!("{} \n", script_content));
    batch_script.push_str(&format!("schtasks /Delete /TN \"{}\" /F > nul 2>&1\n", task_name));
    batch_script.push_str(&format!("del \"{}\" > nul 2>&1\n", vbs_path.display()));
    batch_script.push_str("(goto) 2>nul & del \"%~f0\"\n");

    fs::write(&bat_path, &batch_script)?;

    let wait_on_return = if delay_secs == 0 { "True" } else { "False" };
    let vbs_content = format!(
        "CreateObject(\"Wscript.Shell\").Run \"cmd /c \"\"{}\"\"\", 0, {}",
        bat_path.display(), wait_on_return
    );
    fs::write(&vbs_path, &vbs_content)?;

    let wscript_exe = "C:\\Windows\\System32\\wscript.exe";
    let tr_arg = format!("{} \"{}\"", wscript_exe, vbs_path.display());

    let mut cmd = Command::new("schtasks");
    cmd.args(&["/Create", "/TN", &task_name, "/TR", &tr_arg, "/SC", "ONCE", "/ST", "23:59", "/F"]);

    if is_admin {
        cmd.arg("/RL").arg("HIGHEST");
        if as_system {
            cmd.arg("/RU").arg("SYSTEM");
        }
    }

    let create_output = cmd.output()?;
    if !create_output.status.success() {
        let _ = fs::remove_file(&bat_path);
        let _ = fs::remove_file(&vbs_path);
        return Err(anyhow!("Failed to create task"));
    }

    let run_output = Command::new("schtasks")
        .args(&["/Run", "/TN", &task_name])
        .output()?;

    if !run_output.status.success() {
        let _ = Command::new("schtasks").args(&["/Delete", "/TN", &task_name, "/F"]).output();
        let _ = fs::remove_file(&bat_path);
        let _ = fs::remove_file(&vbs_path);
        return Err(anyhow!("Failed to run task"));
    }

    if delay_secs == 0 {
        let mut retries = 50;
        while retries > 0 {
            let query = Command::new("schtasks")
                .args(&["/Query", "/TN", &task_name])
                .output()?;

            if !query.status.success() {
                return Ok(());
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
            retries -= 1;
        }
        
        let _ = Command::new("schtasks").args(&["/Delete", "/TN", &task_name, "/F"]).output();
        return Err(anyhow!("Timeout waiting for scheduled task execution"));
    }

    Ok(())
}