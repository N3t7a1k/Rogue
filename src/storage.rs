use std::{
    env,
    path::PathBuf,
    sync::OnceLock,
};
use is_elevated::is_elevated;

#[derive(Debug)]
pub struct Storage {
    pub as_system: bool,
    pub dry_run: bool,
    pub is_admin: bool,
    pub system_root: PathBuf,
}

static STORAGE: OnceLock<Storage> = OnceLock::new();

impl Storage {
    pub fn init(dry_run: bool, as_system: bool) {
        let data = Storage {
            as_system,
            dry_run,
            is_admin: is_elevated(),
            system_root: PathBuf::from(env::var("SystemRoot").unwrap_or("C:\\Windows".to_string())),
        };
        
        STORAGE.set(data).expect("Env has already been initialized");
    }

    pub fn instance() -> &'static Storage {
        STORAGE.get().expect("Env is not initialized")
    }
}
