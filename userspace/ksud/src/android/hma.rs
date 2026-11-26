#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

use crate::{
    android::utils::{ensure_dir_exists, ensure_file_exists},
    defs::{BUILTIN_HMA_DIR, BUILTIN_HMA_FLAG, DISABLE_FILE_NAME, DISABLE_HMA_FLAG},
};
use anyhow::{Context, Result};
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Command,
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

/// HMA is disabled by default unless explicitly enabled by the user
pub fn is_enabled() -> bool {
    let enable_flag = Path::new(BUILTIN_HMA_FLAG);
    let disable_flag = Path::new(DISABLE_HMA_FLAG);
    let disable_file = Path::new(BUILTIN_HMA_DIR).join(DISABLE_FILE_NAME);
    enable_flag.exists() && !disable_flag.exists() && !disable_file.exists()
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
        ensure_file_exists(&disable_file)?;
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
    ensure_dir_exists(&fallback)?;
    Ok(fallback)
}

fn get_root_granted_uids() -> HashSet<u32> {
    let mut root_uids = HashSet::new();
    let profile_dir = Path::new("/data/adb/ksu/profile");
    if let Ok(entries) = fs::read_dir(profile_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && let Ok(uid) = name.parse::<u32>()
            {
                root_uids.insert(uid);
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
                    let gen_str = parts[1];
                    if gen_str.starts_with('g') {
                        active_generation = gen_str.to_string();
                        let gen_dir = hma_dir.join("uid_scope").join(gen_str);
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

fn read_or_init_config(hma_dir: &Path) -> Value {
    let config_path = hma_dir.join("config.json");
    let mut cfg = if let Ok(bytes) = fs::read(&config_path) {
        serde_json::from_slice::<Value>(&bytes).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !cfg.is_object() {
        cfg = serde_json::json!({});
    }

    let defaults = [
        ("configVersion", serde_json::json!(95)),
        ("uidFilterEnabled", serde_json::json!(true)),
        ("uidFilterRevision", serde_json::json!(3)),
        ("forceMountData", serde_json::json!(true)),
        ("skipSystemAppDataIsolation", serde_json::json!(true)),
        ("altAppDataIsolation", serde_json::json!(false)),
        ("altVoldAppDataIsolation", serde_json::json!(false)),
        ("disableActivityLaunchProtection", serde_json::json!(false)),
        ("detailLog", serde_json::json!(false)),
        ("errorOnlyLog", serde_json::json!(false)),
        ("maxLogSize", serde_json::json!(512)),
        ("ignoredPackagesForPresets", serde_json::json!([])),
        ("templates", serde_json::json!({})),
        ("scope", serde_json::json!({})),
    ];

    if let Some(obj) = cfg.as_object_mut() {
        for (k, v) in defaults {
            obj.entry(k).or_insert(v);
        }
    }

    cfg
}

fn write_config(hma_dir: &Path, cfg: &Value) -> Result<()> {
    let config_path = hma_dir.join("config.json");
    let pretty = serde_json::to_string_pretty(cfg)?;
    fs::write(&config_path, pretty)
        .with_context(|| format!("Failed to write {}", config_path.display()))
}

pub fn get_config_json() -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    Ok(serde_json::to_string_pretty(&cfg)?)
}

pub fn set_config_key(key: &str, value: &str) -> Result<()> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let mut cfg = read_or_init_config(&hma_dir);

    let bool_keys = [
        "uidFilterEnabled",
        "forceMountData",
        "skipSystemAppDataIsolation",
        "altAppDataIsolation",
        "altVoldAppDataIsolation",
        "disableActivityLaunchProtection",
        "detailLog",
        "errorOnlyLog",
    ];

    if bool_keys.contains(&key) {
        let parsed = matches!(value, "1" | "true" | "True" | "TRUE");
        cfg[key] = serde_json::json!(parsed);
        if key == "uidFilterEnabled" {
            cfg["uidFilterRevision"] = serde_json::json!(3);
        }
    } else if key == "maxLogSize" {
        let parsed: i64 = value
            .parse()
            .with_context(|| format!("Invalid integer for maxLogSize: {value}"))?;
        cfg[key] = serde_json::json!(parsed.clamp(64, 4096));
    } else {
        anyhow::bail!("Unsupported HMA config key: {key}");
    }

    write_config(&hma_dir, &cfg)?;
    let _ = sync_uid_scope()?;
    Ok(())
}

pub fn set_template(name: &str, is_whitelist: bool, apps_csv: &str) -> Result<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Template name cannot be empty");
    }
    let hma_dir = find_or_create_active_hma_dir()?;
    let mut cfg = read_or_init_config(&hma_dir);

    let apps: Vec<String> = apps_csv
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if cfg.get("templates").and_then(Value::as_object).is_none() {
        cfg["templates"] = serde_json::json!({});
    }
    cfg["templates"][trimmed] = serde_json::json!({
        "isWhitelist": is_whitelist,
        "appList": apps,
    });

    write_config(&hma_dir, &cfg)?;
    let _ = sync_uid_scope()?;
    Ok(())
}

pub fn delete_template(name: &str) -> Result<()> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let mut cfg = read_or_init_config(&hma_dir);

    if let Some(templates) = cfg.get_mut("templates").and_then(Value::as_object_mut) {
        templates.remove(name);
    }

    if let Some(scope) = cfg.get_mut("scope").and_then(Value::as_object_mut) {
        for app_cfg in scope.values_mut() {
            if let Some(arr) = app_cfg
                .get_mut("applyTemplates")
                .and_then(Value::as_array_mut)
            {
                arr.retain(|v| v.as_str() != Some(name));
            }
        }
    }

    write_config(&hma_dir, &cfg)?;
    let _ = sync_uid_scope()?;
    Ok(())
}

