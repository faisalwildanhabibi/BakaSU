#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

use crate::{
    android::utils::{ensure_dir_exists, ensure_file_exists},
    defs::{BUILTIN_HMA_DIR, BUILTIN_HMA_FLAG, DISABLE_FILE_NAME, DISABLE_HMA_FLAG},
};
use anyhow::{Context, Result};
use log::info;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

pub const PRESET_HIDDEN_PACKAGES: &[&str] = &[
    "org.bakasu.bakasu",
    "io.github.a13e300.ksu",
    "me.weishu.kernelsu",
    "me.bmax.apatch",
    "com.topjohnwu.magisk",
    "org.lsposed.manager",
    "org.frknkrc44.hma_oss",
    "icu.nullptr.hidemyapplist",
    "com.sukisu.ultra",
    "bin.mt.plus",
    "np.manager",
    "com.chiteroman.playintegrityfix",
];

#[derive(Serialize, Debug, Clone)]
pub struct HmaStatus {
    pub enabled: bool,
    pub installed: bool,
    pub version: String,
    pub uid_filter_active: bool,
    pub uid_scope_ready: bool,
    pub scoped_apps_count: usize,
    pub active_generation: String,
    pub root_granted_hidden_count: usize,
}

pub fn is_installed() -> bool {
    let dir = Path::new(BUILTIN_HMA_DIR);
    dir.is_dir() && dir.join("zygisk/arm64-v8a.so").exists()
}

/// HMA is enabled by default unless explicitly disabled by the user
pub fn is_enabled() -> bool {
    let disable_flag = Path::new(DISABLE_HMA_FLAG);
    let disable_file = Path::new(BUILTIN_HMA_DIR).join(DISABLE_FILE_NAME);
    !disable_flag.exists() && !disable_file.exists()
}

pub fn enable() -> Result<()> {
    let disable_flag = Path::new(DISABLE_HMA_FLAG);
    if disable_flag.exists() {
        let _ = fs::remove_file(disable_flag);
    }
    let disable_file = Path::new(BUILTIN_HMA_DIR).join(DISABLE_FILE_NAME);
    if disable_file.exists() {
        let _ = fs::remove_file(disable_file);
    }
    ensure_file_exists(BUILTIN_HMA_FLAG)?;
    info!("Built-in HMA OSS + UID has been enabled");
    Ok(())
}

pub fn disable() -> Result<()> {
    ensure_file_exists(DISABLE_HMA_FLAG)?;
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

fn find_or_create_active_hma_dir() -> Result<PathBuf> {
    let base = Path::new("/data/misc");
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("hide_my_applist_") || name == "hide_my_applist" {
                    return Ok(path);
                }
            }
        }
    }
    let fallback = base.join("hide_my_applist_main");
    ensure_dir_exists(&fallback.to_string_lossy())?;
    Ok(fallback)
}

fn get_root_granted_uids() -> HashSet<u32> {
    let mut root_uids = HashSet::new();
    let profile_dir = Path::new("/data/adb/ksu/profile");
    if let Ok(entries) = fs::read_dir(profile_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(uid) = name.parse::<u32>() {
                        root_uids.insert(uid);
                    }
                }
            }
        }
    }
    root_uids
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
    let root_granted_hidden_count = get_root_granted_uids().len();

    if let (Some(boot_id), Ok(hma_dir)) = (get_boot_id(), find_or_create_active_hma_dir()) {
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
        root_granted_hidden_count,
    }
}

