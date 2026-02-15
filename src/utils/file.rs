use anyhow::{anyhow, Result};
use std::{
    fs, 
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf}
};
use wildmatch::WildMatch;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, 
        FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS,
    },
};

pub struct SafeHandle(pub HANDLE);

impl Drop for SafeHandle {
    fn drop(&mut self) {
        if self.0 != INVALID_HANDLE_VALUE && !self.0.is_invalid() {
            unsafe { let _ = CloseHandle(self.0); }
        }
    }
}

pub unsafe fn open_handle(path: &str, access_rights: u32) -> Result<SafeHandle> {
    let wide_path: Vec<u16> = OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
    
    let handle = unsafe { CreateFileW(
        windows::core::PCWSTR(wide_path.as_ptr()),
        access_rights,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL | FILE_FLAG_BACKUP_SEMANTICS,
        None,
    )? };

    if handle == INVALID_HANDLE_VALUE {
        return Err(anyhow!("Failed to open handle: {}", path));
    }

    Ok(SafeHandle(handle))
}

pub fn get_files(pattern: &str) -> Result<Vec<PathBuf>> {
    let mut targets = Vec::new();
    let path_obj = Path::new(pattern);

    if !pattern.contains('*') && !pattern.contains('?') {
        if !path_obj.exists() {
             return Err(anyhow!("Target not found: {}", pattern));
        }
        if let Ok(canon) = fs::canonicalize(path_obj) {
            targets.push(canon);
        } else {
            targets.push(path_obj.to_path_buf());
        }
        return Ok(targets);
    }

    let parent_dir = if let Some(parent) = path_obj.parent() {
        if parent.as_os_str().is_empty() { Path::new(".") } else { parent }
    } else { Path::new(".") };
    
    let filename_pattern = path_obj.file_name()
        .ok_or_else(|| anyhow!("Invalid pattern"))?
        .to_string_lossy();
    let matcher = WildMatch::new(&filename_pattern);

    if parent_dir.is_dir() {
        let entries = match fs::read_dir(parent_dir) {
            Ok(e) => e,
            Err(e) => return Err(anyhow!("Failed to read directory {}: {}", parent_dir.display(), e)),
        };

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    
                    if matcher.matches(&name_str) {
                        match fs::canonicalize(&path) {
                            Ok(canon) => targets.push(canon),
                            Err(_) => {
                                targets.push(path);
                            }
                        }
                    }
                }
            }
        }
    } else {
        return Err(anyhow!("Directory not found: {}", parent_dir.display()));
    }
    
    Ok(targets)
}
