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
    if dir.is_dir() && dir.join("zygisk/arm64-v8a.so").exists() {
        return true;
    }
    let has_config_dir = dir.is_dir()
        || fs::read_dir("/data/misc")
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("hide_my_applist"))
            });
    has_config_dir && crate::android::builtin_zygisk::implementation_info().state == "running"
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
    let _ = sync_uid_scope();
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
    let _ = sync_uid_scope();
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

fn load_installed_packages(
    all_packages: &mut HashMap<String, u32>,
    system_packages: &mut HashSet<String>,
) {
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
                    let seinfo = parts.get(4).copied().unwrap_or("");
                    let installer = parts.last().copied().unwrap_or("");
                    let is_system_pkg = !is_user_range
                        || seinfo.contains(":partition=")
                        || seinfo.contains(":privapp")
                        || installer == "@system";
                    if is_system_pkg {
                        system_packages.insert(pkg_name.to_string());
                    }
                }
            }
        }
    }
}

fn get_root_granted_uids_and_packages() -> (HashSet<u32>, HashSet<String>) {
    let mut root_uids = HashSet::new();
    let mut root_pkgs = HashSet::new();

    if let Ok(allow_uids) = crate::android::ksucalls::get_allow_list() {
        root_uids.extend(allow_uids);
    }

    if let Ok(bytes) = fs::read("/data/adb/ksu/.allowlist") {
        if bytes.len() >= 8 {
            let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap_or([0; 4]));
            if magic == 0x4b53_5500 || magic == 0x7f4b_5355 {
                let mut offset = 8;
                const ENTRY_SIZE: usize = 784;
                while offset + ENTRY_SIZE <= bytes.len() {
                    let entry = &bytes[offset..offset + ENTRY_SIZE];
                    let key_bytes = &entry[4..260];
                    let key_len = key_bytes.iter().position(|&b| b == 0).unwrap_or(256);
                    if let Ok(key) = std::str::from_utf8(&key_bytes[..key_len]) {
                        let trimmed = key.trim();
                        if !trimmed.is_empty() {
                            let curr_uid = i32::from_le_bytes(
                                entry[260..264].try_into().unwrap_or([0; 4]),
                            );
                            let allow_su = entry[264] != 0;
                            if allow_su {
                                if curr_uid > 0 {
                                    root_uids.insert(curr_uid as u32);
                                }
                                root_pkgs.insert(trimmed.to_string());
                            }
                        }
                    }
                    offset += ENTRY_SIZE;
                }
            }
        }
    }

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

    for &pkg in PRESET_HIDDEN_PACKAGES {
        root_pkgs.insert(pkg.to_string());
    }

    (root_uids, root_pkgs)
}

pub fn status() -> HmaStatus {
    let installed = is_installed();
    let enabled = is_enabled();
    let version = String::from("177-UID3.1");

    let mut uid_scope_ready = false;
    let mut scoped_apps_count = 0;
    let mut active_generation = String::new();
    let (root_uids, _) = get_root_granted_uids_and_packages();
    let root_granted_hidden_count = root_uids.len();

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
        ("defaultConfig", serde_json::json!(null)),
        ("disabledHooks", serde_json::json!([])),
        ("settingsTemplates", serde_json::json!({})),
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

pub fn list_scopes_json() -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    let scope = cfg.get("scope").cloned().unwrap_or(serde_json::json!({}));
    Ok(serde_json::to_string_pretty(&scope)?)
}

pub fn get_scope_json(package: &str) -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    if let Some(app_cfg) = cfg.get("scope").and_then(|s| s.get(package)) {
        Ok(serde_json::to_string_pretty(app_cfg)?)
    } else {
        anyhow::bail!("Package {package} is not in HMA scope");
    }
}

pub fn clear_scopes() -> Result<()> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let mut cfg = read_or_init_config(&hma_dir);
    cfg["scope"] = serde_json::json!({});
    write_config(&hma_dir, &cfg)?;
    let _ = sync_uid_scope()?;
    Ok(())
}

