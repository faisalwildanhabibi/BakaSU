use std::{path::Path, time::Instant};

use anyhow::{Context, Result};
use log::{error, info, warn};

use crate::{
    android::{
        dynamic_manager, feature, ksucalls,
        module::{self, ScriptWait, handle_updated_modules, metamodule, prune_modules},
        restorecon,
        utils::{self, is_safe_mode},
    },
    assets, defs,
};

pub fn on_post_fs_data() -> Result<()> {
    if let Err(e) = ksucalls::ensure_uapi_version_matched() {
        error!("{e:#}, skip on_post_fs_data");
        return Ok(());
    }

    ksucalls::report_post_fs_data();

    utils::umask(0);

    // Clear all temporary module configs early
    if let Err(e) = crate::android::module::module_config::clear_all_temp_configs() {
        warn!("clear temp configs failed: {e}");
    }

    let _ = catch_bootlog("logcat", &["logcat", "-b", "all"]);
    let _ = catch_bootlog("dmesg", &["dmesg", "-w", "-r"]);

    if utils::has_magisk() {
        warn!("Magisk detected, skip post-fs-data!");
        return Ok(());
    }

    let safe_mode = crate::android::utils::is_safe_mode();
    let wait = ScriptWait::Until(Instant::now() + defs::BOOT_STAGE_TIMEOUT);

    if safe_mode {
        // we should still ensure module directory exists in safe mode
        // because we may need to operate the module dir in safe mode
        warn!("safe mode, skip common post-fs-data.d scripts");
    } else {
        // Then exec common post-fs-data scripts
        if let Err(e) = crate::android::module::exec_common_scripts("post-fs-data.d", wait) {
            warn!("exec common post-fs-data scripts failed: {e}");
        }
        if let Err(e) = dynamic_manager::booted_load() {
            warn!("set dynamic manager failed: {e}");
        }
    }

    let module_dir = defs::MODULE_DIR;

    assets::ensure_binaries(true).with_context(|| "Failed to extract bin assets")?;

    crate::android::hot_install::clean_trash();

    // if we are in safe mode, we should disable all modules
    if safe_mode {
        warn!("safe mode, skip post-fs-data scripts and disable all modules!");
        if let Err(e) = module::disable_all_modules() {
            warn!("disable all modules failed: {e}");
        }
        return Ok(());
    }

    if let Err(e) = handle_updated_modules() {
        warn!("handle updated modules failed: {e}");
    }

    if let Err(e) = prune_modules() {
        warn!("prune modules failed: {e}");
    }

    // Refresh /metadata/watchdog/ksu/{modules.rc,preinit/} so the next boot's kernel hook sees the
    // current module set. Acts as a safety net when state was changed outside
    // of ksud's normal mutation commands.
    if let Err(e) = crate::android::module::regenerate_preinit() {
        warn!("regenerate preinit failed: {e}");
    }

    if let Err(e) = restorecon::restorecon() {
        warn!("restorecon failed: {e}");
    }

    // load sepolicy.rule
    if crate::android::module::load_sepolicy_rule().is_err() {
        warn!("load sepolicy.rule failed");
    }

    if let Err(e) = crate::android::profile::apply_sepolies() {
        warn!("apply root profile sepolicy failed: {e}");
    }

    feature::init_forced_features();

    // load feature config
    if is_safe_mode() {
        warn!("safe mode, skip load feature config");
    } else if let Err(e) = crate::android::feature::init_features() {
        warn!("init features failed: {e}");
    }

    // Load susfs config entries that must capture metadata before mounts/overlays.
    crate::android::susfs::init_event::on_post_fs_data();

    // execute metamodule post-fs-data script first (priority)
    if let Err(e) = metamodule::exec_stage_script("post-fs-data", wait) {
        warn!("exec metamodule post-fs-data script failed: {e}");
    }

    // exec modules post-fs-data scripts
    if let Err(e) = module::exec_stage_script("post-fs-data", wait) {
        warn!("exec post-fs-data scripts failed: {e}");
    }

    // load system.prop
    if let Err(e) = module::load_system_prop() {
        warn!("load system.prop failed: {e}");
    }

    // execute metamodule mount script
    if let Err(e) = metamodule::exec_mount_script(module_dir) {
        warn!("execute metamodule mount failed: {e}");
    }

    // Load umount config and apply to kernel
    if let Err(e) = crate::android::umount_config::load_umount_config() {
        warn!("load umount config failed: {e}");
    }

    run_stage("post-mount", wait);

    std::env::set_current_dir("/").with_context(|| "failed to chdir to /")?;

    Ok(())
}

pub fn run_stage(stage: &str, wait: ScriptWait) {
    utils::umask(0);

    if utils::has_magisk() {
        warn!("Magisk detected, skip {stage}");
        return;
    }

    if is_safe_mode() {
        warn!("safe mode, skip {stage} scripts");
        return;
    }

    if let Err(e) = module::exec_common_scripts(&format!("{stage}.d"), wait) {
        warn!("Failed to exec common {stage} scripts: {e}");
    }

    // execute metamodule stage script first (priority)
    if let Err(e) = metamodule::exec_stage_script(stage, wait) {
        warn!("Failed to exec metamodule {stage} script: {e}");
    }

    // execute regular modules stage scripts
    if let Err(e) = module::exec_stage_script(stage, wait) {
        warn!("Failed to exec {stage} scripts: {e}");
    }
}

