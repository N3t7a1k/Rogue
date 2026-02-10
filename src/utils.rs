use crate::storage::Storage;
use crate::types::{DateTimeWithPrecision, Precision};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, Timelike, TimeZone, Utc};
use log::{debug, info}; 
use std::fs; 
use std::path::PathBuf; 
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::FILETIME;
use windows_sys::Win32::Foundation::SYSTEMTIME;
use winreg::enums::*;
use winreg::RegKey;

pub fn merge_datetime(original: &DateTime<Utc>, new: &DateTimeWithPrecision) -> DateTime<Utc> {
    match new.precision {
        Precision::DateOnly => {
            new.datetime.with_time(original.time()).unwrap()
               .with_nanosecond(original.nanosecond()).unwrap()
        }
        Precision::Seconds => {
            new.datetime.with_nanosecond(original.nanosecond()).unwrap_or(new.datetime)
        }
        Precision::Milliseconds => {
            new.datetime
        }
    }
}

pub fn string_to_datetime_with_precision(input: &str) -> Option<DateTimeWithPrecision> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(DateTimeWithPrecision {
            datetime: Utc.from_utc_datetime(&dt),
            precision: Precision::Milliseconds,
        });
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
        return Some(DateTimeWithPrecision {
            datetime: Utc.from_utc_datetime(&dt),
            precision: Precision::Seconds,
        });
    }
    if let Ok(d) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let dt = Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap());
        return Some(DateTimeWithPrecision {
            datetime: dt,
            precision: Precision::DateOnly,
        });
    }
    None
}

pub fn systemtime_to_datetime(st: &SYSTEMTIME) -> DateTime<Utc> {
    let naive_date = NaiveDate::from_ymd_opt(
        st.wYear as i32, 
        st.wMonth as u32, 
        st.wDay as u32
    ).unwrap_or_default();

    let naive_datetime = naive_date.and_hms_milli_opt(
        st.wHour as u32, 
        st.wMinute as u32, 
        st.wSecond as u32, 
        st.wMilliseconds as u32
    ).unwrap_or_default();


    Utc.from_utc_datetime(&naive_datetime)
}

pub fn filetime_to_datetime(ft: &FILETIME) -> DateTime<Utc> {
    let duration = ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);

    if duration == 0 {
        return Utc.timestamp_opt(0, 0).unwrap();
    }

    let intervals_per_sec = 10_000_000;
    let windows_epoch_offset = 11_644_473_600;
    let total_seconds = duration / intervals_per_sec;
    let nanos = (duration % intervals_per_sec) * 100;
    let unix_seconds = (total_seconds as i64) - windows_epoch_offset;

    Utc.timestamp_opt(unix_seconds, nanos as u32).unwrap()
}

pub fn filetime_to_string(ft: &FILETIME) -> String {
    let dt = filetime_to_datetime(ft);
    let dt_local: DateTime<Local> = DateTime::from(dt);
    dt_local.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

pub fn datetime_to_filetime(dt: DateTime<Utc>) -> FILETIME {
    let windows_epoch_offset = 11_644_473_600i64;
    let intervals_per_sec = 10_000_000;

    let unix_seconds = dt.timestamp();
    let nanos = dt.timestamp_subsec_nanos() as u64;

    let total_seconds = unix_seconds + windows_epoch_offset;
    let total_ticks = (total_seconds as u64 * intervals_per_sec) + (nanos / 100);

    FILETIME {
        dwLowDateTime: total_ticks as u32,
        dwHighDateTime: (total_ticks >> 32) as u32,
    }
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