pub fn set_scope(package: &str, app_config_json: &str) -> Result<()> {
    let app_cfg: Value = serde_json::from_str(app_config_json)
        .with_context(|| format!("Invalid AppConfig JSON for {package}"))?;
    if !app_cfg.is_object() {
        anyhow::bail!("AppConfig must be a JSON object");
    }

    let hma_dir = find_or_create_active_hma_dir()?;
    let mut cfg = read_or_init_config(&hma_dir);

    if cfg.get("scope").and_then(Value::as_object).is_none() {
        cfg["scope"] = serde_json::json!({});
    }
    cfg["scope"][package] = app_cfg;

    if let Some(ignored) = cfg
        .get_mut("ignoredPackagesForPresets")
        .and_then(Value::as_array_mut)
    {
        ignored.retain(|v| v.as_str() != Some(package));
    }

    write_config(&hma_dir, &cfg)?;
    let _ = sync_uid_scope()?;
    Ok(())
}

const HMA_PATHS_CONFIG_PATH: &str = "/data/adb/ksu/.hma_paths.json";
const HMA_NM_STATE_PATH: &str = "/data/adb/ksu/.hma_nm_state.json";
const NOMOUNT_BIN: &str = "/data/adb/modules/nomount/bin/nm";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HmaPathsConfig {
    #[serde(default = "default_true")]
    pub auto_hide_app_paths: bool,
    #[serde(default)]
    pub global_paths: Vec<String>,
    #[serde(default)]
    pub per_app_paths: HashMap<String, Vec<String>>,
}

const fn default_true() -> bool {
    true
}

impl Default for HmaPathsConfig {
    fn default() -> Self {
        Self {
            auto_hide_app_paths: true,
            global_paths: Vec::new(),
            per_app_paths: HashMap::new(),
        }
    }
}

fn read_paths_config() -> HmaPathsConfig {
    fs::read_to_string(HMA_PATHS_CONFIG_PATH)
        .ok()
        .and_then(|s| serde_json::from_str::<HmaPathsConfig>(&s).ok())
        .unwrap_or_default()
}

