use std::env;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;

pub fn get_mtb_bin() -> String {
    env::var("MTB_BIN").unwrap_or_else(|_| "/vendor/bin/mtb".to_string())
}

pub fn get_mtbtool_dir() -> PathBuf {
    env::var("MTBTOOL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/data/adb/mtbtool"))
}

pub fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let dir = get_mtbtool_dir();
    fs::create_dir_all(&dir)?;
    let backups = dir.join("backups");
    fs::create_dir_all(&backups)?;
    Ok(dir)
}

pub fn exec_mtb(args: &[&str]) -> (i32, String) {
    let bin = get_mtb_bin();
    match Command::new(&bin).args(args).output() {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let mut out = String::from_utf8_lossy(&output.stdout).to_string();
            let err = String::from_utf8_lossy(&output.stderr);
            if !err.is_empty() {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&err);
            }
            (exit_code, out)
        }
        Err(e) => (-1, format!("Failed to execute {}: {}", bin, e)),
    }
}

/// Variant for argument lists built at runtime (e.g. decimal NV bytes):
/// each element becomes a separate argv entry — mtb expects one byte per arg.
pub fn exec_mtb_owned(args: Vec<String>) -> (i32, String) {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    exec_mtb(&refs)
}

pub struct FileLock {
    _file: File,
}

#[cfg(unix)]
impl FileLock {
    pub fn acquire() -> Result<Self, String> {
        use std::os::unix::io::AsRawFd;
        let dir = ensure_data_dir().map_err(|e| format!("Failed to create data dir: {}", e))?;
        let lock_path = dir.join(".lock");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| format!("Failed to open lockfile {:?}: {}", lock_path, e))?;

        extern "C" {
            fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
        }
        const LOCK_EX: std::os::raw::c_int = 2;

        let ret = unsafe { flock(file.as_raw_fd(), LOCK_EX) };
        if ret != 0 {
            return Err("Failed to acquire flock on .lock".to_string());
        }

        Ok(FileLock { _file: file })
    }
}

#[cfg(not(unix))]
impl FileLock {
    pub fn acquire() -> Result<Self, String> {
        let dir = ensure_data_dir().map_err(|e| format!("Failed to create data dir: {}", e))?;
        let lock_path = dir.join(".lock");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| format!("Failed to open lockfile {:?}: {}", lock_path, e))?;
        Ok(FileLock { _file: file })
    }
}

#[cfg(unix)]
impl Drop for FileLock {
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        extern "C" {
            fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
        }
        const LOCK_UN: std::os::raw::c_int = 8;
        unsafe {
            flock(self._file.as_raw_fd(), LOCK_UN);
        }
    }
}