/// Automatically harmonizes HMA hide list with root-granted applications,
/// hiding them from all non-system, non-root applications via Seccomp UID filter and PMS hooks.
pub fn sync_uid_scope() -> Result<usize> {
    let Some(boot_id) = get_boot_id() else {
        anyhow::bail!("Failed to read kernel boot_id from /proc/sys/kernel/random/boot_id");
    };

    let hma_dir = find_or_create_active_hma_dir()?;
    let uid_scope_dir = hma_dir.join("uid_scope");
    ensure_dir_exists(&uid_scope_dir.to_string_lossy())?;

    let root_granted_uids = get_root_granted_uids();
    let mut hidden_uids: HashSet<u32> = HashSet::new();
    let mut hidden_packages: HashSet<String> = HashSet::new();

    for pkg in PRESET_HIDDEN_PACKAGES {
        hidden_packages.insert((*pkg).to_string());
    }

    // Map installed packages from /data/system/packages.list
    let mut all_packages: HashMap<String, u32> = HashMap::new();
    let mut user_non_root_apps: Vec<(String, u32)> = Vec::new();

    let packages_list_path = Path::new("/data/system/packages.list");
    if let Ok(file) = File::open(packages_list_path) {
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let pkg_name = parts[0];
                if let Ok(uid) = parts[1].parse::<u32>() {
                    all_packages.insert(pkg_name.to_string(), uid);
                    let data_dir = parts[3];

                    let is_root_granted = root_granted_uids.contains(&uid);
                    let is_preset_hidden = PRESET_HIDDEN_PACKAGES.contains(&pkg_name);

                    if is_root_granted || is_preset_hidden {
                        hidden_uids.insert(uid);
                        hidden_packages.insert(pkg_name.to_string());
                    } else if (10000..=19999).contains(&uid)
                        && (data_dir.starts_with("/data/user/") || data_dir.starts_with("/data/data/"))
                    {
                        user_non_root_apps.push((pkg_name.to_string(), uid));
                    }
                }
            }
        }
    }

    // Ensure all root granted UIDs are in hidden_uids
    for uid in &root_granted_uids {
        hidden_uids.insert(*uid);
    }

    let mut hidden_uid_list: Vec<u32> = hidden_uids.into_iter().collect();
    hidden_uid_list.sort_unstable();
    hidden_uid_list.truncate(128); // HMA-OSS-UID max limit per rule file

    let generation = "g0";
    let gen_dir = uid_scope_dir.join(generation);
    ensure_dir_exists(&gen_dir.to_string_lossy())?;

    // 1. Generate UID Seccomp scope rule files for non-root target applications
    let mut synced_count = 0;
    for (_pkg, target_uid) in &user_non_root_apps {
        let app_hidden: Vec<u32> = hidden_uid_list
            .iter()
            .copied()
            .filter(|&h_uid| h_uid != *target_uid)
            .collect();

        if app_hidden.is_empty() {
            continue;
        }

        let mut content = format!("HMA_UID_3\n{target_uid}\n");
        for h_uid in app_hidden {
            content.push_str(&format!("{h_uid}\n"));
        }

        let rule_path = gen_dir.join(target_uid.to_string());
        if fs::write(&rule_path, content).is_ok() {
            synced_count += 1;
        }
    }

    // 2. Write active marker
    let active_content = format!("{boot_id} {generation}\n");
    let active_path = uid_scope_dir.join("active");
    fs::write(&active_path, active_content)
        .with_context(|| format!("Failed to write {}", active_path.display()))?;

    // 3. Write/Update HMA config.json for system_server Package Manager hooks
    let config_path = hma_dir.join("config.json");
    let mut config_obj = if let Ok(existing_bytes) = fs::read(&config_path) {
        serde_json::from_slice::<serde_json::Value>(&existing_bytes)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    config_obj["configVersion"] = serde_json::json!(95);
    config_obj["uidFilterEnabled"] = serde_json::json!(true);
    config_obj["uidFilterRevision"] = serde_json::json!(3);
    config_obj["forceMountData"] = serde_json::json!(true);
    config_obj["skipSystemAppDataIsolation"] = serde_json::json!(true);

    let hidden_packages_vec: Vec<String> = hidden_packages.into_iter().collect();
    if config_obj.get("templates").is_none() {
        config_obj["templates"] = serde_json::json!({});
    }
    config_obj["templates"]["baka_root_preset"] = serde_json::json!({
        "isWhitelist": false,
        "appList": hidden_packages_vec
    });

    if config_obj.get("scope").is_none() {
        config_obj["scope"] = serde_json::json!({});
    }

    let scope_map = config_obj["scope"].as_object_mut();
    if let Some(scope) = scope_map {
        for (pkg, _uid) in &user_non_root_apps {
            scope.insert(
                pkg.clone(),
                serde_json::json!({
                    "useWhitelist": false,
                    "applyTemplates": ["baka_root_preset"]
                }),
            );
        }
    }

    if let Ok(pretty_config) = serde_json::to_string_pretty(&config_obj) {
        let _ = fs::write(&config_path, pretty_config);
    }

    info!(
        "HMA auto-isolation synced: {} target apps protected from {} root/preset apps",
        synced_count,
        hidden_uid_list.len()
    );

    Ok(synced_count)
}
