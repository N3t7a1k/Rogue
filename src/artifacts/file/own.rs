use crate::{
    storage::Storage,
    utils,
};
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use std::{
    env,
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
};
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{HLOCAL, LocalFree, ERROR_SUCCESS},
        Security::{
            Authorization::{GetSecurityInfo, SE_FILE_OBJECT},
            LookupAccountSidW, 
            SID_NAME_USE, PSECURITY_DESCRIPTOR, PSID,
            OWNER_SECURITY_INFORMATION,
        },
        Storage::FileSystem::READ_CONTROL,
    },
};

#[derive(Debug)]
pub struct SecurityRecord {
    pub filename: String,
    pub path: String,
    pub owner: String,
    pub group: String,
}

fn fetch_owner(path: &Path) -> Result<SecurityRecord> {
    let path_str = path.to_string_lossy().to_string();

    unsafe {
        let safe_handle = utils::file::open_handle(&path_str, READ_CONTROL.0)?;

        let mut owner_psid = PSID::default();
        let mut sec_desc = PSECURITY_DESCRIPTOR::default();

        let result = GetSecurityInfo(
            safe_handle.0,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut owner_psid),
            None,
            None,
            None,
            Some(&mut sec_desc),
        );
        if result != ERROR_SUCCESS {
             return Err(anyhow!("Failed to get security info. Error code: {}", result.0));
        }

        let owner_name = sid_to_name(owner_psid).unwrap_or_else(|_| "Unknown".to_string());
        
        if !sec_desc.is_invalid() {
            let hlocal = HLOCAL(sec_desc.0 as *mut c_void);
            let _ = LocalFree(Some(hlocal)); 
        }

        let filename = path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());

        Ok(SecurityRecord {
            filename,
            path: path_str,
            owner: owner_name,
            group: String::new(),
        })
    }
}

unsafe fn sid_to_name(psid: PSID) -> Result<String> {
    if psid.is_invalid() {
        return Err(anyhow!("Invalid SID"));
    }

    let mut name_len: u32 = 0;
    let mut domain_len: u32 = 0;
    let mut sid_use = SID_NAME_USE::default();

    unsafe {
        let _ = LookupAccountSidW(
            None,
            psid,
            None,
            &mut name_len,
            None,
            &mut domain_len,
            &mut sid_use,
        );
    }

    if name_len == 0 {
        return Err(anyhow!("Failed to verify SID length"));
    }

    let mut name_buf: Vec<u16> = vec![0; name_len as usize];
    let mut domain_buf: Vec<u16> = vec![0; domain_len as usize];

    let result = unsafe {
        LookupAccountSidW(
            None,
            psid,
            Some(PWSTR(name_buf.as_mut_ptr())),
            &mut name_len,
            Some(PWSTR(domain_buf.as_mut_ptr())),
            &mut domain_len,
            &mut sid_use,
        )
    };

    if result.is_err() {
        return Err(anyhow!("LookupAccountSidW failed"));
    }

    let name = String::from_utf16_lossy(&name_buf);
    let domain = String::from_utf16_lossy(&domain_buf);

    let clean_name = name.trim_end_matches('\0');
    let clean_domain = domain.trim_end_matches('\0');

    if clean_domain.is_empty() {
        Ok(clean_name.to_string())
    } else {
        Ok(format!("{}\\{}", clean_domain, clean_name))
    }
}

pub fn get_owner(pattern: &str) -> Result<Vec<SecurityRecord>> {
    let storage = Storage::instance();

    if storage.as_system {
        if !storage.is_admin {
            return Err(anyhow!("Administrator privilege required for system option"));
        }
        let exe_path = env::current_exe()?;
        let exe_str = exe_path.to_string_lossy();
        let output_file = PathBuf::from("C:\\Windows\\Temp\\rogue_owner_output.txt");
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
            "\"{}\" file own get \"{}\" > \"{}\"", 
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
        match fetch_owner(&path) {
            Ok(record) => results.push(record),
            Err(e) => warn!("Skipping {}: {}", path.display(), e),
        }
    }

    Ok(results)
}

pub fn set_owner(pattern: &str, new_owner: &str) -> Result<()> {
    let storage = Storage::instance();

    if !storage.is_admin {
        return Err(anyhow!("Administrator privilege is required to change file ownership."));
    }

    if storage.as_system {
        let exe_path = env::current_exe()?;
        let exe_str = exe_path.to_string_lossy();
        let output_file = PathBuf::from("C:\\Windows\\Temp\\rogue_set_owner_output.txt");
        let output_path_str = output_file.to_string_lossy();
        
        let path_obj = Path::new(pattern);
        let abs_pattern = if path_obj.is_absolute() {
            pattern.to_string()
        } else {
            let cwd = env::current_dir()?;
            cwd.join(pattern).to_string_lossy().to_string()
        };
        
        let safe_pattern = abs_pattern.replace("\"", "\\\""); 
        let safe_owner = new_owner.replace("\"", "\\\"");
        let dry_run = if storage.dry_run { "--dry-run " } else { "" };

        let script_content = format!(
            "\"{}\" {}file own set \"{}\" \"{}\" > \"{}\"", 
            exe_str, dry_run, safe_pattern, safe_owner, output_path_str
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

        return Ok(());
    }

    let targets = utils::file::get_files(pattern)?;
    let total = targets.len();
    let mut success_count = 0;

    info!("Changing owner to '{}' for {} files...", new_owner, total);

    for path in targets {
        let path_str = path.to_string_lossy();
        
        if Storage::instance().dry_run {
            info!("[DRY RUN] Would change owner of '{}' to '{}'", path_str, new_owner);
            success_count += 1;
            continue;
        }

        let _ = std::process::Command::new("takeown")
            .args(&["/f", &*path_str])
            .output();

        let output = std::process::Command::new("icacls")
            .arg(&*path_str)
            .arg("/setowner")
            .arg(new_owner)
            .arg("/q")
            .output();

        match output {
            Ok(out) if out.status.success() => {
                success_count += 1;
                info!("Changed owner of: {}", path_str);
            },
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr);
                error!("Failed to change owner of {}: {}", path_str, err.trim());
            },
            Err(e) => error!("Failed to execute icacls: {}", e),
        }
    }

    info!("Operation completed. {}/{} succeeded.", success_count, total);
    Ok(())
}
