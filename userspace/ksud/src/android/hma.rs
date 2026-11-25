#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

use crate::{
    android::utils::{ensure_dir_exists, ensure_file_exists},
    defs::{BUILTIN_HMA_DIR, BUILTIN_HMA_FLAG, DISABLE_FILE_NAME},
};
use anyhow::{Context, Result};
use log::info;
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Serialize, Debug, Clone)]
pub struct HmaStatus {
    pub enabled: bool,
    pub installed: bool,
    pub version: String,
    pub uid_filter_active: bool,
    pub uid_scope_ready: bool,
    pub scoped_apps_count: usize,
    pub active_generation: String,
}

pub fn is_installed() -> bool {
    let dir = Path::new(BUILTIN_HMA_DIR);
    dir.is_dir() && dir.join("zygisk/arm64-v8a.so").exists()
}

pub fn is_enabled() -> bool {
    let flag = Path::new(BUILTIN_HMA_FLAG);
    let disable_file = Path::new(BUILTIN_HMA_DIR).join(DISABLE_FILE_NAME);
    flag.exists() && is_installed() && !disable_file.exists()
}

pub fn enable() -> Result<()> {
    ensure_file_exists(BUILTIN_HMA_FLAG)?;
    let disable_file = Path::new(BUILTIN_HMA_DIR).join(DISABLE_FILE_NAME);
    if disable_file.exists() {
        let _ = fs::remove_file(disable_file);
    }
    info!("Built-in HMA OSS + UID has been enabled");
    Ok(())
}

pub fn disable() -> Result<()> {
    let flag = Path::new(BUILTIN_HMA_FLAG);
    if flag.exists() {
        let _ = fs::remove_file(flag);
    }
    let dir = Path::new(BUILTIN_HMA_DIR);
    if dir.exists() {
        let disable_file = dir.join(DISABLE_FILE_NAME);
        ensure_file_exists(&disable_file.to_string_lossy())?;
    }
    info!("Built-in HMA OSS + UID has been disabled");
    Ok(())
}

fn get_boot_id() -> Option<String> {
    let path = Path::new("/proc/sys/kernel/random/boot_id");
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn find_active_hma_dir() -> Option<PathBuf> {
    let base = Path::new("/data/misc");
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("hide_my_applist_") || name == "hide_my_applist" {
                    return Some(path);
                }
            }
        }
    }
    None
}

pub fn status() -> HmaStatus {
    let installed = is_installed();
    let enabled = is_enabled();
    let mut version = String::from("177-UID3.1");

    let prop_path = Path::new(BUILTIN_HMA_DIR).join("module.prop");
    if let Ok(file) = File::open(prop_path) {
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if let Some(rest) = line.strip_prefix("version=") {
                version = rest.trim().to_string();
                break;
            }
        }
    }

    let mut uid_scope_ready = false;
    let mut scoped_apps_count = 0;
    let mut active_generation = String::new();

    if let (Some(boot_id), Some(hma_dir)) = (get_boot_id(), find_active_hma_dir()) {
        let active_file = hma_dir.join("uid_scope/active");
        if let Ok(file) = File::open(&active_file) {
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[0] == boot_id {
                    let gen = parts[1];
                    if gen.starts_with('g') {
                        active_generation = gen.to_string();
                        let gen_dir = hma_dir.join("uid_scope").join(gen);
                        if let Ok(entries) = fs::read_dir(&gen_dir) {
                            scoped_apps_count = entries.flatten().count();
                            uid_scope_ready = scoped_apps_count > 0;
                        }
                    }
                }
            }
        }
    }

    HmaStatus {
        enabled,
        installed,
        version,
        uid_filter_active: enabled && installed && uid_scope_ready,
        uid_scope_ready,
        scoped_apps_count,
        active_generation,
    }
}

pub fn sync_uid_scope() -> Result<usize> {
    let Some(boot_id) = get_boot_id() else {
        anyhow::bail!("Failed to read kernel boot_id from /proc/sys/kernel/random/boot_id");
    };

    let Some(hma_dir) = find_active_hma_dir() else {
        anyhow::bail!("No active HMA directory found under /data/misc/hide_my_applist_*");
    };

    let uid_scope_dir = hma_dir.join("uid_scope");
    ensure_dir_exists(&uid_scope_dir.to_string_lossy())?;

    let gen = "g0";
    let active_content = format!("{boot_id} {gen}\n");
    let active_path = uid_scope_dir.join("active");
    fs::write(&active_path, active_content)
        .with_context(|| format!("Failed to write {}", active_path.display()))?;

    let gen_dir = uid_scope_dir.join(gen);
    ensure_dir_exists(&gen_dir.to_string_lossy())?;

    let count = fs::read_dir(&gen_dir)
        .map(|entries| entries.flatten().count())
        .unwrap_or(0);

    info!("HMA UID scope synced successfully for boot_id={boot_id}, generation={gen}, rules_count={count}");
    Ok(count)
}