pub fn check_scope(package: &str) -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    let app_cfg = cfg
        .get("scope")
        .and_then(|s| s.get(package))
        .with_context(|| format!("Package {package} is not in HMA scope"))?;

    let mut all_packages: HashMap<String, u32> = HashMap::new();
    let mut system_packages: HashSet<String> = HashSet::new();
    load_installed_packages(&mut all_packages, &mut system_packages);

    let target_uid = all_packages.get(package).copied().unwrap_or(0);
    let templates = cfg.get("templates").and_then(Value::as_object);

    let hidden_uids = resolve_hidden_uids_for_app(
        package,
        target_uid,
        app_cfg,
        templates,
        &all_packages,
        &system_packages,
    );
    let hidden_packages = resolve_hidden_packages_for_app(
        package,
        app_cfg,
        templates,
        &all_packages,
        &system_packages,
    );

    let mut hidden_pkgs_vec: Vec<String> = hidden_packages.into_iter().collect();
    hidden_pkgs_vec.sort();

    let settings_templates = cfg.get("settingsTemplates").and_then(Value::as_object);
    let mut applied_settings = serde_json::Map::new();
    if let Some(arr) = app_cfg.get("applySettingsPresets").and_then(Value::as_array) {
        for preset in arr.iter().filter_map(Value::as_str) {
            if let Some(st) = settings_templates.and_then(|m| m.get(preset)) {
                applied_settings.insert(preset.to_string(), st.clone());
            }
        }
    }
    if let Some(arr) = app_cfg.get("applySettingTemplates").and_then(Value::as_array) {
        for tpl in arr.iter().filter_map(Value::as_str) {
            if let Some(st) = settings_templates.and_then(|m| m.get(tpl)) {
                applied_settings.insert(tpl.to_string(), st.clone());
            }
        }
    }

    let result = serde_json::json!({
        "package": package,
        "uid": target_uid,
        "useWhitelist": app_cfg.get("useWhitelist").and_then(Value::as_bool).unwrap_or(false),
        "excludeSystemApps": app_cfg.get("excludeSystemApps").and_then(Value::as_bool).unwrap_or(true),
        "applyPresets": app_cfg.get("applyPresets").unwrap_or(&Value::Null),
        "applyTemplates": app_cfg.get("applyTemplates").unwrap_or(&Value::Null),
        "applySettingsPresets": app_cfg.get("applySettingsPresets").unwrap_or(&Value::Null),
        "applySettingTemplates": app_cfg.get("applySettingTemplates").unwrap_or(&Value::Null),
        "extraAppList": app_cfg.get("extraAppList").unwrap_or(&Value::Null),
        "hidden_uids_count": hidden_uids.len(),
        "hidden_uids": hidden_uids,
        "hidden_packages_count": hidden_pkgs_vec.len(),
        "hidden_packages": hidden_pkgs_vec,
        "effective_settings_overrides": applied_settings,
    });

    Ok(serde_json::to_string_pretty(&result)?)
}

pub fn list_templates_json() -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    let tpls = cfg.get("templates").cloned().unwrap_or(serde_json::json!({}));
    Ok(serde_json::to_string_pretty(&tpls)?)
}

pub fn get_template_json(name: &str) -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    if let Some(tpl) = cfg.get("templates").and_then(|t| t.get(name)) {
        Ok(serde_json::to_string_pretty(tpl)?)
    } else {
        anyhow::bail!("Template {name} not found");
    }
}

pub fn list_presets_json() -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    let app_presets = [
        "root_apps",
        "detector_apps",
        "sus_apps",
        "custom_rom",
        "shizuku_dhizuku",
        "xposed",
        "accessibility_apps",
    ];
    let settings_presets = ["accessibility", "input_method", "dev_options"];

    let mut result = serde_json::Map::new();
    let mut ap_map = serde_json::Map::new();
    if let Some(tpls) = cfg.get("templates").and_then(Value::as_object) {
        for &name in &app_presets {
            if let Some(val) = tpls.get(name) {
                ap_map.insert(name.to_string(), val.clone());
            }
        }
    }
    result.insert("app_presets".to_string(), Value::Object(ap_map));

    let mut sp_map = serde_json::Map::new();
    if let Some(sts) = cfg.get("settingsTemplates").and_then(Value::as_object) {
        for &name in &settings_presets {
            if let Some(val) = sts.get(name) {
                sp_map.insert(name.to_string(), val.clone());
            }
        }
    }
    result.insert("settings_presets".to_string(), Value::Object(sp_map));

    Ok(serde_json::to_string_pretty(&Value::Object(result))?)
}