fn write_paths_config(cfg: &HmaPathsConfig) -> Result<()> {
    let json = serde_json::to_string_pretty(cfg)?;
    fs::write(HMA_PATHS_CONFIG_PATH, json)
        .with_context(|| format!("Failed to write {HMA_PATHS_CONFIG_PATH}"))
}

pub fn get_paths_config_json() -> Result<String> {
    let cfg = read_paths_config();
    Ok(serde_json::to_string_pretty(&cfg)?)
}

pub fn set_auto_hide_app_paths(enabled: bool) -> Result<()> {
    let mut cfg = read_paths_config();
    cfg.auto_hide_app_paths = enabled;
    write_paths_config(&cfg)?;
    let _ = sync_uid_scope()?;
    Ok(())
}

fn parse_csv_paths(csv: &str) -> Vec<String> {
    csv.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn set_global_paths(paths_csv: &str) -> Result<()> {
    let mut cfg = read_paths_config();
    cfg.global_paths = parse_csv_paths(paths_csv);
    write_paths_config(&cfg)?;
    let _ = sync_uid_scope()?;
    Ok(())
}

pub fn set_app_paths(package: &str, paths_csv: &str) -> Result<()> {
    let mut cfg = read_paths_config();
    let parsed = parse_csv_paths(paths_csv);
    if parsed.is_empty() {
        cfg.per_app_paths.remove(package);
    } else {
        cfg.per_app_paths.insert(package.to_string(), parsed);
    }
    write_paths_config(&cfg)?;
    let _ = sync_uid_scope()?;
    Ok(())
}

fn collect_data_app_dirs() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let data_app = Path::new("/data/app");
    let Ok(entries) = fs::read_dir(data_app) else {
        return map;
    };

    let mut register_dir = |dir_path: &Path| {
        let Some(name) = dir_path.file_name().and_then(|n| n.to_str()) else {
            return;
        };
        if let Some((pkg, _)) = name.split_once('-') {
            map.insert(pkg.to_string(), dir_path.to_string_lossy().into_owned());
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("~~") {
            if let Ok(sub_entries) = fs::read_dir(&path) {
                for sub in sub_entries.flatten() {
                    let sub_path = sub.path();
                    if sub_path.is_dir() {
                        register_dir(&sub_path);
                    }
                }
            }
        } else {
            register_dir(&path);
        }
    }
    map
}

fn resolve_hidden_packages_for_app(
    target_pkg: &str,
    app_cfg: &Value,
    templates: Option<&serde_json::Map<String, Value>>,
    all_packages: &HashMap<String, u32>,
    system_packages: &HashSet<String>,
) -> HashSet<String> {
    let use_whitelist = app_cfg
        .get("useWhitelist")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let exclude_system = app_cfg
        .get("excludeSystemApps")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let mut listed_pkgs: HashSet<String> = HashSet::new();
    if let Some(extra) = app_cfg.get("extraAppList").and_then(Value::as_array) {
        for item in extra {
            if let Some(pkg) = item.as_str() {
                listed_pkgs.insert(pkg.to_string());
            }
        }
    }

    if let (Some(tpl_map), Some(applied)) = (
        templates,
        app_cfg.get("applyTemplates").and_then(Value::as_array),
    ) {
        for tpl_name in applied.iter().filter_map(Value::as_str) {
            if let Some(tpl) = tpl_map.get(tpl_name) {
                let tpl_whitelist = tpl
                    .get("isWhitelist")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if tpl_whitelist == use_whitelist
                    && let Some(app_list) = tpl.get("appList").and_then(Value::as_array)
                {
                    for pkg in app_list.iter().filter_map(Value::as_str) {
                        listed_pkgs.insert(pkg.to_string());
                    }
                }
            }
        }
    }

    let mut hidden_pkgs: HashSet<String> = HashSet::new();
    if use_whitelist {
        for pkg in all_packages.keys() {
            if pkg == target_pkg || listed_pkgs.contains(pkg) {
                continue;
            }
            if exclude_system && system_packages.contains(pkg) {
                continue;
            }
            hidden_pkgs.insert(pkg.clone());
        }
    } else {
        for pkg in listed_pkgs {
            if pkg != target_pkg && all_packages.contains_key(&pkg) {
                hidden_pkgs.insert(pkg);
            }
        }
    }
    hidden_pkgs
}

fn sync_targeted_paths(
    enabled: bool,
    config_obj: &Value,
    all_packages: &HashMap<String, u32>,
    system_packages: &HashSet<String>,
) {
    let paths_cfg = read_paths_config();
    let templates_map = config_obj.get("templates").and_then(Value::as_object);
    let mut desired_rules: HashSet<(u32, String)> = HashSet::new();

    if enabled && let Some(scope_map) = config_obj.get("scope").and_then(Value::as_object) {
        let data_app_dirs = if paths_cfg.auto_hide_app_paths {
            collect_data_app_dirs()
        } else {
            HashMap::new()
        };

        for (target_pkg, app_cfg) in scope_map {
            let Some(&target_uid) = all_packages.get(target_pkg) else {
                continue;
            };

            let raw_list = paths_cfg
                .per_app_paths
                .get(target_pkg)
                .unwrap_or(&paths_cfg.global_paths);

            let mut exceptions: HashSet<String> = HashSet::new();
            let mut explicit_paths: Vec<String> = Vec::new();
            for item in raw_list {
                if let Some(exc) = item.strip_prefix('!') {
                    let trimmed = exc.trim();
                    if !trimmed.is_empty() {
                        exceptions.insert(trimmed.to_string());
                    }
                } else if item.starts_with('/') {
                    explicit_paths.push(item.clone());
                }
            }

            if paths_cfg.auto_hide_app_paths {
                let hidden_pkgs = resolve_hidden_packages_for_app(
                    target_pkg,
                    app_cfg,
                    templates_map,
                    all_packages,
                    system_packages,
                );
                for h_pkg in hidden_pkgs {
                    if exceptions.contains(&h_pkg) {
                        continue;
                    }
                    if let Some(app_dir) = data_app_dirs.get(&h_pkg)
                        && !exceptions.contains(app_dir)
                    {
                        desired_rules.insert((target_uid, app_dir.clone()));
                    }
                    for p in [
                        format!("/data/data/{h_pkg}"),
                        format!("/data/user/0/{h_pkg}"),
                        format!("/storage/emulated/0/Android/data/{h_pkg}"),
                    ] {
                        if !exceptions.contains(&p) && Path::new(&p).exists() {
                            desired_rules.insert((target_uid, p));
                        }
                    }
                }
            }

            for p in explicit_paths {
                if !exceptions.contains(&p) {
                    desired_rules.insert((target_uid, p));
                }
            }
        }
    }

    if Path::new(NOMOUNT_BIN).exists() {
        let prev_rules: HashSet<(u32, String)> = fs::read_to_string(HMA_NM_STATE_PATH)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<(u32, String)>>(&s).ok())
            .map(|v| v.into_iter().collect())
            .unwrap_or_default();

        for (uid, path) in prev_rules.difference(&desired_rules) {
            let _ = Command::new(NOMOUNT_BIN)
                .args(["rule", "del", "--uid", &uid.to_string(), path])
                .status();
        }
        for (uid, path) in desired_rules.difference(&prev_rules) {
            let _ = Command::new(NOMOUNT_BIN)
                .args(["rule", "add", "--whiteout", "--uid", &uid.to_string(), path])
                .status();
        }

        let state_vec: Vec<(u32, String)> = desired_rules.into_iter().collect();
        if let Ok(json) = serde_json::to_string(&state_vec) {
            let _ = fs::write(HMA_NM_STATE_PATH, json);
        }
    } else if enabled {
        for item in &paths_cfg.global_paths {
            if item.starts_with('/') {
                let _ = crate::android::susfs::api::prelude::add_sus_path(item, true);
            }
        }
    }
}