pub fn on_services() {
    if let Err(e) = ksucalls::ensure_uapi_version_matched() {
        error!("{e:#}, skip on_services");
        return;
    }

    match ksucalls::report_services() {
        Ok(true) => {}
        Ok(false) => {
            info!("services already started, skipping");
            return;
        }
        Err(e) => {
            error!("Failed to report services: {e:#}");
            return;
        }
    }

    info!("on_services triggered!");
    automate_tailscale();
    run_stage("service", ScriptWait::NoWait);
}

fn automate_tailscale() {
    let tailscale_dir = Path::new("/data/adb/tailscale");
    let tailscaled_bin = tailscale_dir.join("tailscaled");
    if !tailscaled_bin.is_file() {
        return;
    }

    info!("tailscaled binary detected at /data/adb/tailscale, automating startup...");

    // 1. Ensure /dev/net/tun exists
    let dev_net = Path::new("/dev/net");
    let dev_tun = Path::new("/dev/tun");
    let dev_net_tun = Path::new("/dev/net/tun");
    if dev_tun.exists() && !dev_net_tun.exists() {
        let _ = std::fs::create_dir_all(dev_net);
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(dev_tun, dev_net_tun);
    }

    // 2. Spawn tailscaled if not already running
    let sock_path = tailscale_dir.join("tailscaled.sock");
    let state_path = tailscale_dir.join("tailscaled.state");
    let log_path = tailscale_dir.join("tailscaled.log");

    let is_running = std::process::Command::new("pidof")
        .arg("tailscaled")
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);

    if !is_running {
        // Clean stale socket file if daemon is not running
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }

        // Ensure proper permissions for TUN devices
        let _ = std::process::Command::new("chmod")
            .args(["0666", "/dev/net/tun", "/dev/tun"])
            .status();

        let mut cmd = std::process::Command::new(&tailscaled_bin);
        cmd.arg(format!("--state={}", state_path.display()))
            .arg(format!("--socket={}", sock_path.display()))
            .arg("--port=41641")
            .arg("--tun=tailscale0");

        if let Ok(log_file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
            cmd.stdout(log_file.try_clone().unwrap_or_else(|_| std::process::Stdio::null().into()));
            cmd.stderr(log_file.into());
        } else {
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
        }

        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                utils::switch_cgroups();
                Ok(())
            });
        }

        if let Ok(child) = cmd.spawn() {
            let pid = child.id();
            info!("tailscaled spawned with PID {pid}");

            let oom_path = format!("/proc/{pid}/oom_score_adj");
            let _ = std::fs::write(oom_path, "-1000");
        }
    }

    // 3. Optional auto-up if authkey or parameters are available
    let tailscale_cli = tailscale_dir.join("tailscale");
    let authkey_file = tailscale_dir.join("authkey.txt");
    if tailscale_cli.is_file() {
        let mut up_cmd = std::process::Command::new(&tailscale_cli);
        up_cmd.arg(format!("--socket={}", sock_path.display()))
            .arg("up");

        if let Ok(authkey) = std::fs::read_to_string(&authkey_file) {
            let key = authkey.trim();
            if !key.is_empty() {
                up_cmd.arg(format!("--auth-key={key}"));
            }
        }
        up_cmd.arg("--accept-routes");

        let _ = up_cmd.spawn();
    }
}

pub fn on_boot_completed() {
    if let Err(e) = ksucalls::ensure_uapi_version_matched() {
        error!("{e:#}, skip on_boot_completed");
        return;
    }

    ksucalls::report_boot_complete();
    info!("on_boot_completed triggered!");

    crate::android::preinit::on_boot_completed();

    run_stage("boot-completed", ScriptWait::NoWait);
    // Load susfs boot-completed
    if !is_safe_mode() {
        crate::android::susfs::init_event::on_boot_completed();
    }
}

fn catch_bootlog(logname: &str, command: &[&str]) -> Result<()> {
    use std::{os::unix::process::CommandExt, process::Stdio};

    let logdir = Path::new(defs::LOG_DIR);
    utils::ensure_dir_exists(logdir)?;
    let bootlog = logdir.join(format!("{logname}.log"));
    let oldbootlog = logdir.join(format!("{logname}.old.log"));

    if bootlog.exists() {
        std::fs::rename(&bootlog, oldbootlog)?;
    }

    let bootlog = std::fs::File::create(bootlog)?;

    let mut args = vec!["-s", "9", defs::BOOTLOG_TIMEOUT];
    args.extend_from_slice(command);
    // timeout -s 9 30s logcat > boot.log
    let result = unsafe {
        std::process::Command::new("timeout")
            .process_group(0)
            .pre_exec(|| {
                utils::switch_cgroups();
                Ok(())
            })
            .args(args)
            .stdout(Stdio::from(bootlog))
            .spawn()
    };

    if let Err(e) = result {
        warn!("Failed to start logcat: {e:#}");
    }

    Ok(())
}