pub fn get_preset_json(name: &str) -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    if let Some(tpl) = cfg.get("templates").and_then(|t| t.get(name)) {
        return Ok(serde_json::to_string_pretty(tpl)?);
    }
    if let Some(st) = cfg.get("settingsTemplates").and_then(|s| s.get(name)) {
        return Ok(serde_json::to_string_pretty(st)?);
    }
    anyhow::bail!("Preset {name} not found");
}

pub fn list_settings_templates_json() -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    let sts = cfg
        .get("settingsTemplates")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    Ok(serde_json::to_string_pretty(&sts)?)
}

pub fn get_settings_template_json(name: &str) -> Result<String> {
    let hma_dir = find_or_create_active_hma_dir()?;
    let cfg = read_or_init_config(&hma_dir);
    if let Some(st) = cfg.get("settingsTemplates").and_then(|s| s.get(name)) {
        Ok(serde_json::to_string_pretty(st)?)
    } else {
        anyhow::bail!("Settings template {name} not found");
    }
}

pub fn set_settings_template(name: &str, json_str: &str) -> Result<()> {
    let val: Value = serde_json::from_str(json_str)
        .with_context(|| format!("Invalid JSON for settings template {name}"))?;
    let hma_dir = find_or_create_active_hma_dir()?;
    let mut cfg = read_or_init_config(&hma_dir);
    if cfg.get("settingsTemplates").and_then(Value::as_object).is_none() {
        cfg["settingsTemplates"] = serde_json::json!({});
    }
    cfg["settingsTemplates"][name] = val;
    write_config(&hma_dir, &cfg)?;
    Ok(())
}