pub fn remove_scope(package: &str) -> Result<()> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let mut cfg = read_or_init_config(&hma_dir);

    if let Some(scope) = cfg.get_mut("scope").and_then(Value::as_object_mut) {
        scope.remove(package);
    }

    if let Some(ignored) = cfg
        .get_mut("ignoredPackagesForPresets")
        .and_then(Value::as_array_mut)
    {
        let already = ignored.iter().any(|v| v.as_str() == Some(package));
        if !already {
            ignored.push(serde_json::json!(package));
        }
    }

    write_config(&hma_dir, &cfg)?;
    let _ = sync_uid_scope()?;
    Ok(())
}

fn resolve_hidden_uids_for_app(
    target_pkg: &str,
    target_uid: u32,
    app_cfg: &Value,
    templates: Option<&serde_json::Map<String, Value>>,
    all_packages: &HashMap<String, u32>,
    system_packages: &HashSet<String>,
) -> Vec<u32> {
    let use_whitelist = app_cfg
        .get("useWhitelist")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let exclude_system = app_cfg
        .get("excludeSystemApps")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let mut listed_pkgs: HashSet<String> = HashSet::new();
    if let Some(extra) = app_cfg.get("extraAppList").and_then(Value::as_array) {
        for item in extra {
            if let Some(pkg) = item.as_str() {
                listed_pkgs.insert(pkg.to_string());
            }
        }
    }

    if let (Some(tpl_map), Some(applied)) = (
        templates,
        app_cfg.get("applyTemplates").and_then(Value::as_array),
    ) {
        for tpl_name in applied.iter().filter_map(Value::as_str) {
            if let Some(tpl) = tpl_map.get(tpl_name) {
                let tpl_whitelist = tpl
                    .get("isWhitelist")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if tpl_whitelist == use_whitelist
                    && let Some(app_list) = tpl.get("appList").and_then(Value::as_array)
                {
                    for pkg in app_list.iter().filter_map(Value::as_str) {
                        listed_pkgs.insert(pkg.to_string());
                    }
                }
            }
        }
    }

    let mut hidden_uids: HashSet<u32> = HashSet::new();
    if use_whitelist {
        for (pkg, &uid) in all_packages {
            if pkg == target_pkg || uid == target_uid || listed_pkgs.contains(pkg) {
                continue;
            }
            if exclude_system && system_packages.contains(pkg) {
                continue;
            }
            hidden_uids.insert(uid);
        }
    } else {
        for pkg in &listed_pkgs {
            if let Some(&uid) = all_packages.get(pkg)
                && uid != target_uid
            {
                hidden_uids.insert(uid);
            }
        }
    }

    let mut uids: Vec<u32> = hidden_uids.into_iter().collect();
    uids.sort_unstable();
    uids.truncate(128);
    uids
}

