use crate::{
    storage::Storage,
    utils,
};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use log::{error, info, warn};
use std::{
    env,
    fs,
    path::{Path, PathBuf}
};
use windows::Win32::{
    Foundation::FILETIME,
    Storage::FileSystem::{
        FILE_READ_ATTRIBUTES, FILE_WRITE_ATTRIBUTES,
        GetFileTime, SetFileTime
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaceType { Created, Accessed, Modified, All }

impl MaceType {
    pub fn to_str(&self) -> &'static str {
        match self {
            MaceType::Created => "created",
            MaceType::Accessed => "accessed",
            MaceType::Modified => "modified",
            MaceType::All => "all",
        }
    }
}

#[derive(Debug)]
pub struct TimestampRecord {
    pub filename: String,
    pub path: String,
    pub created: DateTime<Utc>,
    pub accessed: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

fn fetch_filestamp(path: &Path) -> Result<TimestampRecord> {
    let path_str = path.to_string_lossy().to_string();
    let filename = path.file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path_str.clone());

    unsafe {
        let safe_handle = utils::file::open_handle(&path_str, FILE_READ_ATTRIBUTES.0)?;
        
        let mut c_ft = FILETIME::default();
        let mut a_ft = FILETIME::default();
        let mut m_ft = FILETIME::default();

        let success = GetFileTime(safe_handle.0, Some(&mut c_ft), Some(&mut a_ft), Some(&mut m_ft)).is_ok();
        
        if !success { return Err(anyhow!("Failed to retrieve timestamps")); }

        Ok(TimestampRecord {
            filename,
            path: path_str,
            created: utils::time::filetime_to_datetime(&c_ft),
            accessed: utils::time::filetime_to_datetime(&a_ft),
            modified: utils::time::filetime_to_datetime(&m_ft),
        })
    }
}

fn modify_timestamp(path: &Path, mace: MaceType, time_input: &str) -> Result<()> {
    let new_time = utils::time::string_to_datetime_with_precision(time_input);
    if new_time.is_none() { return Err(anyhow!("Invalid time format")); }
    let new_time = new_time.unwrap();

    let path_str = path.to_string_lossy().to_string();
    
    let (cur_c, cur_a, cur_m) = unsafe {
        let safe_handle = utils::file::open_handle(&path_str, FILE_READ_ATTRIBUTES.0)?;
        let mut c = FILETIME::default();
        let mut a = FILETIME::default();
        let mut m = FILETIME::default();
        
        if GetFileTime(safe_handle.0, Some(&mut c), Some(&mut a), Some(&mut m)).is_err() {
            return Err(anyhow!("Failed to read current timestamps"));
        }
        (
            utils::time::filetime_to_datetime(&c),
            utils::time::filetime_to_datetime(&a),
            utils::time::filetime_to_datetime(&m)
        )
    };

    let (target_c, target_a, target_m) = match mace {
        MaceType::Created => (
            Some(utils::time::datetime_to_filetime(utils::time::merge_datetime(&cur_c, &new_time))),
            None,
            None,
        ),
        MaceType::Accessed => (
            None,
            Some(utils::time::datetime_to_filetime(utils::time::merge_datetime(&cur_a, &new_time))),
            None,
        ),
        MaceType::Modified => (
            None,
            None,
            Some(utils::time::datetime_to_filetime(utils::time::merge_datetime(&cur_m, &new_time))),
        ),
        MaceType::All => {
            let final_c = utils::time::merge_datetime(&cur_c, &new_time);
            let final_m = utils::time::merge_datetime(&cur_m, &new_time);
            
            let final_a = if cur_a > cur_m {
                utils::time::merge_datetime(&cur_a, &new_time)
            } else {
                utils::time::merge_datetime(&cur_m, &new_time)
            };

            (
                Some(utils::time::datetime_to_filetime(final_c)),
                Some(utils::time::datetime_to_filetime(final_a)),
                Some(utils::time::datetime_to_filetime(final_m)),
            )
        }
    };

    if Storage::instance().dry_run {
        if let Some(c) = target_c { info!("Would set Created: {}", utils::time::filetime_to_string(&c)); }
        if let Some(a) = target_a { info!("Would set Accessed: {}", utils::time::filetime_to_string(&a)); }
        if let Some(m) = target_m { info!("Would set Modified: {}", utils::time::filetime_to_string(&m)); }
        return Ok(());
    }

    unsafe {
        {
            let safe_handle = utils::file::open_handle(&path_str, FILE_WRITE_ATTRIBUTES.0)?;
            let result = SetFileTime(
                safe_handle.0,
                target_c.as_ref().map(|t| t as *const _),
                target_a.as_ref().map(|t| t as *const _),
                target_m.as_ref().map(|t| t as *const _),
            );
            
            if result.is_err() {
                return Err(anyhow!("Failed to set timestamps"));
            }
        }

        if mace == MaceType::All || mace == MaceType::Accessed {
            if let Some(final_a) = target_a {
                if let Ok(safe_handle_retry) = utils::file::open_handle(&path_str, FILE_WRITE_ATTRIBUTES.0) {
                     let _ = SetFileTime(
                        safe_handle_retry.0,
                        None,
                        Some(&final_a as *const _),
                        None,
                    );
                }
            }
        }
    }

    Ok(())
}

pub fn get_timestamps(pattern: &str) -> Result<Vec<TimestampRecord>> {
    let storage = Storage::instance();
    
    if storage.as_system {
        if !storage.is_admin {
            return Err(anyhow!("Administrator privilege required for system option"));
        }
        let exe_path = env::current_exe()?;
        let exe_str = exe_path.to_string_lossy();
        let output_file = PathBuf::from("C:\\Windows\\Temp\\rogue_output.txt");
        let output_path_str = output_file.to_string_lossy();
        let path_obj = Path::new(pattern);
        let abs_pattern = if path_obj.is_absolute() {
            pattern.to_string()
        } else {
            let cwd = env::current_dir()?;
            cwd.join(pattern).to_string_lossy().to_string()
        };
        let safe_pattern = abs_pattern.replace("\"", "\\\""); 
        let script_content = format!(
            "\"{}\" time get \"{}\" > \"{}\"", 
            exe_str, safe_pattern, output_path_str
        );

        utils::system::run_scheduled_command(&script_content, true, 0)?;

        if output_file.exists() {
            let content = fs::read_to_string(&output_file)?;
            if content.lines().count() > 4 {
                println!("{}", content.lines().skip(4).collect::<Vec<&str>>().join("\n"));
            }
            let _ = fs::remove_file(&output_file);
        } else {
            error!("Timeout waiting for SYSTEM process output.");
        }

        return Ok(Vec::new());
    }

    let targets = utils::file::get_files(pattern)?;
    let mut results = Vec::new();

    for path in targets {
        match fetch_filestamp(&path) {
            Ok(record) => results.push(record),
            Err(e) => warn!("Skipping {}: {}", path.display(), e),
        }
    }

    Ok(results)
}

pub fn set_timestamps(pattern: &str, mace: MaceType, time: &str) -> Result<()> {
    let storage = Storage::instance();
    
    if storage.as_system {
        if !storage.is_admin {
            return Err(anyhow!("Administrator privilege required for system option"));
        }

        let exe_path = env::current_exe()?;
        let exe_str = exe_path.to_string_lossy();
        let output_file = PathBuf::from("C:\\Windows\\Temp\\rogue_set_output.txt");
        let output_path_str = output_file.to_string_lossy();
        let path_obj = Path::new(pattern);
        let abs_pattern = if path_obj.is_absolute() {
            pattern.to_string()
        } else {
            let cwd = env::current_dir()?;
            cwd.join(pattern).to_string_lossy().to_string()
        };

        let safe_pattern = abs_pattern.replace("\"", "\\\"");
        let safe_time = time.replace("\"", "\\\"");
        let dry_run = if Storage::instance().dry_run { "--dry-run " } else { "" };
        
        let script_content = format!(
            "\"{}\" {}time set \"{}\" \"{}\" \"{}\" > \"{}\"", 
            exe_str, dry_run, mace.to_str(), safe_pattern, safe_time, output_path_str
        ); 

        if output_file.exists() {
            let _ = fs::remove_file(&output_file);
        }

        utils::system::run_scheduled_command(&script_content, true, 0)?;

        if output_file.exists() {
            let content = fs::read_to_string(&output_file)?;
            if content.lines().count() > 4 {
                println!("{}", content.lines().skip(4).collect::<Vec<&str>>().join("\n"));
            }
            let _ = fs::remove_file(&output_file);
        } else {
            error!("Timeout waiting for SYSTEM process output.");
        }

        return Ok(());
    }

    let targets = utils::file::get_files(pattern)?;
    let total = targets.len();
    let mut success_count = 0;

    info!("Found {} targets matching '{}'.", total, pattern);

    for path in targets {
        match modify_timestamp(&path, mace, time) {
            Ok(_) => {
                success_count += 1;
            },
            Err(e) => {
                error!("Failed to modify {}: {}", path.display(), e);
            }
        }
    }

    info!("Operation completed. {}/{} succeeded.", success_count, total);

    Ok(())
}