pub fn delete_settings_template(name: &str) -> Result<()> {
    let default_presets = ["accessibility", "input_method", "dev_options"];
    if default_presets.contains(&name) {
        anyhow::bail!("Cannot delete built-in settings preset {name}");
    }
    let hma_dir = find_or_create_active_hma_dir()?;
    let mut cfg = read_or_init_config(&hma_dir);
    if let Some(sts) = cfg.get_mut("settingsTemplates").and_then(Value::as_object_mut) {
        sts.remove(name);
    }
    write_config(&hma_dir, &cfg)?;
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
        let data_app_dirs = collect_data_app_dirs();

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
            let mut explicit_pkgs: Vec<String> = Vec::new();
            for item in raw_list {
                let trimmed_item = item.trim();
                if trimmed_item.is_empty() || trimmed_item.starts_with('#') {
                    continue;
                }
                if let Some(exc) = trimmed_item.strip_prefix('!') {
                    let trimmed = exc.trim();
                    if !trimmed.is_empty() {
                        exceptions.insert(trimmed.to_string());
                    }
                } else if trimmed_item.starts_with('/') {
                    explicit_paths.push(trimmed_item.to_string());
                } else {
                    explicit_pkgs.push(trimmed_item.to_string());
                }
            }

            let mut pkgs_to_hide: HashSet<String> = explicit_pkgs.into_iter().collect();
            if paths_cfg.auto_hide_app_paths {
                let hidden_pkgs = resolve_hidden_packages_for_app(
                    target_pkg,
                    app_cfg,
                    templates_map,
                    all_packages,
                    system_packages,
                );
                pkgs_to_hide.extend(hidden_pkgs);
            }

            for h_pkg in pkgs_to_hide {
                if exceptions.contains(&h_pkg) || h_pkg == *target_pkg {
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
                    format!("/data/media/0/Android/data/{h_pkg}"),
                ] {
                    if !exceptions.contains(&p) && Path::new(&p).exists() {
                        desired_rules.insert((target_uid, p));
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

    let mut all_packages: HashMap<String, u32> = HashMap::new();
    let mut system_packages: HashSet<String> = HashSet::new();

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
                    let seinfo = parts.get(4).copied().unwrap_or("");
                    let installer = parts.last().copied().unwrap_or("");
                    let is_system_pkg = !is_user_range
                        || seinfo.contains(":partition=")
                        || seinfo.contains(":privapp")
                        || installer == "@system";
                    if is_system_pkg {
                        system_packages.insert(pkg_name.to_string());
                    }
                }
            }
        }
    }

    let mut config_obj = read_or_init_config(&hma_dir);

    if config_obj
        .get("templates")
        .and_then(Value::as_object)
        .is_none()
    {
        config_obj["templates"] = serde_json::json!({});
    }
    if let Some(tpl_obj) = config_obj
        .get_mut("templates")
        .and_then(Value::as_object_mut)
    {
        tpl_obj.remove("baka_root_preset");
    }

    // Populate official HMA-OSS app presets from installed packages
    let mut root_apps_list: Vec<String> = Vec::new();
    let mut detector_apps_list: Vec<String> = Vec::new();
    let mut sus_apps_list: Vec<String> = Vec::new();
    let mut custom_rom_list: Vec<String> = Vec::new();
    let mut shizuku_list: Vec<String> = Vec::new();
    let mut xposed_list: Vec<String> = Vec::new();

    const DETECTOR_EXACT: &[&str] = &[
        "com.reveny.nativecheck",
        "icu.nullptr.nativetest",
        "io.github.rabehx.securify",
        "com.zhenxi.hunter",
        "io.github.vvb2060.mahoshojo",
        "io.github.huskydg.memorydetector",
        "org.akanework.checker",
        "icu.nullptr.applistdetector",
        "com.byxiaorun.detector",
        "com.anycheck.app",
        "com.joeykrim.rootcheck",
        "krypton.tbsafetychecker",
        "gr.nikolasspyr.integritycheck",
        "com.henrikherzig.playintegritychecker",
    ];
    const SUS_EXACT: &[&str] = &[
        "com.reveny.vbmetafix.service",
        "berserker.android.apps.sshdroid",
        "com.iamaner.oneclickfreeze",
        "com.shamanland.privatescreenshots",
        "org.connectbot",
        "com.anydesk.anydeskandroid",
        "com.speedsoftware.rootexplorer",
        "me.zhanghai.android.files",
        "com.lonelycatgames.Xplore",
        "org.fossify.filemanager",
        "com.amaze.filemanager",
    ];

    for (pkg, uid) in &all_packages {
        if root_granted_uids.contains(uid) || PRESET_HIDDEN_PACKAGES.contains(&pkg.as_str()) {
            root_apps_list.push(pkg.clone());
        }
        if DETECTOR_EXACT.contains(&pkg.as_str())
            || pkg.starts_with("me.garfieldhan.")
            || pkg.contains("chunqiu")
        {
            detector_apps_list.push(pkg.clone());
        }
        if SUS_EXACT.contains(&pkg.as_str())
            || pkg.starts_with("com.offsec.")
            || pkg.starts_with("com.termux")
            || pkg.starts_with("com.realvnc.")
            || pkg.starts_with("nextapp.fx")
        {
            sus_apps_list.push(pkg.clone());
        }
        if pkg.starts_with("lineageos.")
            || pkg.starts_with("org.lineageos.")
            || pkg.starts_with("com.caf.")
            || pkg.starts_with("org.calyxos.")
            || pkg.starts_with("co.aospa.")
            || pkg.starts_with("org.omnirom.")
            || pkg.starts_with("org.protonaosp.")
            || pkg.starts_with("org.evolution.")
            || pkg.starts_with("org.evolutionx.")
        {
            custom_rom_list.push(pkg.clone());
        }
        if pkg == "com.rosan.dhizuku" || pkg.starts_with("moe.shizuku.") {
            shizuku_list.push(pkg.clone());
        }
        if pkg == "org.frknkrc44.hma_oss" || pkg.contains("lsposed") || pkg.contains("xposed") {
            xposed_list.push(pkg.clone());
        }
    }
    for list in [
        &mut root_apps_list,
        &mut detector_apps_list,
        &mut sus_apps_list,
        &mut custom_rom_list,
        &mut shizuku_list,
        &mut xposed_list,
    ] {
        list.sort();
    }
    for (preset_name, list) in [
        ("root_apps", root_apps_list),
        ("detector_apps", detector_apps_list),
        ("sus_apps", sus_apps_list),
        ("custom_rom", custom_rom_list),
        ("shizuku_dhizuku", shizuku_list),
        ("xposed", xposed_list),
    ] {
        if !list.is_empty() {
            config_obj["templates"][preset_name] = serde_json::json!({
                "isWhitelist": false,
                "appList": list
            });
        }
    }

    // Populate official HMA-OSS settingsTemplates (Accessibility, Input Method, Dev Options)
    if config_obj
        .get("settingsTemplates")
        .and_then(Value::as_object)
        .is_none()
    {
        config_obj["settingsTemplates"] = serde_json::json!({});
    }
    if let Some(st_obj) = config_obj
        .get_mut("settingsTemplates")
        .and_then(Value::as_object_mut)
    {
        st_obj.entry("accessibility").or_insert_with(|| {
            serde_json::json!({
                "settingsList": [
                    { "name": "accessibility_enabled", "value": "0", "database": "secure" },
                    { "name": "enabled_accessibility_services", "value": "", "database": "secure" }
                ]
            })
        });
        st_obj.entry("input_method").or_insert_with(|| {
            serde_json::json!({
                "settingsList": [
                    {
                        "name": "default_input_method",
                        "value": "com.google.android.inputmethod.latin/com.android.inputmethod.latin.LatinIME",
                        "database": "secure"
                    },
                    { "name": "tts_default_synth", "value": "com.google.android.tts", "database": "secure" }
                ]
            })
        });
        st_obj.entry("dev_options").or_insert_with(|| {
            serde_json::json!({
                "settingsList": [
                    { "name": "adb_enabled", "value": "0", "database": "global" },
                    { "name": "adb_wifi_enabled", "value": "0", "database": "global" },
                    { "name": "development_settings_enabled", "value": "0", "database": "global" },
                    { "name": "hidden_api_policy", "value": null, "database": "global" },
                    { "name": "allow_mock_location", "value": "0", "database": "secure" }
                ]
            })
        });
    }

    const LAUNCHER_AND_SYSTEM_UI_EXCLUDES: &[&str] = &[
        "com.google.android.apps.nexuslauncher",
        "com.android.launcher3",
        "com.miui.home",
        "com.sec.android.app.launcher",
        "com.oppo.launcher",
        "com.oneplus.launcher",
        "com.android.settings",
    ];
    if config_obj.get("scope").and_then(Value::as_object).is_none() {
        config_obj["scope"] = serde_json::json!({});
    }
    if let Some(scope) = config_obj.get_mut("scope").and_then(Value::as_object_mut) {
        scope.retain(|pkg, app_cfg| {
            let uid = all_packages.get(pkg).copied().unwrap_or(0);
            if system_packages.contains(pkg)
                || root_granted_uids.contains(&uid)
                || PRESET_HIDDEN_PACKAGES.contains(&pkg.as_str())
                || LAUNCHER_AND_SYSTEM_UI_EXCLUDES.contains(&pkg.as_str())
            {
                return false;
            }
            if let Some(obj) = app_cfg.as_object_mut()
                && let Some(arr) = obj.get_mut("applyTemplates").and_then(Value::as_array_mut)
            {
                let before = arr.len();
                arr.retain(|v| v.as_str() != Some("baka_root_preset"));
                if arr.len() < before {
                    let has_other_rules = !arr.is_empty()
                        || obj
                            .get("applyPresets")
                            .and_then(Value::as_array)
                            .is_some_and(|a| !a.is_empty())
                        || obj
                            .get("extraAppList")
                            .and_then(Value::as_array)
                            .is_some_and(|a| !a.is_empty())
                        || obj
                            .get("extraOppositeAppList")
                            .and_then(Value::as_array)
                            .is_some_and(|a| !a.is_empty())
                        || obj
                            .get("applySettingTemplates")
                            .and_then(Value::as_array)
                            .is_some_and(|a| !a.is_empty())
                        || obj
                            .get("applySettingsPresets")
                            .and_then(Value::as_array)
                            .is_some_and(|a| !a.is_empty());
                    if !has_other_rules {
                        return false;
                    }
                }
            }
            true
        });
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

    let prev_gen_num = fs::read_to_string(&active_path).ok().and_then(|s| {
        let mut parts = s.split_whitespace();
        let prev_boot = parts.next()?;
        let prev_gen = parts.next()?;
        if prev_boot == boot_id {
            prev_gen.strip_prefix('g')?.parse::<u64>().ok()
        } else {
            None
        }
    });
    let next_gen_num = prev_gen_num.map_or(0, |n| n + 1);
    let generation = format!("g{next_gen_num}");
    let gen_dir = uid_scope_dir.join(&generation);
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

    if let Ok(entries) = fs::read_dir(&uid_scope_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with('g')
                && name != generation
            {
                let _ = fs::remove_dir_all(path);
            }
        }
    }

    sync_targeted_paths(is_enabled(), &config_obj, &all_packages, &system_packages);

    info!(
        "HMA hybrid isolation synced: {} target apps protected",
        synced_count
    );

    Ok(synced_count)
}