/// Automatically harmonizes HMA hide list with root-granted applications,
/// while preserving manual scope/template/isolation customizations in config.json.
pub fn sync_uid_scope() -> Result<usize> {
    let Some(boot_id) = get_boot_id() else {
        anyhow::bail!("Failed to read kernel boot_id from /proc/sys/kernel/random/boot_id");
    };

    let hma_dir = find_or_create_active_hma_dir()?;
    let uid_scope_dir = hma_dir.join("uid_scope");
    ensure_dir_exists(&uid_scope_dir)?;

    let root_granted_uids = get_root_granted_uids();
    let mut hidden_packages: HashSet<String> = HashSet::new();

    for pkg in PRESET_HIDDEN_PACKAGES {
        hidden_packages.insert((*pkg).to_string());
    }

    let mut all_packages: HashMap<String, u32> = HashMap::new();
    let mut system_packages: HashSet<String> = HashSet::new();
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

                    let is_user_range = (10000..=19999).contains(&uid)
                        && (data_dir.starts_with("/data/user/")
                            || data_dir.starts_with("/data/data/"));
                    if !is_user_range {
                        system_packages.insert(pkg_name.to_string());
                    }

                    let is_root_granted = root_granted_uids.contains(&uid);
                    let is_preset_hidden = PRESET_HIDDEN_PACKAGES.contains(&pkg_name);

                    if is_root_granted || is_preset_hidden {
                        hidden_packages.insert(pkg_name.to_string());
                    } else if is_user_range {
                        user_non_root_apps.push((pkg_name.to_string(), uid));
                    }
                }
            }
        }
    }

    let mut config_obj = read_or_init_config(&hma_dir);
    let mut hidden_packages_vec: Vec<String> = hidden_packages.into_iter().collect();
    hidden_packages_vec.sort();

    if config_obj
        .get("templates")
        .and_then(Value::as_object)
        .is_none()
    {
        config_obj["templates"] = serde_json::json!({});
    }
    config_obj["templates"]["baka_root_preset"] = serde_json::json!({
        "isWhitelist": false,
        "appList": hidden_packages_vec
    });

    let ignored: HashSet<String> = config_obj
        .get("ignoredPackagesForPresets")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();

    if config_obj.get("scope").and_then(Value::as_object).is_none() {
        config_obj["scope"] = serde_json::json!({});
    }
    if let Some(scope) = config_obj.get_mut("scope").and_then(Value::as_object_mut) {
        for (pkg, _uid) in &user_non_root_apps {
            if !scope.contains_key(pkg) && !ignored.contains(pkg) {
                scope.insert(
                    pkg.clone(),
                    serde_json::json!({
                        "useWhitelist": false,
                        "applyTemplates": ["baka_root_preset"]
                    }),
                );
            }
        }
    }

    let _ = write_config(&hma_dir, &config_obj);

    let uid_filter_enabled = is_enabled()
        && config_obj
            .get("uidFilterEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(true);

    let active_path = uid_scope_dir.join("active");
    if !uid_filter_enabled {
        if active_path.exists() {
            let _ = fs::remove_file(&active_path);
        }
        info!("HMA UID Seccomp filter disabled via config; PMS config.json updated");
        return Ok(0);
    }

    let generation = "g0";
    let gen_dir = uid_scope_dir.join(generation);
    ensure_dir_exists(&gen_dir)?;

    let templates_map = config_obj.get("templates").and_then(Value::as_object);
    let mut synced_uids: HashSet<String> = HashSet::new();
    let mut synced_count = 0;

    if let Some(scope_map) = config_obj.get("scope").and_then(Value::as_object) {
        for (target_pkg, app_cfg) in scope_map {
            let Some(&target_uid) = all_packages.get(target_pkg) else {
                continue;
            };

            let app_hidden = resolve_hidden_uids_for_app(
                target_pkg,
                target_uid,
                app_cfg,
                templates_map,
                &all_packages,
                &system_packages,
            );

            if app_hidden.is_empty() {
                continue;
            }

            let mut content = format!("HMA_UID_3\n{target_uid}\n");
            for h_uid in app_hidden {
                content.push_str(&format!("{h_uid}\n"));
            }

            let uid_str = target_uid.to_string();
            let rule_path = gen_dir.join(&uid_str);
            if fs::write(&rule_path, content).is_ok() {
                synced_uids.insert(uid_str);
                synced_count += 1;
            }
        }
    }

    if let Ok(entries) = fs::read_dir(&gen_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !synced_uids.contains(name)
            {
                let _ = fs::remove_file(path);
            }
        }
    }

    let active_content = format!("{boot_id} {generation}\n");
    fs::write(&active_path, active_content)
        .with_context(|| format!("Failed to write {}", active_path.display()))?;

    sync_targeted_paths(is_enabled(), &config_obj, &all_packages, &system_packages);

    info!(
        "HMA hybrid isolation synced: {} target apps protected",
        synced_count
    );

    Ok(synced_count)
}
